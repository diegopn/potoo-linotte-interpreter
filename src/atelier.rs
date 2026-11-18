use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use crate::{ActorRef, Diagnostic, Event, Host, MethodResult, Position, Request, Resource, Value};

pub(crate) struct TestHost<'a> {
    host: &'a mut dyn Host,
    entries: VecDeque<(bool, String)>,
    checks: usize,
}

impl<'a> TestHost<'a> {
    pub(crate) fn new(host: &'a mut dyn Host, entries: &[(bool, String)]) -> Self {
        Self {
            host,
            entries: entries.iter().cloned().collect(),
            checks: 0,
        }
    }

    pub(crate) fn checks(&self) -> usize {
        self.checks
    }

    fn take(&mut self, input: bool, actual: Option<&str>) -> io::Result<String> {
        let Some((expected_input, expected)) = self.entries.pop_front() else {
            return Err(io::Error::other(if input {
                "les tests ont échoué : plus de test pour la demande"
            } else {
                "les tests ont échoué : plus de test pour l’affichage"
            }));
        };
        if expected_input != input {
            return Err(io::Error::other(if input {
                "échec du test : affichage attendu avant la demande"
            } else {
                "échec du test : demande attendue avant l’affichage"
            }));
        }
        if let Some(actual) = actual
            && expected != "*"
            && expected != actual
        {
            return Err(io::Error::other(format!(
                "échec du test : valeur attendue « {expected} », reçue « {actual} »"
            )));
        }
        self.checks += 1;
        Ok(expected)
    }
}

impl Host for TestHost<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let value = text.strip_suffix('\n').unwrap_or(text);
        self.take(false, Some(value)).map(|_| ())
    }

    fn prompt(&mut self, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }

    fn read(&mut self, _actor: &str, _current: &Value) -> io::Result<Option<String>> {
        self.take(true, None).map(Some)
    }

    fn load(&mut self, from: &str, path: &str) -> io::Result<(String, String)> {
        self.host.load(from, path)
    }

    fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
        self.host.request(request)
    }

    fn action(
        &mut self,
        resource: Resource,
        operation: &str,
        arguments: &[Value],
    ) -> io::Result<Vec<(usize, Value)>> {
        if resource == Resource::Console && operation == "efface tableau" {
            return Ok(Vec::new());
        }
        self.host.action(resource, operation, arguments)
    }

    fn bound_action(
        &mut self,
        resource: Resource,
        operation: &str,
        values: &[Value],
        actors: &[Option<ActorRef>],
    ) -> io::Result<Vec<(usize, Value)>> {
        if resource == Resource::Console && operation == "efface tableau" {
            return Ok(Vec::new());
        }
        self.host.bound_action(resource, operation, values, actors)
    }

    fn method(
        &mut self,
        operation: &str,
        values: &[Value],
        actors: &[Option<ActorRef>],
    ) -> io::Result<MethodResult> {
        self.host.method(operation, values, actors)
    }

    fn removed(&mut self, actor: &ActorRef, value: &Value) -> io::Result<()> {
        self.host.removed(actor, value)
    }

    fn changed(&mut self, actor: &ActorRef, root: &Value) -> io::Result<()> {
        self.host.changed(actor, root)
    }

    fn event(&mut self, at_end: bool) -> io::Result<Option<Event>> {
        self.host.event(at_end)
    }

    fn transcript(&mut self, book: &str, entries: &[(bool, String)]) -> io::Result<()> {
        self.host.transcript(book, entries)
    }

    fn delay(&mut self, duration: Duration) -> io::Result<Duration> {
        self.host.delay(duration)
    }

    fn checkpoint(&mut self) -> io::Result<()> {
        self.host.checkpoint()
    }
}

pub(crate) fn no_tests() -> Diagnostic {
    Position { line: 1, column: 1 }.error("aucune déclaration de test Atelier")
}
