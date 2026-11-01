<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/hero-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/hero-light.png">
  <img src="docs/images/hero-light.png" width="880" alt="Quado, analizzatore dello spazio su disco per Windows. Sotto il nome, la parte alta della finestra Panoramica con dati di prova: come è usato lo spazio dell'unità, diviso per chi lo gestisce.">
</picture>

[![Licenza: GPL-3.0-or-later](https://img.shields.io/badge/licenza-GPL--3.0--or--later-blue.svg)](LICENSE)
[![CI](https://github.com/Valmairos/quado/actions/workflows/ci.yml/badge.svg)](https://github.com/Valmairos/quado/actions/workflows/ci.yml)

[English](README.md)

Quado è un analizzatore dello spazio su disco per Windows. Mostra quali cartelle occupano più spazio in un'unità e, quando può, che cosa sono.

[Funzionalità](#funzionalità) · [Requisiti](#requisiti) · [Scaricare Quado](#scaricare-quado) · [Privacy](#privacy) · [Compilare dal sorgente](#compilare-dal-sorgente) · [Contribuire](#contribuire)

## Funzionalità

- Analizza unità NTFS, exFAT e FAT32. La scansione legge soltanto e non modifica nessun file.
- Mostra le dimensioni delle cartelle in un elenco e in una mappa, con viste dedicate a file grandi, Download, programmi installati, Windows e sistema.
- Riconosce le cartelle create da programmi comuni e da Windows tramite 26 regole. Le cartelle che nessuna regola riconosce sono indicate come «Non sappiamo».
- Sposta nel Cestino le cartelle selezionate dopo un'anteprima che controlla file in uso, cartelle sincronizzate e capienza del Cestino.
- Per le altre cartelle rimanda all'impostazione di Windows o al programma che le gestisce, oppure copia il comando ufficiale.
- Su richiesta legge come amministratore le cartelle che la scansione normale non può aprire.
- Non contiene codice di rete e non invia dati.
- Interfaccia in italiano e in inglese.

### Regole

Le regole si trovano in `crates/quado-kb/regole`. Ogni regola cita una fonte con un estratto e la data dell'ultima verifica. Una regola che non viene verificata di nuovo per dodici mesi smette di proporre interventi. Le regole si dividono in tre gruppi:

| Regole | Cosa fa Quado |
|---|---|
| 2 | Propone di spostare le cartelle nel Cestino. Queste regole riguardano le versioni vecchie di CapCut e le cartelle dei progetti come `node_modules`, `.next` o `target`. |
| 15 | Rimanda al programma o all'impostazione di Windows che gestisce lo spazio: mostra la procedura (per esempio in Impostazioni › Archiviazione), copia un comando ufficiale o apre la pagina per disinstallare un programma. Dopo «Ho eseguito la procedura», Quado misura di nuovo lo spazio libero. |
| 9 | Spiega che cos'è la cartella, senza proporre interventi. Per parti di Windows come WinSxS e DriverStore, Quado indica «Nessuna procedura: non toccare». |

### Spostamento nel Cestino

Due regole possono proporre di spostare cartelle nel Cestino:

- Cartelle di compilazione dei progetti software, come `node_modules`, `.next` o `target`. Quado ne propone una solo se accanto si trova il lockfile corrispondente, per esempio `Cargo.lock` per `target`, perché in quel caso il progetto può ricostruire la cartella. Non propone mai cartelle di progetto sotto AppData, ProgramData, Programmi, Windows o `.vscode`.
- Versioni precedenti che un programma conserva dopo un aggiornamento. L'unico programma coperto è CapCut. Quado tiene la versione in uso, quella precedente e quelle più nuove che un aggiornamento sta installando.

Quando una regola dipende da un'ipotesi che Quado non può verificare, le sue cartelle sono segnate «Da verificare». In quel caso lo spostamento può partire solo dall'ispettore, dove l'ipotesi e i suoi rischi compaiono sopra il pulsante, e «Svuota il Cestino…» resta disattivato finché non si conferma che il programma si apre ancora.

Per entrambe le regole lo spostamento è preceduto da un'anteprima, che stima lo spazio liberato dopo lo svuotamento del Cestino; fino ad allora il Cestino trattiene i file e lo spazio libero resta più o meno invariato. L'anteprima controlla inoltre i file in uso, i collegamenti, le voci di registro o le attività pianificate che puntano alle cartelle, lo spazio disponibile nel Cestino e le cartelle sincronizzate. Nessun file viene spostato prima della conferma.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/move-preview-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/move-preview-light.png">
  <img src="docs/images/move-preview-light.png" width="880" alt="L'anteprima di Quado prima di spostare nel Cestino le versioni vecchie di CapCut, dopo «Seleziona tutti». Sopra il pulsante: lo spazio che si libera subito e dopo lo svuotamento del Cestino, l'ipotesi e i suoi rischi, gli elementi e i controlli.">
</picture>

### Interventi possibili

Nella Panoramica, la card «Interventi possibili» mostra lo spazio occupato da ogni voce. Una riga separata indica quando quello spazio si libera: «Si libera svuotando il Cestino» oppure «Si libera: lo misura Quado dopo la procedura». Il file di ibernazione e le cache di Chrome, Edge e Brave restano fuori dalla card e compaiono nell'ispettore e nella vista Windows e sistema.

## Schermate

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/overview-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/overview-light.png">
  <img src="docs/images/overview-light.png" width="880" alt="Panoramica: come è usato lo spazio dell'unità e gli elementi più grandi">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/explore-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/explore-light.png">
  <img src="docs/images/explore-light.png" width="880" alt="Esplora: mappa e tabella delle cartelle">
</picture>

Le schermate mostrano Quado in inglese, con dati di prova.

## Requisiti

Quado è stato provato su un solo PC, con Windows 10 22H2 su un processore x64.

| | Provato | Dovrebbe funzionare, non provato | Non supportato |
|---|---|---|---|
| Windows | Windows 10 22H2 | Windows 10 da 1809 a 21H2, Windows 11 | Prima di Windows 10 1809, 32 bit, modalità S |
| Altri sistemi | | | macOS, Linux |
| Processore | x64 | Arm con Windows 11 | Arm con Windows 10 |
| Grafica | Direct3D 12 | WARP, OpenGL | |
| File system | NTFS, exFAT, FAT32 | ReFS, Dev Drive | |
| Unità | Unità interne, VHDX | Unità USB, schede di memoria, unità di rete connesse | Percorsi di rete senza lettera di unità |
| Lingue | Italiano, inglese | | Testo da destra a sinistra |

- La versione minima è dedotta, non provata: `quado.exe` importa `GetDiskSpaceInformationW`, che Windows offre da Windows 10, versione 1809 (build 17763), quindi le versioni precedenti non lo avviano.
- Prima di Windows 10, versione 2004, Windows non indica la cartella delle app dello Store (`Package.InstalledPath`), e la vista Programmi le mostra con «Posizione non dichiarata».
- `quado.exe` è un programma x64, per processori Intel o AMD a 64 bit, quindi Windows a 32 bit non lo avvia. Non lo avvia neppure Windows 10 su Arm, perché emula solo le app x86 a 32 bit. Windows 11 su Arm emula anche le app x64, ma Quado non vi è stato provato.
- Quado disegna con Direct3D 12 sulla scheda video che gestisce il monitor principale. La scheda deve avere un driver Direct3D 12 con livello di funzionalità 11_0 o superiore. Quado è stato provato con una scheda integrata e una dedicata; le altre schede con un driver di questo tipo dovrebbero funzionare.
- Senza quel driver Quado usa WARP, il renderer software di Windows, e la finestra risponde lentamente (circa 230 ms per frame sul primo prototipo). Se Direct3D 12 non si avvia, Quado usa OpenGL. `quado.exe --renderer warp` e `quado.exe --renderer gl` impongono l'uno o l'altro; entrambi sono stati provati solo sul primo prototipo.
- exFAT e FAT32 sono stati provati su dischi virtuali. Questi file system non hanno hard link, e le dimensioni su disco sono le dimensioni arrotondate al cluster.
- Sulle unità di rete connesse e sui file system diversi da NTFS, exFAT e FAT32, Quado segna le sue misure «senza garanzie». Su ReFS e Dev Drive più file possono condividere gli stessi cluster (block cloning), quindi i totali possono superare lo spazio effettivamente usato.
- I file nel cloud sono stati provati con una cartella di sincronizzazione di prova che usa l'API dei file cloud di Windows; OneDrive e le altre app che usano la stessa API dovrebbero funzionare. In una cartella cloud Quado mostra quanto sta nel cloud e quanto su questo disco, senza scaricare i file.
- Quado è stato provato sia quando non era avviato come amministratore sia con «Ripeti come amministratore». Con un account standard dovrebbe funzionare, ma non è stato provato.
- I nomi in arabo ed ebraico sono mostrati da sinistra a destra.

## Scaricare Quado

Scaricare `quado.exe` dalla pagina [Releases](https://github.com/Valmairos/quado/releases). Quado è pubblicato solo su quella pagina; una copia ottenuta da un altro sito può contenere codice diverso. Il programma è un unico file eseguibile e funziona senza installazione. Le versioni di Windows su cui è stato provato sono elencate in [Requisiti](#requisiti).

Conservare `quado.exe` in una cartella dedicata, per esempio `%LOCALAPPDATA%\Programs\Quado`. «Ripeti come amministratore» non parte dalla cartella Download né da una cartella temporanea, perché in quelle cartelle una DLL salvata accanto a `quado.exe` potrebbe essere caricata dalla copia di Quado che legge come amministratore. In quel caso Quado chiede di spostare `quado.exe` e non apre la richiesta di Windows.

### Verificare il file

Prima di avviare `quado.exe`, confrontarne lo SHA-256 con quello indicato nelle note di rilascio. In PowerShell:

```powershell
Get-FileHash .\quado.exe
```

### L'avviso di SmartScreen

`quado.exe` non ha una firma del codice. La prima volta che si apre una copia scaricata, Microsoft Defender SmartScreen può bloccarla con l'avviso «Windows ha protetto il tuo PC». L'avviso indica che SmartScreen non riconosce il file, non che vi abbia trovato un malware. Per avviare il file:

1. Selezionare il collegamento sotto il messaggio che mostra i dettagli dell'app.
2. Controllare che l'app sia `quado.exe` e che l'autore sia sconosciuto.
3. Selezionare «Esegui comunque».

Poiché il file non è firmato, anche «Ripeti come amministratore» apre una richiesta di Windows che indica l'autore come sconosciuto.

Per non avviare un eseguibile scaricato da internet, è possibile [compilare Quado dal sorgente](#compilare-dal-sorgente). Un eseguibile compilato sul proprio PC non è segnato come scaricato, quindi SmartScreen non lo controlla.

## Prima dell'uso

Quado è distribuito senza garanzia, nei limiti consentiti dalla legge. La decisione di spostare un elemento o di eseguire un comando spetta a chi usa Quado, che dovrebbe prima controllare i punti seguenti.

- Le regole sono state verificate su un solo PC, e su un altro PC una regola può riconoscere male una cartella o non essere più aggiornata. Quado scrive «(dedotto)» accanto ai fatti che deduce, e «Da verificare» segna una raccomandazione che si basa su un'ipotesi che Quado non controlla. Prima di confermare, leggere nell'anteprima quale regola si applica e la data della sua ultima verifica.
- Gli elementi spostati restano nel Cestino finché non viene svuotato, a mano o da Sensore memoria. Per ripristinarli, aprire il Cestino, selezionarli e scegliere «Ripristina gli elementi selezionati». Lo svuotamento del Cestino li elimina definitivamente, insieme a tutto il resto del Cestino di quell'unità. Quado non conserva altre copie, quindi prima di svuotare il Cestino occorre fare un backup di ciò che potrebbe servire.
- Quado riconosce le cartelle sincronizzate registrate in Windows, come quelle di OneDrive, e quelle di Dropbox, Syncthing e Resilio Sync. Riconosce Google Drive per desktop solo se la sua posizione è impostata nel registro (`DefaultMountPoint`), e mai in modalità mirroring. Per una cartella riconosciuta, la conferma avverte che lo spostamento elimina l'elemento anche nel cloud o sugli altri dispositivi che la sincronizzano. Le cartelle sincronizzate da altri programmi vanno controllate a mano: spostarne una può eliminarla anche sugli altri dispositivi.
- I file che si trovavano solo nel cloud non arrivano nel Cestino del PC. Si recuperano dal cestino del servizio, sul web.
- Prima di eseguire un comando che Quado copia, leggere i rischi e la riga «Reversibilità» che l'ispettore mostra con il comando.
- Su un PC gestito da un'azienda o da una scuola, consultare prima il reparto IT. Le sue regole possono vietare i programmi senza firma, l'avvio come amministratore o l'eliminazione di alcuni file.

## Accesso ai dati e modifiche ai file

### Dati letti da Quado

La scansione legge nomi, dimensioni, date e attributi, e non richiede che Quado sia avviato come amministratore. Windows non consente all'account corrente di leggere alcune cartelle, come i profili degli altri account: il loro contenuto rientra nello spazio «Non attribuito» e la Panoramica elenca queste cartelle. «Ripeti come amministratore» legge solo quelle cartelle e lo spazio occupato da punti di ripristino e copie shadow, dopo la conferma richiesta da Windows. Non legge il contenuto dei file e, per i profili e i Cestini degli altri account, riporta i totali senza i nomi.

Per riconoscere cartelle e programmi, Quado legge inoltre:

- dal registro, i programmi installati e le impostazioni del Cestino, di Sensore memoria, dei punti di ripristino e dell'alimentazione;
- se Windows tiene traccia degli avvii delle app, i programmi avviati da Start o da Esplora file, con il numero di avvii e la data dell'ultimo (UserAssist, nel registro);
- i file in cui Steam, Epic Games Launcher e CapCut elencano giochi e versioni installati, le date dell'ultima partita ai giochi di Steam e la cartella dei progetti di CapCut;
- il file `.npmrc` nel profilo utente, di cui Quado usa solo le cartelle indicate per la cache di npm e per lo store di pnpm;
- i pacchetti delle app installati per l'account corrente, come le app di Microsoft Store.

Prima di uno spostamento nel Cestino, l'anteprima chiede a Windows se i file sono in uso e da quali programmi. Controlla anche se l'elemento si trova in una cartella sincronizzata:

- chiede a Windows se la cartella è registrata come sincronizzata, come quelle di OneDrive;
- legge il file `info.json` di Dropbox, di cui usa solo i percorsi delle cartelle, e il valore `DefaultMountPoint` di Google Drive per desktop nel registro;
- cerca i marcatori di Syncthing (`.stfolder`) e di Resilio Sync (`.sync`) nell'elemento e nelle cartelle che lo contengono, leggendone solo gli attributi.

Per le versioni vecchie di CapCut e le cartelle dei progetti, l'anteprima cerca anche i riferimenti a questi elementi. Legge i collegamenti di Start e del desktop, le attività pianificate e le voci di registro dei programmi installati, dei percorsi delle app e dell'avvio automatico.

Quado legge il contenuto degli altri file solo su richiesta, nella vista Download:

- «Confronta il contenuto» legge per intero i file con le stesse dimensioni, per trovare le copie identiche.
- «Leggi i dati di versione» legge i dati di versione degli installatori senza eseguirli.
- «Leggi l'indice degli archivi» legge l'elenco dei file di un archivio zip senza estrarlo.

Come avviene con qualsiasi programma che apre un file, queste letture possono aggiornare la data di ultimo accesso del file. I file presenti solo nel cloud vengono saltati, quindi non vengono scaricati.

### Modifiche ai file

Quado modifica i file in un solo modo: li sposta nel Cestino, dopo l'anteprima e la conferma. Non elimina file: «Svuota il Cestino…» apre la finestra di dialogo di Windows, e Windows elimina gli elementi solo se l'eliminazione viene confermata lì. Quado elimina soltanto i propri dati, su richiesta, in Impostazioni › Dati e privacy.

Quado lascia inoltre un elemento al suo posto quando lo spostamento potrebbe portare Windows a eliminare file definitivamente:

- l'unità non ha un Cestino;
- il Cestino è impostato per eliminare subito i file;
- il Cestino è troppo piccolo per la selezione, oppure Quado non riesce a leggerne la capacità;
- il percorso dell'elemento, o di un file al suo interno, è lungo 260 caratteri o più.

Quado non sposta la cartella di Windows, System Volume Information né i file di paging, di ibernazione e di swap. Lascia al loro posto anche le cartelle dei programmi installati, con una sola eccezione: le versioni vecchie di CapCut.

Ogni altro intervento passa da Windows o dal programma che ha installato i file. I pulsanti per disinstallare aprono Impostazioni o Steam. Quando il metodo ufficiale è un comando, come `pnpm store prune`, Quado lo mostra e lo copia negli appunti senza eseguirlo.

## Privacy

Quado non usa la rete. Non ha telemetria, account o controllo degli aggiornamenti e non invia dati: ciò che legge e salva resta sul PC. Per aggiornarlo occorre scaricare la nuova versione o ricompilarlo.

Quado salva solo i file seguenti, in `%APPDATA%\Quado`:

- `impostazioni.txt`, le impostazioni;
- `interventi.jsonl`, il registro degli interventi confermati, creato al primo intervento;
- la cartella `lingue`, con gli eventuali file di lingua aggiunti e, se viene creato, il modello da tradurre.

Impostazioni › Dati e privacy elenca questi file con le loro dimensioni e può cancellare il registro degli interventi. Le opzioni da riga di comando che misurano Quado, e che scrivono altrove, esistono solo negli eseguibili compilati con la feature `misure`. La feature è disattivata per impostazione predefinita, quindi gli eseguibili della pagina Releases non le contengono.

Quando Quado apre la finestra, Direct3D e il driver della scheda video possono scrivere le proprie cache, per esempio quelle degli shader in `%LOCALAPPDATA%`. Queste scritture non dipendono dal codice di Quado.

### Verificare l'assenza di codice di rete

Due controlli verificano che Quado non contenga codice di rete:

- `deny.toml` vieta i crate dei client HTTP e WebSocket, come reqwest, ureq e hyper. Il controllo si esegue con `cargo deny check` (richiede cargo-deny).
- `cargo xtask importazioni` legge la tabella delle importazioni dell'eseguibile. Fallisce se trova una libreria di rete (winhttp, wininet, urlmon, dnsapi, websocket), le funzioni di ws2_32 che aprono connessioni o risolvono nomi, oppure funzioni che avviano un processo, come `CreateProcess`.

Il secondo controllo funziona anche su un `quado.exe` scaricato:

```
cargo xtask importazioni percorso\di\quado.exe
```

La tabella delle importazioni non mostra le funzioni che un programma cerca per nome durante l'esecuzione, cosa che la libreria standard di Rust fa per alcune funzioni facoltative di Windows. Per coprire questo caso, una sessione registrata con la rete collegata ha contato zero connessioni di Quado in 216 campioni (rischio R19 in `docs/08-analisi-dei-rischi.md`).

## Lingue

Quado è disponibile in italiano e in inglese. Al primo avvio usa la prima lingua di visualizzazione di Windows che conosce, altrimenti l'inglese; la lingua si cambia in Impostazioni › Lingua.

I testi dell'interfaccia sono file Fluent in `crates/quado-ui/lingue`, quelli delle regole in `crates/quado-kb/regole`. L'italiano è la lingua di riferimento: in `it.ftl` ogni chiave ha un commento che indica dove compare il testo. Le regole per scrivere i testi e il glossario dei termini fissi sono in `docs/testi.md`.

### Tradurre senza ricompilare

Un testo si può correggere, e una lingua si può aggiungere, senza ricompilare:

1. In Impostazioni › Lingua, selezionare «Crea un modello da tradurre». Quado copia il file italiano, con i commenti, in `%APPDATA%\Quado\lingue` come `modello-da-tradurre.txt`.
2. Rinominare il file con il codice della lingua, per esempio `fr.ftl`, e tradurre i valori.
3. Selezionare «Ricarica i file».

Le chiavi assenti dal file vengono prese dall'inglese, poi dall'italiano.

### Aggiungere una lingua all'eseguibile

`cargo xtask lingue nuova fr` crea `crates/quado-ui/lingue/fr.ftl`, e `cargo xtask lingue` elenca le chiavi mancanti in ogni lingua.

## Compilare dal sorgente

Per compilare Quado servono Rust, installato con rustup, e gli strumenti di compilazione C++ di Microsoft. `rust-toolchain.toml` fissa Rust 1.99.0, che rustup installa la prima volta che si esegue `cargo` nel repository.

```
git clone https://github.com/Valmairos/quado.git
cd quado
cargo build --profile dist
```

L'eseguibile è `target\dist\quado.exe`. Il profilo `dist` usa l'ottimizzazione completa in fase di link, e sul PC di sviluppo una compilazione da zero dura circa dieci minuti. Durante lo sviluppo conviene il profilo release: dopo una modifica in `quado-ui` ha ricompilato in 52 secondi, contro 7 minuti e 21 secondi di `dist`.

```
cargo run --release -p quado
```

### Eseguire i test

```
cargo test --workspace
```

I test segnati come ignorati non vengono eseguiti per impostazione predefinita. La maggior parte richiede i VHDX di prova; gli altri sono controlli manuali sul PC su cui vengono eseguiti, oppure producono misure e immagini.

### Installare dal sorgente

```
cargo xtask installa
```

Il comando compila con il profilo `dist` e controlla le importazioni dell'eseguibile (vedere [Privacy](#privacy)). Se il controllo riesce, copia `quado.exe` in `%LOCALAPPDATA%\Programs\Quado` e aggiunge Quado al menu Start. Tutti i file restano nel profilo utente, e non compare alcuna richiesta di conferma dell'amministratore.

### Disinstallare

Per rimuovere Quado, eliminare `%LOCALAPPDATA%\Programs\Quado`, il collegamento a Quado nel menu Start e `%APPDATA%\Quado`.

## Stato del progetto

Quado è alla versione 0.1.0. È stato compilato e usato su un solo PC, un portatile con un processore a 6 core e un SSD NVMe (vedere [Requisiti](#requisiti)). Su quel PC la scansione completa di un'unità con circa 3,1 milioni di file dura circa 12 secondi, con Quado non avviato come amministratore e con i dati delle cartelle già nella cache di Windows. Con l'intera unità caricata, Quado occupa circa mezzo gigabyte di memoria. La prima scansione dopo un riavvio non è stata misurata. Numeri e metodo sono in `docs/06-piano-di-test.md` §9.

### Limiti noti

- Le 26 regole coprono un insieme limitato di programmi: CapCut, Steam, Epic, le cache di Chrome, Edge e Brave, npm e pnpm, le immagini dell'emulatore Android, i progetti di sviluppo e parti di Windows come WinSxS, Windows.old e il file di ibernazione. Su altri PC molte cartelle risulteranno «Non classificato», con lo stato «Non sappiamo».
- Quado cerca collegamenti, voci di registro e attività pianificate che puntano a un elemento solo prima di spostare le versioni vecchie di CapCut o le cartelle dei progetti. Non controlla i servizi di Windows, le associazioni dei file né il PATH.
- Quado non ha un programma di installazione, una firma del codice né aggiornamenti automatici.
- Un test di usabilità e una sessione completa con la rete scollegata non sono stati svolti.

## Uso dell'AI

Il codice, i documenti e i testi dell'interfaccia di Quado sono stati scritti con Claude Code, l'agente di programmazione sviluppato da Anthropic. Il manutentore, Valmairos, ha stabilito cosa Quado deve fare e cosa non deve fare mai, ha esaminato i risultati e ha richiesto le correzioni. Ha inoltre eseguito le prove che richiedono una persona davanti al PC, come la lettura dell'unità C: come amministratore, che attende la conferma della richiesta di Windows. Ogni decisione è documentata in `docs/` con la sua motivazione, insieme alle misure citate in questo README.

Quado non contiene AI. Le spiegazioni che mostra provengono dai file delle regole in `crates/quado-kb/regole`, descritti in [Funzionalità](#funzionalità).

## Contribuire

Per segnalare un errore o proporre una regola, usare i moduli delle issue su GitHub. Per tradurre Quado, vedere [Lingue](#lingue). Prima di aprire una pull request, leggere [CONTRIBUTING.it.md](CONTRIBUTING.it.md); i problemi di sicurezza vanno segnalati come indicato in [SECURITY.md](SECURITY.md).

## Licenza

Quado è software libero, distribuito secondo i termini della GNU General Public License, versione 3 o (a scelta) qualsiasi versione successiva. È distribuito senza garanzia, nei limiti consentiti dalla legge: vedere le sezioni 15 e 16 di [LICENSE](LICENSE). Impostazioni › Informazioni mostra lo stesso avviso, il testo della licenza e l'indirizzo del codice sorgente. I font in `assets/fonts`, Inter e IBM Plex, sono distribuiti con la SIL Open Font License 1.1.

La licenza non copre il nome Quado. In caso di distribuzione di una versione modificata, darle un nome diverso.

Copyright (C) 2026 Valmairos.

## Marchi

Windows, OneDrive, Steam, Epic Games, CapCut, Chrome, Brave, Android, npm e gli altri nomi di prodotti che compaiono in Quado e nelle sue regole sono marchi dei rispettivi titolari. Quado non è collegato ai titolari né approvato da essi, e usa questi nomi solo per indicarne i prodotti.
