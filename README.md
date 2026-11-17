<h1 align="center">Linotte Interpreter</h1>

<p align="center">An initial Rust library and CLI scaffold for the Linotte programming language.</p>

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

Linotte is a programming language with French syntax and its own vocabulary. This repository will document a specific compatibility target before claiming language coverage.

This repository is only a starting scaffold. It does not yet parse or execute Linotte source, and no interpreter features are currently implemented. The Rust implementation will be developed here independently from the original application.

## Original language references

- [Official Linotte project and source repository](https://github.com/cpc6128/LangageLinotte)

These references define the language compatibility work. This repository will list its supported syntax, runtime behavior, and limitations after implementation begins.

## Development

Run these commands from the repository root:

```sh
cargo fmt --all -- --check
cargo check
cargo test
```

## Project policies

- [Code of conduct](CODE_OF_CONDUCT.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [GPL-3.0-or-later license](LICENSE)
