# 00 · Sintesi (v2, dopo la revisione)

Stato al 12/10/2026: progettazione rivista dopo una revisione critica, nessun codice applicativo. L'unico codice eseguito è la sonda di misura usa e getta, fuori dal progetto (02 §2.1); le prove raccolte durante la revisione sono letture in sola lettura (`revisione/prove-windows.md`). La prima versione dei documenti non è inclusa. Le misure citate sono state prese sul PC di sviluppo, in sola lettura, e registrate con byte esatti, GiB e data.

## 1. Cos'è Quado

Quado è nato come strumento personale per capire come è usato lo spazio del disco su Windows, per un PC con Windows 10 oggi e probabilmente Windows 11 in futuro. Che altri lo usino è un vantaggio, non un obiettivo.

Non si ferma a mostrare cosa occupa il disco. Per ogni elemento dice cos'è, chi lo gestisce e da dove viene ciascuna informazione. Valuta se e come si può intervenire, con uno di cinque stati. Esegue solo spostamenti nel Cestino dopo una simulazione, oppure rimanda alle procedure ufficiali. Dopo l'intervento misura quanto spazio è tornato davvero.

Funziona in locale, senza rete, senza account e senza AI. L'interfaccia è in italiano. L'eseguibile si compila in locale e si tiene in `%LOCALAPPDATA%\Programs\Quado`; i dati stanno in `%APPDATA%\Quado`.

## 2. Cosa ha trovato la revisione

Una revisione avversariale indipendente ha riletto tutti i documenti e l'anteprima. Diagnosi completa in `10-revisione-critica.md` §3-4. Il progetto era solido nell'individuare e nel comprendere, ma non sul resto della catena:
- mancava la verifica del risultato, e la stima dello spazio recuperabile non aveva un metodo;
- le raccomandazioni non avevano stati intermedi e usavano la prova di identità come prova di gestibilità: il caso CapCut violava tre regole scritte negli stessi documenti;
- il Cestino era trattato come rete di sicurezza senza considerare la sua capacità, Sensore memoria e lo svuotamento totale che serviva per ottenere la stima;
- il bilancio del volume aveva definizioni e numeri diversi da un documento all'altro;
- il perimetro era quello di un prodotto commerciale;
- mancavano tre scenari centrali: la cartella Download, i programmi forse non più usati, i residui di software disinstallato.

## 3. Cosa è cambiato

| Esito | Cosa | Dove |
|---|---|---|
| Mantenuto | Scansione senza privilegi con `FileIdExtdDirectoryInfo`; hard link contati una volta per ID; quattro classi di provenienza; regole verificabili, nessuna AI; core in Rust; interfaccia mai elevata; colore riservato ai dati e tratteggio per il non misurato; solo Cestino e procedure ufficiali | 02, 05, 10 §4.1 |
| Aggiunto | Cinque stati e scheda obbligatoria per ogni raccomandazione | 03 §6, 04 §6.1 |
| Aggiunto | Metodo di stima e verifica dopo l'intervento (F-VE) | 03 §6.3 |
| Aggiunto | Controlli prima del Cestino: capacità, Sensore memoria, file in uso, riferimenti alle cartelle spostate (F-AC-7, F-AC-8, F-AC-9) | 03, 08 |
| Aggiunto | Lente Download (F-DL); Programmi con uso noto, disinstallazione ufficiale e residui (F-AP-4, F-AP-5, F-AP-6); "Verifica questa voce"; lettura della configurazione; vista File grandi | 03, 09 |
| Aggiunto | Matrice degli scenari | 09 |
| Ridotto | Modalità con privilegi: un helper avviato su richiesta che enumera solo le cartelle negate | 05 D3 |
| Ridotto | M0: assistente vocale e Desktop remoto non sono più cancelli, se il proprietario del progetto conferma che non li usa | 05 D2 |
| Ridotto | Privacy: restano `cargo-deny`, il controllo delle importazioni e una prova senza rete prima della v1 | 06 |
| Tolto dalla v1 | Parser MFT, fuzzing e oracoli; MSI, firma, winget e Store; benchmark pubblicati; CI con privilegi su più file system; ARM64 e Linux (resta una nota di portabilità); modalità portable; "Esporta diagnostica" | `revisione/decisioni-revisione.md` §1 e §8 |
| Rinviato alla v1.1 | Palette dei comandi | 07 |

