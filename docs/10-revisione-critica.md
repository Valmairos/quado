# 10 · Revisione critica (12/10/2026)

Revisione dell'intero progetto dopo la prima fase. Metodo:
- una revisione avversariale indipendente, condotta da un revisore con contesto separato che ha letto per intero i documenti 00-08 e l'anteprima, con ricalcolo dei numeri e dei contrasti;
- una ricerca sulle prove davvero disponibili in Windows per i nuovi scenari (`revisione/prove-windows.md`);
- il giudizio del revisore principale, con le decisioni in `revisione/decisioni-revisione.md`.

La prima versione dei documenti non è inclusa. Sigle: [B] bloccante prima dell'implementazione, [D] differibile.

## 1. Il problema di riferimento

Un SSD si riempie e l'utente non capisce da dove venga il consumo, né cosa sia sensato fare. La catena da servire è: individuare, comprendere, valutare, decidere, agire in sicurezza, verificare il risultato.

Quado è uno strumento personale. Deve essere solido e manutenibile, ma senza i costi di un prodotto commerciale.

## 2. Inventario

| Documento | Scopo | Decisioni principali prima della revisione |
|---|---|---|
| 00-sintesi | Riepilogo e decisioni aperte | "Analizzatore che spiega"; Rust + egui con cancello M0; helper elevato; nessuna AI |
| 01-ricerca-concorrenti | Concorrenti, lacune, riuso | Velocità e mappe sono commodity; lacune: spiegazione, attribuzione con confidenza, bilancio, incertezza; nessun codice GPL |
| 02-studio-tecnico | API, semantica delle dimensioni, misure | Enumerazione `FileIdExtdDirectoryInfo` (16-26 s misurati); WOF, hard link, cloud, residuo non attribuito; procedure ufficiali |
| 03-specifiche-prodotto | Requisiti v1 | 39 requisiti F-*, modello Gestore/Tipo/Fonte/Gestione, requisiti non funzionali con MSI firmato e IT/EN |
| 04-direzione-ux-ui | Analisi delle tre app di riferimento, direzione visiva | Colore solo per il gestore, tratteggio per il non misurato, 4 glifi di provenienza, IBM Plex |
| 05-decisioni-progettuali | Architettura | D1-D12: helper MFT, arena in memoria, regole TOML, `IFileOperation`, MSI e firma |
| 06-piano-di-test | Verifica | Oracolo, alberi di prova, VHDX in CI, privacy, sicurezza, accessibilità |
| 07-piano-di-sviluppo | Milestone | M0-M6, con parser MFT in M5 e rilascio firmato in M6 |
| 08-analisi-dei-rischi | Rischi | 29 rischi con mitigazioni |
| Anteprima statica (HTML, non inclusa) | Anteprima | Panoramica, Esplora, dialogo di intervento |

## 3. Esito in breve

Il progetto era valido sull'individuazione e sulla comprensione, ma non sulla catena completa.

1. Mancava del tutto la verifica del risultato, e la stima dello spazio recuperabile non aveva un metodo.
2. Le raccomandazioni non avevano stati intermedi e confondevano la prova di identità con la prova di gestibilità. Il caso CapCut violava tre regole scritte negli stessi documenti.
3. Il Cestino era trattato come rete di sicurezza senza considerare tre cose:
   - se si riempie oltre la capacità, Windows elimina da solo gli elementi più vecchi;
   - Sensore memoria può svuotarlo a tempo;
   - per ottenere lo spazio stimato bisognava svuotarlo tutto.
4. Il bilancio del volume aveva definizioni e numeri diversi da un documento all'altro.
5. Il perimetro era quello di un prodotto commerciale: parser MFT, MSI e firma, due lingue, CI privilegiata, benchmark pubblicati.
6. Tre scenari centrali non erano coperti: la cartella Download, i programmi forse non più usati, i residui di software disinstallato.

