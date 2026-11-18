use std::io;

use potoo_linotte_interpreter::{
    ActorRef, Event, EventArgument, Host, MethodResult, Request, Resource, Value,
    execute_with_atelier_tests, execute_with_host,
};

#[derive(Default)]
struct ProbeHost {
    output: Vec<u8>,
    requests: Vec<(Resource, String, Vec<Value>)>,
    actions: Vec<(Resource, String, Vec<Value>, Vec<Option<ActorRef>>)>,
    methods: Vec<(String, Vec<Value>, Vec<Option<ActorRef>>)>,
    transcript: Vec<(bool, String)>,
    loaded: Vec<(String, String)>,
    events: Vec<bool>,
    send_event: bool,
}

impl Host for ProbeHost {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.output.extend_from_slice(bytes);
        Ok(())
    }

    fn load(&mut self, from: &str, path: &str) -> io::Result<(String, String)> {
        self.loaded.push((from.to_owned(), path.to_owned()));
        Ok((
            "child.liv".to_owned(),
            "nomme Livre enfant\nbonjour :\n    affiche \"{livre}\"".to_owned(),
        ))
    }

    fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
        self.requests.push((
            request.resource,
            request.operation.to_owned(),
            request.arguments.to_vec(),
        ));
        let value = match (request.resource, request.operation, request.arguments) {
            (Resource::Clock, "année", []) => Some(Value::Number(2026.into())),
            (Resource::Random, "hasard", [_]) => Some(Value::Number(7.into())),
            (Resource::Media, "'do'", []) => Some(Value::Text("Do".to_owned())),
            (Resource::Graphics, "ecranh", []) => Some(Value::Number(800.into())),
            (Resource::Graphics | Resource::Plugin, "crée", [Value::Text(name)]) => {
                let attributes = if name.starts_with("majordometest") {
                    vec![("serviteur".to_owned(), Value::Text("Nestor".to_owned()))]
                } else {
                    Vec::new()
                };
                Some(Value::Species {
                    name: name.clone(),
                    attributes,
                })
            }
            (Resource::Files, "lire", [Value::Text(_), Value::Text(path)])
                if path == "utf16.txt" =>
            {
                Some(Value::TextUnits(vec![55296]))
            }
            (Resource::Files, "lire", [Value::Text(_), Value::Text(path)])
                if path == "line-endings.txt" =>
            {
                Some(Value::Text("alpha\rbeta\r\ngamma\ndelta".to_owned()))
            }
            (Resource::Files, "lire", [Value::Text(_), Value::Text(_)]) => {
                Some(Value::Text("fichier chargé".to_owned()))
            }
            (Resource::Files, "écrire", [Value::Text(_), Value::Text(_), _]) => None,
            _ => None,
        };
        Ok(value)
    }

    fn action(
        &mut self,
        resource: Resource,
        operation: &str,
        arguments: &[Value],
    ) -> io::Result<Vec<(usize, Value)>> {
        self.actions.push((
            resource,
            operation.to_owned(),
            arguments.to_vec(),
            Vec::new(),
        ));
        Ok(Vec::new())
    }

    fn bound_action(
        &mut self,
        resource: Resource,
        operation: &str,
        values: &[Value],
        actors: &[Option<ActorRef>],
    ) -> io::Result<Vec<(usize, Value)>> {
        self.actions.push((
            resource,
            operation.to_owned(),
            values.to_vec(),
            actors.to_vec(),
        ));
        Ok(Vec::new())
    }

    fn method(
        &mut self,
        operation: &str,
        values: &[Value],
        actors: &[Option<ActorRef>],
    ) -> io::Result<MethodResult> {
        self.methods
            .push((operation.to_owned(), values.to_vec(), actors.to_vec()));
        let result = match (operation, values) {
            ("addition", [_, Value::Number(left), Value::Number(right)]) => {
                Some(Value::Number(left + right))
            }
            ("présentation", [Value::Species { attributes, .. }, Value::Text(name)]) => {
                let servant = attributes
                    .iter()
                    .find(|(key, _)| key == "serviteur")
                    .map(|(_, value)| value.to_string())
                    .unwrap_or_else(|| "Nestor".to_owned());
                Some(Value::Text(format!(
                    "Bonsoir, mon cher {name}, je suis votre serviteur {servant}"
                )))
            }
            ("connexion", [_, Value::Text(_), Value::Number(_)]) => {
                Some(Value::Text("connecté".to_owned()))
            }
            ("echo", [_, value]) => Some(value.clone()),
            ("addition_test", [_, Value::Number(_), Value::Number(_)]) => {
                Some(Value::Number(5.into()))
            }
            _ => None,
        };
        let updates = if operation == "addition_test" {
            vec![(1, Value::Number(7.into())), (2, Value::Number(11.into()))]
        } else {
            Vec::new()
        };
        Ok(MethodResult {
            value: result,
            updates,
        })
    }

    fn transcript(&mut self, _book: &str, entries: &[(bool, String)]) -> io::Result<()> {
        self.transcript.extend_from_slice(entries);
        Ok(())
    }

    fn event(&mut self, at_end: bool) -> io::Result<Option<Event>> {
        self.events.push(at_end);
        if at_end && self.send_event {
            self.send_event = false;
            return Ok(Some(Event {
                paragraph: "on_event".to_owned(),
                arguments: vec![EventArgument::Value(Value::Text("hôte".to_owned()))],
            }));
        }
        Ok(None)
    }
}

