use bigdecimal::{FromPrimitive, RoundingMode, ToPrimitive, Zero};
use std::cmp::Ordering;

use crate::number::{arithmetic, normalize, parse_number};
use crate::parser::{self, Binary, Comparison, Expr, Fragment, Instruction};
use crate::runtime::{Key, Outcome, Place, Runtime};
use crate::{BigDecimal, Position, Resource, Value};

pub(crate) enum Operand {
    Literal(Value),
    Actor(Place),
    Void,
}

impl Operand {
    pub fn value(&self, pos: Position) -> Outcome<Value> {
        match self {
            Self::Actor(actor) => actor.read(pos),
            Self::Literal(value) => Ok(value.clone()),
            Self::Void => Err(pos.error("la fonction ne retourne aucune valeur")),
        }
    }

    pub fn as_place(self, pos: Position) -> Outcome<Place> {
        match self {
            Self::Actor(place) => Ok(place),
            _ => {
                let v = self.value(pos)?;
                let role = v.role();
                Ok(Place::new(v, role))
            }
        }
    }

    pub fn place(self, pos: Position) -> Outcome<Place> {
        match self {
            Self::Actor(p) => Ok(p),
            _ => Err(pos.error("acteur modifiable attendu")),
        }
    }
}

impl Runtime {
    pub fn values(&mut self, expressions: &[Expr]) -> Outcome<Vec<Value>> {
        expressions.iter().map(|e| self.eval(e)).collect()
    }

    pub fn arguments(&mut self, expressions: &[Expr]) -> Outcome<Vec<Place>> {
        expressions
            .iter()
            .map(|e| self.operand(e)?.as_place(position(e)))
            .collect()
    }

    pub fn eval(&mut self, expression: &Expr) -> Outcome<Value> {
        self.operand(expression)?.value(position(expression))
    }

    pub fn operand(&mut self, expr: &Expr) -> Outcome<Operand> {
        let mut stack = Vec::<Operand>::new();
        for instruction in expr {
            let value = match instruction {
                Instruction::Literal(v) => Operand::Literal(v.clone()),
                Instruction::Actor(name, p) => self.actor(name, *p)?,
                Instruction::NameOf(name, p) => {
                    self.actor(name, *p)?;
                    Operand::Literal(Value::Text(name.clone()))
                }
                Instruction::Alternatives(_, p) => {
                    return Err(p.error("groupe hors d’une instruction"));
                }
                Instruction::Indirect(e, p) => {
                    let Value::Text(source) = self.eval(e)? else {
                        return Err(p.error("nom d’acteur attendu"));
                    };
                    p.check_depth(self.indirection)?;
                    self.indirection += 1;
                    let result =
                        parser::parse_expression(&source).and_then(|expr| self.operand(&expr));
                    self.indirection -= 1;
                    result?
                }
                Instruction::Interpolate(fragments) => {
                    let mut text = Vec::new();
                    for part in fragments {
                        match part {
                            Fragment::Text(s) => text.extend(s.encode_utf16()),
                            Fragment::Expression(source, raw) => {
                                match parser::parse_expression(source).and_then(|e| self.eval(&e)) {
                                    Ok(v) => v.append_utf16(&mut text),
                                    Err(_) => text.extend(raw.encode_utf16()),
                                }
                            }
                        }
                    }
                    Operand::Literal(Value::from_utf16_units(text))
                }
                Instruction::Binary(op, p) => {
                    let right = stack.pop().unwrap().value(*p)?;
                    let left = stack.pop().unwrap().value(*p)?;
                    Operand::Literal(binary(*op, left, right, *p, self.precision)?)
                }
                Instruction::Unary(op, p) => {
                    let arg = stack.pop().unwrap().value(*p)?;
                    Operand::Literal(self.builtin(op, &[arg], *p)?)
                }
                Instruction::Casier(items, p) => {
                    let values = self.arguments(items)?;
                    let place = Place::literal(Value::Casier(vec![]));
                    place.set_items(values, *p)?;
                    Operand::Actor(place)
                }
                Instruction::Select(indices, p) => {
                    let mut base = stack.pop().unwrap();
                    for index in indices {
                        let i = index_of(&self.eval(index)?, *p)?;
                        base = match base.value(*p)? {
                            Value::Text(_) | Value::TextUnits(_) => {
                                let units = base.value(*p)?.utf16_units().unwrap();
                                let unit =
                                    *units.get(i).ok_or_else(|| p.error("indice hors limites"))?;
                                Operand::Literal(Value::from_utf16_units(vec![unit]))
                            }
                            Value::Casier(_) => {
                                Operand::Actor(base.as_place(*p)?.child(Key::Index(i)))
                            }
                            _ => return Err(p.error("casier ou texte attendu")),
                        };
                    }
                    base
                }
                Instruction::Attribute(name, p) => Operand::Actor(
                    stack
                        .pop()
                        .unwrap()
                        .as_place(*p)?
                        .child(Key::Attribute(name.clone())),
                ),
                Instruction::Call(name, args, p) if matches!(name.as_str(), "ter" | "ternaire") => {
                    let [test, yes, no] = args.as_slice() else {
                        return Err(p.error("trois paramètres attendus"));
                    };
                    let result = self.eval(test)?.number(*p)?.with_scale(0) > BigDecimal::from(0);
                    Operand::Literal(self.eval(if result { yes } else { no })?)
                }
                Instruction::Call(name, args, p) if parser::is_builtin(name) => {
                    let values = self.values(args)?;
                    Operand::Literal(self.builtin(name, &values, *p)?)
                }
                Instruction::Call(name, args, p) => {
                    let args = self.arguments(args)?;
                    self.call(name, args, *p)?
                        .map_or(Operand::Void, Operand::Literal)
                }
                Instruction::Method(name, args, p) => {
                    let receiver = stack.pop().unwrap().as_place(*p)?;
                    let args = self.arguments(args)?;
                    self.method(receiver, name, args, *p)?
                        .map_or(Operand::Void, Operand::Literal)
                }
            };
            stack.push(value);
        }
        Ok(stack.pop().expect("expression has a result"))
    }

