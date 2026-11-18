use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const CLI: &str = env!("CARGO_BIN_EXE_potoo-linotte");

struct ProgramDirectory(std::path::PathBuf);

impl ProgramDirectory {
    fn new() -> Self {
        let unique = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("potoo-linotte-{}-{unique}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    fn write(&self, source: &str) -> std::path::PathBuf {
        let path = self.0.join("programme.liv");
        fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for ProgramDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_runs_a_liv_file_and_writes_program_output() {
    let directory = ProgramDirectory::new();
    let path = directory.write("affiche \"Bonjour !\"");

    let output = Command::new(CLI).arg(&path).output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Bonjour !\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_reads_piped_input_for_demande() {
    let directory = ProgramDirectory::new();
    let path = directory.write(
        "prénom est un texte\naffiche \"Quel est ton prénom ?\"\ndemande prénom\naffiche \"Bonjour \" + prénom",
    );
    let mut child = Command::new(CLI)
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(b"Jade\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        "Quel est ton prénom ?\nBonjour Jade\n".as_bytes()
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_prefixes_diagnostics_with_the_source_file_and_location() {
    let directory = ProgramDirectory::new();
    let path = directory.write("affiche acteur_inexistant");

    let output = Command::new(CLI).arg(&path).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("programme.liv:1:"), "{stderr}");
}

#[test]
fn cli_requires_exactly_one_source_file() {
    let output = Command::new(CLI).output().unwrap();
    let multiple = Command::new(CLI)
        .args(["premier.liv", "second.liv"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("potoo-linotte <fichier.liv>"));
    assert_eq!(multiple.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&multiple.stderr).contains("potoo-linotte <fichier.liv>"));
}
