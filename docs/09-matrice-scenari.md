# 09 · Matrice degli scenari

Stato: 12/10/2026. Per ogni scenario: cosa serve all'utente, cosa deve sapere Quado, come lo ricava, cosa non può sapere, cosa mostra e cosa permette. Le decisioni di riferimento sono in `revisione/decisioni-revisione.md` (stati al §2, casi specifici al §3, Cestino al §4, stima e verifica al §5, nuove funzioni al §7, prove al §9 bis, requisiti al §9 ter). Le prove sono in `revisione/prove-windows.md` e in 02 §15.

## Come leggerla

- Un blocco per scenario, sempre con gli stessi campi. "Dati del PC di sviluppo" riporta le misure del 12/10/2026, prese in sola lettura: esatte quelle di Windows e i tempi, come ordine di grandezza quelle che descrivono i file e i programmi dell'utente.
- "Raccomandazione mostrata" usa uno dei cinque stati: Gestibile, Da verificare, Decidi tu, Lo gestisce X (con la variante "Nessuna procedura: non toccare"), Non sappiamo. I testi d'esempio seguono 04 §6.8; i valori tra graffe sono segnaposto. Sono testi proposti: il testo in uso sta nei cataloghi, con le regole e il glossario di `testi.md`.
- "Metodo" dice quando serve l'amministratore; dove non è scritto basta l'utente standard. Quado non esegue mai comandi che richiedono l'amministratore: li mostra e li copia (05 D7).
- "Versione": v1 se lo scenario è coperto dai requisiti di decisioni §9 ter; v1.1 o v2 se rinviato; "escluso" se non previsto. Le parti con una versione diversa sono indicate nel campo.
- "Requisiti": gli ID canonici di decisioni §9 ter.
- Le regole comuni qui sotto valgono per ogni scenario con un'azione e non si ripetono nei blocchi.
- In fondo: riepilogo, scelte di questa matrice non scritte nelle decisioni (proposte da confermare), punti dell'anteprima allineati.

## Regole comuni

Protezioni comuni, per ogni spostamento nel Cestino:
1. simulazione obbligatoria; il piano confermato nasce solo dalla conferma (F-AC-2);
2. conferma con la scheda della raccomandazione; Esc, Annulla e la X non eseguono nulla (F-AC-3);
3. elementi protetti respinti (F-AC-4);
4. mai eliminazione definitiva: in `PreDeleteItem` l'elemento si annulla se Windows non lo riciclerebbe (F-AC-5);
5. capacità del Cestino confrontata con il contenuto attuale più la selezione; Sensore memoria dichiarato con i suoi giorni (F-AC-7);
6. Restart Manager sui file da spostare: un file in uso blocca l'elemento con il nome del processo (F-AC-8);
7. subito prima dello spostamento si ricontrollano ID, dimensione e data; un elemento cambiato si salta e compare nel riepilogo;
8. registro dell'intervento con stimato e ottenuto (F-AC-6).

Per le procedure esterne (Impostazioni, Steam, comandi ufficiali) Quado apre la pagina o copia il comando, senza eseguire nulla. Il lanciatore accetta solo valori tipizzati, mai stringhe lette dal registro o da file di configurazione (05 D7).

Verifica comune:
1. prima dell'azione: spazio libero, dimensione del Cestino, stima con metodo e stato (esatta, intervallo, non stimabile) (F-VE-1);
2. nuove misure dopo lo spostamento (variazione attesa circa 0, perché il Cestino trattiene i file), dopo lo svuotamento del Cestino e dopo "Ho eseguito la procedura" (F-VE-2);
3. due numeri distinti, stimato e variazione dello spazio libero; oltre il 5% di differenza, spiegazione voce per voce oppure "non spiegata" (F-VE-3);
4. riscansione dei sottoalberi toccati, aggiornamento del modello e storico nella vista Interventi (F-VE-4, F-VE-5).

Regole di stima comuni (decisioni §5): si contano le allocate dei file i cui nomi stanno tutti nella selezione, deduplicate per ID; un file con altri nomi fuori dalla selezione è escluso e citato; nomi non conoscibili danno "intervallo"; residenti nella MFT 0; segnaposto cloud solo byte locali; contenuto dei dischi virtuali 0 fino alla compattazione; con i punti di ripristino attivi la nota "parte può essere riassorbita".

## 1. Installatori in Download

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere quali installatori corrispondono a programmi già installati, e a quale versione, per decidere quali tenere. |
| Informazioni necessarie | Estensione (`.exe`, `.msi`); nome del prodotto e versione dichiarati dal file; per gli MSI anche `ProductCode`; elenco dei programmi installati (voci di disinstallazione, MSIX); dimensione e date. |
| Metodo | Famiglia per estensione dalla scansione, senza leggere il contenuto. Su richiesta, in background e con cache per (percorso, dimensione, data di modifica, ID file): `GetFileVersionInfoW` per gli exe; `MsiOpenDatabaseW` in sola lettura con una query sulla tabella Property per gli MSI, mai `MsiOpenPackage`. Corrispondenza: `ProductCode` uguale al nome della chiave di disinstallazione vale "da fonte"; per gli exe nome e versione valgono "dedotto", con affidabilità media o bassa (03 §6.2). |
| Limiti e incertezze | Non tutti gli exe hanno la risorsa di versione e il confronto per nome sbaglia. La lettura è lenta per l'analisi di Defender all'apertura e aggiorna la data di ultimo accesso. Un installatore che scarica il programma durante l'installazione può dichiarare una versione diversa da quella che installa (dedotto). Quado non sa se l'installatore serve per reinstallare senza rete o per tenere una versione precisa. |
| Comportamento previsto | La lente mostra la famiglia con numero e dimensione. "Leggi i dati di versione" dichiara quanti file legge e che la lettura aggiorna la data di ultimo accesso. Dopo la lettura i file si dividono in: stessa versione installata, versione diversa installata, programma non installato, dati di versione assenti. |
| Raccomandazione mostrata | Decidi tu. Esempi: "Stesso codice di prodotto di {programma} {versione}, installato (da fonte)."; "Nome e versione coincidono con {programma} {versione}, installato (dedotto, affidabilità media)."; "Nessun programma corrispondente tra quelli installati." |
| Protezioni | Comuni. L'installatore non si esegue mai e non si apre mai in scrittura; nessuna lettura di contenuto prima della richiesta. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1. Firma Authenticode, Zone.Identifier e marcatori NSIS, Inno, Squirrel: escluso dalla v1 (decisioni §9 bis). |
| Requisiti | F-DL-1, F-DL-2, F-DL-3, F-IN-3, F-AC-2, F-AC-8, F-VE-3 |
| Dati del PC di sviluppo | Alcune decine di installatori per alcuni GiB, sia al primo livello sia contando un livello di sottocartelle; quasi tutti exe, gli altri MSI; circa metà con più di un anno. `ProductName` su circa nove exe su dieci; `ProductCode` corrispondente per la maggior parte degli MSI. Lettura di tutti gli exe: da 18 a 25 s. |