#[test]
fn official_314_boulier_plugin_example_uses_the_host_method() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "boulier.liv",
        "calculatrice est un boulier\naffiche calculatrice.addition(23,55)",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, b"78\n");
    assert_eq!(host.methods[0].0, "addition");
    assert_eq!(host.methods[0].2[0].as_ref().unwrap().name, "calculatrice");
}

#[test]
fn official_314_majordome_plugin_examples_keep_default_and_explicit_attributes() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "plugin.liv",
        "messager est un majordometest\naffiche messager.présentation(\"Jules\")\nmessager2 est un majordometest2, serviteur vaut \"Claude\"\naffiche messager2.présentation(\"Jules\")",
        &mut host,
    )
    .unwrap();

    assert_eq!(
        host.output,
        "Bonsoir, mon cher Jules, je suis votre serviteur Nestor\nBonsoir, mon cher Jules, je suis votre serviteur Claude\n".as_bytes()
    );
}

#[test]
fn plugin_method_updates_write_back_to_the_original_argument_actors() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "method.liv",
        "device est un boulier\na est un nombre valant 2\nb est un nombre valant 3\nresult est un nombre valant device.addition_test(a,b)\naffiche result + a + b",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, b"23\n");
    let (_, _, actors) = &host.methods[0];
    assert_eq!(actors[0].as_ref().unwrap().name, "device");
    assert_eq!(actors[1].as_ref().unwrap().name, "a");
    assert_eq!(actors[2].as_ref().unwrap().name, "b");
}

#[test]
fn plugin_values_preserve_unpaired_utf16_units() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "method.liv",
        "device est un boulier\naffiche device.echo(chr(55296))",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, "�\n".as_bytes());
    assert_eq!(host.methods[0].1[1], Value::TextUnits(vec![55296]));
}

#[test]
fn file_actor_load_and_save_preserve_utf16_units_through_the_host() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "file.liv",
        "texte est un texte depuis \"utf16.txt\"\naffiche ascii(texte)\ntexte prend chr(55296)\naffiche ascii(texte)",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, b"55296\n55296\n");
    let saved = host
        .requests
        .iter()
        .find(|(_, operation, _)| operation == "écrire")
        .unwrap();
    assert_eq!(saved.2[2], Value::TextUnits(vec![55296]));
}

#[test]
fn text_mixing_shuffles_utf16_units_like_the_java_string_operation() {
    struct FixedRandom(Vec<u8>);
    impl Host for FixedRandom {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }

        fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
            assert_eq!(request.resource, Resource::Random);
            assert_eq!(request.operation, "hasard");
            Ok(Some(Value::Number(0.into())))
        }
    }

    let mut host = FixedRandom(Vec::new());
    execute_with_host(
        "shuffle.liv",
        "texte est un texte valant \"😀\"\ntexte prend texte.mélanger()\naffiche ascii(texte{0})\naffiche ascii(texte{1})",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.0, b"56832\n55357\n");
}

#[test]
fn host_capabilities_receive_clock_random_media_graphics_files_and_network_calls() {
    let source = "affiche année\naffiche hasard(10)\naffiche 'do'\naffiche ecranh\ntexte est un texte depuis \"notes.txt\"\naffiche texte\ntexte prend \"modifié\"\nrecharge texte\naffiche texte\npoint est un point\nmontre point\nfais réagir point à \"clic souris\" pour clique\njoue 60\nclient est un tcpclient\naffiche client.connexion(\"localhost\", 8080)\nclique :\n    affiche \"clic\"";
    let mut host = ProbeHost::default();
    execute_with_host("demo.liv", source, &mut host).unwrap();

    assert_eq!(
        host.output,
        "2026\n7\nDo\n800\nfichier chargé\nfichier chargé\nconnecté\n".as_bytes()
    );
    for expected in [
        (Resource::Clock, "année"),
        (Resource::Random, "hasard"),
        (Resource::Media, "'do'"),
        (Resource::Graphics, "ecranh"),
        (Resource::Files, "lire"),
        (Resource::Graphics, "crée"),
        (Resource::Plugin, "crée"),
    ] {
        assert!(
            host.requests
                .iter()
                .any(|(resource, operation, _)| { (*resource, operation.as_str()) == expected })
        );
    }
    assert!(host.actions.iter().any(|(resource, operation, _, actors)| {
        *resource == Resource::Graphics
            && operation == "montre"
            && actors[0]
                .as_ref()
                .is_some_and(|actor| actor.name == "point")
    }));
    assert!(
        host.actions
            .iter()
            .any(|(resource, operation, values, actors)| {
                *resource == Resource::Graphics
                    && operation == "fais réagir"
                    && values[1] == Value::Text("clic souris".to_owned())
                    && actors[0]
                        .as_ref()
                        .is_some_and(|actor| actor.name == "point")
            })
    );
    assert!(host.actions.iter().any(|(resource, operation, _, _)| {
        *resource == Resource::Media && operation == "joue"
    }));
    assert!(
        host.methods
            .iter()
            .any(|(operation, _, _)| operation == "connexion")
    );
    assert_eq!(
        host.requests
            .iter()
            .filter(|(resource, operation, _)| {
                *resource == Resource::Files && operation == "lire"
            })
            .count(),
        2
    );
}

