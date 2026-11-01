# 04 · Direzione UX/UI (v2)

Stato: proposta da approvare, rivista dopo la revisione del 12/10/2026 (`10-revisione-critica.md`, decisioni in `revisione/decisioni-revisione.md`). Anteprima visiva: l'anteprima statica delle cinque schermate, una pagina HTML con il selettore delle schermate e del tema. L'anteprima e la prima versione di questo documento non sono incluse.

Cosa cambia rispetto alla prima versione:

| Tema | Prima | Ora |
|---|---|---|
| Schermate dell'anteprima | Panoramica, Esplora, Intervento | In più Download e Risultato (§6.6) |
| Valutazione | Nessuno stato intermedio | Cinque stati con icona ed etichetta, senza colore (§6.1) |
| Provenienza | Un glifo per elemento | Un glifo per singolo fatto (§6.1) |
| Colore del gestore | Assegnato anche alle ipotesi dal nome | Solo da fatti "da fonte" o "dedotto" con affidabilità alta (§6.1, §6.2) |
| Mappa | Etichetta bianca su tutte le tinte | Coppie fisse di riempimento ed etichetta, contrasto ≥ 4,5:1 (§6.2) |
| Livelli di informazione | Non definiti | Cinque livelli raggiunti con l'ispettore e la divulgazione progressiva (§6.5) |
| Flussi | Si fermavano all'esecuzione | Comprendono lo svuotamento del Cestino e la verifica (§6.7) |
| Testi | "Puoi ripristinare tutto"; cause non misurate scritte come certe | Cestino, Sensore memoria, dati d'uso e residui scritti secondo le prove (§6.8) |
| Perimetro | Interfaccia anche in inglese, modalità portable, palette comandi | Italiano e inglese con altre lingue aggiungibili (05 D13, dal 13/10/2026), una sola sede dei dati, palette nella v1.1 |

## 1. Cosa ho esaminato

Le tre app di riferimento (un editor di foto, uno di grafica vettoriale e uno di PDF), nella build scaricata e nel codice del ramo principale:

| Risorsa | Come l'ho verificata |
|---|---|
| Build portable per Windows dell'app di foto, scaricata | SHA256 dello zip identico a quello in `SHA256SUMS` della release corrispondente |
| Build portable per Windows dell'app vettoriale, scaricata | SHA256 identico a quello della release corrispondente |
| Build portable per Windows dell'app PDF, scaricata | SHA256 identico a quello della release corrispondente |
| Codice dell'app di foto, ramo principale al 12/10/2026 | codice dell'interfaccia, guida di design, guida dei dialoghi, schermate |
| Codice dell'app vettoriale, ramo principale al 12/10/2026 | codice dell'interfaccia, schermate |
| Codice dell'app PDF, ramo principale al 12/10/2026 | codice di tema, animazioni, Home, aggiornamenti e modalità portable, schermate |

Le build scaricate sono quindi ufficiali, ma sono indietro di qualche versione minore rispetto al ramo principale. Il codice e le schermate che ho analizzato sono quelli del ramo principale. Le schermate dei repository sono generate dall'app stessa tramite il suo canale di controllo, quindi mostrano l'interfaccia reale e non dei rendering di marketing.

Limite dichiarato: non ho avviato gli eseguibili scaricati. Farlo richiedeva un'approvazione esplicita, e in più l'app di foto, nella build scaricata, scrive le preferenze accanto all'eseguibile, cioè dentro la cartella Download, che non andava modificata. Un confronto diretto tra la build scaricata dell'app di foto e il ramo principale si può fare copiando la cartella in una cartella temporanea e avviandola da lì (00 §7).