    fn actor(&mut self, name: &str, pos: Position) -> Outcome<Operand> {
        if let Some(place) = self.find(name) {
            return Ok(Operand::Actor(place));
        }
        if let Some(rest) = name.strip_prefix("joker")
            && rest.len() <= 3
            && rest.bytes().all(|c| c == 39)
        {
            return self
                .jokers
                .iter()
                .rev()
                .nth(rest.len())
                .cloned()
                .map(Operand::Actor)
                .ok_or_else(|| pos.error("aucun joker dans cette boucle"));
        }
        if name == "création" {
            return self
                .created
                .as_ref()
                .filter(|p| !p.is_removed())
                .cloned()
                .map(Operand::Actor)
                .ok_or_else(|| pos.error("aucune création"));
        }
        let value = match name {
            "vrai" | "faux" => Value::flag(name == "vrai"),
            "pi" => Value::Number(parse_number(
                "3.1415926535897932384626433832795028841971693993751058209749445923078164062862089986280348253421170679",
                pos,
            )?),
            "euler" => Value::Number(parse_number(
                "2.7182818284590452353602874713526624977572470936999595749669676277240766303535475945713821785251664274",
                pos,
            )?),
            "version" => Value::Text("π".into()),
            "spécification" => Value::Text("3".into()),
            "auteur" => Value::Text("Mounès Ronan (http://codevallee.fr)".into()),
            "livre" => Value::Text(self.title.clone()),
            _ => {
                let resource = system_resource(name)
                    .ok_or_else(|| pos.error(format!("acteur « {name} » inconnu")))?;
                self.service(resource, name, &[], pos)?
                    .ok_or_else(|| pos.error("valeur système indisponible"))?
            }
        };
        Ok(Operand::Literal(value))
    }

    pub fn check_name(&self, name: &str, pos: Position) -> Outcome<()> {
        if system_resource(name).is_some()
            || matches!(
                name,
                "vrai"
                    | "faux"
                    | "joker"
                    | "joker'"
                    | "joker''"
                    | "joker'''"
                    | "création"
                    | "pi"
                    | "euler"
                    | "version"
                    | "spécification"
                    | "auteur"
                    | "livre"
            )
        {
            return Err(pos.error(format!("« {name} » est un acteur prédéfini")));
        }
        Ok(())
    }

    fn angle_result(&self, n: f64) -> f64 {
        if self.radians { n } else { n.to_degrees() }
    }

