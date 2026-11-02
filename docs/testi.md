# Testi dell'interfaccia: guida

Vale per ogni testo che l'utente legge in Quado: `crates/quado-ui/lingue/{it,en}.ftl`, `crates/quado-kb/regole/{it,en}.ftl`, nomi accessibili, suggerimenti e messaggi d'errore. Si apre prima di scrivere o cambiare un testo, non dopo.

Fonti, in ordine di peso:
1. Anti-Slop Writing (github.com/adewale/anti-slop-writing): niente pattern da testo generato, dettaglio al posto dell'enfasi.
2. Microsoft, Writing style per le app Windows (learn.microsoft.com/windows/apps/design/style/writing-style): chiarezza, brevità, azioni esplicite, errori che aiutano a risolvere.
3. UX Writing Skill (github.com/content-designer/ux-writing-skill): regole per componente e coerenza del linguaggio in tutta l'app.

Restano valide le regole di prodotto già decise: i cinque stati con le stesse parole ovunque, le parole vietate (§3), nessuna causa scritta come certa se non è misurata, nessun trattino lungo o medio.

## 1. Voce

Quado parla come un tecnico che spiega a una persona non esperta cosa ha misurato e cosa può fare. Frasi brevi, letterali, senza enfasi e senza metafore.

| Sì | No |
|---|---|
| Fatti misurati, con il numero e da dove viene | Aggettivi di valore: "sicuro", "veloce", "semplice", "potente", "intelligente" |
| Una azione chiara per ogni problema | Spiegazioni lunghe al posto dell'azione |
| La conseguenza vera di un'azione, prima di farla | Rassicurazioni che il codice non garantisce |
| Il dubbio detto una volta, dove serve | Avvertenze ripetute in ogni frase |

Persona:
- italiano: si dà del tu; inglese: you;
- l'app non dice "noi" e non dice "io". Quando serve distinguere chi fa cosa (Quado, Windows, un altro programma) si nomina l'attore: "Quado non legge il contenuto dei file", "Windows chiede conferma". Altrimenti la frase ha come soggetto l'oggetto di cui parla: "La cartella non si apre";
- eccezione: lo stato "Non sappiamo" / "We don't know".

## 2. Forma

1. L'essenziale prima: prima cosa è successo o cosa fare, poi il perché. "Seleziona una cartella per vederne le misure", non "Per vedere le misure di una cartella, selezionala".
2. Voce attiva. Il "si" impersonale e il passivo solo quando l'attore non conta.
3. Una idea per frase. Nel corpo al massimo circa 20 parole per frase; titoli e pulsanti molto più corti.
4. Un termine per ogni concetto, quello del glossario (§6). Mai sinonimi per variare.
5. Niente gergo. Nomi tecnici (NTFS, `$MFT`, SID, WMI, `InstallLocation`, HKLM, `.jsonl`, WinSxS) solo dove servono all'utente per verificare o agire, mai in un titolo o in un pulsante; altrimenti si descrive la cosa ("la voce di disinstallazione del programma"). Il dettaglio tecnico, se serve, va nel suggerimento.
6. Numeri sempre con l'unità e il formato della lingua (`12,4 GB`, `1.234 file`). Se il numero non è misurato da Quado si dice da dove viene ("secondo Windows", "dichiarato dal programma").
7. Formulazione in positivo. Una sola negazione per frase; mai catene come "non è una prova e non cambia nessuno stato".
8. Maiuscola solo all'inizio e nei nomi propri. Niente maiuscole all'inglese nei titoli.
9. Punto finale nelle frasi complete del corpo, dei suggerimenti, degli errori e dei dialoghi. Nessun punto in pulsanti, etichette, titoli, voci di elenco brevi, caselle.
10. Punteggiatura: niente trattini lunghi o medi, niente punti esclamativi, niente grassetto per enfasi. I puntini `…` solo in due casi: un comando che apre un dialogo dove serve altro prima di agire ("Sposta nel Cestino…"), e un'operazione in corso ("Scansione in corso…").
11. Due punti con parsimonia: vanno bene per "etichetta: valore" e per introdurre un elenco; non per legare due frasi che starebbero meglio separate.

## 3. Pattern vietati

Il segnale è il gruppo di pattern, ma nell'interfaccia ciascuno basta a riscrivere.

