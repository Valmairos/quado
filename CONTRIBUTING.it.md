# Contribuire a Quado

[English](CONTRIBUTING.md)

## Licenza dei contributi

Quado è distribuito con la GNU General Public License, versione 3 o successiva (`GPL-3.0-or-later`, file [LICENSE](LICENSE)). I contributi (codice, testi, regole, documenti) sono accettati con la stessa licenza. Chi invia un contributo dichiara di averlo scritto o di avere il diritto di inviarlo con quella licenza.

## Prima di aprire una pull request

I testi dell'interfaccia seguono [docs/testi.md](docs/testi.md), che definisce la voce, le regole per ogni componente e il glossario dei termini fissi per le due lingue. Leggerlo prima di modificare un testo nei file `.ftl`, e modificare le due lingue nello stesso commit. I test falliscono se i due file hanno chiavi o segnaposto diversi, o se un testo usa una delle parole vietate.

Eseguire i test e clippy:

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Quado non usa la rete, e due controlli lo verificano. Nessuna dipendenza può usare la rete (`deny.toml` vieta i client HTTP e WebSocket), e l'eseguibile non può importare funzioni di rete:

```
cargo deny check
cargo build --release
cargo xtask importazioni
```

Per impostazione predefinita, `cargo xtask importazioni` controlla `target\release\quado.exe`; per controllare un altro eseguibile, passarne il percorso.

## Codice

I commenti del codice sono in italiano, come i documenti in `docs/` e i nomi dei comandi `xtask`. Per partecipare non è necessario conoscere l'italiano: issue e pull request si possono scrivere in inglese.

Non eseguire `cargo fmt`. Il codice non segue l'impaginazione di rustfmt, quindi una modifica in un punto di un file porterebbe rustfmt a rimandare a capo le righe lunghe in tutto il file; nel solo `settings_view.rs` cambia 40 punti.

Mantenere i commit piccoli: un cambiamento per commit, con un messaggio che dice cosa cambia.

## Regole

Una regola in `crates/quado-kb/regole` richiede una fonte (URL, titolo ed estratto citato), una data di verifica e una fixture anonimizzata. Se dipende dalla versione di un programma, richiede anche l'intervallo di versioni su cui è stata controllata. `capcut-versioni.toml` è un esempio completo. I testi della regola vanno in `crates/quado-kb/regole/it.ftl` e `en.ftl`.

## Nome precedente

Quado si chiamava Rilievo fino al 26/10/2026. Il nome precedente compare ancora nei documenti più vecchi e nel codice che copia i dati da `%APPDATA%\Rilievo`.
