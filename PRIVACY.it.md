# Privacy

[English](PRIVACY.md)

Quado non usa la rete. Non invia dati al manutentore né ad altri, e non ha telemetria, controllo degli aggiornamenti o account. Non apre pagine web: tramite Windows apre solo Impostazioni, Esplora file e la finestra di Steam che disinstalla un gioco. Ciò che legge e salva resta sul PC.

## Dati letti da Quado

La scansione legge nomi, dimensioni, date e attributi, e non richiede che Quado sia avviato come amministratore. Windows non consente all'account corrente di leggere alcune cartelle, come i profili degli altri account: il loro contenuto rientra nello spazio «Non attribuito». «Ripeti come amministratore» legge solo quelle cartelle e lo spazio occupato da punti di ripristino e copie shadow, dopo la conferma richiesta da Windows. Non legge il contenuto dei file e, per i profili e i Cestini degli altri account, riporta i totali senza i nomi.

Per riconoscere cartelle e programmi, Quado legge anche:

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

## Modifiche ai file

Quado modifica i file in un solo modo: li sposta nel Cestino, dopo l'anteprima e la conferma. Non elimina file: «Svuota il Cestino…» apre la finestra di dialogo di Windows, e Windows elimina gli elementi solo se l'eliminazione viene confermata lì.

Quado lascia un elemento al suo posto quando lo spostamento potrebbe portare Windows a eliminare file definitivamente:

- l'unità non ha un Cestino;
- il Cestino è impostato per eliminare subito i file;
- il Cestino è troppo piccolo per la selezione, oppure Quado non riesce a leggerne la capacità;
- il percorso dell'elemento, o di un file al suo interno, è lungo 260 caratteri o più.

Quado non sposta la cartella di Windows, System Volume Information né i file di paging, di ibernazione e di swap. Lascia al loro posto anche le cartelle dei programmi installati, con una sola eccezione: le versioni vecchie di CapCut.

Ogni altro intervento passa da Windows o dal programma che ha installato i file. I pulsanti per disinstallare aprono Impostazioni o Steam. Quando il metodo ufficiale è un comando, come `pnpm store prune`, Quado lo mostra e lo copia negli appunti senza eseguirlo.

## File salvati da Quado

Quado salva solo i file seguenti, in `%APPDATA%\Quado`:

- `impostazioni.txt`, le impostazioni;
- `interventi.jsonl`, il registro degli interventi confermati, creato al primo intervento;
- la cartella `lingue`, con gli eventuali file di lingua aggiunti e, se viene creato, il modello da tradurre.

Impostazioni › Dati e privacy elenca questi file con le loro dimensioni e può cancellare il registro degli interventi. Le opzioni da riga di comando che misurano Quado, e che scrivono altrove, esistono solo negli eseguibili compilati con la feature `misure`. La feature è disattivata per impostazione predefinita, quindi gli eseguibili della pagina Releases non le contengono.

Quando Quado apre la finestra, Direct3D e il driver della scheda video possono scrivere le proprie cache, per esempio quelle degli shader in `%LOCALAPPDATA%`. Queste scritture non dipendono dal codice di Quado.

## Verificare l'assenza di codice di rete

Due controlli verificano che Quado non contenga codice di rete:

- `deny.toml` vieta i crate dei client HTTP e WebSocket, come reqwest, ureq e hyper. Il controllo si esegue con `cargo deny check` (richiede cargo-deny).
- `cargo xtask importazioni` legge la tabella delle importazioni dell'eseguibile. Fallisce se trova una libreria di rete (winhttp, wininet, urlmon, dnsapi, websocket) oppure le funzioni di ws2_32 che aprono connessioni o risolvono nomi. Fallisce anche sulle funzioni che avviano un processo, come `CreateProcess`.

Il secondo controllo funziona anche su un `quado.exe` scaricato:

```powershell
cargo xtask importazioni percorso\di\quado.exe
```

La tabella delle importazioni non mostra le funzioni che un programma cerca per nome durante l'esecuzione. La libreria standard di Rust lo fa per alcune funzioni facoltative di Windows. Per coprire questo caso, `cargo xtask sessione-prova` registra una sessione guidata e campiona le connessioni di Quado. Sul PC di sviluppo, con la rete collegata, ha contato zero connessioni in 216 campioni.

## Segnalazioni su GitHub

Le issue e le segnalazioni aperte su GitHub sono trattate da GitHub secondo la sua informativa sulla privacy. Omettere nomi e percorsi che possano identificare chi scrive.
