### Quado: testi dell'interfaccia in italiano (lingua di riferimento).
### Ogni chiave ha un commento che dice dove compare e cosa evitare. Le regole sono in docs/testi.md e valgono
### per ogni lingua. Frasi brevi e descrittive, nessun punto esclamativo, nessuna parola vietata
### ("sicuro da eliminare", "spazzatura", "ottimizza"), nessuna causa attribuita allo spazio che non si misura.
### I segnaposto ($nome) contengono testo già pronto: numeri e date arrivano formattati.
### $n serve solo a scegliere il plurale; il numero da mostrare è $testo.

## Generali

# Nome della lingua, scritto nella lingua stessa: compare in Impostazioni › Lingua.
lingua-nome = Italiano
# Numero di file, con il plurale. Barra di stato, ispettore, top list. "File" è invariabile.
conteggio-file = { $n ->
    [one] { $testo } file
   *[other] { $testo } file
}
# Numero di cartelle, con il plurale. Barra di stato, ispettore.
conteggio-cartelle = { $n ->
    [one] { $testo } cartella
   *[other] { $testo } cartelle
}

## Unità di misura (come Esplora file)

# Unità "byte", con il plurale. Esplora file in italiano scrive "byte" anche al plurale.
unita-byte = { $n ->
    [one] byte
   *[other] byte
}
# Kilobyte come in Esplora file (1 KB = 1024 byte).
unita-kb = KB
# Megabyte.
unita-mb = MB
# Gigabyte.
unita-gb = GB
# Terabyte.
unita-tb = TB
# Petabyte.
unita-pb = PB
# Kilobyte nel sistema SI (1000 byte), con la k minuscola.
unita-si-kb = kB
# Simbolo dei secondi nelle durate: "14,7 s".
unita-secondi = s

## Elenco delle unità

# Nome di un'unità, come in Esplora file: "Disco locale (C:)". $etichetta è il nome che Windows dà all'unità.
volume-titolo = { $etichetta } ({ $lettera })
# Nome usato quando l'unità non ha un'etichetta, come fa Esplora file.
volume-senza-etichetta = Disco locale
# Uso dell'unità nella barra laterale, sotto la lettera: "402 GB su 476 GB · NTFS".
volume-uso = { $usato } su { $totale } · { $fs }
# Tipo di unità: disco interno. Minuscolo: sta nella riga sotto il nome.
unita-fissa = unità fissa
# Tipo di unità: chiavetta o disco esterno.
unita-rimovibile = unità rimovibile
# Tipo di unità: cartella condivisa di rete.
unita-di-rete = unità di rete
# Tipo di unità: lettore ottico.
unita-ottica = unità ottica
# Tipo di unità: disco in memoria, con il nome che usa Windows.
unita-ram = disco RAM
# Tipo di unità non riconosciuto da Windows.
unita-sconosciuta = tipo di unità sconosciuto
# Titolo della schermata con l'elenco delle unità. È anche il nome con cui le altre schermate la citano.
volumi-titolo = Unità
# Riga sotto il titolo dell'elenco delle unità: cosa fare e cosa legge la scansione.
volumi-meta = Scegli l'unità da scansionare. La scansione elenca nomi, dimensioni e date dei file, senza leggerne il contenuto.
# Pulsante che avvia la scansione di un'unità: "Scansiona C:".
volumi-scansiona = Scansiona { $lettera }
# Uso dell'unità nell'elenco, con la forma di Esplora file: "402 GB usati su 476 GB".
volumi-uso = { $usato } usati su { $totale }
# Indicazione accanto all'unità dove è installato Windows.
volumi-di-sistema = unità di sistema
# Nota per le unità exFAT e FAT32: prima l'effetto sulle misure, poi cosa manca, senza allarmare.
volumi-nota-fat = Su { $fs } le dimensioni su disco sono arrotondate al cluster. Questo file system non ha compressione né file con più nomi (hard link).
# Nota per i file system diversi da NTFS, exFAT e FAT32 (ReFS, unità di rete): la dicitura "senza garanzie" e
# il fatto che la sostiene. I totali sono provati con l'oracolo solo sui tre file system.
volumi-senza-garanzie = Misure senza garanzie: i totali di Quado sono provati solo su NTFS, exFAT e FAT32.
# Nota in fondo all'elenco delle unità: cosa non compare e cosa Quado non legge.
volumi-nota = Le partizioni senza lettera di unità, come quella di un altro sistema operativo, restano fuori da questo elenco: Quado non le legge.
# Stato vuoto dell'elenco delle unità (l'elenco si legge all'avvio): cosa manca, poi una sola azione.
volumi-nessuno = Windows non elenca nessuna unità con una lettera. Collega il disco o assegnagli una lettera, poi riavvia Quado.
# Pulsante per tornare alla Panoramica dall'elenco delle unità.
volumi-torna = Torna alla Panoramica

## Barra del titolo e barra laterale

# Testo del selettore dell'unità prima della prima scansione: il selettore apre l'elenco delle unità.
barra-volumi = Scegli un'unità
# Suggerimento sul selettore dell'unità: cosa fa il clic.
barra-cambia-volume = Apre l'elenco delle unità.
# Pulsante che ripete la scansione dell'unità. Altri testi lo citano tra «».
barra-riesamina = Scansiona di nuovo
# Suggerimento del pulsante «Scansiona di nuovo»: cosa fa, con la scorciatoia. $volume è la lettera ("C:").
barra-riesamina-suggerimento = Rifà la scansione dell'unità { $volume } e aggiorna tutti i totali (F5).
# Testo grigio dentro la casella di ricerca vuota. Cerca solo cartelle e file, non comandi.
ricerca-suggerimento = Cerca cartelle e file
# Scorciatoia mostrata a destra nella casella di ricerca, scritta come le altre scorciatoie.
ricerca-scorciatoia = Ctrl+F
# Voce della barra laterale: il quadro dell'unità.
nav-panoramica = Panoramica
# Voce della barra laterale: mappa e tabella delle cartelle.
nav-esplora = Esplora
# Voce della barra laterale: elenco dei file più grandi.
nav-file-grandi = File grandi
# Titolo piccolo della sezione con le unità nella barra laterale, sopra l'unità scansionata.
nav-volumi = Unità
# Voce in fondo alla barra laterale.
nav-impostazioni = Impostazioni
# Nota in fondo alla barra laterale. Quado sposta file nel Cestino solo dopo l'anteprima e la conferma: non
# promettere che non tocca niente.
nav-nota-sola-lettura = La scansione legge soltanto. Quado sposta file nel Cestino solo dopo l'anteprima e la tua conferma.
# Suggerimento e nome accessibile del pulsante che riduce la barra laterale alle sole icone.
barra-comprimi = Comprimi la barra laterale
# Suggerimento e nome accessibile del pulsante che riapre la barra laterale con i nomi delle viste.
barra-espandi = Espandi la barra laterale
# Suggerimento del pulsante della palette dei comandi (Ctrl+K): porta a una vista, a una cartella o a un comando.
barra-palette = Vai a
# Segnaposto della casella della palette dei comandi (Ctrl+K): cosa si può raggiungere da lì.
palette-segnaposto = Vai a una vista, a una cartella o a un comando
# Palette dei comandi: titolo del gruppo delle schermate (Panoramica, Esplora, Impostazioni...).
palette-gruppo-viste = Viste
# Palette dei comandi: titolo del gruppo delle cartelle e dei file trovati per nome nella scansione.
palette-gruppo-elementi = Cartelle e file
# Palette dei comandi: titolo del gruppo dei comandi (Scansiona di nuovo, Copia percorso...).
palette-gruppo-comandi = Comandi
# Piede della palette, dopo i tasti freccia su e giù: cosa fanno.
palette-sposta = per scegliere
# Piede della palette, dopo il tasto Invio: cosa fa.
palette-apri = per aprire
# Piede della palette, dopo il tasto Esc: cosa fa.
palette-chiudi = per chiudere
# Palette dei comandi senza voci per il testo scritto. Cosa manca, poi cosa provare (come ricerca-nessun-risultato).
palette-nessuno = Nessuna voce con questo testo. La ricerca trova anche una parte del nome: prova con meno lettere.
# Nome del tasto Esc nei riquadri dei tasti (palette dei comandi), come sulla tastiera italiana.
tasto-esc = Esc
# Nome del tasto Invio nei riquadri dei tasti (palette dei comandi), come sulla tastiera italiana.
tasto-invio = Invio

## Barra di stato

# Durante la scansione, con il contatore vero: "Scansione in corso… 1.234.567 file letti".
stato-scansione-in-corso = Scansione in corso… { $file } file letti
# Numero di cartelle che non si sono potute leggere.
stato-non-leggibili = { $n ->
    [one] { $testo } cartella non leggibile
   *[other] { $testo } cartelle non leggibili
}
# Come si conta lo spazio: dimensioni su disco, un file con più nomi conta una volta.
stato-metodo = Dimensioni su disco, ogni file contato una volta
# Come è avviato Quado, a destra nella barra di stato: non elevato. Parla di Quado, non dell'utente: con UAC
# anche un amministratore avvia i programmi senza elevazione (docs/testi.md §6).
stato-senza-privilegi = Quado non è avviato come amministratore
# Come è avviato Quado, a destra nella barra di stato: elevato, con «Esegui come amministratore».
stato-amministratore = Avviato come amministratore

## Avvisi (banner)

# Banner su ogni schermata con dati parziali: la conseguenza e cosa fare. Cita il pulsante barra-riesamina.
banner-interrotta = Scansione interrotta: i totali non comprendono le cartelle non lette. Per completarli, usa «Scansiona di nuovo».
# Banner per exFAT e FAT32: l'effetto sulle misure, poi cosa manca.
banner-fat = Unità { $fs }: le dimensioni su disco sono arrotondate al cluster. Questo file system non ha compressione né file con più nomi (hard link).
# Banner per gli altri file system: la dicitura "senza garanzie" e il fatto che la sostiene.
banner-senza-garanzie = File system { $fs }: misure senza garanzie. I totali di Quado sono provati solo su NTFS, exFAT e FAT32.
# Banner quando la scansione non parte: cosa non è riuscito, cosa fare, il codice di Windows in fondo.
banner-errore-scansione = Impossibile leggere { $volume }. Controlla che l'unità sia collegata, poi riprova dall'elenco delle unità. (Codice { $codice })

## Scansione in corso

# Titolo della schermata di avanzamento: "Scansione di Disco locale (C:)".
scansione-titolo = Scansione di { $volume }
# Riga sotto il titolo della scansione: modalità e cosa succede se annulli. Quado non è avviato come amministratore.
scansione-meta = Sola lettura, Quado non è avviato come amministratore. Se annulli, resta visibile quello che è già stato letto.
# Come scansione-meta, con Quado avviato come amministratore.
scansione-meta-amministratore = Sola lettura, come amministratore. Se annulli, resta visibile quello che è già stato letto.
# Etichetta sotto il numero di file letti.
scansione-file = file letti
# Etichetta sotto il numero di cartelle lette.
scansione-cartelle = cartelle lette
# Etichetta sotto lo spazio già misurato.
scansione-letti = misurati finora
# Etichetta sotto il tempo trascorso.
scansione-tempo = trascorsi
# Pulsante per fermare la scansione.
scansione-annulla = Annulla
# Pulsante, disattivato, mentre la scansione si sta fermando.
scansione-annullo = Annullamento…
# Suggerimento accanto al pulsante Annulla.
scansione-esc = Esc annulla.
# Titolo dell'elenco delle cartelle di primo livello che crescono durante la scansione.
scansione-primo-livello = Primo livello dell'unità
# Nota sotto il titolo dell'elenco di primo livello.
scansione-primo-livello-nota = I totali crescono mentre la scansione procede.
# Testo prima che arrivi l'elenco della radice.
scansione-attesa = Lettura della radice…
# Riga in fondo quando non tutte le voci stanno nello spazio.
scansione-altri = { $n ->
    [one] e un altro elemento
   *[other] e altri { $testo } elementi
}
# Avviso nella barra di stato a fine scansione. $n cartelle con accesso negato, $testo lo stesso numero già
# scritto, $durata già scritta. Il fatto misurato è l'accesso negato da Windows; nessuna causa per il non
# attribuito oltre alle cartelle.
scansione-completata = { $n ->
    [0] Scansione completata in { $durata }.
    [one] Scansione completata in { $durata }. Windows ha negato l'accesso a una cartella: il suo contenuto resta nel non attribuito.
   *[other] Scansione completata in { $durata }. Windows ha negato l'accesso a { $testo } cartelle: il loro contenuto resta nel non attribuito.
}

## Panoramica

# Riga sotto il nome dell'unità: file system e dimensione del cluster.
panoramica-meta-cluster = { $fs } · cluster da { $cluster }
# Quando è stata fatta la scansione, a destra in alto. Quado non è avviato come amministratore.
panoramica-scansione = Scansione del { $data } alle { $ora } · { $durata } · non come amministratore
# Come panoramica-scansione, con Quado avviato come amministratore.
panoramica-scansione-amministratore = Scansione del { $data } alle { $ora } · { $durata } · come amministratore
# Titolo della card del bilancio.
bilancio-titolo = Come è usato lo spazio
# Nota sulle unità di misura, a destra nel titolo del bilancio (Esplora file).
bilancio-unita-esplora = GB come in Esplora file (1 GB = 1024³ byte)
# Nota sulle unità di misura quando l'utente ha scelto il sistema SI.
bilancio-unita-si = GB nel sistema SI (1 GB = 10⁹ byte)
# Etichetta accanto alla cifra grande dello spazio usato.
bilancio-usati = usati
# Etichetta accanto alla cifra grande dello spazio libero.
bilancio-liberi = liberi
# Capacità dell'unità dopo le due cifre grandi: "su 476 GB".
bilancio-su = su { $totale }
# Sotto la cifra grande dello spazio usato in Panoramica: "usati su 476 GB".
bilancio-usati-su = usati su { $totale }
# Segmento della barra: file letti, ogni file contato una volta.
bilancio-file-letti = File letti
# Segmento della barra: metadati di NTFS misurati ($MFT, indici delle cartelle, journal).
bilancio-metadati = Metadati di NTFS
# Segmento della barra: spazio che Windows tiene da parte e non è in nessun file (archiviazione riservata compresa), letto senza privilegi.
bilancio-riservato = Spazio riservato da Windows
# Segmento a tratteggio: spazio usato che la scansione non ha attribuito. Non dire perché.
bilancio-non-attribuito = Non attribuito
# Segmento dello spazio libero.
bilancio-libero = Libero
# Valore del non attribuito con l'incertezza della deriva: "9,30 GB ± 180 KB".
bilancio-deriva = { $valore } ± { $deriva }
# Suggerimento su un segmento della barra: quota della capacità dell'unità.
bilancio-quota-volume = { $quota } dell'unità
# Nota sulla deriva: lo spazio usato è cambiato durante la scansione.
bilancio-nota-deriva = Durante la scansione lo spazio usato è cambiato di { $deriva }: è l'incertezza sul non attribuito.
# Nota sulla quota disco dell'utente.
bilancio-nota-quota = Una quota disco riserva { $quota } dello spazio libero ad altri account.
# Testo nelle card del bilancio e dei conti quando la scansione non è di un'unità intera (per esempio di una cartella).
bilancio-non-disponibile = Disponibile solo dopo la scansione di un'unità intera.
# Titolo della top list.
top-titolo = Dove finisce lo spazio
# Sottotitolo della top list, a destra del titolo.
top-sottotitolo = Elementi più grandi, di qualunque tipo
# Riga di spiegazione di una cartella nella top list: dove sta e quanti file contiene. Solo misure.
top-spiegazione-cartella = In { $percorso } · { $file }
# Riga di spiegazione di un file nella top list.
top-spiegazione-file = File in { $percorso }
# Suggerimento su una riga della top list: il valore e cosa fa il clic.
top-suggerimento = { $quota } dello spazio usato. Fai clic per aprire l'elemento in Esplora.
# Testo se la top list è vuota (unità vuota o scansione interrotta subito).
top-vuoto = Nessun elemento occupa spazio su disco in questa scansione.
# Inizio della legenda dei glifi sotto la top list: misurato, da fonte, dedotto, non disponibile.
top-legenda-fonte = Fonte di ogni fatto:
# Titolo della card dei conti. Il tono è descrittivo, non un allarme.
conti-titolo = Perché i conti non tornano del tutto
# Riga: usato secondo Windows.
conti-usato-windows = Spazio usato secondo Windows
# Riga: file letti, ogni file contato una volta.
conti-file-letti = File letti, ogni file contato una volta
# Riga dei metadati con il dettaglio. I nomi tecnici restano: servono a chi vuole verificare.
conti-metadati = Metadati di NTFS misurati: $MFT { $mft }, indici delle cartelle { $indici }, journal { $journal }
# Riga finale: il resto. Non dire da cosa dipende.
conti-non-attribuito = Non attribuito
# Nota sui file con più nomi: già contati una volta, non una riga da sottrarre. $byte è quanto si
# aggiungerebbe contando ogni nome.
conti-hard-link = File con più nomi (hard link): { $testo } nomi in più per gli stessi dati, già contati una volta. Contando ogni nome si aggiungerebbero { $byte }.
# Titolo della lista dei fatti osservati.
conti-osservato = Osservato
# Cartelle non leggibili per qualunque motivo, con due esempi di nome. Nessun motivo nel testo: non tutte
# sono negate per i privilegi.
conti-non-leggibili = { $n ->
    [one] { $testo } cartella non leggibile: { $esempi }
   *[other] { $testo } cartelle non leggibili, tra cui { $esempi }
}
# Cestini di altri account (cartelle S-1-... non leggibili in $Recycle.Bin).
conti-cestini-altri = { $n ->
    [one] Un Cestino di un altro account, non leggibile
   *[other] { $testo } Cestini di altri account, non leggibili
}
# Profili di altri account (cartelle non leggibili in Users).
conti-profili-altri = { $n ->
    [one] Un profilo di un altro account, non leggibile
   *[other] { $testo } profili di altri account, non leggibili
}
# Lista "Osservato" vuota.
conti-osservato-nulla = Tutte le cartelle sono state lette.
# Titolo della lista di ciò che si misura solo come amministratore (docs/testi.md §6).
conti-possibile = Possibile, misurabile solo come amministratore
# Voce della lista "Possibile", con il termine del glossario.
conti-copie-shadow = Punti di ripristino e copie shadow
# Voce della lista "Possibile", con il nome che usa Windows.
conti-archiviazione-riservata = Archiviazione riservata
# Voce della lista "Possibile". I nomi tecnici servono a chi vuole verificare.
conti-altri-metafile = Altri metafile di NTFS ($LogFile, $Bitmap, $Secure)
# Riga tra i metadati misurati: spazio che Windows tiene riservato e non usato (GetDiskSpaceInformationW, senza privilegi), archiviazione riservata compresa. Solo il fatto.
conti-riservato = Spazio riservato da Windows, archiviazione riservata compresa
# Voce di "Osservato" dopo "Ripeti come amministratore": cartelle negate lette come amministratore, con lo spazio su disco.
conti-lette-amministratore = { $n ->
    [one] Una cartella letta come amministratore: { $dimensione }
   *[other] { $testo } cartelle lette come amministratore: { $dimensione }
}
# Voce di "Osservato": Cestino di un altro account misurato come totale. Il SID serve solo a distinguere i Cestini.
conti-cestino-altro = Cestino di un altro account ({ $sid }): { $dimensione }
# Come sopra, quando il SID non corrisponde più a nessun account di questo PC.
conti-cestino-orfano = Cestino di un account che non esiste più su questo PC ({ $sid }): { $dimensione }
# Voce di "Osservato": profili di altri account misurati come totale, mai con nomi di file o cartelle. $file è il numero di file già scritto ("1.234 file").
conti-profili-misurati = { $n ->
    [one] Un profilo di un altro account: { $dimensione } in { $file }
   *[other] { $testo } profili di altri account: { $dimensione } in { $file }
}
# Voce di "Osservato": spazio di punti di ripristino e copie shadow (Win32_ShadowStorage). I loro file stanno in System Volume Information: il valore non si somma ai conti.
conti-copie-shadow-misurate = Punti di ripristino e copie shadow: { $usato } usati, { $allocato } allocati, massimo { $massimo }. Questo spazio sta in System Volume Information e non si somma ai conti.
# Come sopra, quando Windows non fissa un massimo.
conti-copie-shadow-senza-limite = Punti di ripristino e copie shadow: { $usato } usati, { $allocato } allocati, senza un massimo. Questo spazio sta in System Volume Information e non si somma ai conti.
# Voce di "Osservato": sull'unità non c'è spazio assegnato alle copie shadow.
conti-copie-shadow-nessuna = Punti di ripristino e copie shadow: nessuno spazio assegnato su questa unità.
# Voce di "Osservato": il servizio Copia shadow del volume di Windows (nome del servizio, resta così) è disattivato, quindi Windows non dice lo spazio delle copie shadow.
conti-copie-shadow-servizio-spento = Punti di ripristino e copie shadow: il servizio Copia shadow del volume è disattivato su questo PC. Senza il servizio, Windows non indica lo spazio assegnato.
# Voce di «Possibile» quando la lettura come amministratore delle copie shadow non è riuscita: forma degli errori, codice in fondo.
conti-copie-shadow-errore = Impossibile leggere lo spazio di punti di ripristino e copie shadow. (Codice { $codice })

