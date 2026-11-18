<p align="center">
  <img src="https://diegopn.github.io/potoo-site/src/assets/images/language-interpreter-icon.svg?v=1c651ca7" alt="Potoo IDE language interpreter icon" width="120" />
</p>

<h1 align="center">Linotte Interpreter</h1>

<p align="center">An independent Rust interpreter library and CLI for the Linotte programming language.</p>

<p align="center">
  <a href="https://github.com/diegopn/potoo-linotte-interpreter/releases/latest">Download</a> ·
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/linotte/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Originally developed as part of the Potoo IDE project, this interpreter is an independent Rust software package intended to be distributed separately from the IDE. It can be embedded as a library in other Rust projects or run as a standalone command-line tool on Linux, macOS, and Windows.

Linotte is a programming language with French syntax. This interpreter targets release 3.14 at revision [`0ab8043bd6676e443f279410910ab6a3e1d354e1`](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1).

## Original language references

- [Linotte 3 tutorial](https://github.com/cpc6128/LangageLinotte/wiki/D%C3%A9couvrir-Linotte-3-en-dix-minutes) and [examples from the 3.14 release](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/exemples)
- [Grammar and vocabulary](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/resources/grammaires) and [execution actions](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/actions)
- [Numeric evaluation](https://github.com/cpc6128/LangageLinotte/blob/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/analyse/Mathematiques.java) and [arithmetic operations](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/operation/simple)

The original repository specifies GPL version 3. This independent implementation is licensed under [GPL-3.0-or-later](LICENSE).

## Source example

Save this Linotte book as `bonjour.liv`:

```text
affiche "Bonjour !"
```

## Standalone command-line runner

### Install from source

Install Rust 1.93 or later, then install the CLI from the `develop` branch:

```sh
cargo install --git https://github.com/diegopn/potoo-linotte-interpreter.git --branch develop --locked --bin potoo-linotte
```

Alternatively, clone the repository and build or install the checked-out source:

```sh
git clone --branch develop https://github.com/diegopn/potoo-linotte-interpreter.git
cd potoo-linotte-interpreter
cargo install --path . --locked --bin potoo-linotte
```

To run a book directly from a source checkout without installing the executable, use:

```sh
cargo run --locked --bin potoo-linotte -- bonjour.liv
```

Cargo places an installed executable in its configured binary directory. Ensure that directory is included in your `PATH` to invoke `potoo-linotte` from any folder.

### Command-line usage

```text
potoo-linotte <file.liv>
```

The command accepts exactly one source file and has no read-eval-print loop or command-line options. Linotte programs can use `demande` to read from standard input. When standard input is a terminal, the runner displays `>` before each read; when input is piped, it reads without the terminal marker. For example, this book asks for a name:

```text
prénom est un texte
affiche "Quel est ton prénom ?"
demande prénom
affiche "Bonjour " + prénom
```

Run it interactively with `potoo-linotte salut.liv`, or provide a line without an interactive terminal:

```sh
printf 'Jade\n' | potoo-linotte salut.liv
```

For a numeric actor, the runner repeats the input when it cannot parse the value as a number. File, parsing and runtime errors are reported to standard error with the book path and source location when available. The process exits with a failure status when it cannot run the book; invalid command-line usage exits with status 2.

## Use as a Rust library

Add the crate to a Rust application's `Cargo.toml`:

```toml
[dependencies]
potoo-linotte-interpreter = { git = "https://github.com/diegopn/potoo-linotte-interpreter.git", branch = "develop" }
```

Use `execute` for a book that only needs console output:

```rust
use potoo_linotte_interpreter::execute;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"affiche "Bonjour !""#;
    let mut output = Vec::new();

    execute(source, &mut output)?;
    print!("{}", String::from_utf8(output)?);
    Ok(())
}
```

`execute_with_host(book, source, host)` accepts a `Host` for input, books, files, random values, dates, graphics, audio, plugins, network access, cancellation, and callbacks.

Potoo IDE supplies platform services through this interface. The standalone CLI provides terminal input and output, local books and files, random numbers, calendar fields, and delays.

Compatibility targets the pinned 3.14 release. Atelier test declarations are not executed by this interpreter; platform services are provided by the host.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

## Support

If this interpreter is useful in your work, you can support its continued development:

<p align="left">
  <a href="https://github.com/sponsors/diegopn"><img alt="Become a sponsor" src="https://img.shields.io/badge/Become_a_sponsor-EA4AAA?style=for-the-badge&logo=githubsponsors&logoColor=white"></a>
</p>

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
