#![forbid(unsafe_code)]

//! French-language Linotte execution with injectable platform services.

mod actors;
mod commands;
mod dispatch;
mod eval;
mod files;
mod host;
mod lexer;
mod number;
mod parser;
mod runtime;

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
    Casier(Vec<Value>),
    Species {
        name: String,
        attributes: Vec<(String, Value)>,
    },
}

impl Value {
    fn number(&self, pos: Position) -> Result<&BigDecimal, Diagnostic> {
        match self {
            Self::Number(n) => Ok(n),
            _ => Err(pos.error("une valeur numérique est attendue")),
        }
    }

    fn role(&self) -> Role {
        match self {
            Self::Number(_) => Role::Number,
            Self::Text(_) => Role::Text,
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
