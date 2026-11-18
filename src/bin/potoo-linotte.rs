#![forbid(unsafe_code)]

use std::collections::hash_map::RandomState;
use std::env;
use std::fs;
use std::hash::BuildHasher;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use potoo_linotte_interpreter::{BigDecimal, Host, Request, Resource, Value, execute_with_host};

struct Console;

impl Host for Console {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::stdout().lock().write_all(bytes)
    }

    fn read(&mut self, _actor: &str, current: &Value) -> io::Result<Option<String>> {
        loop {
            if io::stdin().is_terminal() {
                self.write(b">")?;
                io::stdout().flush()?;
            }
            let mut raw = String::new();
            if io::stdin().read_line(&mut raw)? == 0 {
                return Ok(None);
            }
            let line = raw.trim_end_matches(['\n', '\r']);
            if matches!(current, Value::Number(_)) && line.parse::<BigDecimal>().is_err() {
                self.write(b"!Nombre non valide!\n")?;
            } else {
                return Ok(Some(line.to_owned()));
            }
        }
    }

    fn load(&mut self, from: &str, path: &str) -> io::Result<(String, String)> {
        let path = resolve(from, path).canonicalize()?;
        let source = fs::read_to_string(&path)?;
        Ok((path.to_string_lossy().into_owned(), source))
    }

    fn request(&mut self, request: Request<'_>) -> io::Result<Option<Value>> {
        match (request.resource, request.operation, request.arguments) {
            (Resource::Clock, field, []) => {
                local_clock(field).map(|n| Some(Value::Number(n.into())))
            }
            (Resource::Random, "hasard", [Value::Number(limit)]) => {
                let bits = RandomState::new().hash_one(0u8) >> 11;
                let sample = BigDecimal::from(bits) * limit / BigDecimal::from(1u64 << 53);
                Ok(Some(Value::Number(
                    sample.with_scale_round(0, bigdecimal::RoundingMode::Floor),
                )))
            }
            (Resource::Console, "efface tableau", _) => {
                if io::stdout().is_terminal() {
                    self.write(b"\x1b[2J\x1b[H")?;
                }
                Ok(None)
            }
            (Resource::Files, "lire", [Value::Text(book), Value::Text(path)]) => {
                match fs::read_to_string(resolve(book, path)) {
                    Ok(text) => Ok(Some(Value::Text(text))),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(e),
                }
            }
            (Resource::Files, "écrire", [Value::Text(book), Value::Text(path), content]) => {
                let text = match content {
                    Value::Text(text) => text.clone(),
                    Value::TextUnits(units) => String::from_utf16_lossy(units),
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "contenu de fichier invalide",
                        ));
                    }
                };
                fs::write(resolve(book, path), text)?;
                Ok(None)
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("service indisponible : {}", request.operation),
            )),
        }
    }
}

fn local_clock(field: &str) -> io::Result<i32> {
    let index = ["année", "mois", "jour", "heure", "minute", "seconde"]
        .iter()
        .position(|&name| name == field)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "champ d’horloge inconnu"))?;
    #[cfg(windows)]
    let clock = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Date -Format 'yyyy MM dd HH mm ss'",
        ])
        .output()?;
    #[cfg(not(windows))]
    let clock = std::process::Command::new("date")
        .arg("+%Y %m %d %H %M %S")
        .output()?;
    if !clock.status.success() {
        return Err(io::Error::other("lecture de l’horloge impossible"));
    }
    std::str::from_utf8(&clock.stdout)
        .ok()
        .and_then(|s| s.split_whitespace().nth(index))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::other("date locale invalide"))
}

fn resolve(book: &str, path: &str) -> PathBuf {
    Path::new(book)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(path)
}

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let (Some(path), None) = (args.next(), args.next()) else {
        eprintln!("Utilisation : potoo-linotte <fichier.liv>");
        return ExitCode::from(2);
    };
    let path = Path::new(&path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = execute_with_host(&path.to_string_lossy(), &source, &mut Console) {
        eprintln!("{}:{error}", path.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
