use crate::lexer::{self, Kind, Token};
use crate::number::parse_number;
use crate::{Diagnostic, Position, Resource, Value};

pub(crate) type Expr = Vec<Instruction>;

#[derive(Clone)]
pub(crate) enum Instruction {
    Literal(Value),
    Actor(String, Position),
    Alternatives(Vec<Expr>, Position),
    NameOf(String, Position),
    Indirect(Expr, Position),
    Interpolate(Vec<Fragment>),
    Unary(String, Position),
    Binary(Binary, Position),
    Call(String, Vec<Expr>, Position),
    Casier(Vec<Expr>, Position),
    Select(Vec<Expr>, Position),
    Attribute(String, Position),
    Method(String, Vec<Expr>, Position),
}

#[derive(Clone)]
pub(crate) enum Fragment {
    Text(String),
    Expression(String, String),
}

#[derive(Clone, Copy)]
pub(crate) enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Modulo,
    LeftShift,
    RightShift,
    And,
    Or,
    Xor,
    Join,
    Compare(Comparison),
}

#[derive(Clone, Copy)]
pub(crate) enum Comparison {
    Equal,
    Unequal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Clone)]
pub(crate) enum Condition {
    Truth(Expr, Position),
    Contains(Expr, Expr, Position),
    Empty(Expr, bool, Position),
    Collision(Expr, Expr, Position),
}

#[derive(Clone)]
pub(crate) struct Body {
    pub scoped: bool,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Number,
    Text,
    Casier(Option<Box<Role>>),
    Species(String),
    Dynamic,
}

#[derive(Clone)]
pub(crate) enum Initial {
    Default,
    Values(Vec<Expr>),
    File(Expr),
    Attributes(Vec<(String, Expr)>),
}

#[derive(Clone)]
pub(crate) enum Loop {
    While(Condition),
    Range {
        actor: Option<Expr>,
        start: Expr,
        end: Expr,
        next: Option<Expr>,
        position: Position,
    },
    Each {
        actor: Option<Expr>,
        source: Expr,
        position: Position,
    },
}

#[derive(Clone)]
pub(crate) enum Statement {
    Declare {
        actors: Vec<String>,
        role: Role,
        value: Initial,
        input: bool,
        position: Position,
    },
    Assign {
        actor: Expr,
        values: Vec<Expr>,
        op: Option<Binary>,
        position: Position,
    },
    Print(Vec<Expr>, Position),
    RawPrint(String, Position),
    Ask(Vec<Expr>, Position),
    If(Condition, Body, Option<Body>, Position),
    Try(Body, Body, Position),
    Loop(Loop, Body, Position),
    Block(Body, Position),
    Return(Option<Expr>, Position),
    Break(Position),
    Stop(Position),
    Call(Expr, Position),
    Lambda(String, Vec<String>, Expr, Position),
    Create(Expr, Role, Position),
    Jump(String, Position),
    Spawn(String, Vec<Expr>, Position),
    BookCall(String, String, Vec<Expr>, Position),
    Command(String, Vec<Expr>, Position),
    Host(Resource, String, Vec<Expr>, Position),
}

impl Statement {
    pub(crate) fn position(&self) -> Position {
        match self {
            Self::Declare { position, .. }
            | Self::Assign { position, .. }
            | Self::Print(_, position)
            | Self::RawPrint(_, position)
            | Self::Ask(_, position)
            | Self::Try(_, _, position)
            | Self::Break(position)
            | Self::BookCall(_, _, _, position)
            | Self::If(_, _, _, position)
            | Self::Loop(_, _, position)
            | Self::Return(_, position)
            | Self::Stop(position)
            | Self::Call(_, position)
            | Self::Block(_, position)
            | Self::Jump(_, position)
            | Self::Spawn(_, _, position)
            | Self::Create(_, _, position)
            | Self::Lambda(_, _, _, position)
            | Self::Command(_, _, position)
            | Self::Host(_, _, _, position) => *position,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Function {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Statement>,
    pub position: Position,
}

pub(crate) struct Species {
    pub name: String,
    pub parent: Option<String>,
    pub attributes: Vec<String>,
    pub position: Position,
}

pub(crate) struct Program {
    pub global: Vec<Statement>,
    pub main: Vec<Statement>,
    pub functions: Vec<Function>,
    pub templates: Vec<Statement>,
    pub species: Vec<Species>,
    pub methods: Vec<(String, Vec<String>)>,
    pub imports: Vec<(String, String, Position)>,
    pub transcript: Vec<(bool, String)>,
}

pub(crate) fn parse(source: &str) -> Result<Program, Diagnostic> {
    let tokens = lexer::tokenize(source)?;
    Parser {
        tokens: &tokens,
        current: 0,
        attribute_de: true,
        source,
    }
    .program()
}

pub(crate) fn parse_expression(source: &str) -> Result<Expr, Diagnostic> {
    let tokens = lexer::tokenize(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        current: 0,
        attribute_de: true,
        source,
    };
    let expression = parser.expression()?;
    if parser.token().kind != Kind::Eof {
        return Err(parser.token().position.error("fin d’expression attendue"));
    }
    Ok(expression)
}

struct Parser<'a> {
    tokens: &'a [Token<'a>],
    current: usize,
    attribute_de: bool,
    source: &'a str,
}

impl<'a> Parser<'a> {
    fn token(&self) -> Token<'a> {
        self.tokens[self.current]
    }

