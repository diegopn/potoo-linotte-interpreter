<h1 align="center">Linotte Interpreter</h1>

<p align="center">A Rust library and command-line interpreter for the Linotte programming language.</p>

<p align="center">
  <a href="https://diegopn.github.io/potoo-site/">Website</a> ·
  <a href="https://diegopn.github.io/potoo-site/manual/linotte/">User manual</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img alt="Rust" src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="https://doc.rust-lang.org/cargo/"><img alt="Cargo" src="https://img.shields.io/badge/Cargo-DEA584?style=for-the-badge&logo=rust&logoColor=white"></a>
  <img alt="Rust library" src="https://img.shields.io/badge/Rust_library-4A86CF?style=for-the-badge">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-6A4C93?style=for-the-badge">
</p>

Linotte is a programming language with French syntax, not a pseudolanguage. The compatibility target is release 3.14 at revision [`0ab8043bd6676e443f279410910ab6a3e1d354e1`](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1). Its grammar and definitions files have their own internal version numbers.

## Quick start

Save this Linotte book as `bonjour.liv`:

```text
affiche "Bonjour !"
```

Then run it with:

```sh
potoo-linotte bonjour.liv
```

The CLI reads one UTF-8 `.liv` file. It writes program output to standard output and diagnostics to standard error. Diagnostics include the source path and one-based line and column. Imported books and local file-backed actors resolve paths relative to the book that references them.

## Installation

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

## Command-line usage

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

## Language and runtime

- Actors support numbers, text, numeric flags, typed and inferred collections (`casier`) and species. Declarations can initialize multiple actors and collections. Species support attribute templates, inheritance, functional methods and dynamic attributes.
- Expressions support decimal arithmetic, text concatenation and repetition, comparisons, Boolean operations, shifts, numeric functions, lazy ternary selection, nested collections, indexed actors, attribute access and named paragraph calls. Quoted text interpolates expressions inside braces. The runtime preserves unresolved substitutions in the text.
- `affiche` visits each member of a group, collection or species. `demande` reads into a destination actor. The statement verbs also provide copying, conversion, size, searching, insertion, extraction, sorting, shuffling, arithmetic updates and string operations.
- Conditions, alternatives and guarded blocks support inline instructions and block bodies. Loops include `tant que`, inclusive ascending and descending ranges, a `suivant` expression and traversal of numbers, text and collections. Block conditions at `ferme` can still access that iteration's actors. Nested loops expose their current actors through `joker` and its primed forms.
- Books support globals, paragraphs, value returns, typed and inferred parameter aliases, lambdas, named jumps, imported paragraphs and asynchronous paragraph calls. `observe` waits for matching jobs. Calls share actor identities, and host services run on the embedding thread.
- File-backed numbers, text, collections and species use the release's line-oriented format. Species attributes use alphabetical order. The host supplies file contents and receives serialized updates. The `recharge` verb rereads a bound file. For a collection, it appends the records read from the file.
- The runtime implements the release's decimal context for basic arithmetic, including increased precision for large operands, plus the `~précision` and `~angle` settings. Transcendental operations pass through binary floating point, as in the original evaluator.

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

`execute_with_host(book, source, host)` accepts a `Host` implementation for platform services. A host can provide input, book loading, random values, calendar and screen actors, graphical species, collision checks, drawing, audio, plugins, network or file resources, cancellation and callbacks. Commands carry the original operation, evaluated arguments and available actor identities. Plugin methods receive the receiver identity followed by the argument identities. A method can return a value and update any of those actors. The host receives a removal notification after `annihile` removes a binding and can call a paragraph in response to an event. Services not implemented by the host return a diagnostic to the running book.

Potoo IDE can supply its own windows, widgets, rendering, media and resource policies through this interface. The command-line host supplies terminal input and output, local books and files, random numbers, local calendar fields and delays. A request for an unavailable platform service produces a diagnostic.

## Compatibility boundaries

The parser and runtime were reviewed against the pinned grammar, Java actions and resource definitions. Behavioral equivalence with every release program has not been established. The optional Atelier test declarations are recognized as book metadata. Their execution belongs to the original test tool.

The runtime limits nested expressions, calls and actor structures to 128 levels and diagnoses cyclic collection values. Decimal scale and configured precision follow signed 32-bit limits. Text is stored as valid UTF-8. Operations that encounter an isolated UTF-16 surrogate cannot preserve the original Java code unit as a Rust string. CPU scheduling and host events depend on the embedding platform.

## Original language references

- [Linotte 3 tutorial](https://github.com/cpc6128/LangageLinotte/wiki/D%C3%A9couvrir-Linotte-3-en-dix-minutes) and [examples from the 3.14 release](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/exemples).
- [Grammar and vocabulary](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/resources/grammaires) and [execution actions](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/actions), including `ActeurAction`, `ValoirAction`, `ConditionAction` and `AfficherAction`.
- [Numeric evaluation](https://github.com/cpc6128/LangageLinotte/blob/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/analyse/Mathematiques.java) and [arithmetic operations](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/operation/simple).

The original repository's [LICENSE](https://github.com/cpc6128/LangageLinotte/blob/0ab8043bd6676e443f279410910ab6a3e1d354e1/LICENSE) and [Maven metadata](https://github.com/cpc6128/LangageLinotte/blob/0ab8043bd6676e443f279410910ab6a3e1d354e1/pom.xml) specify GPL version 3. Some original Java files carry an older notice referring to GPL version 2 or later and restricting use to educational and non-commercial purposes. This repository's Rust implementation reimplements the language behavior and uses its own GPL-3.0-or-later license.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

Integration tests under `tests/` cover interpreter execution and the standalone command-line runner.

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
