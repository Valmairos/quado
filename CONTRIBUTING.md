# Contributing to Quado

[Italiano](CONTRIBUTING.it.md)

## License of contributions

Quado is distributed under the GNU General Public License, version 3 or later (`GPL-3.0-or-later`, see [LICENSE](LICENSE)). Contributions (code, texts, rules, documents) are accepted under the same license. By submitting a contribution, the contributor states that they wrote it or have the right to submit it under that license.

## Before opening a pull request

Interface texts follow [docs/testi.md](docs/testi.md) (in Italian), which defines the voice, the rules for each component and the glossary of fixed terms for both languages. Read it before changing a string in the `.ftl` files, and change both languages in the same commit. The tests fail when the two files have different keys or placeholders, or when a text uses one of the forbidden words.

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

Code comments are in Italian, like the documents in `docs/` and the names of the `xtask` commands. Italian is not required to take part: issues and pull requests can be written in English.

Do not run `cargo fmt`. The code does not follow the rustfmt layout, so a change in one place of a file would make rustfmt rewrap long lines throughout that file; in `settings_view.rs` alone, it changes 40 places.

Keep commits small: one change per commit, with a message that states what changed.

## Rules

A rule in `crates/quado-kb/regole` requires a source (URL, title and a quoted excerpt), a verification date and an anonymized fixture. When it depends on the version of a program, it also requires the range of versions on which it was checked. `capcut-versioni.toml` is a complete example. The texts of the rule go in `crates/quado-kb/regole/it.ftl` and `en.ftl`.

## Former name

Quado was named Rilievo until October 26, 2026. The former name still appears in older documents and in the code that copies data from `%APPDATA%\Rilievo`.
