# 07 · Piano di sviluppo (v2, dopo la revisione)

Stato: v2 del 13/10/2026, proposta da approvare. Riscritta dopo la revisione critica (`10-revisione-critica.md`) secondo `revisione/decisioni-revisione.md`, che prevale in caso di dubbio. I requisiti si citano con gli ID canonici di `revisione/decisioni-revisione.md` §9 ter, i test con le sezioni di 06, i rischi con gli ID di 08. I numeri misurati qui non si ripetono.

Non ci sono durate in settimane: dipendono dal tempo dedicato al progetto e da quanto lavoro fa Claude. Ogni milestone ha uno sforzo relativo e criteri di uscita verificabili. Una milestone è chiusa quando i suoi criteri sono verificati nella forma prevista da 06, non prima.

| Sforzo | Significato |
|---|---|
| S | Una parte piccola, senza prove su VHDX né misure ripetute |
| M | Un crate o una vista completa, con i suoi test |
| L | Più crate o viste insieme, con prove su VHDX o misure ripetute sul PC di sviluppo |

## Cosa cambia rispetto alla v1

| Milestone | v1 | v2 |
|---|---|---|
| M0 | Tabella da 5 milioni di righe; nodi AccessKit, Assistente vocale e Desktop remoto tra i criteri | Tabella da 3,5 milioni di righe; Assistente vocale e Desktop remoto bloccanti solo se usati |
| M1 | Modello e scansione; picco di memoria ≤ 350 MB | In più bilancio, dati di riferimento e verifiche tecniche di 10 §7; picco ≤ 400 MB misurato con i nomi reali |
| M2 | Bilancio per gestore, italiano e inglese, palette comandi | Bilancio non classificato; solo italiano; palette nella v1.1; il bilancio per gestore passa a M3 (C9) |
| M3 | Classificazione e primo insieme di regole | In più record di raccomandazione, cinque stati, vista Windows e bilancio per gestore |
| M4 | Interventi | Lente Download e Programmi |
| M5 | Modalità MFT con parser e fuzzing | Interventi e verifica; il parser MFT esce dalla v1 |
| M6 | Firma, MSI, winget, benchmark pubblicati | Helper con privilegi, rifinitura, test di usabilità, prova senza rete, build locale |

## 1. Milestone della v1

| Milestone | Contenuto (decisioni §9 ter) | Sforzo |
|---|---|---|
| M0 | Verifica dello stack | M |
| M1 | Scansione, modello, bilancio, dati di riferimento e verifiche tecniche | L |
| M2 | UI di base: Panoramica con bilancio non classificato e top list, Esplora, File grandi, ispettore delle misure, temi | L |
| M3 | Classificazione, record di raccomandazione e cinque stati, vista Windows, prime regole, bilancio per gestore | L |
| M4 | Lente Download e Programmi | M |
| M5 | Interventi e verifica | L |
| M6 | Helper con privilegi, rifinitura, test di usabilità, prova senza rete, build locale | M |

### 1.1 M0 · Verifica dello stack (sforzo M)

Contenuto:
- toolchain Rust aggiornata con `rustup update` e fissata con `rust-toolchain.toml`, dopo il permesso del §8; repository Git del progetto; `cargo-deny` configurato dal primo commit (06 §10);
- prototipo egui/eframe con una tabella ad albero appiattita e uno scroller che mappa la barra su un indice di riga (05 D2), una mappa ad albero con 20.000 aree, testo in IBM Plex a 100% e 150%, navigazione da tastiera, catena di avvio del renderer wgpu DX12, poi WARP, poi glow;
- tempi di frame strumentati (p50, p95, p99) e un'opzione che forza WARP;
- controllo dello stato del ripiego sui font di sistema annunciato per egui 0.37 (05 D2).

Criteri di uscita, tutti bloccanti (decisioni §8, 05 D2), misurati sul PC di sviluppo:
1. tabella da 3,5 milioni di righe: frame p95 ≤ 16 ms e p99 ≤ 33 ms durante uno scorrimento continuo dall'inizio alla fine e durante salti con la barra;
2. mappa con 20.000 aree: frame p95 ≤ 16 ms durante hover e selezione, a 1080p;
3. testo a 100% e 150%: screenshot affiancati allo stesso testo reso con ClearType da un controllo nativo, approvati con un confronto a vista;
4. tastiera: ogni azione del prototipo raggiungibile da tastiera, verificato con `egui_kittest`;
5. WARP forzato: il prototipo si avvia e resta utilizzabile; i tempi di frame si registrano ma non bloccano.

Condizioni:
- Assistente vocale (o un altro lettore di schermo) e Desktop remoto non sono criteri se il proprietario del progetto conferma che non li usa (§8). Se ne usa uno, il criterio corrispondente diventa bloccante.
- Misure ed esiti si registrano con la data; l'esito su D2 si scrive in 05.
- Se M0 passa, D2 è confermata e il piano B si abbandona. Se un criterio fallisce senza una correzione ragionevole, si sceglie tra WinUI 3 e Slint ripetendo sul candidato solo i criteri falliti. Il core non cambia (05 D2, D10).

**Stato (13/10/2026):** M0 chiuso: testo approvato, D2 confermata.

### 1.2 M1 · Scansione, modello, bilancio, dati di riferimento e verifiche tecniche (sforzo L)

Contenuto:
- `quado-model`: arena con indici `u32`, nomi in un pool `u16`, array laterale degli ID file durante la scansione, deduplica degli hard link, aggregazione, riscansione di un sottoalbero (05 D5);
- `quado-scan-win`: elenco dei volumi, enumerazione con `FileIdExtdDirectoryInfo`, correzione dei file WOF, indici delle cartelle, dimensione della `$MFT`, segnaposto cloud, reparse point, cartelle negate, spazio libero a inizio e a fine scansione (05 D4); eventi di avanzamento e arresto su richiesta nel core, la cui interfaccia arriva in M2;
- calcolo del bilancio secondo 03 §6.4: usato a inizio e fine, allocato unico letto, metadati misurati, non attribuito, deriva;
- oracolo indipendente (06 §1.1), generatore degli alberi di prova (06 §1.2), `cargo xtask vhdx` (06 §1.3), harness delle prestazioni (06 §9);
- comando `xtask` che rimisura sul PC di sviluppo i valori di riferimento del volume (bilancio, metadati, hard link, conteggi), con byte esatti, GiB e data (05 D10);
- le verifiche tecniche del §3.