Fatti tecnici utili per il resto del progetto:
- Tutte e tre sono scritte in Rust con UI in egui 0.36 / eframe, renderer wgpu (l'app PDF anche glow) e accessibilità tramite AccessKit. Sopra c'è un motore a comandi: un registro unico che interfaccia, riga di comando e automazioni chiamano allo stesso modo.
- Licenza MIT o Apache-2.0. I marchi e i loghi delle tre app sono esclusi dalla licenza.
- Icone Lucide (licenza ISC), font Inter e JetBrains Mono (OFL).

## 2. Le caratteristiche estetiche che funzionano

1. Il chrome è neutro e il colore è uno solo. Nell'app PDF il tema chiaro ha la barra del titolo grigio chiaro, pannelli bianchi, testo quasi nero e un solo accento blu; in scuro i pannelli sono grigio antracite e l'accento è un blu più chiaro. Il documento è l'unica cosa colorata sullo schermo.
2. I token sono un vincolo, non un suggerimento. La guida di design dell'app di foto lo scrive in chiaro: i colori si leggono dai token, mai valori fissi in un widget. È questo che tiene coerenti una decina di temi e centinaia di pannelli.
3. Il contrasto è testato. In alcuni temi ogni testo deve tenere 4,5:1 su ogni superficie, anche negli stati hover, premuto e selezionato, e c'è un test che lo verifica.
4. La tipografia è compatta e regolare. Inter per l'interfaccia, JetBrains Mono per i valori numerici, corpo 12-14 px, titoli di sezione in semibold senza decorazioni.
5. Le icone hanno un unico disegno a tratto (Lucide). Non ci sono icone piene mescolate a icone lineari.
6. Raggi piccoli (6 px), ombre quasi assenti. Nell'app PDF l'unica ombra evidente è il sollevamento delle card della Home al passaggio del mouse, e scompare con le animazioni disattivate.
7. La gerarchia si regge sugli spazi. Pannello strumenti a sinistra, contenuto al centro, pannello contestuale a destra e una barra di icone sul bordo. Le zone sono separate da filetti di 1 px, non da riquadri pesanti.

## 3. I principi di interazione più interessanti

| Principio | Dove si vede | Cosa ne ricavo |
|---|---|---|
| Familiarità prima di tutto | Menu, scorciatoie e pannelli dove la mano se li aspetta (Photoshop, Acrobat, Illustrator) | La nostra grammatica di riferimento è Esplora file più gli analizzatori già noti (mappa e albero). Chi arriva da WizTree o WinDirStat deve orientarsi subito. |
| Divulgazione progressiva | App PDF: elenco degli strumenti con la voce per vederne altri, modalità di lettura, modifica e conversione che filtrano gli strumenti | La schermata iniziale mostra poche cose; i dettagli si aprono sull'elemento selezionato, nell'ispettore (§6.5). |
| Un fatto, un'azione, una chiusura | Banner dell'app PDF che segnala i campi compilabili del documento, con un solo pulsante e la X | È il modo giusto di dire "113 cartelle non sono leggibili" senza un dialogo modale. |
| Anteprima prima di confermare | Filtri con anteprima dal vivo; esportazione con la dimensione stimata del file | Ogni intervento parte da una simulazione con la stima e il suo stato: esatta, intervallo o non stimabile. |
| Palette dei comandi | Ctrl/⌘K, gruppo di appartenenza allineato a destra, comandi non disponibili visibili ma disattivati | Utile per "vai a…", "cerca file", "apri procedura". Rinviata alla v1.1: nella v1 la casella in alto cerca cartelle e file (F-EX-4). |
| Dialoghi prevedibili | Esc annulla, Invio conferma solo se OK è attivo, Tab scorre solo i campi | Esc e la X non eseguono mai l'azione (regola già decisa: una via d'uscita non compie l'azione). |
| Barra di stato onesta | App vettoriale: tempi di disegno e frame al secondo; app di foto: il file appena aperto | Mostriamo file letti, cartelle non leggibili, modalità di scansione e durata. |
| Rispetto delle impostazioni di sistema | L'app PDF legge "Effetti di animazione" di Windows e azzera ogni animazione, compresa la rotella | Stessa regola da noi, con test. |
| Finestre strette | Si nascondono prima i controlli che esistono anche in un menu, poi il resto | Ordine di rinuncia definito in anticipo per ogni barra. |
| Etichette che non sbordano | Puntini e testo completo nel tooltip | Fondamentale per i percorsi lunghi. |
| Verifica visiva come processo | Ogni modifica visiva si controlla con screenshot offscreen a più dimensioni, scale e temi (`egui_kittest`) | Lo adottiamo nel piano di test (06). |
| Privacy nei dettagli | Nessun controllo aggiornamenti all'avvio, file recenti "solo locali, mai la lista di un'altra app" | Nessuna rete e dati solo in `%APPDATA%\Quado`, documentato e verificabile. |

## 4. Tono di voce osservato

Frasi brevi, descrittive, senza enfasi. Nelle app gli aiuti dicono cosa fa un gesto (il clic sinistro e il clic destro su un colore, il trascinamento di un livello) e i banner dicono un fatto solo. Il README dell'app di foto dice apertamente cosa manca: non sostituisce ancora Photoshop nel lavoro professionale di tutti i giorni. È il registro giusto anche per noi, dove un'affermazione sbagliata può costare un file.

## 5. Cosa adattare e cosa evitare

Da adattare:
- token di colore obbligatori e test di contrasto automatici;
- un solo accento per le azioni, mentre il resto del colore è riservato ai dati;
- banner contestuale con un'azione;
- anteprima con stima prima di ogni intervento;
- rispetto di "Effetti di animazione";
- etichette con puntini e tooltip;
- barra di stato con metriche vere;
- nessuna rete all'avvio;
- screenshot offscreen come test di regressione visiva;
- palette comandi con Ctrl+K, nella v1.1.

Da evitare in questa app:
- La grammatica di un'app professionale densa: barra dei menu con centinaia di voci, dock di pannelli, testo a 12 px ovunque. Quado si apre di rado, di solito quando il disco è pieno, quindi base a 13 px e meno superfici.
- Una decina di temi, come nell'app di foto. Noi ne abbiamo due, più "come il sistema".
- Icone colorate per ogni strumento, come nell'app PDF. Da noi il colore codifica i dati: un'icona colorata verrebbe letta come una categoria.
- Promozioni della community (card Discord in Home e pulsante Discord nella barra del titolo). Nell'interfaccia non deve esserci niente che non riguardi il disco.
- La barra del titolo disegnata in proprio, almeno in v1. Le tre app di riferimento la disegnano su Windows. Così si perde il menu Snap Layouts di Windows 11 sul pulsante Ingrandisci, a meno di implementare `WM_NCHITTEST` con `HTMAXBUTTON`. In v1 teniamo la barra di sistema e mettiamo la nostra barra strumenti sotto (dettagli in 05).
- Marchi, loghi e asset delle tre app: esclusi dalla licenza, non li usiamo. Lucide (ISC) si può riusare.

## 6. Proposta: direzione visiva "rilievo"

Il nome di lavoro dell'app era Rilievo (dal 26/10/2026 è Quado), nel senso del rilievo architettonico: il disegno che documenta con misure ciò che esiste, segnando a parte le zone non rilevate. Da quella pratica prendiamo convenzioni concrete, non un'atmosfera.

### 6.1 Principi

1. Il colore appartiene ai dati. Il chrome è grigio caldo e neutro. Le azioni usano un "inchiostro" quasi nero (`#1F2A44` in chiaro, `#CDD6EE` in scuro), non una tinta, così nessun pulsante si confonde con una categoria della mappa.
2. Il tratteggio segna ciò che non è stato misurato. Nei disegni di rilievo le zone inaccessibili si campiscono a tratteggio. Da noi il tratteggio a 45° indica sempre spazio occupato ma non attribuito, o cartelle non leggibili, in ogni grafico e in ogni elenco. Non è un colore, quindi resta leggibile anche per chi non distingue i colori e in stampa.
3. Ogni affermazione porta la sua fonte, e la provenienza vale per singolo fatto, non per elemento: un elemento può avere fatti con glifi diversi. Nell'ispettore di Download, per esempio, dimensioni e date hanno il glifo "misurato" e il contenuto non ancora confrontato ha quello "non disponibile". Quattro glifi di forma diversa, sempre con etichetta testuale disponibile:

   | Glifo | Etichetta | Significato |
   |---|---|---|
   | quadrato pieno | misurato | letto dal file system in questa scansione |
   | quadrato pieno a metà | da fonte | dichiarato da Windows o dall'app: registro, API, file di configurazione dell'app |
   | quadrato vuoto | dedotto | regola su percorso, nome o struttura; ha un'affidabilità (alta, media, bassa, criteri in 03 §6.2) e può sbagliare |
   | quadrato tratteggiato | non disponibile | non leggibile o non misurabile in questa modalità |

4. Gli stati di valutazione non hanno colore. C'è un solo vocabolario di cinque stati, sempre icona più parola, usato con le stesse parole nelle card, nell'ispettore, nella conferma e nel risultato. Definizioni complete e scheda obbligatoria della raccomandazione in 03 §6.

   | Stato | Icona nell'anteprima | Quando | Cosa abilita |
   |---|---|---|---|
   | Gestibile | cerchio con segno di spunta | prova di gestibilità diretta e metodo a basso rischio (copie identiche verificate con hash, download incompleti non in uso) | azione con simulazione |
   | Da verificare | triangolo con punto esclamativo | gestibilità plausibile, prova parziale; i controlli sono elencati | azione disabilitata finché i controlli automatici non passano e quelli manuali non sono spuntati |
   | Decidi tu | cerchio con un punto al centro | ruolo noto, utilità non determinabile dal software (tuoi file, installatori, immagini disco, programmi grandi) | strumenti per decidere; azione solo su scelta esplicita, con simulazione |
   | Lo gestisce X | quadrato con una freccia in diagonale, come un rinvio | si gestisce solo con lo strumento o la procedura di X; variante "Nessuna procedura: non toccare" per `Windows\Installer` e per le cartelle orfane del Cestino (09, scelta 4) | rinvio alla procedura, mai rimozione manuale |
   | Non sappiamo | cerchio tratteggiato | nessuna prova sul ruolo | solo misure e "Apri in Esplora file" |

   Nell'anteprima Da verificare è l'unico stato con un tono proprio: bordo e testo usano il colore di avviso del chrome (`--warn-text`), che non è una delle tinte dei dati. Gli altri quattro stati sono grigi.
5. Il colore del gestore si assegna solo da fatti "da fonte" o "dedotto" con affidabilità alta. Negli altri casi l'elemento è grigio, con l'etichetta "ipotesi": nell'anteprima succede alla cartella di un programma di progettazione, a quella di un driver video e a quella di un gioco in `AppData\Local`.
6. Illustrativo e misurato sono sempre distinti. Nell'app: ogni stima ha il suo stato (esatta, intervallo, non stimabile), e dopo un intervento si vedono due numeri separati, "stimato" e "variazione dello spazio libero". Nell'anteprima: la testata elenca cosa è illustrativo, la legenda del bilancio dice "ripartizione per gestore illustrativa", la card della verifica dice "numeri illustrativi".
7. I numeri sono il contenuto. Nelle tabelle le dimensioni sono in IBM Plex Mono a cifre tabulari, così si allineano in colonna. Le cifre chiave sono in Plex Sans semibold a cifre tabulari. Le unità sono quelle di Esplora file (1 GB = 1024³ byte) con la nota visibile, la virgola decimale in italiano e la scelta SI nelle impostazioni (F-GE-3). La promessa è avere le stesse unità e lo stesso formato di Esplora sul singolo file; i totali delle cartelle possono differire, perché Quado conta una volta sola i file con più nomi (hard link).
8. Prima il quadro, poi il dettaglio, poi l'azione, poi la verifica. Panoramica, poi Esplora o una vista dedicata (Download, Programmi, Windows e sistema), poi Intervento (simulazione e conferma), poi Risultato. Ogni passo risponde a una domanda: quanto e dove, perché, cosa posso fare, cosa è cambiato davvero.
9. Le azioni sono rare e pesanti. Il pulsante primario in inchiostro compare solo nei dialoghi di conferma. Fuori da lì le azioni sono secondarie e, quando portano a toccare file, finiscono con i puntini ("Sposta nel Cestino…"): aprono prima l'anteprima e la conferma, perché nessun clic singolo deve toccare file.
10. Calma. Transizioni brevi (al massimo 120 ms), nessuna animazione con "Effetti di animazione" disattivati, nessun badge o notifica che non porti un'informazione sul disco.

### 6.2 Colore dei dati

Domanda di partenza: cosa deve codificare il colore nella mappa? La mappa mette accanto categorie qualunque, quindi ogni coppia di colori deve restare distinguibile, anche per chi ha una discromatopsia. Ho fatto girare il validatore di palette (simulazione Machado 2009, ΔE OKLab, contrasto WCAG) su varie combinazioni. Con cinque tinte nessuna combinazione supera il controllo: il caso peggiore è viola/blu, con ΔE 1,5 in deuteranopia. Con tre tinte invece passa.

Per questo il colore codifica una sola dimensione, quella più utile per decidere: chi gestisce quei dati.

| Ruolo | Chiaro (`--c-*`) | Scuro (`--c-*`) | Perché questa dimensione |
|---|---|---|---|
| Programmi e dati delle app | `#3460C4` | `#4A86E8` | si gestiscono dal programma (impostazioni, disinstallazione) |
| Tuoi file | `#C96A2B` | `#D9773A` | decidi solo tu |
| Windows | `#1A8F7C` | `#1FA58F` | si gestiscono con le procedure di Windows |
| Non classificato, ipotesi | `#8F8B84` | `#7D7A74` | grigio neutro, nessuna tinta |
| Non attribuito / non leggibile | tratteggio | tratteggio | non è una categoria, è assenza di misura |

I colori `--c-*` servono per la barra del bilancio, le legende, gli indicatori della top list e i campioni nell'elenco. Contrasto contro il fondo dei pannelli: da 3,25:1 a 5,57:1 in chiaro, da 3,99:1 a 5,55:1 in scuro, quindi ≥ 3:1 come richiesto per gli elementi grafici.

Esito del validatore, ricalcolato in questa revisione sugli stessi valori (simulazione Machado 2009 a severità piena, ΔE OKLab × 100, valore minimo tra le coppie):

| Insieme | Visione normale | Protanopia e deuteranopia | Tritanopia |
|---|---|---|---|
| Tre tinte, chiaro | 19,9 | 10,2 | 7,9 (Programmi / Windows) |
| Tre tinte, scuro | 18,4 | 11,4 | 4,4 (Programmi / Windows) |
| Tre tinte più grigio, chiaro | 11,6 | 1,8 (Windows / Non classificato, protanopia) | 7,9 |
| Tre tinte più grigio, scuro | 13,3 | 6,7 (Windows / Non classificato, deuteranopia) | 4,4 |

La prima versione riportava solo le prime due righe, e solo per protanopia e deuteranopia. Il ricalcolo mostra un punto debole: in chiaro, per chi ha una protanopia, il grigio del non classificato e il verde acqua di Windows quasi coincidono. Il grigio non compare mai senza etichetta ("non classificato" o "ipotesi"), ma nella barra del bilancio due segmenti vicini possono confondersi. Va sistemato prima della UI di M2, schiarendo il grigio o separandolo con un filetto, e il controllo va ripetuto dal test automatico dei colori (06 §7).

Nella mappa le etichette stanno sopra il riempimento. Con l'etichetta bianca su tutte le tinte il contrasto andava da 3,07:1 a 4,28:1, sotto la soglia del testo. Ora ogni gruppo ha una coppia fissa di riempimento ed etichetta per tema (token `--tile-*` dell'anteprima):