    fn advance(&mut self) -> Token<'a> {
        let token = self.token();
        if token.kind != Kind::Eof {
            self.current += 1;
        }
        token
    }

    fn matches(&self, text: &str) -> bool {
        match self.token().kind {
            Kind::Word(word) => word.eq_ignore_ascii_case(text) || word.to_lowercase() == text,
            Kind::Symbol(symbol) => symbol == text,
            _ => false,
        }
    }

    fn take(&mut self, text: &str) -> bool {
        if self.matches(text) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn take_one(&mut self, words: &[&str]) -> bool {
        words.iter().any(|word| self.take(word))
    }

    fn require(&mut self, text: &str) -> Result<(), Diagnostic> {
        if self.take(text) {
            Ok(())
        } else {
            Err(self.token().position.error(format!("« {text} » attendu")))
        }
    }

    fn skip_lines(&mut self) {
        while self.token().kind == Kind::EndLine {
            self.advance();
        }
    }

    fn at_end(&self) -> bool {
        matches!(self.token().kind, Kind::EndLine | Kind::Eof)
    }

    fn line_end(&self) -> Result<(), Diagnostic> {
        if self.at_end() {
            Ok(())
        } else {
            Err(self.token().position.error("fin d’instruction attendue"))
        }
    }

    fn name(&mut self) -> Result<String, Diagnostic> {
        let mut parts = Vec::new();
        while let Kind::Word(word) = self.token().kind {
            if !parts.is_empty() && is_boundary(word) {
                break;
            }
            parts.push(word.to_lowercase());
            self.advance();
            while let Kind::Number(digits) = self.token().kind {
                parts.push(digits.to_owned());
                self.advance();
            }
        }
        if parts.is_empty() {
            return Err(self.token().position.error("nom d’acteur attendu"));
        }
        Ok(parts.join(" "))
    }

    fn names(&mut self) -> Result<Vec<String>, Diagnostic> {
        let mut names = vec![self.name()?];
        while self.take(",") {
            names.push(self.name()?);
        }
        Ok(names)
    }

    fn raw_line(&mut self) -> String {
        let start = self.token().offset;
        while !self.at_end() {
            self.advance();
        }
        let line = &self.source[start..self.token().offset];
        line.split_once("//")
            .map_or(line, |(text, _)| text)
            .trim_end()
            .to_owned()
    }

    fn program(&mut self) -> Result<Program, Diagnostic> {
        let mut program = Program {
            global: Vec::new(),
            main: Vec::new(),
            functions: Vec::new(),
            templates: Vec::new(),
            species: Vec::new(),
            methods: Vec::new(),
            imports: Vec::new(),
            transcript: Vec::new(),
        };
        let mut section = "";
        loop {
            self.skip_lines();
            let position = self.token().position;
            if self.token().kind == Kind::Eof {
                return Ok(program);
            }
            if position.column == 1 {
                let start = self.current;
                if let Some(header) = ["bibliothèque", "globale", "espèces", "prototype", "tests"]
                    .into_iter()
                    .find(|header| self.take(header))
                    && self.at_end()
                {
                    section = header;
                    continue;
                }
                self.current = start;
                if matches!(self.token().kind, Kind::Word(_))
                    && let Ok(name) = self.name()
                    && self.take(":")
                {
                    let parameters = if self.at_end() {
                        Vec::new()
                    } else {
                        self.names()?
                    };
                    self.line_end()?;
                    program.functions.push(Function {
                        name,
                        parameters,
                        body: Vec::new(),
                        position,
                    });
                    section = "function";
                    continue;
                }
                self.current = start;
                section = "";
            }
            match section {
                "bibliothèque" => {
                    let name = self.name()?;
                    self.require("référence")?;
                    let path = if let Kind::Text(text) = self.token().kind {
                        self.advance();
                        text.to_owned()
                    } else {
                        self.raw_line()
                    };
                    if path.is_empty() {
                        return Err(position.error("référence de livre attendue"));
                    }
                    program.imports.push((name, path, position));
                }
                "globale" => program.global.push(self.statement(0)?),
                "espèces" | "prototype" => {
                    let before = self.current;
                    let explicit = self.take_one(&["espèce", "type"]);
                    let proposed = self.name().ok().filter(|_| self.take("propose"));
                    if let Some(species) = proposed {
                        program.methods.push((species, self.names()?));
                    } else if explicit {
                        self.current = before + 1;
                        let name = self.name()?;
                        let parent = if self.take("hérite") {
                            self.require("de")?;
                            let parent = self.name()?;
                            self.take("et");
                            Some(parent)
                        } else {
                            None
                        };
                        let attributes = if self.take("contient") {
                            self.names()?
                        } else {
                            Vec::new()
                        };
                        program.species.push(Species {
                            name,
                            parent,
                            attributes,
                            position,
                        });
                    } else {
                        self.current = before;
                        program.templates.push(self.statement(0)?);
                    }
                }
                "tests" => {
                    let input = self.matches(">");
                    if !self.take_one(&["<", ">"]) {
                        return Err(position.error("entrée ou sortie de test attendue"));
                    }
                    program.transcript.push((input, self.raw_line()));
                }
                "function" => {
                    let statement = self.statement(0)?;
                    program
                        .functions
                        .last_mut()
                        .expect("active paragraph")
                        .body
                        .push(statement);
                }
                _ => program.main.push(self.statement(0)?),
            }
            self.line_end()?;
        }
    }

    fn closing(&mut self) -> bool {
        if self.matches("§")
            && matches!(
                self.tokens.get(self.current + 1).map(|t| t.kind),
                Some(Kind::EndLine | Kind::Eof)
            )
        {
            self.advance();
            return true;
        }
        let start = self.current;
        if self.take("ferme") && (self.at_end() || self.matches("sinon")) {
            true
        } else {
            self.current = start;
            false
        }
    }

    fn block(
        &mut self,
        depth: usize,
        opening: Position,
        alternatives: bool,
    ) -> Result<Body, Diagnostic> {
        let mut statements = Vec::new();
        loop {
            self.skip_lines();
            if self.closing() || (alternatives && (self.matches("sinon") || self.matches("xsinon")))
            {
                return Ok(Body {
                    scoped: true,
                    statements,
                });
            }
            if self.token().kind == Kind::Eof {
                return Err(opening.error("bloc sans « ferme »"));
            }
            statements.push(self.statement(depth)?);
            self.line_end()?;
        }
    }

    fn body(
        &mut self,
        depth: usize,
        opening: Position,
        alternatives: bool,
    ) -> Result<Body, Diagnostic> {
        self.take(",");
        if self.at_end() {
            self.block(depth, opening, alternatives)
        } else {
            Ok(Body {
                scoped: false,
                statements: vec![self.statement(depth)?],
            })
        }
    }

    fn alternative(
        &mut self,
        depth: usize,
        position: Position,
    ) -> Result<Option<Body>, Diagnostic> {
        let end = self.current;
        self.skip_lines();
        if self.take_one(&["sinon", "xsinon"]) {
            Ok(Some(self.body(depth, position, false)?))
        } else {
            self.current = end;
            Ok(None)
        }
    }

    fn statement(&mut self, depth: usize) -> Result<Statement, Diagnostic> {
        let position = self.token().position;
        position.check_depth(depth)?;
        if self.take("si") {
            let condition = self.condition()?;
            let yes = self.body(depth + 1, position, true)?;
            let no = self.alternative(depth + 1, position)?;
            return Ok(Statement::If(condition, yes, no, position));
        }
        if self.take("tant") {
            self.require("que")?;
            let condition = self.condition()?;
            return Ok(Statement::Loop(
                Loop::While(condition),
                self.body(depth + 1, position, false)?,
                position,
            ));
        }
        if self.take_one(&["pour", "de"]) {
            let anonymous = matches!(self.tokens[self.current - 1].kind, Kind::Word(word) if word.eq_ignore_ascii_case("de"));
            self.take("chaque");
            let first = self.before_de(0)?;
            let spec = if anonymous || self.take("de") {
                let (actor, start) = if anonymous {
                    (None, first)
                } else {
                    (Some(first), self.expression()?)
                };
                if self.take("à") {
                    let end = self.expression()?;
                    let next = if self.take("suivant") {
                        Some(self.expression()?)
                    } else {
                        None
                    };
                    Loop::Range {
                        actor,
                        start,
                        end,
                        next,
                        position,
                    }
                } else if let Some(actor) = actor {
                    Loop::Each {
                        actor: Some(actor),
                        source: start,
                        position,
                    }
                } else {
                    return Err(position.error("borne finale de boucle attendue"));
                }
            } else {
                Loop::Each {
                    actor: None,
                    source: first,
                    position,
                }
            };
            return Ok(Statement::Loop(
                spec,
                self.body(depth + 1, position, false)?,
                position,
            ));
        }
        if self.take("essaie") {
            let yes = self.body(depth + 1, position, true)?;
            let no = self
                .alternative(depth + 1, position)?
                .ok_or_else(|| position.error("« sinon » attendu après « essaie »"))?;
            return Ok(Statement::Try(yes, no, position));
        }
        if self.take(":") {
            return Ok(Statement::Block(
                self.body(depth + 1, position, false)?,
                position,
            ));
        }
        if self.take("affiche") {
            let args = self.group()?;
            self.take("!");
            return Ok(Statement::Print(args, position));
        }
        if self.take("_") {
            return Ok(Statement::RawPrint(self.raw_line(), position));
        }
        if self.take("demande") {
            return Ok(Statement::Ask(self.group()?, position));
        }
        if self.take("retourne") {
            return Ok(Statement::Return(Some(self.expression()?), position));
        }
        if self.take("reviens") {
            return Ok(Statement::Return(None, position));
        }
        if self.take("interromps") {
            self.require("boucle")?;
            return Ok(Statement::Break(position));
        }
        if self.take("termine") {
            return Ok(Statement::Stop(position));
        }
        if self.take("crée") {
            let name = self.expression()?;
            if !self.take_one(&["un", "une"]) {
                return Err(position.error("rôle d’acteur attendu"));
            }
            self.take("acteur");
            return Ok(Statement::Create(name, self.role()?, position));
        }
        if self.matches("ne") || self.matches("fais") {
            let start = self.current;
            let remove = self.take("ne");
            if self.take("fais") {
                if remove {
                    self.require("plus")?;
                }
                if self.take("réagir") {
                    let actor = self.grouped_argument()?;
                    self.require("à")?;
                    let event = self.expression()?;
                    let mut args = vec![actor, event];
                    if !remove {
                        self.require("pour")?;
                        args.push(vec![Instruction::Literal(Value::Text(self.raw_line()))]);
                    }
                    return Ok(Statement::Host(
                        Resource::Graphics,
                        if remove {
                            "ne fais plus réagir"
                        } else {
                            "fais réagir"
                        }
                        .into(),
                        args,
                        position,
                    ));
                }
            }
            self.current = start;
        }
        if self.take("va") {
            self.require("vers")?;
            return Ok(Statement::Jump(self.name()?, position));
        }
        if self.take("appelle") {
            let name = self.name()?;
            let args = if self.take("avec") {
                self.values()?
            } else {
                Vec::new()
            };
            return Ok(Statement::Spawn(name, args, position));
        }
        if self.take_one(&["évoque", "evoque", "fais"]) {
            let save = self.current;
            if self.take("le") && self.take("café") {
                return Ok(Statement::Command("fais le café".into(), vec![], position));
            }
            self.current = save;
            return Ok(Statement::Call(self.expression()?, position));
        }
        if self.take("parcours") {
            let name = self.name()?;
            if self.take("de") {
                let book = self.name()?;
                let args = if self.take("avec") {
                    self.values()?
                } else {
                    Vec::new()
                };
                return Ok(Statement::BookCall(book, name, args, position));
            }
            let args = if self.take("avec") {
                self.values()?
            } else if self.take("(") {
                self.arguments(")", 0)?
            } else {
                Vec::new()
            };
            return Ok(Statement::Call(
                vec![Instruction::Call(name, args, position)],
                position,
            ));
        }
        let start = self.current;
        if let Kind::Word(word) = self.token().kind {
            let word = word.to_lowercase();
            if let Some(resource) = resource_verb(&word) {
                self.advance();
                return self.host_statement(resource, word, position);
            }
            if is_command(&word) {
                self.advance();
                if word == "nomme" {
                    return Ok(Statement::Command(
                        word,
                        vec![vec![Instruction::Literal(Value::Text(self.raw_line()))]],
                        position,
                    ));
                }
                if word == "vide" && self.take("les") {
                    self.require("touches")?;
                    return Ok(Statement::Host(
                        Resource::Graphics,
                        "vide les touches".into(),
                        vec![],
                        position,
                    ));
                }
                return Ok(Statement::Command(
                    word,
                    self.command_arguments()?,
                    position,
                ));
            }
        }
        if self.take("~") {
            return Ok(Statement::Command(
                "~".into(),
                vec![vec![Instruction::Literal(Value::Text(self.raw_line()))]],
                position,
            ));
        }
        self.take("§");
        let input = self.take("*");
        if let Ok(first) = self.name() {
            let mut actors = vec![first];
            while self.take("&") {
                if let Ok(name) = self.name() {
                    actors.push(name);
                } else {
                    break;
                }
            }
            let role = if self.take_one(&["est", "::"]) {
                self.take_one(&["un", "une", "des"]);
                if self.take("lambda") {
                    let mut parameters = vec![self.name()?];
                    while self.take("|") {
                        parameters.push(self.name()?);
                    }
                    if !self.take_one(&["<-", "valant"]) {
                        return Err(position.error("valeur de lambda attendue"));
                    }
                    if actors.len() != 1 {
                        return Err(position.error("nom de lambda attendu"));
                    }
                    return Ok(Statement::Lambda(
                        actors.remove(0),
                        parameters,
                        self.expression()?,
                        position,
                    ));
                }
                Some(self.role()?)
            } else if self.matches("valant") || self.matches("<-") {
                Some(Role::Dynamic)
            } else {
                None
            };
            if let Some(role) = role {
                let value = if self.take_one(&["valant", "<-", "ex", "contenant"]) {
                    Initial::Values(self.values()?)
                } else if self.take("depuis") {
                    Initial::File(self.expression()?)
                } else if matches!(role, Role::Species(_)) && self.take(",") {
                    Initial::Attributes(self.attributes()?)
                } else {
                    Initial::Default
                };
                return Ok(Statement::Declare {
                    actors,
                    role,
                    value,
                    input,
                    position,
                });
            }
        }
        self.current = start;
        let mut targets = vec![self.expression_level(4)?];
        while self.take("&") {
            targets.push(self.expression_level(4)?);
        }
        let value = if targets.len() == 1 {
            targets.remove(0)
        } else {
            vec![Instruction::Alternatives(targets, position)]
        };
        let op = if self.take("+=") {
            Some(Some(Binary::Add))
        } else if self.take("-=") {
            Some(Some(Binary::Subtract))
        } else if self.take_one(&["prend", "vaut", "="]) {
            Some(None)
        } else {
            None
        };
        if let Some(op) = op {
            return Ok(Statement::Assign {
                actor: value,
                values: self.values()?,
                op,
                position,
            });
        }
        self.current = start;
        let values = self.group()?;
        if self.take("!") {
            return Ok(Statement::Print(values, position));
        }
        if self.take("?") {
            return Ok(Statement::Ask(values, position));
        }
        if values.len() == 1 {
            let value = values.into_iter().next().expect("one expression");
            if matches!(
                value.last(),
                Some(Instruction::Call(..) | Instruction::Method(..))
            ) {
                return Ok(Statement::Call(value, position));
            }
            if let [Instruction::Actor(name, _)] = value.as_slice() {
                return Ok(Statement::Call(
                    vec![Instruction::Call(name.clone(), Vec::new(), position)],
                    position,
                ));
            }
        }
        Err(position.error("instruction attendue"))
    }

    fn role(&mut self) -> Result<Role, Diagnostic> {
        if self.take_one(&["nombre", "nombres", "drapeau", "drapeaux"]) {
            Ok(Role::Number)
        } else if self.take_one(&["texte", "textes"]) {
            Ok(Role::Text)
        } else if self.take_one(&["casier", "casiers"]) {
            if self.take("de") {
                let role = if self.take_one(&["casier", "casiers"]) {
                    Role::Casier(None)
                } else {
                    self.role()?
                };
                Ok(Role::Casier(Some(Box::new(role))))
            } else {
                Ok(Role::Casier(None))
            }
        } else {
            Ok(Role::Species(self.name()?))
        }
    }

    fn attributes(&mut self) -> Result<Vec<(String, Expr)>, Diagnostic> {
        let mut attributes = Vec::new();
        loop {
            self.take("il");
            self.take("a");
            let name = self.name()?;
            if !self.take_one(&["prend", "vaut", "="]) {
                return Err(self.token().position.error("valeur d’attribut attendue"));
            }
            attributes.push((name, self.expression()?));
            if !self.take(",") {
                return Ok(attributes);
            }
        }
    }

    fn condition(&mut self) -> Result<Condition, Diagnostic> {
        let position = self.token().position;
        let left = self.expression()?;
        if self.take("contient") {
            return Ok(Condition::Contains(left, self.expression()?, position));
        }
        if self.take("est") {
            if self.take("en") {
                self.require("collision")?;
                self.require("avec")?;
                return Ok(Condition::Collision(left, self.expression()?, position));
            }
            let start = self.current;
            let invert = self.take("non");
            if self.take("vide") {
                return Ok(Condition::Empty(left, invert, position));
            }
            self.current = start;
            let mut comparison = Comparison::Equal;
            if self.take_one(&["différent", "différente"]) {
                self.require("de")?;
                comparison = Comparison::Unequal;
            } else if self.matches("plus") || self.matches("moins") {
                let less = self.take("moins");
                if !less {
                    self.require("plus")?;
                }
                if self.take_one(&["grand", "grande"]) {
                    comparison = Comparison::Greater;
                } else {
                    if !self.take_one(&["petit", "petite"]) {
                        return Err(position.error("comparaison attendue"));
                    }
                    comparison = Comparison::Less;
                }
                if less {
                    comparison = match comparison {
                        Comparison::Less => Comparison::Greater,
                        _ => Comparison::Less,
                    };
                }
                if self.take("ou") {
                    self.take_one(&["égal", "égale"]);
                    comparison = match comparison {
                        Comparison::Greater => Comparison::GreaterEqual,
                        _ => Comparison::LessEqual,
                    };
                }
                self.take_one(&["que", "à"]);
            } else if self.take_one(&["égal", "égale"]) {
                self.require("à")?;
            }
            let mut result = left;
            result.extend(self.expression()?);
            result.push(Instruction::Binary(Binary::Compare(comparison), position));
            return Ok(Condition::Truth(result, position));
        }
        Ok(Condition::Truth(left, position))
    }

    fn host_statement(
        &mut self,
        resource: Resource,
        mut word: String,
        position: Position,
    ) -> Result<Statement, Diagnostic> {
        if matches!(word.as_str(), "efface" | "montre") && self.take("toile") {
            word.push_str(" toile");
        } else if word == "efface" && self.take("tableau") {
            word.push_str(" tableau");
            return Ok(Statement::Host(
                Resource::Console,
                word,
                Vec::new(),
                position,
            ));
        } else if word == "joue" {
            self.take("le");
        } else if word == "photographie" {
            self.take("dans");
        } else if word == "tourne" && self.take("à") {
            if self.take("gauche") {
                word.push_str(" à gauche");
            } else if self.take("droite") {
                word.push_str(" à droite");
            }
        }
        if word == "observe" {
            return Ok(Statement::Command(
                word,
                vec![vec![Instruction::Literal(Value::Text(
                    self.raw_line().to_lowercase(),
                ))]],
                position,
            ));
        }
        if word == "déplace" {
            let actor = self.grouped_argument()?;
            let absolute = if self.take("vers") {
                true
            } else {
                self.require("de")?;
                false
            };
            let mut args = vec![actor];
            if absolute && self.take_one(&["la", "le"]) {
                let direction = self.name()?;
                self.require("de")?;
                word.push_str(" vers ");
                word.push_str(&direction);
                args.push(self.expression()?);
            } else {
                word.push_str(if absolute { " vers" } else { " de" });
                args.push(self.before_de(3)?);
                self.require("et")?;
                args.push(self.expression()?);
            }
            return Ok(Statement::Host(resource, word, args, position));
        }
        let mut args = self.command_arguments()?;
        if matches!(word.as_str(), "attends" | "temporise") {
            let unit = if self.take_one(&["milliseconde", "millisecondes"]) {
                "millisecondes"
            } else {
                self.take_one(&["seconde", "secondes"]);
                "secondes"
            };
            args.push(vec![Instruction::Literal(Value::Text(unit.to_owned()))]);
        }
        Ok(Statement::Host(resource, word, args, position))
    }

    fn group(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut expressions = vec![self.expression_level(1)?];
        while self.take("&") {
            expressions.push(self.expression_level(1)?);
        }
        Ok(expressions)
    }

    fn values(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut values = vec![self.expression()?];
        while self.take(",") && !self.at_end() {
            values.push(self.expression()?);
        }
        Ok(values)
    }

    fn command_arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        while !self.at_end()
            && ![
                "sinon",
                "seconde",
                "secondes",
                "milliseconde",
                "millisecondes",
                "ferme",
                "§",
            ]
            .iter()
            .any(|word| self.matches(word))
        {
            if self.matches("la")
                && matches!(self.tokens.get(self.current+1).map(|t|t.kind),Some(Kind::Word(w)) if w.eq_ignore_ascii_case("position"))
            {
                self.advance();
                self.advance();
                self.require("de")?;
            }
            if self.take("partir") {
                self.require("de")?;
            }
            self.take("de");
            let arg = self.grouped_argument()?;
            args.push(arg);
            if !self.take_one(&[
                ",", "dans", "de", "par", "avec", "depuis", "à", "vers", "et", "en", "sur", "pour",
            ]) {
                break;
            }
        }
        Ok(args)
    }

    fn before_de(&mut self, minimum: u8) -> Result<Expr, Diagnostic> {
        let old = std::mem::replace(&mut self.attribute_de, false);
        let result = self.expression_level(minimum);
        self.attribute_de = old;
        result
    }

    fn grouped_argument(&mut self) -> Result<Expr, Diagnostic> {
        let pos = self.token().position;
        let mut values = vec![self.before_de(3)?];
        while self.take("&") {
            values.push(self.before_de(3)?);
        }
        if values.len() == 1 {
            Ok(values.remove(0))
        } else {
            Ok(vec![Instruction::Alternatives(values, pos)])
        }
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic> {
        self.expression_level(0)
    }

    fn expression_level(&mut self, minimum: u8) -> Result<Expr, Diagnostic> {
        let mut out = Vec::new();
        self.expression_into(minimum, 0, &mut out)?;
        Ok(out)
    }

    fn arguments(&mut self, closing: &str, depth: usize) -> Result<Vec<Expr>, Diagnostic> {
        let mut args = Vec::new();
        if !self.take(closing) {
            loop {
                let mut arg = Vec::new();
                self.expression_into(0, depth + 1, &mut arg)?;
                args.push(arg);
                if !self.take(",") {
                    break;
                }
            }
            self.require(closing)?;
        }
        Ok(args)
    }

    fn expression_into(
        &mut self,
        minimum: u8,
        depth: usize,
        out: &mut Expr,
    ) -> Result<(), Diagnostic> {
        let token = self.advance();
        token.position.check_depth(depth)?;
        let start = out.len();
        match token.kind {
            Kind::Symbol("(") => {
                self.expression_into(0, depth + 1, out)?;
                self.require(")")?;
            }
            Kind::Symbol("[") => {
                let name = self.name()?;
                self.require("]")?;
                out.push(Instruction::NameOf(name, token.position));
            }
            Kind::Symbol("<<") => {
                let mut inner = Vec::new();
                self.expression_into(4, depth + 1, &mut inner)?;
                self.require(">>")?;
                out.push(Instruction::Indirect(inner, token.position));
            }
            Kind::Symbol("#" | "µ") => {
                self.expression_into(8, depth + 1, out)?;
                out.push(Instruction::Unary(
                    if token.kind == Kind::Symbol("#") {
                        "clone"
                    } else {
                        "unicode"
                    }
                    .into(),
                    token.position,
                ));
            }
            Kind::Symbol("{") => out.push(Instruction::Casier(
                self.arguments("}", depth)?,
                token.position,
            )),
            Kind::Symbol("-" | "+") => {
                self.expression_into(8, depth + 1, out)?;
                let op = if token.kind == Kind::Symbol("-") {
                    "-"
                } else {
                    "+"
                };
                out.push(Instruction::Unary(op.to_owned(), token.position));
            }
            Kind::Symbol("|") => out.push(Instruction::Literal(Value::Text("\n".to_owned()))),
            Kind::Number(number) => out.push(Instruction::Literal(Value::Number(parse_number(
                number,
                token.position,
            )?))),
            Kind::Note(note) => out.push(Instruction::Actor(format!("'{note}'"), token.position)),
            Kind::Text(text) => out.push(Instruction::Interpolate(fragments(text))),
            Kind::Word(word) => {
                let lower = word.to_lowercase();
                if is_builtin(&lower) && !self.matches("(") {
                    self.expression_into(8, depth + 1, out)?;
                    out.push(Instruction::Unary(lower, token.position));
                } else {
                    self.current -= 1;
                    let name = self.name()?;
                    if self.take("(") {
                        out.push(Instruction::Call(
                            name,
                            self.arguments(")", depth)?,
                            token.position,
                        ));
                    } else {
                        out.push(Instruction::Actor(name, token.position));
                    }
                }
            }
            _ => return Err(token.position.error("expression attendue")),
        }
        loop {
            let token = self.token();
            if self.take("{") {
                let closing = if token.kind == Kind::Symbol("{") {
                    "}"
                } else {
                    "]"
                };
                let indices = self.arguments(closing, depth)?;
                if indices.is_empty() {
                    return Err(token.position.error("indice attendu"));
                }
                out.push(Instruction::Select(indices, token.position));
                continue;
            }
            if self.take(".") {
                let name = self.name()?;
                self.require("(")?;
                out.push(Instruction::Method(
                    name,
                    self.arguments(")", depth)?,
                    token.position,
                ));
                continue;
            }
            if self.take("@") || self.attribute_de && self.take("de") {
                let [Instruction::Actor(name, _)] = &out[start..] else {
                    return Err(token.position.error("attribut attendu"));
                };
                let name = name.clone();
                out.truncate(start);
                self.expression_into(8, depth + 1, out)?;
                out.push(Instruction::Attribute(name, token.position));
                continue;
            }
            let Some((op, precedence)) = binary(token) else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.advance();
            self.expression_into(precedence + 1, depth + 1, out)?;
            out.push(Instruction::Binary(op, token.position));
        }
        Ok(())
    }
}