In M1 non c'è interfaccia: i risultati si leggono dai test e dai comandi `xtask`.

Criteri di uscita:
1. F-SC-1, F-SC-2, F-SC-6, F-SC-7, F-SC-8, F-SC-9 e F-SC-10 verdi secondo 06 §1.1, §1.2 e §1.3, e nessun handle in scrittura durante la scansione (06 §11);
2. la parte di F-OV-1 che non dipende dalla UI: test "Residuo su volume fermo", "Errore iniettato" e "Deriva" di 06 §1.4, con metafile attesi e tolleranza fissati da tre misure e scritti nel test;
3. scansione di C: a cache calda ≤ 22 s, mediana di 5 esecuzioni (06 §9);
4. picco di memoria ≤ 400 MB per 3,3 milioni di voci con i nomi reali (06 §9). Se lo supera, si applicano le riduzioni di 05 D5 nell'ordine scritto e si rimisura; se resta sopra, si decide prima di M2;
5. tempo della prima scansione a freddo misurato e obiettivo scritto nella riga "Scansione a cache fredda" di 06 §9 (C11);
6. tutte le verifiche del §3 eseguite, con l'esito registrato con la data e, quando cambia una decisione, scritto in 05 e in 08;
7. valori di riferimento rimisurati con il comando `xtask`, con le misure di Quado accanto a quelle della sonda.

**Stato (13/10/2026):** criteri 1, 2, 3, 4 e 7 superati; criterio 6: sette verifiche su otto. Resta da fare: la misura a freddo dopo un riavvio (criterio 5, §3 riga 8), con `cargo xtask freddo-c`.

### 1.3 M2 · UI di base (sforzo L)

Contenuto:
- `quado-ui` con i token di 04 §6.2 e §6.3, tema chiaro, scuro e "come il sistema", IBM Plex incorporato, lingue con Fluent (05 D13): `it.ftl` ed `en.ftl` completi, scelta in Impostazioni, file modificabili in `%APPDATA%\Quado\lingue`, `cargo xtask lingue`, numeri e date dal formato dell'area geografica di Windows;
- correzione del grigio di "Non classificato" rispetto al verde acqua di "Windows" in tema chiaro, prima di disegnare la barra del bilancio (04 §6.2, RS23 in 08);
- Panoramica: bilancio non classificato (usato, liberi, non attribuito a tratteggio, deriva come incertezza), top list "Dove finisce lo spazio", card dei conti con le liste "Osservato" e "Possibile, non misurabile senza amministratore";
- Esplora: mappa, tabella virtualizzata, percorso a briciole, ricerca e filtri; vista File grandi;
- ispettore delle sole misure: dimensioni, conteggi, date, tipo dall'estensione, "Apri in Esplora file", "Copia percorso", "Proprietà", "Verifica questa voce";
- avanzamento e annullamento della scansione; barra di stato.

Criteri di uscita:
1. F-SC-4, F-SC-5, F-OV-2, F-OV-3, F-EX-1, F-EX-2, F-EX-3, F-EX-4, F-EX-5, F-IN-3, F-IN-5, F-IN-6, F-GE-1, F-GE-2 e F-GE-3 verdi secondo 06 §1.2, §1.4, §7, §9 e §11;
2. tempi di 06 §9: tabella con frame p95 ≤ 16 ms e p99 ≤ 33 ms con 3,5 milioni di righe e con una cartella da 1 milione di figli; mappa con frame p95 ≤ 16 ms con 20.000 aree; ricerca ≤ 300 ms su 3 milioni di nomi; almeno 4 aggiornamenti al secondo durante la scansione; annullamento ≤ 500 ms;
3. snapshot di ogni schermata nei due temi a 100%, 150% e 200%; contrasto ≥ 4,5:1 per ogni coppia di testo e superficie in ogni stato, compreso `text-3` su `select` in tema scuro (04 §6.3);
4. controllo automatico dei colori dei dati con la simulazione delle discromatopsie (06 §7, richiesto da 04 §6.2), superato da tutte le coppie di tinte adiacenti nel bilancio e nella mappa, grigio compreso;
5. primo frame interattivo dopo il riavvio misurato (06 §9, obiettivo 1 s) e registrato con la data.

Dopo M2 Quado è un analizzatore usabile senza privilegi, ancora senza spiegazioni.

**Stato (13/10/2026):** criteri 1-4 superati. Resta da fare: la misura del primo frame dopo un riavvio (criterio 5), che `cargo xtask freddo-c` fa insieme alla scansione a freddo di M1. Colore e filtro per gestore restano a M3, "Ripeti come amministratore" a M6, come nella tabella del §4.

### 1.4 M3 · Classificazione, record e cinque stati (sforzo L)

Contenuto:
- `quado-kb`: motore delle regole TOML ed estrattori letti durante la scansione (voci di disinstallazione, MSIX, Steam, Epic, CapCut, UserAssist con `Start_TrackProgs`, configurazione in sola lettura) (05 D6);
- record di raccomandazione con i campi obbligatori di 03 §6.1 e i cinque stati; `ProvaDiGestibilita` distinta da `ProvaDiIdentita`; provenienza per singola affermazione con i criteri di affidabilità di 03 §6.2;
- l'insieme iniziale delle regole del §7, scritte una per volta, ciascuna con fonte, estratto, fixture reale anonimizzata, data di verifica e tabella attesa;
- vista Windows e sistema; Cestino misurato in due modi; lettura della configurazione;
- testi delle regole con una chiave per ogni testo visibile, in `regole/it.ftl` e `regole/en.ftl` (05 D13): una regola entra solo con entrambe le lingue;
- ispettore completo: fatti con glifo, "Perché è così grande", "Cosa puoi fare", regola con data e "Vedi regola"; card interventi; colore del gestore nella mappa e bilancio per gestore.

