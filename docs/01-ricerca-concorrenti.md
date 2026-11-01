# 01 · Ricerca sulle soluzioni esistenti

Stato al 12/10/2026. Convenzioni: [V] verificato su fonte primaria (documentazione, changelog, codice, licenza), [P] affermazione del produttore non verificata in modo indipendente, [I] mia deduzione, (u) segnalazione di utenti. Le sigle Sx rimandano al registro delle fonti in fondo. Nessun binario di terzi è stato scaricato o eseguito: velocità e traffico di rete dei concorrenti non sono misurati da me.

## Riesame dopo la revisione (12/10/2026)

Il confronto è stato rivalutato nella revisione critica (`10-revisione-critica.md`) e nelle decisioni che ne sono seguite (`revisione/decisioni-revisione.md` §1). Il contenuto di questo documento resta valido come fotografia del mercato; cambia la conclusione che se ne trae.

- Individuare cosa occupa il disco è già risolto da altri: WizTree, gratuito per uso personale, e le Impostazioni di Windows.
- Il valore di Quado sta nei passi successivi: comprendere, valutare, agire con controlli e verificare il risultato. Nessuno degli strumenti esaminati li copre senza mandare dati a un servizio AI nel cloud.
- È stata considerata l'alternativa di costruire Quado sopra l'export CSV di WizTree. È stata scartata: WizTree è chiuso, è veloce solo da amministratore, il CSV non contiene ID file né attributi, e si finirebbe per usare due app insieme.
- Quado è ora uno strumento personale. Le parti che parlano di pubblico, licenza, distribuzione e benchmark pubblicati contano solo se un giorno verrà pubblicato.

## 1. Sintesi

1. La scansione veloce su NTFS è una commodity. Leggono direttamente la MFT WizTree, WinDirStat dalla 2.5 (gennaio 2026), TreeSize (anche la Free) e diversi progetti Rust nati nel 2026 [V S1 S9 S19 S61]. Un prodotto nuovo non si distingue per velocità.
2. WinDirStat 2.x è il riferimento open source. Dal 2024 ha avuto 18 release, l'ultima è la 2.9.3 dell'11/10/2026 [V S10]. Ha MFT, deduplicazione degli hard link, duplicati via hash, sunburst e flame graph, tema scuro, recupero di file cancellati, rimozione delle copie shadow, ottimizzazione dei VHD e pulizia DISM [V S9 S15]. Dalla 2.9.2 la licenza è GPL-3.0-or-later, non più GPL-2.0 [V S9].
3. WizTree resta il più veloce e semplice, con tre limiti: licenza commerciale a pagamento, velocità solo da amministratore, duplicati solo per nome e dimensione [V S3 S4 S2].
4. TreeSize copre il segmento aziendale: cloud, scansioni pianificate, confronto tra snapshot, annullamento delle cancellazioni [V S16 S17]. La Free è solo per uso non commerciale e dalla 4.8 raccoglie dati d'uso anonimi, disattivabili [V S18 S20].
5. Everything non è un analizzatore visuale. È però l'unico con un indice persistente aggiornato tramite USN journal; dalla 1.5 (beta) indicizza anche le dimensioni delle cartelle ed espone un SDK MIT interrogabile da altre app [V S25 S29 S30].
6. Le funzioni integrate in Windows spiegano poco. Le dimensioni di "App installate" sono stime [V S41]. "Sistema e riservato" e "Altro" restano opachi, e le domande degli utenti non trovano risposta (Microsoft Q&A, maggio 2026) [V S40].
7. Il vuoto "perché è grande e di chi è" è aperto solo in parte. Nel 2026 sono comparsi MangoDisk (GPL-3, regole aperte più spiegazioni con AI nel cloud), SizeTrend (commerciale, panoramica per categorie) e diskplot (MIT, riconosce node_modules e cache) [V S55 S57 S58]. Nessuno fa un bilancio completo del volume (usato = attribuito + overhead di sistema spiegato + residuo non spiegato) con il livello di confidenza di ogni voce [I].

## 2. Confronto degli strumenti richiesti