Tutti i punti bloccanti sono stati corretti nei documenti (§5), e la coerenza tra i documenti è stata ricontrollata il 13/10/2026 (§9). Restano aperte le verifiche tecniche di §7 e le decisioni elencate in `00-sintesi.md` §7.

## 4. Diagnosi

### 4.1 Decisioni solide da mantenere

| Decisione | Perché regge |
|---|---|
| Enumerazione non elevata con `FileIdExtdDirectoryInfo` | Misurata: quasi 3 milioni di file in 16-26 s; dà allocata, attributi e ID senza aprire i file |
| Deduplica degli hard link per ID | Misurata: evita 13,6 GB (10^9) di doppio conteggio |
| Quattro classi di provenienza | Coincidono con il requisito del brief e sono il cuore della fiducia |
| Regole verificabili, nessuna AI a runtime | Determinismo, fonti citabili, privacy |
| Rust per il core, interfaccia sostituibile | Il core resta lo stesso anche se M0 boccia egui |
| UI sempre non elevata | Gli interventi girano con i diritti dell'utente |
| Colore riservato ai dati, tratteggio per il non misurato | Validato per discromatopsia; distingue il dato dall'assenza di dato |
| Solo Cestino e procedure ufficiali in v1 | Nessuna eliminazione definitiva fatta da Quado |

### 4.2 Contraddizioni tra documenti

| # | Contraddizione | Impatto | Correzione | |
|---|---|---|---|---|
| C1 | CapCut: la Panoramica dice "versioni non più in uso" e propone il Cestino, ma 03 F-AC-4 vieta di spostare cartelle di programmi registrati, 03 §6 vieta proposte basate sul nome e 02 §10 chiama la cosa "dedotto" | Test di proprietà violato; CapCut può rompersi se qualcosa punta a una versione vecchia | Stato Da verificare; eccezione scritta a F-AC-4 solo via regola con controlli (Restart Manager, riferimenti in registro, collegamenti, attività pianificate); si tengono le ultime due versioni; testo "dedotto" | B |
| C2 | Cestino orfano presentato come intervento, ma F-AC-1 non ha un tipo applicabile e il comportamento dello svuotamento non è provato | Spinge a svuotare tutto o a toccare `$Recycle.Bin` | Diventa un'anomalia spiegata (F-SY-2), senza cifra recuperabile; prova su VHDX in M1 | B |
| C3 | La card dei conti elenca copie shadow e archiviazione riservata come cause, mentre non sono misurabili | La card che distingue il prodotto afferma ciò che non sa | Due liste: osservato e possibile; nessuna frase causale | B |
| C4 | "Non attribuito" vale 25, 25,4, 26,9, 31,9 o 31,93 GB a seconda del documento; unità mescolate | Testi e test divergeranno | Definizione formale (03 §6.4) e unità sempre esplicite | B |
| C5 | Pulizia disco "non deprecata" (01) contro "deprecata ma mantenuta" (02) | Procedura citata in modo incoerente | Si preferisce Impostazioni › File temporanei; Pulizia disco resta un'alternativa con la fonte ricontrollata | D |
| C6 | File residenti "allocata 0" (06) contro "100 / 104" (02) | Test errato | Allineato a 02; lo spazio dei residenti non torna libero | D |
| C7 | 03 F-IN-2 cita i "duplicati di installatori", ma 03 §8 rinvia l'hash alla v2 | Scenario Download senza definizione | Hash su richiesta, solo nella lente Download, solo tra file della stessa dimensione (F-DL-4) | B |
| C8 | Nell'anteprima la cartella di un programma di progettazione e quella di un driver video, "ipotesi dal nome", sono colorati come Programmi e sommati nel bilancio | Il colore smentisce il testo | Colore del gestore solo da fatti "da fonte" o "dedotto alta"; altrimenti grigio "ipotesi" | B |
| C9 | 07 chiude in M2 un bilancio per gestore che richiede la classificazione di M3 | Criterio di uscita irraggiungibile | Criteri spostati in M3 | D |
| C10 | CI su Windows 10 e 11 con privilegi | Runner non disponibili in quella forma; costo inutile | Test locali sul PC di sviluppo e su VHDX locali | D |
| C11 | Panoramica in meno di 30 s al primo avvio, mentre la sonda misura 26,3 s alla prima esecuzione (stato della cache non controllato) senza costruire l'albero | Obiettivo non sostenuto | Obiettivo a freddo da misurare in M1; a caldo resta 22 s | D |
| C12 | Riferimenti incrociati sbagliati (sonda citata come 02 §8, Linux come 05 §8 e altri) | Documenti difficili da usare | Corretti | D |
| C13 | "Stessi numeri di Esplora" contro gli hard link contati una volta | Promessa impossibile | Limitata a unità e formato sul singolo file | D |
| C14 | Launcher: Steam ed Epic in 03, Xbox in 07, nessun altro | Copertura incoerente | Allineato su ciò che c'è sul PC di sviluppo (Steam, Epic) | D |
| C15 | 01 indica il servizio privilegiato come modello giusto, 05 lo scarta | Incoerenza | 01 annotato | D |
| C16 | L'arena in memoria non ha l'ID file, che però serve a ricontrollo e stima | Stima della memoria troppo bassa | ID solo durante la scansione, poi solo quelli con più nomi; rilettura degli attributi in simulazione | B |
| C17 | 08 parla di "stime reali" | Contraddice la distinzione tra stimato e ottenuto | Riformulato | D |
| C18 | Nell'anteprima "Packages" colorato Windows, "altre {n} cartelle" con il glifo "non disponibile" | Semantica dei segni incoerente | Corretto | D |

