use crate::eval::{index_of, order};
use crate::parser::{self, Expr, Instruction};
use crate::runtime::{Outcome, Place, Runtime};
use crate::{Host, Position, Resource, Value};
use bigdecimal::ToPrimitive;

impl Runtime {
    pub fn dispatch(
        &mut self,
        resource: Option<Resource>,
        op: &str,
        args: &[Expr],
        pos: Position,
    ) -> Outcome<()> {
        if let Some((index, choices)) =
            args.iter()
                .enumerate()
                .rev()
                .find_map(|(i, e)| match e.as_slice() {
                    [Instruction::Alternatives(group, _)] => Some((i, group)),
                    _ => None,
                })
        {
            let mut selected = args.to_vec();
            for e in choices {
                selected[index] = e.clone();
                self.dispatch(resource, op, &selected, pos)?;
            }
            return Ok(());
        }
        match resource {
            Some(r) => self.host_action(r, op, args, pos),
            None => self.command(op, args, pos),
        }
    }

    pub fn host_action(
        &mut self,
        resource: Resource,
        op: &str,
        args: &[Expr],
        pos: Position,
    ) -> Outcome<()> {
        let operands = args
            .iter()
            .map(|arg| self.operand(arg))
            .collect::<Outcome<Vec<_>>>()?;
        let values = operands
            .iter()
            .map(|v| v.value(pos))
            .collect::<Outcome<Vec<_>>>()?;
        if op == "questionne" {
            let [value, Value::Text(prompt)] = values.as_slice() else {
                return Err(pos.error("question invalide"));
            };
            let [crate::eval::Operand::Actor(place), _] = operands.as_slice() else {
                return Err(pos.error("acteur attendu"));
            };
            self.host
                .write(format!("{prompt}\n").as_bytes())
                .map_err(|e| pos.error(e.to_string()))?;
            if let Some(raw) = self
                .host
                .read(
                    &place.reference().map_or_else(String::new, |r| r.name),
                    value,
                )
                .map_err(|e| pos.error(e.to_string()))?
            {
                let result = match value {
                    Value::Number(_) => Value::Number(crate::number::parse_number(&raw, pos)?),
                    Value::Text(_) => Value::Text(raw),
                    _ => return Err(pos.error("nombre ou texte attendu")),
                };
                place.write(result, pos)?;
            }
            return Ok(());
        }
        if resource == Resource::Clock && matches!(op, "attends" | "temporise") {
            let [Value::Number(amount), Value::Text(unit)] = values.as_slice() else {
                self.host
                    .action(resource, op, &values)
                    .map_err(|e| pos.error(e.to_string()))?;
                return Ok(());
            };
            let seconds = amount.to_f64().ok_or_else(|| pos.error("durée invalide"))?
                / if unit == "millisecondes" { 1000.0 } else { 1.0 };
            let duration = std::time::Duration::try_from_secs_f64(seconds)
                .map_err(|_| pos.error("durée invalide"))?;
            let mut rest = self
                .host
                .delay(duration)
                .map_err(|e| pos.error(e.to_string()))?;
            while !rest.is_zero() && !self.host.cancelled() {
                let part = rest.min(std::time::Duration::from_millis(50));
                std::thread::sleep(part);
                rest -= part;
            }
            return Ok(());
        }
        let actors: Vec<_> = operands
            .iter()
            .map(|o| match o {
                crate::eval::Operand::Actor(p) => p.reference(),
                _ => None,
            })
            .collect();
        let changes = self
            .host
            .bound_action(resource, op, &values, &actors)
            .map_err(|e| pos.error(e.to_string()))?;
        for (index, value) in changes {
            let operand = operands
                .get(index)
                .ok_or_else(|| pos.error("indice de paramètre du service invalide"))?;
            if let crate::eval::Operand::Actor(place) = operand {
                place.write(value, pos)?;
            } else {
                return Err(pos.error("le service doit modifier un acteur"));
            }
        }
        Ok(())
    }