fn binary(token: Token<'_>) -> Option<(Binary, u8)> {
    use Binary::*;
    let op = match token.kind {
        Kind::Word(word) => match word.to_lowercase().as_str() {
            "ou" => (Or, 1),
            "xou" => (Xor, 1),
            "et" => (And, 2),
            "sup" => (Compare(Comparison::Greater), 3),
            "inf" => (Compare(Comparison::Less), 3),
            "égal" | "égale" => (Compare(Comparison::Equal), 3),
            "diff" => (Compare(Comparison::Unequal), 3),
            "rg" => (LeftShift, 6),
            "rd" => (RightShift, 6),
            "mod" | "modulo" => (Modulo, 6),
            "puiss" | "puissance" => (Power, 7),
            _ => return None,
        },
        Kind::Symbol("&") => (Join, 0),
        Kind::Symbol("=" | "==") => (Compare(Comparison::Equal), 3),
        Kind::Symbol("!=") => (Compare(Comparison::Unequal), 3),
        Kind::Symbol("<") => (Compare(Comparison::Less), 3),
        Kind::Symbol("<=") => (Compare(Comparison::LessEqual), 3),
        Kind::Symbol(">") => (Compare(Comparison::Greater), 3),
        Kind::Symbol(">=") => (Compare(Comparison::GreaterEqual), 3),
        Kind::Symbol("+") => (Add, 4),
        Kind::Symbol("-") => (Subtract, 4),
        Kind::Symbol("*") => (Multiply, 5),
        Kind::Symbol("/") => (Divide, 5),
        _ => return None,
    };
    Some(op)
}