## 4. Ha senso costruirlo, visto quello che esiste?

Sì, ma solo per lo strato che spiega e verifica (`revisione/decisioni-revisione.md` §1).

- Individuare cosa occupa il disco è già risolto. WizTree è gratuito per uso personale e le Impostazioni di Windows danno le categorie principali. Se serve solo sapere cosa occupa spazio, quegli strumenti bastano.
- Comprendere cos'è una cartella, valutare se e come si può gestire, agire con controlli e verificare il risultato: nessuno degli strumenti esaminati lo fa senza mandare dati a un servizio AI nel cloud (01, riesame in testa al documento).
- Lo scanner costa poco: la sonda, circa 300 righe di Rust, ha letto tutto C: in 16 s (02 §2.1). Il lavoro vero sta nelle regole e nell'interfaccia che spiega.
- È stata valutata l'alternativa di costruire Quado sopra l'export CSV di WizTree, e scartata: WizTree è chiuso, è veloce solo da amministratore, il CSV non ha ID file né attributi, e si finirebbe per usare due app insieme.

## 5. Prontezza per l'implementazione

Prima della revisione la risposta era no: mancava un anello della catena, le raccomandazioni potevano proporre azioni più forti delle prove, e il perimetro avrebbe richiesto mesi prima di dare valore.

Dopo la revisione la direzione è abbastanza solida per cominciare da M0 e M1 (10 §8), con tre condizioni:
1. M0 deve passare prima di scrivere la UI vera. Se fallisce, si sceglie un'altra interfaccia e il core resta invariato (05 D2).
2. Le verifiche di M1 sul Cestino (capacità, `PreDeleteItem`, cartelle orfane) vanno chiuse prima di M5, perché da quelle dipende la sicurezza degli interventi.
3. Le regole della v1 si scrivono una per volta, con fonte, fixture reale e data di verifica. Un insieme piccolo e verificato vale più di un insieme ampio.

Le milestone, da M0 (verifica dello stack) a M6 (helper con privilegi, test di usabilità, build locale), sono in 07.

## 6. Verifiche tecniche ancora necessarie

Da `10-revisione-critica.md` §7.

| Verifica | Quando | Perché |
|---|---|---|
| egui: tabella da 3,5 milioni di righe, mappa da 20.000 aree, testo a 100% e 150% confrontato con ClearType, tastiera completa, ripiego sul renderer WARP | M0 | Decide lo stack |
| Memoria con nomi reali per 3,3 milioni di voci (obiettivo ≤ 400 MB) | M1 | Stima di 05 D5 |
| Svuotamento del Cestino con una cartella `$R` senza `$I` | M1, su VHDX | Il Cestino orfano resta un'anomalia finché non è provato |
| `PreDeleteItem`, flag di riciclo e conteggio della capacità del Cestino | M1, su VHDX con un Cestino piccolo | Nessuna eliminazione definitiva fatta da Quado |
| Lettura di Sensore memoria e della capacità del Cestino senza privilegi | M1 | Controlli prima dello spostamento |
| `/DEPENDENTLOADFLAG` e importazioni dell'helper | M1 | DLL caricate dall'helper elevato |
| Comando per stato e dimensione dell'archiviazione riservata | M1 | Dati dell'helper per la card dei conti |
| Tempo della prima scansione a freddo, su C: dopo un riavvio e su VHDX | M1 | Obiettivo a freddo ancora da fissare (C11) |
| Fonti dell'uso dei programmi e loro copertura | fatto in `revisione/prove-windows.md`, da confermare in M4 | F-AP-4 |
| Costo della lettura delle risorse di versione degli installatori | M4 | F-DL-2 |

## 7. Decisioni del proprietario del progetto