| Gruppo | Chiaro: riempimento / etichetta | Contrasto | Scuro: riempimento / etichetta | Contrasto |
|---|---|---|---|---|
| Programmi e dati delle app | `#3460C4` / `#FFFFFF` | 5,81:1 | `#4A86E8` / `#151514` | 5,10:1 |
| Tuoi file | `#CD763C` / `#1E1D1B` | 5,01:1 | `#D9773A` / `#151514` | 5,79:1 |
| Windows | `#177E6D` / `#FFFFFF` | 4,95:1 | `#1FA58F` / `#151514` | 5,95:1 |
| Non classificato | `#8F8B84` / `#1E1D1B` | 4,97:1 | `#87857F` / `#151514` | 4,95:1 |

Tre riempimenti differiscono di poco dai colori `--c-*` per tenere l'etichetta sopra 4,5:1: in chiaro Tuoi file (`#CD763C` invece di `#C96A2B`) e Windows (`#177E6D` invece di `#1A8F7C`), in scuro il grigio (`#87857F` invece di `#7D7A74`). Tra i riempimenti della mappa il ΔE minimo è 12,2 in chiaro e 11,7 in scuro in visione normale; nelle simulazioni scende a 4,1 in chiaro (Programmi / Windows, tritanopia) e a 3,2 in scuro (Windows / Non classificato, deuteranopia). Il passaggio del mouse non cambia il riempimento, quindi il contrasto resta quello misurato. La versione in uso di un programma ha un contorno interno continuo nel colore dell'etichetta: il tratteggio resta riservato al non misurato.

