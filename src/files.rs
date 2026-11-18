use crate::dispatch::Proxy;
use crate::runtime::Outcome;
use crate::{Host, Position, Request, Resource, Value};

#[derive(Clone)]
pub(crate) struct File {
    pub book: String,
    pub path: String,
    pub element: Option<Value>,
}

impl File {
    pub fn load(&self, host: &mut Proxy, default: &Value, pos: Position) -> Outcome<Option<Value>> {
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
        let Some(raw) = raw else { return Ok(None) };
        let Some(raw) = raw.utf16_units() else {
            return Err(pos.error("contenu du fichier invalide"));
        };
        let mut lines = split_lines(&raw).into_iter().peekable();
        if let Some(element) = &self.element {
            let mut items = Vec::new();
            while lines.peek().is_some() {
                let mut item = element.clone();
                if !read_item(&mut item, &mut lines, pos)? {
                    return Err(pos.error("rôle sans représentation de fichier"));
                }
                items.push(item);
            }
            Ok(Some(Value::Casier(items)))
        } else {
            let mut value = default.clone();
            if !read_item(&mut value, &mut lines, pos)? {
                return Err(pos.error("fichier vide"));
            }
            Ok(Some(value))
        }
    }

    pub fn save(&self, host: &mut Proxy, value: &Value, pos: Position) -> Outcome<()> {
        let mut text = Vec::new();
        write_item(value, &mut text, false);
        let arguments = [
            Value::Text(self.book.clone()),
            Value::Text(self.path.clone()),
            Value::from_utf16_units(text),
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

fn read_item(
    item: &mut Value,
    lines: &mut impl Iterator<Item = Vec<u16>>,
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
                Value::Number(_) => {
                    let text = String::from_utf16(&line)
                        .map_err(|_| pos.error("nombre de fichier UTF-16 invalide"))?;
                    Value::Number(crate::number::parse_number(&text, pos)?)
                }
                _ => Value::from_utf16_units(line),
            };
            Ok(true)
        }
    }
}

fn write_item(item: &Value, text: &mut Vec<u16>, newline: bool) {
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
            item.append_utf16(text);
            if newline {
                text.extend(if cfg!(windows) { "\r\n" } else { "\n" }.encode_utf16());
            }
        }
    }
}

fn split_lines(text: &[u16]) -> Vec<Vec<u16>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < text.len() {
        if matches!(text[index], 10 | 13) {
            lines.push(text[start..index].to_vec());
            if text[index] == 13 && text.get(index + 1) == Some(&10) {
                index += 1;
            }
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    if start < text.len() {
        lines.push(text[start..].to_vec());
    }
    lines
}
