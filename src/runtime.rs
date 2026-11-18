use crate::EventArgument;
pub(crate) use crate::Selector as Key;
pub(crate) use crate::actors::Place;
use crate::actors::convert;
use crate::dispatch::Proxy;
use crate::files::File;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::thread::JoinHandle;

use bigdecimal::{ToPrimitive, Zero};

use crate::parser::{
    self, Body, Condition, Expr, Initial, Instruction, Loop, Program, Role, Statement,
};
use crate::{BigDecimal, Diagnostic, Host, Position, Request, Resource, Value};

pub(crate) type Outcome<T> = Result<T, Diagnostic>;
type Scope = HashMap<String, Place>;

pub(crate) enum Flow {
    Continue,
    Return(Option<Value>),
    Break(Position),
    Stop,
    Jump(String, Position),
}

pub(crate) struct Runtime {
    pub host: Proxy,
    pub book: String,
    pub title: String,
    pub program: Arc<Program>,
    scopes: Vec<Scope>,
    templates: Scope,
    pub jokers: Vec<Place>,
    pub created: Option<Place>,
    pub calls: usize,
    pub stopped: bool,
    pub precision: Option<u64>,
    pub radians: bool,
    pub indirection: usize,
    events: bool,
    lambdas: Vec<HashMap<String, (Vec<String>, Expr)>>,
    pending: VecDeque<(String, JoinHandle<Outcome<()>>)>,
    imports: HashMap<(String, String), (String, Arc<Program>, Scope, Scope)>,
    loading: Vec<String>,
}

pub(crate) fn run(book: &str, program: Program, host: &mut dyn Host) -> Outcome<()> {
    crate::dispatch::serve(host, move |host| run_book(book, program, host))
}

fn run_book(book: &str, program: Program, host: Proxy) -> Outcome<()> {
    let mut rt = Runtime {
        book: book.to_owned(),
        title: if book.is_empty() {
            "Livre sans nom".into()
        } else {
            std::path::Path::new(book)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| book.into())
        },
        program: Arc::new(program),
        host,
        scopes: vec![Scope::new()],
        templates: Scope::new(),
        jokers: Vec::new(),
        created: None,
        calls: 0,
        stopped: false,
        precision: None,
        radians: false,
        indirection: 0,
        events: true,
        lambdas: vec![HashMap::new()],
        pending: VecDeque::new(),
        imports: HashMap::new(),
        loading: Vec::new(),
    };
    let result = rt.start();
    if result.is_err() || rt.stopped {
        rt.host.cancel();
    }
    let joined = rt.observe(None);
    result.and(joined)
}

impl Runtime {
    fn start(&mut self) -> Outcome<()> {
        self.initialize()?;
        let program = Arc::clone(&self.program);
        let pos = Position { line: 1, column: 1 };
        if program
            .main
            .iter()
            .all(|s| matches!(s, Statement::Declare { .. }))
            && let Some(first) = program.functions.first()
        {
            self.execute(&program.main)?;
            let mut args = Vec::new();
            let params = parameters(first);
            for name in &params {
                self.host
                    .write(format!("Quelle est la valeur de l'acteur '{name}' ?\n").as_bytes())
                    .map_err(|e| first.position.error(e.to_string()))?;
                let raw = self
                    .host
                    .read(name, &Value::Text(String::new()))
                    .map_err(|e| first.position.error(e.to_string()))?
                    .unwrap_or_default();
                let value = crate::number::parse_number(&raw, first.position)
                    .map(Value::Number)
                    .unwrap_or(Value::Text(raw));
                args.push(Place::new(value, Role::Dynamic));
            }
            if let Some(result) = self.call(&first.name, args, first.position)? {
                self.print(&result, first.position)?;
            }
        } else {
            match self.execute(&program.main)? {
                Flow::Return(Some(value)) => self.print(&value, pos)?,
                Flow::Jump(name, p) => {
                    self.call(&name, vec![], p)?;
                }
                Flow::Break(p) => return Err(p.error("aucune boucle à interrompre")),
                _ => {}
            }
        }
        self.observe(None)?;
        while self.process_event(true, pos)? {}
        Ok(())
    }
}

