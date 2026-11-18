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

Linotte is a programming language with French syntax. The compatibility target is release 3.14 at revision [`0ab8043bd6676e443f279410910ab6a3e1d354e1`](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1). Its grammar and definitions files have their own internal version numbers.

## Running a book

The command `potoo-linotte <file.liv>` reads a UTF-8 book. With a source checkout, use `cargo run --locked --bin potoo-linotte -- <file.liv>`. Source diagnostics include the book path and a one-based line and column. The command reads standard input, writes console output, resolves imported books relative to their source and supports local file-backed actors.

## Language and runtime

- Actors support numbers, text, numeric flags, typed and inferred collections (`casier`) and species. Declarations can initialize multiple actors and collections. Species support attribute templates, inheritance, functional methods and dynamic attributes.
- Expressions support decimal arithmetic, text concatenation and repetition, comparisons, Boolean operations, shifts, numeric functions, lazy ternary selection, nested collections, indexed actors, attribute access and named paragraph calls. Quoted text interpolates expressions inside braces. The runtime preserves unresolved substitutions in the text.
- `affiche` visits each member of a group, collection or species. `demande` reads into a destination actor. The statement verbs also provide copying, conversion, size, searching, insertion, extraction, sorting, shuffling, arithmetic updates and string operations.
- Conditions, alternatives and guarded blocks support inline instructions and block bodies. Loops include `tant que`, inclusive ascending and descending ranges, a `suivant` expression and traversal of numbers, text and collections. Block conditions at `ferme` can still access that iteration's actors. Nested loops expose their current actors through `joker` and its primed forms.
- Books support globals, paragraphs, value returns, typed and inferred parameter aliases, lambdas, named jumps, imported paragraphs and asynchronous paragraph calls. `observe` waits for matching jobs. Calls share actor identities, and host services run on the embedding thread.
- File-backed numbers, text, collections and species use the release's line-oriented format. Species attributes use alphabetical order. The host supplies file contents and receives serialized updates.
- The runtime implements the release's decimal context for basic arithmetic, including increased precision for large operands, plus the `~précision` and `~angle` settings. Transcendental operations pass through binary floating point, as in the original evaluator.

## Embedding

`execute(source, output)` runs a book with a console writer. `execute_with_host(book, source, host)` accepts a `Host` implementation. The host can supply input, book loading, random values, calendar and screen actors, graphical species, collision checks, drawing, audio, plugins, network or file resources, cancellation and callbacks. Commands carry the original operation, evaluated arguments and available actor identities. A host can return updated argument values or call a paragraph in response to an event.

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
```

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