#[test]
fn imported_books_keep_their_declared_title_and_load_through_the_host() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "main.liv",
        "bibliothèque\n    livre1 référence \"child.liv\"\nparcours bonjour de livre1",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, b"Livre enfant\n");
    assert_eq!(
        host.loaded,
        vec![("main.liv".to_owned(), "child.liv".to_owned())]
    );
}

#[test]
fn atelier_test_declarations_are_metadata_for_the_host() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "tests.liv",
        "tests\n    > prénom demandé\n    < Bonjour Jade\naffiche \"execution normale\"",
        &mut host,
    )
    .unwrap();

    assert_eq!(
        host.transcript,
        vec![
            (true, "prénom demandé".into()),
            (false, "Bonjour Jade".into())
        ]
    );
    assert_eq!(host.output, b"execution normale\n");
}

#[test]
fn atelier_test_mode_consumes_expected_input_output_and_wildcards() {
    let mut host = ProbeHost::default();
    let checks = execute_with_atelier_tests(
        "tests.liv",
        "tests\n    > Jade\n    < Bonjour Jade\n    < *\nefface tableau\ntexte est un texte\ndemande texte\naffiche \"Bonjour \" + texte\naffiche \"valeur variable\"",
        &mut host,
    )
    .unwrap();

    assert_eq!(checks, 3);
    assert!(host.output.is_empty());
    assert!(
        !host
            .actions
            .iter()
            .any(|(_, operation, _, _)| operation == "efface tableau")
    );
}

#[test]
fn atelier_test_mode_reads_automatic_paragraph_parameters_without_testing_the_prompt() {
    let mut host = ProbeHost::default();
    let checks = execute_with_atelier_tests(
        "tests.liv",
        "tests\n    > 41\n    < 42\nprincipal : valeur\n    retourne valeur + 1",
        &mut host,
    )
    .unwrap();

    assert_eq!(checks, 2);
    assert_eq!(host.output, b"");
}

#[test]
fn atelier_test_mode_checks_questionne_display_before_scripted_input() {
    let mut host = ProbeHost::default();
    let checks = execute_with_atelier_tests(
        "tests.liv",
        "tests\n    < Ton prénom ?\n    > Jade\n    < Jade\nprénom est un texte\nquestionne prénom sur \"Ton prénom ?\"\naffiche prénom",
        &mut host,
    )
    .unwrap();

    assert_eq!(checks, 3);
    assert_eq!(host.output, b"");
}

#[test]
fn file_actor_load_splits_cr_lf_and_crlf_like_java_read_line() {
    let mut host = ProbeHost::default();
    execute_with_host(
        "file.liv",
        "lignes est un casier de textes depuis \"line-endings.txt\"\nligne est un texte\npour chaque ligne de lignes affiche ligne",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, b"alpha\nbeta\ngamma\ndelta\n");
}

#[test]
fn atelier_test_mode_reports_a_mismatch_and_excess_output() {
    let mut host = ProbeHost::default();
    let mismatch = execute_with_atelier_tests(
        "tests.liv",
        "tests\n    < attendu\naffiche \"obtenu\"",
        &mut host,
    )
    .unwrap_err();
    assert!(mismatch.message.contains("test"), "{}", mismatch.message);

    let excess = execute_with_atelier_tests(
        "tests.liv",
        "tests\n    < attendu\naffiche \"attendu\"\naffiche \"en trop\"",
        &mut host,
    )
    .unwrap_err();
    assert!(excess.message.contains("plus de test"));
}

#[test]
fn platform_events_call_the_named_paragraph_with_supplied_values() {
    let mut host = ProbeHost {
        send_event: true,
        ..ProbeHost::default()
    };
    execute_with_host(
        "events.liv",
        "affiche \"start\"\non_event : text\n    affiche text",
        &mut host,
    )
    .unwrap();

    assert_eq!(host.output, "start\nhôte\n".as_bytes());
    assert!(host.events.contains(&true));
}