| Pattern | Esempio da evitare | Cosa fare |
|---|---|---|
| Antitesi decorativa: "non X, ma Y", "non solo… ma anche", "X, non Y" | "È una stima, non una promessa" | Dire il fatto: "Stima dalle dimensioni lette prima dello spostamento". L'antitesi resta solo se entrambi i lati sono fatti che l'utente deve sapere, una volta |
| Importanza dichiarata | "È importante sapere che…", "fondamentale", "cruciale" | Togliere, oppure dire cosa cambia |
| Introduzioni e chiusure | "Qui puoi…", "Questa sezione mostra…", "In sintesi" | Partire dal contenuto |
| Copula gonfiata | "rappresenta", "funge da", "si configura come" | "è" |
| Elenchi a tre per ritmo | "veloce, sicuro e affidabile" | Un fatto misurato |
| Ripetizione | Il corpo ripete il titolo; "eliminati definitivamente" accanto a "non si può annullare" | Ogni frase aggiunge un'informazione |
| Burocratese | "effettuare", "procedere", "si è verificato un errore", "si prega di", "al fine di", "risulta" | Verbo semplice e diretto |
| Precisione finta | percentuali o tempi non misurati | Solo numeri misurati, con la fonte |
| Nominalizzazioni in fila | "verifica della presenza di riferimenti nella configurazione" | Verbo: "controlla se la configurazione nomina la cartella" |
| Simmetria evasiva | "Che tu sia esperto o no…" | Togliere |

Parole da non usare mai: "sicuro da eliminare", "sicuro da disinstallare", "puoi ripristinare tutto", "inutilizzato", "mai usato", "non usato da N giorni", "spazio sprecato", "spazzatura", "ottimizza", "pulizia intelligente". In inglese: "safe to delete", "restore everything", "unused", "never used", "wasted space", "junk", "optimize", "smart clean". Niente numeri in rosso per mettere fretta. "Residuo" si scrive solo come possibilità ("potrebbe essere un residuo"). I test dei cataloghi controllano queste parole nei valori, anche nelle forme flesse.

Prova dell'enfasi: si riscrive la frase senza ritmo e senza parole d'effetto. Se non resta un'affermazione concreta, la frase si taglia o si riscrive con il fatto.

## 4. Componenti

| Componente | Regola | Esempio |
|---|---|---|
| Pulsante | Imperativo + oggetto, 1-3 parole, nessun punto. Nei dialoghi risponde alla domanda del titolo; l'uscita è "Annulla" o "Chiudi" e non fa mai l'azione | "Sposta nel Cestino", "Avvia scansione", "Annulla" |
| Collegamento o azione nel testo | Dice dove porta o cosa fa | "Apri le regole" |
| Titolo di schermata o di card | Nome o compito, 2-6 parole | "Come è usato lo spazio" |
| Etichetta, colonna, filtro | Nome, senza punto | "Dimensioni su disco" |
| Suggerimento (tooltip) | Una o due frasi complete: cosa fa il controllo o cosa significa il valore | "Rilegge le dimensioni dal disco e aggiorna i totali." |
| Dialogo di conferma | Titolo: domanda con l'azione e la quantità. Corpo: conseguenza e reversibilità vere. Pulsante principale: lo stesso verbo del titolo | "Spostare 3 cartelle nel Cestino?" / "12,4 GB. Puoi ripristinarle dal Cestino finché non lo svuoti." / "Sposta nel Cestino" |
| Errore | Titolo: cosa non è riuscito. Corpo: perché, se lo sappiamo, e cosa fare. Il codice di Windows in fondo, tra parentesi, per chi deve cercarlo. Mai colpevolizzare, mai "si è verificato un errore" | "Impossibile leggere la cartella" / "Windows nega l'accesso. Riprova come amministratore. (Codice 5)" |
| Avviso (banner) | La conseguenza concreta e cosa fare | "La scansione è stata interrotta: i totali non comprendono le cartelle non lette. Riavviala da Unità." |
| Esito e notifica | Participio o fatto, con il dato | "Scansione completata: 412 GB in 1,2 milioni di file" |
| Stato vuoto | Cosa manca e perché, poi una sola azione; se dipende da filtri, come toglierli | "Nessun file con questi filtri." / "Togli filtri" |
| Avanzamento | Cosa si sta facendo e su cosa, con un contatore vero. Tempi solo se calcolati | "Scansione in corso… 1.204.330 file letti" |
| Impostazione | Nome breve; sotto, una riga sull'effetto | "Unità di misura" / "Come in Esplora file (1 GB = 1024³ byte)" |
| Nome accessibile | Lo stesso testo visibile; per i controlli con sola icona, l'azione | "Chiudi il pannello" |
| Testo lungo (Informazioni, privacy) | Passata anti-slop completa: fatti, niente aggettivi di valore | |

## 5. Lingue