## 2. Copie identiche in Download

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Togliere i file scaricati più volte, tenendone una copia. |
| Informazioni necessarie | Dimensione (solo i file con la stessa dimensione sono candidati); impronta SHA-256 del contenuto; ID file (due nomi dello stesso file non sono due copie); data di creazione; stato cloud. |
| Metodo | Gruppi per dimensione dalla scansione. Su richiesta, SHA-256 dei soli file con la stessa dimensione, in background e annullabile; mai sui segnaposto cloud. |
| Limiti e incertezze | L'hash legge i file per intero e aggiorna la data di ultimo accesso. Il suffisso "(N)" non prova nulla: molte copie con suffisso hanno una dimensione diversa, e molti gruppi identici hanno nomi diversi. Un file cambiato dopo il confronto non è più una copia verificata. |
| Comportamento previsto | Prima del confronto: gruppi "File con le stesse dimensioni, contenuto non confrontato", caselle disattivate, pulsante "Confronta il contenuto" con i byte da leggere. Dopo: gruppi identici; si tiene il più vecchio per data di creazione e l'utente può sceglierne un altro; le altre copie diventano selezionabili. Un confronto annullato non produce gruppi presentati come completi. |
| Raccomandazione mostrata | Prima del confronto, Decidi tu: "Tre file con lo stesso nome e la stessa dimensione. Il contenuto non è ancora stato confrontato." Dopo il confronto, Gestibile: "Contenuto identico (impronta SHA-256). Si tiene {file più vecchio}; le altre {n} copie vanno nel Cestino dopo l'anteprima." |
| Protezioni | Comuni. In ogni gruppo una copia resta fuori dal piano. Se prima dello spostamento dimensione o data sono cambiate, il gruppo torna "non confrontato". |
| Verifica del risultato | Comune; stima esatta (allocate delle copie, deduplicate per ID). |
| Versione | v1 nella lente, su Download o su un'altra cartella dell'utente. Ricerca di copie su tutto il volume: v2. |
| Requisiti | F-DL-3, F-DL-4, F-SC-7, F-SC-9, F-OV-4, F-AC-2, F-VE-3 |
| Dati del PC di sviluppo | Su tutta la cartella Download: oltre cento gruppi identici, alcuni GiB recuperabili, circa un gruppo su cinque con nomi diversi; hash in circa 74 s. Copie con "(N)": centinaia, circa tre su quattro con dimensione diversa dall'originale. Esempio del mockup: tre copie dello stesso archivio zip da poche centinaia di MiB, due delle quali in più. |

## 3. Archivi con la cartella estratta accanto

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere se un archivio è una seconda copia di una cartella già estratta. |
| Informazioni necessarie | Archivio zip con una cartella dello stesso nome accanto; nomi e dimensioni non compresse dalla directory centrale; nomi e dimensioni logiche dei file della cartella, già nel modello. |
| Metodo | Su richiesta, lettura del record di fine directory e della directory centrale (anche zip64), senza decompressione; confronto con l'albero scansionato. |
| Limiti e incertezze | Nome e dimensione uguali non provano che il contenuto sia uguale: un file modificato dopo l'estrazione senza cambiare dimensione non si distingue. I file aggiunti alla cartella dopo l'estrazione non contano. Solo zip: gli altri formati restano nella famiglia "Archivi" senza confronto. Zip corrotto: "non leggibile". La lettura aggiorna la data di ultimo accesso dell'archivio. |
| Comportamento previsto | "{n} voci su {m} presenti", con l'elenco delle voci mancanti o diverse e la prova "misurato". |
| Raccomandazione mostrata | Decidi tu: "Archivio con una cartella omonima accanto: {n} voci su {m} presenti, con la stessa dimensione. Se ti basta la cartella, l'archivio è una seconda copia." |
| Protezioni | Comuni. L'azione riguarda solo l'elemento scelto dall'utente, archivio o cartella. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1 per gli zip. Altri formati: solo famiglia, nessun confronto in v1. |
| Requisiti | F-DL-1, F-DL-5, F-AC-2, F-VE-3 |
| Dati del PC di sviluppo | Esempio del mockup: un archivio zip con la cartella omonima, oltre 1 GiB. Famiglia "Archivi" al primo livello: più di cento file, decine di GiB. |

## 4. Immagini disco

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire se le immagini ISO o IMG scaricate servono ancora. |
| Informazioni necessarie | Estensione (`.iso`, `.img`); nome del file; se l'immagine è montata; dimensione e date. |
| Metodo | Famiglia per estensione. Un'immagine montata da Esplora file è aperta dal sistema: Restart Manager o lo spostamento la trovano in uso (dedotto, da provare in M5). |
| Limiti e incertezze | Il sistema contenuto si ricava solo dal nome del file (dedotto, affidabilità bassa); il contenuto non si legge. Quado non sa se l'immagine si può ancora scaricare né se è l'unica copia di una versione. |
| Comportamento previsto | Elenco nella lente con nome, dimensione e date. |
| Raccomandazione mostrata | Decidi tu: "Immagine disco: {nome}, secondo il nome del file. Quado non ne legge il contenuto." |
| Protezioni | Comuni. Un'immagine montata è bloccata; se il controllo non la vede, lo spostamento fallisce e il riepilogo lo riporta. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1. I dischi di macchine virtuali (`.vhdx`, `.vdi`, `.vmdk`) sono nello scenario 13. |
| Requisiti | F-DL-1, F-IN-3, F-AC-2, F-AC-8, F-VE-3 |
| Dati del PC di sviluppo | Poche immagini, circa 10 GiB in tutto, al primo livello di Download: immagini di installazione di sistemi operativi, secondo il nome del file. |

## 5. Download incompleti

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Togliere i resti dei download interrotti. |
| Informazioni necessarie | Estensione `.crdownload` o `.part`; processo che usa il file; data dell'ultima modifica. |
| Metodo | Famiglia per estensione; Restart Manager (`RmGetList`) sul file. |
| Limiti e incertezze | Un download in pausa può non risultare in uso: spostarlo impedisce di riprenderlo (dedotto). Altre estensioni temporanee dei browser non sono nella regola. |
| Comportamento previsto | Famiglia "Download incompleti" nella lente quando ce ne sono; ogni riga mostra la data dell'ultima modifica e l'esito di Restart Manager. |
| Raccomandazione mostrata | Gestibile: "Nessun programma lo usa, secondo Windows (controllo del {data} alle {ora})". Se un programma lo usa: "In uso da un programma, secondo Windows (controllo del {data} alle {ora})" e "Lo usa {processo}". |
| Protezioni | Comuni; qui Restart Manager è il controllo da cui dipende lo stato. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1. |
| Requisiti | F-DL-1, F-DL-6, F-AC-2, F-AC-8, F-VE-3 |
| Dati del PC di sviluppo | Nessuna misura; la famiglia non compare nel mockup (04 §6.6). |

## 6. Programmi grandi, forse non più usati

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Trovare i programmi che occupano di più e sapere cosa si conosce del loro uso, per decidere se disinstallarli. |
| Informazioni necessarie | Elenco da voci di disinstallazione (HKLM, WOW6432Node, HKCU) e MSIX; percorso con una prova (`InstallLocation`, `InstalledLocation` dei pacchetti; `UninstallString` e `DisplayIcon` solo come indizio di percorso); dimensione misurata e dichiarata (`EstimatedSize`); `InstallDate`; uso noto da UserAssist solo se `Start_TrackProgs` = 1; regole che indicano tuoi file nella cartella. |
| Metodo | Lettura del registro e `PackageManager.FindPackagesForUser("")`; misura dalla scansione. Disinstallazione dall'interfaccia ufficiale: `ms-settings:appsfeatures` con il nome copiato negli appunti; `ms-settings:appsfeatures-app?<PFN>` per i pacchetti MSIX; `appwiz.cpl` come alternativa. |
| Limiti e incertezze | Molte voci non dichiarano la posizione: senza prova del percorso lo spazio non si attribuisce ("posizione non dichiarata"). `EstimatedSize` non è calcolata da Windows e può essere vecchia. UserAssist non registra gli avvii da riga di comando, da altri launcher e da attività pianificate. Epic e MSIX non hanno un dato d'uso. Non esiste un URI per la pagina di una singola app Win32. Il programma di disinstallazione può lasciare dati, o togliere anche dati dell'utente. |
| Comportamento previsto | Vista Programmi ordinata per dimensione misurata, con il dichiarato accanto, la data di installazione e l'uso noto con la sua copertura. |
| Raccomandazione mostrata | Decidi tu: "Installato il {data} (da fonte). Tracciamento degli avvii di Windows disattivato: nessun dato d'uso." Mai "inutilizzato", "mai usato" o "non usato da N giorni". |
| Protezioni | La cartella di un programma registrato è protetta: nessuno spostamento nel Cestino (F-AC-4). Mai `UninstallString` né `QuietUninstallString`. Prima di aprire Impostazioni, avviso se una regola indica tuoi file nella cartella: "Nella cartella di {programma} ci sono tuoi file ({x} GB in {sottocartella}). Copiali altrove prima di disinstallare." |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione della cartella del programma e delle sue cartelle dati note. Stima: non stimabile, perché decide il programma di disinstallazione; come riferimento si mostra la dimensione misurata della cartella. |
| Versione | v1. BAM, Prefetch, Amcache, SRUM e data di ultimo accesso come fonti d'uso: escluso. Esecuzione di `UninstallString`: escluso. |
| Requisiti | F-AP-1, F-AP-2, F-AP-3, F-AP-4, F-AP-5, F-AC-4, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Alcune centinaia di voci con nome, circa un quarto con `InstallLocation` e poco più di un terzo con un percorso utilizzabile; un centinaio di pacchetti MSIX; `Start_TrackProgs` = 0, UserAssist fermo a una data di oltre un anno prima. Un'installazione di Unreal Engine da decine di GiB. |

