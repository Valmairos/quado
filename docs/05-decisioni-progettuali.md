# 05 · Decisioni progettuali (v2, dopo la revisione)

Stato: v2 del 12/10/2026, riscritta dopo la revisione critica (`10-revisione-critica.md`) secondo le decisioni di `revisione/decisioni-revisione.md`, che prevalgono in caso di dubbio. La prima versione dei documenti non è inclusa. Ogni decisione riporta alternative, criteri e compromesso accettato. Fonti: [V] documentazione ufficiale, [F] fonte non ufficiale, [M] misurato sul PC di sviluppo in sola lettura, [I] deduzione. Le misure citate riportano la data; i test usano fixture versionate.

## Cosa cambia rispetto alla v1

| Decisione | Esito | In sintesi |
|---|---|---|
| D1 Linguaggio | Confermata | Rust; la motivazione non è più il parser della MFT, che esce dalla v1 |
| D2 Interfaccia | Confermata con condizioni nuove | egui; M0 ricalibrato per uno strumento personale; piano B (WinUI 3 o Slint) deciso solo dopo M0 |
| D3 Processi e privilegi | Semplificata | L'helper elevato enumera solo le cartelle negate e raccoglie poche informazioni di sistema |
| D4 Scansione | Ridotta | Solo enumerazione per cartelle; MFT e `FSCTL_QUERY_FILE_LAYOUT` alla v2, se serviranno |
| D5 Memoria | Ricalcolata | ID file in un array laterale durante la scansione; obiettivo ≤ 400 MB da misurare in M1 |
| D6 Regole | Estesa | Record di raccomandazione (03 §6.1), estrattori nuovi, letture di contenuto solo su richiesta |
| D7 Interventi | Estesa | Controlli del Cestino, Restart Manager, riferimenti, piano confermato come tipo, verifica F-VE, disinstallazione solo da interfaccia ufficiale |
| D8 Dati locali | Ridotta | Una sola sede, `%APPDATA%\Quado`; niente modalità portable |
| D9 Distribuzione | Riscritta | Eseguibile compilato in locale; niente MSI, firma, winget, Store |
| D10 Codice | Ridotta | Sparisce `quado-mft` |
| D11 Linux | Ridotta | Solo nota di portabilità |
| D12 Strumenti | Ridotta | Niente fuzzing né CI privilegiata; test in locale |
| D13 Lingue | Nuova (13/10/2026) | Italiano e inglese nella v1, altre lingue con un file di testo; Fluent, un file per lingua, modificabile anche senza ricompilare |

## 0. Criteri, in ordine di peso

1. Correttezza e sicurezza: numeri giusti, nessuna scrittura imprevista, privilegi minimi, nessuna raccomandazione più forte delle prove.
2. Accesso efficiente alle API di Windows e memoria compatta per milioni di elementi.
3. Interfaccia su misura e reattiva: mappa, elenco virtualizzato, ispettore.
4. Privacy verificabile: nessuna componente che usi la rete per conto suo.
5. Costo proporzionato a uno strumento personale: un utente, un PC con Windows 10 oggi e probabilmente Windows 11 in futuro.
6. Manutenibilità da parte di una persona, con test automatici eseguiti in locale.

Linux non è più un criterio di scelta: resta una nota (D11).

## D1. Linguaggio del core: Rust

| Alternativa | Pro | Contro |
|---|---|---|
| Rust | Accesso diretto a Win32 e NT con `windows-sys` e `windows` (Microsoft, MIT/Apache) [V]; controllo del layout in memoria (D5); sicurezza della memoria nei lettori di formati non fidati che restano in v1 (VDF di Steam, manifest Epic, XML di CapCut, risorsa di versione PE, directory centrale degli zip); parallelismo con `rayon`; la sonda di misura esiste già in Rust | Curva di apprendimento; codice `unsafe` attorno alle API, da confinare |
| C# .NET 10 | Produttività, UI Automation nativa con WinUI o WPF, P/Invoke con CsWin32 | Per milioni di nodi servono array di struct e pool di caratteri, altrimenti GC e memoria sforano [I]; NativeAOT con limiti; .NET 8 e 9 escono dal supporto il 10/11/2026 [V] |
| C++ | Massimo controllo, ecosistema Win32 | Lettori di dati non fidati senza sicurezza della memoria; produttività bassa per una persona |
| Go | Semplicità | Accesso alle API NT scomodo, GC, nessuna UI desktop matura |

La sonda in Rust (metodo, tempi e memoria in 02 §2.1) mostra che il linguaggio non è il collo di bottiglia. Nella v1 il motivo principale era il parser della MFT. Senza parser (D4) Rust resta la scelta per due motivi: il controllo del layout in memoria, da cui dipende l'obiettivo di D5, e un solo linguaggio dal core alla UI (D2), che per una persona sola riduce il costo di manutenzione.

## D2. Interfaccia: egui/eframe, con la verifica M0 prima di procedere

### Alternative valutate