Correzione fatta in M2, misurata dal test dei token con soglia ΔE 3,0: Windows in chiaro passa da `#1A8F7C` a `#147A69`. Schiarire il grigio non bastava, perché sarebbe sceso sotto 3:1 col fondo dei pannelli. Con la protanopia la coppia Windows / Non classificato passa da 1,8 a 8,5; la coppia peggiore diventa Programmi / Windows con la tritanopia (3,8 in chiaro, 4,4 in scuro). Nella barra del bilancio i metadati di NTFS hanno un neutro proprio (`meta`: `#55524C` in chiaro, `#B4B0A8` in scuro), lontano dal grigio dei dati.

Le categorie più fini (cache, giochi, sviluppo, macchine virtuali, installatori) non hanno un colore. Compaiono come etichetta nella mappa, nell'ispettore, nei filtri e nelle viste dedicate.

### 6.3 Superfici e testo

| Token | Chiaro | Scuro | Uso |
|---|---|---|---|
| `bg` | `#F4F3F0` | `#151514` | fondo finestra |
| `chrome` | `#EBE9E4` | `#1B1B1A` | barre |
| `panel` | `#FBFAF8` | `#1F1F1D` | pannelli laterali |
| `card` | `#FFFFFF` | `#262624` | card e dialoghi |
| `border` / `divider` | `#DCD9D2` / `#E8E5DF` | `#36352F` / `#2D2C29` | filetti |
| `hover` / `select` | `#EFEDE9` / `#E6E9F0` | `#2A2A27` / `#2B3243` | passaggio del mouse, riga o voce selezionata |
| `text` / `text-2` / `text-3` | `#1E1D1B` / `#55524C` / `#6B675F` | `#ECEBE6` / `#B4B0A8` / `#9C988F` | testo principale, secondario, terziario |
| `ink` | `#1F2A44` | `#CDD6EE` | azione primaria, focus, selezione |
| `warn-text` / `warn-bg` | `#8A5200` / `#FBF0DC` | `#F0B45A` / `#3A2C14` | avvisi e stato Da verificare |
| `ok-text` | `#2C6E33` | `#7FC489` | controlli superati nel dialogo di conferma |

