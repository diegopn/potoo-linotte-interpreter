use crate::dispatch::Proxy;
use crate::runtime::Outcome;
use crate::{Host, Position, Request, Resource, Value};

#[derive(Clone)]
pub(crate) struct File {
    pub book: String,
    pub path: String,
}

impl File {
    pub fn load(
        &self,
        host: &mut Proxy,
        default: Value,
        element: Option<&Value>,
        pos: Position,
    ) -> Outcome<Value> {
        let arguments = [
            Value::Text(self.book.clone()),
            Value::Text(self.path.clone()),
        ];
        let raw = host
            .request(Request {
                resource: Resource::Files,
                operation: "lire",
                arguments: &arguments,
            })
            .map_err(|e| pos.error(e.to_string()))?;
        let Some(raw) = raw else { return Ok(default) };
        let Value::Text(raw) = raw else {
            return Err(pos.error("contenu du fichier invalide"));
        };
        let mut lines = raw.lines().peekable();
        if let Some(element) = element {
            let mut items = Vec::new();
            while lines.peek().is_some() {
                let mut item = element.clone();
                if !read_item(&mut item, &mut lines, pos)? {
                    return Err(pos.error("rôle sans représentation de fichier"));
                }
                items.push(item);
            }
            Ok(Value::Casier(items))
        } else {
            let mut value = default;
            if !read_item(&mut value, &mut lines, pos)? {
                return Err(pos.error("fichier vide"));
            }
            Ok(value)
        }
    }

    pub fn save(&self, host: &mut Proxy, value: &Value, pos: Position) -> Outcome<()> {
        let mut text = String::new();
        write_item(value, &mut text, false);
        let arguments = [
            Value::Text(self.book.clone()),
            Value::Text(self.path.clone()),
            Value::Text(text),
        ];
        host.request(Request {
            resource: Resource::Files,
            operation: "écrire",
            arguments: &arguments,
        })
        .map_err(|e| pos.error(e.to_string()))?;
        Ok(())
    }
}

fn read_item<'a>(
    item: &mut Value,
    lines: &mut impl Iterator<Item = &'a str>,
    pos: Position,
) -> Outcome<bool> {
    match item {
        Value::Species { attributes, .. } => {
            let mut fields: Vec<_> = attributes.iter_mut().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            let mut any = false;
            for (_, v) in fields {
                if !read_item(v, lines, pos)? {
                    break;
                }
                any = true;
            }
            Ok(any)
        }
        Value::Casier(_) => Ok(false),
        _ => {
            let Some(line) = lines.next() else {
                return Ok(false);
            };
            *item = match item {
                Value::Number(_) => Value::Number(crate::number::parse_number(line, pos)?),
                _ => Value::Text(line.into()),
            };
            Ok(true)
        }
    }
}

fn write_item(item: &Value, text: &mut String, newline: bool) {
    match item {
        Value::Casier(items) => {
            for v in items {
                write_item(v, text, true);
            }
        }
        Value::Species { attributes, .. } => {
            let mut fields: Vec<_> = attributes.iter().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, v) in fields {
                write_item(v, text, true);
            }
        }
        _ => {
            text.push_str(&item.to_string());
            if newline {
                text.push_str(if cfg!(windows) { "\r\n" } else { "\n" });
            }
        }
    }
}