| Stack | Esito | Motivo principale |
|---|---|---|
| Rust + egui/eframe | Scelto, con la condizione di M0 | Aspetto interamente su misura (lo dimostrano le tre app di riferimento: un editor di foto, uno di grafica vettoriale e uno di PDF), eseguibile singolo, un solo linguaggio, test headless e snapshot con `egui_kittest`. SquirrelDisk è passato da Tauri a egui [V] |
| Core Rust + WinUI 3 (C#) | Piano B, solo se egui non passa M0 | Testo DirectWrite con ClearType e UI Automation nativi; però due toolchain, un'API C tra core e UI, nessun DataGrid ufficiale [V], README dell'SDK che dichiara raccolta di dati d'uso [V], da disattivare e verificare |
| Rust + Slint | Piano B alternativo | Un solo linguaggio, testo con shaping completo, release regolari; però nessun widget albero [V] e licenza royalty-free che richiede l'attribuzione `AboutSlint` [V] |
| Core Rust + Tauri 2 | Scartato | WebView2 con il controllo di reputazione SmartScreen attivo per default, che invia URL a Microsoft [V]: contrasta con la privacy verificabile; IPC lento su Windows [F] |
| C++ + Qt 6 | Scartato | C++ e obblighi LGPL sul linking statico [F] |
| iced, gpui, Xilem, Makepad, Floem | Scartati | Accessibilità assente o non pronta, oppure API instabili [V] |
| Electron, Flutter, Compose, Wails | Scartati | Dimensioni, accessibilità o maturità |

### Limiti noti di egui e risposta

| Limite | Fonte | Risposta |
|---|---|---|
| Una tabella egui appare a UI Automation come una lista piatta di testi, senza ruoli di tabella, riga e cella | issue #8700 [V] | Se l'utente usa un lettore di schermo: nodi AccessKit nostri per tabella e mappa, e il criterio torna in M0. Se conferma che non lo usa: resta l'albero AccessKit di base di egui, con il limite dichiarato |
| Lo scorrimento basato su pixel in `f32` diventa instabile oltre circa 2 milioni di righe | issue #1391 [V] | Scroller nostro che mappa la barra su un indice di riga; la tabella ad albero è una lista appiattita dei soli nodi espansi. È un criterio di M0 |
| Crash del renderer via Desktop remoto e su macchine senza GPU | issue #2573 e #8265 [V] | Catena di avvio wgpu DX12, poi l'adattatore software WARP, poi glow, sul modello dell'app PDF di riferimento. Il ripiego su WARP è un criterio di M0; Desktop remoto lo diventa solo se l'utente lo usa |
| Testo bidirezionale non gestito; ripiego sui font di sistema annunciato per la 0.37 | issue #1016 [V], CHANGELOG [V] | Font incorporati (IBM Plex) più ripiego sui font di sistema; nomi arabi o ebraici in ordine logico, limite dichiarato. Stato della 0.37 da ricontrollare in M0 |
| Resa del testo diversa da ClearType | [I] | Criterio di M0 con screenshot affiancati (RS13 in 08) |
| Nessun controllo nativo | - | Voluto. I dialoghi di sistema (Proprietà, conferme del Cestino) restano quelli di Windows |
| Versioni 0.x con cambiamenti incompatibili ogni 2-3 mesi | [V] | Versione fissata; aggiornamento solo quando serve, con gli snapshot visivi come rete (07 §5.2) |

### Condizione per confermare D2: M0

M0 (07 §1) è un prototipo che deve superare cinque criteri bloccanti, misurati sul PC di sviluppo con tempi di frame strumentati:

| # | Criterio | Misura |
|---|---|---|
| 1 | Tabella ad albero appiattita da 3,5 milioni di righe con scroller a indice | Frame p95 ≤ 16 ms e p99 ≤ 33 ms durante uno scorrimento continuo dall'inizio alla fine e durante salti con la barra |
| 2 | Mappa con 20.000 aree | Frame p95 ≤ 16 ms durante hover e selezione, a 1080p |
| 3 | Testo IBM Plex a 100% e 150% | Screenshot affiancati allo stesso testo reso da un controllo nativo con ClearType; il criterio passa se vengono approvati |
| 4 | Tastiera completa | Ogni azione del prototipo raggiungibile da tastiera, verificato con `egui_kittest` |
| 5 | Ripiego sul renderer WARP | Con WARP forzato il prototipo si avvia e resta utilizzabile; i tempi di frame si registrano ma non sono bloccanti |

Assistente vocale (o altro lettore di schermo) e Desktop remoto non sono criteri bloccanti, a condizione che l'utente confermi di non usarli (07 §8). Se ne usa uno, il criterio corrispondente torna bloccante.

Esito:
- se M0 passa, D2 è confermata e il piano B si abbandona;
- se un criterio fallisce senza una correzione ragionevole, si sceglie tra WinUI 3 e Slint ripetendo sul candidato solo i criteri falliti. Il core Rust non cambia, perché non dipende dalla UI (D10).

### Esito di M0 (13/10/2026)

D2 confermata: il testo è stato approvato il 13/10/2026 (criterio 3). Criteri 1, 2 e 4 superati, 5 nella parte bloccante. Il piano B è abbandonato. Misure sul PC di sviluppo, con la scheda video che guida lo schermo: tabella da 3,5 milioni di righe con frame p95 4,7 ms e p99 5,8 ms, mappa con 20.658 aree, p95 3,6 ms. Con WARP forzato il prototipo parte e risponde, ma impiega circa 230 ms per frame a 1600×1000: resta un ripiego d'emergenza. Il testo di egui ha le stesse forme e misure di quello con ClearType, ma è in scala di grigi, senza sottopixel colorati. Due requisiti nuovi per M2: scegliere la GPU che guida lo schermo della finestra (sulla GPU sbagliata la mappa passa da 3,6 a 16,4 ms) e caricare i font di ripiego di Windows solo quando un nome li richiede (52 MB).

### Barra del titolo

In v1 resta quella di Windows. Disegnarla in proprio, come fanno le app di riferimento con `winsafe`, fa perdere il menu Snap Layouts di Windows 11, a meno di gestire `WM_NCHITTEST` con `HTMAXBUTTON`. Non migliora la risposta a nessuna delle domande del brief.

## D3. Processi e privilegi

| Alternativa | Valutazione |
|---|---|
| Tutta l'app elevata (riavvio come amministratore, come WizTree e TreeSize) | Ogni azione dell'interfaccia girerebbe con privilegi pieni, Cestino compreso. Scartata |
| Servizio di sistema permanente (modello Everything) | Un componente privilegiato sempre attivo per un uso occasionale. Scartato |
| Helper elevato su richiesta | Scelto |

### Cosa fa l'helper (F-SC-3)

- È lo stesso eseguibile con un argomento dedicato, avviato con `ShellExecuteExW` e il verbo `runas` solo quando l'utente lo chiede dalla card dei conti.
- Abilita `SeBackupPrivilege`, che va abilitato esplicitamente nel token [V], ed enumera soltanto le cartelle che la scansione non elevata ha trovato negate. Usa lo stesso codice dello scanner, con `FILE_FLAG_BACKUP_SEMANTICS` sulle aperture delle cartelle. Non legge il contenuto di nessun file.
- Raccoglie inoltre:
  - lo spazio delle copie shadow, interrogando WMI (`Win32_ShadowStorage`) dentro il processo invece di eseguire `vssadmin`, il cui output è localizzato;
  - lo stato dell'archiviazione riservata, con il comando o l'API scelti in M1 (07 §3);
  - i cestini di altri SID in `$Recycle.Bin`, come totale per SID, segnando se il SID corrisponde ancora a un account (RS16);
  - i profili degli altri utenti in forma aggregata: totale e numero di file per profilo, nessun nome di file o di cartella.
- Termina a fine lavoro.

### Pipe

- La crea la UI, con un nome casuale, DACL limitata al SID dell'utente e `PIPE_REJECT_REMOTE_CLIENTS`. L'helper verifica con `GetNamedPipeServerProcessId` che dall'altra parte ci sia il processo che l'ha lanciato.
- L'helper legge un solo messaggio iniziale: il volume e l'elenco delle cartelle da enumerare. Lo valida (percorsi assoluti su quel volume, normalizzati, senza `..`, con lunghezza e numero massimi) e rifiuta tutto il resto. Dopo quel messaggio scrive soltanto. Rispetto alla v1, dove l'helper non leggeva niente, è l'unica differenza: senza l'elenco l'helper non saprebbe quali cartelle sono negate all'utente.

### DLL e posizione (RS11)

- Linker con `/DEPENDENTLOADFLAG:0x800` (`LOAD_LIBRARY_SEARCH_SYSTEM32`) per le DLL importate staticamente. Da verificare in M1 nel load config dell'eseguibile e con una DLL esca (06 §11).
- All'avvio, come prima chiamata, `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)` per i caricamenti dinamici.
- CRT statico, nessuna DLL che non sia di sistema.
- Eseguibile in `%LOCALAPPDATA%\Programs\Quado` (D9). In modalità helper rifiuta di avviarsi se si trova in `Downloads` o in una cartella temporanea.

Interventi, Cestino e procedure girano sempre nella UI non elevata, con i diritti dell'utente. I comandi che richiedono l'amministratore (`powercfg`, `DISM`) si mostrano e si copiano, non si eseguono (D7).

## D4. Strategia di scansione

| Situazione | Metodo | Perché |
|---|---|---|
| NTFS, exFAT e FAT32 senza privilegi (default) | `GetFileInformationByHandleEx` con `FileIdExtdDirectoryInfo` su un handle di cartella, buffer da 64 KiB, parallelismo a work stealing | Una chiamata restituisce dimensione logica, allocata, attributi, tag di reparse e ID file a 128 bit, senza aprire i file [V]. `FindFirstFileExW` non dà l'allocata; `std::fs::read_dir` dà solo la logica. Tempi in 02 §2.1 |
| Cartelle negate, su richiesta | Helper elevato (D3): stesso codice, `SeBackupPrivilege` e `FILE_FLAG_BACKUP_SEMANTICS` | Completa la scansione dove serve, senza un secondo motore |
| HDD | Stesso metodo con concorrenza ridotta, scelta con `StorageDeviceSeekPenaltyProperty` | Molti thread su un disco meccanico peggiorano gli accessi casuali (R22) |
| ReFS, condivisioni di rete, altri file system | Stesso metodo, "senza garanzie", dichiarato nella UI (F-SC-10) | Non sono casi d'uso previsti; la semantica dell'allocata cambia |

Correzioni applicate dalla scansione per cartelle (misure e conteggi in 02 §3 e §7):
- File compressi WOF: la voce di cartella riporta allocata 0 e nasconde il reparse point. Per i file con logica > 0, allocata 0 e senza attributi sparse, offline o cloud si apre il singolo file con `FILE_READ_ATTRIBUTES` (più `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`) e si legge `FileStandardInfo`.
- Indici delle cartelle (`$I30`): `FileStandardInfo` sullo stesso handle già aperto per l'enumerazione.
- Dimensione della `$MFT`: `FSCTL_GET_NTFS_VOLUME_DATA` sull'handle della radice, senza privilegi.
- Segnaposto cloud: all'avvio `RtlSetProcessPlaceholderCompatibilityMode(PHCM_EXPOSE_PLACEHOLDERS)`; le cartelle con `RECALL_ON_DATA_ACCESS` non si elencano per default, perché elencarle può scaricarne l'elenco.
- Reparse point con il bit "name surrogate" mai seguiti; volumi montati in cartella scansionati a parte.
- Flussi alternativi: invisibili all'enumerazione [V]. In v1 non si contano, lo si dichiara, e il loro spazio resta nel non attribuito.
- Dimensioni pigre (R1): per i file aperti in scrittura la voce di cartella può essere vecchia. "Verifica questa voce" (F-IN-6) apre file o cartella con soli attributi, legge `FileStandardInfo` e aggiorna il modello; aprire e chiudere un handle aggiorna anche la voce di cartella [F].

Obiettivi di tempo: a cache calda ≤ 22 s sulla C: del PC di sviluppo, mediana di 5 esecuzioni. A cache fredda l'obiettivo si fissa in M1 dopo la misura (C11).

Fuori dalla v1:
- Lettura diretta della MFT e `FSCTL_QUERY_FILE_LAYOUT`. A uno strumento personale serve leggere le cartelle negate, non la velocità, e l'helper lo fa con l'enumerazione. Passano alla v2 solo se servirà: per esempio se la scansione non elevata su un PC futuro superasse di molto l'obiettivo, o se servisse contare i flussi alternativi. Con loro escono il crate `quado-mft`, il fuzzing e gli oracoli `ntfs` e `mft`.
- Indice persistente aggiornato via USN journal (v2).
- Integrazione con l'SDK di Everything.

## D5. Modello dei dati in memoria

- Arena a struttura di array con indici `u32`: genitore, primo figlio, fratello, offset e lunghezza del nome, dimensione logica e allocata, conteggi aggregati, data di modifica, attributi, flag. Nessun ID file per nodo (C16).
- Nomi in un unico pool di `u16`, come li restituisce Windows, convertiti solo per la visualizzazione: i nomi con surrogati spaiati restano esatti.
- ID file:
  - durante la scansione, un array laterale di coppie (ID a 128 bit, indice del nodo);
  - a fine scansione si ordina, si trovano i file con più nomi e si deduplicano i totali; si tengono solo gli ID con più nomi e l'elenco dei loro nodi (sul PC di sviluppo 115.231 file con più nomi, 02 §2.1);
  - stima e ricontrollo non hanno bisogno degli altri ID: si rileggono gli attributi dei file selezionati (D7).

Stima ricalcolata con i nomi [I]:

| Parte | Byte per voce | Nota |
|---|---|---|
| Campi fissi | 50-60 | indici, dimensioni a 64 bit, data, attributi, contatori |
| Nome in UTF-16 | 40-60 | 20-30 caratteri di media, da misurare |
| Totale dell'albero | 90-120 | 300-400 MB per 3,3 milioni di voci |
| Array laterale degli ID, solo durante la scansione | circa 20 per file | circa 65 MB in più nel picco |

Obiettivo: picco ≤ 400 MB per 3,3 milioni di voci, da misurare in M1 con i nomi reali (IP4). La stima della v1, 70 byte per voce nome compreso, era troppo bassa. Se la misura supera l'obiettivo, le riduzioni si applicano in quest'ordine:
1. liberare l'array laterale prima di costruire gli aggregati, così i due picchi non si sommano;
2. nomi solo ASCII in un pool a 8 bit, con un flag per nodo; gli altri restano in `u16`, quindi tutti restano esatti;
3. data di modifica a 32 bit, in secondi da un'epoca recente.

Altre regole:
- I totali delle cartelle si aggregano con un passaggio dal basso verso l'alto a fine scansione; i totali di primo livello crescono dal vivo con contatori atomici, per l'avanzamento.
- Riscansione di un sottoalbero (F-IN-6, F-VE-4): i nodi vecchi si staccano dal genitore, i nuovi si aggiungono in coda e gli antenati si riaggregano. Quando i nodi staccati superano una soglia, un passaggio di compattazione ne recupera lo spazio.
- Nessun database: l'albero vive in memoria e si butta alla chiusura. Lo storico (v1.1) salverà solo un riassunto.

## D6. Classificazione e raccomandazioni: regole verificabili, nessuna AI a runtime

### Uscita del motore: il record di raccomandazione

- Il motore produce per ogni elemento con un ruolo un record con i campi obbligatori di 03 §6.1 e uno dei cinque stati: Gestibile, Da verificare, Decidi tu, Lo gestisce X (con la variante "Nessuna procedura: non toccare"), Non sappiamo. Le stesse parole valgono in card, ispettore, conferma e risultato.
- In Rust il record è una struttura con tutti i campi obbligatori. Gli stati Gestibile e Da verificare si costruiscono solo passando un valore `ProvaDiGestibilita`, distinto da `ProvaDiIdentita`. I test di proprietà sono in 06 §2.2.
- La provenienza vale per singola affermazione: ogni fatto ha la sua classe (misurato, da fonte, dedotto con affidabilità, non disponibile) e la sua fonte. I criteri di affidabilità sono in 03 §6.2.
- Il colore del gestore nella mappa e nel bilancio nasce solo da fatti "da fonte" o "dedotto alta"; negli altri casi grigio con l'etichetta "ipotesi".
- Regole di comportamento:
  - il tipo non implica la gestione;
  - dimensione, età, nome ed estensione servono a filtri e viste, mai a uno stato Gestibile o Da verificare né a una voce della card interventi;
  - versione installata fuori dall'intervallo della regola: affidabilità bassa, niente Gestibile né Da verificare;
  - due regole in conflitto: si mostrano entrambe, con l'affidabilità più bassa, e non si propone niente.

### Regole

Base di conoscenza in file TOML nel repository, incorporata nell'eseguibile. Ogni regola dichiara:
- dove si applica: cartella nota di Windows più un percorso relativo, oppure una prova trovata (voce di registro, manifest);
- come riconosce la struttura: schemi di nomi, file marcatore;
- quali prove raccoglie, con quale estrattore, e quali di queste valgono come prova di identità e quali come prova di gestibilità;
- i campi del record: stato, azione, metodo preferito e alternative, controlli preventivi automatici e manuali, metodo di stima, reversibilità, privilegi, comportamento in caso di errore, verifica dopo l'azione;
- se richiede i controlli di Restart Manager (F-AC-8) e dei riferimenti (F-AC-9), e se usa l'eccezione a F-AC-4, ammessa solo con stato Da verificare;
- fonti: URL, estratto testuale della frase che giustifica la gestione, intervallo di versioni, data di verifica;
- una fixture reale anonimizzata e la tabella del risultato atteso (06 §2.1).

Chi scrive le regole (RS10): una persona, che copia l'estratto dalla fonte e controlla che dica ciò che la regola afferma. Un'AI può aiutare a cercare le fonti durante lo sviluppo; estratto e controllo restano alla persona. La data di verifica è visibile nell'ispettore e nella conferma; la procedura di riverifica è in 07 §5.1. Scadenza, proposta da 07 §5.1 e da confermare (03 §6.1, regola 8): oltre 6 mesi il banner "N regole da riverificare", oltre 11 mesi l'annuncio in ispettore e conferma, oltre 12 mesi fatti dedotti ad affidabilità bassa, nessuna azione né procedura e stato Non sappiamo.

### Estrattori

| Estrattore | Cosa legge | Quando | Note |
|---|---|---|---|
| Voci di disinstallazione | `...\Uninstall` in HKLM, HKLM WOW6432Node e HKCU: nome, versione, `InstallLocation`, `InstallDate`, `EstimatedSize`, `DisplayIcon`; `UninstallString` solo come indizio di percorso | Scansione | Mai eseguite (D7). Le voci in HKCU sono scrivibili dall'utente (RS22): come prova di percorso valgono con affidabilità media |
| Pacchetti MSIX | `PackageManager.FindPackagesForUser("")`: PFN, `InstalledLocation` | Scansione | Dati in `%LOCALAPPDATA%\Packages\<PFN>` |
| Steam | `libraryfolders.vdf`, `appmanifest_*.acf` (SizeOnDisk, LastPlayed), `userdata\<id>\config\localconfig.vdf` (LastPlayed) | Scansione | Formato non documentato [F] |
| Epic | `Manifests\*.item` (JSON) | Scansione | Nessun dato d'uso |
| CapCut | `Apps\ProductInfo.xml` | Scansione | Insieme alla voce HKCU dà la versione in uso "da fonte" |
| UserAssist | `HKCU\...\Explorer\UserAssist\{GUID}\Count` (ROT13, avvii, ultimo avvio) | Scansione, solo se `Start_TrackProgs` = 1 | Con 0 la UI scrive "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso" |
| Risorsa di versione PE | `GetFileVersionInfoW` | Su richiesta, lente Download | Legge parte del file |
| Proprietà MSI | `MsiOpenDatabaseW` con `MSIDBOPEN_READONLY` e una query sulla tabella Property | Su richiesta | Mai `MsiOpenPackage` né `MsiOpenPackageEx`, che dipendono dallo stato della macchina [V] |
| Directory centrale degli zip | Record di fine directory e directory centrale: nomi e dimensioni | Su richiesta | Il contenuto compresso non si legge |
| Hash | SHA-256 del contenuto | Su richiesta, solo tra file della stessa dimensione | Mai sui segnaposto cloud |
| Configurazione (F-SY-3) | Ibernazione, avvio rapido e azione per batteria critica; Sensore memoria; `MaxCapacity` e `NukeOnDelete` in `HKCU\...\Explorer\BitBucket\Volume\{GUID}`; `Start_TrackProgs`; ripristino configurazione se leggibile; percorso della cache npm da `.npmrc` o dalla variabile d'ambiente | Scansione | Solo lettura di registro, file e API, mai esecuzione di programmi. Ibernazione e batteria si leggono con le API di alimentazione invece di eseguire `powercfg /a`; la corrispondenza con `powercfg /a` si controlla in M3. La lettura di Sensore memoria e del Cestino senza privilegi si verifica in M1 |

Regole comuni agli estrattori:
- Leggono solo i file nominati da una regola o scelti dall'utente nella lente Download, con un limite di dimensione; mai documenti dell'utente di propria iniziativa.
- Le letture di contenuto (risorsa di versione, MSI, zip, hash) avvengono solo su richiesta, perché aggiornano la data di ultimo accesso: su C: l'aggiornamento è attivo anche se `fsutil` scrive "Disabled" (`revisione/prove-windows.md` §A). Prima della lettura la UI scrive "aggiorna la data di ultimo accesso di questi file". Quado non apre handle in scrittura per evitarlo (RS19).
- Le letture di contenuto girano in background, si possono annullare e usano una cache in memoria con chiave (percorso, dimensione, data di modifica, ID file). La cache vive per la sessione: salvarla vorrebbe dire scrivere percorsi su disco, e va progettata con lo storico della v1.1.
- Nessuna lettura di contenuto sui segnaposto cloud.
- Un formato inatteso produce lo stato "estrattore non riuscito" con il motivo, mai un dato ricostruito.

Esclusi dalla v1 (`revisione/prove-windows.md`): firma Authenticode (un'apertura per file, e la verifica rischia la rete); Zone.Identifier con HostUrl (privacy e copertura bassa, RS21); marcatori NSIS, Inno e Squirrel (costosi e ambigui); BAM, Prefetch, Amcache e SRUM (richiedono l'amministratore e hanno un carattere forense); jump list e RecentDocs (contengono nomi di documenti); data di ultimo accesso (rumore).

### Programmi, residui e lente Download

- Uso dei programmi (F-AP-4): "ultimo avvio noto" solo da LastPlayed di Steam e da UserAssist con il tracciamento attivo, ciascuno con la sua copertura (UserAssist non registra gli avvii da riga di comando, da altri launcher e da attività pianificate [F]). `InstallDate` dal registro. Epic e MSIX non hanno un dato d'uso. Mai "inutilizzato", "mai usato" o "non usato da N giorni": lo stato è Decidi tu.
- Residui (F-AP-6): cartelle in Local, Roaming, LocalLow e ProgramData non rivendicate da alcun programma installato né da una regola. Stato Non sappiamo, data di ultima modifica, testo "Nessun programma installato corrispondente (confronto per nome). Ultima modifica: {data}. Potrebbe essere un residuo." (04 §6.8). Nessuna proposta: il confronto per nome dà falsi positivi (`revisione/prove-windows.md` §D).
- Lente Download (F-DL):
  - famiglie per estensione, su `Downloads` e su qualunque cartella dell'utente;
  - suffisso "(N)" solo come filtro;
  - copie identiche: hash su richiesta tra file della stessa dimensione, poi Gestibile; si tiene il più vecchio per data di creazione, e l'utente può sceglierne un altro;
  - archivio con una cartella omonima accanto: "N voci su M presenti", stato Decidi tu con la prova misurata;
  - download incompleti (`.crdownload`, `.part`) non in uso: Gestibile;
  - installatori: nome e versione dalla risorsa PE o dalla tabella Property; corrispondenza con i programmi installati (stessa versione, versione diversa, non installato). ProductCode uguale alla chiave di disinstallazione vale "da fonte"; per gli exe il confronto di nome e versione è dedotto, con l'affidabilità di 03 §6.2.

### Perché niente AI durante l'esecuzione

| Problema | Effetto |
|---|---|
| Non può citare una fonte verificabile per ciò che afferma | Viola la provenienza per affermazione |
| Risposte non deterministiche | Lo stesso disco darebbe spiegazioni diverse; impossibile scrivere test di regressione |
| Modello nel cloud | Percorsi e nomi di file lascerebbero il PC (è la scelta di MangoDisk, 01 §3) |
| Modello locale | Centinaia di MB o più per spiegare poche decine di casi frequenti |

Una regola sbagliata si corregge in un punto, e la correzione è verificabile da un test.

## D7. Interventi

### Tipi (F-AC-1)

| Tipo | Come | Note |
|---|---|---|
| Spostamento nel Cestino | `IFileOperation` con `FOFX_RECYCLEONDELETE` e un sink di avanzamento; mai `FOF_NOCONFIRMATION` | Solo da un piano confermato |
| Procedure di Windows | "Apri il Cestino" (`shell:RecycleBinFolder`); "Svuota il Cestino" con `SHEmptyRecycleBinW` e il dialogo di conferma di Windows; Impostazioni › Archiviazione (`ms-settings:storagesense`) e File temporanei (URI diretto da verificare in M5) | Pulizia disco resta un'alternativa, con la fonte ricontrollata (C5) |
| Disinstallazione | Interfaccia ufficiale: `ms-settings:appsfeatures` con il nome copiato negli appunti; `ms-settings:appsfeatures-app?<PFN>` per i pacchetti MSIX; `appwiz.cpl` (`control /name Microsoft.ProgramsAndFeatures`); `steam://uninstall/<appid>`; libreria del launcher Epic | Mai `UninstallString` né `QuietUninstallString` (RS22) |
| Comandi ufficiali | Mostrati con la fonte e copiati negli appunti, mai eseguiti: `npm cache verify` (alternativa `npm cache clean --force`), `pnpm store prune`, `Dism /Online /Cleanup-Image /StartComponentCleanup`, `powercfg /hibernate /type reduced` e `powercfg /h off`, comandi di WSL e Docker | `--volumes` non si mostra mai; `/ResetBase` solo come alternativa, con la conseguenza scritta |

Dopo una procedura o un comando esterno, il pulsante "Ho eseguito la procedura" avvia le nuove misure della verifica.

### Controlli prima di uno spostamento nel Cestino

1. Elementi protetti (F-AC-4): cartella Windows, WinSxS, Windows\Installer, System Volume Information, file di paging, ibernazione e swap, cartelle di programmi registrati. L'unica eccezione è quella dichiarata da una regola con stato Da verificare e con i suoi controlli (caso CapCut, `revisione/decisioni-revisione.md` §3).
2. Unità: se il volume non ha Cestino (rimovibile, rete) o `NukeOnDelete` vale 1, l'azione è disattivata con la spiegazione.
3. Capacità (F-AC-7, RS1): `MaxCapacity` letta in sola lettura dalla chiave BitBucket del volume (GUID da `GetVolumeNameForVolumeMountPointW`), confrontata con il contenuto attuale (`SHQueryRecycleBinW`) più la selezione. Se il totale supera la capacità, l'azione si blocca. Come Windows conta la capacità (dimensione logica o allocata, orfani inclusi o no) si verifica in M1 su VHDX; fino ad allora il controllo usa la stima più prudente.
4. Sensore memoria (F-AC-7, RS2): se lo svuotamento automatico del Cestino è attivo, la conferma lo dice con il numero di giorni.
5. Restart Manager (F-AC-8, RS6): `RmStartSession`, `RmRegisterResources` con i file dell'elemento, `RmGetList`. Per le cartelle grandi si registrano tutti gli eseguibili e le DLL più un campione fino a 1.000 altri file, scelti tra i modificati più di recente. Se un processo li usa, l'elemento è bloccato con il nome del processo.
6. Riferimenti (F-AC-9, RS5), quando la regola li richiede: registro (`UninstallString`, `DisplayIcon`, `InstallLocation`, App Paths, chiavi Run di HKCU e HKLM), collegamenti `.lnk` di Start e Desktop (dell'utente e pubblici) letti con `IShellLinkW::GetPath` senza `Resolve`, attività pianificate visibili all'utente (`ITaskService`). Un riferimento che punta all'elemento lo blocca e compare nella conferma. Se alcune attività non sono visibili all'utente, il controllo è "parziale" e la conferma lo dice.
7. Stima (03 §6.3): si rileggono gli attributi dei file selezionati con `FileStandardInfo` e `FileIdInfo`, aprendoli con soli attributi, `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`. Si contano le allocate dei file i cui nomi stanno tutti nella selezione, deduplicate per ID; un file con altri nomi fuori dalla selezione è escluso e citato; nomi non conoscibili fanno dire "intervallo"; segnaposto: solo i byte locali; residenti nella MFT: 0; contenuto dei dischi virtuali: 0 fino alla compattazione; nel Cestino: 0 subito; con i punti di ripristino attivi, la nota "parte può essere riassorbita". Stato: esatta, intervallo o non stimabile. Il costo si misura in M5; la simulazione mostra l'avanzamento e si può annullare.
8. File sincronizzati (F-AC-2, R10): se un elemento sta in una sync root registrata, la simulazione aggiunge un controllo manuale che dice che lo spostamento nel Cestino lo elimina anche nel cloud e sugli altri dispositivi; senza la spunta il piano non si costruisce.

### Piano confermato (F-AC-2, V6)

- La simulazione produce un valore `Simulazione`: elenco, totale, stima con metodo e stato, esito dei controlli, scheda della raccomandazione.
- Il tipo `PianoConfermato` si costruisce solo dalla funzione di conferma del dialogo, a partire da una `Simulazione` e dalla scelta dell'utente: i campi sono privati e non esiste un altro costruttore. La costruzione respinge gli elementi protetti (salvo l'eccezione di regola con tutti i controlli soddisfatti), quelli con un controllo automatico fallito e quelli con un controllo manuale non spuntato.
- L'API di esecuzione di `quado-actions` accetta solo un `PianoConfermato` e lo consuma: un piano vale una volta.
- Esc, Annulla e la X chiudono il dialogo senza costruire il piano (F-AC-3).

### Esecuzione

- Subito prima dello spostamento ogni elemento si ricontrolla: ID file, dimensione, data di modifica e, per le cartelle, di nuovo Restart Manager. Se qualcosa è cambiato, l'elemento si salta e compare nel riepilogo (RS6).
- In `PreDeleteItem`, se manca `TSF_DELETE_RECYCLE_IF_POSSIBLE` (cioè Windows eliminerebbe definitivamente), l'operazione per quell'elemento si annulla e si segnala (F-AC-5). Il comportamento del flag e il valore di ritorno che annulla l'elemento si verificano in M1 su VHDX con un Cestino piccolo (IP9). Se la verifica fallisce, resta il solo controllo di capacità e 08 aggiorna R8.
- Il crate `trash` usa la stessa interfaccia [V] ma non espone questo controllo: si chiama l'API direttamente.
- Riepilogo con riusciti, saltati e falliti e il motivo (F-AC-3). In caso di esito parziale dice cosa è nel Cestino e come ripristinarlo (RS8).
- Registro degli interventi (F-AC-6, D8).

### Verifica (F-VE)

1. Misura di partenza (F-VE-1): spazio libero (`GetDiskFreeSpaceExW`), dimensione del Cestino (`SHQueryRecycleBinW`), stima con metodo e stato, ora.
2. Nuove misure (F-VE-2) dopo l'azione, dopo lo svuotamento del Cestino e dopo "Ho eseguito la procedura": ciascuna con lo spazio libero e la riscansione dei sottoalberi toccati.
3. Si mostrano due numeri distinti (F-VE-3): "stimato" (somma delle allocate secondo il metodo di stima) e "variazione del volume" (differenza di spazio libero), che chiamiamo ottenuto. La variazione del volume include l'attività degli altri processi (RS17); per questo accanto c'è la riscansione, che dice quanto della selezione occupa ancora spazio.
4. Oltre il 5% di differenza tra stimato e ottenuto, la differenza si spiega voce per voce (elementi saltati, nomi rimasti fuori, contenuto ancora nel Cestino, file ancora aperti) oppure si dichiara "non spiegata".
5. Il registro conserva stimato e ottenuto (F-AC-6).
6. Il modello si aggiorna riscansionando i sottoalberi toccati (F-VE-4, D5).
7. La vista Interventi mostra lo storico dal registro (F-VE-5).

### Confini nel codice

Tutte le chiamate che modificano file, avviano procedure o scrivono negli appunti stanno in `quado-actions` (D10); un test di architettura lo verifica (06 §5.1). Il lanciatore di procedure accetta solo valori tipizzati (URI di Impostazioni, `steam://uninstall/` con un numero, PFN validato, apertura del Cestino), mai stringhe lette dal registro.

## D8. Dati salvati in locale

| Cosa | Dove | Quando |
|---|---|---|
| Impostazioni | `%APPDATA%\Quado\settings.toml` | Al primo cambio di impostazione |
| Registro degli interventi | `%APPDATA%\Quado\interventi.jsonl`: per intervento data, elementi con percorso, metodo, stima con metodo e stato, misure prima e dopo, ottenuto, differenza e spiegazione, esito per elemento | Dopo il primo intervento |
| Log tecnico | `%APPDATA%\Quado\log\`, rotazione a 5 file da 1 MiB, senza percorsi completi salvo scelta esplicita | Sempre |
| Storico delle scansioni | v1.1, opt-in | Solo se attivato |

- Una sola sede: niente modalità portable e niente file accanto all'eseguibile.
- I percorsi nel registro degli interventi servono a "Apri il Cestino" e alla vista Interventi.
- La pagina "Dati e privacy" elenca questi file con la loro dimensione e ha un pulsante che li cancella.
- Nessun crash reporter di rete: i crash finiscono nel log tecnico.
- La cache dei metadati degli installatori resta in memoria (D6).

## D9. Distribuzione e aggiornamenti

- Build locale: `cargo build --release` per x64 con CRT statico; `cargo xtask installa` copia l'eseguibile in `%LOCALAPPDATA%\Programs\Quado` e crea il collegamento nel menu Start dell'utente (07 §6).
- Un eseguibile compilato in locale non ha il Mark-of-the-Web: SmartScreen non interviene e la firma del codice non serve. Il prompt UAC dell'helper mostra "Autore sconosciuto", ed è atteso.
- Niente MSI, firma, winget, Store, zip portable, ARM64 in v1.
- Aggiornare significa ricompilare dal sorgente e reinstallare. Nessun controllo di aggiornamenti e nessuna rete.

## D10. Organizzazione del codice

| Crate | Responsabilità | Perché separato |
|---|---|---|
| `quado-model` | Arena, aggregazione, query (figli ordinati, top N, ricerca, file grandi), tipi dei fatti e record di raccomandazione | Indipendente dalla piattaforma: test veloci |
| `quado-scan-win` | Volumi, enumerazione, correzioni, rilettura degli attributi, spazio libero, protocollo dell'helper | Unico punto con le API di lettura del file system |
| `quado-kb` | Regole, motore, estrattori, letture di configurazione | Le regole hanno test e ciclo di aggiornamento propri |
| `quado-actions` | Controlli preventivi, piano confermato, Cestino, procedure, verifica, registro e dati locali | Unico crate con API di scrittura e di avvio di procedure: si audita da solo |
| `quado-ui` | Token, widget, mappa, tabella, ispettore, dialoghi, lente Download, Programmi, Interventi | Sostituibile (piano B) senza toccare il resto |
| `quado` | Eseguibile: avvio, modalità helper | Sottile |
| `xtask` | Alberi di prova, VHDX di prova, prestazioni, dati di riferimento, controllo delle importazioni, installazione locale | Strumenti di sviluppo |

- Dipendenze in una sola direzione: `model` non dipende da nessuno; `scan-win` e `kb` da `model`; `actions` da `model` e `scan-win`; la UI da tutti.
- `#![forbid(unsafe_code)]` vale ovunque tranne `scan-win`, il modulo degli estrattori Windows in `kb`, `actions` e il punto di ingresso dell'helper. Ogni blocco `unsafe` ha il suo invariante scritto e un test.
- Niente plugin, servizi, database o motore a comandi generalizzato come quello delle app di riferimento: non c'è un caso d'uso che li giustifichi. La palette dei comandi passa alla v1.1.

## D11. Linux

Solo nota di portabilità, nessun lavoro in v1.

| Riutilizzabile | Solo Windows |
|---|---|
| `quado-model`, `quado-ui`, motore di `quado-kb`, struttura dei test | Enumerazione NT, helper, regole su registro, MSIX, Steam ed Epic per Windows, procedure di Windows, `IFileOperation`, Restart Manager |

Un porting richiederebbe la scansione con `statx` (`st_blocks` per l'allocata, coppia dispositivo e inode per gli hard link, rispetto dei punti di montaggio), il Cestino secondo la specifica freedesktop e regole per i percorsi XDG. Niente in v1 deve renderlo più difficile, e niente si fa apposta per facilitarlo.

## D12. Strumenti

- Rust stable. Sulla macchina di sviluppo c'è la 1.88.0 (giugno 2025): egui 0.36 e le dipendenze correnti richiedono una toolchain più recente, quindi serve `rustup update`, da approvare prima di eseguirlo (07 §8). La toolchain si fissa con `rust-toolchain.toml`.
- `cargo-deny` per licenze, divieto dei crate di rete e avvisi di sicurezza.
- `egui_kittest` per test della UI e snapshot; `proptest` per i test di proprietà; `trybuild` per i test di compilazione del piano confermato.
- Process Monitor ed ETW per i controlli manuali di sola lettura e di rete.
- `dumpbin` dei Build Tools di Visual Studio, oppure un comando `xtask` con il crate `object`, per importazioni e load config.
- Test in locale invece della CI privilegiata: tutto gira sul PC di sviluppo. I test su VHDX richiedono l'amministratore per creare e montare i volumi, con il prompt UAC confermato da una persona davanti al PC a ogni sessione (06, introduzione). Una CI ospitata senza privilegi è facoltativa e non serve alla v1.

## D13. Lingue dell'interfaccia (13/10/2026)

Decisione del 13/10/2026, priorità alta: l'interfaccia non si limita all'italiano. Italiano e inglese sono completi nella v1; altre lingue si aggiungono dopo senza toccare il codice. Il proprietario del progetto e Claude devono poter correggere un testo o aggiungere una lingua modificando file di testo. Sostituisce la decisione di togliere l'inglese (`revisione/decisioni-revisione.md` §1).

| Aspetto | Scelta | Perché |
|---|---|---|
| Formato | Fluent (`.ftl`, progetto Mozilla, crate `fluent-bundle`), un file per lingua: `crates/quado-ui/lingue/<tag>.ftl` (`it.ftl`, `en.ftl`) | File di testo leggibili e modificabili; plurali e varianti secondo le regole CLDR di ogni lingua, non solo singolare e plurale; segnaposto con nome che il traduttore può spostare. Le app di riferimento usano cataloghi TSV propri: stessa idea, ma Fluent copre i plurali di tutte le lingue senza codice nostro |
| Chiavi | Identificatori stabili (`panoramica-titolo`, `stato-decidi-tu`), non il testo italiano | Cambiare una frase in una lingua non rompe le altre |
| Lingua di riferimento | Italiano: `it.ftl` ha un commento di contesto per ogni chiave, che spiega dove compare e cosa non dire (04 §6.8) | È la lingua in cui sono scritti i testi di 04; il commento serve a chi traduce, persona o Claude |
| Scelta della lingua | Impostazioni ▸ Lingua, cambio immediato senza riavvio. Al primo avvio la lingua dell'interfaccia di Windows, se disponibile; altrimenti inglese | egui ridisegna ogni frame: il cambio è gratuito |
| Ripiego | Lingua scelta, poi inglese, poi italiano, per chiave | Una lingua aggiunta e incompleta resta usabile |
| Modifiche senza ricompilare | I file in `%APPDATA%\Quado\lingue\` sostituiscono o aggiungono lingue all'avvio; un file con errori si ignora per le sole righe sbagliate e l'elenco degli errori compare in Impostazioni ▸ Lingua | L'utente può correggere un testo con un editor; i file incorporati restano la base sicura |
| Numeri e date | Dal formato dell'area geografica di Windows (separatore decimale e delle migliaia, data breve), come Esplora file, indipendenti dalla lingua dell'interfaccia | F-GE-3 promette lo stesso formato di Esplora. Le unità restano 1 GB = 1024³ byte; le etichette ("byte", "Dimensioni su disco") vengono dai file di lingua |
| Testi del core | Il core (`model`, `scan-win`, `kb`, `actions`) non produce mai frasi: restituisce codici e parametri (motivo di una cartella negata, stato, controllo fallito), che la UI traduce | Una sola sede dei testi; il registro degli interventi salva codici e parametri e si rilegge nella lingua corrente |
| Regole (M3) | Ogni testo visibile di una regola ha una chiave; i testi stanno in `regole/<tag>.ftl`. Una regola entra solo con il testo in italiano e in inglese | La base delle regole resta verificabile e traducibile come il resto |
| Font | IBM Plex copre le lingue latine; per le altre scritture valgono i font di ripiego di Windows (M0), caricati solo quando servono. Arabo ed ebraico: limite noto di egui sul testo bidirezionale (D2), da rivalutare prima di aggiungere quelle lingue | |
| Strumenti | `cargo xtask lingue`: copertura di ogni lingua (chiavi mancanti, in più, segnaposto diversi) e scheletro di una lingua nuova, con i commenti di contesto | Aggiungere una lingua diventa: generare lo scheletro, tradurre, provare |

Test (06 §7): completezza di `it` ed `en` (stesse chiavi, stessi segnaposto, nessuna riga con errori); pseudo-lingua di prova con testi più lunghi del 40% e caratteri accentati, per trovare testi tagliati e stringhe scritte nel codice; snapshot di ogni schermata anche in inglese; i testi del core sono solo codici.

## Aggiornamento dopo M1 (13/10/2026)

- **D4.** La correzione dell'allocata con `FileStandardInfo` vale anche per i file sparse con allocata 0 nella voce di cartella (la voce può restare a 0 dopo la scrittura). Su exFAT e FAT32 `FileIdExtdDirectoryInfo` non è supportato: si usa `FileFullDirectoryInfo`, senza identità dei file (quei file system non hanno hard link). Prestazioni a cache calda: mediana 14,7 s su C:.
- **D5.** Misurati 326 MB di picco per 3,41 milioni di nodi con i nomi reali, dopo la riduzione 2 (nomi ASCII a 8 bit in un pool a blocchi, gli altri in UTF-16 esatti); le riduzioni 1 e 3 non servono. Gli hard link si attribuiscono al nome con il percorso alfabeticamente minore, senza distinzione di maiuscole: per esempio `System32` prima di `WinSxS`.
- **D7.** `PreDeleteItem` non è una protezione affidabile (08 R8; prove in `m1/cestino-verifiche.md` e `m1/cestino-verifiche-bis.md`). F-AC-5 si realizza così: controllo preventivo per elemento (dimensione logica ≤ `MaxCapacity` del volume, `NukeOnDelete` spento), un'operazione `IFileOperation` per elemento, verifica con `PostDeleteItem` dopo ciascuno e arresto del piano al primo elemento non andato nel Cestino. F-AC-7 conta la dimensione logica e non conta le cartelle orfane.

## Aggiornamento dopo M2 (13/10/2026)

- **D2.** La finestra si apre con la scheda video collegata al monitor principale (DXGI, vendor e device dell'adattatore): su un PC con due schede video si evita così la copia tra GPU misurata in M0. Se la finestra passa su uno schermo collegato all'altra scheda, l'adattatore resta quello scelto all'avvio.
- **D13, come è stata realizzata.** `crates/quado-ui/lingue/*.ftl` si incorporano tutti da `build.rs`: aggiungere una lingua al programma è aggiungere un file (`cargo xtask lingue nuova <tag>` genera lo scheletro con i commenti). I file in `%APPDATA%\Quado\lingue` sostituiscono chiave per chiave; Impostazioni ▸ Lingua ha "Apri la cartella delle lingue", "Crea un modello da tradurre" (copia `it.ftl` commentato) e "Ricarica i file". La pseudo-lingua `x-pseudo` è costruita sull'italiano con la trasformazione di Fluent. I numeri si passano ai testi già formattati; `$n` serve solo al plurale. Con Windows in inglese e il formato regionale italiano, al primo avvio Quado parte in inglese con numeri e date all'italiana, come la regola prevede.
- **Unità (F-GE-3).** Esplora file, dai GB in su, calcola dai MB interi e poi tronca; `format::size` fa lo stesso ed è confrontato con `StrFormatByteSizeEx` da un test.
- **Top list senza regole (F-OV-2).** Fino a M3 un elemento è un nodo dell'albero: si parte dai figli della radice e una cartella lascia il posto al suo contenuto quando i suoi figli che entrano nei primi dieci coprono almeno metà dei suoi byte. Così `Users` e la cartella del profilo si aprono, `Downloads` resta un elemento; ma una catena di cartelle quasi uniche porta in fondo (dalla cartella di un gioco fino a una sua sottocartella interna). Con M3 le unità le decidono le regole (Steam, CapCut, Cestino) e la regola strutturale resta solo per ciò che nessuna regola riconosce.
- **Mappa.** "Due livelli con intestazione" (F-EX-1) si realizza come nell'anteprima: sotto la cartella aperta si vedono i gruppi con l'intestazione e, dentro, le loro parti (tre livelli di aree). Le aree sotto 2 pixel quadrati non si dispongono: una cartella con un milione di figli non produce un milione di aree.
- **Lavori in background.** Ricerca, File grandi, misure dell'ispettore e verifica di una voce girano in thread propri su una copia condivisa del modello (`Arc`); prima di modificare il modello (verifica, nuova scansione) l'interfaccia aspetta che finiscano. Gli indici delle viste si costruiscono nel thread della scansione.

## Aggiornamento dopo M3 (13/10/2026)

- **D6, come è stata realizzata.** Ogni regola è un file `crates/quado-kb/regole/<id>.toml` con fonti (URL o file del PC di sviluppo, estratto, ufficiale sì o no), data di verifica, fixture e un riconoscitore scritto in Rust (percorso sotto una cartella nota, versioni di CapCut, Cestino, Steam, Epic, programmi registrati, MSIX, cache di npm e pnpm, lockfile, cartelle dell'utente). I testi sono chiavi `regola-<id>-<campo>` in `regole/<lingua>.ftl`, incorporati con le lingue dell'interfaccia. Il motore calcola l'affidabilità con 03 §6.2; un byte ha un solo proprietario, l'elemento riconosciuto più profondo. Due regole sullo stesso elemento: se concordano su gestore e stato vale la più specifica, altrimenti conflitto. Le fixture reali anonimizzate si rigenerano con `cargo xtask fixture-regole`.
- **Stato "Lo gestisce X" senza azione** vale come la variante "Nessuna procedura: non toccare" (DriverStore, file di paging, `Windows\Installer`, cartella di Windows, cartelle orfane del Cestino).
- **Card interventi.** Le disinstallazioni (programmi, giochi, app MSIX) restano fuori dalla card e vanno nella vista Programmi (M4): nella card ci sono solo le azioni del disco e le procedure.
- **Estrattori.** Girano insieme alla scansione (49-69 ms). CapCut legge anche `User Data\Config\globalSetting` (`currentCustomDraftPath`), che dichiara la cartella delle bozze. Il ripristino configurazione non si legge senza amministratore (`SPP\Clients`): "non disponibile". La configurazione letta sul PC di sviluppo: Sensore memoria ogni 7 giorni con il Cestino dopo 30, Cestino di C: circa il 5% dell'unità, `Start_TrackProgs` = 0, batteria critica = ibernazione.
- **Prestazioni.** Classificazione di 3,44 milioni di voci in circa 52 ms, costruita in parallelo con gli indici delle viste: dopo un annullamento il risultato pronto arriva in al massimo 442 ms.

## Aggiornamento dopo M4 (14/10/2026)

- **Record senza regola.** Le voci della lente Download e dei Programmi non sono regole della base (07 §7): il record porta `Origin::Function` (stessa dimensione, copie identiche, installatore, archivio con cartella, immagine disco, download incompleto, programma, possibile residuo) e decide solo da prove misurate o lette. Le regole restano `Origin::Rule`, gli elementi senza regola `Origin::NoRule`.
- **Letture del contenuto.** SHA-256 con CNG di Windows (nessuna dipendenza nuova), tre thread; directory centrale degli zip letta dalla coda del file; risorsa di versione con `FILE_VER_GET_NEUTRAL` (le stringhe del file, non del MUI); MSI con `MsiOpenDatabaseW` in sola lettura e un controllo della struttura prima (un MSI troncato si apriva ancora); percorsi oltre 260 caratteri con il nome breve 8.3 per MSI e Restart Manager. Le letture passano da un insieme di funzioni iniettabile: i test della UI non leggono file.
- **Copie identiche.** I nomi secondari degli hard link non entrano nel confronto: sono gli stessi dati di un altro nome. L'elenco della lente disegna le 200 voci più grandi, con "Mostra altre voci".
- **Restart Manager** gira all'apertura della lente sui soli download incompleti (non legge il contenuto, non cambia l'ultimo accesso); da confermare.
- **Programmi.** Le voci "Steam App N" del registro si uniscono al gioco di Steam. I possibili residui si confrontano per parole (tutte le parole del nome della cartella in uno stesso nome di programma, editore, pacchetto o gioco); da confermare. La disinstallazione apre l'interfaccia ufficiale con un lanciatore a valori tipizzati; il nome si copia negli appunti come "Copia percorso" (egui), le aperture passano da `quado-actions`.

## Aggiornamento dopo M5 (14/10/2026)

- **D7, uso dei file (F-AC-8, e F-DL-6 di M4).** Restart Manager resta per i file comuni, ma non per tutti: su VHDX `RmRegisterResources` più `RmGetList` decomprimevano i file compressi WOF (il volume perdeva lo spazio della compressione) e chiedevano al provider i dati dei segnaposto cloud. Per questi file, e per i percorsi di 260 caratteri o più senza nome breve, l'uso si legge da NTFS con `FileProcessIdsUsingFileInformation` e un handle dei soli attributi (`FILE_FLAG_OPEN_REPARSE_POINT`, `FILE_FLAG_OPEN_NO_RECALL`): niente si decomprime né si scarica. Non vale per tutti perché costa circa 28 ms per file (Windows scorre tutti gli handle del sistema), mentre Restart Manager fa un solo giro per sessione. La strada di un file si decide aprendolo con i soli attributi: segnaposto dagli attributi e dal tag, WOF con `FSCTL_GET_EXTERNAL_BACKING`. Nella domanda diretta un processo conta se tiene il file in quattro domande a 250 ms di distanza (Defender apre per un attimo i file appena scritti) e il nome è quello dell'eseguibile. Campione invariato: tutti gli eseguibili e le DLL più i 1.000 file più recenti.
- **D7, stima (03 §6.3).** Contano anche gli indici delle cartelle (`FileStandardInfo.AllocationSize` di una cartella), che lo svuotamento libera; i file residenti nella MFT (allocata non multipla del cluster, 16 byte per un file da 10) valgono 0. Ogni elemento del piano porta la sua quota della stima (un file con un nome fuori non è di nessuno). Tolleranza di 06 §6.1 confermata: 1% della stima più 1 MiB.
- **D7, controlli.** Percorsi di 260 caratteri o più, anche dentro una cartella, bloccati in simulazione (la shell non li mette nel Cestino e chiede di eliminarli per sempre). Capacità del Cestino non registrata (`MaxCapacity` assente): blocco. Riferimenti "parziali" (attività pianificate non visibili a un utente standard: 48 sul PC di sviluppo) dichiarati nella conferma, mai un blocco, nemmeno per l'eccezione di CapCut.
- **D7, esecuzione.** Flag di `IFileOperation`: `FOF_ALLOWUNDO`, `FOFX_RECYCLEONDELETE`, `FOF_SILENT`, `FOF_NOERRORUI`, `FOF_WANTNUKEWARNING`; mai `FOF_NOCONFIRMATION` né `FOF_NO_UI`: se Windows volesse eliminare per sempre, chiede all'utente. Un'operazione senza elemento nel Cestino (`NotRecycled`) ferma sempre il piano. "Svuota il Cestino" con il dialogo di Windows: annullarlo restituisce comunque S_OK, quindi l'esito si legge dalle misure.
- **D7, procedure.** Non c'è un URI diretto per File temporanei su Windows 10 22H2: si apre Archiviazione (`ms-settings:storagesense`). "Ho eseguito la procedura" anche dopo la disinstallazione (F-AP-5), con la misura di partenza presa prima dell'apertura.
- **D8, registro.** Una riga JSON per misura con lo stesso id (vale l'ultima), codici e valori; ora delle misure con `GetSystemTimePreciseAsFileTime`.
- **D10.** `quado-actions` dipende da `quado-model` e `quado-scan-win`; registro, collegamenti e attività pianificate li legge `quado-kb`, e l'interfaccia li passa a `WinProbe` (`KbConfig`). Le prove di M1 con `FOF_NO_UI` e `SHERB_NOCONFIRMATION` sono passate in `xtask`; lo svuotamento silenzioso esiste solo in `quado-testkit::bin`, che rifiuta ogni volume che non sia un VHDX di prova.

## Aggiornamento dopo M6 (16/10/2026)

- **D3, archiviazione riservata.** Non passa più dall'helper: `GetDiskSpaceInformationW` dà a un utente normale lo spazio riservato e non usato (`TotalReservedAllocationUnits`, su C: 5.922 MB, di cui 5.605 MB di archiviazione riservata, uguale alla garanzia di `fsutil`). Entra nel bilancio come metadato misurato (`Metadata.reserved`), con una voce propria in barra e nella card dei conti.
- **D3, ambito deciso dall'helper.** È l'helper, non l'interfaccia, a decidere dal percorso cosa trasmettere: `Users\<nome>` diverso dal profilo di chi l'ha avviato (letto dal token del processo server e da `ProfileList`) e `$Recycle.Bin\S-...` diverso dal suo SID arrivano solo come totali; `Users` e `$Recycle.Bin` interi mai con i nomi. Se il percorso chiesto risulta un albero con i nomi, si controlla anche il percorso finale (junction, nomi brevi) e vince l'ambito più restrittivo.
- **D3, avvio.** L'avvio con `ShellExecuteExW` e `runas` sta in `quado-actions` (`elevate`), l'unico crate che crea processi; protocollo, pipe, modalità helper e raccolta stanno in `quado-scan-win` (`helper`). L'helper tiene aperto l'handle del genitore fino alla fine (il suo pid non si riusa) e risponde `Failed` con 223 a un sottoalbero oltre `MAX_REPLY_BYTES`.
- **D3, interfaccia.** Le risposte si applicano tutte insieme all'arrivo di `Done`: un No al prompt, un annullamento o un errore non cambiano il modello. Cartelle elencate innestate con `Tree::graft`; profili e Cestini altrui con `Tree::set_measured` (`Flags::MEASURED`). Le copie shadow spiegano una parte di `System Volume Information` e non si sommano al bilancio.
- **D9, distribuzione.** `cargo xtask importazioni` (parser PE, normali e ritardate) e `cargo xtask installa` (build, importazioni, copia, `Quado.lnk` nel menu Start; con `--rimuovi-rilievo` toglie anche cartella e collegamento della versione che si chiamava Rilievo, dopo averli riconosciuti). Dal 26/10/2026 l'eseguibile ha l'icona del segno «Metà» come risorsa (`crates/quado/icona`, generata da `cargo xtask icona`, incorporata da `build.rs`): il collegamento la prende dall'eseguibile.