fn fragments(text: &str) -> Vec<Fragment> {
    let mut fragments = Vec::new();
    let mut rest = text;
    while let Some((prefix, expression)) = rest.split_once('{') {
        let Some((expression, tail)) = expression.split_once('}') else {
            break;
        };
        if expression.is_empty() {
            fragments.push(Fragment::Text(format!("{prefix}{{}}")));
        } else {
            let open = if let Some(prefix) = prefix.strip_suffix('$') {
                fragments.push(Fragment::Text(prefix.to_owned()));
                "$"
            } else {
                fragments.push(Fragment::Text(prefix.to_owned()));
                ""
            };
            fragments.push(Fragment::Expression(
                expression.to_owned(),
                format!("{open}{{{expression}}}"),
            ));
        }
        rest = tail;
    }
    fragments.push(Fragment::Text(rest.to_owned()));
    fragments
}

pub(crate) fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "hasard"
            | "carré"
            | "sin"
            | "asin"
            | "acos"
            | "cos"
            | "entier"
            | "décimal"
            | "arrondi"
            | "tan"
            | "atan"
            | "atan2"
            | "log"
            | "logn"
            | "abs"
            | "racine"
            | "cube"
            | "non"
            | "ascii"
            | "unicode"
            | "chr"
            | "ter"
            | "ternaire"
            | "clone"
            | "exp"
    )
}