## 7. Librerie di giochi (Steam, Epic)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere quali giochi occupano più spazio e quando sono stati usati, per disinstallare quelli che non servono. |
| Informazioni necessarie | `libraryfolders.vdf`; `appmanifest_*.acf` (`SizeOnDisk`, `LastPlayed`, cartella); `localconfig.vdf` (`LastPlayed`); manifest Epic `Manifests\*.item` (`InstallLocation`); misura delle cartelle. |
| Metodo | Lettura dei file dei launcher. Disinstallazione: `steam://uninstall/<appid>` con un numero tipizzato; per Epic, rinvio alla libreria del launcher. |
| Limiti e incertezze | Formati non documentati dai produttori: un formato inatteso dà "estrattore non riuscito". `SizeOnDisk` è una dimensione logica e può essere vecchia. `LastPlayed` probabilmente non registra gli avvii fuori da Steam (dedotto). Epic non registra l'ultimo utilizzo. Più prodotti Epic possono dichiarare la stessa cartella: ogni byte ha un solo proprietario (prefisso più lungo). |
| Comportamento previsto | Giochi nella vista Programmi e nella top list, con dimensione misurata, `SizeOnDisk` etichettato come dichiarato e ultima sessione. |
| Raccomandazione mostrata | Lo gestisce Steam: "Ultima sessione registrata da Steam: {data}. Lo spazio si libera disinstallando il gioco da Steam." Lo gestisce Epic Games Launcher: "Epic non registra quando un gioco è stato usato l'ultima volta. Lo spazio si libera dalla libreria del launcher." |
| Protezioni | Cartelle dei giochi protette come cartelle di programmi (F-AC-4). Il lanciatore accetta solo `steam://uninstall/` seguito da un numero. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione della libreria. Stima: dimensione allocata della cartella del gioco, stato intervallo, perché Steam può togliere o lasciare anche cache e contenuti del workshop (dedotto). |
| Versione | v1 per Steam ed Epic. Altri launcher (Xbox, GOG, EA, Ubisoft): non in v1, perché il PC di sviluppo non li usa e manca una fixture reale (C14); le loro cartelle sono Non sappiamo. |
| Requisiti | F-AP-1, F-AP-2, F-AP-3, F-AP-4, F-AP-5, F-AC-4, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Giochi di Steam: un paio di centinaia di GiB, per lo più in pochi giochi da decine di GiB ciascuno; `LastPlayed` presente per quasi tutti i giochi installati. La somma di `SizeOnDisk`, con unità non dichiarata, resta minore della misura in entrambe le basi: `SizeOnDisk` è una dimensione logica, può essere vecchia e non comprende il resto della libreria (cache degli shader, workshop, download in corso; dedotto). Si mostra accanto alla misura, mai al suo posto, e si riconcilia gioco per gioco in M4. |

## 8. Cartelle dati di app molto grandi (CapCut)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire perché la cartella di un'app in AppData è enorme e cosa si può togliere senza perdere i propri progetti. |
| Informazioni necessarie | Voce di disinstallazione (versione, `UninstallString`); file di configurazione dell'app (`Apps\ProductInfo.xml`); struttura delle sottocartelle (`Apps\<versione>`, `User Data\Cache`, `User Data\Projects`); misure; processi che usano i file; riferimenti alle versioni in registro, collegamenti e attività pianificate. |
| Metodo | Regola `capcut.versions` con l'estrattore di `ProductInfo.xml`. Controlli: Restart Manager su un campione e su tutti gli eseguibili; riferimenti in `UninstallString`, `DisplayIcon`, `InstallLocation`, App Paths e chiavi Run; `.lnk` di Start e Desktop letti con `IShellLinkW::GetPath` senza `Resolve`; attività pianificate visibili all'utente (`ITaskService`). |
| Limiti e incertezze | Formati di CapCut non documentati. Che le versioni precedenti non servano più è dedotto. Non esiste una procedura ufficiale: la disinstallazione rischierebbe `User Data\Projects`. Le attività pianificate non visibili all'utente rendono il controllo "parziale". Il contenuto della cache non è documentato. |
| Comportamento previsto | Ispettore con "Perché è così grande": versioni, cache, progetti. La simulazione mostra la versione in uso e la precedente, visibili ma disattivate ("esclusa", "tenuta"). |
| Raccomandazione mostrata | Versioni precedenti, Da verificare: "{n} cartelle di versioni precedenti (dedotto)." Conferma: "Spostare nel Cestino {n} versioni precedenti di CapCut? Liberato subito: 0 GB. Dopo lo svuotamento, stima esatta: {x} GB.", con l'avviso "Prova CapCut prima di svuotare il Cestino." `Projects` e `Videos`, Decidi tu con gestore "tu": "I tuoi progetti video. Non è cache: Quado non propone di eliminarli." Cache, Lo gestisce CapCut: "CapCut non documenta una pulizia della cache: Quado la spiega e non la elimina." |
| Protezioni | Comuni. Eccezione scritta a F-AC-4 solo tramite questa regola con stato Da verificare. Azione disabilitata finché Restart Manager e i riferimenti non passano. Le ultime due versioni restano fuori dal piano e dalla stima. |
| Verifica del risultato | Comune; stima esatta delle cartelle candidate. Dopo lo spostamento la variazione attesa è circa 0; dopo lo svuotamento circa la stima. |
| Versione | v1 (regola in M3, azione in M5). |
| Requisiti | F-IN-1, F-IN-2, F-IN-4, F-OV-4, F-AC-2, F-AC-4, F-AC-7, F-AC-8, F-AC-9, F-VE-3 |
| Dati del PC di sviluppo | `AppData\Local\CapCut` decine di GiB, per la maggior parte in `Apps`, che contiene decine di cartelle di versione; le versioni precedenti alle ultime due valgono circa due terzi della cartella; `Cache` e `Projects` alcuni GiB ciascuna. |

