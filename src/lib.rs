#![forbid(unsafe_code)]

//! French-language Linotte execution with injectable platform services.

mod actors;
mod atelier;
mod commands;
mod dispatch;
mod eval;
mod files;
mod host;
mod lexer;
mod number;
mod parser;
mod runtime;
mod text;

use std::fmt;
use std::io::{self, Write};

pub use bigdecimal::BigDecimal;
pub use host::{ActorRef, Event, EventArgument, Host, MethodResult, Request, Resource, Selector};
pub use parser::Role;

#[derive(Debug)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[derive(Clone, Copy)]
struct Position {
    line: usize,
    column: usize,
}

impl Position {
    fn error(self, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            line: self.line,
            column: self.column,
            message: message.into(),
        }
    }

    fn check_depth(self, depth: usize) -> Result<(), Diagnostic> {
        if depth >= 128 {
            return Err(self.error("limite d’imbrication dépassée"));
        }
        Ok(())
    }
}

/// An actor value exchanged with the host. Species preserve attribute order.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(BigDecimal),
    Text(String),
    /// UTF-16 text containing at least one unpaired surrogate.
    ///
    /// Use [`Value::from_utf16_units`] to normalize well-formed units to `Text`.
    TextUnits(Vec<u16>),
    Casier(Vec<Value>),
    Species {
        name: String,
        attributes: Vec<(String, Value)>,
    },
}

impl Value {
    /// Builds text from Java-compatible UTF-16 code units without discarding
    /// isolated surrogate values.
    pub fn from_utf16_units(units: impl Into<Vec<u16>>) -> Self {
        let units = units.into();
        match String::from_utf16(&units) {
            Ok(text) => Self::Text(text),
            Err(_) => Self::TextUnits(units),
        }
    }

    /// Returns the exact UTF-16 units when this value is text.
    pub fn utf16_units(&self) -> Option<Vec<u16>> {
        match self {
            Self::Text(text) => Some(text.encode_utf16().collect()),
            Self::TextUnits(units) => Some(units.clone()),
            _ => None,
        }
    }

    pub(crate) fn is_text(&self) -> bool {
        matches!(self, Self::Text(_) | Self::TextUnits(_))
    }

    pub(crate) fn text_lossy(&self) -> Option<String> {
        self.utf16_units()
            .map(|units| String::from_utf16_lossy(&units))
    }

    pub(crate) fn text_utf8(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::TextUnits(units) => String::from_utf16(units).ok(),
            _ => None,
        }
    }

    pub(crate) fn append_utf16(&self, target: &mut Vec<u16>) {
        match self {
            Self::Text(text) => target.extend(text.encode_utf16()),
            Self::TextUnits(units) => target.extend(units),
            Self::Number(number) => target.extend(number::format_number(number).encode_utf16()),
            Self::Casier(values) => {
                target.push(u16::from(b'{'));
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        target.extend(", ".encode_utf16());
                    }
                    value.append_utf16(target);
                }
                target.push(u16::from(b'}'));
            }
            Self::Species { name, .. } => target.extend(name.encode_utf16()),
        }
    }

    fn number(&self, pos: Position) -> Result<&BigDecimal, Diagnostic> {
        match self {
            Self::Number(n) => Ok(n),
            _ => Err(pos.error("une valeur numérique est attendue")),
        }
    }

    fn role(&self) -> Role {
        match self {
            Self::Number(_) => Role::Number,
            Self::Text(_) | Self::TextUnits(_) => Role::Text,
            Self::Casier(_) => Role::Casier(None),
            Self::Species { name, .. } => Role::Species(name.clone()),
        }
    }

    fn flag(value: bool) -> Self {
        Self::Number(i32::from(value).into())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => f.write_str(&number::format_number(n)),
            Self::Text(s) => f.write_str(s),
            Self::TextUnits(units) => f.write_str(&String::from_utf16_lossy(units)),
            Self::Casier(values) => {
                f.write_str("{")?;
                for (i, value) in values.iter().enumerate() {
                    if i != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_str("}")
            }
            Self::Species { name, .. } => f.write_str(name),
        }
    }
}

/// Runs a book with console output directed to the given writer.
pub fn execute(source: &str, output: &mut impl Write) -> Result<(), Diagnostic> {
    struct Writer<'a, W>(&'a mut W);
    impl<W: Write> Host for Writer<'_, W> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.0.write_all(bytes)
        }
    }
    execute_with_host("", source, &mut Writer(output))
}

/// Runs the UTF-8 source of a named book using the supplied host.
pub fn execute_with_host(book: &str, source: &str, host: &mut dyn Host) -> Result<(), Diagnostic> {
    runtime::run(book, parser::parse(source)?, host)
}

/// Runs a book in Atelier transcript mode, using `>` declarations as scripted
/// input and `<` declarations as expected console output. A single `*` output
/// declaration matches any displayed value. Other host services are forwarded.
/// Returns the number of transcript entries consumed.
pub fn execute_with_atelier_tests(
    book: &str,
    source: &str,
    host: &mut dyn Host,
) -> Result<usize, Diagnostic> {
    let program = parser::parse(source)?;
    if program.transcript.is_empty() {
        return Err(atelier::no_tests());
    }
    let entries = program.transcript.clone();
    let mut test_host = atelier::TestHost::new(host, &entries);
    runtime::run(book, program, &mut test_host)?;
    Ok(test_host.checks())
}