Il grigio è caldo apposta: i grigi freddi dell'app PDF sono perfetti per un lettore di documenti, ma accanto all'arancio e al verde acqua della mappa sembrerebbero sporchi.

Ricalcolo sui valori dell'anteprima: tutti i testi superano 4,5:1 su tutte le superfici, compresi hover e selezione, con un'eccezione. In scuro `text-3` su `select` è 4,45:1: succede al contatore della voce di navigazione selezionata. Il test automatico del contrasto (06) deve coprire anche questi stati, come fa l'app di foto. In M2 `text-3` scuro è passato a `#A29E95` e il test copre tre testi su sette superfici e stati.

Tastiera completa e rispetto di "Effetti di animazione" sono requisiti. Assistente vocale e Desktop remoto non sono più criteri bloccanti di M0, se il proprietario del progetto conferma che non li usa (05 D2, 00 §7).

### 6.4 Tipografia

IBM Plex Sans (interfaccia) e IBM Plex Mono (dimensioni, percorsi, comandi), entrambi OFL 1.1 e incorporati nell'eseguibile. Rispetto a Inter danno un'identità diversa dalle tre app di riferimento. Plex Mono ha cifre ben distinte (1/l, 0/O), che contano nei percorsi.

Scala: 11 (etichette di sezione), 12 (secondario), 13 (base), 14 (titoli di card), 16 (titolo ispettore), 20 (titolo schermata), 24-28 (cifre chiave).

Nota tecnica: egui non applica le funzioni OpenType come `tnum`, quindi nell'app le colonne numeriche usano Plex Mono e le cifre chiave in Plex Sans vanno verificate a vista. Plex Sans ha cifre a larghezza fissa di default; da confermare sul file del font prima dell'implementazione. La resa del testo in egui si confronta con ClearType a 100% e 150% in M0, con screenshot affiancati da approvare (05 D2).

### 6.5 Livelli di informazione

Le informazioni stanno su cinque livelli. Si raggiungono con la divulgazione progressiva e con l'ispettore, non con modalità separate: non c'è un interruttore "base / esperto", e il dettaglio tecnico sta nello stesso posto per chiunque lo cerchi.

| Livello | Domanda | Dove sta |
|---|---|---|
| Quadro | Quanto è usato e dove finisce lo spazio | Panoramica: bilancio, "Dove finisce lo spazio", card dei conti |
| Cause | Perché è così grande | Ispettore, blocco "Perché è così grande"; per lo spazio non attribuito, la card dei conti |
| Dettagli tecnici | Da dove viene ogni fatto, con quale regola | Fatti con glifo nell'ispettore, regola con data di verifica e "Vedi regola", conteggi di file e cartelle, suggerimenti al passaggio del mouse, barra di stato |
| Azioni | Cosa posso fare e con quale stato | Blocco "Cosa puoi fare" dell'ispettore, card "Interventi possibili", dialogo di conferma |
| Avanzato | Andare oltre la scansione standard | "Ripeti come amministratore", comandi mostrati e copiati, "Rileggi dal disco" (F-IN-6), filtri, vista File grandi |

### 6.6 Schermate

L'anteprima ha cinque schermate, scelte dal selettore in alto o dalla barra laterale. La barra laterale elenca anche Programmi, File grandi e Windows e sistema, che non sono disegnate: i requisiti sono in 03 (F-AP, F-EX-5, F-SY) e i comportamenti attesi in 09. La voce Interventi, con il numero di interventi registrati, apre la schermata Risultato. In basso la barra di stato riporta file e cartelle letti, cartelle non leggibili, il metodo di conteggio ("Dimensione allocata, hard link contati una volta") e la modalità ("Senza privilegi di amministratore").