1. Approvare la direzione rivista e il perimetro della v1. I requisiti sono in 03, con l'elenco canonico in `revisione/decisioni-revisione.md` §9 ter; cosa resta fuori è in `revisione/decisioni-revisione.md` §1. Comprende lo stack: Rust per il core, egui per l'interfaccia se passa M0 (05 D2).
2. Confermare di non usare un lettore di schermo (Assistente vocale o altri) né Desktop remoto. Non sono più criteri bloccanti di M0; se uno dei due è in uso, torna a esserlo.
3. Giudicare l'anteprima statica rivista: cinque schermate (Panoramica, Esplora, Download, Intervento, Risultato) e cinque stati (Gestibile, Da verificare, Decidi tu, Lo gestisce X, Non sappiamo). Le domande sono in 04 §6.9.
4. Confermare il nome "Rilievo" prima che entri nel codice, oppure sceglierne un altro.
5. Permessi per la fase successiva:
   - aggiornare la toolchain Rust con `rustup update` (sul PC di sviluppo c'è la 1.88 di giugno 2025);
   - inizializzare Git nella cartella del progetto.
6. Consenso per i test locali su VHDX in M1, che richiedono l'amministratore: creare e montare un disco virtuale di prova per misurare il comportamento del Cestino (capacità, cartelle orfane, `PreDeleteItem`) e la stima contro lo spazio liberato davvero. Le prove scrivono solo dentro il disco virtuale, non tra i file dell'utente.
7. Indicare lo stato dei punti 7-9 di 07 §8: unità esterne (file system, dischi meccanici), Docker e WSL, OneDrive.
8. Confermare o correggere le proposte: gli stati marcati "proposto" e la riverifica delle regole a 6, 11 e 12 mesi in 07 (§7 e §5.1); le scelte di 09 non scritte nelle decisioni (sezione "Scelte di questa matrice non scritte in decisioni-revisione.md").
9. Facoltativo: avviare le tre app di riferimento (un editor di foto, uno di grafica vettoriale e uno di PDF) da una copia in una cartella temporanea, per confrontarle dal vivo. Finora non le ho eseguite (04 §1).

### 7.1 Esito (13/10/2026)

Le decisioni sono state approvate in blocco. Effetti:

| Punto | Esito |
|---|---|
| 1 | Direzione, perimetro della v1 e stack approvati; egui resta condizionato a M0 |
| 2 | Nessun lettore di schermo né Desktop remoto: non sono criteri di M0 |
| 3 | Anteprima approvata; resta aperto solo il punto noto di 04 §6.6 (controllo di capacità del Cestino), che dipende dalle prove su VHDX di M1 |
| 4 | Nome "Rilievo" confermato. Dal 26/10/2026 il nome è Quado, con il segno «Metà» (`brand/`) |
| 5 | Permesso per `rustup update` e `git init` |
| 6 | Consenso ai test su VHDX da amministratore (con conferma UAC a ogni sessione) |
| 7 | Nessuna risposta specifica: si assume lo stato attuale del PC di sviluppo, che non usa unità esterne, Docker, WSL né OneDrive; per questi casi valgono le condizioni di 07 §8, punti 7-9. Da rivedere quando cambia |
| 8 | Proposte di 07 e 09 confermate |
| 9 | Facoltativo, non eseguito |

### 7.2 Lingue (13/10/2026)

Nuovo requisito, priorità alta: l'interfaccia deve poter cambiare lingua, con l'inglese e altre lingue oltre all'italiano, e deve essere possibile modificare i testi o aggiungere lingue, a mano o con Claude. Decisione in 05 D13: Fluent, un file di testo per lingua, italiano e inglese completi nella v1, file modificabili anche senza ricompilare. Costruzione in M2, testi delle regole in M3.

La licenza non è più una decisione aperta: per uno strumento personale non serve. Diventa una domanda solo se si decide di pubblicare Quado, e allora conta anche per la firma del codice e per la pubblicazione delle regole.

## 8. Cosa non è stato verificato in questa fase

- Velocità, memoria e traffico di rete dei concorrenti: nessun loro binario è stato eseguito (01).
- Modalità con privilegi (cartelle negate, copie shadow, archiviazione riservata): la sessione non era elevata (02 §6-7).
- Comportamento con OneDrive: il PC di sviluppo non lo usa, quindi i segnaposto cloud si provano con un provider di prova (02 §5, 06 §1.2).
- Se svuotare il Cestino elimina le cartelle orfane: non provato, perché avrebbe modificato il disco (02 §15.4).
- Resa del testo e prestazioni di egui: sono il motivo di M0 (05 D2).