## 9. Cache di app e browser

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere quanto occupano le cache e come si liberano senza rompere le app. |
| Informazioni necessarie | Regola per app: percorso (cartella nota più percorso relativo) e fonte che documenta la cache e la sua procedura (per i browser, la cancellazione dei dati di navigazione dalle impostazioni); misura; app in esecuzione. |
| Metodo | Misura dalla scansione; rinvio alla procedura dell'app, descritta con la sua fonte; nessun comando eseguito. |
| Limiti e incertezze | Molte app non documentano la cache. Una cache in uso si riscrive subito e l'app la ricrea con l'uso. Il solo nome "Cache" è un'ipotesi con affidabilità bassa. Il browser decide da solo quanta cache tenere (dedotto). |
| Comportamento previsto | La cache compare come parte della cartella dell'app, con la procedura se documentata. |
| Raccomandazione mostrata | Con una regola, Lo gestisce {app}: "Questa cache la gestisce {app}: {procedura}. Quado non la elimina." Se l'app non documenta una procedura: "CapCut non documenta una pulizia della cache: Quado la spiega e non la elimina." Senza regola, Non sappiamo: "Ipotesi dal nome: cache. Nessuna regola la descrive." |
| Protezioni | Nessuno spostamento nel Cestino proposto: con Lo gestisce X mai rimozione manuale. Colore del gestore solo da un fatto "da fonte" o "dedotto alta". |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione della cartella. Stima: dimensione allocata della cache, stato intervallo, perché l'app può tenerne una parte. |
| Versione | v1 per le app con una regola; le altre restano Non sappiamo. |
| Requisiti | F-IN-1, F-IN-2, F-IN-4, F-OV-4, F-AC-1, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Cache di CapCut: alcuni GiB. La cartella di un browser occupa alcuni GiB, ma la sola cache non è misurata. |

## 10. Cache dei gestori di pacchetti (npm, pnpm, cargo)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Liberare spazio dalle cache degli strumenti di sviluppo senza rompere i progetti. |
| Informazioni necessarie | Percorso della cache dalla configurazione: per npm `.npmrc` o la variabile d'ambiente, altrimenti `%LocalAppData%\npm-cache`; per pnpm lo store in `%LOCALAPPDATA%\pnpm\store`; per cargo la cartella `registry` sotto `CARGO_HOME`. Misura e, per pnpm, i file con più nomi. |
| Metodo | Lettura della configurazione, mai esecuzione di npm o pnpm. Comandi ufficiali mostrati e copiati: `npm cache verify` (preferito), `npm cache clean --force` come alternativa con le sue conseguenze; `pnpm store prune`. |
| Limiti e incertezze | `npm cache verify` toglie solo i dati non più necessari: quanto libera non si sa prima. Dopo `npm cache clean --force` npm riscarica i pacchetti e le installazioni senza rete non trovano più la copia locale. Lo store di pnpm è collegato con hard link ai `node_modules`: lo spazio recuperabile non si stima senza contare i link. Per cargo una regola si scrive solo se la documentazione di cargo indica una procedura per la cache del registro; fino ad allora la cartella è spiegata senza proposta. |
| Comportamento previsto | Card interventi e ispettore con il comando, la sua fonte e le conseguenze; pulsante "Ho eseguito la procedura". |
| Raccomandazione mostrata | Lo gestisce npm: "Questa cache la gestisce npm, con il comando npm cache verify. Quado lo mostra e lo copia, ma non lo esegue. Quando hai finito, scegli «Ho eseguito la procedura» per misurare il risultato." Lo gestisce pnpm: "Comando pnpm store prune. Stima: non stimabile senza contare i link verso node_modules." |
| Protezioni | Nessuno spostamento nel Cestino delle cache. I comandi vengono da un catalogo tipizzato, mai da stringhe lette nella configurazione. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione della cache. Stima per npm con `verify`: non stimabile, perché quanto libera non si sa prima; la dimensione della cache si mostra solo come riferimento. Con `clean --force`: la dimensione della cache. Per pnpm: non stimabile; contano la variazione dello spazio libero e la riscansione. |
| Versione | v1, con i comandi mostrati e copiati. Esecuzione dei comandi dall'app: non in v1. Yarn, pip, uv, conda, NuGet, Gradle, Maven: solo con una regola con fonte (02 §9). |
| Requisiti | F-IN-4, F-OV-4, F-AC-1, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Cache di npm e store di pnpm: alcuni GiB ciascuno. |

## 11. Output di build e dipendenze di progetto (node_modules, target, .next, build)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Recuperare spazio dai progetti di sviluppo non attivi. |
| Informazioni necessarie | Marcatori del progetto: `package.json` con un lockfile (`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`) per `node_modules`; `Cargo.toml` e `Cargo.lock` per `target`; configurazione di Next.js per `.next`; per `build`, il marcatore dello strumento che la genera. File con più nomi (pnpm); processi che usano i file. |
| Metodo | Riconoscimento della struttura per regola, controllando solo l'esistenza dei marcatori, senza leggerne il contenuto. Per `target` si mostra e si copia `cargo clean`; per le altre cartelle, spostamento nel Cestino su scelta esplicita. |
| Limiti e incertezze | Le cartelle si ricostruiscono solo se il progetto si può ancora ricostruire: pacchetti ritirati, versioni non più disponibili, dipendenze private, assenza di rete (decisioni §3). Il nome `build` è generico e può contenere file dell'utente: senza marcatore non si riconosce. In un `node_modules` di pnpm i file sono hard link allo store: spostarlo libera poco. Con molti file piccoli lo spostamento nel Cestino è lento, e i file residenti nella MFT non liberano spazio. |
| Comportamento previsto | L'ispettore mostra progetto, lockfile, data dell'ultima modifica del progetto e dimensione. |
| Raccomandazione mostrata | Decidi tu: "node_modules di {progetto}: si ricrea con l'installazione dei pacchetti, se il progetto si può ancora ricostruire. Lockfile presente." Per `target`: "Cartella di compilazione di Rust: si toglie con cargo clean nella cartella del progetto." |
| Protezioni | Comuni. Controllo automatico: lockfile presente, altrimenti nessuna azione. Restart Manager ferma lo spostamento se un server di sviluppo usa i file. Nessuna azione su cartelle senza marcatori. |
| Verifica del risultato | Comune; stima esatta per i file i cui nomi sono tutti nella selezione; i file con altri nomi fuori (store di pnpm) sono esclusi e citati. Per `cargo clean`, misure dopo "Ho eseguito la procedura". |
| Versione | v1. |
| Requisiti | F-SC-7, F-IN-4, F-AC-1, F-AC-2, F-AC-8, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Documenti: decine di GiB, con progetti che contengono `node_modules` e `.next` secondo il mockup (attribuzione illustrativa); nessuna misura per singola cartella. |

## 12. Residui di software disinstallato

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Trovare le cartelle lasciate dai programmi disinstallati. |
| Informazioni necessarie | Cartelle in Local, Roaming, LocalLow e ProgramData; programmi installati (registro, MSIX, Steam, Epic); cartelle rivendicate dalle regole; data dell'ultima modifica; dimensione. |
| Metodo | Confronto per nome con i programmi installati e con le regole. |
| Limiti e incertezze | Il confronto per nome dà falsi positivi: le cartelle modificate di recente sono quasi sempre in uso. Senza il punto di partenza di una disinstallazione appena fatta la ricerca è debole (prove §D). I nomi dei produttori sono comuni a più programmi. |
| Comportamento previsto | Elenco delle cartelle non rivendicate nella vista Programmi, con dimensione e data dell'ultima modifica. |
| Raccomandazione mostrata | Non sappiamo: "Nessun programma installato corrispondente (confronto per nome). Ultima modifica: {data}. Potrebbe essere un residuo." |
| Protezioni | Nessuna azione oltre "Apri in Esplora file"; un test di proprietà lo verifica (06 §4). |
| Verifica del risultato | Nessun intervento di Quado. Ciò che l'utente cambia a mano compare con "Rileggi dal disco" o con una nuova scansione. |
| Versione | v1. |
| Requisiti | F-AP-6, F-IN-1, F-IN-5, F-IN-6 |
| Dati del PC di sviluppo | Circa un terzo delle cartelle senza corrispondenza per nome; di queste circa un quarto vuote, circa un quinto modificate negli ultimi 90 giorni, quasi un terzo ferme da più di un anno. |