1. Panoramica. La testata ha volume, disco, file system e cluster, e a destra la riga della scansione: ora, durata, "senza privilegi di amministratore". La card "Come è usato lo spazio" mostra usati e liberi in cifre grandi, la barra unica del volume divisa per gestore con il tratteggio del non attribuito e lo spazio libero, e la legenda con i valori e la nota "ripartizione per gestore illustrativa". Sotto, a sinistra, "Dove finisce lo spazio": i dieci elementi più grandi di qualunque tipo, senza sovrapposizioni, ognuno con l'indicatore del gestore, il glifo della fonte, una riga di spiegazione, una barra e la dimensione. Il Cestino compare con le sole misure ("su disco" e "secondo Windows"). A destra due card. "Perché i conti non tornano del tutto" mette in colonna usato secondo Windows, file letti (ogni file contato una volta), metadati NTFS misurati e non attribuito; una nota dice che gli hard link sono già contati una volta; seguono le liste "Osservato" e "Possibile, non misurabile senza amministratore", e il pulsante "Ripeti come amministratore" con la spiegazione di cosa legge. "Interventi possibili" elenca tre voci con stima e stato: versioni precedenti di CapCut (Da verificare, decine di GB), cache di npm (Lo gestisce npm: la dimensione della cache, con la stima "non stimabile") e file di ibernazione (Decidi tu). Le copie in Download entrano nella card solo dopo il confronto, come Gestibile (F-DL-4); il Cestino orfano non è tra gli interventi.
2. Esplora. Percorso a briciole con dimensione e quota dello spazio usato (non del disco). Una barra con "Mappa ed elenco" o "Solo elenco", il criterio di colore e un filtro di dimensione. La mappa ad albero è annidata su due livelli, con un'intestazione per ogni gruppo; dentro CapCut, Projects e Videos hanno il colore dei tuoi file e la versione in uso ha il contorno continuo. L'elenco è sincronizzato con la mappa e ha dimensione, quota del genitore e "Gestito da" con il glifo. L'ispettore a destra ha nome e tipo, dimensione con numero di file e cartelle, quota dello spazio usato; poi i fatti, ciascuno con il suo glifo; poi "Perché è così grande", con le parti principali e una riga ciascuna; poi "Cosa puoi fare", con le azioni e il loro stato (versioni precedenti: Da verificare, con "Prepara lo spostamento nel Cestino"; cache: Lo gestisce CapCut, solo spiegata). In fondo la regola, con la versione e la data su cui è stata verificata, e "Vedi regola".
3. Download. La testata ha percorso, dimensione e numero di file, e dichiara il metodo: "Tipi secondo l'estensione. Versioni dai dati di versione degli installatori, senza eseguirli." Una riga di sei riquadri riassume le famiglie con dimensione e numero di file. L'elenco "Cosa si capisce di questa cartella" porta in testata la regola "nessuna proposta nasce da nome, data o dimensione da soli". Ogni voce ha una spiegazione, una cifra e uno stato: tre file con lo stesso nome e la stessa dimensione (Decidi tu finché il contenuto non è confrontato, Gestibile se risultano identici); stesso nome e contenuto diverso (Decidi tu); installatori di programmi già installati alla stessa versione (Decidi tu); archivio con una cartella omonima accanto (Decidi tu, con il confronto tra l'indice dell'archivio e la cartella come prova); immagini disco (Decidi tu); installatore di un programma non installato (Decidi tu). In fondo la legenda dei cinque stati. L'ispettore mostra i fatti (dimensioni e date misurate, contenuto non confrontato), i file con le caselle disattivate e il pulsante "Confronta il contenuto", che dice quanti byte legge; le caselle si attivano solo dopo il confronto.
4. Intervento. Dialogo modale sopra l'Esplora, con oscuramento: qui serve, perché l'azione cambia lo stato del disco. Il titolo è una domanda ("Spostare nel Cestino {n} versioni precedenti di CapCut?"), seguita da "Anteprima di cosa succede. Nessun file è stato ancora toccato." e da una riga con stato, prove e regola. Tre numeri: da spostare, liberato subito (0 GB, perché il Cestino trattiene i file) e dopo lo svuotamento, con lo stato della stima ("stima esatta"). L'elenco con le caselle mostra anche la versione in uso e la precedente, visibili ma disattivate, marcate "esclusa" e "tenuta". Seguono i controlli con il loro esito: Restart Manager con l'ora, riferimenti in registro, collegamenti e attività pianificate, capacità del Cestino, Sensore memoria; poi l'avviso "Prova CapCut prima di svuotare il Cestino" con la frase sulla reversibilità. Nel piede "Esc annulla.", Annulla e il primario "Sposta nel Cestino".
5. Risultato. Quattro passi numerati: lo spostamento (quante cartelle su quante, nessun elemento saltato, ogni cartella ricontrollata prima di spostarla); lo spazio libero invariato, con prima, ora e differenza; l'eliminazione dal Cestino, con i due pulsanti "Apri il Cestino" e "Svuota il Cestino…"; la verifica dopo lo svuotamento. A destra un esempio di verifica completata, marcato "numeri illustrativi": stima, spazio libero in più misurato, differenza, le possibili spiegazioni e la regola del 5%.

Stato dell'anteprima rispetto alle decisioni, al 13/10/2026. Corretti nell'HTML: Sensore memoria attivo, con i giorni della sua regola, nel dialogo; nel Risultato il conteggio di Windows al posto delle cartelle orfane; "Confronta il contenuto" che dichiara l'aggiornamento della data di ultimo accesso; ricerca solo di cartelle e file; stesso glifo "da fonte" per la cache di npm nella top list e nell'Esplora; nella top list il numero misurato di cartelle orfane; cache di npm senza cifra recuperabile; registro `interventi.jsonl`; dispositivi virtuali Android e pnpm con i valori di 02 §9; copie in Download fuori dalla card interventi finché non sono confrontate. Restano due note:
- Il riquadro delle famiglie non ha "Download incompleti" (F-DL-6): nell'app compare quando ce ne sono.
- Nel dialogo il controllo di capacità del Cestino risulta superato. Dopo M1 è anche corretto: Windows conta la capacità sulla dimensione logica e non conta le cartelle orfane (07 §3 righe 1-3), quindi le versioni precedenti di CapCut, decine di GB, entrano nel Cestino di C:, che sul PC di sviluppo può contenere circa il 5% dell'unità. Il punto è chiuso.