    fn builtin(&mut self, name: &str, args: &[Value], pos: Position) -> Outcome<Value> {
        if let [cond, yes, no] = args
            && matches!(name, "ter" | "ternaire")
        {
            return Ok(if cond.number(pos)?.is_zero() { no } else { yes }.clone());
        }
        if let [y, x] = args
            && name == "atan2"
        {
            return decimal_float(self.angle_result(float(y, pos)?.atan2(float(x, pos)?)), pos);
        }
        let [arg] = args else {
            return Err(pos.error("nombre de paramètres incorrect"));
        };
        match (name, arg) {
            ("clone", _) => return Ok(arg.clone()),
            ("ascii", value) if value.is_text() => {
                return Ok(Value::Number(
                    value
                        .utf16_units()
                        .unwrap()
                        .first()
                        .copied()
                        .unwrap_or(32)
                        .into(),
                ));
            }
            ("unicode", value) if value.is_text() => {
                let text = value
                    .text_utf8()
                    .ok_or_else(|| pos.error("code Unicode invalide"))?;
                let n = i32::from_str_radix(&text, 16)
                    .map_err(|_| pos.error("code Unicode invalide"))?;
                return Ok(Value::from_utf16_units(vec![n as u16]));
            }
            ("hasard", _) => {
                return self
                    .service(Resource::Random, name, args, pos)?
                    .ok_or_else(|| pos.error("nombre aléatoire attendu"));
            }
            _ => {}
        }
        let n = arg.number(pos)?;
        let answer = match name {
            "-" => -n,
            "+" => n.clone(),
            "entier" => n.with_scale(0),
            "décimal" => n - n.with_scale(0),
            "arrondi" => n.with_scale_round(0, RoundingMode::HalfDown),
            "abs" => n.abs(),
            "carré" => n * n,
            "cube" => n * n * n,
            "non" => return Ok(Value::flag(n.with_scale(0).is_zero())),
            "chr" => {
                let code = n.with_scale(0) % BigDecimal::from(1u32 << 16);
                return Ok(Value::from_utf16_units(vec![
                    code.to_i64().expect("remainder fits in an i64") as u16,
                ]));
            }
            _ => {
                let x = float(arg, pos)?;
                let angle = if self.radians { x } else { x.to_radians() };
                let result = match name {
                    "sin" => angle.sin(),
                    "cos" if !self.radians && x % 90.0 == 0.0 => angle.cos().round(),
                    "cos" if !self.radians && x % 60.0 == 0.0 => angle.cos().floor() + 0.5,
                    "cos" => angle.cos(),
                    "tan" if !self.radians && x % 45.0 == 0.0 => (angle.tan() + 0.5).floor(),
                    "tan" => angle.tan(),
                    "asin" => self.angle_result(x.asin()),
                    "acos" => self.angle_result(x.acos()),
                    "atan" => self.angle_result(x.atan()),
                    "log" => x.log10(),
                    "logn" => x.ln(),
                    "racine" => x.sqrt(),
                    "exp" => x.exp(),
                    _ => return Err(pos.error(format!("opération « {name} » inconnue"))),
                };
                return decimal_float(result, pos);
            }
        };
        Ok(Value::Number(normalize(answer, pos)?))
    }
}

fn system_resource(name: &str) -> Option<Resource> {
    Some(match name {
        "année" | "mois" | "jour" | "heure" | "minute" | "seconde" => Resource::Clock,
        "polices" | "sourisx" | "sourisy" | "ecranv" | "ecranh" | "couleurs" | "touche" => {
            Resource::Graphics
        }
        "'do'" | "'re'" | "'mi'" | "'fa'" | "'sol'" | "'la'" | "'si'" => Resource::Media,
        _ => return None,
    })
}

pub(crate) fn position(e: &Expr) -> Position {
    e.iter()
        .find_map(|i| match i {
            Instruction::Literal(_) | Instruction::Interpolate(_) => None,
            Instruction::Alternatives(_, p)
            | Instruction::NameOf(_, p)
            | Instruction::Indirect(_, p)
            | Instruction::Actor(_, p)
            | Instruction::Unary(_, p)
            | Instruction::Binary(_, p)
            | Instruction::Call(_, _, p)
            | Instruction::Casier(_, p)
            | Instruction::Select(_, p)
            | Instruction::Attribute(_, p)
            | Instruction::Method(_, _, p) => Some(*p),
        })
        .unwrap_or(Position { line: 1, column: 1 })
}

fn float(value: &Value, pos: Position) -> Outcome<f64> {
    value
        .number(pos)?
        .to_f64()
        .ok_or_else(|| pos.error("nombre hors limites"))
}

fn decimal_float(n: f64, pos: Position) -> Outcome<Value> {
    BigDecimal::from_f64(n)
        .map(Value::Number)
        .ok_or_else(|| pos.error("résultat non fini"))
}

pub(crate) fn index_of(value: &Value, pos: Position) -> Outcome<usize> {
    value
        .number(pos)?
        .to_usize()
        .ok_or_else(|| pos.error("indice hors limites"))
}