fn parameters(function: &parser::Function) -> Vec<String> {
    if !function.parameters.is_empty() {
        return function.parameters.clone();
    }
    function
        .body
        .iter()
        .filter_map(|s| match s {
            Statement::Declare {
                input: true,
                actors,
                ..
            } => Some(actors.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

impl Runtime {
    fn initialize(&mut self) -> Outcome<()> {
        let program = self.program.clone();
        if !program.transcript.is_empty() {
            self.host
                .transcript(&self.book, &program.transcript)
                .map_err(|e| Position { line: 1, column: 1 }.error(e.to_string()))?;
        }
        for (alias, path, pos) in &program.imports {
            let id = (self.book.clone(), alias.clone());
            if self.imports.contains_key(&id) {
                return Err(pos.error(format!("livre « {alias} » déjà déclaré")));
            }
            let (book, source) = self
                .host
                .load(&self.book, path)
                .map_err(|e| pos.error(e.to_string()))?;
            if self.loading.contains(&book) || book == self.book {
                return Err(pos.error("référence circulaire entre livres"));
            }
            pos.check_depth(self.loading.len())?;
            let imported = Arc::new(parser::parse(&source)?);
            self.loading.push(self.book.clone());
            let saved_book = std::mem::replace(&mut self.book, book.clone());
            let saved_title = std::mem::replace(&mut self.title, book.clone());
            let saved_program = std::mem::replace(&mut self.program, imported.clone());
            let saved_scopes = std::mem::replace(&mut self.scopes, vec![Scope::new()]);
            let saved_templates = std::mem::take(&mut self.templates);
            let result = self.initialize();
            let globals = std::mem::replace(&mut self.scopes, saved_scopes).remove(0);
            let templates = std::mem::replace(&mut self.templates, saved_templates);
            self.book = saved_book;
            self.title = saved_title;
            self.program = saved_program;
            self.loading.pop();
            result?;
            self.imports
                .insert(id, (book, imported, globals, templates));
        }
        self.execute(&program.templates)?;
        self.templates = std::mem::take(&mut self.scopes[0]);
        self.execute(&program.global)?;
        Ok(())
    }

    pub fn find(&self, name: &str) -> Option<Place> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn current(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("book has a global scope")
    }

    pub fn service(
        &mut self,
        resource: Resource,
        operation: &str,
        args: &[Value],
        pos: Position,
    ) -> Outcome<Option<Value>> {
        self.host
            .request(Request {
                resource,
                operation,
                arguments: args,
            })
            .map_err(|e| pos.error(e.to_string()))
    }

    fn print(&mut self, value: &Value, pos: Position) -> Outcome<()> {
        match value {
            Value::Casier(items) => {
                for item in items {
                    self.print(item, pos)?;
                }
            }
            Value::Species { attributes, .. } => {
                for (_, item) in attributes {
                    self.print(item, pos)?;
                }
            }
            value => self
                .host
                .write(format!("{value}\n").as_bytes())
                .map_err(|e| pos.error(e.to_string()))?,
        }
        Ok(())
    }

    pub fn target(&mut self, expr: &Expr, pos: Position) -> Outcome<Place> {
        self.operand(expr)?.place(pos)
    }

    fn assign(&mut self, target: &Expr, value: Value, pos: Position) -> Outcome<()> {
        if let [Instruction::Actor(name, _)] = target.as_slice()
            && self.find(name).is_none()
        {
            self.check_name(name, pos)?;
            let role = value.role();
            let place = Place::new(value, role).bind(name, &self.host, None, pos)?;
            self.current().insert(name.clone(), place);
            return Ok(());
        }
        self.target(target, pos)?.write(value, pos)
    }

    fn default_value(&mut self, role: &Role, pos: Position, depth: usize) -> Outcome<Value> {
        pos.check_depth(depth)?;
        Ok(match role {
            Role::Number | Role::Dynamic => Value::Number(0.into()),
            Role::Text => Value::Text(String::new()),
            Role::Casier(_) => Value::Casier(Vec::new()),
            Role::Species(name) => {
                let program = Arc::clone(&self.program);
                if let Some(species) = program.species.iter().find(|s| s.name == *name) {
                    let mut attributes = if let Some(parent) = &species.parent {
                        match self.default_value(
                            &Role::Species(parent.clone()),
                            species.position,
                            depth + 1,
                        )? {
                            Value::Species { attributes, .. } => attributes,
                            _ => unreachable!(),
                        }
                    } else {
                        Vec::new()
                    };
                    for field in &species.attributes {
                        let template = self.templates.get(field).ok_or_else(|| {
                            species
                                .position
                                .error(format!("attribut « {field} » inconnu"))
                        })?;
                        if let Some((_, v)) = attributes.iter_mut().find(|(k, _)| k == field) {
                            *v = template.read(pos)?;
                        } else {
                            attributes.push((field.clone(), template.read(pos)?));
                        }
                    }
                    Value::Species {
                        name: name.clone(),
                        attributes,
                    }
                } else {
                    let resource = if graphic(name) {
                        Resource::Graphics
                    } else {
                        Resource::Plugin
                    };
                    self.service(resource, "crée", &[Value::Text(name.clone())], pos)?
                        .ok_or_else(|| pos.error(format!("espèce « {name} » inconnue")))?
                }
            }
        })
    }

    fn declare(
        &mut self,
        name: &str,
        role: &Role,
        initial: &Initial,
        input: bool,
        pos: Position,
    ) -> Outcome<()> {
        self.check_name(name, pos)?;
        if input && let Some(bound) = self.current().get(name) {
            convert(role, bound.read(pos)?, pos)?;
            return Ok(());
        }
        if self.current().contains_key(name) {
            return Err(pos.error(format!("acteur « {name} » déjà déclaré")));
        }
        let mut file = None;
        let mut entries = None;
        let value = match initial {
            Initial::Default => self.default_value(role, pos, 0)?,
            Initial::Values(exprs) => {
                if matches!(role, Role::Casier(_)) || exprs.len() > 1 {
                    let actors = self.arguments(exprs)?;
                    let values = actors
                        .iter()
                        .map(|p| p.read(pos))
                        .collect::<Outcome<Vec<_>>>()?;
                    entries = Some(actors);
                    Value::Casier(values)
                } else {
                    self.eval(&exprs[0])?
                }
            }
            Initial::Attributes(pairs) => {
                let mut value = self.default_value(role, pos, 0)?;
                let Value::Species { attributes, .. } = &mut value else {
                    return Err(pos.error("espèce attendue"));
                };
                for (key, expr) in pairs {
                    let slot = attributes
                        .iter_mut()
                        .find(|(k, _)| k == key)
                        .ok_or_else(|| pos.error(format!("attribut « {key} » inconnu")))?;
                    slot.1 = convert(&slot.1.role(), self.eval(expr)?, pos)?;
                }
                value
            }
            Initial::File(expr) => {
                let Value::Text(path) = self.eval(expr)? else {
                    return Err(pos.error("chemin de fichier attendu"));
                };
                let source = File {
                    book: self.book.clone(),
                    path,
                };
                let default = self.default_value(role, pos, 0)?;
                let element = match role {
                    Role::Casier(Some(kind)) => Some(self.default_value(kind, pos, 0)?),
                    _ => None,
                };
                let value = source.load(&mut self.host, default, element.as_ref(), pos)?;
                file = Some(source);
                value
            }
        };
        let value = convert(role, value, pos)?;
        let role = if *role == Role::Dynamic {
            value.role()
        } else {
            role.clone()
        };
        let place = Place::new(value, role);
        if let Some(entries) = entries {
            place.set_items(entries, pos)?;
        }
        let place = place.bind(name, &self.host, file, pos)?;
        self.current().insert(name.to_owned(), place.clone());
        self.created = Some(place.clone());
        if input {
            self.ask(name, place, pos)?;
        }
        Ok(())
    }

    fn ask(&mut self, name: &str, place: Place, pos: Position) -> Outcome<()> {
        let value = place.read(pos)?;
        if let Some(raw) = self
            .host
            .read(name, &value)
            .map_err(|e| pos.error(e.to_string()))?
        {
            let value = match value {
                Value::Number(_) => Value::Number(crate::number::parse_number(&raw, pos)?),
                Value::Text(_) => Value::Text(raw),
                _ => return Err(pos.error("un texte ou un nombre est attendu")),
            };
            place.write(value, pos)?;
        }
        Ok(())
    }

    pub fn call(&mut self, name: &str, args: Vec<Place>, pos: Position) -> Outcome<Option<Value>> {
        self.invoke(name, args, None, pos)
    }

    pub fn method(
        &mut self,
        receiver: Place,
        name: &str,
        args: Vec<Place>,
        pos: Position,
    ) -> Outcome<Option<Value>> {
        let attached = receiver.has_method(name, pos)?;
        let mut declared = false;
        if let Value::Species { name: kind, .. } = receiver.read(pos)? {
            let mut current = Some(kind.as_str());
            let mut remaining = self.program.species.len() + 1;
            while let Some(kind) = current {
                if remaining == 0 {
                    return Err(pos.error("cycle d’héritage"));
                }
                remaining -= 1;
                if self
                    .program
                    .methods
                    .iter()
                    .any(|(species, methods)| species == kind && methods.iter().any(|m| m == name))
                {
                    declared = true;
                    break;
                }
                current = self
                    .program
                    .species
                    .iter()
                    .find(|s| s.name == kind)
                    .and_then(|s| s.parent.as_deref());
            }
        }
        if !attached && !declared {
            let mut values = vec![receiver.read(pos)?];
            for arg in args {
                values.push(arg.read(pos)?);
            }
            if let Some(value) = self.internal_method(name, &values, pos)? {
                return Ok(Some(value));
            }
            return self.service(Resource::Plugin, name, &values, pos);
        }
        self.invoke(name, args, Some(receiver), pos)
    }

    pub fn attach(&mut self, args: &[Expr], pos: Position) -> Outcome<()> {
        let [method, dest] = args else {
            return Err(pos.error("deux paramètres attendus"));
        };
        let [Instruction::Actor(name, _)] = method.as_slice() else {
            return Err(pos.error("nom attendu"));
        };
        let target = self.target(dest, pos)?;
        if self.program.functions.iter().any(|f| f.name == *name) {
            target.add_method(name.clone(), pos)?;
        } else {
            let mut value = target.read(pos)?;
            let Value::Species { attributes, .. } = &mut value else {
                return Err(pos.error("espèce attendue"));
            };
            let attr = self.eval(method)?;
            if let Some(slot) = attributes.iter_mut().find(|(key, _)| key == name) {
                slot.1 = attr;
            } else {
                attributes.push((name.clone(), attr));
            }
            target.write(value, pos)?;
        }
        Ok(())
    }

    fn invoke(
        &mut self,
        name: &str,
        args: Vec<Place>,
        receiver: Option<Place>,
        pos: Position,
    ) -> Outcome<Option<Value>> {
        let mut flow = self.invoke_once(name, args, receiver, pos)?;
        while let Flow::Jump(name, p) = flow {
            flow = self.invoke_once(&name, vec![], None, p)?;
        }
        match flow {
            Flow::Return(v) => Ok(v),
            Flow::Break(p) => Err(p.error("aucune boucle à interrompre")),
            _ => Ok(None),
        }
    }

    fn invoke_once(
        &mut self,
        name: &str,
        args: Vec<Place>,
        receiver: Option<Place>,
        pos: Position,
    ) -> Outcome<Flow> {
        pos.check_depth(self.calls)?;
        if let Some((params, expr)) = self
            .lambdas
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
        {
            if params.len() != args.len() {
                return Err(pos.error("nombre de paramètres incorrect"));
            }
            self.scopes.push(params.into_iter().zip(args).collect());
            self.calls += 1;
            let result = self.eval(&expr);
            self.calls -= 1;
            self.scopes.pop();
            return result.map(|v| Flow::Return(Some(v)));
        }
        let program = self.program.clone();
        let function = program
            .functions
            .iter()
            .find(|f| f.name == name)
            .ok_or_else(|| pos.error(format!("fonction « {name} » inconnue")))?;
        let parameters = parameters(function);
        if parameters.len() != args.len() {
            return Err(pos.error("nombre de paramètres incorrect"));
        }
        let mut frame: Scope = parameters.into_iter().zip(args).collect();
        if let Some(obj) = receiver {
            frame.insert("moi".into(), obj);
        }
        let caller = self.scopes.split_off(1);
        self.scopes.push(frame);
        self.lambdas.push(HashMap::new());
        self.calls += 1;
        let result = self.execute(&function.body);
        self.calls -= 1;
        self.lambdas.pop();
        self.scopes.pop();
        self.scopes.extend(caller);
        result
    }

    pub fn observe(&mut self, filter: Option<&str>) -> Outcome<()> {
        let mut result = Ok(());
        while let Some(index) = self
            .pending
            .iter()
            .position(|(name, _)| filter.is_none_or(|f| f == name))
        {
            let (_, job) = self.pending.remove(index).unwrap();
            let joined = job.join().unwrap_or_else(|_| {
                Err(Position { line: 1, column: 1 }.error("arrêt inattendu du paragraphe"))
            });
            if result.is_ok() && !self.host.cancelled() {
                result = joined;
            }
        }
        result
    }

    fn spawn(&mut self, name: &str, args: &[Expr], pos: Position) -> Outcome<()> {
        if !self.program.functions.iter().any(|f| f.name == name) {
            return Err(pos.error(format!("fonction « {name} » inconnue")));
        }
        let args = self.arguments(args)?;
        let mut job = Runtime {
            host: self.host.clone(),
            book: self.book.clone(),
            title: self.title.clone(),
            program: self.program.clone(),
            scopes: vec![self.scopes[0].clone()],
            templates: self.templates.clone(),
            jokers: vec![],
            calls: 0,
            created: None,
            stopped: false,
            precision: self.precision,
            radians: self.radians,
            indirection: 0,
            events: false,
            lambdas: vec![self.lambdas[0].clone()],
            pending: VecDeque::new(),
            imports: self.imports.clone(),
            loading: self.loading.clone(),
        };
        let function = name.to_owned();
        let handle = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let result = job.call(&function, args, pos).map(|_| ());
                let children = job.observe(None);
                result.and(children)
            })
            .map_err(|e| pos.error(e.to_string()))?;
        self.pending.push_back((name.to_owned(), handle));
        Ok(())
    }

    fn book_call(
        &mut self,
        name: &str,
        function: &str,
        args: &[Expr],
        pos: Position,
    ) -> Outcome<()> {
        let args = self.arguments(args)?;
        let key = (self.book.clone(), name.to_owned());
        let (book, program, global, templates) = self
            .imports
            .get(&key)
            .cloned()
            .ok_or_else(|| pos.error(format!("livre « {name} » inconnu")))?;
        let saved_title = std::mem::replace(&mut self.title, book.clone());
        let saved_book = std::mem::replace(&mut self.book, book);
        let saved_program = std::mem::replace(&mut self.program, program);
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![global]);
        let saved_templates = std::mem::replace(&mut self.templates, templates);
        let result = self.call(function, args, pos);
        self.title = saved_title;
        let book = std::mem::replace(&mut self.book, saved_book);
        let program = std::mem::replace(&mut self.program, saved_program);
        let global = std::mem::replace(&mut self.scopes, saved_scopes).remove(0);
        let templates = std::mem::replace(&mut self.templates, saved_templates);
        if result.is_ok() {
            self.imports.insert(key, (book, program, global, templates));
        }
        result.map(|_| ())
    }

    fn process_event(&mut self, at_end: bool, pos: Position) -> Outcome<bool> {
        if !self.events || self.host.cancelled() {
            return Ok(false);
        }
        let Some(event) = self
            .host
            .event(at_end)
            .map_err(|e| pos.error(e.to_string()))?
        else {
            return Ok(false);
        };
        let args = event
            .arguments
            .into_iter()
            .map(|arg| match arg {
                EventArgument::Value(v) => {
                    let role = v.role();
                    Ok(Place::new(v, role))
                }
                EventArgument::Actor(reference) => {
                    let mut place = self
                        .scopes
                        .iter()
                        .rev()
                        .flat_map(|s| s.values())
                        .find(|place| place.reference().is_some_and(|b| b.id == reference.id))
                        .cloned()
                        .ok_or_else(|| pos.error("acteur d’événement hors de portée"))?;
                    place.path = reference.path;
                    Ok(place)
                }
            })
            .collect::<Outcome<Vec<_>>>()?;
        self.events = false;
        let result = self.call(&event.paragraph, args, pos);
        self.events = true;
        result.map(|_| true)
    }

    fn execute(&mut self, statements: &[Statement]) -> Outcome<Flow> {
        for statement in statements {
            if self.stopped || self.host.cancelled() {
                return Ok(Flow::Stop);
            }
            let pos = statement.position();
            self.process_event(false, pos)?;
            self.host
                .checkpoint()
                .map_err(|e| pos.error(e.to_string()))?;
            let flow = match statement {
                Statement::Create(expr, role, p) => {
                    let Value::Text(name) = self.eval(expr)? else {
                        return Err(p.error("nom d’acteur texte attendu"));
                    };
                    let name = name.to_lowercase();
                    let scopes = self.scopes.split_off(1);
                    let result = self.declare(&name, role, &Initial::Default, false, *p);
                    self.scopes.extend(scopes);
                    result?;
                    Flow::Continue
                }
                Statement::Declare {
                    actors,
                    role,
                    value,
                    input,
                    ..
                } => {
                    for name in actors {
                        self.declare(name, role, value, *input, pos)?;
                    }
                    Flow::Continue
                }
                Statement::Assign {
                    actor, values, op, ..
                } if matches!(actor.as_slice(), [Instruction::Alternatives(..)]) => {
                    let [Instruction::Alternatives(targets, _)] = actor.as_slice() else {
                        unreachable!()
                    };
                    for target in targets {
                        let one = Statement::Assign {
                            actor: target.clone(),
                            values: values.clone(),
                            op: *op,
                            position: pos,
                        };
                        self.execute(std::slice::from_ref(&one))?;
                    }
                    Flow::Continue
                }
                Statement::Assign {
                    actor, values, op, ..
                } => {
                    let mut rhs = self.values(values)?;
                    let mut value = if rhs.len() == 1 {
                        rhs.pop().unwrap()
                    } else {
                        Value::Casier(rhs)
                    };
                    if matches!(op, Some(parser::Binary::Add))
                        && matches!(self.eval(actor)?, Value::Casier(_))
                    {
                        let target = self.target(actor, pos)?;
                        let mut items = target.items(pos)?;
                        items.push(Place::literal(value));
                        target.set_items(items, pos)?;
                        continue;
                    }
                    if let Some(op) = op {
                        value = crate::eval::binary(
                            *op,
                            self.eval(actor)?,
                            value,
                            pos,
                            self.precision,
                        )?;
                    }
                    self.assign(actor, value, pos)?;
                    Flow::Continue
                }
                Statement::Print(exprs, _) => {
                    for expr in exprs {
                        let value = self.eval(expr)?;
                        self.print(&value, pos)?;
                    }
                    Flow::Continue
                }
                Statement::RawPrint(text, _) => {
                    self.print(&Value::Text(text.clone()), pos)?;
                    Flow::Continue
                }
                Statement::Ask(exprs, _) => {
                    for expr in exprs {
                        let name = match expr.first() {
                            Some(Instruction::Actor(name, _)) => name.as_str(),
                            _ => "",
                        };
                        let place = self.target(expr, pos)?;
                        self.ask(name, place, pos)?;
                    }
                    Flow::Continue
                }
                Statement::Try(body, alternative, _) => match self.body(body) {
                    Ok(flow) => flow,
                    Err(_) => self.body(alternative)?,
                },
                Statement::If(condition, yes, no, _) => {
                    if self.condition(condition)? {
                        self.body(yes)?
                    } else if let Some(no) = no {
                        self.body(no)?
                    } else {
                        Flow::Continue
                    }
                }
                Statement::Block(body, _) => self.body(body)?,
                Statement::Loop(spec, body, _) => self.run_loop(spec, body, pos)?,
                Statement::Return(expr, _) => {
                    Flow::Return(expr.as_ref().map(|e| self.eval(e)).transpose()?)
                }
                Statement::Break(_) => Flow::Break(pos),
                Statement::Stop(_) => {
                    self.stopped = true;
                    self.host.cancel();
                    Flow::Stop
                }
                Statement::Call(expr, _) => {
                    self.operand(expr)?;
                    Flow::Continue
                }
                Statement::Lambda(name, params, expr, _) => {
                    self.lambdas
                        .last_mut()
                        .unwrap()
                        .insert(name.clone(), (params.clone(), expr.clone()));
                    Flow::Continue
                }
                Statement::Jump(name, _) => Flow::Jump(name.clone(), pos),
                Statement::Spawn(name, args, _) => {
                    self.spawn(name, args, pos)?;
                    Flow::Continue
                }
                Statement::BookCall(book, function, args, _) => {
                    self.book_call(book, function, args, pos)?;
                    Flow::Continue
                }
                Statement::Command(op, args, _) => {
                    self.dispatch(None, op, args, pos)?;
                    Flow::Continue
                }
                Statement::Host(resource, op, args, _) => {
                    self.dispatch(Some(*resource), op, args, pos)?;
                    Flow::Continue
                }
            };
            if !matches!(flow, Flow::Continue) {
                return Ok(flow);
            }
        }
        Ok(Flow::Continue)
    }

    fn scoped<T>(
        &mut self,
        body: &Body,
        action: impl FnOnce(&mut Self) -> Outcome<T>,
    ) -> Outcome<T> {
        if body.scoped {
            self.scopes.push(Scope::new());
            self.lambdas.push(HashMap::new());
        }
        let result = action(self);
        if body.scoped {
            self.scopes.pop();
            self.lambdas.pop();
        }
        result
    }

    fn body(&mut self, body: &Body) -> Outcome<Flow> {
        self.scoped(body, |rt| rt.execute(&body.statements))
    }

    fn run_loop(&mut self, spec: &Loop, body: &Body, pos: Position) -> Outcome<Flow> {
        let joker = Place::new(Value::Number(0.into()), Role::Dynamic);
        self.jokers.push(joker.clone());
        let result = self.iterate(spec, body, joker, pos);
        self.jokers.pop();
        match result? {
            Flow::Break(_) => Ok(Flow::Continue),
            flow => Ok(flow),
        }
    }

    fn iterate(&mut self, spec: &Loop, body: &Body, joker: Place, pos: Position) -> Outcome<Flow> {
        match spec {
            Loop::While(condition) => {
                let mut repeat = self.condition(condition)?;
                while repeat {
                    self.host
                        .checkpoint()
                        .map_err(|e| pos.error(e.to_string()))?;
                    let (flow, next) = self.scoped(body, |rt| {
                        let flow = rt.execute(&body.statements)?;
                        let next = matches!(flow, Flow::Continue) && rt.condition(condition)?;
                        Ok((flow, next))
                    })?;
                    if !matches!(flow, Flow::Continue) {
                        return Ok(flow);
                    }
                    repeat = next;
                }
            }
            Loop::Range {
                actor,
                start,
                end,
                next,
                position,
            } => {
                let start = self.eval(start)?.number(*position)?.clone();
                let end = self.operand(end)?;
                let initial_end = end.value(pos)?.number(pos)?.clone();
                let target = match actor {
                    Some(expr) => self.target(expr, pos)?,
                    None => joker,
                };
                let mut counter = if next.is_some() {
                    start.clone()
                } else {
                    start.with_scale(0)
                };
                let ascending = next.is_some() || counter <= initial_end.with_scale(0);
                loop {
                    self.host
                        .checkpoint()
                        .map_err(|e| pos.error(e.to_string()))?;
                    let end = end.value(pos)?.number(pos)?.clone();
                    let end = if next.is_none() {
                        end.with_scale(0)
                    } else {
                        end
                    };
                    if (ascending && counter > end)
                        || (!ascending && counter < end)
                        || (next.is_some() && counter < start)
                    {
                        break;
                    }
                    target.write(Value::Number(counter.clone()), pos)?;
                    let (flow, new_counter) = self.scoped(body, |rt| {
                        let flow = rt.execute(&body.statements)?;
                        let new_counter = if matches!(flow, Flow::Continue)
                            && let Some(expr) = next
                        {
                            Some(rt.eval(expr)?.number(pos)?.clone())
                        } else {
                            None
                        };
                        Ok((flow, new_counter))
                    })?;
                    if !matches!(flow, Flow::Continue) {
                        return Ok(flow);
                    }
                    counter =
                        new_counter.unwrap_or_else(|| counter + if ascending { 1 } else { -1 });
                    if next.is_some() {
                        target.write(Value::Number(counter.clone()), pos)?;
                    }
                }
            }
            Loop::Each {
                actor,
                source,
                position,
            } => {
                let source = self.operand(source)?.as_place(*position)?;
                let mut i = 0usize;
                loop {
                    self.host
                        .checkpoint()
                        .map_err(|e| pos.error(e.to_string()))?;
                    let item = match source.read(pos)? {
                        Value::Casier(items) if i < items.len() => {
                            source.clone().child(Key::Index(i)).detached_path(pos)?
                        }
                        Value::Number(n) if n.to_usize().is_some_and(|count| i < count) => {
                            Place::new(Value::Number((i as u64).into()), Role::Number)
                        }
                        Value::Text(s) => match s.chars().nth(i) {
                            Some(c) => Place::new(Value::Text(c.to_string()), Role::Text),
                            None => break,
                        },
                        Value::Species { .. } => {
                            return Err(pos.error("casier, texte ou nombre attendu"));
                        }
                        _ => break,
                    };
                    *self.jokers.last_mut().unwrap() = item.clone();
                    if let Some(expr) = actor {
                        let target = self.target(expr, pos)?;
                        if target.read(pos)?.role() != item.read(pos)?.role() {
                            return Err(pos.error("rôle du parcours incompatible"));
                        }
                        if let [Instruction::Actor(name, _)] = expr.as_slice() {
                            if let Some(slot) = self
                                .scopes
                                .iter_mut()
                                .rev()
                                .find_map(|scope| scope.get_mut(name))
                            {
                                *slot = item;
                            }
                        } else {
                            target.write(item.read(pos)?, pos)?;
                        }
                    }
                    let flow = self.body(body)?;
                    if !matches!(flow, Flow::Continue) {
                        return Ok(flow);
                    }
                    i += 1;
                }
            }
        }
        Ok(Flow::Continue)
    }

    fn condition(&mut self, condition: &Condition) -> Outcome<bool> {
        match condition {
            Condition::Truth(expr, pos) => {
                Ok(self.eval(expr)?.number(*pos)? == &BigDecimal::from(1))
            }
            Condition::Empty(expr, invert, pos) => {
                let empty = match self.eval(expr)? {
                    Value::Number(n) => n.is_zero(),
                    Value::Text(s) => s.is_empty(),
                    Value::Casier(c) => c.is_empty(),
                    _ => return Err(pos.error("condition de vide invalide")),
                };
                Ok(empty != *invert)
            }
            Condition::Contains(expr, sought, pos) => {
                let haystack = self.eval(expr)?;
                let needle = self.eval(sought)?;
                match (haystack, needle) {
                    (Value::Text(s), Value::Text(n)) => {
                        Ok(s.to_lowercase().contains(&n.to_lowercase()))
                    }
                    (Value::Casier(c), n) => Ok(c.iter().any(|v| crate::eval::equal(v, &n))),
                    _ => Err(pos.error("texte ou casier attendu")),
                }
            }
            Condition::Collision(left, right, pos) => {
                let pair = [self.eval(left)?, self.eval(right)?];
                Ok(self
                    .service(Resource::Graphics, "collision", &pair, *pos)?
                    .ok_or_else(|| pos.error("résultat de collision attendu"))?
                    .number(*pos)?
                    == &BigDecimal::from(1))
            }
        }
    }
}

fn graphic(name: &str) -> bool {
    matches!(
        name,
        "point"
            | "ligne"
            | "cercle"
            | "graffiti"
            | "graphique"
            | "toile"
            | "rectangle"
            | "scribe"
            | "crayon"
            | "parchemin"
            | "mégalithe"
            | "polygone"
            | "chemin"
            | "praxinoscope"
    )
}