fn is_boundary(word: &str) -> bool {
    let word = word.to_lowercase();
    is_command(&word)
        || resource_verb(&word).is_some()
        || matches!(
            word.as_str(),
            "un" | "une"
                | "est"
                | "valant"
                | "vaut"
                | "prend"
                | "contenant"
                | "ex"
                | "de"
                | "à"
                | "suivant"
                | "dans"
                | "par"
                | "avec"
                | "depuis"
                | "vers"
                | "en"
                | "sur"
                | "si"
                | "sinon"
                | "xsinon"
                | "tant"
                | "pour"
                | "chaque"
                | "ferme"
                | "affiche"
                | "demande"
                | "retourne"
                | "reviens"
                | "termine"
                | "appelle"
                | "parcours"
                | "évoque"
                | "evoque"
                | "fais"
                | "interromps"
                | "essaie"
                | "crée"
                | "contient"
                | "hérite"
                | "propose"
                | "référence"
                | "seconde"
                | "secondes"
                | "milliseconde"
                | "millisecondes"
                | "et"
                | "ou"
                | "xou"
                | "puiss"
                | "puissance"
                | "mod"
                | "modulo"
                | "sup"
                | "inf"
                | "égal"
                | "diff"
                | "rg"
                | "rd"
                | "va"
                | "ne"
        )
}

fn is_command(word: &str) -> bool {
    matches!(
        word,
        "vide"
            | "mélange"
            | "inverse"
            | "trie"
            | "copie"
            | "convertis"
            | "concatène"
            | "fusionne"
            | "mesure"
            | "ajoute"
            | "soustrais"
            | "multiplie"
            | "divise"
            | "ôte"
            | "ote"
            | "supprime"
            | "cherche"
            | "insère"
            | "extrais"
            | "modifie"
            | "incrémente"
            | "décrémente"
            | "évalue"
            | "evalue"
            | "provoque"
            | "annihile"
            | "recharge"
            | "attache"
            | "nomme"
    )
}

fn resource_verb(word: &str) -> Option<Resource> {
    Some(match word {
        "projette" | "montre" | "efface" | "déplace" | "pique" | "rafraîchis" | "avance"
        | "recule" | "tourne" | "peins" => Resource::Graphics,
        "joue" => Resource::Media,
        "attends" | "temporise" => Resource::Clock,
        "photographie" => Resource::Files,
        "ouvre" | "ferme" | "charge" | "décharge" | "configure" | "stimule" | "explore" => {
            Resource::Plugin
        }
        "observe" | "questionne" | "débogue" => Resource::System,
        _ => return None,
    })
}