Italiano:
- pulsanti all'imperativo di seconda persona singolare ("Apri", "Sposta", "Annulla"); il negativo è "Non" + infinito ("Non mostrare più");
- titoli d'errore con la forma di Windows in italiano: "Impossibile" + infinito + oggetto;
- niente aggettivi o participi riferiti all'utente che danno per scontato il genere ("Sei sicuro?", "Benvenuto"): "Spostare…?", "Vuoi…?";
- nomi di Windows come li scrive Windows in italiano: Esplora file, Cestino, Impostazioni, Archiviazione, Sensore memoria, Proprietà;
- plurali con i selettori Fluent; "file" è invariabile, "cartella" no;
- virgolette basse «» per citare il nome di un pulsante o di una voce.

Inglese:
- testo parallelo, non traduzione parola per parola; stesse informazioni, stessa lunghezza relativa;
- sentence case, you, imperativo nei pulsanti, contrazioni dove suonano naturali ("can't", "don't");
- errori: "Can't" + verbo + oggetto ("Can't read the folder");
- nomi di Windows in inglese: File Explorer, Recycle Bin, Settings, Storage, Storage Sense, Properties.

Il testo italiano è circa il 20-25% più lungo: i controlli devono reggere almeno il doppio della lunghezza inglese (pseudo-lingua dei test).

## 6. Glossario

Termini fissi. Chi cambia un termine lo cambia qui e in tutto il catalogo nello stesso commit.

| Concetto | Italiano | Inglese | Note |
|---|---|---|---|
| L'unità scansionata | unità | drive | "volume" solo dove serve il termine tecnico (file system, VHDX) |
| Leggere un'unità o una cartella | scansione, scansionare | scan | |
| Ripetere la scansione dell'unità | «Scansiona di nuovo», "rilegge tutta l'unità" | "Rescan", "reads the whole drive again" | Prima «Riesamina» |
| Rileggere un elemento dal disco | «Rileggi dal disco», rileggere, rilettura | "Reread from disk", read again | Prima «Verifica questa voce». "Verificare" resta allo stato Da verificare e alla verifica dopo un intervento |
| Spazio | spazio usato, spazio libero, capacità | used space, free space, capacity | |
| Effetto misurato di un intervento | variazione dello spazio libero | change in free space | Mai "variazione del volume" |
| Spazio di un elemento nella card degli interventi | «Occupa», «Si libera svuotando il Cestino: X», «Si libera: lo misura Quado dopo la procedura» | "Uses", "Freed when you empty the Recycle Bin: X", "Freed: Quado measures it after the steps" | Dal 01/11/2026: la cifra grande è quanto occupa, mai presentata come spazio che si libera |
| Dimensione di un file | dimensioni, dimensioni su disco, file con le stesse dimensioni | size, size on disk, files of the same size | Come nella finestra Proprietà di Windows; mai "della stessa dimensione" |
| Ciò che Quado misura | misure | measurements | In inglese mai "measures" come nome |
| Spazio non spiegato dalla scansione | non attribuito | unaccounted for | Mai una causa, solo il valore e l'incertezza |
| Cestino | Cestino, spostare nel Cestino, svuotare il Cestino, ripristinare | Recycle Bin, move to the Recycle Bin, empty the Recycle Bin, restore | Mai "eliminare" per uno spostamento nel Cestino |
| Comando che porta all'anteprima dello spostamento | «Sposta nel Cestino…» | "Move to the Recycle Bin…" | Prima «Prepara lo spostamento nel Cestino…». Il pulsante della conferma è «Sposta nel Cestino», senza puntini |
| Azione fatta da Quado sul disco | intervento | action | |
| Procedura di un altro programma o di Windows | procedura, «Ho eseguito la procedura», «Nessuna procedura: non toccare» | steps, "I've completed the steps", "No steps to follow: don't touch" | "Apri la procedura" / "Open the steps" |
| Comando ufficiale da copiare | comando | command | Quado lo mostra e lo copia, non lo esegue |
| Chi gestisce lo spazio | Programmi e dati delle app, Tuoi file, Windows, Non classificato | Programs and app data, Your files, Windows, Not classified | |
| I cinque stati | Gestibile, Da verificare, Decidi tu, Lo gestisce {X}, Non sappiamo | Manageable, To verify, You decide, Managed by {X}, We don't know | Uguali in card, ispettore, conferma e risultato |
| Riconoscimento di un elemento | regola | rule | |
| Da dove viene un fatto | fonte | source | La provenienza di un fatto si indica con "da fonte", "dedotto" o "dedotto alta" |
| Lettura con i privilegi | come amministratore; Quado non è avviato come amministratore (forma negativa); si legge (si misura) solo come amministratore | as administrator; Quado isn't running as administrator; readable (measurable) only as administrator | La forma negativa parla di Quado, non della persona: con UAC anche un amministratore avvia i programmi senza elevazione. Mai "senza i privilegi di amministratore", "helper", "elevato", "token", "senza amministratore" nell'interfaccia |
| File aperti da altri programmi | in uso, il controllo dei file in uso | in use | Mai "Restart Manager" né "file aperti" |
| File con più nomi | file con più nomi (hard link) | files with more than one name (hard links) | Il termine tecnico tra parentesi alla prima occorrenza della schermata |
| Account Windows | account, tutti gli account | account, all accounts | SID solo per distinguere due Cestini; mai "tutti gli utenti" |
| Copie shadow | punti di ripristino e copie shadow | restore points and shadow copies | |
| Schermata delle cartelle dell'utente | vista Download | Downloads view | Mai "lente" nell'interfaccia (resta nei nomi delle chiavi e nel codice) |
| Assenza di garanzia legale | senza garanzia, nei limiti consentiti dalla legge | no warranty, to the extent the law allows | Solo in Informazioni (GPLv3 §15 e §16). "Senza garanzie" per le misure su file system non provati (`volumi-senza-garanzie`) è un altro concetto, in un'altra schermata |
| Codice di Windows | (Codice N) | (Code N) | In fondo alla frase, con la maiuscola, anche dopo un frammento: "non letto (Codice 5)". Mai "errore N di Windows" o "codice N" in mezzo alla frase |

