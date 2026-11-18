use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};

use crate::dispatch::Proxy;
use crate::files::File;
use crate::runtime::Outcome;
use crate::{ActorRef, Host, Position, Role, Selector, Value};

struct Binding {
    host: Proxy,
    id: u64,
    name: String,
    file: Option<File>,
    root: Weak<Cell>,
}

#[derive(Clone)]
enum Data {
    Scalar(Value),
    Casier(Vec<Place>),
    Species(String, Vec<(String, Place)>),
}

impl Data {
    fn from_value(value: Value) -> Self {
        match value {
            Value::Casier(values) => Self::Casier(values.into_iter().map(Place::literal).collect()),
            Value::Species { name, attributes } => Self::Species(
                name,
                attributes
                    .into_iter()
                    .map(|(k, v)| (k, Place::literal(v)))
                    .collect(),
            ),
            v => Self::Scalar(v),
        }
    }
}

struct Cell {
    role: Role,
    data: Mutex<Data>,
    methods: Mutex<Vec<String>>,
    observers: Mutex<Vec<Weak<Binding>>>,
}

#[derive(Clone)]
pub(crate) struct Place {
    cell: Arc<Cell>,
    binding: Option<Arc<Binding>>,
    pub path: Vec<Selector>,
}

impl Place {
    pub fn literal(value: Value) -> Self {
        let role = value.role();
        Self::new(value, role)
    }

    pub fn new(value: Value, role: Role) -> Self {
        Self::from_data(Data::from_value(value), role)
    }

    fn from_data(data: Data, role: Role) -> Self {
        Self {
            cell: Arc::new(Cell {
                role,
                data: Mutex::new(data),
                methods: Default::default(),
                observers: Default::default(),
            }),
            binding: None,
            path: Vec::new(),
        }
    }