### 4.3 Requisiti vaghi o non verificabili

| # | Requisito | Problema | Nuovo criterio | |
|---|---|---|---|---|
| V1 | F-OV-1 "somma uguale alla capacità al byte" | Tautologico: il residuo è calcolato per differenza | Su VHDX fermo, residuo ≤ metafile attesi + tolleranza; errore iniettato che deve spostare il residuo; deriva dell'usato mostrata come incertezza | B |
| V5 | F-IN-4 "test sulle regole" | Non dice cosa si verifica | Tabella attesa per regola: ingresso, stato, metodo, controlli | B |
| V6 | F-AC-2 "non esiste un percorso nel codice" | Un test non dimostra un'assenza | Piano confermato come tipo costruibile solo dalla conferma; test di proprietà con selezioni che includono elementi protetti | B |
| V7 | Stima "subito e dopo il Cestino" | Nessun metodo né tolleranza | Metodo in 03 §6.3; stati esatta, intervallo, non stimabile | B |
| V3, V10, V11, V12 | "Fluido", "utilizzabile entro 1 s", "testo nitido", "oltre una soglia" | Soggettivi o indefiniti | p95/p99 dei frame; primo frame interattivo dopo il riavvio; screenshot affiancati da approvare; soglia in byte | D |
| V2, V4, V8, V13-V16 | Criteri legati allo stato del PC, "affermazione" non definita, confidenza senza criteri, riverifica delle regole indefinita | Non verificabili | Fixture versionate; criteri di affidabilità in 03 §6.2; procedura di riverifica in 07 §5 | D |

### 4.4 La catena

| Anello | Prima | Dopo la revisione |
|---|---|---|
| Individuare | Solido | Invariato |
| Comprendere | Debole su Download e residui | Lente Download (F-DL), residui (F-AP-6) |
| Valutare | Mancavano gli stati intermedi e ogni dato sull'uso dei programmi | Cinque stati con scheda obbligatoria; "ultimo avvio noto" solo da fonti affidabili, mai "inutilizzato" |
| Decidere | La conferma non mostrava "dedotto", né la regola né le alternative | Scheda completa nella conferma |
| Agire | Stima gonfiabile, svuotamento totale implicito, nessuna disinstallazione ufficiale | Metodo di stima, controlli del Cestino, rinvio alle procedure di disinstallazione |
| Verificare | Assente | F-VE: prima e dopo, riscansione, stimato contro ottenuto, registro |