### 6.1 Scelte del ripasso di coerenza (16/10/2026)

Sono proposte e si possono cambiare: chi le cambia aggiorna questa sezione, il glossario e il catalogo nello stesso commit.

1. Restano «Rileggi dal disco» / "Reread from disk" e «Scansiona di nuovo» / "Rescan".
2. La forma negativa di "come amministratore" è «Quado non è avviato come amministratore» / "Quado isn't running as administrator" (dal 27/10/2026; prima «senza i privilegi di amministratore» / "without administrator rights"). Con il Controllo dell'account utente (UAC) anche un amministratore avvia i programmi senza elevazione: la vecchia frase sembrava dire che l'utente non è amministratore, invece parlava del processo. La frase nomina quindi il programma, non la persona. Dove il soggetto è già chiaro basta «non come amministratore» (la riga della scansione nella Panoramica). Con Quado avviato come amministratore la barra di stato dice «Avviato come amministratore» / "Running as administrator" e le frasi sulla scansione dicono «come amministratore». Il titolo della lista della card dei conti è «Possibile, misurabile solo come amministratore» / "Possible, measurable only as administrator".
3. Il codice di Windows sta sempre in fondo alla frase, «(Codice N)» / "(Code N)". Dove cadeva dentro un frammento (il motivo di una cartella non leggibile, il motivo di un errore della lettura come amministratore), il motivo è diventato una frase completa che chiude il testo.
4. Restano «Variazione dello spazio libero», «Sposta nel Cestino…» e «Ho eseguito la procedura» ("I've completed the steps"); la schermata è la «vista Download».
5. Dialogo di disinstallazione: Quado non disinstalla, apre l'interfaccia ufficiale. Il titolo chiede quello che fa il pulsante principale: «Aprire Impostazioni per disinstallare X?» con «Apri Impostazioni», «Aprire Steam per disinstallare X?» con «Apri Steam». Quando Quado non apre niente (Epic, dopo l'apertura, dopo un errore) il titolo nomina il compito, «Come disinstallare X»: il pulsante è «Chiudi», con «Ho eseguito la procedura» dopo l'apertura.

Frasi composte dal codice: un testo che si mette dopo "etichetta:" non contiene altri due punti (il motivo di una fonte non letta è "non letto, formato inatteso"), e la riga di una regola non si accoda a un'altra frase con i due punti («Fa parte di X» e la riga della regola stanno su due righe). La stima del record si mostra da sola: il testo comincia già con «Stima».

## 7. Controllo prima del commit

1. Ogni testo nuovo o cambiato passa §2 e §3; per i testi lunghi, la prova dell'enfasi.
2. Il termine è quello del glossario.
3. Pulsanti e titoli dei dialoghi si leggono come domanda e risposta.
4. Ogni errore dice cosa fare; ogni stato vuoto ha al più una azione.
5. Nessuna promessa, nessuna causa non misurata, nessun numero senza fonte.
6. `cargo test -p quado-ui` e `cargo test -p quado-kb`: chiavi, segnaposto e commenti uguali nelle due lingue, parole vietate, pseudo-lingua, stati uguali ovunque.