Le azioni (spostamento nel Cestino, apertura di procedure, comandi copiati) arrivano con M5. In M3 il record le descrive e basta.

Criteri di uscita:
1. F-OV-1 completo con la riga "Bilancio per gestore" di 06 §1.4, e F-OV-4, F-IN-1, F-IN-2, F-IN-4, F-SY-1, F-SY-2 e F-SY-3 verdi secondo 06 §1.4, §2.1, §2.2, §2.4 e la riga "Cestino misurato in due modi" di 06 §5.3;
2. ogni regola del §7 supera la sua tabella attesa (06 §2.1) e il test sulla base delle regole (06 §2.2);
3. test di proprietà di 06 §2.2 verdi, compreso "Regole da riverificare" con date simulate oltre 6, 11 e 12 mesi (§5.1);
4. ogni estrattore provato con un caso valido, uno troncato e uno con formato cambiato (06 §2.3);
5. lettura di ibernazione, avvio rapido e azione per batteria critica con le API di alimentazione confrontata con `powercfg /a` (05 D6, 06 §2.4);
6. controllo manuale con data: il caso CapCut del PC di sviluppo è spiegato come in decisioni §3 (stato Da verificare, ultime due versioni tenute ed escluse dalla stima, testo "(dedotto)").

Dopo M3 Quado spiega cosa occupa il disco e cosa si può fare, senza ancora agire.