## 13. Macchine virtuali, WSL, Docker, emulatori Android

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire quanto occupano dischi virtuali ed emulatori e come recuperare spazio su C:. |
| Informazioni necessarie | WSL: `BasePath` in `HKCU\...\Lxss\{GUID}` e file `ext4.vhdx`. Docker Desktop: `%LOCALAPPDATA%\Docker\wsl\disk\docker_data.vhdx`. Hyper-V `.vhdx`, VirtualBox `.vdi`, VMware `.vmdk`. Android: `%LOCALAPPDATA%\Android\Sdk` e `.android\avd`. Dimensione allocata dei file. |
| Metodo | Lettura di registro e file. Procedure mostrate e copiate: `docker system df`, `docker system prune`, `docker builder prune` o "Clean up data" di Docker Desktop; per WSL la compattazione documentata, con le sue condizioni; `VBoxManage modifymedium --compact`; per Android `sdkmanager --uninstall` e Device Manager di Android Studio. La compattazione dei dischi virtuali richiede l'amministratore e i servizi fermi; `Optimize-VHD` non esiste su Windows Home. |
| Limiti e incertezze | Lo spazio liberato dentro un disco virtuale torna su C: solo dopo la compattazione. `wsl --manage <distro> --set-sparse true` è segnalato come rischioso sulle versioni recenti di WSL: da verificare sulla versione installata. Il contenuto del disco virtuale non si scansiona. `docker system prune --volumes` elimina dati. |
| Comportamento previsto | Ispettore con il tipo di disco virtuale, il gestore e la procedura. |
| Raccomandazione mostrata | Lo gestisce Docker, Lo gestisce WSL: "Stima su C: 0 GB fino alla compattazione del disco virtuale." Lo gestisce Android Studio: "Dispositivo virtuale di Android: si elimina da Device Manager di Android Studio." Dischi di VirtualBox, VMware, Hyper-V: Lo gestisce {programma}, con la procedura del programma. |
| Protezioni | `--volumes` non compare mai. Un disco virtuale collegato è in uso e Restart Manager lo blocca. Un file più grande della capacità del Cestino è bloccato (F-AC-7). I comandi da amministratore sono solo mostrati. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione. Stima 0 fino alla compattazione; dopo la compattazione conta la variazione della dimensione allocata del file. |
| Versione | v1 per riconoscimento e procedure con una regola con fonte. Compattazione eseguita da Quado: escluso. |
| Requisiti | F-IN-2, F-IN-4, F-AC-1, F-AC-7, F-AC-8, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | AVD: alcuni GiB per tutti i dispositivi virtuali, lo stesso ordine di grandezza del mockup. Il PC di sviluppo non usa WSL né Docker: senza una fixture reale la loro regola non entra (07 §7). Nessuna misura di altre macchine virtuali nei documenti. |

## 14. File multimediali grandi

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Trovare video, audio e foto che occupano molto, per decidere cosa tenere. |
| Informazioni necessarie | Estensione; dimensione; date; posizione (cartelle note da `SHGetKnownFolderPath`; sottocartelle di app indicate come "tuoi file" da una regola); stato cloud. |
| Metodo | Vista File grandi e lente su qualunque cartella dell'utente, con filtri per tipo e dimensione; nessuna lettura del contenuto. |
| Limiti e incertezze | Quado non sa se esiste un'altra copia (backup, cloud, altro disco). I file multimediali dentro le cartelle dei programmi possono essere risorse dell'app: il proprietario viene solo da una regola. |
| Comportamento previsto | Elenco per dimensione con tipo, cartella e date. |
| Raccomandazione mostrata | Decidi tu: "Tuo file video, {dimensione}. Quado non sa se ne hai un'altra copia." |
| Protezioni | Comuni. Un file più grande della capacità del Cestino è bloccato, perché Windows lo eliminerebbe definitivamente (F-AC-5, F-AC-7). Segnaposto cloud: solo byte locali, nessuna idratazione. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1. Spostamento su un altro disco: non previsto in v1 (F-AC-1 non ha questo tipo). |
| Requisiti | F-EX-4, F-EX-5, F-DL-1, F-SC-9, F-AC-2, F-AC-5, F-AC-7, F-VE-3 |
| Dati del PC di sviluppo | Famiglia "Video e audio" al primo livello di Download: centinaia di file, decine di GiB. `Projects` di CapCut: alcuni GiB. Capacità del Cestino: circa il 5% dell'unità, decine di GiB. |

## 15. File grandi dimenticati

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Scoprire file grandi in posti inattesi: esportazioni vecchie, dump, log cresciuti, file temporanei di app. |
| Informazioni necessarie | I file più grandi del volume; date di modifica; regola che li descrive, se esiste; attributi (in uso, cloud). |
| Metodo | Query sul modello in memoria con filtri per dimensione, tipo e data; "Rileggi dal disco" per i file che crescono. |
| Limiti e incertezze | Età e dimensione sono filtri, mai una prova (05 D6). La data di ultimo accesso è rumore e non si usa. I file nelle cartelle negate non compaiono senza l'helper. Un file aperto in scrittura può avere una dimensione vecchia nella voce di cartella. |
| Comportamento previsto | Vista File grandi con gestore, glifo della fonte e regola, se c'è. |
| Raccomandazione mostrata | Dipende dalla regola. Nelle cartelle dell'utente, senza regola: Decidi tu. Fuori dalle cartelle dell'utente, senza regola: Non sappiamo: "Nessuna regola descrive questo file. Ultima modifica: {data}." File di sistema: lo stato del loro scenario (17-22, 26). |
| Protezioni | Comuni; elementi protetti respinti (`pagefile.sys`, `hiberfil.sys`, `swapfile.sys` e gli altri di F-AC-4). |
| Verifica del risultato | Comune; "Rileggi dal disco" prima di decidere su un file che cresce. |
| Versione | v1. |
| Requisiti | F-EX-4, F-EX-5, F-IN-1, F-IN-6, F-AC-4, F-VE-3 |
| Dati del PC di sviluppo | Nessuna misura dedicata: la vista File grandi non è disegnata nel mockup. |

## 16. Backup ridondanti

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Trovare backup vecchi o ripetuti che occupano spazio. |
| Informazioni necessarie | Cartelle di backup documentate dai produttori (regola con fonte); nome, date, dimensione; contenuto identico solo con l'hash della lente. |
| Metodo | Riconoscimento per regola quando una fonte documenta la cartella; altrimenti il nome ("backup", ".bak") è solo un filtro. Copie identiche con l'hash su richiesta, nella lente, su una cartella scelta (F-DL-4). |
| Limiti e incertezze | Quado non sa se un backup è l'unica copia, né se si può ancora ripristinare. Due backup con lo stesso nome possono avere contenuto diverso. I backup di Windows (Cronologia file, immagini di sistema) di solito stanno su un'altra unità. |
| Comportamento previsto | Filtro per nome e data nella vista File grandi e nella lente; gruppi identici dopo l'hash. |
| Raccomandazione mostrata | Con una regola, Decidi tu: "Backup di {programma} (da fonte). Quado non sa se è l'unica copia." Copie identiche dopo l'hash: Gestibile, come nello scenario 2. Senza regola, Non sappiamo: "Ipotesi dal nome: backup." |
| Protezioni | Comuni. Nessuna proposta nata da nome o età. |
| Verifica del risultato | Comune; stima esatta. |
| Versione | v1 con regole e lente. Ricerca di copie su tutto il volume: v2. |
| Requisiti | F-DL-1, F-DL-4, F-EX-4, F-IN-4, F-AC-2, F-VE-3 |
| Dati del PC di sviluppo | Nessuna misura di backup nei documenti. |