| | WizTree | WinDirStat 2.x | TreeSize Free / Pro | SpaceSniffer | Everything | Windows (Impostazioni) |
|---|---|---|---|---|---|---|
| Versione | 4.33, 16/09/2026 [S2] | 2.9.3, 11/10/2026 [S10] | Free 4.8.1 (20/11/2025); Pro 9.9 [S18 S16] | 2.2.0.27, 25/03/2026 [S23] | 1.4.1.1032 stabile; 1.5.0.1423b beta (27/08/2026) [S27 S26] | Windows 11, aggiornamento continuo |
| Scansione | MFT diretta da amministratore, API altrimenti [S1 S3] | MFT opzionale da amministratore, altrimenti `NtQueryDirectoryFile` multithread [S9 S14] | MFT su NTFS locale, altrimenti thread paralleli [S19] | Enumerazione con motore in tempo reale [S24] | Indice MFT + USN persistente (servizio o amministratore) [S25] | Non documentato |
| Hard link contati una volta | Sì, solo NTFS e da amministratore [S2] | Sì dalla 2.5 [S9] | Sì, anche flussi alternativi [P S19] | n.d. | n.d. | No: Esplora sovrastima WinSxS [S37] |
| Allocato e logico | Sì [S3] | Sì [S9] | Sì [I] | Sì nei filtri [S24] | "Dimensioni su disco" migliorata nella 1.5 [S26] | No |
| Duplicati | Nome e dimensione [S2] | Hash, con hashing parziale sopra 64 MiB [S9] | Solo Personal/Pro, MD5/SHA256 [S17] | No | No | No |
| Viste | Mappa, albero, file | Mappa, sunburst, flame, curve di Hilbert/Moore [S9] | Mappa 2D/3D [S17] | Mappa dal vivo | Elenco | Categorie |
| Pulizia | Cancellazione con dialoghi di Windows [S2] | Cestino o definitiva con conferma; copie shadow, VHD, DISM, ibernazione [S15 S9] | Free base; Pro con annullamento e anteprima [S17 S16] | Cancellazione | Operazioni sui file | Sensore memoria, consigli di pulizia [S35 S36] |
| Licenza | Gratis per uso personale, commerciale a pagamento [S1 S4] | GPL-3.0-or-later [S9] | Free non commerciale; Pro da 41 € per utente all'anno [S17 S20] | Freeware chiuso [S22] | Licenza in stile MIT, codice chiuso [S28] | Sistema operativo |
| Rete | Validazione online della licenza, controllo aggiornamenti [S3 S2] | Nessun controllo aggiornamenti, scelta esplicita [S11] | Free: dati d'uso anonimi, disattivabili [S18] | Non documentato | Controllo aggiornamenti disattivabile [S34] | Telemetria di sistema |

### 2.1 WizTree

- Legge la MFT "direttamente dal disco" [P S1 S5]. La scansione NTFS ad alta velocità richiede i diritti di amministratore [V S3]. Senza, e su volumi non NTFS, di rete o su singole cartelle, usa l'enumerazione standard, migliorata nella 4.21 e nella 4.28 [V S2]. ReFS ed exFAT non sono mai citati nel changelog [V S2].
- Gli hard link sono contati una volta nell'allocato, ma rilevati solo su NTFS da amministratore [V S2]. Correzioni nel tempo per i file cloud di OneDrive (3.40, 4.28) e per i file compressi LZX (3.36); include i flussi dati con nome (3.00) [V S2].
- Velocità: "46x faster than WinDirStat" senza metodologia [P S1]. L'unico confronto pubblico è dell'autore di un concorrente: 6,5 s a caldo, 17,2 s a freddo, 635 MB di picco, senza hardware né numero di file dichiarati [S59, dato di parte].
- Nessuna attribuzione alle applicazioni e nessun consiglio di pulizia [I da S1 S2].
- Licenza: gratis per uso personale. L'uso commerciale richiede un codice a pagamento, da 25 $ (individuale) a 1.800 $ (enterprise multi sede), perpetuo [V S1 S4].
- Rete: contatta antibody-software.com per validare il codice [V S3]; controllo aggiornamenti configurabile [V S2]. Nella 3.37 ha cambiato la verifica della firma perché WinVerifyTrust "would trigger unwanted Internet access" [V S2].