### 6.7 Flussi

Dettaglio dei requisiti in 03, comportamento per scenario in 09.

- Primo avvio: elenco dei volumi, poi scansione del volume di sistema con un clic. Durante la scansione i totali delle cartelle di primo livello crescono dal vivo, poi si arriva alla Panoramica.
- Dalla Panoramica all'Esplora: un clic su una riga di "Dove finisce lo spazio" apre l'Esplora già posizionata su quell'elemento.
- Completamento con privilegi: dalla card dei conti, "Ripeti come amministratore". Windows chiede conferma; l'helper legge in sola lettura solo le cartelle negate, lo spazio delle copie shadow e i cestini di altri account (F-SC-3). La card mostra poi cosa è cambiato.
- Download: dalla famiglia o dalla voce all'ispettore; "Confronta il contenuto" legge i file della stessa dimensione e lo dichiara; se coincidono lo stato diventa Gestibile, si tiene il più vecchio e si attivano le caselle; da lì si passa alla simulazione.
- Intervento con il Cestino: dall'ispettore o dalla card, sempre attraverso la simulazione. I controlli automatici devono passare; per Da verificare i controlli manuali si spuntano. Prima di spostare, Quado confronta la capacità del Cestino con il contenuto attuale più la selezione e blocca l'azione se Windows dovrebbe eliminare da solo gli elementi più vecchi (F-AC-7). Dopo la conferma si arriva al Risultato, che separa ciò che è riuscito da ciò che non lo è.
- Liberare davvero lo spazio: non c'è mai uno "svuota tutto" implicito. "Apri il Cestino" serve a eliminare solo gli elementi spostati da Quado, che il registro elenca. "Svuota il Cestino…" apre il dialogo di Windows, dopo aver detto quanti altri elementi e quanti GB contiene secondo Windows.
- Verifica dopo l'azione: prima dell'azione Quado registra spazio libero, dimensione del Cestino e stima con il suo metodo. Rimisura dopo l'azione, dopo lo svuotamento del Cestino e dopo "Ho eseguito la procedura": spazio libero e riscansione dei sottoalberi toccati. Mostra stimato e variazione dello spazio libero; oltre il 5% di differenza la spiega voce per voce o la dichiara non spiegata. Il registro conserva entrambi i numeri e la vista Interventi li elenca (F-VE).
- Procedure esterne: per ciò che lo gestisce un altro strumento, Quado mostra e copia il comando, oppure apre la pagina giusta (Impostazioni › File temporanei, Impostazioni › App, `steam://uninstall/<appid>`). Non esegue nulla. Il pulsante "Ho eseguito la procedura" avvia la nuova misura.
- Disinstallazione di un programma: dalla vista Programmi, Quado avvisa se nella cartella ci sono tuoi file, copia il nome negli appunti e apre Impostazioni › App (`ms-settings:appsfeatures`); per Steam usa `steam://uninstall/<appid>`, per Epic rimanda alla libreria del launcher. Non esegue mai `UninstallString` (F-AP-5). Il titolo del dialogo chiede quello che fa il pulsante ("Aprire Impostazioni per disinstallare {programma}?" con "Apri Impostazioni"), perché la disinstallazione la fa l'interfaccia ufficiale.

### 6.8 Esempi di testo

I valori tra parentesi graffe sono segnaposto. Il testo in uso sta nei cataloghi (`crates/quado-ui/lingue/it.ftl` e `crates/quado-kb/regole/it.ftl`, F-GE-1, 05 D13); le regole per scriverne uno nuovo e il glossario sono in `testi.md`. La tabella riporta il testo in uso al 16/10/2026 per le situazioni decise qui: se cambia nel catalogo, si aggiorna anche qui. La versione inglese ne conserva il senso, in particolare per stati e avvisi (RS29), e il layout regge testi più lunghi del 40% (RS30).

