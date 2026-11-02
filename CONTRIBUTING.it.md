# Contribuire a Quado

[English](CONTRIBUTING.md)

## Licenza dei contributi

Quado è distribuito con la GNU General Public License, versione 3 o successiva (`GPL-3.0-or-later`, file [LICENSE](LICENSE)). I contributi (codice, testi, regole, documenti) sono accettati con la stessa licenza. Chi invia un contributo dichiara di averlo scritto o di avere il diritto di inviarlo con quella licenza.

## Prima di aprire una pull request

I testi dell'interfaccia seguono [docs/testi.md](docs/testi.md). La guida definisce la voce, le regole per ogni componente e il glossario dei termini fissi per le due lingue. Leggerlo prima di modificare un testo nei file `.ftl`, e modificare le due lingue nello stesso commit. I test falliscono se i due file hanno chiavi o segnaposto diversi, o se un testo usa una delle parole vietate.

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

I commenti del codice, la guida dei testi dell'interfaccia e i nomi dei comandi `xtask` sono in italiano. Per partecipare non è necessario conoscere l'italiano: issue e pull request si possono scrivere in inglese. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (in inglese) descrive i crate e come lavorano insieme.

Non eseguire `cargo fmt`. Il codice non segue l'impaginazione di rustfmt, quindi rustfmt rimanderebbe a capo le righe lunghe in tutto il file che tocca. Nel solo `settings_view.rs` cambia 40 punti.

Mantenere i commit piccoli: un cambiamento per commit, con un messaggio che dice cosa cambia.

## Regole

Una regola in `crates/quado-kb/regole` richiede una fonte (URL, titolo ed estratto citato), una data di verifica e una fixture anonimizzata. Se dipende dalla versione di un programma, richiede anche l'intervallo di versioni su cui è stata controllata. `capcut-versioni.toml` è un esempio completo. I testi della regola vanno in `crates/quado-kb/regole/it.ftl` e `en.ftl`.

## Traduzioni

I testi dell'interfaccia sono file Fluent in `crates/quado-ui/lingue`, quelli delle regole in `crates/quado-kb/regole`. L'italiano è la lingua di riferimento: in `it.ftl` ogni chiave ha un commento che indica dove compare il testo.

Un testo si può correggere, e una lingua si può aggiungere, senza ricompilare:

1. In Impostazioni › Lingua, selezionare «Crea un modello da tradurre». Quado copia il file italiano, con i commenti, in `%APPDATA%\Quado\lingue` come `modello-da-tradurre.txt`.
2. Rinominare il file con il codice della lingua, per esempio `fr.ftl`, e tradurre i valori.
3. Selezionare «Ricarica i file».

Le chiavi assenti dal file vengono prese dall'inglese, poi dall'italiano. Per aggiungere una lingua all'eseguibile, `cargo xtask lingue nuova fr` crea `crates/quado-ui/lingue/fr.ftl`, e `cargo xtask lingue` elenca le chiavi mancanti in ogni lingua.

## Nome precedente

Quado si chiamava Rilievo fino al 26/10/2026. Il nome precedente compare ancora nel codice che copia i dati da `%APPDATA%\Rilievo` e in alcuni comandi di sviluppo.