## Ripeti come amministratore

# Spiegazione sopra il pulsante "Ripeti come amministratore": cosa legge Quado, cosa non legge, poi cosa
# chiede Windows. Solo fatti, veri con e senza firma: niente citazione esatta del riquadro di Windows, che
# cambia con la versione e la lingua.
helper-spiegazione = { $n ->
    [one] Come amministratore, Quado legge solo la cartella con accesso negato e lo spazio di punti di ripristino e copie shadow. Non legge il contenuto dei file. Di profili e Cestini di altri account mostra solo i totali. Windows chiede conferma: per un programma senza firma digitale indica l'autore come sconosciuto.
   *[other] Come amministratore, Quado legge solo le { $testo } cartelle con accesso negato e lo spazio di punti di ripristino e copie shadow. Non legge il contenuto dei file. Di profili e Cestini di altri account mostra solo i totali. Windows chiede conferma: per un programma senza firma digitale indica l'autore come sconosciuto.
}
# Pulsante della card dei conti: apre la richiesta di conferma di Windows. Nome fisso, citato da altri testi.
helper-ripeti = Ripeti come amministratore
# Stato: Windows mostra la richiesta di conferma (prompt UAC).
helper-attesa = In attesa della conferma di Windows…
# Avanzamento: cartelle con una risposta su quelle chieste.
helper-lettura = { $n ->
    [one] Lettura come amministratore in corso… { $fatte } di { $testo } cartella
   *[other] Lettura come amministratore in corso… { $fatte } di { $testo } cartelle
}
# Pulsante: ferma la lettura come amministratore.
helper-annulla = Annulla
# Esito: lettura finita, cartelle lette su quelle chieste.
helper-fine = { $n ->
    [one] Lettura come amministratore completata: { $lette } di { $testo } cartella.
   *[other] Lettura come amministratore completata: { $lette } di { $testo } cartelle.
}
# Esito: cartelle che non si sono aperte neanche come amministratore, con i codici di Windows in fondo. Restano tra le non leggibili.
helper-fine-non-lette = { $n ->
    [one] Una cartella non si è aperta neanche come amministratore e resta tra le non leggibili. (Codice { $codici })
   *[other] { $testo } cartelle non si sono aperte neanche come amministratore e restano tra le non leggibili. (Codici { $codici })
}
# Esito: risposte su cartelle che nel frattempo non sono più nel modello (riscansione o nuova scansione).
helper-fine-ignorate = { $n ->
    [one] Una risposta riguarda una cartella che non è più nell'elenco: Quado non l'ha applicata.
   *[other] { $testo } risposte riguardano cartelle che non sono più nell'elenco: Quado non le ha applicate.
}
# Esito: la lettura si è chiusa con un codice di errore; le risposte arrivate prima sono applicate.
helper-fine-codice = La lettura come amministratore si è chiusa con un errore: le risposte arrivate prima sono applicate. (Codice { $codice })
# Esito: Windows non ha abilitato il privilegio di backup (nome tecnico, serve a chi verifica) e le cartelle negate restano non lette.
helper-senza-privilegio = Windows non ha concesso il privilegio di backup: le cartelle con accesso negato restano non lette.
# Esito: risposta No alla richiesta di Windows.
helper-rifiutato = Hai risposto No alla richiesta di Windows: niente è cambiato.
# Esito: lettura annullata con "Annulla".
helper-annullato = Lettura come amministratore annullata: niente è cambiato.
# Errore prima della fine: cosa non è riuscito, cosa resta e cosa fare, poi il motivo. $motivo è una delle voci helper-motivo,
# una frase completa; quando c'è il codice di Windows, sta in fondo.
helper-errore = Impossibile leggere come amministratore. Niente è cambiato: puoi riprovare. { $motivo }
# Motivo di helper-errore: solo il codice di Windows, tra parentesi come in fondo agli altri errori.
helper-motivo-codice = (Codice { $codice })
# Motivo di helper-errore: il processo di lettura si è chiuso prima della fine, con il suo codice.
helper-motivo-uscita = Il processo di lettura si è chiuso prima della fine. (Codice { $codice })
# Motivo di helper-errore: nessuna connessione entro il tempo.
helper-motivo-tempo = Nessuna risposta entro { $secondi } secondi.
# Motivo di helper-errore: dall'altra parte della connessione non c'era il processo avviato.
helper-motivo-processo = Ha risposto un processo diverso da quello avviato.
# Motivo di helper-errore: messaggio non valido dal processo di lettura.
helper-motivo-protocollo = Il processo di lettura ha mandato un messaggio non valido.
# Motivo di helper-errore: connessione chiusa prima della fine.
helper-motivo-chiuso = La connessione si è chiusa prima della fine.
# Esito: quado.exe sta in Download o in una cartella temporanea, da cui Quado non si avvia come amministratore.
# Lì un altro programma può lasciare una DLL esca. Il controllo si fa prima della richiesta di Windows, che non
# compare. Cosa non è riuscito, perché, poi cosa fare: il percorso d'esempio è quello dove si installa Quado.
helper-posizione = Impossibile leggere come amministratore da questa cartella. Per sicurezza Quado non si avvia come amministratore da Download o da una cartella temporanea. Sposta quado.exe in una cartella sua, per esempio %LOCALAPPDATA%\Programs\Quado, e riaprilo da lì.

## Esplora

# Nome e suggerimento del pulsante con la sola icona: cartella precedente.
esplora-indietro = Indietro (Alt+Freccia sinistra)
# Nome e suggerimento del pulsante con la sola icona: cartella successiva.
esplora-avanti = Avanti (Alt+Freccia destra)
# Nome e suggerimento del pulsante con la sola icona: va alla cartella che contiene quella attuale.
esplora-su = Vai alla cartella superiore (Backspace)
# Dopo il percorso: dimensione e quota dello spazio usato (non del disco).
esplora-dimensione-quota = { $dimensione } · { $quota } dello spazio usato
# Scelta della vista: mappa sopra, tabella sotto.
esplora-mappa-ed-elenco = Mappa ed elenco
# Scelta della vista: solo tabella.
esplora-solo-elenco = Solo elenco
# Tabella dell'Esplora vuota: la cartella è stata letta e non contiene niente.
esplora-vuoto = Questa cartella è vuota.
# Tabella dell'Esplora vuota perché la cartella non si è potuta leggere o la scansione si è fermata prima.
# Cita il pulsante barra-riesamina.
esplora-vuoto-non-letta = Quado non ha letto il contenuto di questa cartella: il motivo è nell'ispettore. «Scansiona di nuovo» rilegge tutta l'unità.
# Elenco di Esplora vuoto con i soli filtri attivi (senza un nome cercato): nessun elemento della cartella
# aperta li soddisfa. Cita il pulsante filtro-azzera.
esplora-filtri-nessuno = Nessun elemento in questa cartella con questi filtri. Prova con «Togli i filtri».
# Legenda: il colore grigio dei dati non classificati.
gestore-non-classificato = Non classificato
# Nome del gruppo di colore dei programmi.
gestore-app = Programmi e dati delle app
# Nome del gruppo di colore dei tuoi file.
gestore-tuoi = Tuoi file
# Nome del gruppo di colore di Windows.
gestore-windows = Windows
# Legenda: il tratteggio.
legenda-non-leggibile = Non leggibile
# Suggerimento sopra un'area della mappa: dimensione e quota dello spazio usato.
mappa-suggerimento = { $dimensione } · { $quota } dello spazio usato
# Intestazione di colonna.
colonna-nome = Nome
# Intestazione di colonna: come in Esplora file.
colonna-dimensioni-su-disco = Dimensioni su disco
# Intestazione di colonna: quota della cartella che contiene l'elemento.
colonna-quota = Quota della cartella
# Intestazione di colonna: numero di file contenuti.
colonna-file = File
# Intestazione di colonna, come in Esplora file.
colonna-ultima-modifica = Ultima modifica
# Intestazione di colonna: chi gestisce l'elemento.
colonna-gestito-da = Gestito da
# Intestazione di colonna nei risultati: la cartella che contiene l'elemento.
colonna-cartella = Cartella
# Cella "Dimensioni su disco" di una cartella non leggibile: mai 0.
tabella-non-leggibile = non leggibile
# Cella "Gestito da" prima della classificazione.
tabella-non-classificato = non classificato
# Indicazione accanto al nome di una junction o di un collegamento simbolico.
tag-collegamento = collegamento, non seguito
# Indicazione accanto al nome di una cartella non leggibile.
tag-non-leggibile = non leggibile
# Indicazione accanto a un file segnaposto del cloud.
tag-cloud = nel cloud
# Indicazione accanto a una cartella cloud non elencata per non scaricarne il contenuto.
tag-cloud-non-elencata = cloud, non elencata

# Nome della mappa per i lettori di schermo (non compare a video).
accessibile-mappa = Mappa delle cartelle
# Nome della tabella ad albero per i lettori di schermo (non compare a video).
accessibile-tabella = Tabella delle cartelle e dei file
# Nome degli elenchi piatti (ricerca, File grandi) per i lettori di schermo.
accessibile-elenco = Elenco di cartelle e file

## Ricerca e filtri

# Titolo sopra i risultati: "Risultati per «foto»".
ricerca-titolo = Risultati per «{ $testo }»
# Stato mentre la ricerca è in corso.
ricerca-in-corso = Ricerca in corso…
# Numero di risultati e tempo impiegato.
ricerca-risultati = { $n ->
    [one] { $testo } elemento · { $ms } ms
   *[other] { $testo } elementi · { $ms } ms
}
# Stato vuoto: nessun risultato e nessun filtro attivo. Cosa manca, poi cosa provare.
ricerca-nessun-risultato = Nessuna cartella o file con questo nome. La ricerca trova anche una parte del nome: prova con meno lettere.
# Stato vuoto: nessun risultato con almeno un filtro attivo. Cita il pulsante filtro-azzera.
ricerca-nessun-risultato-filtri = Nessuna cartella o file con questo nome e con questi filtri. Prova con «Togli i filtri».
# Pulsante che chiude la ricerca e torna all'albero.
ricerca-chiudi = Chiudi la ricerca
# Etichetta del filtro delle dimensioni minime (confronta le dimensioni su disco). L'unità è esplicita accanto al numero.
filtro-dimensione-minima = Dimensioni minime
# Sotto i filtri, quando le dimensioni minime scritte non sono un numero: cosa scrivere, poi l'effetto.
filtro-dimensione-non-valida = Scrivi le dimensioni minime in cifre, per esempio 1,5. Per ora il filtro non si applica.
# Etichetta del filtro per tipo.
filtro-tipo = Tipo
# Voce del filtro per tipo: nessun filtro.
filtro-tipo-tutti = Tutti i tipi
# Etichetta del filtro per data: lo stesso nome della colonna.
filtro-modificato = Ultima modifica
# Voce del filtro per data.
filtro-data-qualsiasi = In qualunque data
# Voce del filtro per data.
filtro-data-30-giorni = Negli ultimi 30 giorni
# Voce del filtro per data.
filtro-data-ultimo-anno = Nell'ultimo anno
# Voce del filtro per data.
filtro-data-oltre-un-anno = Più di un anno fa
# Voce del filtro per data.
filtro-data-oltre-tre-anni = Più di tre anni fa
# Pulsante che toglie tutti i filtri. Altri testi lo citano tra «».
filtro-azzera = Togli i filtri
# Nota sotto i filtri: il tipo viene dall'estensione, non dal contenuto.
filtro-nota-estensione = Tipi secondo l'estensione: Quado non legge il contenuto dei file.
# Famiglia di file secondo l'estensione.
famiglia-video = Video
# Famiglia di file secondo l'estensione.
famiglia-audio = Audio
# Famiglia di file secondo l'estensione.
famiglia-immagini = Immagini
# Famiglia di file secondo l'estensione.
famiglia-documenti = Documenti
# Famiglia di file secondo l'estensione: zip, rar, 7z.
famiglia-archivi = Archivi compressi
# Famiglia di file secondo l'estensione: ISO, VHDX e simili.
famiglia-immagini-disco = Immagini disco
# Famiglia di file secondo l'estensione: exe, msi, msix.
famiglia-installatori = Programmi e installatori
# Famiglia di file secondo l'estensione: dll, sys.
famiglia-componenti = Componenti di programmi
# Famiglia di file secondo l'estensione: sorgenti e configurazioni.
famiglia-codice = Codice e configurazioni
# Famiglia di file secondo l'estensione: db, sqlite, pak. "Archivi" resta solo per gli archivi compressi.
famiglia-database = Database e file di dati
# Famiglia di file secondo l'estensione: log e dump.
famiglia-log = Log e dump
# Famiglia di file con un'estensione non in elenco.
famiglia-altro = Altre estensioni
# Famiglia dei file senza estensione.
famiglia-senza-estensione = Senza estensione

## File grandi

# Titolo della schermata File grandi.
file-grandi-titolo = File grandi
# Riga sotto il titolo. $volume è la lettera ("C:").
file-grandi-meta = I file più grandi di { $volume } per dimensioni su disco, ogni file contato una volta: { $testo } in elenco.
# Testo mentre l'elenco si calcola.
file-grandi-calcolo = Calcolo dell'elenco…
# Stato vuoto quando i filtri escludono tutto: cosa manca e come togliere i filtri. Cita il pulsante filtro-azzera.
file-grandi-nessuno = Nessun file con questi filtri. Per vederli tutti, usa «Togli i filtri».
# Stato vuoto senza filtri (unità vuota o scansione interrotta subito). Cita il pulsante barra-riesamina.
file-grandi-vuoto = Nessun file misurato in questa scansione. «Scansiona di nuovo» rilegge tutta l'unità.

## Ispettore

# Testo dell'ispettore senza selezione.
ispettore-nessuna-selezione = Seleziona una cartella o un file per vederne le misure.
# Tipo della radice dell'unità.
tipo-unita = Unità
# Tipo di una cartella.
tipo-cartella = Cartella
# Tipo di una junction o di un collegamento simbolico.
tipo-collegamento = Collegamento a un'altra cartella
# Tipo di un file: "File MP4 · Video, secondo l'estensione".
tipo-file = File { $estensione } · { $famiglia }, secondo l'estensione
# Tipo di un file senza estensione.
tipo-file-senza-estensione = File · { $famiglia }
# Cifra grande di una cartella non leggibile (mai 0).
ispettore-non-leggibile = Non leggibile
# Quota dello spazio usato sotto la cifra grande.
ispettore-quota-usato = { $quota } dello spazio usato
# Fatto misurato: dimensione logica, come "Dimensioni" in Esplora file.
fatto-dimensioni = Dimensioni: { $dimensione } ({ $byte } byte)
# Fatto misurato: spazio allocato, come "Dimensioni su disco" in Esplora file.
fatto-dimensioni-su-disco = Dimensioni su disco: { $dimensione } ({ $byte } byte)
# Fatto misurato: data e ora dell'ultima modifica.
fatto-ultima-modifica = Ultima modifica: { $data } { $ora }
# Fatto non disponibile: la cartella non si legge. $motivo viene da «motivo-*»: una frase completa, con il codice di Windows in fondo quando c'è.
fatto-non-leggibile = Non leggibile: il contenuto resta nel non attribuito. { $motivo }
# Fatto: segnaposto cloud con le due misure.
fatto-cloud = Segnaposto cloud: { $nel-cloud } nel cloud, { $locale } su questo disco.
# Fatto non disponibile: cartella cloud non elencata, perché elencarla può scaricare dati dal cloud.
fatto-cloud-non-elencata = Cartella cloud non elencata: elencarla può avviare un download dal cloud.
# Fatto: junction o collegamento simbolico non seguito.
fatto-collegamento = Collegamento non seguito: il contenuto si conta nella cartella a cui punta.
# Fatto: file con più nomi (hard link), contato qui.
fatto-hard-link = File con più nomi (hard link): i suoi byte sono contati una volta, qui.
# Fatto: nome secondario di un file con più nomi, contato altrove.
fatto-hard-link-secondario = File con più nomi (hard link): i suoi byte sono già contati altrove, con un altro nome.
# Fatto: dimensioni su disco lette aprendo il file con soli attributi (compressione di Windows).
fatto-allocata-corretta = Dimensioni su disco lette dal file: Windows lo comprime.
# Fatto: attributi di Windows, elenco separato da virgole.
fatto-attributi = Attributi: { $elenco }
# Attributo di Windows.
attributo-compresso = compresso
# Attributo di Windows.
attributo-sparse = sparse
# Attributo di Windows.
attributo-cifrato = cifrato
# Attributo di Windows.
attributo-nascosto = nascosto
# Attributo di Windows.
attributo-sistema = di sistema
# Motivo di una cartella non leggibile, dopo fatto-non-leggibile: frase completa. Quado non è avviato come amministratore.
motivo-accesso-negato = Windows nega l'accesso se Quado non è avviato come amministratore.
# Come motivo-accesso-negato, con Quado avviato come amministratore: la cartella resta chiusa lo stesso.
motivo-accesso-negato-amministratore = Windows nega l'accesso anche con Quado avviato come amministratore.
# Motivo di una cartella non leggibile: frase completa.
motivo-in-uso = È in uso da un altro programma.
# Motivo di una cartella non leggibile: il "forse" resta, la causa non è misurata.
motivo-non-trovata = Windows non la trova più: forse è stata eliminata durante la scansione.
# Motivo: l'elenco si è interrotto a metà, con il codice di Windows in fondo.
motivo-interrotta = L'elenco si è interrotto a metà. (Codice { $codice })
# Motivo: scansione annullata prima di arrivare qui.
motivo-annullata = La scansione è stata annullata prima di arrivarci.
# Motivo generico, con il codice di Windows in fondo.
motivo-altro = Windows ha restituito un errore. (Codice { $codice })
# Titolo di sezione: le parti più grandi di una cartella (al più cinque).
ispettore-parti-principali = Parti più grandi
# Titolo di sezione: ripartizione per tipo.
ispettore-tipi-di-file = Tipi di file
# Nota sotto la ripartizione per tipo, uguale a quella sotto i filtri.
ispettore-secondo-estensione = Tipi secondo l'estensione: Quado non legge il contenuto dei file.
# Testo mentre la ripartizione per tipo si calcola.
ispettore-calcolo = Calcolo dei tipi di file…
# Testo se la cartella non contiene file.
ispettore-nessun-file = Nessun file in questa cartella.
# Titolo di sezione delle azioni.
ispettore-azioni = Azioni
# Pulsante: seleziona l'elemento in Esplora file senza aprirlo. Nome fisso, citato da altri testi.
azione-apri-in-esplora = Apri in Esplora file
# Pulsante: copia il percorso completo negli appunti.
azione-copia-percorso = Copia percorso
# Pulsante: finestra Proprietà di Windows.
azione-proprieta = Proprietà
# Suggerimento del pulsante Proprietà: la shell carica estensioni di altri programmi.
azione-proprieta-suggerimento = Apre la finestra Proprietà di Windows, che può caricare estensioni di altri programmi.
# Pulsante: rilegge l'elemento dal disco. Il verbo "verificare" resta allo stato Da verificare e alla verifica
# dopo un intervento.
azione-verifica = Rileggi dal disco
# Suggerimento del pulsante «Rileggi dal disco».
azione-verifica-suggerimento = Rilegge le dimensioni dal disco, in sola lettura, e aggiorna i totali.
# Mentre la rilettura è in corso.
verifica-in-corso = Rilettura in corso…
# Esito: nessun cambiamento.
verifica-invariata = Riletto alle { $ora }: dimensioni su disco invariate.
# Esito: dimensioni cambiate.
verifica-cambiata = Riletto alle { $ora }: dimensioni su disco da { $prima } a { $dopo }.
# Errore: la voce non si è potuta rileggere. Cosa non è riuscito, cosa resta, cosa fare, il codice in fondo.
# Cita il pulsante barra-riesamina.
verifica-errore = Impossibile rileggere l'elemento. Restano le misure della scansione: riprova, oppure usa «Scansiona di nuovo». (Codice { $codice })
# Classe di provenienza (legenda, suggerimento del glifo, «Provenienza della cartella»): letto da Quado in questa scansione. Parola fissa del glossario (docs/testi.md §6).
fonte-misurato = misurato
# Classe di provenienza: dichiarato da Windows o da un programma. Parola fissa del glossario (docs/testi.md §6).
fonte-da-fonte = da fonte
# Classe di provenienza: ricavato da indizi, può sbagliare. Parola fissa del glossario (docs/testi.md §6).
fonte-dedotto = dedotto
# Classe di provenienza: non leggibile o non misurabile.
fonte-non-disponibile = non disponibile