pub(crate) fn equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (a, b) if a.is_text() && b.is_text() => {
            crate::text::lower(&a.utf16_units().unwrap())
                == crate::text::lower(&b.utf16_units().unwrap())
        }
        (Value::Casier(a), Value::Casier(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (
            Value::Species {
                name: a,
                attributes: aa,
            },
            Value::Species {
                name: b,
                attributes: bb,
            },
        ) => {
            a == b
                && aa.len() == bb.len()
                && aa
                    .iter()
                    .all(|(k, v)| bb.iter().any(|(key, val)| key == k && equal(v, val)))
        }
        _ => left == right,
    }
}

pub(crate) fn order(left: &Value, right: &Value, pos: Position) -> Outcome<Ordering> {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => Ok(a.cmp(b)),
        (a, b) if a.is_text() && b.is_text() => {
            Ok(a.utf16_units().unwrap().cmp(&b.utf16_units().unwrap()))
        }
        _ => Err(pos.error("deux nombres ou deux textes sont attendus")),
    }
}

pub(crate) fn binary(
    op: Binary,
    left: Value,
    right: Value,
    pos: Position,
    precision: Option<u64>,
) -> Outcome<Value> {
    match op {
        Binary::Compare(c) => {
            if left.role() != right.role() {
                return Err(pos.error("rôles incompatibles"));
            }
            return Ok(Value::flag(match c {
                Comparison::Equal => equal(&left, &right),
                Comparison::Unequal => !equal(&left, &right),
                Comparison::Greater => order(&left, &right, pos)?.is_gt(),
                Comparison::Less => order(&left, &right, pos)?.is_lt(),
                Comparison::LessEqual => order(&left, &right, pos)?.is_le() || equal(&left, &right),
                Comparison::GreaterEqual => {
                    order(&left, &right, pos)?.is_ge() || equal(&left, &right)
                }
            }));
        }
        Binary::And | Binary::Or | Binary::Xor => {
            let a = !left.number(pos)?.with_scale(0).is_zero();
            let b = !right.number(pos)?.with_scale(0).is_zero();
            return Ok(Value::flag(match op {
                Binary::And => a && b,
                Binary::Or => a || b,
                _ => a ^ b,
            }));
        }
        Binary::LeftShift | Binary::RightShift => {
            let a = left
                .number(pos)?
                .to_i64()
                .ok_or_else(|| pos.error("entier hors limites"))? as i32;
            let b = right
                .number(pos)?
                .to_i64()
                .ok_or_else(|| pos.error("entier hors limites"))? as u32;
            return Ok(Value::Number(
                if matches!(op, Binary::LeftShift) {
                    a.wrapping_shl(b)
                } else {
                    a.wrapping_shr(b)
                }
                .into(),
            ));
        }
        Binary::Join => return Ok(Value::Casier(vec![left, right])),
        Binary::Add
            if (left.is_text()
                && matches!(
                    right,
                    Value::Text(_) | Value::TextUnits(_) | Value::Number(_)
                ))
                || (right.is_text() && matches!(left, Value::Number(_))) =>
        {
            let mut output = Vec::new();
            left.append_utf16(&mut output);
            right.append_utf16(&mut output);
            return Ok(Value::from_utf16_units(output));
        }
        Binary::Multiply => {
            if let (Some(text), Value::Number(count)) = (left.utf16_units(), &right)
                && left.is_text()
            {
                return repeat_text(text, count, pos);
            }
            if let (Value::Number(count), Some(text)) = (&left, right.utf16_units())
                && right.is_text()
            {
                return repeat_text(text, count, pos);
            }
        }
        _ => {}
    }
    arithmetic(op, left.number(pos)?, right.number(pos)?, pos, precision).map(Value::Number)
}

fn repeat_text(text: Vec<u16>, count: &BigDecimal, pos: Position) -> Outcome<Value> {
    let count = if count <= &BigDecimal::from(0) {
        0
    } else {
        count
            .to_usize()
            .ok_or_else(|| pos.error("texte trop long"))?
    };
    let size = text
        .len()
        .checked_mul(count)
        .ok_or_else(|| pos.error("texte trop long"))?;
    let mut out = Vec::new();
    out.try_reserve_exact(size)
        .map_err(|_| pos.error("texte trop long"))?;
    if !text.is_empty() {
        for _ in 0..count {
            out.extend_from_slice(&text);
        }
    }
    Ok(Value::from_utf16_units(out))
}