### 4.5 Raccomandazioni più forti delle prove

| Caso | Problema | Correzione |
|---|---|---|
| CapCut, versioni precedenti | Prova di identità usata come prova di gestibilità | Da verificare, con controlli; tenere le ultime due versioni |
| Cestino orfano | Proposto come intervento | Anomalia spiegata |
| Ibernazione | Ometteva amministratore, perdita della sospensione ibrida e `/type reduced`, rischio su portatile | Decidi tu, con le due opzioni affiancate e la lettura preventiva della configurazione |
| npm | "Ricostruibile" assoluto | Lo gestisce npm; `npm cache verify` preferito |
| pnpm | Stima ignorava gli hard link verso `node_modules` | Stima "non stimabile senza conteggio dei link" |
| Docker e WSL | Lo spazio non torna su C: senza compattazione | Stima 0 fino alla compattazione; mai `--volumes` |
| WinSxS `/ResetBase` | Conseguenza omessa | Scritta |
| SoftwareDistribution, Temp, miniature | Rischio di cancellazione manuale | Solo Impostazioni |
| node_modules, target | "Ricostruibile" senza controlli | Decidi tu, con controllo del lockfile e `cargo clean` |

### 4.6 Rischi sottovalutati o mancanti

Aggiunti in 08 con rilevazione, prevenzione e rischio residuo:
- RS1: Cestino oltre la capacità, con eliminazione automatica degli elementi più vecchi.
- RS2: Sensore memoria che svuota il Cestino.
- RS3: svuotamento totale per ottenere la stima.
- RS4: disinstallatore che elimina dati dell'utente.
- RS5: riferimenti a versioni spostate.
- RS6: file in uso e intervallo tra controllo e uso.
- RS7: stime gonfiate.
- RS8: spostamento parziale.
- RS9: ibernazione su portatile.
- RS10: regole con fonti sbagliate.
- RS11: DLL caricate dall'helper elevato.
- RS12: perimetro eccessivo per uno strumento personale.
- RS13: qualità del testo in egui.
- RS14: idratazione via "Apri" e "Proprietà".
- RS15: migrazione a Windows 11.
- RS16: cestini di SID orfani.

### 4.7 Funzioni tolte, ridotte, aggiunte

| Esito | Funzione | Motivo |
|---|---|---|
| Tolta dalla v1 | Parser MFT, fuzzing, oracoli `ntfs`/`mft` | Per uno strumento personale serve leggere le cartelle negate, non la velocità: l'helper enumera con `SeBackupPrivilege` |
| Tolta dalla v1 | MSI, firma, winget, Store, benchmark pubblicati, ARM64 | Un eseguibile compilato in locale non ha bisogno di firma: senza Mark-of-the-Web SmartScreen non interviene |
| Tolta dalla v1 | Inglese, modalità portable, Esporta diagnostica, CI privilegiata multi file system | Un solo utente, un solo PC |
| Rinviata alla v1.1 | Palette comandi | Utile ma non tocca la catena |
| Ridotta | M0: Assistente vocale e RDP non sono più cancelli, se il proprietario del progetto conferma che non li usa | Non sono bisogni dell'utente reale |
| Ridotta | Test di privacy a ogni release | Restano `cargo-deny`, il controllo delle importazioni e una prova senza rete prima della v1 |
| Aggiunta | Verifica del risultato (F-VE) | La catena era interrotta |
| Aggiunta | Lente Download (F-DL) | Scenario più comune e prima voce "tua" sul PC di sviluppo (un centinaio di GiB, cartella intera) |
| Aggiunta | Programmi: uso noto, disinstallazione ufficiale, residui (F-AP-4..6) | Scenari 2 e 4 del brief |
| Aggiunta | Controlli del Cestino, Restart Manager, riferimenti (F-AC-7..9) | Rischi RS1-RS6 |
| Aggiunta | "Verifica questa voce" (F-IN-6), lettura della configurazione (F-SY-3), vista "File grandi" (F-EX-5) | Dimensioni pigre, raccomandazioni condizionali, viste già presenti nella navigazione senza requisito |