## Avvisi brevi nella barra di stato

# Esito dopo «Copia percorso». Esito breve: senza punto.
avviso-percorso-copiato = Percorso copiato negli appunti
# Errore quando Esplora file, Proprietà o una pagina di Impostazioni non si aprono. $codice è il codice esadecimale di Windows, in fondo tra parentesi.
avviso-shell-errore = Impossibile aprire la finestra. Riprova. (Codice { $codice })
# Esito dopo «Ricarica i file».
avviso-lingue-ricaricate = File delle lingue ricaricati
# Esito dopo «Crea un modello da tradurre».
avviso-modello-creato = Modello da tradurre creato nella cartella delle lingue
# Errore quando il modello da tradurre non si può scrivere. $codice è il codice di Windows.
avviso-errore-file = Impossibile creare il modello. Controlla di poter scrivere nella cartella delle lingue. (Codice { $codice })
# Avviso al primo avvio dopo il cambio di nome: impostazioni, registro e lingue copiati da %APPDATA%\Rilievo. $cartella è %APPDATA%\Quado.
avviso-rilievo-copiati = Dati di Rilievo copiati in { $cartella }
# Avviso: la copia dei dati di Rilievo non è riuscita; la cartella di Quado non c'è e la copia si riprova al prossimo avvio.
avviso-rilievo-errore = Impossibile copiare i dati di Rilievo: Quado parte con le impostazioni predefinite e riprova al prossimo avvio. (Codice { $codice })

## Impostazioni

# Titolo della schermata.
impostazioni-titolo = Impostazioni
# Titolo della card della lingua.
impostazioni-lingua = Lingua
# Tra parentesi accanto a una lingua che viene da un file dell'utente.
impostazioni-lingua-file-utente = dal tuo file
# Nota sotto la scelta della lingua. $cartella è il percorso. Il nome del file è il codice della lingua (i18n::user_files); un file it.ftl corregge l'italiano.
impostazioni-lingua-nota = Puoi correggere un testo o aggiungere una lingua con un file .ftl in { $cartella }. Il nome del file è il codice della lingua, per esempio it.ftl o fr.ftl. Poi scegli «Ricarica i file».
# Pulsante: apre la cartella delle lingue in Esplora file.
impostazioni-lingua-apri-cartella = Apri la cartella delle lingue
# Pulsante: copia il file italiano commentato nella cartella, come base per una traduzione.
impostazioni-lingua-modello = Crea un modello da tradurre
# Pulsante: rilegge i file delle lingue. Citato in impostazioni-lingua-nota e impostazioni-lingua-errori.
impostazioni-lingua-ricarica = Ricarica i file
# Titolo dell'elenco degli errori trovati nei file dell'utente (file:riga e dettaglio sotto). Dice cosa resta valido e cosa fare.
impostazioni-lingua-errori = { $n ->
    [one] Quado ignora una riga dei file delle lingue e usa tutte le altre. Correggila e scegli «Ricarica i file».
   *[other] Quado ignora { $n } righe dei file delle lingue e usa tutte le altre. Correggile e scegli «Ricarica i file».
}
# Titolo della card del tema.
impostazioni-tema = Tema
# Scelta del tema: segue la modalità chiara o scura di Windows.
tema-sistema = Come Windows
# Scelta del tema.
tema-chiaro = Chiaro
# Scelta del tema.
tema-scuro = Scuro
# Nota sotto il tema: l'impostazione di Windows che toglie le animazioni.
impostazioni-tema-nota = Quado non usa animazioni se in Windows gli «Effetti di animazione» sono disattivati.
# Titolo della card delle unità di misura.
impostazioni-unita = Unità di misura
# Scelta delle unità come Esplora file (esempio di docs/testi.md §4).
unita-scelta-esplora = Come in Esplora file (1 GB = 1024³ byte)
# Scelta delle unità decimali del Sistema internazionale.
unita-scelta-si = Decimali (1 GB = 10⁹ byte)
# Nota sotto le unità. Prima occorrenza nella schermata dei file con più nomi: il termine tecnico tra parentesi.
impostazioni-unita-nota = Con «Come in Esplora file» un singolo file ha le stesse dimensioni che vedi in Esplora file. Il totale di una cartella può differire perché Quado conta una volta sola i file con più nomi (hard link).
# Titolo della card delle informazioni sul programma.
impostazioni-informazioni = Informazioni
# Riga: versione di Quado.
info-versione = Versione
# Riga: scheda video e renderer.
info-grafica = Grafica
# Riga: percorso della cartella dei dati di Quado.
info-dati = Cartella dei dati
# Titolo del gruppo degli avvisi legali in Informazioni (GPLv3 §0): copyright, licenza, garanzia, testo della licenza.
info-licenza-titolo = Licenza
# Cosa permette la licenza e quale licenza è (GPLv3 §0, "Appropriate Legal Notices"). Nome della licenza in inglese, come nel testo ufficiale.
info-licenza = Quado è software libero: puoi ridistribuirlo e modificarlo secondo la GNU General Public License, versione 3 o (a tua scelta) successiva.
# Assenza di garanzia (GPLv3 §15 e §16). "Garanzia" qui è quella legale, non le misure di volumi-senza-garanzie.
info-garanzia = Quado è distribuito senza garanzia, nei limiti consentiti dalla legge: vedi le sezioni 15 e 16 della licenza.
# Dove sono le licenze dei componenti di altri autori: nello zip della release, non nell'eseguibile.
info-terze-parti = Le licenze delle librerie e dei font di altri autori sono nello zip della release: THIRD-PARTY-LICENSES.html e la cartella licenses.
# Riga apribile con il testo completo della licenza, incorporato nell'eseguibile. Il testo è in inglese.
info-testo-licenza = Testo della licenza, in inglese
# Riga: indirizzo del codice sorgente, con «Copia indirizzo» accanto. Quado non apre la pagina.
info-sorgente = Codice sorgente
# Sotto il marchio in Informazioni: il nome di prima. Lo hanno ancora la copia installata, il collegamento nel
# menu Start e la cartella dei dati della versione precedente.
info-nome-nota = Quado si chiamava Rilievo fino al 26/10/2026.
# Titolo della sezione con tema, accento, colori dei dati, testo, densità e animazioni. Compare anche nell'indice.
impostazioni-aspetto = Aspetto
# Nome accessibile dell'indice delle sezioni, a sinistra nella schermata.
impostazioni-indice = Sezioni delle impostazioni
# Riga della sezione Aspetto: il colore delle azioni.
impostazioni-accento = Colore di accento
# Sotto «Colore di accento»: cosa cambia e cosa resta uguale.
impostazioni-accento-nota = Pulsanti, selezione e focus. I colori dei dati restano gli stessi.
# Nome di un colore di accento: grigio quasi nero in chiaro, quasi bianco in scuro. Suggerimento e nome accessibile del campione.
accento-grafite = Grafite
# Nome di un colore di accento.
accento-viola = Viola
# Nome di un colore di accento.
accento-rosa = Rosa
# Riga della sezione Aspetto: i colori dei quattro gestori.
impostazioni-colori-dati = Colori dei dati
# Sotto «Colori dei dati». Forma e nome del gestore non cambiano: il colore non è mai l'unico segno.
impostazioni-colori-dati-nota = Colori dei quattro gestori nella barra, nelle legende e nella mappa. Forma e nome restano gli stessi.
# Scelta dei colori dei dati: quelli di partenza, tenui.
colori-dati-predefiniti = Predefiniti
# Scelta dei colori dei dati: più luminosi e saturi, con le etichette nere sulla mappa.
colori-dati-contrasto = Alto contrasto
# Scelta dei colori dei dati: distinguibili con protanopia, deuteranopia e tritanopia. Niente "sicuri": è una scelta di colori, non una garanzia.
colori-dati-discromatopsie = Per le discromatopsie
# Riga della sezione Aspetto: dimensione del testo, da 90% a 150%.
impostazioni-testo = Dimensione del testo
# Sotto «Dimensione del testo»: cosa cambia e le scorciatoie, scritte come le altre scorciatoie.
impostazioni-testo-nota = Ingrandisce o riduce testo, spazi e controlli di Quado. Scorciatoie: Ctrl+più e Ctrl+meno.
# Riga della sezione Aspetto: altezza delle righe.
impostazioni-densita = Densità
# Sotto «Densità».
impostazioni-densita-nota = Altezza delle righe di tabelle ed elenchi.
# Scelta della densità: righe più alte.
densita-comoda = Comoda
# Scelta della densità: righe più basse, più voci sullo schermo.
densita-compatta = Compatta
# Riga della sezione Aspetto: animazioni dell'interfaccia.
impostazioni-animazioni = Animazioni
# Sotto «Animazioni»: l'impostazione di Windows che le spegne comunque, con il nome che usa Windows.
impostazioni-animazioni-nota = Spente se in Windows gli «Effetti di animazione» sono disattivati.
# Scelta delle animazioni: seguono l'impostazione di Windows.
animazioni-windows = Come Windows
# Scelta delle animazioni: sempre spente.
animazioni-spente = Spente
# Riga della sezione Esplora: con quale vista si apre Esplora. Le scelte sono esplora-mappa-ed-elenco ed esplora-solo-elenco.
impostazioni-esplora-vista = Si apre con
# Riga della sezione Esplora: colonne facoltative della tabella. Le caselle usano i nomi delle colonne.
impostazioni-colonne = Colonne della tabella
# Sotto «Colonne della tabella»: le due colonne che restano sempre. Cita colonna-nome e colonna-dimensioni-su-disco.
impostazioni-colonne-nota = Nome e Dimensioni su disco si vedono sempre.
# Riga della sezione Esplora: barra laterale con i nomi delle viste o con le sole icone.
impostazioni-barra-laterale = Barra laterale
# Scelta: barra laterale con icone e nomi.
barra-laterale-espansa = Espansa
# Scelta: barra laterale con le sole icone.
barra-laterale-compressa = Compressa
# Titolo della colonna a destra con l'anteprima dal vivo di colori, densità e testo.
impostazioni-anteprima = Anteprima

## Classificazione, stati, record di raccomandazione, Windows e sistema, regole.

# Voce della barra laterale: vista Windows e sistema.
nav-sistema = Windows e sistema
# Voce della barra laterale, in basso: elenco delle regole.
nav-regole = Regole
# Voce della barra laterale: vista Download.
nav-download = Download
# Voce della barra laterale: vista Programmi.
nav-programmi = Programmi

# Banner della Panoramica quando ci sono regole verificate da più di 6 mesi. Dice la conseguenza: a 12 mesi dalla verifica la regola non propone più interventi.
banner-regole-da-riverificare = { $n ->
    [one] { $testo } regola da riverificare: è stata verificata più di sei mesi fa. A dodici mesi dalla verifica non propone più interventi.
   *[other] { $testo } regole da riverificare: sono state verificate più di sei mesi fa. A dodici mesi dalla verifica non propongono più interventi.
}
# Pulsante del banner: apre l'elenco delle regole (docs/testi.md §4).
banner-regole-apri = Apri le regole

# Legenda della barra del bilancio: grigio dei dati senza un gestore sicuro.
bilancio-non-classificato-ipotesi = Non classificato o ipotesi
# Legenda della mappa, stesso significato.
legenda-non-classificato-ipotesi = Non classificato o ipotesi
# Etichetta accanto a un'attribuzione incerta: grigio, nessun colore del gestore.
etichetta-ipotesi = ipotesi

# Titolo della card degli interventi possibili.
interventi-titolo = Interventi possibili
# Sottotitolo: da dove vengono le voci e dove si agisce. Dalla card non parte nessuna azione: un clic apre l'elemento con l'ispettore.
interventi-sottotitolo = Ogni voce viene da una regola, con la fonte e la data di verifica. Seleziona una voce per vedere nell'ispettore cosa puoi fare, con ipotesi e rischi. Da questa card Quado non cambia niente sul disco.
# Etichetta davanti alla cifra grande di una voce: quanto occupa l'elemento, non quanto si libera.
interventi-occupa = Occupa
# Sotto lo stato di una voce che sposta nel Cestino: lo spazio si libera solo svuotando il Cestino. $valore stima esatta.
interventi-libera-cestino = Si libera svuotando il Cestino: { $valore }
# Come sopra, con una stima a intervallo.
interventi-libera-cestino-intervallo = Si libera svuotando il Cestino: tra { $minimo } e { $massimo }
# Come sopra, quando la stima si calcola solo nell'anteprima.
interventi-libera-cestino-anteprima = Si libera svuotando il Cestino: lo calcola l'anteprima
# Sotto lo stato di una voce con una procedura o un comando: lo spazio liberato si misura dopo.
interventi-libera-procedura = Si libera: lo misura Quado dopo la procedura
# Voce che raccoglie più elementi della stessa regola o funzione. $nome è il nome della regola o della funzione.
interventi-gruppo = { $nome }: { $n ->
    [one] { $testo } elemento
   *[other] { $testo } elementi
}
# Voci non mostrate nella card: si vedono nell'ispettore dei loro elementi.
interventi-altri = { $n ->
    [one] Un'altra voce è nell'ispettore del suo elemento.
   *[other] Altre { $testo } voci sono nell'ispettore dei loro elementi.
}
# Card degli interventi senza voci (stato vuoto). Ricorda come ci arrivano le copie: solo dopo il confronto. Cita il pulsante della vista Download.
interventi-vuoto = Nessuna regola propone interventi su questa unità. Le copie identiche in Download compaiono qui dopo «Confronta il contenuto».
# Pulsante di una voce della card degli interventi: apre l'elemento in Esplora, dove l'ispettore dice cosa
# puoi fare. Le stesse parole della sezione dell'ispettore (ispettore-cosa-puoi-fare).
interventi-cosa-puoi-fare = Vedi cosa puoi fare

# Filtro "Gestito da" in ricerca e File grandi.
filtro-gestore = Gestito da
# Filtro senza vincolo.
filtro-gestore-tutti = Tutti

# Stati di valutazione: sempre con l'icona, parole fisse del glossario. Gestibile: prova di gestibilità diretta.
stato-gestibile = Gestibile
# Da verificare: si legge "non ancora", mai "puoi cancellare".
stato-da-verificare = Da verificare
# Decidi tu: ruolo noto, utilità non determinabile dal software.
stato-decidi-tu = Decidi tu
# Lo gestisce X, con la sua procedura. $chi è un nome (Windows, npm, CapCut).
stato-lo-gestisce = Lo gestisce { $chi }
# Variante senza procedura.
stato-lo-gestisce-nessuna-procedura = Lo gestisce { $chi }. Nessuna procedura: non toccare
# Non sappiamo: nessuna prova sul ruolo.
stato-non-sappiamo = Non sappiamo
# Perché Non sappiamo: nessuna regola. Cita il pulsante dell'ispettore.
motivo-stato-nessuna-regola = Nessuna regola riconosce questo elemento. Puoi vederne le misure e usare «Apri in Esplora file».
# Perché Non sappiamo: versione fuori dall'intervallo della regola.
motivo-stato-versione = La versione installata è fuori da quelle verificate per la regola, oppure non si conosce. Quado non propone interventi.
# Perché Non sappiamo: regola scaduta, oltre 12 mesi dalla verifica. $data è la data di verifica.
motivo-stato-regola-scaduta = La regola è stata verificata il { $data }, più di un anno fa. Quado mostra la spiegazione come ipotesi e non propone interventi.
# Perché Non sappiamo: due regole in conflitto.
motivo-stato-conflitto = Due regole descrivono questo elemento in modo diverso. Quado le mostra entrambe e non propone interventi.
# Perché Non sappiamo: solo il nome.
motivo-stato-solo-nome = Il ruolo di questo elemento si ricava solo dal nome. Quado non propone interventi.
# Perché Non sappiamo: manca la prova di gestibilità.
motivo-stato-senza-prova = La regola non trova una prova che agire su questo elemento abbia effetti noti. Quado non propone interventi.

# Affidabilità dei fatti dedotti: livello. Parola fissa del glossario ("dedotto alta", docs/testi.md §6).
affidabilita-alta = alta
# Livello medio.
affidabilita-media = media
# Livello basso.
affidabilita-bassa = bassa
# Riga dell'ispettore: livello e criterio.
affidabilita-riga = Affidabilità dei fatti dedotti: { $livello } ({ $criterio })
# Criterio: due fonti concordi.
criterio-due-fonti = due fonti concordi
# Criterio: fonte ufficiale con la struttura descritta.
criterio-fonte-ufficiale = fonte ufficiale e struttura come descritta
# Criterio: una sola fonte non ufficiale.
criterio-una-fonte = una sola fonte non ufficiale
# Criterio: struttura riconosciuta senza fonte.
criterio-struttura = struttura riconosciuta senza fonte ufficiale
# Criterio: solo nome.
criterio-nome = solo il nome o una somiglianza
# Criterio: versione fuori dall'intervallo della regola.
criterio-versione = versione fuori da quelle verificate per la regola
# Criterio: regola scaduta, oltre 12 mesi dalla verifica.
criterio-scaduta = regola verificata più di un anno fa
# Criterio: conflitto.
criterio-conflitto = regole in conflitto

# Tipo di azione del record, dopo «Azione:».
azione-tipo-cestino = spostamento nel Cestino, dopo l'anteprima
# Procedura di Windows o del programma che gestisce l'elemento.
azione-tipo-procedura = procedura di Windows o del programma che lo gestisce
# Comando mostrato e copiato, mai eseguito.
azione-tipo-comando = comando ufficiale, che Quado mostra e copia senza eseguirlo
# Disinstallazione dalla vista Programmi, con Impostazioni › App o con il launcher del gioco.
azione-tipo-disinstallazione = disinstallazione dalla vista Programmi, con Impostazioni › App o con il launcher
# Nessuna azione oltre a quelle sempre presenti nell'ispettore.
azione-tipo-nessuna = nessuna, oltre a quelle sempre disponibili
# Privilegi, dopo «Privilegi:»: basta l'account standard.
privilegi-nessuno = non serve essere amministratore
# Privilegi: amministratore. Quado non esegue comandi da amministratore.
privilegi-amministratore = il comando va eseguito come amministratore, quindi Quado lo mostra e lo copia senza eseguirlo
# Esito di un controllo che si esegue nell'anteprima prima di agire.
controllo-esito-simulazione = si controlla nell'anteprima
# Controllo superato.
controllo-esito-superato = superato
# Controllo non superato: l'avviso vale.
controllo-esito-non-superato = non superato
# Controllo manuale: lo spunta l'utente.
controllo-esito-da-spuntare = da spuntare
# Stima esatta. Compare sempre da sola, senza etichetta davanti (ispettore, card, vista Download, storico): per
# questo comincia con la parola Stima.
stima-esatta = Stima esatta: { $valore }
# Stima a intervallo.
stima-intervallo = Stima: tra { $minimo } e { $massimo }
# Non stimabile: file con altri nomi (hard link) in cartelle che la regola non conta (store di pnpm).
stima-non-stimabile-link = Non stimabile: i file hanno altri nomi (hard link) fuori da questa cartella
# Non stimabile: procedura o comando esterno. Il risultato lo dà la verifica dopo «Ho eseguito la procedura».
stima-non-stimabile-procedura = Non stimabile: il risultato si misura dopo la procedura
# Nessuna azione: niente da stimare.
stima-nessuna-azione = Nessuna stima: nessun intervento proposto
# Si calcola nell'anteprima: cartelle non leggibili o file con altri nomi.
stima-nella-simulazione = Si calcola nell'anteprima, quando Quado rilegge i file