### 2.2 WinDirStat 2.x

- Dalla 2.0.1 (ottobre 2024): multithreading, duplicati via hash, build x64 e ARM, MSI, CSV, privilegi di backup per leggere file inaccessibili. Dalla 2.5.0 (18/01/2026): tema scuro, scansione diretta della MFT, hard link, ottimizzazione VHD, analisi DISM. Dalla 2.7: costi di archiviazione per età. Dalla 2.8: conferme per le azioni distruttive. Dalla 2.9: dispositivi MTP, recupero file cancellati, passaggio da MFC a Win32/GDI [V S9].
- Scansione standard con `NtQueryDirectoryFile` (`FileIdFullDirectoryInformation`) più `GetCompressedFileSize` per la dimensione fisica; scansione NTFS con un parser dei record in `FinderNtfs.cpp` [V S14].
- Mostra una voce `<Unknown>` per la differenza tra usato e scansionato. Un collaboratore ammette che con la gestione degli hard link lo spazio da essi risparmiato finisce in `<Unknown>` "mixed with the actual unknown amount" [V S13]. È proprio il problema che il nostro bilancio vuole risolvere.
- Idratazione involontaria di OneDrive nella 2.5.0 durante la navigazione dell'albero, poi corretta [V S12].
- Nessun controllo aggiornamenti "not planned"; un collaboratore vuole "a utility that does not phone home" [V S11].
- Lamentele ricorrenti: rallentamenti dopo la scansione, problemi della scansione MFT, OneDrive, hard link nella mappa [V S13 S104].

### 2.3 TreeSize (JAM Software)