### 4.8 Ipotesi tecniche ancora da verificare

Vedi §7. Le più importanti:
- memoria per 3,3 milioni di voci con nomi reali (IP4);
- egui sui criteri di M0 (IP5);
- comportamento di `PreDeleteItem` e del flag di riciclo (IP9);
- svuotamento del Cestino con un `$R` orfano (IP10);
- contrasto delle etichette della mappa (IP21, già corretto nell'anteprima).

### 4.9 Usabilità e anteprima

Corretti nell'anteprima:
- la card dei conti, con gli hard link come nota;
- il bilancio coerente con la top list e i valori illustrativi marcati come tali;
- "{p}% dello spazio usato" al posto di "{p}% del disco";
- etichette ambigue ("altri {n}", "Applicazioni {n}");
- un vocabolario unico degli stati;
- il dialogo di conferma senza "puoi ripristinare tutto";
- il bordo tratteggiato della versione in uso, che si confondeva con il segno del non misurato.

Aggiunte le schermate Download e Risultato. Il carico informativo della Panoramica resta accettabile: quattro blocchi, con dettagli al passaggio del mouse e nell'ispettore.

### 4.10 Manutenzione

| Rischio | Correzione |
|---|---|
| Le regole invecchiano senza che nessuno se ne accorga | Data di verifica visibile, avviso "N regole da riverificare", procedura scritta |
| Formati non documentati (Steam, Epic, CapCut) | Stato "estrattore non riuscito", fixture reali, test che fallisce su un formato inatteso |
| egui 0.x | Aggiornare solo quando serve, con gli snapshot visivi come rete |
| Numeri copiati tra documenti | Le misure citate nei documenti sono state prese sul PC di sviluppo, in sola lettura, con la data; i test usano fixture versionate |

## 5. Piano delle modifiche, nell'ordine applicato

1. Modello delle raccomandazioni, stima e verifica (03 §6, 05 D6-D7, 06), perché tutto il resto ne dipende.
2. Casi che violavano le regole: CapCut, Cestino, ibernazione, npm, pnpm, Docker (03, 04, anteprima).
3. Controlli del Cestino e rischi nuovi (03 F-AC, 08).
4. Riduzione del perimetro (00, 03 §7-8, 05 D3, D9, 07).
5. Nuovi scenari: Download, Programmi, residui (03, 09, anteprima).
6. Bilancio e numeri misurati (03, anteprima).
7. Contraddizioni minori e riferimenti incrociati.

## 6. Matrice degli scenari

In `09-matrice-scenari.md`: per ogni scenario l'esigenza, le informazioni necessarie, il metodo, i limiti, il comportamento previsto, la raccomandazione, le protezioni e la verifica.

## 7. Verifiche tecniche ancora necessarie

| Verifica | Quando | Perché |
|---|---|---|
| egui: tabella da 3,5 milioni di righe, mappa da 20.000 aree, testo a confronto con ClearType, ripiego su WARP | M0 | Decide lo stack |
| Memoria con nomi reali per 3,3 milioni di voci | M1 | IP4 |
| Svuotamento del Cestino con un `$R` senza `$I` | M1, su VHDX | C2, IP10 |
| `PreDeleteItem` e `TSF_DELETE_RECYCLE_IF_POSSIBLE`, capacità del Cestino | M1, su VHDX con Cestino piccolo | IP9, RS1 |
| Lettura delle impostazioni di Sensore memoria e della capacità del Cestino senza privilegi | M1 | RS1, RS2 |
| `/DEPENDENTLOADFLAG` e importazioni dell'helper | M1 | RS11 |
| Comando per lo stato e la dimensione dell'archiviazione riservata | M1 | IP12 |
| Tempo della prima scansione a freddo, su C: dopo un riavvio e su VHDX rimontati | M1 | C11 |
| Fonti dell'uso dei programmi e loro copertura sul PC di sviluppo | fatto in `revisione/prove-windows.md`, da confermare in M4 | F-AP-4 |
| Costo della lettura delle risorse di versione degli installatori | M4 | F-DL-2 |

## 8. Prontezza per l'implementazione

Prima della revisione la risposta era no: mancava un anello della catena, il sistema di raccomandazioni poteva proporre azioni più forti delle prove, e il perimetro avrebbe richiesto mesi prima di dare valore.

Dopo la revisione la direzione è abbastanza solida per cominciare da M0 e M1. Restano tre condizioni:
1. M0 deve passare prima di scrivere la UI vera. Se fallisce, si decide tra Slint e WinUI 3 con il core invariato.
2. Le verifiche di M1 sul Cestino (capacità, `PreDeleteItem`, orfani) vanno chiuse prima di M5, perché da quelle dipende la sicurezza degli interventi.
3. Le regole della v1 si scrivono una per volta, con fonte, fixture reale e data. Un insieme piccolo e verificato vale più di un insieme ampio.

Le decisioni ancora da prendere sono in `00-sintesi.md` §7.

## 9. Verifica finale di coerenza (13/10/2026)

Controllo incrociato di README, documenti 00-10, `revisione/` e anteprima: ID dei requisiti e dei rischi, riferimenti tra sezioni, numeri e unità, stati e raccomandazioni.

Sistemato:
- ID: i 57 requisiti di decisioni §9 ter sono definiti in 03 §5, assegnati a una sola milestone in 07 §4.1 e coperti in 06 §13; i rischi citati da 03, 05 e 06 esistono in 08.
- Numeri e unità: unità esplicite in 01 §8, 02 (§1, §6, §7, §8, §9, §15.4) e §4.7 qui; anteprima allineata alle misure (somma per nomi, cartelle orfane, dispositivi virtuali Android e pnpm come 02 §9); `SizeOnDisk` di Steam spiegato in 09 §7, non sommato alla misura.
- Stati: cache di npm "non stimabile" con `verify` (03, 06, 09, anteprima); copie in Download fuori dalla card interventi prima dell'hash; cartelle orfane del Cestino nella variante "Nessuna procedura: non toccare" (03, 04, 06, 07); Docker e WSL tra i casi minimi solo se installati (03, 06, 07).
- Lacune chiuse: controllo automatico dei colori dei dati con il grigio (06 §7); prima scansione a freddo di C: dopo un riavvio (06 §1.3, §9, §7 qui); file sincronizzati nel Cestino come controllo manuale di F-AC-2 (03, 04 §6.8, 05 D7, 06 §5.2, 08 R10, 09 §25); riverifica delle regole a 6, 11 e 12 mesi in 03 §6.1 e 05 D6, come proposta da confermare.
- Testi: registro `interventi.jsonl` anche nell'anteprima; testo dei residui uguale in 04, 05, 09 e nelle prove; elenchi "da correggere" di 04 §6.6 e 09 aggiornati allo stato attuale; domande aperte di 07 §8 riportate in 00 §7.

Resta aperto:
- le verifiche tecniche di §7, da M0 a M4;
- nell'anteprima, il controllo di capacità del Cestino risulta superato a titolo illustrativo, mentre sul PC di sviluppo la stima prudente lo bloccherebbe (04 §6.6);
- le decisioni e le proposte da confermare elencate in `00-sintesi.md` §7.