# Ispettore, sezione "Cos'è".
ispettore-cos-e = Cos'è
# Fatto sotto "Cos'è" quando nessuna regola riconosce l'elemento.
ispettore-nessuna-regola = Nessuna regola lo riconosce
# Elemento dentro un elemento riconosciuto. $nome è il nome dell'elemento; la sua spiegazione (la riga della regola)
# sta nella riga sotto, perché spesso ha già i suoi due punti.
ispettore-fa-parte-di = Fa parte di { $nome }
# Pulsante: seleziona l'elemento riconosciuto nominato nella riga sopra.
ispettore-vai-elemento = Vai a quell'elemento
# Titolo dei fatti della prova di gestibilità.
ispettore-prova-gestibilita = Prova di gestibilità
# Riga del gestore.
ispettore-gestito-da = Gestito da: { $gestore }
# Sezione "Perché è così grande": schemi riconosciuti dalle regole.
ispettore-schemi = Riconosciuto dalle regole
# Sotto uno schema: la regola che lo riconosce.
ispettore-schema-regola = Regola: { $regola }
# Sezione "Cosa puoi fare".
ispettore-cosa-puoi-fare = Cosa puoi fare
# Nodo dentro un elemento riconosciuto: le azioni sono dell'elemento che lo contiene.
ispettore-azioni-elemento = Seleziona l'elemento che lo contiene per vedere cosa puoi fare.
# Sezione della regola.
ispettore-regola = Regola
# Altre regole che riconoscono lo stesso elemento.
ispettore-anche = Lo riconoscono anche: { $regole }
# Pulsante: apre la regola nell'elenco delle regole.
ispettore-vedi-regola = Apri la regola
# Sezione età dei file.
ispettore-eta = Età dei file
# Nota sotto l'età.
ispettore-eta-nota = Età calcolata dalla data di ultima modifica.
# Fascia di età.
eta-30-giorni = Ultimi 30 giorni
# Fascia di età.
eta-1-anno = Da 30 giorni a un anno
# Fascia di età.
eta-3-anni = Da uno a tre anni
# Fascia di età.
eta-oltre = Più di tre anni
# Campi del record: azione.
record-azione = Azione
# Pulsante accanto a un comando o a un indirizzo: lo copia negli appunti. Citato in procedura-comando-nota.
record-copia = Copia
# Titolo del metodo preferito.
record-metodo = Metodo
# Titolo delle alternative.
record-alternative = Alternative
# Titolo dei controlli preventivi.
record-controlli = Controlli prima di agire
# Privilegi.
record-privilegi = Privilegi
# Reversibilità.
record-reversibilita = Reversibilità
# Errore o esito parziale.
record-errore = Se si ferma a metà
# Verifica dopo l'azione.
record-verifica = Verifica
# Esito nella barra di stato dopo «Copia» (comando o indirizzo).
avviso-comando-copiato = Copiato negli appunti
# Esito nella barra di stato dopo «Copia indirizzo» nell'elenco delle regole.
avviso-link-copiato = Indirizzo copiato negli appunti

# Elenco delle regole: titolo.
regole-titolo = Regole
# Sottotitolo: quante regole, cosa ha ognuna e come vederne le fonti (un clic sulla card la apre).
regole-sottotitolo = { $n ->
    [one] Quado usa { $testo } regola. Ogni regola ha una fonte con l'estratto, una data di verifica e un caso di prova nei test di Quado. Seleziona una regola per vederne le fonti.
   *[other] Quado usa { $testo } regole. Ogni regola ha una fonte con l'estratto, una data di verifica e un caso di prova nei test di Quado. Seleziona una regola per vederne le fonti.
}
# Elenco delle regole vuoto: la base incorporata non si è caricata. Cita il pulsante dell'ispettore.
regole-vuoto = Impossibile leggere le regole incluse in Quado. Senza regole, per ogni elemento restano le misure e «Apri in Esplora file».
# Data di verifica.
regole-verificata = Verificata il { $data }
# Intervallo di versioni.
regole-versioni = { $programma } da { $min } a { $max }
# Oltre 6 mesi, sotto la riga «Verificata il».
regole-scadenza-riverificare = Da riverificare: la verifica del { $data } ha più di sei mesi.
# Oltre 11 mesi: quando la regola smette di proporre interventi. $limite è la data di verifica più 12 mesi.
regole-scadenza-annuncio = La verifica del { $data } vale fino al { $limite }. Se la regola non viene riverificata, da quel giorno non propone più interventi.
# Oltre 12 mesi.
regole-scadenza-scaduta = La verifica del { $data } ha più di un anno. La regola non propone più interventi e la sua spiegazione vale come ipotesi.
# Tipo di fonte.
regole-fonte-ufficiale = fonte ufficiale
# Tipo di fonte.
regole-fonte-non-ufficiale = fonte non ufficiale
# Pulsante: copia l'indirizzo della fonte negli appunti.
regole-copia-link = Copia indirizzo
# Fonte che è un file scritto da un programma sul PC (file:///): sopra il percorso dalla cartella nota.
regole-fonte-file = File del programma sul tuo PC
# Fonte che è una prova fatta con Quado (docs/): sopra il nome del documento nel codice sorgente.
regole-fonte-prova = Prova fatta con Quado, descritta in questo documento del codice sorgente
# Caso di prova.
regole-fixture = Caso di prova: { $nome }
# Numero di fonti, con la regola chiusa.
regole-fonti = { $n ->
    [one] { $testo } fonte
   *[other] { $testo } fonti
}

# Esito di un estrattore nella colonna dei valori: la fonte non c'è.
estrattore-non-presente = non presente
# Accesso negato senza amministratore.
estrattore-accesso-negato = si legge solo come amministratore
# Segnaposto cloud: il contenuto non si legge per non scaricarlo.
estrattore-segnaposto-cloud = non letto, il file è solo nel cloud
# Errore di Windows: il codice in fondo, tra parentesi.
estrattore-errore = non letto (Codice { $codice })
# Formato inatteso. $motivo dice quale (formato-*). Niente due punti: programmi-fonte-errore mette il valore dopo «Fonte:».
estrattore-non-riuscito = non letto, { $motivo }
# Motivi.
formato-troncato = il file finisce a metà
# Motivi.
formato-inatteso = formato inatteso
# Motivi.
formato-vuoto = il file è vuoto
# Motivi.
formato-troppo-annidato = troppi livelli uno dentro l'altro
# Motivi.
formato-codifica = codifica dei caratteri non valida
# Motivi.
formato-troppo-grande = il file supera il limite di lettura
# Motivi: $campo è il nome tecnico del campo.
formato-campo-mancante = manca il campo { $campo }
# Motivi.
formato-tipo-diverso = il campo { $campo } ha un tipo inatteso
# Motivi.
formato-valore = il campo { $campo } ha un valore inatteso

# Valori di configurazione.
valore-attivo = attivo
# Valori di configurazione.
valore-non-attivo = non attivo
# Valore assente nella configurazione letta.
valore-non-registrato = non impostato

# Vista Windows e sistema: titolo.
sistema-titolo = Windows e sistema
# Sottotitolo.
sistema-sottotitolo = Spazio usato da Windows su questa unità, con la misura e lo stato di ogni voce.
# Titolo della card delle voci.
sistema-voci = Voci di sistema
# Voci. Il nome del file o della cartella tra parentesi serve a ritrovarla.
sistema-voce-paging = File di paging (pagefile.sys)
# Voci.
sistema-voce-swap = File di swap (swapfile.sys)
# Voci.
sistema-voce-ibernazione = File di ibernazione (hiberfil.sys)
# Voci.
sistema-voce-winsxs = Archivio dei componenti (WinSxS)
# Voci.
sistema-voce-softwaredistribution = File di Windows Update (SoftwareDistribution)
# Voci.
sistema-voce-installer = Cache di Windows Installer
# Voci.
sistema-voce-driverstore = Archivio dei driver (DriverStore)
# Voci.
sistema-voce-cestino = Cestino
# Voci.
sistema-voce-windows-old = Installazione precedente di Windows (Windows.old)
# Voci non misurabili senza amministratore. Termine del glossario.
sistema-voce-copie-shadow = Punti di ripristino e copie shadow
# Voci non misurabili senza amministratore.
sistema-voce-archiviazione-riservata = Archiviazione riservata
# Metadati di NTFS.
sistema-voce-mft = Tabella dei file ($MFT)
# Metadati di NTFS.
sistema-voce-indici = Indici delle cartelle
# Metadati di NTFS.
sistema-voce-usn = Registro delle modifiche ai file ($UsnJrnl)
# Voce assente su questa unità.
sistema-non-presente = non presente
# Misura del Cestino secondo Windows.
sistema-cestino-windows = { $valore }, { $n ->
    [one] { $testo } elemento
   *[other] { $testo } elementi
}
# Etichetta.
sistema-cestino-secondo-windows = Secondo Windows
# Etichetta.
sistema-cestino-su-disco = Su disco
# Etichetta della riga con le cartelle del Cestino senza file indice, accanto alla spiegazione della regola cestino-orfane.
sistema-cestino-orfane = Cartelle senza indice
# Valore non disponibile senza amministratore: mai 0.
sistema-non-disponibile-admin = si misura solo come amministratore
# Metadati non disponibili su questo file system.
sistema-non-disponibile-fs = non disponibile su questo file system
# Nota in fondo alla card: un clic su una voce la apre in Esplora con l'ispettore.
sistema-nota-procedure = Seleziona una voce per vedere nell'ispettore la procedura o il comando di Microsoft, con la fonte in Regole. Quado mostra e copia i comandi senza eseguirli.

# Configurazione letta in sola lettura: titolo.
configurazione-titolo = Configurazione letta
# Sottotitolo.
configurazione-sottotitolo = Letta durante la scansione da Windows e dai file di configurazione dei programmi, senza cambiare niente.
# Voci.
configurazione-ibernazione = Ibernazione
# Voci.
configurazione-avvio-rapido = Avvio rapido
# Voci.
configurazione-batteria-critica = Azione con la batteria critica
# Valori dell'azione per batteria critica.
configurazione-batteria-0 = nessuna azione
# Valori.
configurazione-batteria-1 = sospensione
# Valori.
configurazione-batteria-2 = ibernazione
# Valori.
configurazione-batteria-3 = arresto
# Valori.
configurazione-batteria-4 = altro valore
# Voci.
configurazione-alimentazione = Alimentazione
# Voci.
configurazione-sensore = Sensore memoria
# Voci: valore configurazione-giorni o configurazione-sensore-poco-spazio.
configurazione-sensore-frequenza = Frequenza di Sensore memoria
# Valore della frequenza.
configurazione-sensore-poco-spazio = quando lo spazio libero è poco
# Durata in giorni, valore della frequenza e delle due voci «eliminati dopo».
configurazione-giorni = { $n ->
    [one] { $testo } giorno
   *[other] { $testo } giorni
}
# Voci: dopo quanti giorni Sensore memoria elimina gli elementi del Cestino (configurazione-giorni o «mai»).
configurazione-sensore-cestino = Elementi del Cestino eliminati dopo
# Voci: dopo quanti giorni Sensore memoria elimina i file di Download.
configurazione-sensore-download = File di Download eliminati dopo
# Valore: regola spenta.
configurazione-mai = mai
# Valore.
configurazione-mai-configurato = mai configurato
# Voci.
configurazione-cestino-capacita = Capacità del Cestino su questa unità
# Valore.
configurazione-capacita-predefinita = valore predefinito di Windows
# Voci: Proprietà del Cestino, «Non spostare i file nel Cestino».
configurazione-cestino-elimina-subito = Eliminazione senza passare dal Cestino
# Voci: il nome del valore nel registro serve a verificarlo.
configurazione-tracciamento = Tracciamento degli avvii delle app (Start_TrackProgs)
# Valore con Start_TrackProgs = 0: lo stesso testo di programmi-tracciamento-spento.
configurazione-tracciamento-spento = Tracciamento degli avvii di Windows disattivato: nessun dato d'uso
# Voci: nome di Windows della finestra.
configurazione-ripristino = Protezione sistema (punti di ripristino)
# Voci.
configurazione-cache-npm = Cache di npm
# Voci.
configurazione-store-pnpm = Store di pnpm
# Da dove viene il percorso.
configurazione-origine-ambiente = dalla variabile d'ambiente
# Da dove viene il percorso.
configurazione-origine-npmrc = da .npmrc
# Da dove viene il percorso.
configurazione-origine-predefinito = percorso predefinito della documentazione

# Fonti lette durante la scansione: titolo.
fonti-titolo = Fonti lette durante la scansione
# Fonte.
fonti-disinstallazione = Voci di disinstallazione
# Esito.
fonti-voci = { $n ->
    [one] { $testo } voce
   *[other] { $testo } voci
}{ $saltate ->
    [0] {""}
    [one] , { $saltate_testo } scartata per formato inatteso
   *[other] , { $saltate_testo } scartate per formato inatteso
}
# Fonte.
fonti-msix = Pacchetti di Microsoft Store
# Esito.
fonti-pacchetti = { $n ->
    [one] { $testo } pacchetto
   *[other] { $testo } pacchetti
}
# Fonte.
fonti-steam = Steam
# Esito.
fonti-giochi = { $n ->
    [one] { $testo } gioco
   *[other] { $testo } giochi
}
# Fonte.
fonti-epic = Epic Games Launcher
# Esito.
fonti-installazioni = { $n ->
    [one] { $testo } installazione
   *[other] { $testo } installazioni
}
# Fonte.
fonti-capcut = CapCut
# Esito.
fonti-versione = versione { $versione }
# Fonte.
fonti-userassist = Avvii registrati da Windows (UserAssist)
# Esito con il tracciamento spento.
fonti-userassist-non-usato = non letto, il tracciamento degli avvii è disattivato
# Esito.
fonti-voci-semplici = { $n ->
    [one] { $testo } voce
   *[other] { $testo } voci
}

# Fonte di un fatto dichiarato, dopo il fatto e «·». Codici di `Source`. In parole comprensibili; il nome tecnico, tra parentesi, solo dove serve a verificare il fatto.
origine-scansione = scansione
# Fonte: voce di disinstallazione nel registro, per tutti gli account.
origine-disinstallazione-hklm = voce di disinstallazione per tutti gli account (HKLM)
# Fonte: voce di disinstallazione dei programmi a 32 bit, per tutti gli account.
origine-disinstallazione-wow = voce di disinstallazione a 32 bit per tutti gli account (HKLM, WOW6432Node)
# Fonte: voce del tuo account, scrivibile dall'utente.
origine-disinstallazione-hkcu = voce di disinstallazione del tuo account (HKCU)
# Fonte: pacchetto MSIX.
origine-msix = pacchetto di Microsoft Store
# Fonte: appmanifest_*.acf della libreria di Steam.
origine-steam-manifest = dati di installazione di Steam (appmanifest)
# Fonte: localconfig.vdf di un account Steam.
origine-steam-localconfig = dati dell'account Steam su questo PC (localconfig.vdf)
# Fonte: file .item nella cartella Manifests di Epic Games Launcher.
origine-epic-manifest = dati di installazione di Epic Games Launcher (Manifests)
# Fonte: Apps\ProductInfo.xml di CapCut.
origine-capcut-productinfo = dati di versione di CapCut (ProductInfo.xml)
# Fonte: impostazioni di CapCut (User Data\Config\globalSetting).
origine-capcut-config = impostazioni di CapCut
# Fonte: conteggio di Windows (SHQueryRecycleBinW).
origine-cestino-api = conteggio del Cestino di Windows
# Fonte: API di alimentazione; gli stessi valori sono in Opzioni risparmio energia.
origine-alimentazione = Opzioni risparmio energia di Windows
# Fonte: chiavi di Sensore memoria.
origine-sensore-memoria = impostazioni di Sensore memoria
# Fonte: chiave BitBucket del volume, la stessa delle Proprietà del Cestino.
origine-bitbucket = Proprietà del Cestino
# Fonte: .npmrc, variabile d'ambiente o percorso predefinito.
origine-npm-config = configurazione di npm
# Fonte: cartelle note di Windows (SHGetKnownFolderPath).
origine-cartella-nota = percorso indicato da Windows
# Fonte: la regola stessa.
origine-regola = regola
# Fonte: impronta SHA-256 calcolata su richiesta.
origine-hash-contenuto = confronto del contenuto (SHA-256)
# Fonte: indice di un archivio zip.
origine-indice-zip = elenco dei file dell'archivio zip
# Fonte: risorsa di versione di un eseguibile, la stessa della scheda Dettagli in Proprietà.
origine-risorsa-versione = dati di versione del file (Proprietà › Dettagli)
# Fonte: tabella Property di un MSI.
origine-proprieta-msi = dati dell'installatore MSI (tabella Property)
# Fonte: il controllo dei file in uso di Windows. Mai "Restart Manager" (glossario).
origine-restart-manager = file in uso secondo Windows
# Fonte: UserAssist (avvii da Esplora file e Start).
origine-userassist = avvii registrati da Windows (UserAssist)
# Colonna "Gestito da": nome breve del gestore.
gestore-app-breve = Programmi
# Nome breve.
gestore-tuoi-breve = Tuoi file
# Nome breve.
gestore-windows-breve = Windows
# Nome breve.
gestore-non-classificato-breve = Non classificato

## Vista Download. Nomi delle famiglie in `lente-famiglia-<codice>`.

# Titolo della schermata Download.
download-titolo = Download
# Metodo dichiarato nella testata.
download-metodo = Quado riconosce il tipo dall'estensione e legge la versione degli installatori senza eseguirli.
# Riga sotto il titolo: percorso della cartella, dimensioni su disco, numero di file ($file è già scritto, per esempio "2.420 file").
download-meta = { $percorso } · { $dimensione } · { $file }
# Cartelle del sottoalbero non lette (accesso negato, scansione interrotta): i totali sono un minimo.
download-meta-minimo = { $n ->
    [one] Una cartella non è stata letta: i totali non comprendono il suo contenuto.
   *[other] { $testo } cartelle non sono state lette: i totali non comprendono il loro contenuto.
}
# Etichetta del menu con la cartella della vista Download.
download-cartella = Cartella
# Cartelle dell'utente nel menu (cartelle note di Windows). $codice è il codice della cartella.
download-cartella-nota = { $codice ->
    [documenti] Documenti
    [download] Download
    [desktop] Desktop
    [immagini] Immagini
    [video] Video
    [musica] Musica
   *[other] Cartella
}
# Vista Download in costruzione: dal modello della scansione, nessun file aperto.
download-costruzione = Raggruppamento dei file in corso… Quado usa i dati della scansione, senza aprire i file.
# La cartella scelta non è nella scansione. $percorso è il percorso.
download-non-trovata = La scansione non comprende la cartella { $percorso }.
# Windows non ha dato il percorso della cartella Download.
download-non-indicata = Windows non ha indicato la cartella Download di questo account.
# Come scegliere un'altra cartella.
download-scegli = Scegli un'altra cartella dal menu «Cartella», oppure da Esplora con «Apri nella vista Download».

# Riquadro della famiglia: installatori (exe, msi, msix e simili), secondo l'estensione.
lente-famiglia-installatori = Installatori
# Riquadro della famiglia: archivi (zip, rar, 7z e simili).
lente-famiglia-archivi = Archivi
# Riquadro della famiglia: immagini disco (iso, vhdx e simili).
lente-famiglia-immagini-disco = Immagini disco
# Riquadro della famiglia: video e audio.
lente-famiglia-video-audio = Video e audio
# Riquadro della famiglia: documenti e immagini.
lente-famiglia-documenti-immagini = Documenti e immagini
# Riquadro della famiglia: download incompleti (.crdownload, .part). Compare solo se ce ne sono.
lente-famiglia-download-incompleti = Download incompleti
# Tutti gli altri file: non hanno un riquadro, compaiono nella riga sotto i riquadri.
lente-famiglia-altro = Altri file
# Nome accessibile di un riquadro: famiglia, dimensione, numero di file già scritto.
download-riquadro = { $famiglia }: { $dimensione }, { $file }
# Riga sotto i riquadri: i file di altri tipi, così la somma dei riquadri torna con la testata.
download-altri-file = Altri file: { $dimensione } in { $file }