## 17. WinSxS

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire perché WinSxS sembra enorme e se si può ridurre. |
| Informazioni necessarie | Allocata unica, con gli hard link condivisi con System32 contati una volta; analisi ufficiale `DISM /AnalyzeComponentStore`. |
| Metodo | Scansione con deduplica per ID; i dati condivisi si attribuiscono al percorso con priorità più alta (System32 prima di WinSxS) e si dichiarano condivisi. Comando mostrato e copiato: `Dism /Online /Cleanup-Image /StartComponentCleanup`, da amministratore; `/ResetBase` solo come alternativa. L'analisi di DISM richiede anch'essa l'amministratore. |
| Limiti e incertezze | Esplora file sovrastima la cartella. Quanto si recupera lo dice solo l'analisi di DISM. Con `/ResetBase` gli aggiornamenti installati non si possono più disinstallare. |
| Comportamento previsto | Vista Windows e sistema con la dimensione unica e la nota sugli hard link. |
| Raccomandazione mostrata | Lo gestisce Windows: "Comando ufficiale, da amministratore: Dism /Online /Cleanup-Image /StartComponentCleanup. Quado lo mostra e lo copia." Alternativa: "/ResetBase: dopo, gli aggiornamenti installati non si potranno più disinstallare." |
| Protezioni | WinSxS è protetta (F-AC-4); mai a mano, perché il PC può non avviarsi (02 §8). |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione di WinSxS. Stima: non stimabile senza l'analisi di DISM. |
| Versione | v1. |
| Requisiti | F-SC-7, F-SY-1, F-AC-1, F-AC-4, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | WinSxS 9,69 GB 10^9 = 9,02 GiB unici; `C:\Windows` 35,77 GB 10^9 contando i nomi e 28,04 deduplicati. |