    pub fn command(&mut self, op: &str, args: &[Expr], pos: Position) -> Outcome<()> {
        match op {
            "annihile" | "recharge" => {
                let [expr] = args else {
                    return Err(pos.error("un acteur attendu"));
                };
                let place = self.target(expr, pos)?;
                return if op == "annihile" {
                    self.remove_actor(&place, pos)
                } else {
                    place.reload(pos)
                };
            }
            "fusionne" => return self.host_action(Resource::Graphics, op, args, pos),
            "modifie" => return self.host_action(Resource::Plugin, op, args, pos),

            "attache" => return self.attach(args, pos),
            _ => {}
        }
        let operands = args
            .iter()
            .map(|e| self.operand(e))
            .collect::<Outcome<Vec<_>>>()?;
        let values = operands
            .iter()
            .map(|o| o.value(pos))
            .collect::<Outcome<Vec<_>>>()?;
        if op == "ajoute"
            && matches!(
                values.as_slice(),
                [Value::Species { .. }, Value::Species { .. }]
            )
        {
            let actors: Vec<_> = operands
                .iter()
                .map(|o| {
                    if let crate::eval::Operand::Actor(p) = o {
                        p.reference()
                    } else {
                        None
                    }
                })
                .collect();
            self.host
                .bound_action(Resource::Plugin, op, &values, &actors)
                .map_err(|e| pos.error(e.to_string()))?;
            return Ok(());
        }
        if let Some(i) = match (op, values.as_slice()) {
            ("ajoute" | "ôte" | "ote" | "supprime", [_, Value::Casier(_)]) => Some(1),
            ("insère", [_, _, Value::Casier(_)]) => Some(2),
            ("trie" | "inverse" | "mélange", [Value::Casier(_)]) => Some(0),
            _ => None,
        } {
            let crate::eval::Operand::Actor(target) = &operands[i] else {
                return Err(pos.error("casier modifiable attendu"));
            };
            let mut items = target.items(pos)?;
            let first = match &operands[0] {
                crate::eval::Operand::Actor(p) => p.clone(),
                _ => Place::literal(values[0].clone()),
            };
            match op {
                "ajoute" => items.push(first),
                "insère" => {
                    let n = one_based(&values[1], pos)?;
                    if n > items.len() {
                        return Err(pos.error("indice hors limites"));
                    }
                    items.insert(n, first);
                }
                "ôte" | "ote" | "supprime" => {
                    let mut found = None;
                    for (idx, item) in items.iter().enumerate() {
                        if item.same(&first, pos)? {
                            found = Some(idx);
                            break;
                        }
                    }
                    if let Some(idx) = found {
                        items.remove(idx);
                    }
                }
                "inverse" => items.reverse(),
                "mélange" => self.shuffle(&mut items, pos)?,
                "trie" => {
                    let mut keys = items
                        .into_iter()
                        .map(|p| Ok((p.read(pos)?, p)))
                        .collect::<Outcome<Vec<_>>>()?;
                    keys.sort_by(|(a, _), (b, _)| {
                        order(a, b, pos).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    items = keys.into_iter().map(|(_, p)| p).collect();
                }
                _ => unreachable!(),
            }
            return target.set_items(items, pos);
        }
        let (index, value) = match (op, values.as_slice()) {
            ("nomme", [Value::Text(name)]) => {
                self.title.clone_from(name);
                return Ok(());
            }
            ("observe", [Value::Text(name)]) => return self.observe(Some(name)),
            ("~", [Value::Text(line)]) => {
                let (key, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
                match key {
                    "précision" => {
                        self.precision = Some(
                            value
                                .trim()
                                .parse::<u64>()
                                .ok()
                                .filter(|n| *n <= i32::MAX as u64)
                                .ok_or_else(|| pos.error("précision hors limites"))?,
                        );
                    }
                    "angle" => self.radians = value.trim().eq_ignore_ascii_case("radian"),
                    _ => {
                        self.service(Resource::System, "~", &values, pos)?;
                    }
                }
                return Ok(());
            }
            ("fais le café", []) => return Err(pos.error("impossible de faire le café")),
            ("provoque", [Value::Number(code), Value::Text(message)])
                if code >= &crate::BigDecimal::from(10100) =>
            {
                return Err(pos.error(format!("{code} : {message}")));
            }
            ("vide", [Value::Casier(_)]) => (0, Value::Casier(Vec::new())),
            ("vide", [Value::Text(_)]) => (0, Value::Text(String::new())),
            ("vide", [Value::Number(_)]) => (0, Value::Number(0.into())),
            ("inverse", [Value::Text(s)]) => (0, Value::Text(s.chars().rev().collect())),
            ("inverse" | "mélange", [Value::Number(_)]) => {
                let value = self
                    .service(Resource::Random, "hasard", &values, pos)?
                    .ok_or_else(|| pos.error("nombre aléatoire attendu"))?;
                (0, value)
            }
            ("mélange", [Value::Text(s)]) => {
                let mut chars: Vec<_> = s.chars().collect();
                self.shuffle(&mut chars, pos)?;
                (0, Value::Text(chars.into_iter().collect()))
            }
            ("copie", [from, _]) => (1, from.clone()),
            ("convertis", [Value::Text(s), Value::Number(_)]) => (
                1,
                Value::Number(crate::number::parse_number(s, pos).unwrap_or_else(|_| 0.into())),
            ),
            ("convertis", [Value::Number(n), Value::Text(_)]) => {
                (1, Value::Text(crate::number::format_number(n)))
            }
            ("mesure", [Value::Text(s), Value::Number(_)]) => {
                (1, Value::Number((s.encode_utf16().count() as u64).into()))
            }
            ("mesure", [Value::Casier(c), Value::Number(_)]) => {
                (1, Value::Number((c.len() as u64).into()))
            }
            ("concatène", [parts @ .., Value::Text(_)]) if !parts.is_empty() => {
                if parts
                    .iter()
                    .any(|v| !matches!(v, Value::Text(_) | Value::Number(_)))
                {
                    return Err(pos.error("texte ou nombre attendu"));
                }
                (
                    parts.len(),
                    Value::Text(parts.iter().map(ToString::to_string).collect()),
                )
            }
            ("ajoute", [Value::Number(a), Value::Number(b)]) => (1, Value::Number(a + b)),
            ("ajoute", [a, Value::Text(s)]) => (1, Value::Text(format!("{s}{a}"))),
            ("soustrais", [Value::Number(a), Value::Number(b)]) => (1, Value::Number(b - a)),
            ("multiplie", [Value::Number(a), Value::Number(b)]) => (0, Value::Number(a * b)),
            ("divise", [Value::Number(a), Value::Number(b)]) => (
                0,
                Value::Number(crate::number::divide_verb(a, b, self.precision, pos)?),
            ),
            ("incrémente" | "décrémente", [Value::Number(n)]) => (
                0,
                Value::Number(n + if op == "incrémente" { 1 } else { -1 }),
            ),
            ("cherche", [Value::Number(_), needle, Value::Casier(items)]) => {
                let index = items
                    .iter()
                    .position(|v| crate::eval::equal(v, needle))
                    .map_or(0, |i| i + 1);
                (0, Value::Number((index as u64).into()))
            }
            ("cherche", [Value::Number(_), Value::Text(n), Value::Text(s)]) => {
                let s = s.to_lowercase();
                let index = s
                    .find(&n.to_lowercase())
                    .map(|byte| s[..byte].encode_utf16().count());
                (0, Value::Number(index.map_or(-1, |i| i as i64).into()))
            }
            ("insère", [Value::Text(v), n, Value::Text(s)]) => {
                let mut units: Vec<_> = s.encode_utf16().collect();
                let i = one_based(n, pos)?;
                if i > units.len() {
                    return Err(pos.error("indice hors limites"));
                }
                units.splice(i..i, v.encode_utf16());
                (2, Value::Text(String::from_utf16_lossy(&units)))
            }
            ("extrais", [source, a, b, _]) => {
                let range = one_based(a, pos)?..index_of(b, pos)?;
                let value = match source {
                    Value::Text(s) => {
                        let u: Vec<_> = s.encode_utf16().collect();
                        Value::Text(String::from_utf16_lossy(
                            u.get(range)
                                .ok_or_else(|| pos.error("indices hors limites"))?,
                        ))
                    }
                    Value::Casier(c) => Value::Casier(
                        c.get(range)
                            .ok_or_else(|| pos.error("indices hors limites"))?
                            .to_vec(),
                    ),
                    _ => return Err(pos.error("texte ou casier attendu")),
                };
                (3, value)
            }
            ("évalue" | "evalue", [Value::Text(s)]) => {
                let value = self.eval(&parser::parse_expression(s)?)?;
                (0, Value::Text(value.to_string()))
            }
            _ => return Err(pos.error(format!("paramètres invalides pour « {op} »"))),
        };
        let crate::eval::Operand::Actor(place) = &operands[index] else {
            return Err(pos.error("acteur modifiable attendu"));
        };
        place.write(value, pos)
    }

    pub fn internal_method(
        &mut self,
        name: &str,
        values: &[Value],
        pos: Position,
    ) -> Outcome<Option<Value>> {
        let result = match (name, values) {
            ("minuscule", [Value::Text(s)]) => Value::Text(s.to_lowercase()),
            ("majuscule", [Value::Text(s)]) => Value::Text(s.to_uppercase()),
            ("inverser", [Value::Text(s)]) => Value::Text(s.chars().rev().collect()),
            ("capitaliser", [Value::Text(s)]) => {
                let words = s
                    .split(char::is_whitespace)
                    .map(|word| {
                        let lower = word.to_lowercase();
                        let mut chars = lower.chars();
                        chars
                            .next()
                            .map(|c| format!("{}{}", c.to_uppercase(), chars.as_str()))
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                Value::Text(words.join(" "))
            }
            ("remplacer", [Value::Text(s), Value::Text(from), Value::Text(to)]) => {
                Value::Text(s.replace(from, to))
            }
            ("mélanger", [Value::Text(s)]) => {
                let mut chars: Vec<_> = s.chars().collect();
                self.shuffle(&mut chars, pos)?;
                Value::Text(chars.into_iter().collect())
            }
            ("mélanger", [Value::Casier(c)]) => {
                let mut c = c.clone();
                self.shuffle(&mut c, pos)?;
                Value::Casier(c)
            }
            ("taille", [Value::Text(s)]) => Value::Number((s.encode_utf16().count() as u64).into()),
            ("taille", [Value::Casier(c)]) => Value::Number((c.len() as u64).into()),
            ("position", [Value::Text(s), start, Value::Text(query)]) => {
                let start = index_of(start, pos)?;
                let units: Vec<_> = s.encode_utf16().collect();
                let index = units
                    .get(start..)
                    .and_then(|tail| {
                        let text = String::from_utf16_lossy(tail).to_lowercase();
                        text.find(&query.to_lowercase())
                            .map(|i| (start + text[..i].encode_utf16().count()) as i64)
                    })
                    .unwrap_or(-1);
                Value::Number(index.into())
            }
            ("extraire", [Value::Text(s), start, end]) => {
                let units: Vec<_> = s.encode_utf16().collect();
                let range = index_of(start, pos)?..index_of(end, pos)?;
                Value::Text(String::from_utf16_lossy(
                    units
                        .get(range)
                        .ok_or_else(|| pos.error("indices hors limites"))?,
                ))
            }
            ("extraire", [Value::Casier(c), start, end]) => {
                let range = one_based(start, pos)?..index_of(end, pos)?;
                Value::Casier(
                    c.get(range)
                        .ok_or_else(|| pos.error("indices hors limites"))?
                        .to_vec(),
                )
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    fn shuffle<T>(&mut self, items: &mut [T], pos: Position) -> Outcome<()> {
        for i in (1..items.len()).rev() {
            let value = self
                .service(
                    Resource::Random,
                    "hasard",
                    &[Value::Number(((i + 1) as u64).into())],
                    pos,
                )?
                .ok_or_else(|| pos.error("nombre aléatoire attendu"))?;
            let j = index_of(&value, pos)?;
            if j > i {
                return Err(pos.error("nombre aléatoire hors limites"));
            }
            items.swap(i, j);
        }
        Ok(())
    }
}

fn one_based(value: &Value, pos: Position) -> Outcome<usize> {
    index_of(value, pos)?
        .checked_sub(1)
        .ok_or_else(|| pos.error("indice hors limites"))
}