# Filtro: file con il suffisso " (N)" nel nome. È solo un filtro.
download-filtro-suffisso = Con «(N)» nel nome: { $n ->
    [one] un file
   *[other] { $testo } file
}
# Titolo del dettaglio del filtro.
download-filtro-titolo = File con «(N)» nel nome
# Nota del filtro: non raggruppa, non è una prova, non cambia lo stato.
download-filtro-nota = Il filtro mostra i file con «(N)» nel nome e lascia invariati voci e stati. Il suffisso non prova che due file siano copie: lo dice solo il confronto del contenuto.
# Nessun file con il suffisso.
download-filtro-vuoto = Nessun file con «(N)» nel nome in questa cartella.

# Comando: impronta SHA-256 dei file della stessa dimensione. Prima chiede conferma.
download-comando-confronta = Confronta il contenuto
# Comando: risorsa di versione degli exe e tabella Property degli MSI. Prima chiede conferma.
download-comando-installatori = Leggi i dati di versione
# Comando: indice degli zip con una cartella dello stesso nome accanto. Prima chiede conferma.
download-comando-archivi = Leggi l'indice degli archivi
# Comando: interroga di nuovo il controllo dei file aperti sui download incompleti.
download-comando-uso = Controlla di nuovo l'uso
# Suggerimento del comando: cosa fa e cosa non fa.
download-comando-uso-suggerimento = Chiede a Windows quali programmi hanno aperto questi file, senza leggerne il contenuto.
# Motivo di un comando disattivato: confronto già fatto per tutti i gruppi.
download-motivo-gia-confrontati = Tutti i file con le stesse dimensioni sono già stati confrontati.
# Motivo di un comando disattivato: i file della stessa dimensione sono segnaposto cloud.
download-motivo-cloud = I file con le stesse dimensioni sono solo nel cloud: Quado non li legge, per non scaricarli.
# Motivo di un comando disattivato: nessun gruppo da confrontare.
download-motivo-nessun-gruppo = Non ci sono file con le stesse dimensioni da confrontare.
# Motivo di un comando disattivato: nessun installatore exe o msi.
download-motivo-nessun-installatore = Nessun installatore exe o msi da leggere in questa cartella.
# Motivo di un comando disattivato: nessuno zip con la cartella accanto.
download-motivo-nessun-archivio = Nessun archivio zip con una cartella dello stesso nome accanto.
# Motivo di un comando disattivato: un'altra lettura in corso o in attesa di conferma.
download-motivo-lettura-in-corso = Quado fa una lettura alla volta: questo comando si attiva quando l'altra termina o la fermi.

# Prima del confronto: quanti file, quanti byte, e l'ultimo accesso. $byte è una dimensione.
download-confronta-domanda = { $n ->
    [one] Il confronto legge per intero il file ({ $byte }) e ne calcola l'impronta, sul tuo PC. Aggiorna la data di ultimo accesso del file.
   *[other] Il confronto legge per intero i { $testo } file ({ $byte }) e ne calcola l'impronta, sul tuo PC. Aggiorna la data di ultimo accesso di questi file.
}
# Segnaposto cloud esclusi dal confronto.
download-confronta-cloud = { $n ->
    [one] Quado salta un file che è solo nel cloud, per non scaricarlo.
   *[other] Quado salta { $testo } file che sono solo nel cloud, per non scaricarli.
}
# Prima della lettura degli installatori: quanti file, la loro dimensione, e l'ultimo accesso.
download-installatori-domanda = { $n ->
    [one] La lettura apre l'installatore ({ $byte }) solo nella parte con nome e versione, senza eseguirlo. Aggiorna la data di ultimo accesso del file.
   *[other] La lettura apre i { $testo } installatori ({ $byte } in tutto) solo nella parte con nome e versione, senza eseguirli. Aggiorna la data di ultimo accesso di questi file.
}
# Prima della lettura degli indici degli zip: quanti archivi, la loro dimensione, e l'ultimo accesso.
download-archivi-domanda = { $n ->
    [one] La lettura apre l'archivio ({ $byte }) solo nella parte finale, dove c'è l'indice dei file, senza decomprimerlo. Aggiorna la data di ultimo accesso del file.
   *[other] La lettura apre i { $testo } archivi ({ $byte } in tutto) solo nella parte finale, dove c'è l'indice dei file, senza decomprimerli. Aggiorna la data di ultimo accesso di questi file.
}
# Pulsante di conferma del confronto.
download-conferma-confronta = Confronta
# Pulsante di conferma delle altre letture.
download-conferma-leggi = Leggi
# Pulsante che rinuncia alla lettura chiesta.
download-annulla = Annulla
# Pulsante che ferma la lettura in corso: i file già letti restano letti, con la data di ultimo accesso aggiornata.
download-annulla-lettura = Interrompi la lettura

# Avanzamento del confronto. $file e $totale già scritti, $letti e $byte dimensioni.
download-avanzamento-confronto = Confronto in corso… { $file } file letti su { $totale } ({ $letti } su { $byte })
# Avanzamento della lettura dei dati di versione.
download-avanzamento-installatori = Lettura dei dati di versione in corso… { $file } file su { $totale }
# Avanzamento della lettura degli indici.
download-avanzamento-archivi = Lettura degli indici in corso… { $file } archivi su { $totale }
# Prima della conferma degli installatori si controllano solo gli attributi (dimensione, data, ID), senza leggere il contenuto.
download-avanzamento-attributi = Controllo degli installatori in corso… { $file } su { $totale }. Quado guarda solo dimensioni e data, senza leggere il contenuto.

# Fine del confronto. $letti e $totale già scritti.
download-esito-confronto = Confronto completato: { $letti } file letti su { $totale }.
# Confronto annullato a metà: nessun gruppo parziale vale come completo.
download-esito-confronto-interrotto = Confronto interrotto: { $letti } file letti su { $totale }. I gruppi letti solo in parte restano con il contenuto non confrontato.
# Fine della lettura dei dati di versione.
download-esito-installatori = Dati di versione letti: { $letti } file su { $totale }.
# Lettura dei dati di versione annullata.
download-esito-installatori-interrotto = Lettura dei dati di versione interrotta: { $letti } file letti su { $totale }. Gli altri restano da leggere.
# Seconda richiesta sugli stessi file: nessuna lettura, dati dalla cache della sessione.
download-esito-installatori-cache = I dati di versione vengono dalla lettura già fatta in questa sessione: da allora i file non sono cambiati.
# Fine della lettura degli indici.
download-esito-archivi = Indici letti: { $letti } su { $totale }.
# Lettura degli indici annullata.
download-esito-archivi-interrotto = Lettura degli indici interrotta: { $letti } indici letti su { $totale }.
# File che non si sono potuti leggere, con il motivo nel dettaglio.
download-esito-non-letti = { $n ->
    [one] Un file non è stato letto: il motivo è nel dettaglio della sua voce.
   *[other] { $testo } file non sono stati letti: i motivi sono nel dettaglio delle voci.
}

# Titolo dell'elenco delle voci.
download-elenco-titolo = Cosa riconosce Quado in questa cartella
# Regola in testata all'elenco.
download-elenco-regola = Nome, data o dimensioni da soli non bastano per proporre un'azione.
# Elenco vuoto.
download-elenco-vuoto = Quado non riconosce il ruolo di nessun file in questa cartella.
# Nome accessibile dell'elenco.
download-elenco-accessibile = Voci della cartella
# Legenda degli stati: Lo gestisce X, senza un nome.
download-legenda-gestisce = Lo gestisce un programma o Windows

# Titolo di riga: file della stessa dimensione non confrontati o confrontati solo in parte. $n numero di file (qui non usato).
download-voce-stessa-dimensione = File con le stesse dimensioni, contenuto non confrontato
# Titolo di riga: copie con lo stesso contenuto dopo il confronto.
download-voce-copie = Copie identiche
# Titolo di riga: stessa dimensione, contenuto diverso.
download-voce-contenuto-diverso = Stesse dimensioni, contenuto diverso
# Titolo di riga: installatori non ancora letti. $n numero di file.
download-voce-installatori-non-letti = { $n ->
    [one] Installatore con i dati di versione non letti
   *[other] Installatori con i dati di versione non letti
}
# Titolo di riga: installatori solo nel cloud.
download-voce-installatori-cloud = { $n ->
    [one] Installatore solo nel cloud
   *[other] Installatori solo nel cloud
}
# Titolo di riga: lettura non riuscita.
download-voce-installatori-non-riusciti = { $n ->
    [one] Installatore con i dati di versione non leggibili
   *[other] Installatori con i dati di versione non leggibili
}
# Titolo di riga: programma installato alla stessa versione.
download-voce-installatori-stessa-versione = { $n ->
    [one] Installatore di un programma già installato alla stessa versione
   *[other] Installatori di programmi già installati alla stessa versione
}
# Titolo di riga: programma installato in un'altra versione.
download-voce-installatori-altra-versione = { $n ->
    [one] Installatore di un programma installato in un'altra versione
   *[other] Installatori di programmi installati in un'altra versione
}
# Titolo di riga: nessun programma installato corrispondente.
download-voce-installatori-non-installati = { $n ->
    [one] Installatore di un programma non installato
   *[other] Installatori di programmi non installati
}
# Titolo di riga: letti, ma senza confronto (né nome né codice, o voci di disinstallazione non lette).
download-voce-installatori-senza-confronto = { $n ->
    [one] Installatore senza confronto con i programmi installati
   *[other] Installatori senza confronto con i programmi installati
}
# Titolo di riga: zip con una cartella dello stesso nome accanto.
download-voce-archivio = Archivio con una cartella dello stesso nome accanto
# Titolo di riga: immagini disco.
download-voce-immagini-disco = { $n ->
    [one] Immagine disco
   *[other] Immagini disco
}
# Titolo di riga: download incompleti che il controllo dei file aperti non trova in uso.
download-voce-incompleti-liberi = { $n ->
    [one] Download incompleto non in uso
   *[other] Download incompleti non in uso
}
# Titolo di riga: download incompleti in uso, con il nome del processo nei fatti.
download-voce-incompleti-in-uso = { $n ->
    [one] Download incompleto in uso
   *[other] Download incompleti in uso
}
# Titolo di riga: il controllo dei file aperti non ha risposto.
download-voce-incompleti-uso-non-noto = { $n ->
    [one] Download incompleto, uso non noto
   *[other] Download incompleti, uso non noto
}
# Titolo di riga: uso non ancora controllato.
download-voce-incompleti-non-controllati = { $n ->
    [one] Download incompleto, uso non controllato
   *[other] Download incompleti, uso non controllato
}
# Nome del primo file e quanti altri. $primo è un nome di file, $altri il numero.
download-nomi = { $primo } e { $altri ->
    [one] un altro file
   *[other] altri { $altri_testo } file
}
# Riga che raccoglie più gruppi di file. $file già scritto ("25 file").
download-riga-gruppi = { $n ->
    [one] Un gruppo
   *[other] { $testo } gruppi
}, { $file }

# Dettaglio senza selezione.
download-dettaglio-vuoto = Seleziona una voce per vedere i fatti, i file e cosa puoi fare.
# Sotto il titolo del dettaglio: numero di file già scritto e dimensioni su disco.
download-dettaglio-sottotitolo = { $file } · { $dimensione }
# Voci non mostrate nel dettaglio.
download-dettaglio-altre-voci = { $n ->
    [one] e un'altra voce
   *[other] e altre { $testo } voci
}
# Pulsante in fondo all'elenco: mostra altre voci, più piccole. $n quante restano.
download-altre-voci = { $n ->
    [one] Mostra un'altra voce
   *[other] Mostra altre voci ({ $testo } restanti)
}
# File non elencati nel dettaglio.
download-dettaglio-altri-file = { $n ->
    [one] e un altro file
   *[other] e altri { $testo } file
}
# File della stessa dimensione non letto durante il confronto. $nome del file, $motivo uno dei testi sotto, senza altri
# due punti.
download-non-letto = { $nome } non letto: { $motivo }
# Motivo: il file non c'è più.
download-non-letto-assente = il file non c'è più
# Motivo: accesso negato.
download-non-letto-accesso = Windows nega l'accesso
# Motivo: un altro programma tiene il file aperto in modo esclusivo.
download-non-letto-in-uso = è in uso da un altro programma, che lo tiene aperto in modo esclusivo
# Motivo: errore di Windows con il codice.
download-non-letto-errore = errore di Windows (Codice { $codice })
# Motivo: segnaposto cloud.
download-non-letto-cloud = è solo nel cloud e Quado non lo legge, per non scaricarlo
# Motivo: dimensione, data o identità cambiate durante la lettura.
download-non-letto-cambiato = è cambiato durante la lettura, quindi l'impronta non vale
# Motivo: collegamento o cartella.
download-non-letto-non-file = non è un file (collegamento o cartella)
# Motivo: lettura interrotta con «Interrompi la lettura».
download-non-letto-annullato = lettura interrotta
# Origine delle voci: funzioni di Quado, non regole con una fonte (Origin::Function).
download-origine = Quado ha costruito questa voce dalle misure della scansione e dalle letture chieste in questa sessione, senza usare una regola.
# Installatori che nessun lettore legge.
download-installatori-altri-tipi = Quado non legge i dati di versione dei pacchetti MSIX e APPX e delle patch MSP: restano senza confronto.
# Prima del confronto le caselle sono disattivate.
download-caselle-dopo = Dopo «Confronta il contenuto», se i file risultano identici, scegli quale copia tenere.
# Dopo il confronto: quale copia resta, e come si spostano le altre (solo dall'anteprima, mai da qui direttamente).
download-tieni-nota = Quado tiene la copia creata per prima, se non ne scegli un'altra. Le altre vanno nel Cestino solo con «Sposta nel Cestino…», dopo l'anteprima e la tua conferma.
# Data di creazione di una copia, letta durante il confronto.
download-file-creato = Creato il { $data }
# Data di ultima modifica, dalla scansione.
download-file-modificato = Modificato il { $data }
# La copia che resta.
download-tenuta = resta
# Le altre copie del gruppo.
download-da-spostare = da spostare nel Cestino, dopo l'anteprima
# Nome accessibile della casella disattivata. $nome del file.
download-casella = Scegli { $nome }
# Nome accessibile della scelta della copia da tenere. $nome del file.
download-tieni = Tieni { $nome }
# Dettaglio di una famiglia: prima dei file più grandi.
download-famiglia-nota = Tipo riconosciuto dall'estensione. I file più grandi:
# Comando nell'ispettore di Esplora su una cartella del profilo dell'utente: apre la schermata Download su quella cartella.
download-apri-lente = Apri nella vista Download
# Suggerimento del comando.
download-apri-lente-suggerimento = Mostra tipi di file e voci di questa cartella dai dati della scansione, senza aprire i file.
# Sottotitolo della card interventi quando contiene anche voci della vista Download (copie confrontate, download incompleti).
# Stessa forma di interventi-sottotitolo.
interventi-sottotitolo-lente = Ogni voce viene da una regola, con la fonte e la data di verifica, o da prove misurate nella vista Download. Seleziona una voce per vedere nell'ispettore cosa puoi fare. Da questa card Quado non cambia niente sul disco.

## Programmi.