## 18. Windows Update e file temporanei di Windows

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Liberare lo spazio degli aggiornamenti scaricati e dei file temporanei di sistema. |
| Informazioni necessarie | Dimensione di `SoftwareDistribution\Download`, `%TEMP%`, cache delle miniature, segnalazioni errori (WER), `Windows.old`; configurazione di Sensore memoria. |
| Metodo | Misura dalla scansione dove la cartella è leggibile. Rinvio a Impostazioni › Sistema › Archiviazione › File temporanei (`ms-settings:storagesense`; l'URI diretto della pagina dei file temporanei si verifica in M5). Pulizia disco resta un'alternativa, con la fonte ricontrollata (C5). |
| Limiti e incertezze | `Windows\Temp` e Ottimizzazione recapito sono negate all'utente standard: senza helper la loro dimensione non è nota. I gestori di Pulizia disco non sono pensati per un'app esterna: Quado non li usa. I file in uso restano. `Windows.old` si elimina da sola dopo 10 giorni. |
| Comportamento previsto | Vista Windows e sistema con le categorie misurate e quelle non leggibili. |
| Raccomandazione mostrata | Lo gestisce Windows: "Si libera da Impostazioni › Archiviazione › File temporanei. Quado apre la pagina e non cancella questi file a mano." |
| Protezioni | Mai a mano: nessuno spostamento nel Cestino per queste cartelle, compresa `%TEMP%` del profilo (decisioni §3). |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione delle cartelle. Stima: somma delle allocate misurate, stato intervallo, perché Windows sceglie cosa togliere e i file in uso restano. |
| Versione | v1. |
| Requisiti | F-SY-1, F-SY-3, F-AC-1, F-AC-4, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Nessuna misura di queste cartelle nei documenti. Sensore memoria attivo; 29 gestori di Pulizia disco registrati. |

## 19. Archivio dei driver (DriverStore)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire se le versioni precedenti dei driver si possono togliere. |
| Informazioni necessarie | Dimensione di `DriverStore\FileRepository`; elenco dei pacchetti di driver e quali sono in uso. |
| Metodo | Misura dalla scansione (cartella leggibile). Comandi mostrati e copiati: `pnputil /enum-drivers` per l'elenco, `pnputil /delete-driver oemNN.inf` per la rimozione, che richiede l'amministratore. |
| Limiti e incertezze | Senza l'elenco di `pnputil` Quado non sa quali pacchetti sono in uso. Togliere un pacchetto in uso può far smettere di funzionare un dispositivo; senza il pacchetto precedente non si torna indietro. |
| Comportamento previsto | Vista Windows e sistema con dimensione e procedura. |
| Raccomandazione mostrata | Lo gestisce Windows: "I pacchetti dei driver si elencano con pnputil /enum-drivers e si rimuovono uno alla volta con pnputil /delete-driver, da amministratore. Quado non tocca i file." |
| Protezioni | Cartella dentro Windows, protetta (F-AC-4); mai a mano. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione. Stima: non stimabile, perché dipende dai pacchetti scelti. |
| Versione | v1. |
| Requisiti | F-SY-1, F-AC-1, F-AC-4, F-VE-2 |
| Dati del PC di sviluppo | 6,33 GB 10^9 = 5,90 GiB. |

## 20. Ibernazione

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Recuperare lo spazio di `hiberfil.sys` sapendo cosa si perde. |
| Informazioni necessarie | Dimensione di `hiberfil.sys`; ibernazione, avvio rapido e sospensione ibrida attivi o no; azione per batteria critica; presenza di una batteria. |
| Metodo | Dimensione dall'enumerazione della radice (le API sul singolo file danno violazione di condivisione). Configurazione con le API di alimentazione in sola lettura, invece di eseguire `powercfg /a` (F-SY-3). Comandi mostrati e copiati: `powercfg /hibernate /type reduced` e `powercfg /h off`, entrambi da amministratore. |
| Limiti e incertezze | Con `/type reduced` la nuova dimensione la decide Windows. `/h off` toglie anche l'avvio rapido e la sospensione ibrida. Su un portatile con l'ibernazione come azione per batteria critica, toglierla espone a perdere il lavoro quando la batteria si scarica (dedotto). |
| Comportamento previsto | Card interventi e vista Windows e sistema con le due opzioni affiancate e i loro effetti. |
| Raccomandazione mostrata | Decidi tu: "Due opzioni, entrambe richiedono l'amministratore. Ridurre il file di ibernazione mantiene l'avvio rapido e toglie l'ibernazione. Disattivarla toglie anche l'avvio rapido e la sospensione ibrida." |
| Protezioni | `hiberfil.sys` è protetto e non si sposta mai (F-AC-4). Comandi non eseguiti. Avviso se l'azione per batteria critica è l'ibernazione. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e nuova lettura della radice. Stima con `/h off`: esatta, la dimensione del file. Con `/type reduced`: non stimabile prima. |
| Versione | v1. |
| Requisiti | F-SY-1, F-SY-3, F-OV-4, F-AC-1, F-AC-4, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | `hiberfil.sys` di una decina di GiB. |

## 21. File di paging e swap

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire perché `pagefile.sys` e `swapfile.sys` occupano spazio. |
| Informazioni necessarie | Dimensioni dei due file. |
| Metodo | Enumerazione della radice. Procedura di Windows: Memoria virtuale nelle impostazioni avanzate di sistema, citata con la sua fonte. |
| Limiti e incertezze | Windows dimensiona il file di paging in automatico (02 §8); ridurlo a mano può causare errori di memoria nelle app (dedotto). |
| Comportamento previsto | Vista Windows e sistema con dimensione e spiegazione. |
| Raccomandazione mostrata | Lo gestisce Windows: "Lo gestisce Windows, che lo dimensiona in automatico (Memoria virtuale). Quado non propone modifiche." |
| Protezioni | Protetti (F-AC-4); nessuna azione. |
| Verifica del risultato | Nessun intervento proposto. Se l'utente cambia l'impostazione, "Ho eseguito la procedura" rilegge la radice. |
| Versione | v1. |
| Requisiti | F-SY-1, F-AC-4 |
| Dati del PC di sviluppo | `pagefile.sys` 6,50 GiB; `swapfile.sys` 16 MiB. |

## 22. Windows\Installer

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire cos'è la cartella e perché non va toccata. |
| Informazioni necessarie | Dimensione; indicazione ufficiale di non eliminarla (02 §8). |
| Metodo | Misura dalla scansione (cartella leggibile). |
| Limiti e incertezze | Non esiste una procedura ufficiale per ridurla. Eliminarne il contenuto può impedire di riparare, aggiornare o disinstallare programmi, e il ripristino può richiedere di reinstallare Windows. |
| Comportamento previsto | Vista Windows e sistema con dimensione e spiegazione; nessuna stima. |
| Raccomandazione mostrata | Lo gestisce Windows, variante "Nessuna procedura: non toccare": "Copie dei pacchetti di installazione che Windows usa per riparare, aggiornare e disinstallare i programmi. Nessuna procedura: non toccare." |
| Protezioni | Protetta (F-AC-4); nessuna azione. |
| Verifica del risultato | Nessun intervento. |
| Versione | v1. |
| Requisiti | F-SY-1, F-IN-4, F-AC-4 |
| Dati del PC di sviluppo | 4,07 GB 10^9 = 3,79 GiB. |

## 23. Cestino e cartelle orfane

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere quanto occupa il Cestino, liberarlo senza perdere ciò che serve, capire le cartelle che Windows non elenca. |
| Informazioni necessarie | Dimensione e numero di elementi secondo Windows (`SHQueryRecycleBinW`); occupazione su disco da `$Recycle.Bin\<SID>` dell'utente; coppie `$I` e `$R`; `MaxCapacity` e `NukeOnDelete`; Sensore memoria; registro degli elementi spostati da Quado; cestini di altri SID. |
| Metodo | Senza amministratore per l'utente corrente; helper per gli altri SID, come totale per SID con i SID orfani segnati. Procedure: "Apri il Cestino" (`shell:RecycleBinFolder`) e "Svuota il Cestino…" (`SHEmptyRecycleBinW` con il dialogo di Windows). |
| Limiti e incertezze | Non è provato che lo svuotamento rimuova un `$R` senza `$I`, né che Windows conti gli orfani nella capacità: prova su VHDX in M1. Sensore memoria può svuotare il Cestino in qualsiasi momento secondo la sua regola. |
| Comportamento previsto | Top list con il solo misurato: "{x} GB su disco, {y} MB secondo Windows". Ispettore con la spiegazione degli orfani. Nessuna cifra recuperabile nella card interventi. |
| Raccomandazione mostrata | Contenuto elencato da Windows, Lo gestisce Windows: "Apri il Cestino ed elimina solo i {n} elementi spostati da Quado il {data} alle {ora}." oppure "Il dialogo di Windows elimina anche {n} altri elementi per {x} GB, secondo il conteggio di Windows." Cartelle orfane: anomalia spiegata (F-SY-2), con Lo gestisce Windows nella variante "Nessuna procedura: non toccare": "Nella cartella del Cestino ci sono {x} GB che il Cestino di Windows non elenca: cartelle senza il loro file indice. Non è provato che svuotare il Cestino le elimini." |
| Protezioni | Quado non tocca mai `$Recycle.Bin` direttamente. Nessuno "svuota tutto" implicito: prima del dialogo di Windows si dichiarano numero e GB degli altri elementi. Testo sulla reversibilità: "Puoi ripristinare dal Cestino finché non viene svuotato, da te o da Sensore memoria." |
| Verifica del risultato | Dopo lo svuotamento: spazio libero e riscansione di `$Recycle.Bin`, che dice anche se gli orfani sono spariti. L'esito della prova di M1 è in `m1/cestino-verifiche.md`. |
| Versione | v1; cestini di altri SID con l'helper (M6). |
| Requisiti | F-SY-2, F-SY-3, F-SC-3, F-OV-2, F-OV-4, F-AC-1, F-AC-7, F-VE-2 |
| Dati del PC di sviluppo | Secondo Windows pochi MiB; alcune cartelle orfane per decine di GiB; `MaxCapacity` circa il 5% dell'unità, decine di GiB; Sensore memoria attivo con la regola sul Cestino; alcuni cestini di altri SID, in parte orfani. |

## 24. Copie shadow e punti di ripristino

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere se una parte dello spazio usato sta nelle copie shadow e se lo spazio liberato può tornare occupato. |
| Informazioni necessarie | Spazio usato, allocato e massimo delle copie shadow; stato di Protezione sistema, se leggibile. |
| Metodo | Senza amministratore non è misurabile. L'helper legge `Win32_ShadowStorage` via WMI dentro il processo (F-SC-3). Procedure mostrate: `vssadmin resize shadowstorage`, da amministratore, o la finestra di Protezione sistema. |
| Limiti e incertezze | Copy-on-write: dopo un'eliminazione le scritture successive sugli stessi cluster possono riassorbire lo spazio liberato. Quando la diff area non basta, Windows elimina la copia più vecchia. |
| Comportamento previsto | Card dei conti, nella lista "Possibile, misurabile solo come amministratore"; dopo "Ripeti come amministratore", il valore misurato. Nella verifica degli interventi, la nota sui punti di ripristino. |
| Raccomandazione mostrata | Lo gestisce Windows: "Lo spazio delle copie shadow si legge solo come amministratore." Nella verifica: "Con i punti di ripristino attivi, parte dello spazio liberato può tornare occupato nei giorni successivi." |
| Protezioni | System Volume Information è protetta (F-AC-4); nessuna azione; comandi solo mostrati. |
| Verifica del risultato | Nessun intervento proprio; la nota accompagna la verifica degli altri scenari. |
| Versione | v1 (lettura con l'helper, nota nella verifica). Ridimensionamento eseguito da Quado: escluso. |
| Requisiti | F-SC-3, F-OV-3, F-SY-3, F-VE-3 |
| Dati del PC di sviluppo | Non misurate: `vssadmin` e `Win32_ShadowStorage` sono negati all'utente standard. |

## 25. File nel cloud (segnaposto)

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Capire quanto spazio occupano davvero i file sincronizzati e liberarlo senza perderli. |
| Informazioni necessarie | Attributi (`OFFLINE`, `RECALL_ON_OPEN`, `RECALL_ON_DATA_ACCESS`, `PINNED`, `UNPINNED`); stato dal tag (`CfGetPlaceholderStateFromAttributeTag`); byte locali (`CfGetPlaceholderInfo` su un handle con soli attributi). |
| Metodo | Enumerazione e aperture con soli attributi; `PHCM_EXPOSE_PLACEHOLDERS` all'avvio; nessuna lettura di contenuto. Procedura del provider: "Libera spazio", che toglie il file dal disco e lo mantiene nel cloud. |
| Limiti e incertezze | Eliminare un file solo online lo elimina anche nel cloud. Elencare una cartella con `RECALL_ON_DATA_ACCESS` può scaricarne l'elenco: per default non si elenca. Non è documentato che il filtro rispetti `FILE_FLAG_OPEN_NO_RECALL`. `PINNED` e `UNPINNED` da soli non indicano un segnaposto. |
| Comportamento previsto | Due valori, "nel cloud" e "su questo disco"; contenuto mai letto. |
| Raccomandazione mostrata | Lo gestisce {provider}: "Nel cloud: {x} GB. Su questo disco: {y} GB. Per liberare spazio senza eliminare il file dal cloud usa «Libera spazio» di {provider}." |
| Protezioni | Nessun hash, nessuna lettura, nessun flusso alternativo sui segnaposto; stima solo dei byte locali. Se l'utente sceglie di spostare nel Cestino un file sincronizzato, la simulazione aggiunge un controllo manuale, da spuntare, che dice che il file sparisce anche dal cloud (F-AC-2). "Proprietà" si apre solo su richiesta. |
| Verifica del risultato | Dopo "Ho eseguito la procedura": spazio libero e riscansione con i byte locali. |
| Versione | v1. |
| Requisiti | F-SC-9, F-DL-4, F-IN-5, F-AC-2, F-VE-2, F-VE-3 |
| Dati del PC di sviluppo | Il PC di sviluppo non usa OneDrive e nessuna cartella nota è reindirizzata, quindi il test usa un provider di prova (06 §1.2). Qualche migliaio di file con `PINNED` o `UNPINNED` fuori dal cloud. |

## 26. Elementi protetti, sconosciuti, non leggibili

| Campo | Contenuto |
|---|---|
| Esigenza dell'utente | Sapere cosa non si può toccare, cosa Quado non riconosce e cosa non riesce a leggere. |
| Informazioni necessarie | Elenco degli elementi protetti di F-AC-4; cartelle negate con il motivo; copertura delle regole. |
| Metodo | Cartelle negate elencate durante la scansione. Su richiesta, l'helper con `SeBackupPrivilege` (amministratore, con la conferma di Windows) le enumera in sola lettura e restituisce i profili altrui in forma aggregata. |
| Limiti e incertezze | Senza helper il contenuto delle cartelle negate resta nel non attribuito. I profili altrui arrivano solo come totale e numero di file. La junction "Default User" è negata apposta. |
| Comportamento previsto | Card dei conti con la lista "Osservato" (cartelle negate e il loro numero, cestini di altri SID, profili altrui); glifo "non disponibile"; barra di stato con il numero di cartelle non leggibili. |
| Raccomandazione mostrata | Protetti: lo stato del loro scenario, mai uno spostamento. Senza regola, Non sappiamo: "Nessuna regola riconosce questo elemento. Puoi vederne le misure e usare «Apri in Esplora file»." Non leggibili, Non sappiamo con il glifo "non disponibile": "Non leggibile: il contenuto resta nel non attribuito. Windows nega l'accesso se Quado non è avviato come amministratore." |
| Protezioni | La costruzione del piano respinge ogni selezione con un elemento protetto (test di proprietà, 06 §5.2); unica eccezione la regola Da verificare di CapCut. "Apri in Esplora file" seleziona senza aprire; "Proprietà" solo su richiesta. |
| Verifica del risultato | Nessun intervento. Dopo l'helper la card dei conti mostra cosa è cambiato. |
| Versione | v1 (helper in M6). Lettura diretta della MFT: v2, solo se servirà. |
| Requisiti | F-SC-3, F-SC-6, F-OV-3, F-IN-1, F-IN-5, F-AC-4 |
| Dati del PC di sviluppo | 113 cartelle negate e 8 altri errori; cestini di altri SID, in parte orfani; non attribuito al momento della sonda: 25,4 GiB. |

## Riepilogo

| # | Scenario | Stato tipico | Versione |
|---|---|---|---|
| 1 | Installatori in Download | Decidi tu | v1 |
| 2 | Copie identiche in Download | Decidi tu prima del confronto, Gestibile dopo | v1; su tutto il volume v2 |
| 3 | Archivi con la cartella estratta | Decidi tu | v1 (zip) |
| 4 | Immagini disco | Decidi tu | v1 |
| 5 | Download incompleti | Gestibile | v1 |
| 6 | Programmi grandi, forse non più usati | Decidi tu | v1 |
| 7 | Librerie di giochi | Lo gestisce Steam, Lo gestisce Epic Games Launcher | v1; altri launcher non in v1 |
| 8 | Cartelle dati di app grandi (CapCut) | Da verificare (versioni precedenti), Decidi tu (progetti), Lo gestisce CapCut (cache) | v1 |
| 9 | Cache di app e browser | Lo gestisce {app}; Non sappiamo senza regola | v1 |
| 10 | Cache dei gestori di pacchetti | Lo gestisce npm, Lo gestisce pnpm | v1 |
| 11 | Output di build e dipendenze | Decidi tu | v1 |
| 12 | Residui di software disinstallato | Non sappiamo | v1 |
| 13 | Macchine virtuali, WSL, Docker, emulatori | Lo gestisce Docker, WSL, Android Studio o il programma della macchina virtuale | v1; compattazione da Quado esclusa |
| 14 | File multimediali grandi | Decidi tu | v1 |
| 15 | File grandi dimenticati | Decidi tu o Non sappiamo | v1 |
| 16 | Backup ridondanti | Decidi tu; Gestibile dopo l'hash; Non sappiamo senza regola | v1; su tutto il volume v2 |
| 17 | WinSxS | Lo gestisce Windows | v1 |
| 18 | Windows Update e file temporanei | Lo gestisce Windows | v1 |
| 19 | Archivio dei driver | Lo gestisce Windows | v1 |
| 20 | Ibernazione | Decidi tu | v1 |
| 21 | File di paging e swap | Lo gestisce Windows | v1 |
| 22 | Windows\Installer | Lo gestisce Windows, "Nessuna procedura: non toccare" | v1 |
| 23 | Cestino e cartelle orfane | Lo gestisce Windows; orfani come anomalia, "Nessuna procedura: non toccare" | v1 |
| 24 | Copie shadow e punti di ripristino | Lo gestisce Windows | v1; ridimensionamento escluso |
| 25 | File nel cloud | Lo gestisce {provider} | v1 |
| 26 | Protetti, sconosciuti, non leggibili | Non sappiamo (protetti: stato del loro scenario) | v1 |

## Scelte di questa matrice non scritte in decisioni-revisione.md

Sono coerenti con le decisioni, ma non vi compaiono in modo esplicito. Vanno confermate o corrette.

1. Stato della stima per le procedure esterne: npm con `verify` non stimabile, con la dimensione della cache come riferimento; programmi disinstallati non stimabile, con la dimensione della cartella come riferimento; giochi, cache delle app e file temporanei di Windows intervallo; WinSxS e driver non stimabile; ibernazione esatta con `/h off` e non stimabile con `/type reduced`.
2. Copie identiche: in ogni gruppo una copia resta sempre fuori dal piano; se dimensione o data cambiano prima dello spostamento, il gruppo torna "non confrontato".
3. Copie identiche prima del confronto: Decidi tu. Deriva da decisioni §2 (Da verificare solo con una prova di gestibilità) e da 05 D6 (nome e dimensione non producono Da verificare né voci della card interventi).
4. Cartelle orfane del Cestino: Lo gestisce Windows nella variante "Nessuna procedura: non toccare", finché la prova di M1 non indica una procedura. Contenuto normale del Cestino: Lo gestisce Windows.
5. Archivi: confronto solo per gli zip in v1, perché decisioni §7 parla solo della directory centrale degli zip.
6. Cargo: regola sulla cache del registro solo con una procedura documentata da cargo.
7. Download incompleti: la riga e la conferma mostrano la data dell'ultima modifica, perché un download in pausa può non risultare in uso.
8. File sincronizzati con il cloud: se l'utente li sceglie per il Cestino, un controllo manuale nella simulazione dice che spariscono anche dal cloud. Ora è un criterio di F-AC-2 in 03, con il test in 06 §5.2.
9. Dischi di VirtualBox, VMware, Hyper-V: Lo gestisce il programma della macchina virtuale. Launcher diversi da Steam ed Epic: Non sappiamo.
10. Ricerca di copie identiche su tutto il volume: v2, come in 03 §8.

## Punti dell'anteprima allineati a questa matrice

Al 13/10/2026 l'anteprima e 04 §6.6 sono allineati:
- copie in Download prima del confronto: Decidi tu, fuori dalla card "Interventi possibili" finché l'hash non le rende Gestibile (punto 3);
- archivio con la cartella omonima: Decidi tu, con la prova misurata (decisioni §7);
- cache di npm: dimensione della cache con la stima "non stimabile" accanto a `npm cache verify`;
- Cestino nella top list: le cartelle orfane.
