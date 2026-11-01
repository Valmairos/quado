# 03 · Specifiche di prodotto (v2, dopo la revisione)

Stato: v2, riscritta dopo la revisione critica del 12/10/2026 (`10-revisione-critica.md`) secondo `revisione/decisioni-revisione.md`, che prevale in caso di dubbio. La v1 di questo documento non è inclusa. Gli ID dei requisiti sono quelli dell'elenco canonico (`revisione/decisioni-revisione.md` §9 ter). I numeri misurati compaiono solo quando servono a definire un criterio: sono misure prese sul PC di sviluppo, in sola lettura, con la data, mentre i test usano fixture versionate.

Cosa cambia rispetto alla v1:

| Tema | v1 | v2 |
|---|---|---|
| Raccomandazioni | Quattro attributi (gestore, tipo, fonte, gestione), nessuno stato intermedio | Cinque stati e scheda obbligatoria; prova di identità e prova di gestibilità distinte (§6.1) |
| Affidabilità dei fatti dedotti | Tre livelli senza criteri | Criteri per alta, media e bassa; colore del gestore solo da fatti "da fonte" o "dedotto alta" (§6.2) |
| Stima | "Subito e dopo il Cestino", senza metodo | Rilettura degli attributi in simulazione; stati esatta, intervallo, non stimabile (§6.3) |
| Verifica del risultato | Assente | Requisiti F-VE |
| Bilancio | "Somma uguale alla capacità al byte", vero per costruzione | Definizione formale, deriva come incertezza, test su VHDX con errore iniettato (§6.4) |
| Requisiti | 39 | 57, con gli ID canonici; nuovi gruppi F-DL e F-VE, nuovi F-AP-4..6, F-SY-3, F-AC-7..9, F-IN-6, F-EX-5 |
| Modalità con privilegi | Lettura della MFT | Helper che enumera solo le cartelle negate (F-SC-3) |
| Perimetro | MSI firmato, italiano e inglese, modalità portable, palette dei comandi | Eseguibile compilato in locale, italiano e inglese con altre lingue aggiungibili (05 D13, deciso il 13/10/2026), una sola sede dei dati, palette nella v1.1 (§8) |
| Requisiti non funzionali | Obiettivi scritti come dati (avvio in 1 s, Panoramica in meno di 30 s) | Obiettivi marcati come tali, ciascuno con il metodo di misura (§7) |

## 1. Il problema