# Titolo della schermata Programmi.
programmi-titolo = Programmi
# Sottotitolo: da dove viene l'elenco e come si conta lo spazio.
programmi-sottotitolo = Programmi registrati in Windows, app di Microsoft Store, giochi di Steam e prodotti di Epic. Quado conta lo spazio di un programma solo quando una fonte ne dichiara la cartella.
# Testata, primo riquadro: quanti programmi sono elencati.
programmi-tessera-programmi = Programmi elencati
# Nota sotto il numero, con i componenti di sistema nascosti.
programmi-tessera-senza-componenti = Senza i componenti di sistema
# Nota sotto il numero, con i componenti di sistema mostrati.
programmi-tessera-con-componenti = Con i componenti di sistema
# Testata, secondo riquadro: spazio misurato dei programmi elencati.
programmi-tessera-spazio = Spazio misurato
# Nota sullo spazio: una cartella condivisa conta una volta, il dichiarato non si somma.
programmi-tessera-spazio-nota = Dimensioni su disco. Una cartella dichiarata da più programmi conta una volta. Le dimensioni dichiarate restano fuori dal totale.
# Testata, terzo riquadro: programmi senza prova del percorso.
programmi-tessera-non-dichiarata = Posizione non dichiarata
# Nota: niente spazio per somiglianza del nome.
programmi-tessera-non-dichiarata-nota = Quado attribuisce spazio solo con una prova del percorso, mai per somiglianza del nome.
# Start_TrackProgs = 0: testo esatto, senza punto finale.
programmi-tracciamento-spento = Tracciamento degli avvii di Windows disattivato: nessun dato d'uso
# Start_TrackProgs = 1: copertura di UserAssist.
programmi-tracciamento-attivo = Tracciamento degli avvii di Windows attivo: l'ultimo avvio viene da UserAssist. UserAssist conta gli avvii da Esplora file e da Start. Restano fuori quelli da riga di comando, da altri launcher e da attività pianificate.
# Start_TrackProgs assente, inatteso o non leggibile.
programmi-tracciamento-non-letto = Impostazione del tracciamento degli avvii non letta: nessun dato d'uso da UserAssist.
# Copertura delle altre fonti d'uso. Mai "inutilizzato" o "mai usato".
programmi-uso-copertura = Steam registra l'ultima sessione dei suoi giochi; Epic e Microsoft Store non registrano l'uso.
# Fonte dei programmi non letta. $fonte nome della fonte, $motivo dal codice di errore.
programmi-fonte-errore = { $fonte }: { $motivo }
# Elemento di una fonte saltato. $percorso chiave di registro o file; il motivo in fondo, così il codice di Windows chiude la riga.
programmi-fonte-errore-elemento = { $fonte }, { $percorso }: { $motivo }
# Scelta della sezione: l'elenco dei programmi. $testo numero già scritto.
programmi-sezione-programmi = Programmi ({ $testo })
# Scelta della sezione: possibili residui. "Residuo" solo come possibilità.
programmi-sezione-residui = Possibili residui ({ $testo })
# Interruttore dei componenti di sistema, spento per default. $testo numero già scritto.
programmi-mostra-componenti = Mostra componenti di sistema ({ $testo })
# Suggerimento nella casella di ricerca dei programmi.
programmi-cerca = Cerca un programma per nome
# Intestazione di colonna: da dove viene il programma.
programmi-colonna-fonte = Fonte
# Intestazione di colonna.
programmi-colonna-versione = Versione
# Intestazione di colonna: dimensione dichiarata dalla fonte, mai sommata.
programmi-colonna-dichiarata = Dimensioni dichiarate
# Intestazione di colonna: ultimo uso noto.
programmi-colonna-uso = Ultimo uso noto
# Intestazione di colonna: InstallDate.
programmi-colonna-installazione = Installato il
# Intestazione di colonna dei possibili residui: Local, Roaming, LocalLow, ProgramData.
programmi-colonna-base = Cartella di base
# Intestazione di colonna dei possibili residui.
programmi-colonna-stato = Stato
# Fonte nella tabella: voce di disinstallazione in HKLM.
programmi-fonte-hklm = Registro, tutti gli account
# Fonte nella tabella: voce in HKLM\WOW6432Node.
programmi-fonte-wow = Registro, 32 bit
# Fonte nella tabella: voce in HKCU.
programmi-fonte-hkcu = Registro, il tuo account
# Fonte nella tabella: pacchetto MSIX.
programmi-fonte-msix = Microsoft Store
# Fonte nella tabella: manifest di Steam.
programmi-fonte-steam = Steam
# Fonte nella tabella: manifest di Epic.
programmi-fonte-epic = Epic
# Spazio misurato senza prova del percorso: mai 0, mai una cifra.
programmi-cella-non-dichiarata = posizione non dichiarata
# Cartella dichiarata ma non nella scansione o non leggibile.
programmi-cella-non-misurato = non misurato
# Cartella dichiarata che non può essere quella di un programma.
programmi-cella-non-plausibile = nessuno spazio attribuito
# Dimensione dichiarata con la sua fonte, accanto al misurato. $dimensione già scritta, $fonte breve.
programmi-cella-dichiarata = { $dimensione } ({ $fonte })
# Fonte breve del dichiarato: EstimatedSize della voce di disinstallazione.
programmi-dichiarata-registro = registro
# Fonte breve del dichiarato: SizeOnDisk del manifest di Steam.
programmi-dichiarata-steam = Steam
# Ultimo uso da Steam (LastPlayed). $data già scritta.
programmi-uso-steam = { $data } (Steam)
# Steam senza LastPlayed. Mai "mai usato".
programmi-uso-steam-nessuna = nessuna sessione registrata
# Ultimo avvio da UserAssist, solo con il tracciamento attivo. $data già scritta.
programmi-uso-userassist = { $data } (UserAssist)
# Tracciamento attivo, nessuna voce per la cartella. Mai "mai usato".
programmi-uso-userassist-nessuno = nessun avvio in UserAssist
# Nessun dato d'uso nella tabella: tracciamento spento, UserAssist non letto, Epic e Microsoft Store. Il
# perché sta nella testata e nel dettaglio.
programmi-uso-nessun-dato = nessun dato d'uso
# Accanto al nome di un componente di sistema mostrato.
programmi-tag-componente = componente di sistema
# Accanto al nome di un programma la cui cartella è dichiarata anche da altri programmi.
programmi-tag-condivisa = cartella in comune
# Nome della tabella per le tecnologie assistive.
programmi-accessibile-tabella = Elenco dei programmi
# Nome dell'elenco dei possibili residui per le tecnologie assistive.
programmi-accessibile-residui = Elenco dei possibili residui
# Stato vuoto: la scansione non ha letto nessuna fonte dei programmi (scansione senza prove).
programmi-vuoto-senza-fonti = Questa scansione non ha letto le fonti dei programmi: l'elenco è vuoto.
# Stato vuoto: fonti lette, nessun programma.
programmi-vuoto-nessuno = Le fonti lette non elencano programmi.
# Stato vuoto della ricerca.
programmi-vuoto-ricerca = Nessun programma ha questo testo nel nome. Svuota la ricerca per tornare all'elenco.
# Stato vuoto dei possibili residui.
programmi-residui-nessuno = Nessun possibile residuo in Local, Roaming, LocalLow e ProgramData.
# Dettaglio senza selezione.
programmi-nessuna-selezione = Seleziona un programma per vederne fonte, spazio e uso noto.
# Dettaglio dei residui senza selezione.
programmi-residuo-nessuna-selezione = Seleziona una cartella per vederne le misure.
# Testata della sezione dei possibili residui: il criterio e cosa si può fare.
programmi-residui-regola = Cartelle in Local, Roaming, LocalLow e ProgramData che Quado non associa a un programma installato o a una regola. Il confronto è solo per nome e può sbagliare, quindi Quado non propone azioni. Per decidere, guarda la cartella con «Apri in Esplora file».
# Base di un possibile residuo. $base nome della cartella nota.
programmi-residuo-base = Cartella di base: { $base }
# Tipo nel dettaglio: voce di disinstallazione in HKLM.
programmi-tipo-hklm = Programma registrato in Windows per tutti gli account (HKLM)
# Tipo nel dettaglio: voce in WOW6432Node.
programmi-tipo-wow = Programma a 32 bit registrato in Windows (HKLM, WOW6432Node)
# Tipo nel dettaglio: voce in HKCU, che l'utente può scrivere.
programmi-tipo-hkcu = Programma registrato in Windows per il tuo account (HKCU)
# Tipo nel dettaglio: pacchetto MSIX.
programmi-tipo-msix = App di Microsoft Store
# Tipo nel dettaglio: gioco di Steam.
programmi-tipo-steam = Gioco di Steam
# Tipo nel dettaglio: prodotto di Epic.
programmi-tipo-epic = Prodotto di Epic Games Launcher
# Sotto la cifra grande del dettaglio.
programmi-misurato-nota = Dimensioni su disco, misurate in questa scansione
# Al posto della cifra: nessuna prova del percorso.
programmi-titolo-non-dichiarata = Posizione non dichiarata
# Al posto della cifra: cartella non nella scansione o non leggibile.
programmi-titolo-non-misurato = Non misurato
# Al posto della cifra: cartella dichiarata non plausibile.
programmi-titolo-non-plausibile = Nessuno spazio attribuito
# Sezione del dettaglio: cartella e prova del percorso.
programmi-sezione-posizione = Posizione
# Sezione del dettaglio: misurato, dichiarato, dati dalle regole.
programmi-sezione-spazio = Spazio
# Sezione del dettaglio: uso noto con la copertura.
programmi-sezione-uso = Uso noto
# Sezione del dettaglio: elementi riconosciuti dalle regole come dati del programma.
programmi-sezione-dati = Dati riconosciuti dalle regole
# Sezione del dettaglio: la funzione di Quado che produce il record.
programmi-sezione-funzione = Funzione
# Prova del percorso: InstallLocation.
programmi-prova-installlocation = Prova del percorso: InstallLocation della voce di disinstallazione.
# Indizio del percorso: cartella dell'eseguibile nominato dal comando di disinstallazione o dall'icona, comando mai eseguito.
programmi-prova-indizio = Indizio del percorso: la cartella dell'eseguibile nominato dal comando di disinstallazione o dall'icona. Quado legge quel comando e non lo esegue mai.
# Prova del percorso: InstalledLocation del pacchetto MSIX.
programmi-prova-msix = Prova del percorso: InstalledLocation del pacchetto.
# Prova del percorso: manifest di Steam.
programmi-prova-steam = Prova del percorso: libraryfolders.vdf e il manifest del gioco.
# Prova del percorso: manifest di Epic.
programmi-prova-epic = Prova del percorso: InstallLocation del manifest di Epic.
# Provenienza della cartella. $classe misurato, da fonte, dedotto.
programmi-provenienza = Provenienza della cartella: { $classe }
# Altri programmi con la stessa cartella. $nomi elenco.
programmi-condivisa-con = Stessa cartella di: { $nomi }
# Dati di una regola dentro la cartella del programma (già nel misurato). $n elementi.
programmi-dati-dentro = { $n ->
    [one] Un elemento nella cartella del programma
   *[other] { $n_testo } elementi nella cartella del programma
}
# Dati di una regola fuori dalla cartella, che la regola dichiara di questo programma.
programmi-dati-fuori = { $n ->
    [one] Un elemento fuori dalla cartella, gestito da questo programma
   *[other] { $n_testo } elementi fuori dalla cartella, gestiti da questo programma
}
# Nota sul dichiarato.
programmi-dichiarato-nota = Le dimensioni dichiarate vengono dal programma di installazione o da Steam e possono essere vecchie. Quado le mostra accanto a quelle misurate e le tiene fuori dai totali.
# Copertura di Steam come fonte d'uso.
programmi-copertura-steam = Steam registra le sessioni dei giochi avviati da Steam; gli avvii fuori da Steam potrebbero mancare.
# Pulsante che apre il dialogo di disinstallazione. Non disinstalla nulla da solo.
programmi-disinstalla = Disinstalla…
# Pulsante: porta l'Esplora di Quado sulla cartella.
programmi-mostra-in-esplora = Mostra in Esplora
# Titolo del dialogo per Win32 e Microsoft Store, prima dell'apertura: una domanda con lo stesso verbo del pulsante
# programmi-dialogo-apri-impostazioni (docs/testi.md §4). Quado non disinstalla: apre l'interfaccia ufficiale.
programmi-dialogo-titolo = Aprire Impostazioni per disinstallare { $programma }?
# Titolo del dialogo per Steam, prima dell'apertura: risponde al pulsante programmi-dialogo-apri-steam.
programmi-dialogo-titolo-steam = Aprire Steam per disinstallare { $programma }?
# Titolo del dialogo quando Quado non apre niente: Epic, e ogni programma dopo l'apertura o dopo un errore. Nome del
# compito, senza domanda: il pulsante è «Chiudi», con «Ho eseguito la procedura» dopo l'apertura.
programmi-dialogo-titolo-guida = Come disinstallare { $programma }
# Sotto il titolo del dialogo: cosa fa Quado e cosa no.
programmi-dialogo-sottotitolo = La disinstallazione avviene nell'interfaccia ufficiale, dove confermi tu. Quado non esegue mai il comando di disinstallazione scritto nel registro.
# Programmi Win32: cosa succede con il pulsante.
programmi-dialogo-impostazioni = Quado copia il nome «{ $programma }» negli appunti e apre Impostazioni › App, dove lo incolli nella casella di ricerca.
# App di Microsoft Store.
programmi-dialogo-msix = Quado apre la pagina di { $programma } in Impostazioni › App.
# Giochi di Steam.
programmi-dialogo-steam = Quado apre Steam, che chiede conferma prima di disinstallare { $programma }.
# Epic: nessun URI, solo il rinvio alla libreria del launcher.
programmi-dialogo-epic = Per disinstallare { $programma }, apri la libreria del launcher Epic, scegli i tre puntini accanto al nome e poi Disinstalla. Quado non può aprire quella pagina.
# Piede del dialogo.
programmi-dialogo-esc = Esc annulla.
# Pulsante del dialogo.
programmi-dialogo-annulla = Annulla
# Pulsante principale per Win32 e Microsoft Store.
programmi-dialogo-apri-impostazioni = Apri Impostazioni
# Pulsante principale per Steam.
programmi-dialogo-apri-steam = Apri Steam
# Pulsante che chiude il dialogo dopo l'apertura.
programmi-dialogo-chiudi = Chiudi
# Dopo l'apertura per Win32: il nome del programma è negli appunti.
programmi-dialogo-aperto-impostazioni = Si apre Impostazioni › App. Il nome «{ $programma }» è negli appunti: incollalo nella casella di ricerca.
# Dopo l'apertura per Microsoft Store.
programmi-dialogo-aperto-msix = Si apre Impostazioni › App, nella pagina di { $programma }.
# Dopo l'apertura per Steam.
programmi-dialogo-aperto-steam = Si apre Steam: conferma lì la disinstallazione di { $programma }.
# La shell non ha aperto la pagina. $codice esadecimale.
programmi-dialogo-errore = Impossibile aprire la pagina. Riprova, oppure aprila a mano. (Codice { $codice })
# PFN o appid non accettati dal lanciatore: non si apre nulla.
programmi-dialogo-non-valido = Impossibile aprire la pagina: il nome del pacchetto o il numero del gioco non è valido. Aprila a mano da Impostazioni › App o da Steam.

## Interventi, parti comuni.

# Voce della barra laterale: vista Interventi, con il risultato dell'ultimo spostamento e lo storico.
nav-interventi = Interventi
# Pulsante sotto il record: avvia l'anteprima (simulazione) e apre la conferma dello spostamento. I puntini dicono che prima dell'azione c'è un dialogo.
azione-prepara-cestino = Sposta nel Cestino…
# Pulsante sotto il record: misura lo spazio libero e apre la pagina della procedura (Impostazioni, Cestino).
azione-apri-procedura = Apri la procedura
# Pulsante sotto il record: dopo una procedura o un comando, avvia le nuove misure.
azione-ho-eseguito = Ho eseguito la procedura

## Spostamento nel Cestino, conferma e risultato.