**Stato (13/10/2026):** criteri 1-6 superati. 25 regole: quelle del §7 (le cache dei browser sono tre regole, una per fonte) e 5 di base da confermare (programmi registrati, dati delle app MSIX, cartelle dell'utente, cartella di Windows, ipotesi dal nome). Resta da fare: controllo manuale degli estratti delle fonti (RS10), dall'elenco Regole dell'app.

### 1.5 M4 · Lente Download e Programmi (sforzo M)

Contenuto:
- lente Download su `Downloads` e su qualunque cartella dell'utente: famiglie per estensione, suffisso "(N)" come solo filtro, metadati degli installatori su richiesta in background con cache, confronto tra zip e cartella estratta, hash delle copie identiche su richiesta, download incompleti con l'interrogazione di Restart Manager (05 D6);
- vista Programmi: elenco, spazio solo con prova del percorso (prefisso più lungo), dichiarato contro misurato, uso noto, residui;
- disinstallazione tramite interfaccia ufficiale, con il lanciatore a valori tipizzati in `quado-actions` (05 D7).

Criteri di uscita:
1. F-DL-1, F-DL-2, F-DL-3, F-DL-4, F-DL-5 e F-DL-6 verdi secondo 06 §3.1-3.6, compreso l'effetto sull'ultimo accesso provato su VHDX (06 §3.6);
2. F-AP-1, F-AP-2, F-AP-3, F-AP-4, F-AP-5 e F-AP-6 verdi secondo 06 §4 e §11;
3. frame della UI p95 ≤ 16 ms durante la lettura degli installatori e durante l'hash (06 §9);
4. costo della lettura delle risorse di versione misurato con il protocollo di 06 §9 e registrato con la data (10 §7);
5. fonti dell'uso dei programmi riconfermate sul PC di sviluppo con un controllo manuale con data (10 §7, `revisione/prove-windows.md` §A).

**Stato (14/10/2026):** criteri 1, 3, 4 e 5 superati; criterio 2 superato salvo il controllo manuale con data degli URI di disinstallazione (F-AP-5), che resta da fare. Quattro punti da decidere: confronto dei possibili residui, avviso sui file dell'utente prima di disinstallare CapCut, Restart Manager all'apertura della lente, lente ricorsiva.

### 1.6 M5 · Interventi e verifica (sforzo L)

Condizione per cominciare: le verifiche del Cestino del §3 (righe 1-4) sono chiuse (10 §8).

Contenuto:
- `quado-actions`: valore `Simulazione`, tipo `PianoConfermato` costruibile solo dalla conferma, esecuzione che consuma il piano (05 D7);
- controlli prima dello spostamento: elementi protetti con l'eccezione via regola, unità senza Cestino e `NukeOnDelete`, capacità del Cestino, Sensore memoria, Restart Manager, riferimenti, stima con rilettura degli attributi;
- spostamento con `IFileOperation` e controllo in `PreDeleteItem`; ricontrollo di ogni elemento subito prima; riepilogo; registro degli interventi;
- procedure di Windows ("Apri il Cestino", "Svuota il Cestino" con il dialogo di Windows, URI di Impostazioni), comandi ufficiali mostrati e copiati, pulsante "Ho eseguito la procedura";
- verifica: misura di partenza, nuove misure, stimato contro variazione del volume, riscansione dei sottoalberi toccati; vista Interventi.

Criteri di uscita:
1. F-AC-1, F-AC-2, F-AC-3, F-AC-4, F-AC-5, F-AC-6, F-AC-7, F-AC-8 e F-AC-9 verdi secondo 06 §5.1-5.7;
2. F-VE-1, F-VE-2, F-VE-3, F-VE-4 e F-VE-5 verdi secondo 06 §6.1 e §6.2;
3. tolleranza di 06 §6.1 confermata o corretta con le prime misure e scritta nel test;
4. costo della stima in simulazione misurato (05 D7) e registrato con la data;
5. URI diretto di Impostazioni › File temporanei verificato sul PC di sviluppo (05 D7, 06 §5.7);
6. controllo manuale con data, su C:, di un intervento reale a basso rischio, con Sensore memoria, SilentCleanup e Defender attivi (06 §6.2).

**Stato (14/10/2026):** criteri 2, 3 e 4 superati; criterio 1 superato salvo i controlli manuali con data delle pagine di Impostazioni (06 §5.7) e dei riferimenti con HKCU vero (06 §5.5, con un consenso esplicito); criterio 5: non esiste un URI diretto per File temporanei su Windows 10 22H2, si apre Archiviazione. Condizione di partenza: righe 1-3 del §3 chiuse, riga 4 salvo il confronto a vista. Resta da fare (controllo manuale): il controllo a vista di Archiviazione (criterio 5), l'intervento reale del criterio 6 e il confronto a vista della riga 4.

Dopo M5 la catena è completa: individuare, comprendere, valutare, decidere, agire, verificare.

### 1.7 M6 · Helper, rifinitura, usabilità, prova senza rete, build locale (sforzo M)

Contenuto:
- helper elevato su richiesta (05 D3): stesso eseguibile, `SeBackupPrivilege` solo sulle cartelle negate, spazio delle copie shadow con `Win32_ShadowStorage`, archiviazione riservata con il comando scelto in M1, cestini di altri SID, profili altrui in forma aggregata; pipe con nome casuale e DACL limitata all'utente;
- "Ripeti come amministratore" nella card dei conti;
- rifinitura: correzioni emerse dal test di usabilità, testi di 04 §6.8, messaggi di errore e stati vuoti;
- test di usabilità a compiti (06 §8); prova senza rete (06 §10); installazione con la build locale (§6).

Criteri di uscita:
1. F-SC-3 verde secondo 06 §1.5 e §11;
2. test di usabilità: il proprietario del progetto completa i cinque compiti senza aiuto; le domande 1, 3, 6 e 7 ricevono la risposta corretta da tutti i partecipanti; ogni risposta sbagliata ha portato a correggere il testo e a ripetere la domanda (06 §8);
3. sessione senza rete senza errori e sessione in rete con zero connessioni; scritture solo in `%APPDATA%\Quado` e nel Cestino; nessuna apertura di `:Zone.Identifier` (06 §10);
4. screenshot del testo a confronto con ClearType approvati di nuovo (06 §7);
5. tutti i test di 06 verdi e nessuna riga scoperta nella tabella di 06 §13;
6. Quado installato con `cargo xtask installa`, dopo il controllo delle importazioni (§6, 06 §10);
7. 08 aggiornato: ogni rischio marcato "aperto" ha un livello oppure il motivo per cui resta aperto.

**Stato (16/10/2026):** criteri 1, 5, 6 e 7 superati (helper provato elevato su `R:`, su C: e dall'app; Quado installato); criterio 3 superato con la rete. Resta da fare (controllo manuale): la sessione senza rete (criterio 3), il test di usabilità (criterio 2) e l'approvazione delle immagini del confronto con ClearType (criterio 4).

Chiusa M6, la v1 è pronta.

## 2. Perché quest'ordine

- M0 prima di tutto, perché la scelta dell'interfaccia è l'incertezza più costosa se scoperta tardi (R26 e RS13 in 08). Scoprirla dopo M2 vorrebbe dire rifare tutta la UI. Il core in Rust non dipende dalla UI (05 D10), quindi un fallimento di M0 non fa perdere lavoro sul core.
- M1 subito dopo, per tre motivi. Modello e scansione servono a tutte le viste. Le verifiche sul Cestino decidono se F-AC-5 e F-AC-7 si possono realizzare come sono scritti, e farle all'inizio lascia il tempo di cambiare progetto prima di M5 (10 §8). La misura della memoria con i nomi reali può cambiare il layout di 05 D5 prima che altro codice ne dipenda.
- M2 prima della classificazione, perché dà presto un analizzatore usabile e mette alla prova modello e UI con i dati veri del PC di sviluppo. Il bilancio per gestore resta a M3 perché richiede la classificazione (C9).
- M3 prima di Download, Programmi e interventi, perché ogni proposta nasce da un record di raccomandazione: i tipi del record, con la prova di gestibilità distinta da quella di identità, devono esistere prima di qualsiasi azione.
- M4 prima di M5, perché la lente Download produce i primi casi Gestibile a basso rischio (copie identiche, download incompleti). Su quei casi si provano per primi gli interventi di M5 e il compito 5 del test di usabilità (06 §8).
- M5 dopo ciò da cui dipende: i record (M3), la rilettura degli attributi (M1), gli esiti delle verifiche sul Cestino (M1).
- M6 per ultimo, perché l'helper completa solo i conti della card ed è l'unico componente elevato: senza di lui la catena funziona già. Il test di usabilità e la prova senza rete hanno senso solo sull'app completa.

## 3. Verifiche tecniche di M1

Da `10-revisione-critica.md` §7. Le prove su VHDX richiedono l'amministratore per creare e montare i volumi: una persona davanti al PC approva il prompt UAC a ogni sessione (06, "Dove si eseguono i test"; 00 §7 punto 6). Le prove scrivono solo dentro il VHDX. Gli esiti si registrano con la data (le uscite delle prove sul Cestino, righe 1-3, sono in `m1/cestino-verifiche.md` e `m1/cestino-verifiche-bis.md`); se cambiano una decisione, vanno anche in 05 e in 08.

| # | Verifica | Come | Cosa decide | Riferimenti |
|---|---|---|---|---|
| 1 | Svuotamento del Cestino con una cartella `$R` senza `$I` | Su un VHDX NTFS: spostare una cartella nel Cestino, togliere il suo `$I`, svuotare con il dialogo di Windows, riscansionare `$Recycle.Bin` | Il testo dell'ispettore sulle cartelle orfane (F-SY-2, 04 §6.8). Le orfane restano un'anomalia senza cifra recuperabile (decisioni §3) | IP10, C2, RS25 |
| 2 | `PreDeleteItem` e `TSF_DELETE_RECYCLE_IF_POSSIBLE` | Su un VHDX con il Cestino ridotto: spostare con `IFileOperation` e un sink un elemento più grande della capacità; registrare i flag ricevuti e quale valore di ritorno annulla il singolo elemento senza fermare gli altri | Come si realizza F-AC-5. Se non funziona resta il solo controllo di capacità e si riscrive R8 (05 D7) | IP9, R8 |
| 3 | Conteggio della capacità del Cestino | Sullo stesso VHDX: riempire il Cestino vicino alla capacità con file di dimensione logica e allocata diverse (compressi, sparse) e con una cartella orfana; spostare un altro elemento e osservare quali elementi Windows elimina | La formula del controllo di F-AC-7: dimensione logica o allocata, cartelle orfane contate o no (decisioni §9 bis). Fino alla verifica il controllo usa la stima più prudente (05 D7) | RS1 |
| 4 | Lettura senza privilegi di Sensore memoria, `MaxCapacity` e `NukeOnDelete` | Lettura da utente standard sul PC di sviluppo; confronto con Impostazioni e con le Proprietà del Cestino (06 §2.4) | Se F-SY-3 e F-AC-7 funzionano senza privilegi come previsto | RS1, RS2 |
| 5 | `/DEPENDENTLOADFLAG:0x800` | Build dell'eseguibile; `DependentLoadFlags` = 0x800 nel load config (`dumpbin /loadconfig` o `xtask`); una DLL esca nella cartella dell'eseguibile non viene caricata (Process Monitor) (06 §11) | Se il flag del linker, con `SetDefaultDllDirectories`, basta all'helper | RS11 |
| 6 | Comando per lo stato e la dimensione dell'archiviazione riservata | Prova da amministratore, con un consenso esplicito, dei candidati: `DISM /Online /Get-ReservedStorageState` (02 §7) e `fsutil storagereserve query C:` [I]. Preferito un risultato non localizzato, come per `Win32_ShadowStorage` (05 D3) | Cosa raccoglie l'helper in M6 per la card dei conti (F-SC-3, F-OV-3) | IP12 |
| 7 | Memoria con i nomi reali | Scansione di C: con il modello vero; picco di working set con il protocollo di 06 §9 | Obiettivo ≤ 400 MB per 3,3 milioni di voci; riduzioni di 05 D5 se serve | IP4, R23 |
| 8 | Tempo della prima scansione a freddo | C: dopo un riavvio, 5 misure con un riavvio prima di ciascuna; in più i VHDX smontati e rimontati (06 §1.3) | L'obiettivo a freddo di 06 §9 | C11 |

## 4. Requisiti per milestone e dopo la v1

### 4.1 Assegnazione degli ID

Ogni ID di `revisione/decisioni-revisione.md` §9 ter è assegnato a una sola milestone: quella in cui il requisito si chiude con i test di 06 (tabella di copertura in 06 §13). Alcune parti si costruiscono prima, e la colonna Note lo dice.

| ID | Requisito | Milestone | Note |
|---|---|---|---|
| F-SC-1 | Elenco dei volumi | M1 | - |
| F-SC-2 | Scansione non elevata in sola lettura | M1 | - |
| F-SC-3 | Completamento con privilegi (helper) | M6 | Il comando dell'archiviazione riservata si sceglie in M1 (§3, riga 6) |
| F-SC-4 | Avanzamento | M2 | Gli eventi nascono nel core in M1 |
| F-SC-5 | Annullamento e risultato parziale | M2 | Arresto nel core in M1; banner "Scansione interrotta" in M2 |
| F-SC-6 | Cartelle negate elencate | M1 | - |
| F-SC-7 | Hard link e reparse point | M1 | - |
| F-SC-8 | Dimensione logica e allocata, correzione WOF | M1 | - |
| F-SC-9 | Segnaposto cloud | M1 | - |
| F-SC-10 | File system | M1 | La dicitura "senza garanzie" compare nella UI in M2 |
| F-OV-1 | Bilancio (03 §6.4) | M3 | Calcolo e test su VHDX in M1; barra non classificata in M2; ripartizione per gestore in M3 |
| F-OV-2 | Top list con regola di non sovrapposizione | M2 | - |
| F-OV-3 | Card dei conti: osservato e possibile | M2 | I valori dell'helper si aggiungono in M6 |
| F-OV-4 | Card interventi, solo da record | M3 | Le voci portano alla simulazione quando c'è M5 |
| F-EX-1 | Mappa | M2 | Il colore del gestore arriva in M3 |
| F-EX-2 | Tabella virtualizzata | M2 | - |
| F-EX-3 | Navigazione e tastiera | M2 | - |
| F-EX-4 | Ricerca e filtri | M2 | - |
| F-EX-5 | Vista File grandi | M2 | - |
| F-IN-1 | Fatti con provenienza ciascuno | M3 | In M2 solo fatti misurati |
| F-IN-2 | Perché è grande | M3 | - |
| F-IN-3 | Tipo dall'estensione, senza leggere il contenuto | M2 | - |
| F-IN-4 | Cosa puoi fare, dal record | M3 | - |
| F-IN-5 | Apri in Esplora file, Copia percorso, Proprietà | M2 | - |
| F-IN-6 | Verifica questa voce | M2 | Usa la riscansione di un sottoalbero di M1 |
| F-DL-1 | Famiglie per tipo, su qualunque cartella dell'utente | M4 | - |
| F-DL-2 | Metadati degli installatori su richiesta e corrispondenza | M4 | - |
| F-DL-3 | Suffisso "(N)" solo come filtro | M4 | - |
| F-DL-4 | Copie identiche con hash su richiesta | M4 | Lo spostamento delle copie arriva con M5 |
| F-DL-5 | Archivio confrontato con la cartella estratta | M4 | - |
| F-DL-6 | Download incompleti non in uso | M4 | Usa l'interrogazione di Restart Manager, che in M5 serve anche a F-AC-8 |
| F-AP-1 | Elenco: registro, MSIX, Steam, Epic | M4 | Gli estrattori nascono in M3 per la classificazione |
| F-AP-2 | Spazio solo con prova del percorso | M4 | La regola del prefisso più lungo vale già nella classificazione di M3 |
| F-AP-3 | Dichiarato contro misurato | M4 | - |
| F-AP-4 | Uso noto, mai "inutilizzato" | M4 | - |
| F-AP-5 | Disinstallazione tramite interfaccia ufficiale | M4 | Lanciatore a valori tipizzati in `quado-actions` |
| F-AP-6 | Residui | M4 | - |
| F-SY-1 | Vista Windows e sistema | M3 | - |
| F-SY-2 | Cestino misurato in due modi, orfani come anomalia | M3 | Il testo sulle orfane dipende dal §3, riga 1 |
| F-SY-3 | Configurazione in sola lettura | M3 | Lettura senza privilegi verificata in M1 (§3, riga 4) |
| F-AC-1 | Tipi di intervento | M5 | - |
| F-AC-2 | Simulazione obbligatoria e piano confermato | M5 | - |
| F-AC-3 | Conferma, Esc e riepilogo | M5 | - |
| F-AC-4 | Elementi protetti ed eccezione via regola | M5 | - |
| F-AC-5 | Mai eliminazione definitiva (`PreDeleteItem`) | M5 | Dipende dal §3, riga 2 |
| F-AC-6 | Registro con stimato e ottenuto | M5 | - |
| F-AC-7 | Capacità del Cestino e Sensore memoria | M5 | Dipende dal §3, righe 3 e 4 |
| F-AC-8 | Restart Manager | M5 | - |
| F-AC-9 | Riferimenti in registro, collegamenti e attività pianificate | M5 | - |
| F-VE-1 | Misura di partenza | M5 | - |
| F-VE-2 | Nuove misure | M5 | - |
| F-VE-3 | Stimato contro ottenuto, con spiegazione oltre il 5% | M5 | - |
| F-VE-4 | Aggiornamento del modello con riscansione | M5 | - |
| F-VE-5 | Vista Interventi | M5 | - |
| F-GE-1 | Lingue: italiano e inglese, altre aggiungibili (05 D13) | M2 | Testi delle regole in M3 |
| F-GE-2 | Temi e contrasto | M2 | - |
| F-GE-3 | Unità come Esplora file sul singolo file, SI a scelta | M2 | - |

Conteggio: M1 7 requisiti, M2 15, M3 8, M4 12, M5 14, M6 1; totale 57, come le righe di 06 §13. M0 non chiude requisiti: decide lo stack.

### 4.2 v1.1

| Funzione | Contenuto | Condizione |
|---|---|---|
| Palette dei comandi | Ctrl+K con "vai a", "cerca file", "apri procedura" (04 §3) | v1 stabile |
| Storico delle scansioni e "cosa è cresciuto" | Opt-in; salva solo un riassunto per scansione (05 D5); confronto tra due scansioni | Progetto privacy approvato prima di scrivere percorsi su disco (R21 in 08 §8). Con lo storico si decide anche se salvare la cache dei metadati degli installatori (05 D6) |
| Export | Tabella, File grandi e Programmi in CSV e JSON | Formato documentato |
| Esecuzione dei comandi ufficiali | Dall'app, con l'output visibile, al posto della sola copia negli appunti | Cambia la regola per cui Quado non crea processi oltre all'helper (05 D7, 06 §5.1): prima servono una revisione di D7 e nuovi test di architettura |
| Esclusioni | Percorsi esclusi dalla scansione o dalle viste | Il bilancio dichiara lo spazio escluso a parte, così il non attribuito resta corretto (F-OV-1) |

### 4.3 v2

| Funzione | Contenuto | Condizione |
|---|---|---|
| USN e MFT | Indice persistente aggiornato con l'USN journal; lettura diretta della MFT e `FSCTL_QUERY_FILE_LAYOUT` | Solo se servirà: per esempio una scansione non elevata molto oltre l'obiettivo su un PC futuro, o la necessità di contare i flussi alternativi (05 D4). Con la MFT tornano il parser, il fuzzing e il rischio R5 (08 §8) |
| Duplicati fuori da Download | Confronto con hash esteso ad altre cartelle | Stesse regole di F-DL-4: solo su richiesta, solo tra file della stessa dimensione, mai sui segnaposto |
| Barra del titolo su misura | Disegnata da Quado, con Snap Layouts di Windows 11 gestiti tramite `WM_NCHITTEST` con `HTMAXBUTTON` (05 D2, 04 §5) | Eventuale: solo se porta un beneficio concreto che la barra di sistema non dà |
| Linux | Solo valutazione, sulla base della nota di 05 D11 | Nessun lavoro nella v1 |

ARM64 resta una nota di portabilità (decisioni §1), senza una versione prevista.

## 5. Manutenzione

### 5.1 Riverifica delle regole

Ogni regola ha una data di verifica, visibile nell'ispettore e nella conferma (05 D6). Il comportamento nel tempo, provato da 06 §2.2 "Regole da riverificare":

| Tempo dalla data di verifica | Cosa succede |
|---|---|
| Fino a 6 mesi | Nessun avviso |
| Oltre 6 mesi | La regola conta nel banner "N regole da riverificare" della Panoramica, con un solo pulsante che apre l'elenco delle regole con fonte e data |
| Oltre 11 mesi | Ispettore e conferma degli elementi spiegati dalla regola dicono: "Regola verificata il {data}. Se non viene riverificata entro il {data + 12 mesi}, non proporrà più interventi." |
| Oltre 12 mesi | La regola smette di proporre interventi. I fatti dedotti dalla regola scendono ad affidabilità bassa (03 §6.2), quindi il colore del gestore diventa il grigio "ipotesi". Il record non ha azioni né procedure, l'elemento non compare nella card interventi e lo stato è Non sappiamo. Il testo dice perché: "Regola non riverificata dal {data}: Quado mostra la spiegazione come ipotesi e non propone interventi." |

Gli elementi protetti restano protetti anche con una regola scaduta, perché F-AC-4 non dipende dalle regole. L'eccezione a F-AC-4 per le versioni di CapCut decade con la sua regola.

Procedura di riverifica, eseguita da una persona (05 D6, RS10 in 08):
1. aprire l'URL della fonte nel browser;
2. controllare che l'estratto citato dalla regola sia ancora presente e dica la stessa cosa;
3. controllare l'intervallo di versioni della regola contro la versione installata sul PC, letta dall'estrattore;
4. se la struttura delle cartelle è cambiata, produrre una nuova fixture reale anonimizzata (06 §2.3) e aggiornare la tabella attesa (06 §2.1);
5. eseguire i test delle regole (06 §2.1, §2.2, §2.3);
6. solo se i passi 2-5 sono andati a buon fine, aggiornare la data di verifica; poi ricompilare e reinstallare (§6).

Se la fonte non c'è più o dice altro, la regola si corregge (stato, metodo, controlli, tabella attesa) oppure si toglie; la data non si aggiorna. Un'AI può aiutare a cercare le fonti, ma estratto e controllo restano alla persona.

Quando: quando compare il banner; dopo un aggiornamento importante di un'app che ha una regola (CapCut, Steam, Epic, npm, pnpm); al passaggio a Windows 11 (§5.3).

### 5.2 Aggiornamento delle dipendenze

- Versioni fissate: `Cargo.lock` nel repository, toolchain in `rust-toolchain.toml` (05 D12).
- Si aggiorna solo quando serve: un avviso di sicurezza segnalato da `cargo-deny`, un errore che tocca Quado, una funzione necessaria, un cambiamento di Windows. Nessun aggiornamento periodico.
- egui, eframe ed `egui_kittest` si aggiornano insieme.
- Procedura:
  1. snapshot di tutte le schermate con la versione attuale (06 §7);
  2. aggiornamento e `cargo-deny` (06 §10);
  3. tutti i test;
  4. snapshot con la nuova versione e confronto: ogni differenza si guarda e si approva una per una;
  5. se cambiano egui o il renderer, si ripetono le misure di tabella e mappa (06 §9) e gli screenshot del testo a confronto con ClearType (06 §7);
  6. controllo delle importazioni alla build di installazione (06 §10).

### 5.3 Passaggio a Windows 11

Il passaggio è probabile (decisioni §1). Quando avviene (RS15 in 08):
- controllo manuale con data di ogni URI di Impostazioni usato (06 §5.7);
- confronto delle letture di configurazione con Impostazioni: Sensore memoria, Cestino, alimentazione, `Start_TrackProgs` (06 §2.4);
- nuove fixture dove un formato o una chiave sono cambiati (06 §2.3);
- riverifica delle regole con una fonte di Windows (§5.1);
- nuove misure di scansione a cache calda e fredda (06 §9), con la data;
- la barra del titolo resta quella di sistema, quindi Snap Layouts funziona senza lavoro (05 D2);
- un eventuale Dev Drive è ReFS e resta "senza garanzie" (F-SC-10, R28 in 08).

### 5.4 Dati di riferimento

- Le misure citate nei documenti sono state prese sul PC di sviluppo, in sola lettura, e registrate con byte esatti, GiB e data (decisioni §6).
- Da M1 il comando `xtask` rimisura i valori di riferimento del volume; ogni misura porta la sua data.
- I test usano fixture versionate; i confronti con lo stato reale del PC sono controlli manuali con data (06, principi).

## 6. Distribuzione

- Build locale: `cargo build --release` per x64 con CRT statico (05 D9).
- Prima dell'installazione: controllo delle importazioni con `cargo xtask importazioni` (06 §10).
- Installazione: `cargo xtask installa` copia l'eseguibile in `%LOCALAPPDATA%\Programs\Quado` e crea il collegamento nel menu Start dell'utente. In modalità helper l'eseguibile rifiuta di partire da `Downloads` o da una cartella temporanea (05 D3).
- Un eseguibile compilato in locale non ha il Mark-of-the-Web: SmartScreen non interviene e la firma non serve. Il prompt UAC dell'helper mostra "Autore sconosciuto", ed è atteso (05 D9).
- Niente MSI, firma del codice, winget, Store, zip portable, ARM64.
- Aggiornare significa ricompilare dal sorgente e reinstallare. Nessun controllo di aggiornamenti e nessun accesso alla rete.
- Dati solo in `%APPDATA%\Quado`; la pagina "Dati e privacy" li elenca e li cancella (05 D8). Per togliere Quado: cancellare i dati da quella pagina, poi la cartella dell'eseguibile e il collegamento nel menu Start.

## 7. Insieme iniziale delle regole

Entrano nella v1 solo regole per ciò che esiste sul PC di sviluppo: ogni regola richiede una fixture reale anonimizzata (05 D6) e un caso che si possa controllare a mano. La presenza viene dalle misure di 02 e di `revisione/prove-windows.md` e da un controllo in sola lettura del 13/10/2026.

Ordine:
1. priorità 1: casi già decisi in decisioni §3 e presenti sul PC di sviluppo;
2. priorità 2: altre aree grandi presenti sul PC di sviluppo, con una procedura ufficiale nota;
3. priorità 3: aree più piccole o sparse.

Dentro ogni priorità le regole si scrivono in ordine di dimensione misurata. Nella colonna Origine, "proposto" indica uno stato che non è in decisioni §3 né in 06 §2.1: si conferma con la fonte quando si scrive la regola.

| Priorità | Regola | Stato | Metodo o procedura | Origine dello stato |
|---|---|---|---|---|
| 1 | CapCut, versioni precedenti | Da verificare | Spostamento nel Cestino con l'eccezione a F-AC-4; Restart Manager, riferimenti, ultime due versioni tenute ed escluse dalla stima; conferma con "Prova CapCut prima di svuotare il Cestino" | decisioni §3 |
| 1 | CapCut, `User Data\Projects` | Decidi tu, gestore "tu" | Nessuna proposta automatica | 06 §2.1 |
| 1 | CapCut, cache | Lo gestisce CapCut | Solo spiegazione | 04 §6.6, proposto |
| 1 | Ibernazione (`hiberfil.sys`) | Decidi tu | `powercfg /hibernate /type reduced` e `powercfg /h off` affiancati, mostrati e copiati; avviso se l'azione per batteria critica è l'ibernazione (il PC di sviluppo è un portatile) | decisioni §3 |
| 1 | Cestino | Misurato in due modi; cartelle orfane come anomalia (F-SY-2), Lo gestisce Windows nella variante "Nessuna procedura: non toccare" (09, scelta 4) | "Apri il Cestino" e "Svuota il Cestino" con il dialogo di Windows | decisioni §3 e §4 |
| 1 | WinSxS | Lo gestisce Windows | `Dism /Online /Cleanup-Image /StartComponentCleanup`; `/ResetBase` solo come alternativa, con la conseguenza scritta | decisioni §3 |
| 1 | `Windows\Installer` | Lo gestisce Windows, variante "Nessuna procedura: non toccare" | Nessuna | decisioni §2 |
| 1 | File temporanei di Windows: Temp, SoftwareDistribution, miniature | Lo gestisce Windows | Impostazioni › File temporanei, mai a mano; Pulizia disco come alternativa | decisioni §3 |
| 1 | Cache di npm | Lo gestisce npm | `npm cache verify`; alternativa `npm cache clean --force` con le conseguenze; percorso dalla configurazione, mai eseguendo npm | decisioni §3 |
| 1 | Store di pnpm | Lo gestisce pnpm | `pnpm store prune`; stima "non stimabile senza contare i link" | decisioni §3 |
| 2 | Steam: librerie e giochi | Decidi tu (programma grande) | `steam://uninstall/<appid>`; ultima sessione da LastPlayed | 06 §2.1 |
| 2 | Epic: giochi, Unreal Engine, Quixel Bridge, plugin Fab | Decidi tu (programma grande) | Libreria del launcher Epic; un byte ha un solo proprietario, il prefisso più lungo (06 §4); nessun dato d'uso | 06 §2.1 |
| 2 | Android AVD (`.android\avd`) | Lo gestisce Android Studio | Device Manager di Android Studio (02 §9) | proposto |
| 2 | `DriverStore\FileRepository` | Lo gestisce Windows | Solo spiegazione finché la fonte non è verificata; candidato `pnputil /enum-drivers` e `/delete-driver`, da amministratore (02 §8) | proposto |
| 2 | File di paging e swap (`pagefile.sys`, `swapfile.sys`) | Lo gestisce Windows (elemento protetto, F-AC-4) | Solo spiegazione; si cita la pagina Memoria virtuale di Windows (02 §8) | proposto |
| 3 | Cache di Chrome, Brave ed Edge | Lo gestisce il browser | Impostazioni del browser per cancellare i dati di navigazione, una fonte per browser. Firefox non ha una regola nella v1: manca una fixture reale | proposto |
| 3 | `node_modules`, `.next`, `target`, `build` | Decidi tu | Spostamento nel Cestino su scelta esplicita, solo con il lockfile accanto; `cargo clean` per `target`. Sul PC di sviluppo ci sono `node_modules` anche dentro le estensioni di VS Code, senza lockfile: il controllo le esclude | decisioni §3 |

Aggiunte in M3: le cache dei browser sono tre regole (`cache-chrome`, `cache-brave`, `cache-edge`), una per fonte; cinque regole di base, da confermare, danno al bilancio i gruppi Windows, Programmi e Tuoi file: `programmi-registrati`, `pacchetti-msix`, `cartelle-utente`, `cartella-windows`, `ipotesi-dal-nome`. Le versioni di CapCut, i progetti di CapCut e le cartelle orfane del Cestino non hanno una pagina ufficiale: la fonte è un file sul PC di sviluppo o la verifica su VHDX, e il livello dei fatti dedotti è "media".

Fuori dall'insieme iniziale:
- Docker e WSL. Il caso è deciso (decisioni §3), ma il PC di sviluppo non li usa: senza una fixture reale la regola non supera il test di 06 §2.2, quindi non è tra i minimi di 06 §2.1. Entra quando la fixture c'è (§8).
- `Windows.old`: assente sul PC di sviluppo.
- Le altre aree di 02 §8 e §9 (pacchetti di Visual Studio, JetBrains, altre cache di sviluppo, macchine virtuali, Ottimizzazione recapito) entrano dopo, una per volta, con le stesse condizioni.
- Le prove della lente Download (copie identiche, download incompleti, archivi) e della vista Programmi non sono regole di questo elenco: sono funzioni di M4 (F-DL, F-AP).

## 8. Cosa serve per cominciare

Elenco di ciò che serviva prima di M0, M1 e M2; gli esiti sono in 00 §7.1.

Per M0:
1. l'approvazione dei documenti v2, del perimetro della v1 (decisioni §1 e §9 ter) e dello stack condizionato a M0 (00 §7 punto 1);
2. lettore di schermo e Desktop remoto: sono criteri bloccanti di M0 solo se usati (00 §7 punto 2; 05 D2);
3. il nome "Rilievo" confermato o sostituito prima che entri nel codice (00 §7 punto 4);
4. i permessi per `rustup update` e per `git init` (00 §7 punto 5; 05 D12).

Per M1:
5. il consenso ai test su VHDX, che richiedono l'amministratore per creare e montare i volumi (00 §7 punto 6), e alla prova da amministratore dei comandi dell'archiviazione riservata (§3, riga 6).

Prima di M2:
6. il giudizio sull'anteprima statica rivista delle cinque schermate (00 §7 punto 3; domande in 04 §6.9).

Punti nati da questo piano:
7. unità esterne: la prova in sola lettura di 06 §1.3 e R22 in 08 richiedono un'unità esterna; il file system e il tipo di disco (meccanico o SSD) cambiano l'esito;
8. Docker e WSL: il PC di sviluppo non li usa, e finché non c'è una fixture reale la loro regola non entra (§7);
9. OneDrive: l'avviso che spostare nel Cestino un elemento sincronizzato lo elimina anche nel cloud è un controllo manuale di F-AC-2, provato con un provider di prova (06 §5.2; R10 in 08); con OneDrive configurato si aggiunge un controllo manuale con data sul provider vero.

Facoltativo:
10. avviare le tre app di riferimento (un editor di foto, uno di grafica vettoriale e uno di PDF) da una copia in una cartella temporanea, per confrontarle dal vivo (00 §7 punto 9).