| Situazione | Testo in uso |
|---|---|
| Fine scansione | Scansione completata in {durata}. Windows ha negato l'accesso a {n} cartelle: il loro contenuto resta nel non attribuito. |
| Card dei conti, titoli delle liste | «Osservato» e «Possibile, misurabile solo come amministratore». Nessuna frase che attribuisca una causa allo spazio mancante. |
| Hard link | File con più nomi (hard link): {n} nomi in più per gli stessi dati, già contati una volta. Contando ogni nome si aggiungerebbero {x} GB. |
| Cestino con cartelle orfane, top list | {x} GB su disco, {y} MB secondo Windows |
| Cestino con cartelle orfane, ispettore | Nella cartella del Cestino ci sono {x} GB che il Cestino di Windows non elenca: cartelle senza il loro file indice. Svuotare il Cestino non le elimina: provato su un disco virtuale. |
| Attribuzione incerta | Ipotesi dal nome: {nome}. Nessuna voce di installazione lo conferma. |
| Dati personali dentro una cartella di app | I tuoi progetti video. Non è cache: Quado non propone di eliminarli. |
| Versioni di CapCut, Panoramica | {n} cartelle di versioni precedenti (dedotto) |
| Conferma | Spostare {n} elementi di «Versioni precedenti di CapCut» nel Cestino? Tre numeri: da spostare, liberato subito (0 byte), dopo lo svuotamento del Cestino, con lo stato «stima esatta». Pulsante «Sposta nel Cestino». |
| Avviso nella conferma di CapCut | Prova CapCut prima di svuotare il Cestino. |
| Reversibilità | Puoi ripristinare gli elementi dal Cestino finché non lo svuoti tu o Sensore memoria. |
| Sensore memoria attivo | Sensore memoria elimina in automatico gli elementi che restano nel Cestino per più di {giorni} giorni. |
| Cestino troppo piccolo per la selezione | Spostamento bloccato: il Cestino di {volume} può contenere {capacità} GB e con questa selezione arriverebbe a {totale} GB. Oltre quel limite Windows elimina da solo gli elementi più vecchi. |
| Apri il Cestino | Apri il Cestino ed elimina solo i {n} elementi spostati da Quado il {data} alle {ora}: seguono i nomi. |
| Svuota il Cestino | Il dialogo di Windows elimina anche {n} altri elementi per {x} GB, secondo il conteggio di Windows. |
| Errore parziale | {n} elementi non sono stati spostati. Gli elementi spostati ({m}) sono nel Cestino: per rimetterli al loro posto apri il Cestino, selezionali e scegli «Ripristina». |
| Procedura esterna | Cache di npm: la gestisce npm con i suoi comandi. Sotto il comando: Quado mostra e copia il comando, ma non lo esegue. «Copia» misura anche lo spazio libero di partenza. Dopo aver eseguito il comando, scegli «Ho eseguito la procedura» per misurare il risultato. |
| Lettura del contenuto | Il confronto legge per intero i {n} file ({x} MB) e ne calcola l'impronta, sul tuo PC. Aggiorna la data di ultimo accesso di questi file. |
| Installatore MSI di un programma installato | Stesso codice di prodotto di {programma} {versione}, installato (glifo «da fonte») |
| Installatore exe di un programma installato | Nome e versione coincidono con {programma} {versione}, installato (glifo «dedotto», affidabilità media) |
| Uso noto da Steam | Ultima sessione registrata da Steam: {data} |
| Uso non disponibile | Tracciamento degli avvii di Windows disattivato: nessun dato d'uso |
| Uso non registrato dal launcher | Epic non registra quando un gioco è stato usato l'ultima volta. |
| Possibile residuo | Nessun programma installato corrispondente (confronto per nome). Ultima modifica: {data}. Potrebbe essere un residuo. |
| Disinstallazione | Titolo «Aprire Impostazioni per disinstallare {programma}?», pulsante «Apri Impostazioni». Dopo l'apertura: Si apre Impostazioni › App. Il nome «{programma}» è negli appunti: incollalo nella casella di ricerca. |
| Tuoi file nella cartella di un programma | Nella cartella di {programma} ci sono tuoi file ({x} GB in {sottocartella}). Copiali altrove prima di disinstallare. |
| Ibernazione | Ridurre il file di ibernazione: mantiene l'avvio rapido e toglie l'ibernazione. Disattivare l'ibernazione: toglie anche l'avvio rapido e la sospensione ibrida. Entrambi i comandi vanno eseguiti come amministratore. |
| Disco virtuale di Docker o WSL | Stima su C: 0 GB fino alla compattazione del disco virtuale. (Testo proposto: la regola non c'è ancora.) |
| File sincronizzato nel Cestino (controllo manuale, F-AC-2) | Questo elemento è sincronizzato con {provider}: spostarlo nel Cestino lo elimina anche nel cloud e sugli altri dispositivi. Per liberare spazio senza eliminarlo usa «Libera spazio» di {provider}. |
| Verifica | Stimato: {x} GB. Variazione dello spazio libero: {y} GB. La variazione comprende anche ciò che altri programmi hanno scritto o eliminato nel frattempo. |
| Differenza oltre il 5% non spiegata | Differenza non spiegata: {z} GB ({p}%). |
| Punti di ripristino attivi | Con i punti di ripristino attivi, parte dello spazio liberato può tornare occupato nei giorni successivi. |

Parole da non usare mai: "sicuro da eliminare", "sicuro da disinstallare", "puoi ripristinare tutto", "inutilizzato", "mai usato", "non usato da N giorni", "spazio sprecato", "spazzatura", "ottimizza", "pulizia intelligente", punti esclamativi, numeri in rosso per mettere fretta. "Residuo" si scrive solo come possibilità ("potrebbe essere un residuo"). Usiamo gli stessi termini di Esplora file: "Dimensioni" per la dimensione dei dati e "Dimensioni su disco" per lo spazio allocato.

### 6.9 Cosa giudicare nell'anteprima

- Se i cinque stati si capiscono dalla parola e dall'icona, senza colore, e se "Da verificare" viene letto come "non ancora" e non come "puoi cancellare". Se il colore di avviso su Da verificare aiuta, o se si confonde con l'arancio dei tuoi file.
- Se la parola «Stato», usata nel dialogo, nella legenda e nei documenti, si capisce senza spiegazioni.
- Se il tratteggio come segno del "non misurato" si capisce senza leggere la legenda.
- Se la mappa a tre colori più il grigio regge, o se mancano colori per categoria fine.
- La card "Perché i conti non tornano del tutto" con le due liste: è il tratto distintivo, ma non deve sembrare un avviso.
- La schermata Download: se le voci aiutano a decidere, e se "Decidi tu" sugli installatori è lo stato giusto.
- Il Risultato: se i due pulsanti del Cestino e la verifica si capiscono senza spiegazioni.
- L'inchiostro come colore delle azioni al posto di un blu, e IBM Plex al posto di Inter.

Nell'anteprima la prima riga imita la barra del titolo di Windows 10, che in v1 resta quella di sistema. La barra strumenti dell'app (volume, ricerca, Riesamina) sta subito sotto.