- La Free usa la MFT sui volumi NTFS locali [P S19]; con account amministratore o privilegio di backup legge anche le cartelle negate [V S21].
- Free: analisi, mappa 2D/3D, filtri, compressione NTFS, PDF. Mancano file grandi, duplicati, CSV, confronto snapshot e CLI. Solo uso non commerciale; niente Windows Server dalla 4.7.3 [V S17 S18 S20].
- Personal (20,40 € per utente all'anno): duplicati con checksum, annullamento di cancellazioni e spostamenti, confronto snapshot. Professional (da 41 €): pianificazione, CLI, cloud (SharePoint, Google Drive, S3, Azure) [V S17].
- 9.8 (12/06/2026): anteprima prima delle operazioni sui duplicati. 9.9: selezione dei duplicati a regole e chatbot "JAMie" nell'app [V S16].
- La Free 4.8 (06/10/2025) raccoglie dati d'uso anonimi, disattivabili [V S18], mentre la pagina del prodotto dice "No data ever leaves your PC" [P S20].

### 2.4 SpaceSniffer

Fermo dal 2016, ripreso nel 2025; attuale 2.2.0.27 del 25/03/2026, freeware chiuso, solo 64 bit, portable [V S22 S23 S24]. È il riferimento per la mappa navigabile "dal vivo" con zoom, ma non ha attribuzione, duplicati o export strutturati.

### 2.5 Everything

- Indici NTFS aggiornati con l'USN journal; per NTFS servono il servizio di Everything o l'esecuzione come amministratore [V S25]. Supporta ReFS via USN; FAT, exFAT e rete come cartelle indicizzate, con monitoraggio "imperfect" [V S25 S33].
- Circa 100 MB di RAM per un milione di file [P S25].
- La 1.5 (beta dal 15/05/2026) indicizza le dimensioni delle cartelle di default [V S30]. L'SDK3 (MIT, named pipe) offre `Everything3_GetFolderSizeFromFilename`; Everything deve essere in esecuzione [V S29].
- Il modello "servizio privilegiato più interfaccia non elevata" è quello giusto per evitare l'UAC a ogni avvio [I]. Nota di revisione: vale per un prodotto che serve spesso i privilegi. Per uno strumento personale 05 D3 sceglie invece un helper elevato avviato su richiesta, senza un servizio installato: l'UAC compare solo quando si chiede il completamento con privilegi (F-SC-3).

### 2.6 Windows

- Impostazioni › Archiviazione: categorie come App installate, File temporanei, Sistema e riservato (visibile con "Mostra più categorie") [V S35]. "Altro" raccoglie le cartelle fuori dalle categorie note [V S46].
- Le dimensioni di "App installate" vengono da `EstimatedSize`; per le app senza dimensione dichiarata lo Storage Service fa una stima e aggiorna la registrazione [V S41]. Casi documentati: 10,3 GB mostrati per un'app da circa 300 MB [V S42]; 185 GB di "App installate" senza spiegazione (Q&A Microsoft, 07/05/2026) [V S40].
- Bug di luglio 2026: un file `CapabilityAccessManager.db-wal` arrivato a 70-80 GB falsava "App installate" e "Sistema e riservato", poi corretto con KB5095093 [V S39].
- Sensore memoria lavora solo sull'unità di sistema; pulisce file temporanei e Cestino per età; rende "solo online" i file OneDrive non aperti da 30 giorni senza eliminarli [V S36]. Pulizia disco è mantenuta in Windows (elenco delle funzioni deprecate aggiornato il 23/09/2026) [V S38]. Quado rimanda prima a Impostazioni › Archiviazione › File temporanei e cita Pulizia disco come alternativa.
- `DISM /AnalyzeComponentStore` distingue la dimensione riportata da Esplora da quella reale di WinSxS, perché Esplora non tiene conto degli hard link [V S37]. È l'esempio canonico della spiegazione che gli analizzatori non danno.
- Microsoft PC Manager (3.22.6, 02/10/2026) fa pulizia profonda di cache di app e browser; criticato per la spinta verso Edge e Bing [V S43 S44].

## 3. Altri strumenti rilevanti

| Strumento | Perché conta | Fatti principali |
|---|---|---|
| MangoDisk | Il più vicino alla nostra idea | Rust + Tauri, GPL-3.0, creato il 01/08/2026, v1.1.7, circa 4.000 stelle. Pulizia per categorie, disinstallazione con residui, duplicati, mappa. Regole TOML pubbliche con livello di rischio; distingue il permesso di lettura da quello di cancellazione; inventari incompleti bloccano l'esecuzione. "AI Explanations" tramite servizio cloud; modulo di telemetria legato agli aggiornamenti; perimetro da "ottimizzatore di PC" [V S55 S56] |
| SizeTrend | Il concorrente commerciale più vicino sull'attribuzione | Legge i metadati di NTFS, FAT32 ed exFAT senza elevazione; panoramica che riconosce grandi app e giochi, Download vecchi, component store, cache dei browser, Teams/Outlook, VM, repository Git; rimanda agli strumenti nativi invece di cancellare [P S57] |
| SquirrelDisk 2 | Riferimento di interfaccia open source | Riscritto in Rust con egui, AGPL-3.0, v2.5.1 (12/10/2026). File solo-cloud contati zero, cartelle collegate contate una volta; aggiornamento automatico e banner sponsor; niente MFT [V S54] |
| diskplot | Onestà sui propri limiti | MIT, ottobre 2026. Mappa e albero, snapshot "cosa è cresciuto", riconosce node_modules e cache, cancella solo nel Cestino. Dichiara che conta gli hard link per nome e quindi gonfia WinSxS [V S58] |
| sizetree, mathom, DiskRaptor, Drive Sonar | Mostrano che "MFT + mappa in Rust" si replica in poche settimane | MIT o Apache-2.0, adozione minima [V S60 S61] |
| Bulk Crap Uninstaller | Logica di rilevamento app e residui | Apache-2.0, C#, v6.3; Store, Steam, NSIS, Inno, MSI; cerca maintainer [V S84] |
| Czkawka (core) / Krokiet | Duplicati | Core MIT; Krokiet GPL-3.0 per i vincoli di Slint; "does not have access to the Internet" [V S62] |
| BleachBit, CCleaner | Pulizia a regole; esempio negativo di privacy | BleachBit GPL-3.0 attivo [V S65]; CCleaner con storia di raccolta dati non disattivabile [V S66] |
| Kondo, npkill | Artefatti di sviluppo | MIT; Kondo è "essentially rm -rf with a prompt" [V S77 S78] |
| Sysinternals du, dua-cli, dust, gdu, pdu, ncdu | Riferimenti a riga di comando | pdu è l'unico con benchmark pubblici generati in CI [V S69 S71 S72 S73 S74 S76] |
| DaisyDisk (macOS) | Riferimento di spiegazione | Spiega lo spazio che manca "without an obvious reason", scompone "Other" e "System Data", trova gli snapshot [V S82] |

## 4. Benchmark: cosa esiste davvero

Non esiste un confronto indipendente e riproducibile tra WizTree, WinDirStat 2.x, TreeSize ed Everything con hardware e numero di file dichiarati [I]. Lo "46x" di WizTree non ha metodologia [P S1]; l'unico confronto pubblico è scritto da un concorrente [S59]; diskplot dichiara 2,15 milioni di file in 10 s a caldo [P S58]; pdu è l'unico con un workflow di benchmark pubblico [V S74]. Pubblicare una suite riproducibile (cache calda e fredda, con e senza amministratore, numero di file, hardware) sarebbe già un elemento di credibilità [I]. Per uno strumento personale i benchmark pubblicati sono stati tolti dalla v1 (`revisione/decisioni-revisione.md` §1); le misure di prestazione che servono a Quado stanno in 06.

Per avere un riferimento nostro ho misurato sul PC di sviluppo l'enumerazione non elevata con una sonda in Rust: quasi 3 milioni di file in 16-26 s (02 §2.1). Non è un confronto con i concorrenti, che non ho eseguito.

## 5. Chi è più forte e per cosa

| Bisogno | Riferimento attuale | Nota |
|---|---|---|
| Scansione NTFS più veloce e semplice | WizTree | Solo da amministratore, commerciale a pagamento |
| Analizzatore gratuito, aperto e completo | WinDirStat 2.9 | GPL-3+, nessuna rete, molte funzioni per esperti |
| Reportistica aziendale e cloud | TreeSize Professional | Abbonamento |
| Indice persistente e ricerche istantanee | Everything 1.5 | Nessuna visualizzazione |
| Mappa dal vivo | SpaceSniffer | Chiuso, perimetro ridotto |
| Pulizia a regole con spiegazioni | MangoDisk | GPL-3, AI nel cloud, perimetro da ottimizzatore |
| Panoramica per categorie e crescita | SizeTrend, TreeSize Personal/Pro | Commerciali |
| Duplicati | Czkawka, AllDup, WinDirStat | Core di Czkawka MIT |
| Spiegazione dello spazio nascosto | DaisyDisk (macOS) | Nessun equivalente maturo su Windows [I] |

## 6. Cosa sarebbe solo una duplicazione

- Scansione MFT veloce con ripiego sulle API.
- Mappa ad albero, sunburst, flame graph, statistiche per estensione, file più grandi.
- Duplicati via hash.
- Logico contro allocato, hard link, flussi alternativi.
- Cancellazione con Cestino o definitiva e conferma.
- Export CSV, JSON, SQLite e riga di comando.
- Tema scuro, ARM64, winget e Store.
- Ricerca istantanea su indice (Everything).
- Confronto tra snapshot (TreeSize, SizeTrend, diskplot).

Alcune di queste servono comunque al prodotto (mappa, elenco, hard link, Cestino): vanno fatte bene, ma non sono il motivo per cui qualcuno sceglierebbe Quado.

## 7. Lacune che restano

1. Perché è grande. Nessun analizzatore diffuso dice cos'è una cartella, chi la usa, se si ricrea, cosa si perde rimuovendola. Gli utenti chiedono senza risposta [V S40 S48]. MangoDisk lo fa con regole e AI nel cloud [S55], SizeTrend per categorie [S57].
2. Attribuzione alle applicazioni con un grado di confidenza. Le dimensioni di Impostazioni sono stime [S41]; gli analizzatori attribuiscono per cartella o per proprietario NTFS [S8 S17]; il journal USN non contiene l'identità del processo [V S52].
3. Bilancio dello spazio. WinDirStat mescola nell'`<Unknown>` il risparmio degli hard link e lo spazio davvero ignoto [V S13]; WizTree mostra solo lo spazio riservato [S2]. Le cause sono documentate ma disperse: copie shadow, ibernazione e paging, WinSxS, VHDX di WSL e Docker, archiviazione riservata, bug di sistema [S37 S39 S49 S51 S100]. Sul PC di sviluppo la sonda conferma la scala: 13,6 GB di hard link e 31,9 GB di differenza tra usato e file letti su più di 700 GB usati, prima di sottrarre `$MFT` e indici (unità 10^9). La definizione del non attribuito è in 03 §6.4.
4. Incertezza dichiarata nell'interfaccia. Nessuno dice, accanto a ogni numero, quante cartelle non sono state lette, quali segnaposto cloud sono esclusi, come sono contati gli hard link [I]. diskplot lo scrive solo nel README [S58].
5. Pulizia spiegata e reversibile. Rischi reali documentati: idratazione di OneDrive [S12], Prefetch cancellato da PC Manager [S99], strumenti "rm -rf con conferma" [S77]. Anteprima con stima, Cestino e registro esistono solo in TreeSize a pagamento [S17] e in MangoDisk [S55].
6. Privacy e licenza insieme. WizTree, TreeSize Free, MangoDisk e SquirrelDisk usano la rete [S3 S18 S55 S54]; WizTree, TreeSize Free e UltraSearch Free vietano l'uso commerciale [S1 S20 S68]. WinDirStat è senza rete ma GPL-3.

## 8. L'opportunità

Un analizzatore che spiega, per Windows, locale e senza rete, che non compete sulla velocità di scansione ma su tre risultati che oggi nessuno mette insieme:

1. Il bilancio del volume: usato = file attribuiti + overhead di sistema esplicitato + residuo non spiegato, con fonte e confidenza per voce.
2. L'attribuzione a programmi e "proprietari logici" da prove verificabili (registro, pacchetti MSIX, manifest dei launcher, file di configurazione noti delle app), sempre etichettata come da fonte, dedotta o sconosciuta.
3. Interventi spiegati e reversibili: cos'è, chi lo ricrea, cosa si perde, procedura ufficiale quando esiste, altrimenti Cestino con simulazione.

Rispetto ai più vicini:
- MangoDisk: niente AI nel cloud, niente "ottimizzatore", più profondità su Windows, licenza non GPL.
- SizeTrend: regole e confidenza trasparenti.
- WinDirStat: pensato per chi non è esperto, con spiegazioni al posto di un catalogo di strumenti.

Pubblico più ricettivo: sviluppatori e utenti esperti, dove VHDX, cache ed emulatori fanno sparire decine di GB [V u S51]. Sul PC di sviluppo ci sono una decina di GiB di dispositivi virtuali Android e alcuni GiB tra cache npm e store pnpm. L'utente comune resta il pubblico di riferimento per il linguaggio.

## 9. Sviluppare, riusare o partire da una base esistente

| Base possibile | Licenza | Valutazione |
|---|---|---|
| WinDirStat | GPL-3.0-or-later | Obbliga tutto il prodotto alla GPL-3; C++/Win32; upstream molto attivo, un fork divergerebbe. Si studia, non si incorpora |
| SquirrelDisk | AGPL-3.0-only | Copyleft forte |
| MangoDisk | GPL-3.0 | Anche le regole sono GPL. I fatti sui percorsi si ricercano in modo indipendente, senza copiare la raccolta |
| Krokiet | GPL-3.0-only | Il core Czkawka invece è MIT |
| sizetree, mathom, DiskRaptor, diskplot, Drive Sonar | MIT / Apache-2.0 | Troppo giovani e poco adottati per farci da base; utili come confronto |

Mattoni permissivi disponibili:

| Crate o libreria | Licenza | Stato | Uso possibile |
|---|---|---|---|
| `windows` / `windows-sys` | MIT o Apache-2.0 | 0.62 / 0.61, Microsoft | Tutte le API Win32 |
| `ntfs` (Colin Finck) | MIT o Apache-2.0 | 0.4.0 del 2023, repository vivo | Parser NTFS completo: oracolo per un lettore MFT, che non c'è più nella v1 |
| `mft` (omerbenamram) | MIT o Apache-2.0 | 0.7.0, 12/2025 | Parser forense di file `$MFT`: secondo oracolo, solo se il lettore MFT tornerà nella v2 |
| `ntfs-reader` | MIT o Apache-2.0 | 0.6.0, 01/10/2026, 22 stelle | Lettura MFT e USN; bus factor basso |
| `usn-journal-rs` | MIT | 0.4.1, 05/2026 | USN, per la v2 |
| `jwalk`, `ignore` | MIT / Unlicense | 2026 | Walk paralleli; probabilmente non servono, vedi 05 |
| `trash` | MIT | 5.2.9, 09/2026 | Cestino multipiattaforma; su Windows preferiamo `IFileOperation` diretto per il controllo sull'eliminazione definitiva (05) |
| Everything SDK3 | MIT | 2026 | Integrazione opzionale futura |
| Bulk Crap Uninstaller | Apache-2.0 | v6.3 | Logica di rilevamento app da studiare |

Conclusione: core scritto da zero in Rust con dipendenze permissive, nessun codice o regola copiata da progetti GPL. Nella v1 non c'è un lettore MFT: l'helper con privilegi enumera solo le cartelle negate, con lo stesso codice dello scanner; MFT e oracoli passano alla v2, solo se serviranno (`revisione/decisioni-revisione.md` §8). Il valore si concentra nello strato di attribuzione e spiegazione e nella base di regole, scritta e verificata da noi. La licenza diventa una domanda solo se Quado verrà pubblicato (00).

## 10. Registro delle fonti

- S1 https://diskanalyzer.com/
- S2 https://diskanalyzer.com/whats-new
- S3 https://diskanalyzer.com/faq
- S4 https://diskanalyzer.com/donate
- S5 https://diskanalyzer.com/about
- S6 https://diskanalyzer.com/privacy-policy
- S7 https://malwaretips.com/threads/wiztree-3-36-does-it-phone-home.106905/
- S8 https://github.com/windirstat/windirstat
- S9 https://github.com/windirstat/windirstat/blob/master/CHANGELOG.md
- S10 https://github.com/windirstat/windirstat/releases
- S11 https://github.com/windirstat/windirstat/issues/547
- S12 https://github.com/windirstat/windirstat/issues/416
- S13 https://github.com/windirstat/windirstat/issues/448
- S14 https://github.com/windirstat/windirstat/blob/master/windirstat/FinderBasic.cpp e FinderNtfs.cpp
- S15 https://github.com/windirstat/windirstat/blob/master/windirstat/res/langs/lang_en.txt
- S16 https://www.jam-software.com/treesize/changes.shtml
- S17 https://jam-software.com/treesize_free/comparison.shtml
- S18 https://www.jam-software.com/treesize_free/changes.shtml
- S19 https://www.jam-software.com/treesize/features.shtml
- S20 https://www.jam-software.com/treesize_free
- S21 https://manuals.jam-software.com/treesize/EN/notesonntfs.html
- S22 https://www.uderzo.it/main_products/space_sniffer/index.html
- S23 https://en.wikipedia.org/wiki/SpaceSniffer
- S24 https://www.snapfiles.com/apphistory/spacesniffer_history.html
- S25 https://www.voidtools.com/faq/
- S26 https://www.voidtools.com/forum/viewtopic.php?t=9787
- S27 https://www.voidtools.com/downloads/
- S28 https://www.voidtools.com/License.txt
- S29 https://github.com/voidtools/everything_sdk3
- S30 https://www.voidtools.com/forum/viewtopic.php?p=75613
- S31 https://www.voidtools.com/support/everything/sdk/
- S32 https://mail.voidtools.com/forum/viewtopic.php?p=27734
- S33 https://voidtools.com/forum/viewtopic.php?p=77970
- S34 https://voidtools.com/forum/viewtopic.php?p=49753
- S35 https://support.microsoft.com/help/4027321/
- S36 https://support.microsoft.com/en-us/windows/manage-drive-space-with-storage-sense-654f6ada-7bfc-45e5-966b-e24aded96ad5
- S37 https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/determine-the-actual-size-of-the-winsxs-folder
- S38 https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features
- S39 https://www.guru3d.com/story/microsoft-fixes-windows-11-bug-that-could-consume-up-to-80gb-of-storage/
- S40 https://learn.microsoft.com/en-us/answers/questions/5884022/storage-issue-installed-apps-is-reporting-185gb-of
- S41 https://devblogs.microsoft.com/oldnewthing/20180522-00/?p=98805
- S42 https://techsupport.screenplay.com/hc/en-us/articles/35876045097869
- S43 https://en.wikipedia.org/wiki/Microsoft_PC_Manager
- S44 https://pcmanager.microsoft.com/en-US/termsofservice
- S45 https://winaero.com/use-windows-11-cleanup-recommendations-to-clean-system-drive
- S46 https://www.thurrott.com/books/windows-11-field-guide/hardware-windows-11-field-guide/338893/storage-2
- S47 https://answers.microsoft.com/en-us/windows/forum/all/what-the-heck-is-other-people-using-105-gb-storage/2ffde7d5-c8a7-4541-9eb7-d50be44e2cd3
- S48 https://www.lifehacker.jp/article/2610-windows-told-me-my-drive-was-full-but-this-free-tool-showed-me-what-was-actually-eating-it/
- S49 https://learn.microsoft.com/en-us/windows/wsl/disk-space
- S50 https://devblogs.microsoft.com/commandline/windows-subsystem-for-linux-september-2023-update/
- S51 https://forums.docker.com/t/docker-desktop-wsl2-vhdx-grows-by-1-5gb-after-every-update-no-way-to-reclaim-space/151729
- S52 https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-usn_record_v2
- S53 https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
- S54 https://github.com/adileo/squirreldisk
- S55 https://github.com/harry0703/MangoDisk
- S56 https://github.com/harry0703/MangoDisk/tree/main/src-tauri/crates/mangodisk-core/rules
- S57 https://dimastr.com/sizetrend/
- S58 https://github.com/rodrigokiller/diskplot
- S59 https://dev.to/barun_saha/windirstat-vs-wiztree-vs-drive-sonar-which-free-disk-space-analyzer-should-you-use-in-2026-56lo
- S60 https://github.com/barun-saha/drive-sonar
- S61 https://github.com/TonyWoo/sizetree · https://github.com/gitRasheed/mathom · https://github.com/SunMe1977/DiskRaptor
- S62 https://github.com/qarmin/czkawka
- S63 https://github.com/arsenetar/dupeguru
- S64 https://alldup.de/en/alldup/version-history.php
- S65 https://github.com/bleachbit/bleachbit
- S66 https://en.wikipedia.org/wiki/CCleaner
- S68 https://www.jam-software.com/ultrasearch
- S69 https://learn.microsoft.com/en-us/sysinternals/downloads/du
- S71 https://github.com/Byron/dua-cli
- S72 https://github.com/bootandy/dust
- S73 https://github.com/dundee/gdu
- S74 https://github.com/KSXGitHub/parallel-disk-usage
- S76 https://dev.yorhel.nl/ncdu/changes2
- S77 https://github.com/tbillington/kondo
- S78 https://github.com/voidcosmos/npkill
- S82 https://daisydiskapp.com/
- S84 https://github.com/Klocman/Bulk-Crap-Uninstaller
- S99 https://hothardware.com/news/microsoft-pc-manager-app-may-users-on-edge
- S100 https://www.bleepingcomputer.com/forums/t/581736/60gb-unknown-file-in-windirstat/
- S104 https://github.com/windirstat/windirstat/issues

Da ricontrollare prima di citare all'esterno: i prezzi di TreeSize (la pagina mostra tariffe annuali, listini terzi anche perpetue), il numero di thread di TreeSize (32 o 64 secondo la pagina), lo stato di PC Manager nell'UE. Le pagine di changelog di SpaceSniffer su Neowin e MalwareTips hanno risposto 403.
