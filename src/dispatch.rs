use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use crate::{ActorRef, Diagnostic, Event, Host, Request, Resource, Value};

type Service = Box<dyn FnOnce(&mut dyn Host) + Send>;

/// Serializes platform calls on the embedding thread while paragraph jobs run independently.
#[derive(Clone)]
pub(crate) struct Proxy {
    sender: mpsc::Sender<Service>,
    cancelled: Arc<AtomicBool>,
}

pub(crate) fn serve(
    host: &mut dyn Host,
    start: impl FnOnce(Proxy) -> Result<(), Diagnostic> + Send,
) -> Result<(), Diagnostic> {
    std::thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel::<Service>();
        let proxy = Proxy {
            sender,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let runner = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(scope, move || start(proxy))
            .map_err(|e| Diagnostic {
                line: 1,
                column: 1,
                message: e.to_string(),
            })?;
        for service in receiver {
            service(host);
        }
        runner.join().map_err(|_| Diagnostic {
            line: 1,
            column: 1,
            message: "arrêt inattendu du livre".into(),
        })?
    })
}

impl Proxy {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    fn send<T: Send + 'static>(
        &self,
        op: impl FnOnce(&mut dyn Host) -> io::Result<T> + Send + 'static,
    ) -> io::Result<T> {
        let (reply, result) = mpsc::sync_channel(1);
        self.sender
            .send(Box::new(move |host| {
                let _ = reply.send(op(host));
            }))
            .map_err(|_| io::Error::other("hôte déconnecté"))?;
        result
            .recv()
            .map_err(|_| io::Error::other("hôte déconnecté"))?
    }
}

impl Host for Proxy {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let bytes = bytes.to_vec();
        self.send(move |h| h.write(&bytes))
    }

    fn read(&mut self, actor: &str, current: &Value) -> io::Result<Option<String>> {
        let actor = actor.to_owned();
        let current = current.clone();
        self.send(move |h| h.read(&actor, &current))
    }

    fn load(&mut self, from: &str, path: &str) -> io::Result<(String, String)> {
        let from = from.to_owned();
        let path = path.to_owned();
        self.send(move |h| h.load(&from, &path))
    }

    fn request(&mut self, r: Request<'_>) -> io::Result<Option<Value>> {
        let resource = r.resource;
        let operation = r.operation.to_owned();
        let arguments = r.arguments.to_vec();
        self.send(move |h| {
            h.request(Request {
                resource,
                operation: &operation,
                arguments: &arguments,
            })
        })
    }

    fn action(
        &mut self,
        resource: Resource,
        operation: &str,
        arguments: &[Value],
    ) -> io::Result<Vec<(usize, Value)>> {
        let operation = operation.to_owned();
        let arguments = arguments.to_vec();
        self.send(move |h| h.action(resource, &operation, &arguments))
    }

    fn bound_action(
        &mut self,
        r: Resource,
        op: &str,
        values: &[Value],
        actors: &[Option<ActorRef>],
    ) -> io::Result<Vec<(usize, Value)>> {
        let op = op.to_owned();
        let values = values.to_vec();
        let actors = actors.to_vec();
        self.send(move |h| h.bound_action(r, &op, &values, &actors))
    }
    fn changed(&mut self, actor: &ActorRef, root: &Value) -> io::Result<()> {
        let actor = actor.clone();
        let root = root.clone();
        self.send(move |h| h.changed(&actor, &root))
    }
    fn event(&mut self, at_end: bool) -> io::Result<Option<Event>> {
        self.send(move |h| h.event(at_end))
    }

    fn transcript(&mut self, book: &str, entries: &[(bool, String)]) -> io::Result<()> {
        let book = book.to_owned();
        let entries = entries.to_vec();
        self.send(move |h| h.transcript(&book, &entries))
    }

    fn delay(&mut self, duration: Duration) -> io::Result<Duration> {
        self.send(move |h| h.delay(duration))
    }

    fn checkpoint(&mut self) -> io::Result<()> {
        if self.cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "livre terminé"));
        }
        self.send(|h| h.checkpoint())
    }
}