# Titolo del dialogo di conferma: domanda con l'azione e la quantità (docs/testi.md §4). Il pulsante principale usa lo stesso verbo. $n elementi scelti, $n_testo già scritto, $nome regola o funzione.
spostamento-titolo = { $n ->
    [one] Spostare un elemento di «{ $nome }» nel Cestino?
   *[other] Spostare { $n_testo } elementi di «{ $nome }» nel Cestino?
}
# Sotto il titolo del dialogo, anche durante la simulazione: niente è ancora successo e cosa fa partire lo spostamento. Cita spostamento-sposta.
spostamento-sottotitolo = Anteprima: Quado sposta gli elementi solo quando scegli «Sposta nel Cestino».
# Nome accessibile del pulsante con la sola icona X in alto a destra del dialogo: chiude senza spostare niente.
spostamento-chiudi = Chiudi senza spostare
# Nome quando la regola o la funzione non ha un nome tradotto: compare tra «» nel titolo e in testa al risultato.
spostamento-elementi-scelti = Elementi scelti
# Titolo del dialogo mentre la simulazione rilegge gli attributi. $nome regola o funzione.
spostamento-simulazione-titolo = Anteprima dello spostamento nel Cestino: { $nome }
# Durante la simulazione: cosa legge e cosa no.
spostamento-simulazione-testo = Quado rilegge solo gli attributi degli elementi, poi esegue i controlli. I file che stanno solo nel cloud non vengono scaricati.
# Avanzamento della simulazione. $n voci lette, $testo già scritto.
spostamento-simulazione-letti = { $n ->
    [one] Un file o una cartella letti
   *[other] { $testo } file e cartelle letti
}
# Riga sotto il titolo: le prove. $prove fatti separati da punto e virgola.
spostamento-riga-prove = Prove: { $prove }
# Riga sotto il titolo: la regola. $nome nome, $id identificativo, $data data di verifica.
spostamento-riga-regola = Regola: { $nome } ({ $id }), verificata il { $data }
# Riga sotto il titolo per una funzione di Quado (vista Download), che non ha una regola. $nome nome della funzione.
spostamento-riga-funzione = Funzione di Quado: { $nome }. Usa solo prove misurate o lette in questa sessione.
# Primo dei tre numeri: dimensioni su disco degli elementi scelti.
spostamento-numero-da-spostare = Da spostare
# Secondo numero: spazio liberato subito, sempre zero perché il Cestino trattiene i file.
spostamento-numero-subito = Liberato subito
# Terzo numero: stima dello spazio dopo lo svuotamento del Cestino, con il suo stato.
spostamento-numero-dopo = Dopo lo svuotamento del Cestino
# Stato della stima esatta, sotto il terzo numero e sotto "Stimato" nel risultato.
spostamento-stima-esatta = stima esatta
# Valore di una stima a intervallo. $minimo e $massimo dimensioni già scritte.
spostamento-stima-intervallo-valore = da { $minimo } a { $massimo }
# Stato di una stima a intervallo, con il motivo: la selezione contiene cartelle che si leggono solo come amministratore (estimate.rs).
spostamento-stima-intervallo = stima a intervallo: alcune cartelle si leggono solo come amministratore
# Valore del terzo numero quando la stima non ha un valore.
spostamento-stima-nessun-valore = non stimabile
# Sotto i tre numeri: perché il valore "Liberato subito" è zero.
spostamento-numeri-nota = Il Cestino trattiene i file: lo spazio si libera quando lo svuoti tu o Sensore memoria.
# File esclusi dalla stima perché hanno altri nomi. $n file, $n_testo già scritto, $byte dimensioni su disco già scritte.
spostamento-esclusi-nomi = { $n ->
    [one] Un file escluso dalla stima ({ $byte }): ha altri nomi (hard link) fuori dalla selezione, quindi il suo spazio non si libera.
   *[other] { $n_testo } file esclusi dalla stima ({ $byte }): hanno altri nomi (hard link) fuori dalla selezione, quindi il loro spazio non si libera.
}
# Cartelle non leggibili nella selezione: la stima diventa un intervallo. $n cartelle, $n_testo già scritto.
spostamento-cartelle-negate = { $n ->
    [one] Una cartella della selezione si legge solo come amministratore: la stima non comprende il suo contenuto.
   *[other] { $n_testo } cartelle della selezione si leggono solo come amministratore: la stima non comprende il loro contenuto.
}
# Titolo dell'elenco con le caselle.
spostamento-elenco = Elementi
# Accanto al titolo dell'elenco del dialogo di conferma: quanti elementi sono scelti tra quelli che si
# possono spostare. Segue le caselle.
spostamento-selezionati = { $n ->
    [one] { $testo } di { $totale } selezionato
   *[other] { $testo } di { $totale } selezionati
}
# Accanto a un elemento fisso e disattivato: la versione in uso di CapCut.
spostamento-fisso-versione-in-uso = esclusa: è la versione in uso
# Accanto a un elemento fisso e disattivato: la versione precedente di CapCut, tenuta per default.
spostamento-fisso-versione-tenuta = tenuta: è la versione precedente
# Accanto a un elemento fisso e disattivato: la copia che resta tra le copie identiche.
spostamento-fisso-copia = tenuta: è la copia da conservare
# Sotto un elemento bloccato: cartella di Windows.
spostamento-blocco-protetto-windows = Non si sposta: è una cartella di Windows.
# Sotto un elemento bloccato: WinSxS. Il nome tecnico tra parentesi serve a riconoscerla.
spostamento-blocco-protetto-winsxs = Non si sposta: è l'archivio dei componenti di Windows (WinSxS).
# Sotto un elemento bloccato: Windows\Installer, con il nome della regola.
spostamento-blocco-protetto-installer = Non si sposta: è la cache di Windows Installer.
# Sotto un elemento bloccato: System Volume Information.
spostamento-blocco-protetto-svi = Non si sposta: è System Volume Information, una cartella di sistema di Windows.
# Sotto un elemento bloccato: file di paging.
spostamento-blocco-protetto-paging = Non si sposta: è il file di paging di Windows.
# Sotto un elemento bloccato: file di ibernazione.
spostamento-blocco-protetto-ibernazione = Non si sposta: è il file di ibernazione di Windows.
# Sotto un elemento bloccato: file di swap.
spostamento-blocco-protetto-swap = Non si sposta: è il file di swap di Windows.
# Sotto un elemento bloccato: cartella di un programma registrato, senza l'eccezione della regola o con un suo controllo non superato. Niente invito a disinstallare: per una versione di CapCut in uso toglierebbe anche i progetti.
spostamento-blocco-protetto-programma = Non si sposta: sta nella cartella di un programma installato.
# Sotto un elemento bloccato: unità senza Cestino.
spostamento-blocco-senza-cestino = Questa unità non ha un Cestino: Windows eliminerebbe l'elemento per sempre.
# Sotto un elemento bloccato: NukeOnDelete = 1. La conseguenza: niente da ripristinare.
spostamento-blocco-elimina-subito = Il Cestino di questa unità è impostato per eliminare subito i file: l'elemento non si potrebbe ripristinare.
# Sotto un elemento bloccato: capacità del Cestino non registrata.
spostamento-blocco-capacita-ignota = Quado non sa quanto può contenere il Cestino di questa unità, quindi l'elemento resta al suo posto.
# Sotto un elemento bloccato: elemento più grande del Cestino. Misurato su R: il 14/10/2026: Windows chiede di eliminarlo per sempre (mover.rs). $dimensione dimensione logica, $capacita capacità, già scritte.
spostamento-blocco-piu-grande = Elemento da { $dimensione } in un Cestino da { $capacita }: Windows non lo sposterebbe nel Cestino e chiederebbe di eliminarlo per sempre.
# Sotto un elemento bloccato: programmi che usano i file dell'elemento. $processi elenco di nomi.
spostamento-blocco-in-uso = In uso da { $processi }
# Un programma nell'elenco: il nome dato da Windows e il numero che mostra Gestione attività. $nome nome del processo, $pid numero del processo.
spostamento-processo = { $nome } (PID { $pid })
# Sotto un elemento bloccato: il controllo dei file in uso non ha risposto. $codice errore di Windows, in fondo come in docs/testi.md §4.
spostamento-blocco-restart-manager = Windows non ha detto se i file sono in uso, quindi l'elemento resta al suo posto. (Codice { $codice })
# Sotto un elemento bloccato: collegamenti, voci di registro o attività che puntano all'elemento. $elenco riferimenti trovati.
spostamento-blocco-riferimenti = Puntano a questo elemento: { $elenco }
# Un riferimento nell'elenco, dopo «Puntano a questo elemento:». $dove chiave, collegamento o attività, $tipo dove è
# stato trovato (spostamento-rif-*), tra parentesi per non ripetere i due punti.
spostamento-riferimento = { $dove } ({ $tipo })
# Tipo di riferimento: UninstallString di una voce di disinstallazione.
spostamento-rif-uninstall-string = comando di disinstallazione
# Tipo di riferimento: DisplayIcon.
spostamento-rif-display-icon = icona di un programma installato
# Tipo di riferimento: InstallLocation.
spostamento-rif-install-location = cartella di installazione dichiarata da un programma
# Tipo di riferimento: App Paths. Il nome della chiave di registro resta tra parentesi per chi la cerca.
spostamento-rif-app-paths = percorso di un'app nel registro (App Paths)
# Tipo di riferimento: chiave Run di HKCU.
spostamento-rif-run-utente = avvio automatico del tuo account
# Tipo di riferimento: chiave Run di HKLM.
spostamento-rif-run-sistema = avvio automatico per tutti gli account
# Tipo di riferimento: collegamento .lnk di Start o del desktop, letto senza risolverlo.
spostamento-rif-collegamento = collegamento
# Tipo di riferimento: attività pianificata.
spostamento-rif-attivita = attività pianificata
# Sotto un elemento bloccato: percorso di 260 caratteri o più (comportamento di Windows provato su un VHDX).
spostamento-blocco-percorso-lungo = Percorso di 260 caratteri o più: Windows non lo sposterebbe nel Cestino e chiederebbe di eliminarlo per sempre.
# Sotto una cartella bloccata: un elemento al suo interno ha un percorso di 260 caratteri o più.
spostamento-blocco-contiene-lungo = Contiene un percorso di 260 caratteri o più: Windows non sposterebbe la cartella nel Cestino e chiederebbe di eliminarla per sempre.
# Sotto un elemento bloccato: attributi non letti. $codice errore di Windows.
spostamento-blocco-illeggibile = Impossibile leggere l'elemento, quindi resta al suo posto. (Codice { $codice })
# Sotto un elemento bloccato: l'elemento non esiste più.
spostamento-blocco-mancante = Non esiste più.
# Sotto un elemento bloccato: elemento su un'altra unità rispetto agli altri.
spostamento-blocco-altra-unita = Sta su un'altra unità rispetto agli altri elementi.
# Titolo della sezione dei controlli automatici.
spostamento-controlli = Controlli
# Controllo dei file in uso superato. $ora ora del controllo.
spostamento-rm-ok = Nessun file è in uso, secondo Windows (controllo delle { $ora }).
# Controllo dei file in uso con elementi in uso, con l'azione per spostarli. $n elementi in uso, $n_testo già scritto,
# $ora ora del controllo. Cita azione-prepara-cestino.
spostamento-rm-in-uso = { $n ->
    [one] Un elemento è in uso secondo Windows (controllo delle { $ora }) e resta al suo posto. Per spostarlo, chiudi il programma che lo usa e scegli di nuovo «Sposta nel Cestino…».
   *[other] { $n_testo } elementi sono in uso secondo Windows (controllo delle { $ora }) e restano al loro posto. Per spostarli, chiudi i programmi che li usano e scegli di nuovo «Sposta nel Cestino…».
}
# Il controllo dei file in uso non ha risposto per alcuni elementi. $n elementi, $n_testo già scritto.
spostamento-rm-errore = { $n ->
    [one] Per un elemento Windows non ha detto se i file sono in uso: resta al suo posto.
   *[other] Per { $n_testo } elementi Windows non ha detto se i file sono in uso: restano al loro posto.
}
# Nessun riferimento trovato.
spostamento-rif-ok = Nessun collegamento, voce di registro o attività pianificata punta a questi elementi.
# Riferimenti trovati. $n elementi con riferimenti, $n_testo già scritto.
spostamento-rif-trovati = { $n ->
    [one] Collegamenti, voci di registro o attività pianificate puntano a un elemento: resta al suo posto.
   *[other] Collegamenti, voci di registro o attività pianificate puntano a { $n_testo } elementi: restano al loro posto.
}
# Controllo dei riferimenti parziale. $fonti tipi di fonte non letti per intero.
spostamento-rif-parziale = Controllo parziale. Fonti non lette per intero: { $fonti }.
# Capacità del Cestino sufficiente. $volume lettera, $totale contenuto più selezione, $capacita capacità, già scritte.
spostamento-capacita-ok = Il Cestino di { $volume } può contenere questa selezione: arriverebbe a { $totale } su { $capacita }.
# Capacità superata: spostamento bloccato e cosa fa Windows oltre il limite. $volume lettera, $capacita e $totale già scritte.
spostamento-capacita-superata = Spostamento bloccato: il Cestino di { $volume } può contenere { $capacita } e con questa selezione arriverebbe a { $totale }. Oltre quel limite Windows elimina da solo gli elementi più vecchi.
# Capacità o contenuto del Cestino non letti. $volume lettera.
spostamento-capacita-ignota = Spostamento bloccato: impossibile leggere la capacità o il contenuto del Cestino di { $volume }.
# Sensore memoria attivo: dopo quanti giorni elimina gli elementi del Cestino. $giorni numero, $giorni_testo già scritto.
spostamento-sensore = { $giorni ->
    [one] Sensore memoria elimina in automatico gli elementi che restano nel Cestino per più di un giorno.
   *[other] Sensore memoria elimina in automatico gli elementi che restano nel Cestino per più di { $giorni_testo } giorni.
}
# Un altro controllo automatico della regola, con il suo esito. $controllo testo del controllo, $esito.
spostamento-controllo-regola = { $controllo }: { $esito }
# Titolo dei controlli manuali da spuntare.
spostamento-manuali = Da spuntare prima di spostare
# Controllo manuale di un elemento: il nome e il testo del controllo. $nome nome dell'elemento, $testo.
spostamento-manuale-elemento = { $nome }: { $testo }
# Controllo manuale per un file in una cartella che Quado riconosce dalla configurazione del programma (Dropbox, Google Drive). Qui "elimina" è giusto. $provider nome del programma.
spostamento-sincronizzato-cloud = Questo elemento sta nella cartella di { $provider }: spostarlo nel Cestino lo elimina anche nel cloud e sugli altri dispositivi collegati.
# Controllo manuale per un file in una cartella sincronizzata tra dispositivi senza cloud (Syncthing, Resilio Sync), riconosciuta dal suo marcatore. $provider nome del programma.
spostamento-sincronizzato-dispositivi = Questo elemento sta in una cartella sincronizzata con { $provider }: spostarlo nel Cestino lo elimina anche sugli altri dispositivi che sincronizzano la cartella.
# Esito del controllo delle cartelle sincronizzate quando nessun elemento ne sta in una: cosa Quado guarda e cosa no.
spostamento-sinc-ok = Nessun elemento sta in una cartella sincronizzata che Quado riconosce. Quado riconosce le cartelle registrate in Windows, come quelle di OneDrive, e quelle di Dropbox, Syncthing e Resilio Sync. Le cartelle di altri programmi di sincronizzazione controllale tu.
# Sotto i controlli, per ogni servizio con un cloud: le copie nel cloud non passano dal Cestino del PC. $provider nome del programma.
spostamento-sincronizzato-ripristino = I file che stanno solo nel cloud non arrivano nel Cestino di questo PC: si recuperano dal cestino di { $provider } sul web.
# Controllo manuale per un file sincronizzato. Qui "elimina" è giusto: il file sparisce dal cloud. $provider nome del provider.
spostamento-sincronizzato = Questo elemento è sincronizzato con { $provider }: spostarlo nel Cestino lo elimina anche nel cloud e sugli altri dispositivi. Per liberare spazio senza eliminarlo usa «Libera spazio» di { $provider }.
# Reversibilità, come si ripristina (nome del comando di Windows) e la domanda di Windows per gli elementi che non entrano nel Cestino: Quado non la sopprime (mover.rs; misurato: con "No" l'elemento resta).
spostamento-reversibilita = Puoi ripristinare gli elementi dal Cestino finché non lo svuoti tu o Sensore memoria: apri il Cestino, seleziona gli elementi e scegli «Ripristina gli elementi selezionati». Se Windows chiede di eliminare un elemento per sempre, scegli No: l'elemento resta al suo posto.
# Titolo della parte della conferma con le ipotesi e i rischi della raccomandazione, sopra l'elenco.
spostamento-ipotesi-rischi = Ipotesi e rischi
# Pulsante accanto all'elenco della conferma: spunta tutti gli elementi che si possono spostare.
spostamento-seleziona-tutti = Seleziona tutti
# Nel risultato, sotto i nomi degli elementi spostati: come rimetterli al loro posto (nome del comando di Windows).
spostamento-ripristina = Per rimetterli al loro posto: apri il Cestino, seleziona gli elementi e scegli «Ripristina gli elementi selezionati».
# Avviso con i punti di ripristino attivi.
spostamento-punti-ripristino = Con i punti di ripristino attivi, parte dello spazio liberato può tornare occupato nei giorni successivi.
# Stato dei punti di ripristino non letto.
spostamento-punti-ripristino-ignoti = Impossibile leggere lo stato dei punti di ripristino. Se sono attivi, parte dello spazio liberato può tornare occupato nei giorni successivi.
# Titolo della scheda completa della raccomandazione, in fondo al dialogo.
spostamento-scheda = Scheda della raccomandazione
# Sottotitolo della scheda: i fatti che dicono cos'è l'elemento.
spostamento-scheda-identita = Prova di identità
# Regola nella scheda: nome e identificativo. $nome, $id.
spostamento-scheda-regola = { $nome } · { $id }
# Fonte della regola nella scheda. $titolo titolo della fonte, $tipo ufficiale o no.
spostamento-scheda-fonte = Fonte: { $titolo } · { $tipo }
# Piede del dialogo.
spostamento-esc = Esc annulla.
# Pulsante del dialogo: chiude senza spostare niente.
spostamento-annulla = Annulla
# Pulsante principale del dialogo, l'unico in inchiostro: stesso verbo del titolo.
spostamento-sposta = Sposta nel Cestino
# Perché il pulsante principale è disattivato: nessun elemento scelto.
spostamento-manca-scelta = Scegli almeno un elemento.
# Perché il pulsante principale è disattivato: controlli manuali non spuntati.
spostamento-manca-spunta = Spunta i controlli qui sopra per spostare gli elementi.
# Perché il pulsante principale è disattivato: capacità del Cestino superata o non nota. Il dettaglio è nella riga del controllo.
spostamento-manca-capacita = Il controllo della capacità del Cestino blocca lo spostamento.
# Perché il pulsante principale è disattivato: elemento scelto con un controllo non superato.
spostamento-manca-blocco = Un elemento scelto ha un controllo non superato.
# Perché il pulsante principale è disattivato: il record non propone uno spostamento nel Cestino.
spostamento-manca-azione = Questa raccomandazione non propone uno spostamento nel Cestino.
# Avviso nella barra di stato: un altro spostamento è già in corso.
spostamento-occupato = Uno spostamento è già in corso: aspetta che finisca.
# Titolo del risultato.
spostamento-risultato-titolo = Risultato dello spostamento
# Sotto il titolo del risultato. $nome regola o funzione, $data e $ora della misura di partenza.
spostamento-risultato-quando = { $nome } · { $data } alle { $ora }
# Registro scritto. Il risultato sta in cima alla vista Interventi, sopra lo storico.
spostamento-registro-scritto = Intervento registrato nello storico qui sotto
# Registro in scrittura.
spostamento-registro-in-corso = Registrazione in corso…
# Registro non scritto, con la conseguenza. $codice errore di Windows.
spostamento-registro-errore = Impossibile scrivere il registro degli interventi: questo intervento non comparirà nello storico. (Codice { $codice })
# Registro non scritto perché manca la cartella dei dati di Quado.
spostamento-registro-nessuna-cartella = Intervento non registrato: la cartella dei dati di Quado non è disponibile.
# Passo 1 del risultato.
spostamento-passo-spostamento = Spostamento
# Esecuzione in corso. $fatti elementi conclusi, $totale elementi del piano, già scritti.
spostamento-in-corso = Spostamento in corso… { $fatti } di { $totale }
# Misura di partenza non riuscita: niente spostato. $codice errore di Windows.
spostamento-partenza-errore = Impossibile misurare lo spazio libero prima di spostare, quindi Quado non ha spostato niente. (Codice { $codice })
# Elementi spostati. $n spostati, $n_testo già scritto, $totale elementi del piano già scritto.
spostamento-spostati = { $n ->
    [one] Un elemento su { $totale } spostato nel Cestino
   *[other] { $n_testo } elementi su { $totale } spostati nel Cestino
}
# Tutti spostati: il ricontrollo li ha trovati come nell'anteprima.
spostamento-nessuno-saltato = Quado ha ricontrollato ogni elemento subito prima di spostarlo: tutti erano come nell'anteprima.
# Alcuni non spostati: il ricontrollo.
spostamento-ricontrollati = Quado ha ricontrollato ogni elemento subito prima di spostarlo. Gli elementi non spostati sono qui sotto, con il motivo.
# Un elemento non spostato con il suo esito. $nome nome, $esito.
spostamento-esito-riga = { $nome }: { $esito }
# Esito: saltato perché cambiato dopo l'anteprima.
spostamento-esito-cambiato = saltato perché è cambiato dopo l'anteprima. È ancora al suo posto.
# Esito: saltato perché non c'era più.
spostamento-esito-mancante = saltato perché non c'era più.
# Esito: saltato perché in uso al ricontrollo.
spostamento-esito-in-uso = saltato perché era in uso al ricontrollo. È ancora al suo posto.
# Esito: saltato perché il ricontrollo non lo ha letto. $codice errore di Windows.
spostamento-esito-illeggibile = saltato perché il ricontrollo non è riuscito a leggerlo. È ancora al suo posto. (Codice { $codice })
# Esito: Windows non lo ha spostato, anche quando l'utente risponde "No" alla domanda di Windows. $codice HRESULT esadecimale.
spostamento-esito-fallito = non spostato da Windows. È ancora al suo posto. (Codice { $codice })
# Esito: sparito senza arrivare nel Cestino; il piano si ferma qui e non tocca gli elementi seguenti. $codice HRESULT.
spostamento-esito-non-nel-cestino = è sparito dal suo posto senza arrivare nel Cestino. Quado si è fermato qui e ha lasciato al loro posto gli elementi seguenti. (Codice { $codice })
# Esito: non raggiunto perché il piano si è fermato prima.
spostamento-esito-non-raggiunto = non spostato: Quado si è fermato prima di arrivarci. È ancora al suo posto.
# Esito parziale: come ripristinare gli elementi spostati. «Ripristina» è il comando del Cestino di Windows. $n non spostati, $n_testo, $m nel Cestino già scritto.
spostamento-parziale = { $n ->
    [one] Un elemento non è stato spostato. Gli elementi spostati ({ $m }) sono nel Cestino: per rimetterli al loro posto apri il Cestino, selezionali e scegli «Ripristina».
   *[other] { $n_testo } elementi non sono stati spostati. Gli elementi spostati ({ $m }) sono nel Cestino: per rimetterli al loro posto apri il Cestino, selezionali e scegli «Ripristina».
}
# Nessun elemento spostato.
spostamento-nessuno = Nessun elemento è stato spostato nel Cestino.
# Passo 2 del risultato: spazio libero.
spostamento-passo-spazio = Spazio libero
# Spazio libero alla misura di partenza.
spostamento-spazio-prima = Prima
# Spazio libero alla misura dopo l'azione.
spostamento-spazio-ora = Ora
# Differenza tra le due misure.
spostamento-spazio-differenza = Differenza
# Perché lo spazio libero non cambia subito.
spostamento-spazio-nota = Lo spazio libero resta quasi lo stesso finché il Cestino trattiene i file: si libera quando lo svuoti tu o Sensore memoria.
# Misura non riuscita, dopo lo spostamento o dopo lo svuotamento. $codice errore di Windows.
spostamento-misura-errore = Impossibile misurare lo spazio libero: manca il confronto con la stima. (Codice { $codice })
# Valore con il segno più. $valore dimensione già scritta.
spostamento-piu = +{ $valore }
# Valore con il segno meno. $valore dimensione già scritta.
spostamento-meno = -{ $valore }
# Passo 3 del risultato: eliminazione dal Cestino.
spostamento-passo-cestino = Eliminazione dal Cestino
# Passo 3, sopra i pulsanti: eliminare solo gli elementi spostati da Quado, seguito dai nomi. $n elementi spostati, $n_testo, $data e $ora dello spostamento.
spostamento-apri-testo = { $n ->
    [one] Apri il Cestino ed elimina solo l'elemento spostato da Quado il { $data } alle { $ora }:
   *[other] Apri il Cestino ed elimina solo i { $n_testo } elementi spostati da Quado il { $data } alle { $ora }:
}
# Pulsante: apre il Cestino di Windows.
spostamento-apri = Apri il Cestino
# Passo 3, sopra i pulsanti: cos'altro elimina «Svuota il Cestino…». $n altri elementi, $n_testo, $byte dimensione già scritta.
spostamento-svuota-testo = { $n ->
    [one] Il dialogo di Windows elimina anche un altro elemento per { $byte }, secondo il conteggio di Windows.
   *[other] Il dialogo di Windows elimina anche { $n_testo } altri elementi per { $byte }, secondo il conteggio di Windows.
}
# Nessun altro elemento nel Cestino secondo Windows.
spostamento-svuota-nessun-altro = Secondo il conteggio di Windows, il Cestino contiene solo gli elementi spostati ora.
# Contenuto del Cestino non letto.
spostamento-svuota-ignoto = Il dialogo di Windows elimina anche gli altri elementi del Cestino. Windows non ha detto quanti sono.
# Pulsante: svuota il Cestino con il dialogo di conferma di Windows.
spostamento-svuota = Svuota il Cestino…
# Mentre il dialogo di Windows è aperto.
spostamento-svuota-in-corso = Il dialogo di Windows è aperto: conferma o annulla lì.
# Il Cestino non è stato svuotato: dialogo annullato o errore. $codice HRESULT esadecimale.
spostamento-svuota-errore = Il Cestino non è stato svuotato: il dialogo è stato annullato o Windows ha dato un errore. (Codice { $codice })
# Passo 4 del risultato: verifica.
spostamento-passo-verifica = Verifica dopo lo svuotamento
# Prima dello svuotamento: cosa farà la verifica. Le misure partono solo dal pulsante del passo 3 (spostamento-svuota, citato).
spostamento-verifica-attesa = Quando svuoti il Cestino con «Svuota il Cestino…», Quado misura di nuovo lo spazio libero e le cartelle toccate, poi confronta il risultato con la stima.
# Titolo del confronto subito dopo lo spostamento.
spostamento-confronto-dopo-azione = Subito dopo lo spostamento
# Titolo del confronto dopo lo svuotamento.
spostamento-confronto-dopo-svuotamento = Dopo lo svuotamento del Cestino
# Primo dei due numeri separati del confronto: la stima.
spostamento-stimato = Stimato
# Secondo dei due numeri separati del confronto: l'ottenuto, cioè la variazione misurata dello spazio libero.
spostamento-variazione = Variazione dello spazio libero
# Sotto i due numeri: cosa comprende la variazione.
spostamento-variazione-nota = La variazione comprende anche ciò che altri programmi hanno scritto o eliminato nel frattempo.
# Differenza tra stimato e ottenuto. $valore con il segno, $percento.
spostamento-differenza = Differenza: { $valore } ({ $percento })
# Differenza senza percentuale (stima a intervallo che parte da zero). $valore con il segno.
spostamento-differenza-senza-percento = Differenza: { $valore }
# La variazione cade nella stima.
spostamento-differenza-nessuna = La variazione rientra nella stima.
# Stima senza valore o pari a zero: solo l'ottenuto.
spostamento-solo-ottenuto = Senza un valore stimato conta solo la variazione dello spazio libero.
# Voce di spiegazione: elementi saltati o falliti. $n elementi, $n_testo, $byte già scritti.
spostamento-voce-saltati = { $n ->
    [one] Un elemento non spostato: { $byte } della stima restano al loro posto.
   *[other] { $n_testo } elementi non spostati: { $byte } della stima restano al loro posto.
}
# Voce di spiegazione: file con altri nomi. $n file, $n_testo, $byte già scritti.
spostamento-voce-altri-nomi = { $n ->
    [one] Un file ha altri nomi (hard link) fuori dalla selezione ({ $byte }): il suo spazio si libera solo quando non resta nessun nome. La stima lo esclude già.
   *[other] { $n_testo } file hanno altri nomi (hard link) fuori dalla selezione ({ $byte }): il loro spazio si libera solo quando non resta nessun nome. La stima li esclude già.
}
# Voce di spiegazione: contenuto ancora nel Cestino. $byte già scritti.
spostamento-voce-nel-cestino = Ancora nel Cestino: { $byte }. Questo spazio si libera quando il Cestino si svuota.
# Voce di spiegazione: altri elementi eliminati svuotando il Cestino. $n elementi, $n_testo, $byte già scritti.
spostamento-voce-altri-elementi = { $n ->
    [one] Svuotando il Cestino è stato eliminato anche un altro elemento, per { $byte } secondo il conteggio di Windows.
   *[other] Svuotando il Cestino sono stati eliminati anche { $n_testo } altri elementi, per { $byte } secondo il conteggio di Windows.
}
# Voce di spiegazione: file ancora in uso allo svuotamento. $n file, $n_testo.
spostamento-voce-file-aperti = { $n ->
    [one] Un file della selezione era ancora in uso quando il Cestino è stato svuotato, secondo Windows.
   *[other] { $n_testo } file della selezione erano ancora in uso quando il Cestino è stato svuotato, secondo Windows.
}
# Voce di spiegazione: attività di altri programmi. $valore con il segno.
spostamento-voce-altri-processi = Attività di altri programmi: { $valore }. La nuova scansione delle cartelle conferma che gli elementi spostati non occupano più spazio, quindi questa parte della differenza non viene dall'intervento.
# Differenza oltre il 5% che le voci non spiegano. $valore con il segno, $percento.
spostamento-non-spiegata = Differenza non spiegata: { $valore } ({ $percento }).
# Regola del 5%.
spostamento-regola-5 = Oltre il 5% di differenza, Quado la spiega voce per voce o la dichiara non spiegata.
# Nuova scansione delle cartelle toccate in corso.
spostamento-riscansione-in-corso = Nuova scansione delle cartelle toccate in corso…
# Nuova scansione conclusa.
spostamento-riscansione-fatta = Cartelle toccate scansionate di nuovo: i totali sono aggiornati.
# Tra le cartelle toccate c'è la radice dell'unità: nessuna nuova scansione parziale. Cita barra-riesamina.
spostamento-riscansione-radice = Alcuni elementi stavano nella radice dell'unità. Per aggiornare i totali scegli «Scansiona di nuovo»: Quado rilegge tutta l'unità.
# Cartelle non scansionate di nuovo. Cita barra-riesamina. $n cartelle, $n_testo.
spostamento-riscansione-fallite = { $n ->
    [one] Impossibile scansionare di nuovo una cartella toccata: i suoi totali restano quelli di prima. Per aggiornarli scegli «Scansiona di nuovo».
   *[other] Impossibile scansionare di nuovo { $n_testo } cartelle toccate: i loro totali restano quelli di prima. Per aggiornarli scegli «Scansiona di nuovo».
}

