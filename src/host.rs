use std::io;

use crate::Value;

/// Platform service requested by a Linotte instruction or actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resource {
    Console,
    Clock,
    Random,
    Graphics,
    Media,
    Files,
    Plugin,
    System,
}

/// Identifies an actor and nested collection items or species attributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorRef {
    pub id: u64,
    pub name: String,
    pub path: Vec<Selector>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selector {
    Index(usize),
    Attribute(String),
}

/// A paragraph call requested by a platform event.
#[derive(Clone, Debug)]
pub struct Event {
    pub paragraph: String,
    pub arguments: Vec<EventArgument>,
}

#[derive(Clone, Debug)]
pub enum EventArgument {
    Value(Value),
    Actor(ActorRef),
}

/// A Linotte verb or predefined actor name with evaluated arguments.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub resource: Resource,
    pub operation: &'a str,
    pub arguments: &'a [Value],
}

/// Return value and positional actor changes from a plugin method.
#[derive(Default, Debug)]
pub struct MethodResult {
    pub value: Option<Value>,
    /// Position zero is the receiver. Subsequent positions are method arguments.
    pub updates: Vec<(usize, Value)>,
}

/// Supplies optional services to a Linotte program. The host controls access to
/// external resources and may reject an operation with an I/O error.
pub trait Host {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Reads one console value. No value leaves the destination actor unchanged.
    fn read(&mut self, _actor: &str, _current: &Value) -> io::Result<Option<String>> {
        Err(unavailable("demande"))
    }

    /// Resolves a book reference, returning its stable name and UTF-8 source.
    fn load(&mut self, _from: &str, path: &str) -> io::Result<(String, String)> {
        Err(unavailable(path))
    }

    /// Performs an external operation and optionally returns a Linotte value.
    fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
        Err(unavailable(request.operation))
    }

    /// Performs a statement. Each returned pair replaces the indicated actor argument.
    fn action(
        &mut self,
        resource: Resource,
        operation: &str,
        arguments: &[Value],
    ) -> io::Result<Vec<(usize, Value)>> {
        self.request(Request {
            resource,
            operation,
            arguments,
        })?;
        Ok(Vec::new())
    }

    /// Supplies stable actor identities for a statement that accesses the platform.
    fn bound_action(
        &mut self,
        resource: Resource,
        operation: &str,
        values: &[Value],
        _actors: &[Option<ActorRef>],
    ) -> io::Result<Vec<(usize, Value)>> {
        self.action(resource, operation, values)
    }

    /// Calls a plugin method with the receiver first in both parallel slices.
    fn method(
        &mut self,
        operation: &str,
        values: &[Value],
        _actors: &[Option<ActorRef>],
    ) -> io::Result<MethodResult> {
        Ok(MethodResult {
            value: self.request(Request {
                resource: Resource::Plugin,
                operation,
                arguments: values,
            })?,
            updates: Vec::new(),
        })
    }

    /// Releases any platform resources associated with a removed actor.
    fn removed(&mut self, _actor: &ActorRef, _value: &Value) -> io::Result<()> {
        Ok(())
    }

    /// Receives the current root value after a declaration or assignment.
    fn changed(&mut self, _actor: &ActorRef, _root: &Value) -> io::Result<()> {
        Ok(())
    }

    /// Supplies pending events. At book end, a graphical host may wait for input.
    fn event(&mut self, _at_end: bool) -> io::Result<Option<Event>> {
        Ok(None)
    }

    /// Receives the book transcript declarations. True marks input and false marks expected output.
    fn transcript(&mut self, _book: &str, _entries: &[(bool, String)]) -> io::Result<()> {
        Ok(())
    }

    /// Selects a delay for this paragraph. Other paragraph jobs remain runnable.
    fn delay(&mut self, duration: std::time::Duration) -> io::Result<std::time::Duration> {
        Ok(duration)
    }

    /// Checks whether the host wishes to continue before the next instruction.
    fn checkpoint(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn unavailable(operation: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("service indisponible : {operation}"),
    )
}