    pub fn bind(
        mut self,
        name: &str,
        host: &Proxy,
        file: Option<File>,
        pos: Position,
    ) -> Outcome<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let binding = Arc::new(Binding {
            host: host.clone(),
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            name: name.into(),
            file,
            root: Arc::downgrade(&self.cell),
        });
        self.watch(&binding, pos, 0)?;
        self.binding = Some(binding);
        host.clone()
            .changed(&self.reference().unwrap(), &self.read(pos)?)
            .map_err(|e| pos.error(e.to_string()))?;
        Ok(self)
    }

    fn watch(&self, binding: &Arc<Binding>, pos: Position, depth: usize) -> Outcome<()> {
        pos.check_depth(depth)?;
        let cell = self.resolve(pos)?;
        {
            let mut observers = cell.observers.lock().unwrap();
            let weak = Arc::downgrade(binding);
            observers.retain(|b| b.strong_count() > 0);
            if observers.iter().any(|b| b.ptr_eq(&weak)) {
                return Ok(());
            }
            observers.push(weak);
        }
        let data = cell.data.lock().unwrap().clone();
        Self::watch_children(&data, binding, pos, depth + 1)
    }

    fn watch_children(
        data: &Data,
        binding: &Arc<Binding>,
        pos: Position,
        depth: usize,
    ) -> Outcome<()> {
        match data {
            Data::Casier(c) => {
                for item in c {
                    item.watch(binding, pos, depth)?;
                }
            }
            Data::Species(_, fields) => {
                for (_, item) in fields {
                    item.watch(binding, pos, depth)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve(&self, pos: Position) -> Outcome<Arc<Cell>> {
        let mut cell = self.cell.clone();
        for selector in &self.path {
            let next = match (&*cell.data.lock().unwrap(), selector) {
                (Data::Casier(items), Selector::Index(i)) => items.get(*i).cloned(),
                (Data::Species(_, fields), Selector::Attribute(name)) => fields
                    .iter()
                    .find(|(k, _)| k == name)
                    .map(|(_, p)| p.clone()),
                _ => None,
            }
            .ok_or_else(|| pos.error("indice ou attribut inconnu"))?;
            cell = next.resolve(pos)?;
        }
        Ok(cell)
    }

    pub fn detached_path(&self, pos: Position) -> Outcome<Self> {
        Ok(Self {
            cell: self.resolve(pos)?,
            binding: None,
            path: Vec::new(),
        })
    }

    pub fn reference(&self) -> Option<ActorRef> {
        self.binding.as_ref().map(|b| ActorRef {
            id: b.id,
            name: b.name.clone(),
            path: self.path.clone(),
        })
    }

    pub fn child(mut self, key: Selector) -> Self {
        self.path.push(key);
        self
    }

    pub fn read(&self, pos: Position) -> Outcome<Value> {
        Self::export(&self.resolve(pos)?, pos, 0)
    }

    fn export(cell: &Arc<Cell>, pos: Position, depth: usize) -> Outcome<Value> {
        pos.check_depth(depth)?;
        let data = cell.data.lock().unwrap().clone();
        Ok(match data {
            Data::Scalar(v) => v,
            Data::Casier(items) => Value::Casier(
                items
                    .iter()
                    .map(|v| Self::export(&v.resolve(pos)?, pos, depth + 1))
                    .collect::<Outcome<_>>()?,
            ),
            Data::Species(name, fields) => Value::Species {
                name,
                attributes: fields
                    .into_iter()
                    .map(|(k, p)| Ok((k, Self::export(&p.resolve(pos)?, pos, depth + 1)?)))
                    .collect::<Outcome<_>>()?,
            },
        })
    }

    pub fn items(&self, pos: Position) -> Outcome<Vec<Place>> {
        if let Data::Casier(c) = &*self.resolve(pos)?.data.lock().unwrap() {
            Ok(c.clone())
        } else {
            Err(pos.error("casier attendu"))
        }
    }

    pub fn write(&self, value: Value, pos: Position) -> Outcome<()> {
        let cell = self.resolve(pos)?;
        self.replace(
            &cell,
            Data::from_value(convert(&cell.role, value, pos)?),
            pos,
        )
    }

    pub fn set_items(&self, items: Vec<Place>, pos: Position) -> Outcome<()> {
        let cell = self.resolve(pos)?;
        let Role::Casier(element) = &cell.role else {
            return Err(pos.error("casier attendu"));
        };
        let mut expected = element.as_deref().cloned();
        let mut entries = Vec::new();
        for item in items {
            let place = item.detached_path(pos)?;
            let kind = place.read(pos)?.role();
            if !compatible(expected.get_or_insert(kind.clone()), &kind) {
                return Err(pos.error("rôles incompatibles dans le casier"));
            }
            if Self::contains(&place.cell, &cell, pos, 0)? {
                return Err(pos.error("référence circulaire dans le casier"));
            }
            entries.push(place);
        }
        self.replace(&cell, Data::Casier(entries), pos)
    }

    fn contains(
        cell: &Arc<Cell>,
        needle: &Arc<Cell>,
        pos: Position,
        depth: usize,
    ) -> Outcome<bool> {
        if Arc::ptr_eq(cell, needle) {
            return Ok(true);
        }
        pos.check_depth(depth)?;
        let children = match cell.data.lock().unwrap().clone() {
            Data::Casier(c) => c,
            Data::Species(_, fields) => fields.into_iter().map(|(_, p)| p).collect(),
            _ => return Ok(false),
        };
        for item in children {
            if Self::contains(&item.resolve(pos)?, needle, pos, depth + 1)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn same(&self, other: &Place, pos: Position) -> Outcome<bool> {
        Ok(Arc::ptr_eq(&self.resolve(pos)?, &other.resolve(pos)?))
    }

    pub fn has_method(&self, name: &str, pos: Position) -> Outcome<bool> {
        Ok(self
            .resolve(pos)?
            .methods
            .lock()
            .unwrap()
            .iter()
            .any(|m| m == name))
    }

    pub fn add_method(&self, name: String, pos: Position) -> Outcome<()> {
        self.resolve(pos)?.methods.lock().unwrap().push(name);
        Ok(())
    }

    fn replace(&self, cell: &Arc<Cell>, data: Data, pos: Position) -> Outcome<()> {
        let observers: Vec<_> = cell
            .observers
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for b in &observers {
            Self::watch_children(&data, b, pos, 0)?;
        }
        *cell.data.lock().unwrap() = data;
        for b in observers {
            let Some(root) = b.root.upgrade() else {
                continue;
            };
            let value = Self::export(&root, pos, 0)?;
            let mut host = b.host.clone();
            if let Some(file) = &b.file {
                file.save(&mut host, &value, pos)?;
            }
            let path = self
                .reference()
                .filter(|r| r.id == b.id)
                .map_or(Vec::new(), |r| r.path);
            host.changed(
                &ActorRef {
                    id: b.id,
                    name: b.name.clone(),
                    path,
                },
                &value,
            )
            .map_err(|e| pos.error(e.to_string()))?;
        }
        Ok(())
    }
}

fn compatible(expected: &Role, kind: &Role) -> bool {
    expected == kind || matches!((expected, kind), (Role::Casier(_), Role::Casier(_)))
}

pub(crate) fn convert(role: &Role, value: Value, pos: Position) -> Outcome<Value> {
    match (role, value) {
        (Role::Text, Value::Number(n)) => Ok(Value::Text(crate::number::format_number(&n))),
        (Role::Casier(element), Value::Casier(items)) => {
            let expected = element
                .as_deref()
                .cloned()
                .or_else(|| items.first().map(Value::role));
            if items.iter().any(|item| {
                expected
                    .as_ref()
                    .is_some_and(|e| !compatible(e, &item.role()))
            }) {
                return Err(pos.error("rôles incompatibles dans le casier"));
            }
            Ok(Value::Casier(items))
        }
        (Role::Dynamic, value) => Ok(value),
        (role, value) if compatible(role, &value.role()) => Ok(value),
        _ => Err(pos.error(format!("valeur incompatible avec le rôle {role:?}"))),
    }
}