## Vista Interventi, procedure e comandi, Dati e privacy.

# Procedure e comandi sotto il record.
# Pulsante della procedura del Cestino: misura lo spazio libero e apre il Cestino in Esplora file.
procedura-apri-cestino = Apri il Cestino
# Pulsante della procedura di Impostazioni › Sistema › Archiviazione (ms-settings:storagesense): File temporanei è a un clic da lì.
procedura-apri-archiviazione = Apri Impostazioni › Archiviazione
# Pulsante della procedura File temporanei, se Windows ha una pagina diretta.
procedura-apri-file-temporanei = Apri Impostazioni › File temporanei
# Pulsante per le procedure dentro un'app (cache di Chrome, di CapCut): Quado non apre l'app, misura lo spazio libero di partenza.
procedura-misura-partenza = Misura lo spazio libero
# Suggerimento del pulsante precedente. Cita azione-ho-eseguito.
procedura-misura-partenza-suggerimento = Legge lo spazio libero e il contenuto del Cestino secondo Windows. Poi segui la procedura nell'app e scegli «Ho eseguito la procedura».
# Pulsante del record del Cestino: apre il dialogo di conferma di Windows, dopo il testo con il conteggio. Mai "svuota tutto" implicito.
procedura-svuota-cestino = Svuota il Cestino…
# Prima di "Svuota il Cestino…": conteggio di Windows letto ora. Svuotare elimina per sempre. $n e $testo numero di elementi, $dimensione già scritta. Cita procedura-apri-cestino.
procedura-cestino-conteggio = Il dialogo di Windows elimina per sempre { $n ->
    [one] l'unico elemento del Cestino
   *[other] tutti i { $testo } elementi del Cestino
}: { $dimensione } secondo il conteggio di Windows. Per eliminarne solo alcuni usa «Apri il Cestino».
# Il conteggio di Windows non è ancora arrivato.
procedura-cestino-lettura = Lettura del Cestino in corso…
# Windows dice che il Cestino è vuoto: il pulsante è disattivato.
procedura-cestino-vuoto = Il Cestino è vuoto secondo Windows.
# Windows non dà il conteggio (unità senza Cestino): lo svuotamento non si propone. Cita procedura-svuota-cestino.
procedura-cestino-non-letto = Windows non dice quanti elementi contiene il Cestino, quindi «Svuota il Cestino…» resta disattivato.
# La lettura del Cestino non è riuscita. $codice numero di Windows. Cita procedura-svuota-cestino.
procedura-cestino-errore = Impossibile leggere il Cestino, quindi «Svuota il Cestino…» resta disattivato. (Codice { $codice })
# Misure in corso dopo un pulsante.
procedura-misura-in-corso = Misura in corso… spazio libero, Cestino e cartelle toccate
# Dopo "Apri…": la misura di partenza è stata presa prima di aprire la pagina. $ora, $libero (dimensione), $volume (C:\). Cita azione-ho-eseguito.
procedura-aperta = Pagina aperta alle { $ora }. Spazio libero prima della procedura: { $libero } su { $volume }. Quando hai finito, scegli «Ho eseguito la procedura».
# Dopo "Copia" di un comando: la misura di partenza è stata presa alla copia. Cita azione-ho-eseguito.
procedura-copiato = Comando copiato alle { $ora }. Spazio libero prima del comando: { $libero } su { $volume }. Quado non esegue il comando: dopo averlo eseguito, scegli «Ho eseguito la procedura».
# Dopo "Misura lo spazio libero". Cita azione-ho-eseguito.
procedura-partenza = Spazio libero alle { $ora }, prima della procedura: { $libero } su { $volume }. Quando hai finito, scegli «Ho eseguito la procedura».
# La misura di partenza non è riuscita: la variazione non si potrà calcolare. $codice numero di Windows.
procedura-partenza-errore = Impossibile misurare lo spazio libero prima della procedura: la variazione non si potrà calcolare. (Codice { $codice })
# La shell non ha aperto la pagina. $codice esadecimale.
procedura-pagina-errore = Impossibile aprire la pagina. Riprova, oppure aprila a mano. (Codice { $codice })
# URI della regola non tra quelli che il lanciatore conosce: non si apre niente (barra di stato).
procedura-pagina-non-valida = Impossibile aprire la pagina di questa procedura: il suo indirizzo non è tra quelli che Quado apre.
# Comando ufficiale, prima della copia. Cita record-copia e azione-ho-eseguito.
procedura-comando-nota = Quado mostra e copia il comando, ma non lo esegue. «Copia» misura anche lo spazio libero di partenza. Dopo aver eseguito il comando, scegli «Ho eseguito la procedura» per misurare il risultato.
# "Ho eseguito la procedura" senza misura di partenza: si registra la sola misura. Cita azione-ho-eseguito.
procedura-senza-partenza = Senza una misura dello spazio libero presa prima della procedura, «Ho eseguito la procedura» registra solo lo spazio libero di adesso e non calcola la variazione.
# La misura dopo la procedura non è riuscita. $codice numero di Windows.
procedura-misura-errore = Impossibile misurare lo spazio libero: Quado non ha registrato niente. (Codice { $codice })
# Esito senza misura di partenza.
procedura-solo-misura = Spazio libero alle { $ora }: { $libero }. Senza la misura di partenza la variazione non si calcola.
# Gli elementi stanno nella radice dell'unità (file di ibernazione): la nuova scansione parziale non si fa. Cita barra-riesamina.
procedura-serve-riesamina = Questi elementi stanno nella radice dell'unità: per aggiornare i totali scegli «Scansiona di nuovo».
# Cartelle non scansionate di nuovo. $n e $testo quante, $codice del primo errore.
procedura-riscansione-errore = Impossibile scansionare di nuovo { $n ->
    [one] una cartella
   *[other] { $testo } cartelle
}: i totali restano quelli di prima. (Codice { $codice })
# Codice restituito dal dialogo di Windows per lo svuotamento: l'esito lo dicono le misure.
procedura-svuota-codice = Il dialogo di Windows si è chiuso con un codice: cosa è cambiato lo dicono le misure. (Codice { $codice })
# Registro scritto.
procedura-registrato = Intervento registrato nella vista Interventi.
# Registro non scritto, con la conseguenza. $codice numero di Windows.
procedura-registro-errore = Impossibile scrivere il registro degli interventi: questa misura non comparirà nella vista Interventi. (Codice { $codice })
# Dialogo di disinstallazione, dopo l'apertura. Cita azione-ho-eseguito e barra-riesamina.
procedura-disinstallazione-dopo = Quando hai finito, scegli «Ho eseguito la procedura»: Quado misura di nuovo lo spazio libero e la cartella del programma. «Scansiona di nuovo» rilegge tutta l'unità.

# Verifica: testi comuni allo storico e all'esito delle procedure.
# Variazione positiva: spazio liberato. $valore dimensione già scritta.
storico-piu = +{ $valore }
# Variazione negativa: spazio occupato.
storico-meno = -{ $valore }
# Stima a intervallo, nella riga "Stimato": stessa forma del dialogo.
storico-intervallo = da { $minimo } a { $massimo }
# Stimato e ottenuto: due numeri separati, mai sommati.
storico-verifica = Stimato: { $stimato }. Variazione dello spazio libero: { $variazione }. La variazione comprende anche ciò che altri programmi hanno scritto o eliminato nel frattempo.
# Stima non stimabile o pari a 0: solo l'ottenuto.
storico-verifica-solo-variazione = Variazione dello spazio libero: { $variazione }. La variazione comprende anche ciò che altri programmi hanno scritto o eliminato nel frattempo.
# Differenza oltre il 5%: seguono le voci. $percentuale già scritta.
storico-differenza-oltre = Differenza: { $differenza } ({ $percentuale }), oltre il 5%.
# Differenza entro il 5%.
storico-differenza-entro = Differenza: { $differenza } ({ $percentuale }), entro il 5%.
# Differenza senza percentuale (intervallo che parte da 0).
storico-differenza = Differenza: { $differenza }.
# Parte della differenza che le voci non spiegano.
storico-non-spiegata = Differenza non spiegata: { $differenza } ({ $percentuale }).
# Voce: elementi saltati o falliti (riepilogo dell'esecuzione).
storico-spiegazione-saltati = { $n ->
    [one] Un elemento non spostato
   *[other] { $testo } elementi non spostati
}: { $dimensione } della stima restano al loro posto.
# Voce: file esclusi perché hanno altri nomi.
storico-spiegazione-altri-nomi = { $n ->
    [one] Un file escluso dalla stima perché ha altri nomi (hard link) fuori dalla selezione
   *[other] { $testo } file esclusi dalla stima perché hanno altri nomi (hard link) fuori dalla selezione
}: { $dimensione }.
# Voce: contenuto ancora nel Cestino.
storico-spiegazione-nel-cestino = Ancora nel Cestino: { $dimensione }.
# Voce: altri elementi eliminati svuotando il Cestino.
storico-spiegazione-altri-elementi = Svuotando il Cestino sono stati eliminati anche { $n ->
    [one] un altro elemento
   *[other] { $testo } altri elementi
}: { $dimensione }.
# Voce: file ancora in uso al momento dello svuotamento.
storico-spiegazione-file-aperti = { $n ->
    [one] Un file era ancora in uso
   *[other] { $testo } file erano ancora in uso
} quando il Cestino è stato svuotato.
# Voce: attività di altri programmi, quando la nuova scansione dice che gli elementi non occupano più spazio. Non si attribuisce all'intervento.
storico-spiegazione-altri-processi = Attività di altri programmi: { $variazione }. La nuova scansione conferma che gli elementi non occupano più spazio.
# Quanto occupano ancora gli elementi secondo la nuova scansione delle cartelle toccate.
storico-ancora-presenti = Secondo la nuova scansione, gli elementi occupano ancora { $dimensione }.

# Vista Interventi: storico dal registro.
# Titolo dello storico.
storico-titolo = Storico degli interventi
# Sottotitolo: cosa c'è in ogni voce, poi il percorso del file. $percorso del file interventi.jsonl.
storico-sottotitolo = Misure prima e dopo ogni intervento, con la stima e la variazione dello spazio libero. Registro: { $percorso }
# Senza cartella dei dati (nei test, o con %APPDATA% non disponibile).
storico-non-disponibile = Storico non disponibile in questa sessione: manca la cartella dei dati di Quado.
# Il registro non si legge. $codice numero di Windows.
storico-errore = Impossibile leggere il registro degli interventi. Il file resta com'è e lo trovi in Impostazioni › Dati e privacy. (Codice { $codice })
# Righe illeggibili del registro, dichiarate con le due cause possibili (scritte a metà o da un'altra versione).
storico-illeggibili = { $n ->
    [one] Una riga del registro non si legge e lo storico la omette. Può essere scritta a metà o venire da un'altra versione di Quado.
   *[other] { $testo } righe del registro non si leggono e lo storico le omette. Possono essere scritte a metà o venire da un'altra versione di Quado.
}
# Stato vuoto: registro assente o vuoto. Dice cosa comparirà.
storico-vuoto = Nessun intervento registrato. Gli spostamenti nel Cestino e le procedure misurate da Quado compaiono qui.
# Pulsante dello stato vuoto dello storico, l'unica azione: porta alla card "Interventi possibili" della Panoramica.
storico-vuoto-azione = Vedi gli interventi possibili
# Numero di interventi sopra l'elenco.
storico-numero = { $n ->
    [one] Un intervento registrato
   *[other] { $testo } interventi registrati
}, dal più recente.
# Titolo di una voce: data e ora della misura di partenza, tipo, regola o funzione.
storico-voce = { $data } alle { $ora } · { $tipo } · { $origine }
# Tipo: spostamento nel Cestino.
storico-tipo-spostamento = Spostamento nel Cestino
# Tipo: procedura di Windows. $nome della procedura.
storico-tipo-procedura = Procedura: { $nome }
# Tipo: procedura dentro l'app che gestisce i dati.
storico-tipo-procedura-app = Procedura nell'app che gestisce i dati
# Tipo: comando ufficiale copiato.
storico-tipo-comando = Comando ufficiale, copiato e non eseguito da Quado
# Nomi delle procedure nel registro.
storico-procedura-cestino-apri = Apri il Cestino
# Nome di procedura.
storico-procedura-cestino-svuota = Svuota il Cestino
# Nome di procedura.
storico-procedura-archiviazione = Impostazioni › Archiviazione
# Nome di procedura.
storico-procedura-file-temporanei = Impostazioni › File temporanei
# Nome di procedura: disinstallazione dall'interfaccia ufficiale (Impostazioni, Steam o Epic).
storico-procedura-disinstallazione = disinstallazione del programma
# Elementi di uno spostamento. $testo quanti, $esiti già scritti.
storico-elementi = Elementi: { $n ->
    [one] uno
   *[other] { $testo }
} ({ $esiti })
# Spostamento senza elementi.
storico-elementi-nessuno = Elementi: nessuno
# Procedure e comandi: Quado non tocca elementi.
storico-elementi-procedura = Elementi: nessuno toccato da Quado
# Esito: nel Cestino.
storico-esito-nel-cestino = { $testo } nel Cestino
# Esito: saltati.
storico-esito-saltati = { $n ->
    [one] { $testo } saltato
   *[other] { $testo } saltati
}
# Esito: falliti, cioè non spostati da Windows.
storico-esito-falliti = { $n ->
    [one] { $testo } non spostato da Windows
   *[other] { $testo } non spostati da Windows
}
# Esito: spariti senza arrivare nel Cestino.
storico-esito-non-nel-cestino = { $n ->
    [one] { $testo } non arrivato nel Cestino
   *[other] { $testo } non arrivati nel Cestino
}
# Esito: non raggiunti (piano fermato prima).
storico-esito-non-raggiunti = { $n ->
    [one] { $testo } non raggiunto
   *[other] { $testo } non raggiunti
}
# Metodo: URI, comando o codice dell'azione, e l'unità.
storico-metodo = Metodo: { $metodo } · unità { $volume }
# Metodo non registrato.
storico-metodo-nessuno = nessuno
# Una sola misura: senza la partenza la variazione non si calcola.
storico-una-misura = Spazio libero alle { $ora }: { $libero }. Senza la misura di partenza la variazione non si calcola.
# Nessuna misura nel registro.
storico-nessuna-misura = Nessuna misura registrata.
# Più misure ma nessun confronto.
storico-senza-confronto = Misure registrate senza un confronto con la stima.

# Impostazioni › Dati e privacy.
# Titolo della card.
privacy-titolo = Dati e privacy
# Nota. $cartella è %APPDATA%\Quado.
privacy-nota = Quado non usa la rete. Salva solo questi file, in { $cartella } sul tuo PC.
# Senza cartella dei dati (nei test, o con %APPDATA% non disponibile).
privacy-non-disponibile = In questa sessione manca la cartella dei dati, quindi Quado non salva impostazioni e registro degli interventi.
# Nessun file salvato.
privacy-niente = Quado non ha ancora salvato niente.
# Riga: file delle impostazioni.
privacy-file-impostazioni = Impostazioni: { $dimensione }
# Riga: registro degli interventi.
privacy-file-registro = Registro degli interventi: { $dimensione }
# Riga: cartella delle lingue.
privacy-file-lingue = File delle lingue: { $dimensione }
# Pulsante: chiede conferma prima di cancellare il registro.
privacy-cancella-registro = Cancella il registro degli interventi…
# Il registro non c'è: il pulsante è disattivato.
privacy-registro-assente = Il registro degli interventi non c'è ancora: si crea al primo intervento.
# Conferma, in forma di domanda: il file si cancella per sempre (journal::delete). $dimensione del registro.
privacy-conferma = Cancellare il registro degli interventi ({ $dimensione })? Lo storico della vista Interventi si svuota e non si può recuperare. Impostazioni, lingue ed elementi nel Cestino restano come sono.
# Pulsante di conferma: stesso verbo della domanda.
privacy-cancella-conferma = Cancella il registro
# Pulsante che chiude la conferma senza cancellare.
privacy-annulla = Annulla
# Barra di stato: registro cancellato.
privacy-registro-cancellato = Registro degli interventi cancellato.
# Barra di stato: cancellazione non riuscita. $codice numero di Windows.
privacy-registro-errore = Impossibile cancellare il registro degli interventi: il file è ancora nella cartella dei dati. (Codice { $codice })
# Dati e privacy, cartella della versione precedente (%APPDATA%\Rilievo, legacy.rs): compare solo se la cartella c'è.
# Titolo della riga.
privacy-rilievo-titolo = Dati di Rilievo, la versione precedente
# Sotto il titolo. $dimensione della cartella, $cartella il percorso. Quado ne copia i dati solo se la sua cartella non c'è ancora.
privacy-rilievo-nota = { $dimensione } in { $cartella }. Quado ne copia impostazioni, registro e lingue solo al primo avvio, poi non la usa più.
# Pulsante: chiede conferma prima di cancellare la cartella.
privacy-rilievo-cancella = Cancella la cartella di Rilievo…
# Conferma, in forma di domanda: la cartella si cancella per sempre (legacy::delete_old). $dimensione della cartella.
privacy-rilievo-conferma = Cancellare la cartella di Rilievo ({ $dimensione })? I file non si possono recuperare. I dati di Quado restano come sono.
# Pulsante di conferma: stesso verbo della domanda.
privacy-rilievo-cancella-conferma = Cancella la cartella
# Avviso dopo la cancellazione.
privacy-rilievo-cancellata = Cartella di Rilievo cancellata.
# Errore della cancellazione: di solito un file aperto da Rilievo, se è ancora in esecuzione.
privacy-rilievo-errore = Impossibile cancellare tutta la cartella di Rilievo: alcuni file sono ancora lì. Chiudi Rilievo, se è aperto, e riprova. (Codice { $codice })