Un SSD si riempie e chi lo usa non capisce da dove venga il consumo, né cosa sia sensato fare. Gli strumenti esistenti rispondono bene alla prima domanda, cosa occupa spazio: WizTree, gratuito per uso personale, e le Impostazioni di Windows lo fanno già (01). Le domande successive (cos'è, si può gestire, come si interviene senza danni, quanto spazio è tornato) restano senza risposta, a meno di mandare dati a un servizio AI nel cloud (`revisione/decisioni-revisione.md` §1). Quado serve a quella parte.

La catena da servire, in ordine: Individuare › Comprendere › Valutare › Decidere › Agire in sicurezza › Verificare.

| # | Anello | Domanda dell'utente | Cosa fa Quado | Requisiti |
|---|---|---|---|---|
| 1 | Individuare | Cosa occupa spazio, e dove? | Scansione senza privilegi, Panoramica, Esplora, File grandi | F-SC, F-OV-2, F-EX |
| 2 | Comprendere | Cos'è, a chi appartiene, perché è così grande? | Fatti con provenienza, regole, lente Download, Programmi, vista Windows e sistema, bilancio | F-IN, F-DL-1..3, F-AP-1..3, F-SY, F-OV-1, F-OV-3 |
| 3 | Valutare | Si può gestire? Con quale metodo, rischio e stima? | Record di raccomandazione con uno di cinque stati | §6.1-§6.3, F-IN-4, F-OV-4 |
| 4 | Decidere | Cosa scelgo, sapendo cosa è dedotto? | Strumenti per decidere (confronti, corrispondenze, uso noto) e scheda completa nella conferma | F-DL-2, F-DL-4, F-DL-5, F-AP-4, F-AC-3 |
| 5 | Agire in sicurezza | Come intervengo senza rompere niente? | Simulazione, controlli, solo Cestino o procedure ufficiali | F-AC, F-AP-5 |
| 6 | Verificare | Quanto spazio è tornato davvero? | Misure prima e dopo, stimato contro ottenuto, registro | F-VE, F-AC-6 |

Quado è nato come strumento personale, per un PC con Windows 10 22H2 oggi e probabilmente Windows 11 in futuro. Che altri lo usino è un vantaggio, non un obiettivo. Il riferimento per il linguaggio è invece una persona non esperta: ogni testo deve essere comprensibile da chi conosce Esplora file e non conosce NTFS. La prova è il test di usabilità a compiti (06 §8).

Un esempio misurato sul PC di sviluppo, con le dimensioni in 09 §8. `AppData\Local\CapCut` è una cartella grande. Un analizzatore classico mostra cartelle e sottocartelle. Per capire cosa contiene bisogna incrociare la voce di disinstallazione di CapCut in HKCU con il file `Apps\ProductInfo.xml`: insieme dicono quale versione è in uso ("da fonte", 02 §10). Accanto ci sono altre cartelle di versione e, in `User Data\Projects`, i progetti dell'utente. Che le versioni precedenti non servano più resta una deduzione: per questo l'intervento ha stato Da verificare, tiene le ultime due versioni e passa da controlli dedicati (§6.1). È il lavoro che Quado fa al posto dell'utente, dicendo da dove viene ogni informazione.

## 2. Utenti

| Profilo | Ruolo | Situazione | Cosa gli serve |
|---|---|---|---|
| Sviluppatore che gioca e monta video | Utente principale. Il perimetro e i criteri soggettivi, come la resa del testo in M0, li decide e li approva il proprietario del progetto | Sviluppo software (Rust, npm, pnpm, Android SDK, Visual Studio), giochi Steam ed Epic, Unreal Engine, video con CapCut | Attribuzione ai programmi, procedure ufficiali di launcher, app e strumenti di sviluppo, stime con un metodo, verifica del risultato, dettagli tecnici raggiungibili dall'ispettore |
| Persona non esperta | Riferimento per il linguaggio; partecipante facoltativo al test di usabilità | Conosce Esplora file e poco altro; apre Quado quando Windows avvisa che il disco è pieno | Testi senza gergo, stati che non si fraintendono, nessuna azione che richieda di capire NTFS |

Fuori obiettivo: amministratori di più PC o di server, report programmati, scansione di massa di condivisioni di rete, memoria RAM.

## 3. Le domande e dove trovano risposta

### 3.1 Le dieci domande del brief iniziale

| # | Domanda | Dove trova risposta | Requisiti |
|---|---|---|---|
| 1 | Cosa occupa più spazio? | Panoramica ("Dove finisce lo spazio"), Esplora, File grandi | F-OV-2, F-EX-1, F-EX-2, F-EX-5 |
| 2 | Quali app, categorie o cartelle sono responsabili? | Bilancio per gestore, Programmi, ispettore | F-OV-1, F-AP-1, F-AP-2, F-AP-3, F-IN-1 |
| 3 | Perché certe cartelle sono così grandi? | Ispettore, blocco "Perché è così grande" | F-IN-2 |
| 4 | Cosa c'è dentro, senza aprire i documenti? | Ripartizione per tipo secondo l'estensione, lente Download | F-IN-3, F-DL-1 |
| 5 | Cosa è essenziale, protetto, usato da altre app, sconosciuto? | Fatti con provenienza, stati, elementi protetti, residui | F-IN-1, F-IN-4, F-AC-4, F-AP-6, §6.1 |
| 6 | Cosa si può gestire o rimuovere, e come? | "Cosa puoi fare", card interventi, tipi di intervento, disinstallazione ufficiale | F-IN-4, F-OV-4, F-AC-1, F-AP-5 |
| 7 | Quanto spazio recupera ogni intervento? | Stima in simulazione, poi stimato contro ottenuto | F-AC-2, F-VE-3, §6.3 |
| 8 | Perché lo spazio libero non torna con la somma dei file? | Bilancio, card dei conti, Cestino misurato in due modi, completamento con privilegi | F-OV-1, F-OV-3, F-SY-2, F-SC-3, §6.4 |
| 9 | Cosa occupa spazio in modo poco evidente? | Vista Windows e sistema; hard link, compressione, segnaposto cloud, dimensioni non aggiornate | F-SY-1, F-SC-7, F-SC-8, F-SC-9, F-IN-6 |
| 10 | Come decidere senza competenze avanzate? | Cinque stati con scheda, strumenti per decidere, conferma completa, testi per non esperti | §6.1, F-IN-4, F-DL-4, F-DL-5, F-AC-3, F-GE-1 |

### 3.2 Le otto domande del brief di revisione

| # | Domanda | Dove trova risposta | Requisiti |
|---|---|---|---|
| 1 | Cosa occupa spazio? | Panoramica, Esplora, File grandi | F-OV-1, F-OV-2, F-EX-1, F-EX-2, F-EX-5 |
| 2 | Dove? | Barra del percorso, mappa ed elenco sincronizzati, ricerca e filtri | F-EX-1..4 |
| 3 | A cosa appartiene, e perché lo sappiamo? | Fatti con provenienza e affidabilità, Programmi, regola visibile | F-IN-1, F-IN-2, F-AP-1, F-AP-2, §6.1, §6.2 |
| 4 | Cosa è gestibile? | Stato del record di raccomandazione | F-IN-4, F-OV-4, §6.1 |
| 5 | Quali interventi recuperano spazio, e quanto? | Card interventi con stima e stato; copie identiche, download incompleti | F-OV-4, F-AC-2, F-DL-4, F-DL-6, §6.3 |
| 6 | Cosa lasciare intatto? | Elementi protetti, "Lo gestisce X" e la variante "Nessuna procedura: non toccare", Non sappiamo, residui | F-AC-4, F-SY-1, F-AP-6, §6.1 |
| 7 | Cosa richiede verifiche? | Stato Da verificare con i suoi controlli; capacità del Cestino, file in uso, riferimenti | F-AC-7, F-AC-8, F-AC-9, F-DL-4, §6.1 |
| 8 | Come agire, e come verificare il risultato? | Simulazione, conferma, Cestino o procedure ufficiali, misure prima e dopo, registro | F-AC-1..6, F-VE-1..5 |

## 4. Scenari

Elenco breve. Per ogni scenario la matrice completa (esigenza, informazioni necessarie, metodo, limiti, comportamento, raccomandazione, protezioni, verifica) è in `09-matrice-scenari.md`.

| Scenario | Comportamento atteso in breve | Requisiti principali |
|---|---|---|
| Disco pieno: cosa occupa? Primo avvio, scansione di C: | Panoramica con bilancio, "Dove finisce lo spazio" e card dei conti; obiettivo di tempo in §7 | F-SC-1..6, F-OV-1..3 |
| AppData è grande: perché? | Esplora su `AppData\Local`; l'ispettore separa versioni, cache e tuoi file, con la provenienza di ogni fatto | F-EX, F-IN-1, F-IN-2 |
| I conti non tornano | Card dei conti con le liste "Osservato" e "Possibile"; Cestino orfano come anomalia; completamento con privilegi su richiesta | F-OV-1, F-OV-3, F-SY-2, F-SC-3 |
| Cosa tengo in Download? | Famiglie, metadati degli installatori, copie identiche dopo l'hash, archivi già estratti, download incompleti | F-DL |
| Un programma grande: lo uso ancora? | Spazio con prova del percorso, uso noto solo da fonti affidabili, disinstallazione dall'interfaccia ufficiale | F-AP-1..5 |
| Residui di programmi disinstallati | Stato Non sappiamo, data di ultima modifica, nessuna proposta | F-AP-6 |
| Giochi Steam ed Epic | Spazio dai manifest, ultima sessione registrata da Steam, disinstallazione dal launcher | F-AP |
| Cache e progetti di sviluppo | npm e pnpm: Lo gestisce npm o pnpm; `node_modules` e `target`: Decidi tu con controlli; Docker e WSL: 0 GB su C: fino alla compattazione | F-IN-4, F-AC-1, §6.1, §6.3 |
| File di Windows: ibernazione, WinSxS, file temporanei | Stato e procedura ufficiale per ogni voce; comandi mostrati e copiati | F-SY-1, F-SY-3 |
| Cartella sconosciuta | Solo misure e fatti; stato Non sappiamo | F-IN-1, F-IN-3, §6.1 |
| Liberare spazio senza rompere niente, e verificare | Simulazione, controlli del Cestino, Restart Manager, riferimenti, misure prima e dopo | F-AC, F-VE |
| Disco esterno exFAT o chiavetta | Scansione funzionante; la UI dice cosa manca su quel file system (hard link, compressione, Cestino) | F-SC-10, F-AC-1 |

## 5. Requisiti funzionali v1

Ogni riga ha il requisito, un criterio di accettazione verificabile e il metodo di verifica, con il rimando alla sezione di `06-piano-di-test.md`. Termini usati nei criteri:
- "albero di prova": struttura sintetica generata da `xtask` su NTFS (06 §1.2);
- "oracolo": secondo percorso di misura che apre ogni file con soli attributi (06 §1.1);
- "VHDX": volume virtuale locale creato e montato con il consenso di chi esegue la prova (06 §1.3);
- "fixture": dati reali del PC di sviluppo, anonimizzati e versionati (06 §2.3);
- "controllo manuale con data": confronto con il PC reale, registrato con la data della misura, non test di regressione.

Le soglie di prestazione sono obiettivi, con il metodo di misura in §7 e in 06 §9. La distribuzione dei requisiti nelle milestone è in `revisione/decisioni-revisione.md` §9 ter e in 07.

### 5.1 Scansione (F-SC)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-SC-1 | Elenco dei volumi: lettera, etichetta, file system, tipo di unità (fissa, rimovibile, di rete), capacità e spazio libero. Il volume di sistema è il primo. | Con un VHDX NTFS e uno exFAT montati, lettera, etichetta, file system, tipo e capacità di ogni volume coincidono con `Get-Volume` nella stessa sessione. | Controllo manuale con data (06 §1.3) |
| F-SC-2 | Scansione non elevata in sola lettura di un volume o di una cartella, con `GetFileInformationByHandleEx` e `FileIdExtdDirectoryInfo` (05 D4). Nessun file si apre per leggerne il contenuto; si aprono con soli attributi solo i file da correggere (F-SC-8). | Sugli alberi di prova i totali logici e allocati coincidono al byte con l'oracolo. Durante la scansione nessun handle ha accesso in scrittura e nessun `ReadFile` tocca file dell'utente. Tempi e memoria come in §7. | Alberi di prova contro l'oracolo (06 §1.1, §1.2); traccia ETW `FileIo` o Process Monitor sulla maschera di accesso (06 §11); protocollo di prestazioni (06 §9) |
| F-SC-3 | Completamento con privilegi su richiesta, dal pulsante "Ripeti come amministratore" della card dei conti. Un helper elevato (lo stesso eseguibile) abilita `SeBackupPrivilege` ed enumera con `FILE_FLAG_BACKUP_SEMANTICS` solo le cartelle trovate negate, con lo stesso codice dello scanner. Raccoglie lo spazio delle copie shadow, lo stato dell'archiviazione riservata e i cestini di altri SID; restituisce i profili altrui in forma aggregata; termina a fine lavoro. La UI non è mai elevata. | Su un VHDX preparato i totali delle cartelle negate coincidono al byte con l'oracolo eseguito come amministratore. I profili altrui arrivano solo come totale e numero di file: la traccia dei messaggi sulla pipe non contiene nomi. I cestini di altri SID arrivano come totale per SID, con il SID orfano segnato. Copie shadow e archiviazione riservata: valore letto oppure "non disponibile" con il motivo, senza errori. Il token della UI resta non elevato; dopo l'ultimo messaggio il processo dell'helper non esiste più. | VHDX con il consenso di chi esegue la prova (06 §1.5); test di sicurezza dell'helper (06 §11) |
| F-SC-4 | Avanzamento dal vivo: file letti, cartelle, tempo trascorso, totali delle cartelle di primo livello che crescono durante la scansione. | Almeno 4 aggiornamenti al secondo; tempo di frame della UI p95 ≤ 16 ms durante la scansione (obiettivi, §7). | Tempi di frame strumentati in build di test (06 §9) |
| F-SC-5 | Annullamento in qualunque momento. Il risultato parziale resta visibile ed è marcato come parziale. | Arresto entro 500 ms dalla richiesta (obiettivo, §7). Ogni schermata con dati parziali mostra il banner "Scansione interrotta". | Albero di prova e protocollo di prestazioni (06 §1.2, §9) |
| F-SC-6 | Cartelle negate elencate con percorso e motivo (accesso negato, altro errore, in uso) e contate a fine scansione. Il loro contenuto resta nel non attribuito e non si mostra mai come zero. | Su un albero di prova con un'ACL di negazione la cartella compare nell'elenco con il motivo "accesso negato", il suo contenuto non entra nell'allocato letto e la card dei conti la conta nella lista "Osservato". | Albero di prova (06 §1.2); test della Panoramica (06 §1.4) |
| F-SC-7 | Hard link contati una volta per volume, per identità (numero di serie del volume, ID file a 128 bit). Junction, collegamenti simbolici e punti di montaggio (tag di reparse con il bit "name surrogate") non si seguono e sono indicati come tali; un volume montato in una cartella si scansiona a parte. | Su un albero di prova con hard link tra cartelle lontane, una junction che crea un ciclo e un symlink: totale uguale all'oracolo, nessun ciclo, junction e symlink indicati. Nella card dei conti gli hard link compaiono come nota "già contati una volta". | Albero di prova (06 §1.2) |
| F-SC-8 | Dimensione logica ("Dimensioni") e allocata ("Dimensioni su disco") per ogni file e cartella; la vista usa l'allocata per default. I file con logica > 0, allocata 0 e senza attributi sparse, offline o cloud (compressi WOF) si correggono aprendoli con soli attributi e leggendo `FileStandardInfo`. Gli indici delle cartelle si misurano con `FileStandardInfo` sull'handle già aperto. I flussi alternativi non si contano e la UI lo dichiara. | Sull'albero di prova l'allocata di file normali, compressi NTFS, WOF, sparse e residenti nella MFT coincide al byte con `FileStandardInfo` letto dall'oracolo, come nella matrice di 02 §3. | Albero di prova (06 §1.2) |
| F-SC-9 | Segnaposto cloud riconosciuti da attributi e tag di reparse, senza aprire i dati, con due valori: "nel cloud" (dimensione logica) e "su questo disco" (byte locali). La scansione non li idrata e, per default, non elenca le cartelle con `RECALL_ON_DATA_ACCESS`. | Con una sync root di prova registrata con la Cloud Filter API, il provider non riceve richieste di dati né di elenco durante scansione, ispettore e lente Download. I due valori coincidono con quelli dichiarati dal provider. | Albero di prova con provider di prova (06 §1.2, §3.2) |
| F-SC-10 | File system: NTFS completo; exFAT e FAT32 sulle unità rimovibili, con allocata pari all'arrotondamento al cluster e senza hard link; gli altri (ReFS, Dev Drive, condivisioni di rete) "senza garanzie", dichiarato nella UI. La UI dice cosa non è disponibile su quel file system (hard link, compressione, Cestino). | Su VHDX NTFS, exFAT e FAT32 i totali coincidono con l'oracolo. Con un volume di altro tipo compare la dicitura "senza garanzie". | VHDX (06 §1.3); snapshot (06 §7) |

### 5.2 Panoramica (F-OV)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-OV-1 | Bilancio del volume secondo la definizione di §6.4: usato e libero; usato diviso in allocato unico letto, metadati misurati e non attribuito a tratteggio; deriva dell'usato mostrata come incertezza. Dalla M3 l'allocato letto si divide per gestore, con il colore solo da fatti "da fonte" o "dedotto alta" (§6.2). | Su un VHDX NTFS fermo il non attribuito è ≤ metafile attesi + tolleranza, valori fissati in M1 da tre misure e scritti nel test. Con una build di test che non deduplica gli hard link il non attribuito si sposta esattamente dei byte contati due volte. Su C:, con attività normale, l'usato a inizio e a fine scansione è registrato e la differenza compare come incertezza. Sulle fixture classificate le somme per gestore rispettano la definizione e gli elementi con attribuzione "ipotesi" restano nel grigio. | VHDX ed errore iniettato (06 §1.4); test di proprietà sul colore (06 §2.2) |
| F-OV-2 | "Dove finisce lo spazio": i 10 elementi più grandi a qualunque livello, solo elementi (file o cartelle), mai categorie o aggregati. Un elemento non compare insieme a un suo antenato già elencato, salvo quando la sua spiegazione viene da una regola diversa. Ogni riga ha gestore oppure "ipotesi", glifo della fonte, una riga di spiegazione e la dimensione; le quote sono "dello spazio usato". Il Cestino con cartelle orfane compare con le sole misure: "{x} GB su disco, {y} MB secondo Windows". | Su alberi sintetici con antenati e discendenti grandi l'elenco coincide con quello atteso dalla regola di non sovrapposizione. Nessuna riga è una categoria e nessuna riga mostra una cifra recuperabile. | Alberi sintetici (06 §1.4) |
| F-OV-3 | Card dei conti: usato secondo Windows, file letti (ogni file contato una volta), metadati NTFS misurati, non attribuito. Gli hard link compaiono come nota ("già contati una volta"), non come riga da sottrarre. Due liste: "Osservato" (cartelle negate con il loro numero, cestini di altri SID, profili altrui) e "Possibile, misurabile solo come amministratore" (copie shadow, archiviazione riservata, altri metafile). Pulsante "Ripeti come amministratore" con la spiegazione di cosa legge. Nessuna frase causale. | Su fixture con cartelle negate, cestini di altri SID e profili altrui le due liste hanno il contenuto atteso. Nessuna stringa della card attribuisce una causa allo spazio mancante (controllo del catalogo). Dopo il completamento con privilegi le voci misurate dall'helper passano da "Possibile" a "Osservato" con il loro valore. | Fixture e controllo del testo (06 §1.4); helper su VHDX (06 §1.5) |
| F-OV-4 | Card interventi: solo voci nate da un record di raccomandazione (§6.1), ciascuna con stato, stima con il suo stato e conseguenze. Mai voci generate da dimensione, età, nome o estensione; mai voci con stato Non sappiamo o con la variante "Nessuna procedura: non toccare"; nessuna voce per il Cestino orfano. | Test di proprietà: con record generati a caso e con elementi classificati solo da euristiche, la card contiene solo voci con un record e uno stato tra Gestibile, Da verificare, Decidi tu e Lo gestisce X con una procedura. | Test di proprietà (06 §1.4, §2.2) |

### 5.3 Esplora (F-EX)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-EX-1 | Mappa ad albero annidata (squarified, due livelli con intestazione), colore per gestore secondo §6.2, tratteggio per il non attribuito e il non leggibile, etichette solo dove l'area le contiene, coppie fisse di riempimento ed etichetta (04 §6.2). | Tempo di frame p95 ≤ 16 ms durante hover e selezione con 20.000 aree visibili in una finestra a 1080p sul PC di sviluppo (obiettivo, §7). Contrasto di ogni coppia di riempimento ed etichetta ≥ 4,5:1 nei due temi. | Tempi di frame strumentati (06 §9); test automatico dei token (06 §7) |
| F-EX-2 | Tabella ad albero virtualizzata e sincronizzata con la mappa: lista appiattita dei soli nodi espansi, con uno scroller che mappa la barra su un indice di riga. Colonne: nome, dimensioni su disco, quota del genitore, file, ultima modifica, "Gestito da" con il glifo. | Frame p95 ≤ 16 ms e p99 ≤ 33 ms durante uno scorrimento continuo dall'inizio alla fine e durante salti con la barra, con 3,5 milioni di righe e con una cartella da 1 milione di figli diretti (obiettivi, §7). Selezionare una riga seleziona la stessa area nella mappa, e viceversa. | Tempi di frame strumentati (06 §9); albero di prova (06 §1.2); test UI (06 §7) |
| F-EX-3 | Navigazione e tastiera: barra del percorso con dimensione e quota dello spazio usato, avanti e indietro, tastiera completa (frecce, Invio, Backspace, Tab), focus sempre visibile. | Ogni azione dell'Esplora si esegue da tastiera, senza mouse, in un test automatico. Il focus è visibile in ogni stato (snapshot). | `egui_kittest` (06 §7) |
| F-EX-4 | Ricerca per nome sull'albero scansionato e filtri: dimensione minima in byte con l'unità esplicita, tipo secondo l'estensione, data di modifica, gestore. La casella cerca cartelle e file; i comandi arrivano con la palette nella v1.1. | Ricerca ≤ 300 ms su 3 milioni di nomi (obiettivo, §7). Ogni filtro, da solo e combinato con gli altri, restituisce lo stesso insieme di una query di riferimento sul modello. | Protocollo di prestazioni (06 §9); test sul modello (06 §7) |
| F-EX-5 | Vista File grandi: i file più grandi del volume per dimensione su disco, con i filtri di F-EX-4. | Sull'albero di prova l'elenco coincide, nello stesso ordine, con l'ordinamento per allocata calcolato dall'oracolo. | Test contro l'oracolo (06 §7) |

### 5.4 Ispettore (F-IN)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-IN-1 | "Cos'è": nome, tipo, gestore, programma e versione quando noti. Ogni fatto ha la sua classe di provenienza (§6.1) con glifo ed etichetta testuale, la sua fonte e, se dedotto, l'affidabilità (§6.2). In fondo la regola che ha prodotto la classificazione, con versione, data di verifica e "Apri la regola". | Test di proprietà: ogni fatto di ogni record ha una classe e una fonte non vuote; ogni fatto dedotto ha l'affidabilità calcolata con i criteri di §6.2. Ogni fatto mostrato ha il glifo e l'etichetta testuale (snapshot). | Test di proprietà (06 §2.2); `egui_kittest` (06 §7) |
| F-IN-2 | "Perché è così grande": parti principali con una riga ciascuna, schemi riconosciuti dalle regole (cartelle di versione, cache, dati dell'utente dentro la cartella di un'app), distribuzione per tipo secondo l'estensione e per età. | Con la fixture di CapCut l'ispettore mostra le cartelle di versione, indica quella in uso con due fonti concordi (voce HKCU e `Apps\ProductInfo.xml`, "da fonte") e separa `User Data\Projects` come tuoi file. Ogni schema mostrato cita la regola che lo riconosce. | Tabella attesa per regola (06 §2.1) |
| F-IN-3 | Tipo dall'estensione e dagli attributi, senza leggere il contenuto, dichiarato "secondo l'estensione". | Durante scansione e ispettore nessun `ReadFile` su file dell'utente. Il testo "secondo l'estensione" accompagna ogni ripartizione per tipo (snapshot). | Process Monitor o ETW (06 §11); `egui_kittest` (06 §7) |
| F-IN-4 | "Cosa puoi fare": stato, azione, metodo preferito e alternative, controlli, stima con il suo stato, reversibilità, privilegi e regola, tutti presi dal record di raccomandazione (§6.1). Nessuna azione senza record. Con stato Non sappiamo solo misure e "Apri in Esplora file". | Per ogni regola il record prodotto dalla fixture coincide campo per campo con la tabella attesa: ingresso, stato, azione, metodo e alternative, controlli, stato della stima, privilegi. Per una cartella senza regola lo stato è Non sappiamo e l'unica azione è "Apri in Esplora file". | Tabella attesa per regola e test di proprietà (06 §2.1, §2.2) |
| F-IN-5 | Azioni sempre disponibili: "Apri in Esplora file", che seleziona l'elemento senza aprirlo; "Copia percorso"; "Proprietà" di Windows, solo su richiesta esplicita perché la shell carica estensioni di terzi (RS14 in 08). | Le tre azioni funzionano con percorsi oltre 260 caratteri, fino al limite di circa 32.767 caratteri (02 §12). "Apri in Esplora file" non apre il file. | Albero di prova (06 §1.2); test UI (06 §7) |
| F-IN-6 | "Rileggi dal disco" (nei documenti più vecchi "Verifica questa voce"): apre il file o la cartella con soli attributi, legge `FileStandardInfo`, aggiorna il modello e riaggrega gli antenati. Corregge le dimensioni non aggiornate dei file aperti in scrittura. Non legge il contenuto. | Con un file tenuto aperto in scrittura mentre cresce, dopo la verifica la dimensione coincide con l'oracolo e i totali degli antenati sono aggiornati. Nessun `ReadFile`. Su un VHDX con l'aggiornamento dell'ultimo accesso attivo la data di ultimo accesso non cambia. | Albero di prova (06 §1.2); VHDX (06 §3.6) |

### 5.5 Lente Download (F-DL)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-DL-1 | Famiglie per tipo secondo l'estensione, su `Downloads` per default e su qualunque cartella dell'utente: installatori, archivi, immagini disco, video e audio, documenti e immagini, download incompleti. Per ogni famiglia dimensione e numero di file. La testata dichiara il metodo: "Tipi secondo l'estensione". | Su fixture con ogni famiglia, in `Downloads` e in un'altra cartella scelta, dimensioni e conteggi per famiglia coincidono con i valori attesi. All'apertura della lente nessuna lettura di contenuto. | Fixture e Process Monitor (06 §3.1, §3.2) |
| F-DL-2 | Metadati degli installatori su richiesta: nome prodotto e versione dalla risorsa di versione PE (`GetFileVersionInfoW`) e, per gli MSI, dalla tabella Property letta con `MsiOpenDatabaseW` in sola lettura, mai con `MsiOpenPackage`. Lettura in background, annullabile, con una cache per (percorso, dimensione, data di modifica, ID file). Corrispondenza con i programmi installati: stessa versione, versione diversa, non installato. ProductCode uguale alla chiave di disinstallazione: "da fonte"; nome e versione di un exe: "dedotto", con l'affidabilità di §6.2. Prima della lettura la UI dice quanti file legge e che la lettura "aggiorna la data di ultimo accesso di questi file". | Sulle fixture (con risorsa, senza risorsa, con risorsa malformata) l'esito è il valore atteso oppure "estrattore non riuscito" con il motivo. Dopo la lettura di un MSI contenuto e data di modifica sono invariati. Durante la lettura frame p95 ≤ 16 ms. Una seconda richiesta sugli stessi file non li rilegge; se cambia dimensione o data di un file, si rilegge solo quello. I tre esiti della corrispondenza sono corretti sulle fixture. | Fixture e Process Monitor (06 §3.5, §3.6); tempi di frame (06 §9) |
| F-DL-3 | Suffisso "(N)" nel nome: solo un filtro. Non raggruppa, non è una prova e non cambia lo stato. | Con `setup (1).exe` di dimensione diversa da `setup.exe` il filtro lo trova e lo stato non cambia. Due file di nome diverso e contenuto identico formano un gruppo solo dopo il confronto di F-DL-4. | Fixture (06 §3.1) |
| F-DL-4 | Copie identiche: SHA-256 calcolato solo su richiesta ("Confronta il contenuto", che dice quanti file e quanti byte legge e che aggiorna la data di ultimo accesso), solo tra file della stessa dimensione, mai sui segnaposto cloud. Dopo il confronto le copie sono Gestibile: si tiene il più vecchio per data di creazione e l'utente può sceglierne un altro. Prima del confronto i file restano Decidi tu e la card interventi non mostra per loro una cifra recuperabile. | Prima della richiesta nessun `ReadFile` sui file della cartella. I file di dimensione unica non si leggono. Nella sync root di prova il comando è disattivato con il motivo e il provider non riceve richieste. Con un annullamento a metà nessun gruppo parziale è presentato come completo. Durante l'hash frame p95 ≤ 16 ms. | Process Monitor e provider di prova (06 §3.2); tempi di frame (06 §9) |
| F-DL-5 | Archivio zip (compreso zip64) con una cartella omonima accanto: si leggono solo il record di fine directory e la directory centrale (nomi e dimensioni) e si confrontano con la cartella: "{N} voci su {M} presenti". Stato Decidi tu, con la prova misurata. Gli altri formati di archivio restano nella loro famiglia, senza confronto. | Sulle fixture (cartella completa, parziale, con dimensioni diverse, con sottocartelle; zip64; zip corrotto) il conteggio è corretto; lo zip corrotto dà "non leggibile" senza interrompere la lente. Letture solo nella coda del file, nessuna decompressione. | Fixture e Process Monitor (06 §3.3) |
| F-DL-6 | Download incompleti (`.crdownload`, `.part`) non in uso secondo Restart Manager: Gestibile. In uso: bloccati, con il nome del processo. | File chiusi: Gestibile. Gli stessi file tenuti aperti da un processo di prova: bloccati, con il nome del processo. | Fixture con processo di prova (06 §3.4) |

### 5.6 Programmi (F-AP)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-AP-1 | Elenco dei programmi: voci di disinstallazione in HKLM, HKLM WOW6432Node e HKCU; pacchetti MSIX (`PackageManager.FindPackagesForUser`); giochi Steam (`libraryfolders.vdf`, `appmanifest_*.acf`) ed Epic (`Manifests\*.item`). Le voci con `SystemComponent=1` sono nascoste per default. Altri launcher non sono coperti in v1 (§8). | Dalle fixture ogni voce con `DisplayName` è considerata e quelle con `SystemComponent=1` sono nascoste. Un formato inatteso produce "estrattore non riuscito" con il motivo, mai un dato ricostruito. Sul PC di sviluppo i conteggi coincidono con un controllo manuale con data. | Fixture (06 §2.3, §4); controllo manuale con data |
| F-AP-2 | Spazio per programma solo con una prova del percorso: `InstallLocation`, percorso contenuto in `UninstallString` o `DisplayIcon` (solo come indizio di percorso, mai eseguito), `InstalledLocation` di MSIX, manifest dei launcher; cartelle dati dalle regole. Ogni byte ha un solo proprietario, scelto per prefisso più lungo. Senza prova: "posizione non dichiarata" e nessuno spazio per somiglianza del nome. | Un programma senza prova di percorso mostra "posizione non dichiarata" e 0 byte attribuiti. Con la fixture di Unreal Engine e dei plugin Epic che dichiarano la stessa `InstallLocation` ogni byte è contato una volta. | Fixture (06 §4) |
| F-AP-3 | Dichiarato contro misurato: `EstimatedSize` del registro (in KB, non più calcolata dal sistema) e `SizeOnDisk` di Steam (dimensione logica, può essere vecchia) accanto al misurato, ciascuno etichettato con la sua fonte. La differenza si spiega quando la regola la conosce. | Le cifre dichiarate sono sempre etichettate con la fonte e non entrano mai nelle somme del misurato. | Fixture e snapshot (06 §4) |
| F-AP-4 | Uso noto: "Ultima sessione registrata da Steam: {data}" da `LastPlayed`; "Ultimo avvio da Esplora file o Start: {data} (UserAssist)" solo se `Start_TrackProgs` = 1, con la copertura dichiarata; con `Start_TrackProgs` = 0 il testo "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso". `InstallDate` dal registro. Per Epic e MSIX "nessun dato d'uso". Mai "inutilizzato", "mai usato", "non usato da N giorni", "sicuro da disinstallare": lo stato è Decidi tu. | Con `Start_TrackProgs` = 0 non compare nessun valore UserAssist, anche se la chiave contiene dati vecchi (caso del PC di sviluppo). `LastPlayed` di `localconfig.vdf` e degli `.acf` concorda sulle fixture. Nessuna stringa del catalogo e nessun record contiene le parole vietate. | Fixture (06 §2.4, §4); controllo automatico del catalogo (06 §7) |
| F-AP-5 | Disinstallazione solo tramite interfaccia ufficiale: Win32 con `ms-settings:appsfeatures` e il nome copiato negli appunti (in alternativa `appwiz.cpl`); MSIX con `ms-settings:appsfeatures-app?<PFN>`; Steam con `steam://uninstall/<appid>`; Epic con la libreria del launcher. Mai `UninstallString` né `QuietUninstallString`. Prima di aprire l'interfaccia, se una regola indica tuoi file nella cartella del programma, Quado chiede di copiarli altrove. | Il lanciatore accetta solo valori tipizzati: un test di compilazione mostra che non si costruisce da una stringa letta dal registro. Quado non crea processi, a parte l'helper. Con la fixture di CapCut compare l'avviso sui tuoi file. Sul PC di sviluppo ogni URI apre la pagina attesa. | Test di compilazione e di architettura (06 §4, §5.1, §11); controllo manuale con data (06 §5.7) |
| F-AP-6 | Residui: cartelle in Local, Roaming, LocalLow e ProgramData non rivendicate da alcun programma installato né da una regola. Stato Non sappiamo, data di ultima modifica, testo "Nessun programma installato corrispondente (confronto per nome). Ultima modifica: {data}. Potrebbe essere un residuo." Nessuna proposta. | Sulle fixture le cartelle non rivendicate hanno stato Non sappiamo, data e testo. Test di proprietà: nessun residuo riceve un'azione diversa da "Apri in Esplora file" né una voce nella card interventi. | Fixture e test di proprietà (06 §4) |

### 5.7 Windows e sistema (F-SY)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-SY-1 | Vista Windows e sistema: file di paging, ibernazione e swap, WinSxS, SoftwareDistribution, Windows\Installer, DriverStore, Cestino, Windows.old, copie shadow, archiviazione riservata, metadati NTFS (`$MFT`, indici delle cartelle, `$UsnJrnl`). Per ogni voce: misura oppure "non disponibile" con il motivo, spiegazione, stato (§6.1) e procedura ufficiale con il link alla documentazione Microsoft. | Il record di ogni voce coincide con la tabella attesa (casi di §6.1). Le voci non misurabili senza amministratore mostrano "non disponibile" con il motivo, mai 0. | Tabella attesa per regola (06 §2.1); snapshot (06 §7) |
| F-SY-2 | Cestino misurato in due modi: "secondo Windows" (`SHQueryRecycleBinW`) e "su disco" (scansione di `$Recycle.Bin` dell'utente corrente). Le cartelle `$R` senza `$I` sono un'anomalia spiegata nell'ispettore: nessuna cifra recuperabile, nessun intervento. La top list riporta solo le due misure. | Su un VHDX con un `$R` senza `$I`: "su disco" lo conta, "secondo Windows" no, e la card interventi non ha voci per lui. Sul PC di sviluppo le due misure coincidono con un controllo manuale con data. | VHDX (06 §5.3); tabella attesa (06 §2.1); controllo manuale con data |
| F-SY-3 | Configurazione in sola lettura: ibernazione, avvio rapido e azione per batteria critica (API di alimentazione); Sensore memoria (attivo, frequenza, giorni per Cestino e Download); `MaxCapacity` e `NukeOnDelete` del Cestino per volume; tracciamento degli avvii (`Start_TrackProgs`); ripristino configurazione se leggibile; percorso della cache di npm da `.npmrc` o dalla variabile d'ambiente. Solo lettura di registro, file e API, mai esecuzione di programmi. Le letture servono alle raccomandazioni condizionali e avvengono con la scansione: non c'è un controllo continuo. | Fixture per ogni combinazione. Durante le letture nessuna chiave è aperta in scrittura e nessun processo viene creato. Sul PC di sviluppo i valori coincidono con Impostazioni e con `powercfg /a` eseguito a mano. Ripristino configurazione non leggibile: "non disponibile" con il motivo. | Fixture, Process Monitor, controllo manuale con data (06 §2.4) |

### 5.8 Interventi (F-AC)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-AC-1 | Tipi di intervento in v1, e solo questi: spostamento nel Cestino di elementi scelti, da un piano confermato (F-AC-2); procedure di Windows ("Apri il Cestino"; "Svuota il Cestino" con il dialogo di Windows, dopo aver detto quanti altri elementi e quanti GB contiene secondo Windows; Impostazioni › Archiviazione e File temporanei, con Pulizia disco come alternativa); disinstallazione tramite interfaccia ufficiale (F-AP-5); comandi ufficiali mostrati con la fonte e copiati negli appunti, mai eseguiti. Dopo una procedura o un comando esterno, "Ho eseguito la procedura" avvia le nuove misure (F-VE-2). Nessuno "svuota tutto" implicito. | Le chiamate che modificano file o avviano procedure compaiono solo in `quado-actions` (test di architettura). "Svuota il Cestino" mostra il dialogo di Windows (nessun `SHERB_NOCONFIRMATION`), preceduto dal numero e dai GB degli altri elementi. "Apri il Cestino" elenca gli elementi spostati da Quado presi dal registro. `--volumes` non compare in nessuna stringa; `/ResetBase` compare solo tra le alternative, con la conseguenza. Con `NukeOnDelete` = 1 o su un'unità senza Cestino lo spostamento è disattivato con la spiegazione. | Test di architettura (06 §5.1); VHDX con Cestino piccolo (06 §5.3); controllo del catalogo (06 §5.7) |
| F-AC-2 | Simulazione obbligatoria prima di ogni spostamento nel Cestino. La simulazione produce un valore `Simulazione`: elenco, totale, stima con metodo e stato (§6.3), esito dei controlli, scheda della raccomandazione. L'esecuzione accetta solo un `PianoConfermato`, tipo dedicato che si costruisce solo dalla funzione di conferma del dialogo, a partire da una `Simulazione` e dalla scelta dell'utente; i campi sono privati e non esiste un altro costruttore. La costruzione respinge gli elementi protetti (salvo l'eccezione di F-AC-4 con tutti i controlli soddisfatti), quelli con un controllo automatico fallito e quelli con un controllo manuale non spuntato. Un piano si esegue una volta sola. Se la selezione contiene elementi dentro una sync root registrata (segnaposto o file sincronizzati, F-SC-9), la simulazione aggiunge il controllo manuale "spostarlo nel Cestino lo elimina anche nel cloud e sugli altri dispositivi" (testo in 04 §6.8; R10 in 08). | Test di compilazione: fuori dal modulo di conferma un `PianoConfermato` non si costruisce; l'API di esecuzione non accetta altri tipi; un piano eseguito non si riusa. Test di proprietà con selezioni casuali che includono elementi protetti: la costruzione le respinge sempre. Nella sync root di prova una selezione con un file sincronizzato mostra quel controllo manuale; senza la spunta la costruzione la respinge, e il provider non riceve richieste di dati. | `trybuild` e `proptest` (06 §5.1, §5.2) |
| F-AC-3 | Conferma esplicita: titolo in forma di domanda, "Anteprima di cosa succede. Nessun file è stato ancora toccato.", scheda della raccomandazione completa (stato, prove, fatti dedotti marcati come tali, regola con fonte e data, alternative), elenco con caselle, stima con il suo stato, controlli con il loro esito, testo sulla reversibilità. Esc, Annulla e la X chiudono senza eseguire. Dopo l'esecuzione un riepilogo separa riusciti, saltati e falliti con il motivo; in caso di esito parziale dice cosa è nel Cestino e come ripristinarlo. | Esc, Annulla e la X non costruiscono un `PianoConfermato` e non chiamano `IFileOperation`. Con un file in uso il riepilogo è corretto e nessun file oltre a quelli confermati viene toccato. Il dialogo contiene tutti i campi del record (snapshot). | Test UI e Process Monitor (06 §5.1, §5.6) |
| F-AC-4 | Elementi protetti, mai spostabili: cartella Windows, WinSxS, Windows\Installer, System Volume Information, file di paging, ibernazione e swap, cartelle di programmi registrati (si rimanda alla disinstallazione, F-AP-5). Il comando è disattivato con la spiegazione. Unica eccezione: una regola con stato Da verificare e controlli dedicati, documentata in §6.1; in v1 la usa solo la regola di CapCut. | Test di proprietà: ogni selezione con un elemento protetto è respinta. Per l'eccezione: un caso positivo con tutti i controlli soddisfatti e un caso negativo per ciascun controllo mancante. | `proptest` su alberi di prova (06 §5.2) |
| F-AC-5 | Mai eliminazione definitiva. Lo spostamento usa `IFileOperation` con `FOFX_RECYCLEONDELETE`, mai `FOF_NOCONFIRMATION`. In `PreDeleteItem`, se manca `TSF_DELETE_RECYCLE_IF_POSSIBLE`, l'operazione per quell'elemento si annulla e si segnala. Il comportamento si verifica in M1 su VHDX; se la verifica fallisce resta il solo controllo di capacità (F-AC-7). | Su un VHDX con un Cestino piccolo un elemento più grande del Cestino non viene eliminato: resta al suo posto con lo stesso ID e la stessa dimensione ed è riportato come saltato. | VHDX con Cestino piccolo (06 §5.3) |
| F-AC-6 | Registro degli interventi in `%APPDATA%\Quado\interventi.jsonl`: data, elementi con percorso, metodo, stima con metodo e stato, misure prima e dopo, ottenuto, differenza e spiegazione, esito per elemento. Si crea al primo intervento, si consulta dalla vista Interventi (F-VE-5) e si cancella dalle impostazioni. | Il file non esiste prima del primo intervento. Dopo un intervento su VHDX contiene tutti i campi con un valore, compresi stimato e ottenuto. Dopo la cancellazione dalle impostazioni il file non esiste più. | VHDX e Process Monitor (06 §5.3, §5.6) |
| F-AC-7 | Capacità del Cestino e Sensore memoria, prima di ogni spostamento. `MaxCapacity` del volume, letta in sola lettura dalla chiave BitBucket, si confronta con il contenuto attuale (`SHQueryRecycleBinW`) più la selezione: se il totale supera la capacità, lo spostamento si blocca con il testo di 04 §6.8, perché Windows eliminerebbe da solo gli elementi più vecchi. Finché la verifica di M1 non dice come Windows conta la capacità, il confronto usa la stima più prudente (05 D7). Se Sensore memoria svuota il Cestino in automatico, la conferma lo dice con il numero di giorni. | Su un VHDX con `MaxCapacity` ridotta una selezione che supererebbe la capacità è bloccata prima di `IFileOperation`, e gli elementi già nel Cestino restano tutti. Con la fixture di Sensore memoria attivo la conferma riporta i giorni letti. | VHDX con Cestino piccolo e fixture (06 §2.4, §5.3) |
| F-AC-8 | Restart Manager: prima dello spostamento si registrano i file dell'elemento (per le cartelle grandi tutti gli eseguibili e le DLL più un campione degli altri file, 05 D7); se un processo li usa, l'elemento è bloccato con il nome del processo. Subito prima dello spostamento ogni elemento si ricontrolla (ID, dimensione, data di modifica); se qualcosa è cambiato, si salta. | File aperto da un processo di prova: bloccato, con il nome del processo. File aperto e chiuso prima del controllo: ammesso. File aperto dopo il controllo e prima dello spostamento: fallito nel riepilogo, mentre gli altri proseguono. Cartella grande: il campione comprende tutti gli eseguibili e le DLL. | Processo di prova su albero di prova (06 §3.4, §5.4) |
| F-AC-9 | Riferimenti, quando la regola li richiede: registro (`UninstallString`, `DisplayIcon`, `InstallLocation`, App Paths, chiavi Run di HKCU e HKLM), collegamenti `.lnk` di Start e Desktop (dell'utente e pubblici) letti senza `Resolve`, attività pianificate visibili all'utente. Un riferimento all'elemento lo blocca e compare nella conferma. Se alcune attività non sono visibili all'utente, il controllo è "parziale" e la conferma lo dice. | Una cartella su VHDX referenziata da `DisplayIcon`, da una chiave Run, da un `.lnk` nel menu Start e da un'attività pianificata: ciascun riferimento la blocca e compare nella conferma. I `.lnk` non si risolvono (nessun accesso di rete e nessuna ricerca). Un'attività non visibile rende il controllo "parziale". | VHDX, sorgente di registro simulata, Process Monitor (06 §5.5) |

### 5.9 Verifica (F-VE)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-VE-1 | Misura di partenza, prima dell'azione o dell'apertura della procedura: spazio libero (`GetDiskFreeSpaceExW`), dimensione del Cestino (`SHQueryRecycleBinW`), stima con metodo e stato (§6.3), ora. | Nel registro i quattro valori hanno un'ora precedente alla prima chiamata di `IFileOperation` o all'apertura della procedura. | VHDX (06 §6.2) |
| F-VE-2 | Nuove misure dopo l'azione, dopo lo svuotamento del Cestino e dopo "Ho eseguito la procedura": spazio libero e riscansione dei sottoalberi toccati, con l'ora. | Su VHDX ciascuno dei tre eventi produce nel registro una misura con spazio libero, riscansione e ora. | VHDX (06 §6.2) |
| F-VE-3 | Stimato contro ottenuto: due numeri separati, "stimato" (metodo di §6.3) e "variazione dello spazio libero" (l'ottenuto). Oltre il 5% di differenza, calcolata come in §6.3, la differenza si spiega voce per voce con fatti misurati oppure si dichiara "non spiegata". Con i punti di ripristino attivi compare la nota sullo spazio che può tornare occupato. | Su VHDX, per ogni scenario di 06 §6.1, la variazione reale cade entro la tolleranza di 06 §6.1 rispetto alla stima esatta, oppure dentro l'intervallo. Con un processo che scrive sul VHDX durante la verifica la variazione cambia e la riscansione no; la differenza è dichiarata e non attribuita all'intervento. Ogni voce di spiegazione corrisponde a un fatto misurato. | VHDX (06 §6.1, §6.2) |
| F-VE-4 | Aggiornamento del modello con la riscansione dei sottoalberi toccati: nodi vecchi staccati, nodi nuovi aggiunti, antenati riaggregati. | Dopo la riscansione i totali coincidono con quelli di una scansione completa nuova dello stesso volume fermo. | VHDX (06 §6.2) |
| F-VE-5 | Vista Interventi: storico dal registro con data, elementi, stato, metodo, stimato, ottenuto, differenza e spiegazione. | Con un registro di fixture di N interventi la vista mostra N righe con gli stessi valori del file. | Fixture e snapshot (06 §6.2) |

### 5.10 Generali (F-GE)

| ID | Requisito | Criterio di accettazione | Verifica |
|---|---|---|---|
| F-GE-1 | Interfaccia in più lingue (05 D13): italiano e inglese completi nella v1, altre lingue aggiungibili con un file di testo per lingua, anche senza ricompilare. Lingua scelta in Impostazioni con cambio immediato; al primo avvio quella di Windows se disponibile, altrimenti inglese. Il core restituisce codici, la UI li traduce. | Ogni stringa visibile viene dai file di lingua: con la pseudo-lingua di prova ogni testo a schermo è pseudo-localizzato. `it` ed `en` hanno le stesse chiavi e gli stessi segnaposto. Nessun testo tagliato con testi più lunghi del 40%. Il catalogo non contiene le parole vietate di 04 §6.8 né "non più in uso". | Controllo automatico (06 §7) |
| F-GE-2 | Temi chiaro, scuro e "come il sistema", con colori presi solo dai token. | Ogni coppia di testo e superficie, in ogni stato (normale, hover, premuto, selezionato), ha contrasto ≥ 4,5:1; i colori dei dati hanno contrasto ≥ 3:1 con il fondo dei pannelli e le coppie di tinte, grigio di "Non classificato" compreso, superano il controllo delle discromatopsie di 04 §6.2. Snapshot di ogni schermata nei due temi a scala 100%, 150% e 200%. | Test automatico dei token, `egui_kittest` (06 §7) |
| F-GE-3 | Unità come Esplora file sul singolo file: 1 GB = 1024³ byte, virgola decimale, etichette "Dimensioni" e "Dimensioni su disco"; SI (1 GB = 10⁹ byte) a scelta nelle impostazioni. I totali delle cartelle possono differire da Esplora, perché Quado conta una volta i file con più nomi. | Su file di prova "Dimensioni su disco" coincide con le Proprietà di Esplora file, nella stessa unità. Con SI attivo tutte le unità mostrate cambiano di conseguenza (snapshot). | Controllo manuale con data e snapshot (06 §7) |

## 6. Modello delle raccomandazioni

Le regole, gli estrattori e la struttura del record in Rust sono in 05 D6; le tabelle attese per regola e i test di proprietà in 06 §2.

### 6.1 Stati e scheda della raccomandazione

Ogni elemento a cui una regola assegna un ruolo riceve dal motore un record di raccomandazione con uno di cinque stati. Un elemento senza regola ha stato Non sappiamo. Gli stati hanno un vocabolario unico, con le stesse parole in card, ispettore, conferma e risultato, sempre con icona più parola e mai con il solo colore (04 §6.1).

| Stato | Quando | Cosa abilita | Esempi |
|---|---|---|---|
| Gestibile | Prova di gestibilità diretta, non solo di identità, e metodo a basso rischio | Azione con simulazione (F-AC-2) | Copie identiche verificate con hash; download incompleti non in uso |
| Da verificare | Gestibilità plausibile, prova parziale. I controlli sono elencati: quelli automatici devono passare, quelli manuali li spunta l'utente | Azione disabilitata finché i controlli non sono soddisfatti | Versioni precedenti di CapCut |
| Decidi tu | Ruolo noto, utilità non determinabile dal software | Strumenti per decidere (raggruppamenti, corrispondenze, confronti); azione solo su scelta esplicita, con simulazione | Tuoi file, installatori, immagini disco, programmi grandi, ibernazione, `node_modules` |
| Lo gestisce X | Si gestisce solo con lo strumento o la procedura di X (Windows, Steam, npm, Docker, l'app) | Rinvio alla procedura: URI aperto, oppure comando mostrato e copiato; mai rimozione manuale | Cache di npm, store di pnpm, WinSxS, file temporanei di Windows |
| Lo gestisce X, variante "Nessuna procedura: non toccare" | X non offre una procedura per ridurlo | Nessuna azione oltre a quelle sempre disponibili (F-IN-5) | `Windows\Installer` |
| Non sappiamo | Nessuna prova sul ruolo | Solo misure e "Apri in Esplora file" | Cartella senza regola, possibili residui |

Prova di identità e prova di gestibilità sono distinte:

| Prova | Cosa stabilisce | Esempio |
|---|---|---|
| Identità | Cos'è l'elemento e a chi appartiene | Voce di disinstallazione in HKCU e `Apps\ProductInfo.xml` concordi sulla versione in uso di CapCut |
| Gestibilità | Che l'azione proposta ha effetti noti e limitati su quell'elemento | Hash uguale: togliere una copia lascia lo stesso contenuto nell'altra. Restart Manager: nessun processo usa il `.crdownload` |

Gli stati Gestibile e Da verificare si costruiscono solo con una prova di gestibilità distinta dalla prova di identità: il costruttore del record richiede un valore `ProvaDiGestibilita` (05 D6) e un test di proprietà lo verifica anche su record generati dal motore con ingressi casuali (06 §2.2). Una prova di identità, per quanto forte, non basta: sapere che una cartella contiene una versione precedente di CapCut non dice che spostarla non abbia effetti.

Campi obbligatori del record. Nessun campo può essere vuoto: quando un valore non esiste il campo dice perché (per esempio "nessun privilegio richiesto", "non reversibile").

| Campo | Contenuto |
|---|---|
| Stato | Uno dei cinque stati |
| Azione | Spostamento nel Cestino, procedura, comando, disinstallazione, oppure nessuna |
| Prova di identità | Fatti che dicono cos'è l'elemento e a chi appartiene, ciascuno con la sua provenienza |
| Prova di gestibilità | Fatti che dicono che l'azione ha effetti noti; obbligatoria per Gestibile e Da verificare, tenuta separata dalla prova di identità |
| Ipotesi | Ciò che si assume senza prova, scritto come tale |
| Affidabilità | Livello dei fatti dedotti, con il criterio di §6.2 che lo produce |
| Rischi ed effetti | Cosa può succedere: nuovi download, perdita di impostazioni, un programma che non si avvia |
| Metodo preferito e alternative | Per esempio `npm cache verify`, con `npm cache clean --force` come alternativa e le sue conseguenze |
| Controlli preventivi | Automatici, con il loro esito, e manuali, da spuntare |
| Stima | Valore, metodo e stato: esatta, intervallo o non stimabile (§6.3) |
| Reversibilità | Cosa si può ripristinare e fino a quando, per esempio "dal Cestino finché non viene svuotato, da te o da Sensore memoria" |
| Privilegi richiesti | Nessuno, oppure amministratore (in quel caso il comando si mostra e si copia) |
| Errore o esito parziale | Cosa succede a ogni elemento se l'azione si ferma a metà, e come si ripristina |
| Verifica dopo l'azione | Quali misure si prendono e quando (F-VE) |
| Regola | Identificativo, fonte con URL ed estratto testuale, intervallo di versioni, data di verifica |

La provenienza vale per singolo fatto, non per elemento: una riga può avere più fatti con classi diverse. Nell'ispettore di Download, per esempio, dimensioni e date sono "misurato" e il contenuto non ancora confrontato è "non disponibile".

| Classe | Glifo (04 §6.1) | Significato | Esempi |
|---|---|---|---|
| misurato | quadrato pieno | Letto dal file system o da un'API di misura in questa sessione | Allocata, date, totale e libero del volume, `$MFT`, indici delle cartelle |
| da fonte | quadrato pieno a metà | Dichiarato da Windows o dall'app: registro, API, manifest, file di configurazione | Voce di disinstallazione, pacchetto MSIX, manifest di Steam, `ProductInfo.xml` |
| dedotto | quadrato vuoto | Ricavato da una regola su percorso, nome o struttura; ha un'affidabilità (§6.2) e può sbagliare | Versioni precedenti non più usate, corrispondenza tra un installatore exe e un programma, residuo trovato per nome |
| non disponibile | quadrato tratteggiato | Non leggibile o non misurabile in questa modalità; mai mostrato come zero | Contenuto delle cartelle negate, copie shadow e archiviazione riservata senza amministratore |

Regole di comportamento:
1. Il tipo non implica la gestione. Una cache può essere in uso e un file vecchio può essere importante: la gestione viene solo da una regola che la dichiara, con una fonte.
2. Dimensione, età, nome ed estensione servono a filtri e viste ("installatori in Download", "file più vecchi di un anno"), mai a una proposta: non producono Gestibile o Da verificare e non producono voci nella card interventi.
3. Le raccomandazioni nascono solo da regole con una fonte ufficiale o verificabile (documentazione Microsoft, documentazione del programma, manifest), con l'estratto della frase che giustifica la gestione, l'intervallo di versioni e la data di verifica. L'utente vede sempre la regola, con il link alla fonte.
4. Se la regola vale solo per certe versioni e la versione installata è fuori intervallo o non si conosce, i fatti che dipendono dalla regola scendono a "dedotto, bassa" e non c'è proposta: né Gestibile né Da verificare, nessuna voce nella card interventi, nessuna procedura della regola.
5. Se due regole sono in conflitto si mostrano entrambe, vale l'affidabilità più bassa e non si propone niente.
6. I dati prodotti dall'utente dentro la cartella di un programma (progetti, salvataggi, esportazioni) hanno gestore "tu" quando una regola lo stabilisce: stato Decidi tu, mai proposti per la rimozione.
7. Nessun testo dice "sicuro da eliminare", "inutilizzato", "non più in uso" o "puoi ripristinare tutto". L'elenco completo delle parole vietate è in 04 §6.8.
8. Riverifica delle regole, proposta da 07 §5.1 e da confermare (non è in `revisione/decisioni-revisione.md`): oltre 6 mesi dalla data di verifica la regola entra nel banner "N regole da riverificare"; oltre 11 mesi ispettore e conferma annunciano la scadenza; oltre 12 mesi i fatti dedotti dalla regola scendono ad affidabilità bassa, il record non ha azioni né procedure, l'elemento esce dalla card interventi e lo stato è Non sappiamo, con il motivo scritto. Gli elementi protetti restano protetti (F-AC-4); l'eccezione di CapCut decade con la sua regola.

Casi già decisi (`revisione/decisioni-revisione.md` §3 e §7). Il comportamento completo è in 09, le tabelle attese in 06 §2.1.

| Caso | Stato | Note |
|---|---|---|
| Versioni precedenti di CapCut | Da verificare | Eccezione a F-AC-4, descritta sotto |
| `CapCut\User Data\Projects` | Decidi tu, gestore "tu" | Mai proposto |
| Copie identiche in una cartella dell'utente, dopo l'hash | Gestibile | Si tiene il più vecchio |
| File della stessa dimensione non ancora confrontati | Decidi tu | Strumento "Confronta il contenuto"; nessuna cifra recuperabile prima del confronto |
| Download incompleti non in uso | Gestibile | Restart Manager |
| Archivio zip con una cartella omonima accanto | Decidi tu | Prova misurata: "{N} voci su {M} presenti" |
| Installatori, immagini disco | Decidi tu | Corrispondenza con i programmi installati (F-DL-2) |
| Programmi grandi | Decidi tu | Uso noto solo da fonti affidabili (F-AP-4); disinstallazione ufficiale |
| Residui | Non sappiamo | Data di ultima modifica, nessuna proposta |
| Ibernazione | Decidi tu | Due opzioni affiancate, entrambe da amministratore: `powercfg /hibernate /type reduced` (mantiene l'avvio rapido) e `powercfg /h off` (toglie anche avvio rapido e sospensione ibrida). Lettura preventiva della configurazione; avviso se l'azione per batteria critica è l'ibernazione |
| Cache di npm | Lo gestisce npm | `npm cache verify` preferito; `npm cache clean --force` come alternativa, con le conseguenze (nuovi download, installazioni offline). Percorso dalla configurazione, mai eseguendo npm. Con `verify` la stima è non stimabile e la dimensione della cache si mostra solo come riferimento (09 §10) |
| Store di pnpm | Lo gestisce pnpm | `pnpm store prune`; stima non stimabile senza contare i link |
| Docker, WSL | Lo gestisce Docker / WSL | Stima su C: 0 GB fino alla compattazione del disco virtuale; `--volumes` mai mostrato; compattazione documentata con le sue condizioni. La regola entra solo con una fixture reale: il PC di sviluppo non li usa (07 §7) |
| WinSxS | Lo gestisce Windows | `StartComponentCleanup` senza `/ResetBase`; `/ResetBase` solo come alternativa, con la conseguenza: gli aggiornamenti non si possono più disinstallare |
| SoftwareDistribution, Temp, miniature | Lo gestisce Windows | Solo Impostazioni › File temporanei, mai a mano |
| `Windows\Installer` | Lo gestisce Windows, variante "Nessuna procedura: non toccare" | Nessuna azione |
| `node_modules`, `target`, `.next`, `build` | Decidi tu | Controlli: lockfile presente; `cargo clean` per `target`. Ricostruibili solo se il progetto si può ancora ricostruire |
| Cestino con cartelle `$R` orfane | Lo gestisce Windows, variante "Nessuna procedura: non toccare" (09, scelta 4, da confermare) | Anomalia spiegata (F-SY-2): nessuna cifra recuperabile, nessun intervento |
| Cartella senza regola | Non sappiamo | Solo misure e "Apri in Esplora file" |

Eccezione documentata a F-AC-4: versioni precedenti di CapCut.

CapCut tiene le sue versioni in `AppData\Local\CapCut`, che è la cartella di un programma registrato e quindi è protetta da F-AC-4. Non esiste una procedura ufficiale per togliere le versioni precedenti, e la disinstallazione rischierebbe `User Data\Projects`. La regola di CapCut usa l'eccezione a queste condizioni:
- stato Da verificare: la versione in uso è "da fonte" (voce HKCU e `Apps\ProductInfo.xml` concordi), mentre che le altre versioni non servano più è "dedotto";
- controlli automatici: Restart Manager su un campione dei file e sugli eseguibili (F-AC-8); nessun riferimento alle versioni da spostare nel registro (`UninstallString`, `DisplayIcon`, App Paths, Run), nei collegamenti `.lnk` di Start e Desktop e nelle attività pianificate (F-AC-9);
- per default si tengono le ultime due versioni, quella in uso e la precedente: nella conferma sono visibili ma disattivate, e la stima le esclude;
- testi: nella Panoramica "{n} cartelle di versioni precedenti (dedotto)", mai "non più in uso"; nella conferma "Prova CapCut prima di svuotare il Cestino".

In v1 nessun'altra regola usa l'eccezione. Una nuova eccezione richiede le stesse condizioni (stato Da verificare e controlli dedicati) e va documentata in questa sezione.

### 6.2 Affidabilità dei fatti dedotti

Ogni fatto dedotto ha un livello di affidabilità. Il livello lo calcola il motore dalle prove raccolte, con questa tabella; una regola non può dichiararne uno più alto.

| Livello | Criterio | Esempi |
|---|---|---|
| alta | Due fonti concordi, oppure una fonte ufficiale con struttura conforme | Cartella in un percorso indicato dalla documentazione del programma, con i file marcatore descritti dalla regola |
| media | Una sola fonte non ufficiale, oppure struttura riconosciuta senza fonte | Installatore exe con nome e versione uguali a quelli di un programma installato; percorso di installazione ricavato da una voce in HKCU, che l'utente può scrivere (05 D6) |
| bassa | Solo nome o somiglianza | Cartella con un nome simile a quello di un programma; installatore exe con nome solo in parte uguale; possibile residuo |

"Da fonte" e "dedotto alta" non sono la stessa cosa. Un fatto è "da fonte" quando Windows o l'app lo dichiarano direttamente: la voce di disinstallazione dice la versione installata. Un fatto è "dedotto" quando si ricava con una regola, anche se a partire da fonti: la cartella X contiene i dati del programma Y perché la documentazione di Y indica quel percorso.

Colore del gestore:
- nella mappa e nel bilancio un elemento ha il colore del suo gestore solo se l'attribuzione è "da fonte" o "dedotto alta";
- negli altri casi è grigio, con l'etichetta "ipotesi" (testo in 04 §6.8: "Ipotesi dal nome: {nome}. Nessuna voce di installazione lo conferma.");
- nel bilancio per gestore i byte con attribuzione "ipotesi" stanno nel segmento grigio, insieme al non classificato, e non nella quota del gestore ipotizzato.

Il test di proprietà sul colore è in 06 §2.2.

### 6.3 Stima dello spazio recuperabile

La stima si calcola nella simulazione (F-AC-2), non dal modello della scansione: il modello conserva l'ID solo dei file con più nomi (05 D5). Per ogni file selezionato si rileggono gli attributi con `FileStandardInfo` e `FileIdInfo`, aprendolo con soli attributi, `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`; il contenuto non si legge. La simulazione mostra l'avanzamento e si può annullare.

| Caso | Come entra nella stima |
|---|---|
| File con tutti i nomi nella selezione | Allocata, una volta per ID |
| File con altri nomi fuori dalla selezione (numero di link maggiore dei nomi selezionati) | Escluso e citato: "{n} file esclusi perché hanno altri nomi fuori dalla selezione" |
| Nomi non conoscibili, perché la selezione contiene cartelle negate | Intervallo: estremo inferiore senza i file che potrebbero avere altri nomi in quelle cartelle, estremo superiore con loro. Il contenuto delle cartelle negate non entra nella stima, e la conferma lo dice |
| File compressi NTFS, WOF, sparse | Allocata, non logica |
| File residenti nella MFT | 0, perché la MFT non si restringe |
| Segnaposto cloud | Solo i byte locali; mai idratazione |
| Contenuto di dischi virtuali (Docker, WSL, macchine virtuali) | 0 su C: fino alla compattazione del disco virtuale |
| Elementi spostati nel Cestino | 0 subito ("liberato subito"); la stima vale dopo lo svuotamento |
| Store di pnpm e altri casi con hard link verso cartelle che la regola non conta | Non stimabile senza contare i link |
| Comandi e procedure esterne | Metodo dichiarato dalla regola; senza metodo, non stimabile |
| Punti di ripristino attivi | Il valore non cambia; si aggiunge la nota "parte può essere riassorbita" (`revisione/prove-windows.md` §C) |

Stati della stima:

| Stato | Quando | Come si mostra |
|---|---|---|
| esatta | Ogni file della selezione ha allocata e nomi noti | Un valore |
| intervallo | Alcuni nomi non sono conoscibili | Minimo e massimo, con il motivo |
| non stimabile | Non esiste un metodo: hard link non contati, procedure esterne senza metodo | Nessun valore, con il motivo; vale solo la verifica |

Nella conferma di uno spostamento nel Cestino compaiono due valori: "liberato subito" (0 GB) e "dopo lo svuotamento del Cestino", con lo stato della stima. Dopo l'azione si mostrano due numeri separati, mai sommati o fusi:
- "stimato": la stima calcolata con questo metodo;
- "variazione dello spazio libero", cioè l'ottenuto: spazio libero alla nuova misura meno spazio libero alla misura di partenza (F-VE-1), letti con `GetDiskFreeSpaceExW`. Comprende anche ciò che altri processi hanno scritto o eliminato nel frattempo; la riscansione dei sottoalberi toccati dice quanto della selezione occupa ancora spazio.

Confronto tra stimato e ottenuto (F-VE-3):

| Stato della stima | Differenza | Percentuale per la soglia del 5% |
|---|---|---|
| esatta | ottenuto - stimato | valore assoluto della differenza / stimato × 100 |
| intervallo | 0 se l'ottenuto cade nell'intervallo; altrimenti la distanza dall'estremo più vicino | distanza / estremo più vicino × 100 |
| non stimabile, oppure stimato pari a 0 | Si mostra solo l'ottenuto | Nessuna |

Oltre il 5% la differenza si spiega voce per voce; ogni voce deve corrispondere a un fatto misurato:

| Voce | Fatto misurato che la sostiene |
|---|---|
| Elementi saltati o falliti | Riepilogo dell'esecuzione |
| File esclusi perché hanno altri nomi | Rilettura degli attributi in simulazione |
| Contenuto ancora nel Cestino | Dimensione del Cestino (`SHQueryRecycleBinW`) e riscansione |
| Altri elementi eliminati svuotando il Cestino | Dimensione del Cestino alla misura di partenza e dopo lo svuotamento |
| File ancora aperti al momento dello svuotamento | Restart Manager sui file della selezione |
| Attività di altri processi | Riscansione: la selezione non occupa più spazio, ma la variazione dello spazio libero è diversa |

Se le voci non bastano compare "Differenza non spiegata: {z} GB ({p}%)". Le copie shadow non sono una voce di spiegazione con un valore, perché le loro statistiche richiedono l'amministratore: compaiono solo come nota quando i punti di ripristino sono attivi.

### 6.4 Bilancio del volume

Definizione (`revisione/decisioni-revisione.md` §6):

| Grandezza | Definizione | Come si misura |
|---|---|---|
| Capacità | Byte totali del volume | `GetDiskFreeSpaceExW`, `TotalNumberOfBytes` |
| Libero | Byte liberi del volume | `GetDiskFreeSpaceExW`, `TotalNumberOfFreeBytes` |
| Usato (U) | Capacità - libero, misurato a fine scansione | Due letture alla fine della scansione |
| Usato a inizio scansione (U0) | Stessa misura all'avvio della scansione | Due letture all'avvio |
| Deriva | U - U0 | Si mostra come incertezza sul non attribuito, ± il suo valore assoluto |
| Allocato unico letto (A) | Somma delle allocate dei file letti, ogni ID contato una volta | Scansione (F-SC-2, F-SC-7, F-SC-8) |
| Metadati misurati (M) | `$MFT` + indici delle cartelle + `$UsnJrnl` | `FSCTL_GET_NTFS_VOLUME_DATA`, `FileStandardInfo` sugli handle delle cartelle, `FSCTL_QUERY_USN_JOURNAL`; senza privilegi (02 §7) |
| Non attribuito (N) | U - A - M | Per differenza |

Conseguenze:
- A + M + N = U vale per costruzione: controllarlo non prova niente. I test di F-OV-1 sono altri: residuo su un VHDX fermo entro i metafile attesi più una tolleranza, ed errore iniettato che deve spostare il residuo.
- Se lo spazio disponibile all'utente (`FreeBytesAvailableToCaller`) è minore dei byte liberi per una quota, la differenza compare come nota.
- N non si scompone senza amministratore. La card dei conti lo accompagna con due liste e nessuna frase causale:
  - "Osservato": cartelle negate, con il loro numero; cestini di altri SID; profili altrui;
  - "Possibile, misurabile solo come amministratore": copie shadow, archiviazione riservata, altri metafile (`$LogFile`, `$Bitmap`, `$Secure`).
- Restano nel non attribuito anche i flussi alternativi, che la v1 non conta (F-SC-8).
- Gli hard link compaiono come nota ("già contati una volta"), non come riga da sottrarre.
- I file residenti nella MFT hanno un'allocata minima che si somma alla `$MFT`: è un piccolo doppio conteggio, dichiarato (02 §3).
- Dopo il completamento con privilegi (F-SC-3) il contenuto delle cartelle negate entra in A, e le voci della lista "Possibile" che l'helper misura passano a "Osservato" con il loro valore.
- Ripartizione per gestore, dalla M3: A si divide tra Windows, Programmi e dati delle app, Tuoi file, Non classificato o ipotesi (§6.2). M e N restano parti separate; N ha il tratteggio.
- Le quote si scrivono come "% dello spazio usato", mai come "% del disco".

Unità: nell'interfaccia 1 GB = 1024³ byte, come Esplora file (F-GE-3). Nei documenti l'unità si scrive sempre; le misure della sonda in unità 10⁹ si scrivono come "GB 10^9".

## 7. Requisiti non funzionali

Le soglie di prestazione sono obiettivi. Un numero di prestazioni compare come misura solo se misurato con il protocollo di 06 §9 (5 esecuzioni, mediana e minimo-massimo, annotando hardware, versione di Windows, Defender e data); fino ad allora resta scritto come obiettivo. Hardware e misure della sonda sono in 02 §2.1.

| Area | Requisito | Misura e metodo |
|---|---|---|
| Scansione a cache calda | Obiettivo: C: del PC di sviluppo in ≤ 22 s, mediana di 5 esecuzioni. La sonda ha misurato la sola enumerazione; il margine serve alla costruzione dell'albero | Protocollo di 06 §9 sulla C: reale |
| Prima scansione a cache fredda | Obiettivo da fissare in M1 dopo la misura: la sonda a freddo non costruiva l'albero (C11) | VHDX smontati e rimontati, e prima scansione dopo il riavvio (06 §9) |
| Memoria | Obiettivo: picco ≤ 400 MB per 3,3 milioni di voci con i nomi reali, da misurare in M1 (05 D5) | Picco di working set (06 §9) |
| Reattività | Obiettivo: frame p95 ≤ 16 ms durante scansione, mappa, scorrimento e letture in background; per la tabella anche p99 ≤ 33 ms | Tempi di frame strumentati in build di test (06 §9) |
| Interazioni | Obiettivi: avanzamento ≥ 4 aggiornamenti al secondo, annullamento ≤ 500 ms, ricerca ≤ 300 ms su 3 milioni di nomi | 06 §9 |
| Avvio | Primo frame interattivo dopo il riavvio del PC: misurato in M2, obiettivo 1 s | 06 §9 |
| Privacy: rete | Nessuna connessione di rete, nessuna telemetria, nessun account, nessun controllo degli aggiornamenti, nessun invio di crash | `cargo-deny` sulle dipendenze; tabella delle importazioni a ogni build di installazione; una sessione completa senza rete e con il campionamento delle connessioni, prima della v1 (06 §10) |
| Privacy: letture | Scansione in sola lettura. Letture di contenuto (risorsa di versione, MSI, directory centrale degli zip, hash) solo su richiesta e dichiarate prima, perché aggiornano la data di ultimo accesso: su C: l'aggiornamento è attivo anche se `fsutil` scrive "Disabled". Quado non apre handle in scrittura per evitarlo. Effetto dichiarato: Sensore memoria potrebbe rinviare la pulizia dei file di Download letti così. Nessuna lettura di `Zone.Identifier` | Process Monitor ed ETW (06 §3.6, §10, §11) |
| Privacy: dati salvati | Solo in `%APPDATA%\Quado`: impostazioni, registro degli interventi, log senza percorsi completi salvo scelta esplicita. La pagina "Dati e privacy" li elenca con la dimensione e li cancella. L'albero della scansione resta in memoria e si scarta alla chiusura | Process Monitor (06 §10) |
| Sicurezza: privilegi | UI mai elevata. Helper solo su richiesta, in sola lettura, con la pipe limitata al SID dell'utente; termina a fine lavoro. I comandi da amministratore (`powercfg`, `DISM`) si mostrano e si copiano, mai si eseguono | 06 §11 |
| Sicurezza: interventi | Solo i tipi di F-AC-1, da un `PianoConfermato`; mai `UninstallString`; mai eliminazione definitiva | 06 §5, §11 |
| Sicurezza: DLL | `/DEPENDENTLOADFLAG:0x800` (da verificare in M1) e `SetDefaultDllDirectories` come prima chiamata; l'helper rifiuta di partire da `Downloads` o da cartelle temporanee | 06 §11 |
| Accessibilità: tastiera | Tastiera completa, focus sempre visibile | `egui_kittest` (06 §7) |
| Accessibilità: contrasto | Testo ≥ 4,5:1 su ogni superficie e in ogni stato; elementi grafici ≥ 3:1 | Test automatico dei token (06 §7) |
| Accessibilità: colore | Nessuna informazione affidata solo al colore: stati con icona e parola, provenienza con glifi di forma diversa, non misurato con tratteggio | Snapshot (06 §7) |
| Accessibilità: movimento | Con "Effetti di animazione" disattivati in Windows nessuna animazione (04 §6.1) | Test UI (06 §7) |
| Accessibilità: lettore di schermo | Best effort: l'albero AccessKit di base di egui, con i limiti dichiarati in 05 D2. Diventa un requisito con criteri bloccanti in M0 se l'utente principale usa Assistente vocale o un altro lettore di schermo (00 §7); lo stesso vale per Desktop remoto | Snapshot dell'albero AccessKit (06 §7); prova manuale solo se l'utente principale usa un lettore |
| Linguaggio | Testi comprensibili da una persona non esperta; gli stati non si fraintendono | Test di usabilità a compiti: le domande di comprensione 1, 3, 6 e 7 di 06 §8 ricevono la risposta corretta da tutti i partecipanti |
| Compatibilità | Windows 10 22H2 x64 ora, sul PC di sviluppo; Windows 11 x64 in seguito. Al passaggio si ripetono i controlli manuali di URI e procedure (06 §5.7). Solo x64 | Controlli manuali con data |
| Distribuzione | Eseguibile unico compilato in locale (`cargo build --release`, CRT statico), copiato in `%LOCALAPPDATA%\Programs\Quado` con un collegamento nel menu Start dell'utente. Nessun installer e nessuna firma: un file compilato in locale non ha il Mark-of-the-Web e SmartScreen non interviene; il prompt UAC dell'helper mostra "Autore sconosciuto". Aggiornare significa ricompilare e reinstallare | 05 D9, 07 |
| Manutenibilità delle regole | Ogni regola ha fonte con URL ed estratto, fixture reale anonimizzata, tabella attesa e data di verifica; procedura di riverifica in 07 §5.1 | Test della base delle regole (06 §2.2) |
| Robustezza | Percorsi lunghi, nomi UTF-16 non validi, file creati e cancellati durante la scansione, cartelle molto profonde o con moltissimi figli: nessun crash, nomi conservati esatti | Alberi di prova (06 §1.2) |

## 8. Fuori dalla v1

| Funzione | Quando | Motivo |
|---|---|---|
| MSI, firma del codice, winget, Store, zip portable | Solo se Quado verrà pubblicato | Strumento personale compilato in locale: senza Mark-of-the-Web SmartScreen non interviene (`revisione/decisioni-revisione.md` §1) |
| Modalità portable | Non pianificata | Una sola sede dei dati, `%APPDATA%\Quado` |
| Palette dei comandi | v1.1 | Utile, ma non serve a nessun anello della catena; in v1 la casella cerca cartelle e file |
| "Esporta diagnostica" | Non pianificata | Costo senza un beneficio proporzionato per un solo utente |
| Benchmark pubblicati | Non pianificati | Nessun pubblico |
| CI con privilegi su più file system | Non pianificata | Test in locale sul PC di sviluppo e su VHDX (06) |
| Parser della MFT, `FSCTL_QUERY_FILE_LAYOUT`, fuzzing della MFT | v2, solo se servirà | A uno strumento personale serve leggere le cartelle negate, non la velocità: l'helper lo fa con l'enumerazione (05 D4) |
| ARM64, Linux | Non pianificati | Restano come nota di portabilità (05 D11) |
| Esecuzione di `UninstallString` e `QuietUninstallString` | Mai in v1 | Voci in HKCU scrivibili dall'utente, voci `MsiExec /I` che aprono la manutenzione invece di disinstallare, percorsi con spazi senza virgolette (02 §15.7) |
| Firma Authenticode degli installatori | Non in v1 | Richiede un'apertura per file e la verifica rischia di usare la rete |
| `Zone.Identifier` con HostUrl | Non in v1 | Rivela da dove è stato scaricato un file e copre pochi installatori |
| Marcatori NSIS, Inno, Squirrel | Non in v1 | Costano letture e restano ambigui |
| BAM, Prefetch, Amcache, SRUM come dati d'uso | Non in v1 | Richiedono l'amministratore e hanno un carattere forense |
| Jump list, RecentDocs, data di ultimo accesso come dati d'uso | Non in v1 | Jump list e RecentDocs contengono nomi di documenti; l'ultimo accesso è rumore |
| Launcher diversi da Steam ed Epic | Dopo la v1, se compariranno sul PC di sviluppo | Copertura allineata a ciò che c'è sul PC di sviluppo (C14) |
| Storico delle scansioni e "cosa è cresciuto" | v1.1 | Ha valore dal secondo utilizzo; richiede di salvare percorsi su disco, da progettare con la privacy (attivazione esplicita, cancellazione) |
| Esportazione CSV o JSON della scansione | v1.1 | Non serve a capire |
| Esecuzione dei comandi ufficiali dall'app, con l'output visibile | v1.1 | In v1 si mostrano e si copiano: meno codice che esegue programmi di terzi |
| Esclusione di percorsi dalla scansione | v1.1 | Raramente necessaria con una scansione completa che ha come obiettivo 22 s |
| Copie identiche su tutto il volume | v2 | In v1 l'hash esiste solo nella lente Download, su richiesta (F-DL-4); su tutto il volume richiederebbe di leggere molti file |
| Indice persistente aggiornato tramite USN journal | v2 | La scansione completa è già veloce; l'indice aggiunge complessità e dati salvati |
| Analisi del contenuto dei file oltre le letture su richiesta della lente Download | Non pianificata | Nessun caso d'uso la richiede |
