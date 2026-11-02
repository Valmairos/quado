# Contributing to Quado

[Italiano](CONTRIBUTING.it.md)

## License of contributions

Quado is distributed under the GNU General Public License, version 3 or later (`GPL-3.0-or-later`, see [LICENSE](LICENSE)). Contributions (code, texts, rules, documents) are accepted under the same license. By submitting a contribution, the contributor states that they wrote it or have the right to submit it under that license.

## Before opening a pull request

Interface texts follow [docs/testi.md](docs/testi.md) (in Italian). It defines the voice, the rules for each component and the glossary of fixed terms for both languages. Read it before changing a string in the `.ftl` files, and change both languages in the same commit. The tests fail when the two files have different keys or placeholders, or when a text uses one of the forbidden words.

Run the tests and clippy:

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Quado does not use the network, and two checks verify this. No dependency may use the network (`deny.toml` blocks HTTP and WebSocket clients), and the executable may not import network functions:

```
cargo deny check
cargo build --release
cargo xtask importazioni
```

By default, `cargo xtask importazioni` checks `target\release\quado.exe`; pass a path to check a different build.

## Code

Code comments, the guide for interface texts and the names of the `xtask` commands are in Italian. Italian is not required to take part: issues and pull requests can be written in English. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the crates and how they work together.

Do not run `cargo fmt`. The code does not follow the rustfmt layout, so rustfmt would rewrap long lines throughout every file it touches. In `settings_view.rs` alone, it changes 40 places.

Keep commits small: one change per commit, with a message that states what changed.

## Rules

A rule in `crates/quado-kb/regole` requires a source (URL, title and a quoted excerpt), a verification date and an anonymized fixture. When it depends on the version of a program, it also requires the range of versions on which it was checked. `capcut-versioni.toml` is a complete example. The texts of the rule go in `crates/quado-kb/regole/it.ftl` and `en.ftl`.

## Translations

Interface texts are Fluent files in `crates/quado-ui/lingue`, and rule texts are in `crates/quado-kb/regole`. Italian is the reference language: in `it.ftl`, each key has a comment that states where the text appears.

A text can be corrected, or a language added, without recompiling:

1. In Settings › Language, select "Create a template to translate". Quado copies the Italian file, comments included, to `%APPDATA%\Quado\lingue` as `modello-da-tradurre.txt`.
2. Rename the file with the language code, for example `fr.ftl`, and translate the values.
3. Select "Reload files".

Keys missing from the file fall back to English, then to Italian. To add a language to the build, `cargo xtask lingue nuova fr` creates `crates/quado-ui/lingue/fr.ftl`, and `cargo xtask lingue` lists the missing keys of every language.

## Former name

Quado was named Rilievo until October 26, 2026. The former name still appears in the code that copies data from `%APPDATA%\Rilievo` and in some development commands.
