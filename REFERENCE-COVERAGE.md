# Linotte reference coverage

This crate targets Linotte 3.14 at revision `0ab8043bd6676e443f279410910ab6a3e1d354e1`. The target is the pinned release grammar, vocabulary, execution actions, examples, and numeric implementation. It does not claim support for every profile or extension configurable in the Atelier.

| Reference area | Implemented profile | Boundary or validation status |
| --- | --- | --- |
| Books and vocabulary | UTF-8 books, declared values, actors, instructions, expressions, and the selected release's language vocabulary | Covered by runtime, parser, CLI, and official-example tests; not all Atelier configuration is represented. |
| Values, expressions, and mathematics | Numeric and text operations, comparisons, conversions, and the pinned release's mathematical evaluation | Java locale-dependent case conversion and random-number sequences are not reproduced exactly. |
| Flow and routines | Structured actions, paragraphs, calls, return behavior, and the tested control-flow forms from the selected grammar/actions | Nested parsing, calls, imports, and values have a depth limit of 128; this is a Rust runtime guard, not a Linotte rule. |
| Actors and services | Built-in action vocabulary and typed host requests for actor, plugin, file, date/time, random, graphics, audio, and network interactions | The standalone CLI provides only selected terminal, file, calendar, random, and delay behavior. Other services require an embedding host. |
| Modules and imports | Book imports resolved through host services, preserving each imported book's source path for nested imports | Resolver behavior is tested; IDE integration and every Atelier plugin remain unverified. |
| Atelier test declarations | Declared input/output transcript checks are available through an explicit test-mode entry point; ordinary execution preserves declarations as metadata | This mode is opt-in and does not change normal book execution. |
| Text encoding | Internal text can preserve UTF-16 code units, including isolated surrogates, through Value::TextUnits | Console input/output crosses the UTF-8 host boundary; isolated surrogates are replaced when rendered there. |

Existing representative coverage is in tests/runtime.rs, tests/host.rs, tests/cli.rs, and the pinned official examples under tests.

## Sources

- [Linotte 3 tutorial](https://github.com/cpc6128/LangageLinotte/wiki/D%C3%A9couvrir-Linotte-3-en-dix-minutes)
- [Linotte 3.14 examples](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/exemples)
- [Pinned grammar and vocabulary](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-resources/src/main/resources/grammaires)
- [Pinned execution actions](https://github.com/cpc6128/LangageLinotte/tree/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/actions)
- [Pinned numeric evaluation](https://github.com/cpc6128/LangageLinotte/blob/0ab8043bd6676e443f279410910ab6a3e1d354e1/linotte-moteur/src/main/java/org/linotte/moteur/xml/analyse/Mathematiques.java)
