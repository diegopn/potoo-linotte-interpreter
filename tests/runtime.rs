use std::io;

use potoo_linotte_interpreter::{Host, Value, execute, execute_with_host};

#[test]
fn documented_hello_book_writes_one_line() {
    let mut output = Vec::new();

    execute("affiche \"Bonjour !\"", &mut output).unwrap();

    assert_eq!(output, b"Bonjour !\n");
}

#[test]
fn inclusive_range_executes_once_for_each_number() {
    let source = r#"compteur est un nombre
pour compteur de 1 à 3 affiche compteur"#;
    let mut output = Vec::new();

    execute(source, &mut output).unwrap();

    assert_eq!(output, b"1\n2\n3\n");
}

#[test]
fn collection_iteration_exposes_each_value_through_joker() {
    let source = r#"nombres est un casier de nombres valant 2, 4, 6
pour chaque nombres
    affiche joker * joker
ferme"#;
    let mut output = Vec::new();

    execute(source, &mut output).unwrap();

    assert_eq!(output, b"4\n16\n36\n");
}

#[test]
fn paragraph_parameters_and_return_values_are_evaluated() {
    let source = r#"double : nombre
    retourne nombre * 2

affiche double(4)"#;
    let mut output = Vec::new();

    execute(source, &mut output).unwrap();

    assert_eq!(output, b"8\n");
}

#[test]
fn conditional_executes_only_the_matching_branch() {
    let source = r#"note est un nombre
note prend 12
si note >= 10 affiche "Très bien !"
sinon affiche "Continue à pratiquer""#;
    let mut output = Vec::new();

    execute(source, &mut output).unwrap();

    assert_eq!(output, "Très bien !\n".as_bytes());
}

#[test]
fn demande_reads_the_named_text_actor_from_the_host() {
    #[derive(Default)]
    struct InputHost {
        output: Vec<u8>,
        requested_actor: Option<String>,
    }

    impl Host for InputHost {
        fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.output.extend_from_slice(bytes);
            Ok(())
        }

        fn read(&mut self, actor: &str, current: &Value) -> io::Result<Option<String>> {
            assert_eq!(current, &Value::Text(String::new()));
            self.requested_actor = Some(actor.to_owned());
            Ok(Some("Jade".to_owned()))
        }
    }

    let source = r#"prénom est un texte
affiche "Quel est ton prénom ?"
demande prénom
affiche "Bonjour " + prénom"#;
    let mut host = InputHost::default();

    execute_with_host("salut.liv", source, &mut host).unwrap();

    assert_eq!(host.requested_actor.as_deref(), Some("prénom"));
    assert_eq!(
        host.output,
        "Quel est ton prénom ?\nBonjour Jade\n".as_bytes()
    );
}

#[test]
fn invalid_actor_reports_its_source_line_and_column() {
    let diagnostic = execute("affiche acteur_inexistant", &mut Vec::new()).unwrap_err();

    assert_eq!(diagnostic.line, 1);
    assert!(diagnostic.column > 0);
}
