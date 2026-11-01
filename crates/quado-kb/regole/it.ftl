## Testi delle regole e del motore di classificazione (05 D6, D13). Lingua di riferimento: ogni chiave
## ha un commento di contesto. Segnaposto: i numeri arrivano due volte, `$x` (per il plurale) e
## `$x_testo` (già formattato con il formato dell'area geografica); dimensioni e date arrivano già
## scritte. I valori dei fatti sono `$a`, `$b`, `$c` nell'ordine. Parole vietate (04 §6.8): mai
## "sicuro da eliminare", "inutilizzato", "non più in uso", "spazzatura", "ottimizza". Guida: docs/testi.md.

## Regole: `nome` è il titolo breve (card interventi, elenco delle regole); `riga` è la spiegazione di
## una riga sotto il nome dell'elemento (top list, ispettore). Punto finale solo nelle frasi complete:
## i frammenti nominali e i percorsi di menu (A › B) non lo hanno.

# Titolo della regola sulle cartelle di versione di CapCut.
regola-capcut-versioni-nome = Versioni precedenti di CapCut
# Riga sotto le cartelle di versione. $n cartelle. "(dedotto)" resta sempre (decisioni §3).
regola-capcut-versioni-riga = { $n ->
    [one] { $n_testo } cartella di una versione precedente (dedotto)
   *[other] { $n_testo } cartelle di versioni precedenti (dedotto)
}
# Metodo: spostamento nel Cestino dopo l'anteprima (M5). Quado è l'attore.
regola-capcut-versioni-metodo = Quado sposta nel Cestino le cartelle che scegli, dopo l'anteprima. La versione in uso e quella precedente restano al loro posto.
# Ipotesi della regola, scritta come tale.
regola-capcut-versioni-ipotesi-1 = Ipotesi: CapCut avvia solo la versione in uso e le altre cartelle sono rimaste dagli aggiornamenti.
# Rischio se l'ipotesi è sbagliata, con il modo per tornare indietro (valutazione dei consigli, 01/11/2026).
regola-capcut-versioni-rischio-1 = Se l'ipotesi è sbagliata, CapCut non si avvia. Le cartelle si ripristinano dal Cestino finché non lo svuoti.
# Da fare prima dell'anteprima: con CapCut aperto il controllo dei file in uso blocca lo spostamento.
regola-capcut-versioni-rischio-2 = Chiudi CapCut prima dell'anteprima: se è aperto, il controllo dei file in uso blocca lo spostamento.
# Nel risultato dello spostamento, prima di «Svuota il Cestino…»: cosa provare (decisioni §3, 04 §6.8).
regola-capcut-versioni-prova = Prima di svuotare il Cestino apri CapCut e controlla che funzioni.
# Casella che attiva «Svuota il Cestino…» nel risultato. Parla l'utente.
regola-capcut-versioni-prova-fatta = Ho aperto CapCut e funziona

# Titolo della regola sui progetti di CapCut.
regola-capcut-progetti-nome = Progetti di CapCut
# Riga sotto Projects e Videos di CapCut (04 §6.8).
regola-capcut-progetti-riga = I tuoi progetti video. Non è cache: Quado non propone di eliminarli.

# Titolo della regola sulla cache di CapCut.
regola-capcut-cache-nome = Cache di CapCut
# Riga sotto la cache di CapCut: cosa è e dove la svuoti.
regola-capcut-cache-riga = Cache di CapCut: la svuoti dalle impostazioni di CapCut.
# Procedura nell'app, dalla fonte. Percorso di menu.
regola-capcut-cache-metodo = In CapCut: Impostazioni › Bozza › Svuota cache (l'icona del cestino accanto alla dimensione della cache)

# Titolo della regola sul file di ibernazione.
regola-ibernazione-nome = File di ibernazione
# Riga sotto hiberfil.sys.
regola-ibernazione-riga = File di ibernazione di Windows: puoi ridurlo o toglierlo con powercfg.
# Primo dei due comandi affiancati (decisioni §3, 04 §6.8), sopra powercfg /hibernate /type reduced.
regola-ibernazione-metodo = Ridurre il file di ibernazione: mantiene l'avvio rapido e toglie l'ibernazione.
# Secondo comando, sopra powercfg /h off.
regola-ibernazione-alternativa-1 = Disattivare l'ibernazione: toglie anche l'avvio rapido e la sospensione ibrida.
# Conseguenza da dire sempre (decisioni §3), in tono di avviso: anche il file ridotto toglie l'ibernazione.
regola-ibernazione-rischio-1 = Entrambi i comandi tolgono l'ibernazione e vanno eseguiti come amministratore.
# Solo se il PC ha una batteria (lo aggiunge il motore).
regola-ibernazione-rischio-batteria = Con la batteria critica il portatile non può più ibernarsi: se la batteria si scarica, il lavoro non salvato può andare perso.
# Solo con la batteria e lo standby moderno (Adaptive Hibernate di Microsoft).
regola-ibernazione-rischio-standby = Questo PC usa lo standby moderno: in sospensione Windows lo iberna da solo per non scaricare la batteria. Senza ibernazione la batteria può scaricarsi del tutto.

# Titolo della regola sul Cestino.
regola-cestino-nome = Cestino
# Riga della top list: solo le due misure (F-OV-2, 04 §6.8).
regola-cestino-riga = { $disco } su disco, { $windows } secondo Windows
# Procedura preferita (04 §6.7).
regola-cestino-metodo = Apri il Cestino ed elimina solo gli elementi che scegli.
# Alternativa: dialogo di Windows.
regola-cestino-alternativa-1 = Svuota il Cestino con il dialogo di Windows: elimina tutti gli elementi.

# Titolo della regola sulle cartelle orfane del Cestino.
regola-cestino-orfane-nome = Cartelle del Cestino senza indice
# Riga sotto le cartelle orfane (04 §6.8, ispettore).
regola-cestino-orfane-riga = Nella cartella del Cestino ci sono { $disco } che il Cestino di Windows non elenca: cartelle senza il loro file indice. Svuotare il Cestino non le elimina: provato su un disco virtuale.
# Ipotesi.
regola-cestino-orfane-ipotesi-1 = Ipotesi: sono rimaste da spostamenti nel Cestino interrotti o da file indice rimossi.

# Titolo della regola su WinSxS. Il nome della cartella compare già come nome dell'elemento.
regola-winsxs-nome = Archivio dei componenti di Windows
# Riga sotto WinSxS: cosa è, chi lo riduce e la conseguenza scritta nella fonte ufficiale. Nomi di Pulizia disco
# come li scrive Windows in italiano.
regola-winsxs-riga = Componenti di Windows: Windows riduce l'archivio da solo. Per ridurlo prima usa Pulizia disco › Pulisci file di sistema › Pulizia di Windows Update. Eliminare file a mano può danneggiare Windows.

# Titolo della regola su Windows\Installer.
regola-windows-installer-nome = Cache di Windows Installer
# Riga sotto Windows\Installer: cosa contiene e la conseguenza scritta nella fonte ufficiale.
regola-windows-installer-riga = Pacchetti che servono a riparare e disinstallare i programmi: eliminarli fa fallire queste operazioni.

# Titolo della regola sui file temporanei.
regola-file-temporanei-nome = File temporanei di Windows
# Riga sotto Temp, SoftwareDistribution\Download e cache delle miniature.
regola-file-temporanei-riga = File temporanei: li elimini da Impostazioni › File temporanei, mai a mano.
# Procedura preferita: percorso in Impostazioni, sopra l'indirizzo ms-settings:storagesense.
regola-file-temporanei-metodo = Impostazioni › Sistema › Archiviazione › File temporanei
# Alternativa: nome dello strumento di Windows, sopra il comando cleanmgr.
regola-file-temporanei-alternativa-1 = Pulizia disco di Windows
# Errore possibile fuori da Quado, nella stessa pagina di Impostazioni (valutazione dei consigli, 01/11/2026).
regola-file-temporanei-rischio-1 = Lascia senza spunta Download e Cestino se vuoi tenerli: Windows elimina i file senza passare dal Cestino.

# Titolo della regola sulla cache di npm.
regola-npm-cache-nome = Cache di npm
# Riga sotto la cache di npm.
regola-npm-cache-riga = Cache di npm: la gestisce npm con i suoi comandi.
# Comando preferito (04 §6.8), sopra npm cache verify: cosa fa secondo la documentazione di npm.
regola-npm-cache-metodo = Controlla la cache ed elimina solo i dati che non servono più: di solito libera poco spazio (dedotto).
# Alternativa, sopra npm cache clean --force. Le conseguenze sono nel rischio.
regola-npm-cache-alternativa-1 = Svuota tutta la cache: è il comando che recupera lo spazio.
# Conseguenze di clean --force (decisioni §3).
regola-npm-cache-rischio-1 = Dopo clean --force, npm riscarica i pacchetti alla prossima installazione; senza rete non li trova.

# Titolo della regola sullo store di pnpm.
regola-pnpm-store-nome = Store di pnpm
# Riga sotto lo store di pnpm. Hard link: il termine tecnico tra parentesi (glossario).
regola-pnpm-store-riga = Store di pnpm: lo gestisce pnpm. Contiene file con più nomi (hard link), usati anche dai progetti.
# Comando preferito, sopra pnpm store prune.
regola-pnpm-store-metodo = Elimina dallo store i pacchetti a cui nessun progetto fa riferimento.

# Titolo della regola sui giochi di Steam.
regola-steam-nome = Giochi di Steam
# Riga sotto la cartella di un gioco. $nome dal manifest.
regola-steam-riga = { $nome }, gioco di Steam
# Metodo.
regola-steam-metodo = Disinstalla dalla libreria di Steam.

# Titolo della regola sulle installazioni di Epic.
regola-epic-nome = Installazioni di Epic
# Riga sotto una cartella di Epic. $n prodotti nella stessa cartella.
regola-epic-riga = { $n ->
    [one] { $nome }, installato da Epic Games Launcher
   *[other] { $nome } e altri prodotti nella stessa cartella ({ $n_testo } in tutto), installati da Epic Games Launcher
}
# Metodo: percorso di menu nel launcher.
regola-epic-metodo = Libreria di Epic Games Launcher › i tre puntini accanto al titolo › Disinstalla

# Titolo della regola sui dispositivi virtuali Android.
regola-android-avd-nome = Dispositivi virtuali Android
# Riga sotto le cartelle .avd: cosa sono e dove li elimini.
regola-android-avd-riga = Dischi dei dispositivi virtuali di Android Studio: li elimini da Device Manager.
# Procedura nell'app: percorso di menu, con i nomi inglesi di Android Studio.
regola-android-avd-metodo = Android Studio › Device Manager › menu del dispositivo › Delete
# Conseguenza della procedura (valutazione dei consigli, 01/11/2026).
regola-android-avd-rischio-1 = Si perdono le app e i dati dentro il dispositivo virtuale.

# Titolo della regola sull'archivio dei driver.
regola-driverstore-nome = Archivio dei driver
# Riga sotto DriverStore\FileRepository: cosa contiene e cosa dice la documentazione di Microsoft.
regola-driverstore-riga = Pacchetti dei driver installati: secondo Microsoft non vanno rimossi né modificati.

# Titolo della regola su file di paging e swap.
regola-paging-nome = File di paging e swap
# Riga sotto pagefile.sys e swapfile.sys: lasciare le dimensioni a Windows (fonte ufficiale).
regola-paging-riga = Memoria virtuale di Windows: lascia che sia Windows a gestirne le dimensioni. Se la riduci, Windows può bloccarsi quando la memoria finisce.

# Titolo della regola sulla cache di Chrome.
regola-cache-chrome-nome = Cache di Chrome
# Riga sotto la cache di Chrome.
regola-cache-chrome-riga = Cache di Chrome: la svuoti da Chrome.
# Procedura nel browser: percorso di menu.
regola-cache-chrome-metodo = In Chrome: menu in alto a destra › Elimina dati di navigazione. Spunta solo «Immagini e file memorizzati nella cache», con l'intervallo «Dall'inizio».
# Errore possibile nella stessa finestra del browser (vale anche per Brave ed Edge).
regola-cache-chrome-rischio-1 = Lascia senza spunta cookie e password: se li elimini, esci dai siti in cui sei entrato.

# Titolo della regola sulla cache di Brave.
regola-cache-brave-nome = Cache di Brave
# Riga sotto la cache di Brave.
regola-cache-brave-riga = Cache di Brave: la svuoti da Brave.
# Procedura nel browser: l'indirizzo va scritto nella barra degli indirizzi (fonte).
regola-cache-brave-metodo = In Brave: scrivi brave://settings/clearBrowserData nella barra degli indirizzi. Spunta solo «Immagini e file memorizzati nella cache».
# Errore possibile nella stessa finestra del browser.
regola-cache-brave-rischio-1 = Lascia senza spunta cookie e password: se li elimini, esci dai siti in cui sei entrato.

# Titolo della regola sulla cache di Edge.
regola-cache-edge-nome = Cache di Edge
# Riga sotto la cache di Edge.
regola-cache-edge-riga = Cache di Edge: la svuoti da Edge.
# Procedura nel browser: percorso di menu.
regola-cache-edge-metodo = In Edge: Impostazioni › Privacy › Cancella dati delle esplorazioni. Spunta solo «Immagini e file memorizzati nella cache».
# Errore possibile nella stessa finestra del browser.
regola-cache-edge-rischio-1 = Lascia senza spunta cookie e password: se li elimini, esci dai siti in cui sei entrato.

# Titolo della regola su node_modules, target e .next.
regola-progetti-sviluppo-nome = Dipendenze e build dei progetti
# Riga sotto la cartella. $nome è node_modules, target o .next; $lockfile il file accanto; $comando il comando che
# di solito la ricrea (cargo build, npm install, npm run build...). Deduzione, quindi "(dedotto)".
regola-progetti-sviluppo-riga = { $nome } con { $lockfile } accanto: di solito si ricrea con { $comando } (dedotto)
# Metodo: spostamento nel Cestino su scelta dell'utente, dopo l'anteprima. Quado è l'attore.
regola-progetti-sviluppo-metodo = Quado sposta la cartella nel Cestino se lo scegli, dopo l'anteprima.
# Alternativa per Rust, sopra il comando cargo clean.
regola-progetti-sviluppo-alternativa-1 = Per una cartella target di Rust: esegui cargo clean nella cartella del progetto.
# Condizione da dire sempre (decisioni §3).
regola-progetti-sviluppo-rischio-1 = Si ricostruisce solo se il progetto si installa o si compila ancora.
# Da fare prima dell'anteprima: un editor o un terminale aperto sulla cartella blocca lo spostamento.
regola-progetti-sviluppo-rischio-2 = Chiudi l'editor e i terminali del progetto prima dell'anteprima: se usano la cartella, il controllo dei file in uso blocca lo spostamento.
# Nel risultato dello spostamento, prima di «Svuota il Cestino…»: cosa provare.
regola-progetti-sviluppo-prova = Prima di svuotare il Cestino controlla che il progetto si installi o si compili ancora.

# Titolo della regola sui programmi registrati.
regola-programmi-registrati-nome = Programmi installati
# Riga sotto la cartella di un programma. $nome dalla voce di disinstallazione.
regola-programmi-registrati-riga = Programma installato: { $nome }
# Metodo: disinstallazione ufficiale (M4), sopra l'indirizzo ms-settings:appsfeatures.
regola-programmi-registrati-metodo = Disinstalla da Impostazioni › App.

# Titolo della regola sui dati delle app MSIX.
regola-pacchetti-msix-nome = Dati delle app di Microsoft Store
# Riga sotto Packages\<nome>: cosa fa Windows, secondo la documentazione di Microsoft.
regola-pacchetti-msix-riga = Dati dell'app { $nome }: Windows li elimina quando disinstalli l'app.
# Metodo, sopra l'indirizzo ms-settings:appsfeatures.
regola-pacchetti-msix-metodo = Disinstalla da Impostazioni › App.

# Titolo della regola sulle cartelle dell'utente.
regola-cartelle-utente-nome = Cartelle dell'utente
# Riga sotto Documenti, Download, Desktop, Immagini, Video, Musica.
regola-cartelle-utente-riga = Tuoi file: decidi tu cosa tenere.

# Titolo della regola sulla cartella di Windows.
regola-cartella-windows-nome = Cartella di Windows
# Riga sotto la cartella Windows: frammento nominale.
regola-cartella-windows-riga = File di sistema protetti da Windows

# Titolo della regola sull'installazione precedente di Windows (Windows.old).
regola-windows-old-nome = Installazione precedente di Windows
# Riga sotto Windows.old: cosa è e cosa fa Windows, secondo la fonte ufficiale.
regola-windows-old-riga = Windows di prima dell'ultimo aggiornamento: Windows la elimina da solo 10 giorni dopo l'aggiornamento.
# Procedura in Impostazioni, sopra l'indirizzo ms-settings:storagesense. Percorso di menu.
regola-windows-old-metodo = Impostazioni › Sistema › Archiviazione › File temporanei, casella della versione precedente di Windows
# Conseguenza scritta nella fonte ufficiale.
regola-windows-old-rischio-1 = Dopo l'eliminazione non puoi più tornare alla versione precedente di Windows.

# Titolo della regola sulle ipotesi dal nome.
regola-ipotesi-dal-nome-nome = Ipotesi dal nome
# Riga sotto una cartella con il nome di un editore (04 §6.8).
regola-ipotesi-dal-nome-riga = Ipotesi dal nome: { $nome }. Nessuna voce di installazione lo conferma.

## Controlli preventivi (03 §6.1). Si eseguono nella simulazione di M5 salvo dove detto. Ogni testo è la
## condizione che il controllo verifica; accanto compare l'esito ("superato", "non superato").

# Controllo dei file in uso (F-AC-8) su un campione: tutti gli eseguibili e le DLL, più i file modificati per ultimi.
controllo-restart-manager = Eseguibili, librerie e file modificati per ultimi non in uso, secondo Windows
# Riferimenti nel registro, nei collegamenti e nelle attività pianificate (F-AC-9).
controllo-riferimenti = Nessun riferimento nel registro, nei collegamenti di Start e del desktop o nelle attività pianificate
# Capacità del Cestino dell'unità (F-AC-7).
controllo-capacita-cestino = Il Cestino dell'unità può contenere la selezione
# Avviso se la batteria critica fa ibernare il portatile (decisioni §3).
controllo-batteria-critica = Con la batteria critica il portatile non si iberna
# Lockfile accanto alla cartella (decisioni §3).
controllo-lockfile = Il lockfile del progetto è accanto alla cartella

## Reversibilità, errore, verifica (03 §6.1): una frase ciascuno. Compaiono dopo le etichette
## «Reversibilità», «Se si ferma a metà» e «Verifica» (record-reversibilita, record-errore, record-verifica).

# Elementi spostati nel Cestino (decisioni §4, 04 §6.8), con il comando di Windows per ripristinarli. Mai
# "puoi ripristinare tutto".
reversibilita-cestino = Puoi ripristinare gli elementi dal Cestino finché non lo svuoti tu o Sensore memoria. Apri il Cestino, seleziona gli elementi e scegli «Ripristina gli elementi selezionati».
# Nessuna azione.
reversibilita-nessuna-azione = Niente da ripristinare.
# Cache e store.
reversibilita-cache = Il programma riscarica o ricrea i dati quando gli servono.
# Ibernazione: il comando per tornare indietro da ciascuna delle due opzioni.
reversibilita-ibernazione = Si torna indietro con powercfg /hibernate /type full dopo la riduzione, con powercfg /h on dopo la disattivazione.
# Svuotamento del Cestino.
reversibilita-svuotamento = Gli elementi eliminati dal Cestino non si possono più ripristinare.
# Installazione precedente di Windows, dopo la procedura in Impostazioni.
reversibilita-windows-old = Non si annulla: la versione precedente di Windows non si può più ripristinare.
# Procedure di Windows (file temporanei).
reversibilita-procedura-windows = Windows elimina i file senza passare dal Cestino: non si possono ripristinare.
# Disinstallazione.
reversibilita-disinstallazione = La disinstallazione non si annulla: per riavere il programma va reinstallato.
# Procedura in un'app (dispositivi virtuali Android).
reversibilita-procedura-app = L'app elimina i dati senza passare dal Cestino: non si possono ripristinare.
# Esito parziale di uno spostamento (04 §6.8).
errore-per-elemento = Gli elementi si spostano uno alla volta. Il riepilogo dice quali sono nel Cestino e quali sono rimasti al loro posto.
# Nessuna azione.
errore-nessuna-azione = Non si applica.
# Procedure e comandi esterni: Quado non vede la procedura.
errore-procedura-esterna = La procedura la esegue un altro programma e Quado non la vede: la misura successiva mostra cosa è cambiato.
# Comandi copiati.
errore-comando = Il comando lo esegui tu: se si ferma, leggi il messaggio nel terminale.
# Misure dopo uno spostamento (F-VE-2).
verifica-riscansione = Dopo l'intervento Quado misura di nuovo lo spazio libero e rilegge gli elementi toccati.
# Nessuna azione.
verifica-nessuna-azione = Non si applica.
# Dopo una procedura esterna (F-VE-2). Cita il pulsante «Ho eseguito la procedura» (azione-ho-eseguito).
verifica-dopo-procedura = Quando scegli «Ho eseguito la procedura», Quado misura di nuovo lo spazio libero e rilegge la cartella.

## Fatti prodotti dal motore (F-IN-1). Ogni fatto ha accanto il glifo della sua provenienza.

# Percorso indicato dalla documentazione della regola. $a numero di elementi.
fatto-percorso-documentato = Percorso indicato dalla fonte della regola
# Ibernazione attiva secondo le API di alimentazione. $a 1 sì, 0 no.
fatto-ibernazione-attiva = { $a ->
    [1] Ibernazione attiva
   *[other] Ibernazione non attiva
}
# Avvio rapido (HiberbootEnabled).
fatto-avvio-rapido = { $a ->
    [1] Avvio rapido attivo
   *[other] Avvio rapido non attivo
}
# Standby moderno (AoAc), dalle capacità di alimentazione: nessun valore.
fatto-standby-moderno = Standby moderno attivo
# Azione per batteria critica: 0 niente, 1 sospensione, 2 ibernazione, 3 arresto; altrimenti il codice letto.
fatto-batteria-critica = { $a ->
    [0] Batteria critica: nessuna azione
    [1] Batteria critica: sospensione
    [2] Batteria critica: ibernazione
    [3] Batteria critica: arresto
   *[other] Batteria critica: azione con codice { $a_testo }
}
# Impostazioni di alimentazione non leggibili.
fatto-alimentazione-non-letta = Impostazioni di alimentazione non lette
# Numero di cartelle di versione misurate.
fatto-cartelle-di-versione = { $a ->
    [one] { $a_testo } cartella di versione
   *[other] { $a_testo } cartelle di versione
}
# Versione in uso, dichiarata da una fonte (registro o ProductInfo.xml).
fatto-versione-in-uso = Versione in uso: { $a }
# Versioni precedenti, dedotto.
fatto-versioni-precedenti = { $a ->
    [one] { $a_testo } versione precedente
   *[other] { $a_testo } versioni precedenti
}
# Prova di gestibilità: queste cartelle non sono la versione in uso. $a la versione in uso.
fatto-non-versione-in-uso = Versione diversa da quella in uso ({ $a })
# Prova di gestibilità: versioni tenute.
fatto-versioni-tenute = { $a ->
    [one] Resta { $a_testo } versione: quella in uso
   *[other] Restano { $a_testo } versioni: quella in uso e la precedente
}
# Cestino misurato su disco.
fatto-cestino-su-disco = Su disco: { $a }
# Cestino secondo Windows: dimensioni ($a) e numero di elementi ($b).
fatto-cestino-secondo-windows = Secondo Windows: { $a } in { $b ->
    [one] un elemento
   *[other] { $b_testo } elementi
}
# Cestino secondo Windows quando Windows non dà il conteggio (glifo «non disponibile»): nessun valore.
fatto-cestino-secondo-windows-non-letto = Secondo Windows: conteggio non letto
# Sensore memoria svuota il Cestino dopo $a giorni (04 §6.8).
fatto-sensore-memoria-cestino = Sensore memoria elimina in automatico gli elementi che restano nel Cestino per più di { $a_testo } giorni.
# Capacità del Cestino dell'unità.
fatto-cestino-capacita = Capacità del Cestino: { $a }
# Cartelle orfane: numero e dimensioni.
fatto-cartelle-orfane = { $a ->
    [one] { $a_testo } cartella senza file indice, { $b }
   *[other] { $a_testo } cartelle senza file indice, { $b }
}
# Gioco di Steam dal manifest: nome e ID dell'app. $b è l'ID come lo scrive Steam, senza separatore delle migliaia.
fatto-gioco-steam = { $a } (Steam, ID app { $b })
# Dimensioni dichiarate dal programma (mai sommate al misurato, F-AP-3).
fatto-dimensione-dichiarata = Dimensioni dichiarate: { $a }
# LastPlayed di Steam (04 §6.8).
fatto-ultima-sessione-steam = Ultima sessione registrata da Steam: { $a }
# Steam senza LastPlayed.
fatto-uso-non-registrato = Nessuna sessione registrata da Steam
# Installazione di Epic dal manifest.
fatto-installazione-epic = { $a } (Epic)
# Epic non registra l'uso (04 §6.8).
fatto-epic-nessun-dato-uso = Epic non registra quando un gioco è stato usato l'ultima volta.
# Programma installato secondo la voce di disinstallazione: nome e versione.
fatto-programma-installato = Installato: { $a } { $b }
# Indizio di percorso dal comando di disinstallazione.
fatto-programma-indizio = Cartella del programma di disinstallazione di { $a } { $b }
# Ipotesi dal nome.
fatto-ipotesi-dal-nome = Stesso nome dell'editore { $a }
# Pacchetto MSIX.
fatto-pacchetto-msix = Pacchetto { $a }
# Percorso della cache di npm e da dove viene.
fatto-percorso-cache-npm = { $b ->
    [ambiente] Cache di npm indicata dalla variabile d'ambiente: { $a }
    [npmrc] Cache di npm indicata da .npmrc: { $a }
   *[other] Cache di npm nel percorso predefinito: { $a }
}
# Percorso dello store di pnpm e da dove viene.
fatto-percorso-store-pnpm = { $b ->
    [ambiente] Store di pnpm indicato dalla variabile d'ambiente: { $a }
    [npmrc] Store di pnpm indicato da .npmrc: { $a }
   *[other] Store di pnpm nel percorso predefinito: { $a }
}
# Cartella con il lockfile del suo tipo accanto: nome e lockfile. Il fatto è dedotto (glifo accanto).
fatto-cartella-ricostruibile = { $a } accanto a { $b }
# Lockfile presente.
fatto-lockfile-presente = { $a } presente accanto alla cartella
# Cartella nota di Windows.
fatto-cartella-nota = { $a ->
    [documenti] Cartella Documenti di Windows
    [download] Cartella Download di Windows
    [desktop] Cartella Desktop di Windows
    [immagini] Cartella Immagini di Windows
    [video] Cartella Video di Windows
    [musica] Cartella Musica di Windows
   *[other] Cartella nota di Windows
}
# Cartella delle bozze dichiarata da CapCut.
fatto-cartella-bozze = Cartella delle bozze secondo CapCut: { $a }

## M4: installatori (risorsa di versione, MSI, corrispondenza con i programmi installati, F-DL-2).
## Chiavi `fatto-installatore-*` e `funzione-installatore-*`.

# Titolo delle voci degli installatori nella vista Download (card interventi, elenco della vista).
funzione-installatore-nome = Installatori

# Nome e versione del prodotto letti dall'installatore (risorsa di versione dell'exe o tabella Property dell'MSI). $a nome, $b versione.
fatto-installatore-prodotto = Prodotto { $a }, versione { $b }
# Come sopra, quando il file indica il nome ma non la versione. $a nome.
fatto-installatore-prodotto-senza-versione = Prodotto { $a }, versione non indicata
# Come sopra, quando il file indica la versione ma non il nome del prodotto. $a versione.
fatto-installatore-versione-senza-nome = Versione { $a }, nome del prodotto non indicato
# Editore dichiarato dall'installatore (CompanyName dell'exe, Manufacturer dell'MSI). $a editore.
fatto-installatore-editore = Editore: { $a }
# ProductCode dell'MSI: il GUID che Windows Installer usa come nome della voce di disinstallazione. $a codice.
fatto-installatore-codice-prodotto = Codice di prodotto { $a }
# FileVersion dell'exe, solo quando è diversa dalla versione del prodotto. $a versione.
fatto-installatore-versione-file = Versione del file { $a }
# Il file non ha dati da confrontare: exe senza risorsa di versione, o MSI senza nome, versione e codice. Esito normale, non un errore. $a exe o msi.
fatto-installatore-senza-dati = { $a ->
    [msi] Il pacchetto non indica nome, versione né codice di prodotto
   *[other] Il file non contiene dati di versione
}
# Corrispondenza da fonte (04 §6.8): il ProductCode dell'MSI è il nome di una voce di disinstallazione, stessa versione. $a programma, $b versione installata.
fatto-installatore-stesso-codice = Stesso codice di prodotto di { $a } { $b }, installato
# Corrispondenza dedotta, affidabilità media (04 §6.8): nome e versione uguali a quelli di un programma installato. $a programma, $b versione installata.
fatto-installatore-stesso-nome = Nome e versione coincidono con { $a } { $b }, installato
# Stesso ProductCode di un programma installato, in un'altra versione (da fonte). $a programma, $b versione installata, $c versione dell'installatore.
fatto-installatore-codice-versione-diversa = Stesso codice di prodotto di { $a }, installato nella versione { $b }; questo installatore ha la versione { $c }
# Stesso nome di un programma installato, in un'altra versione (dedotto, affidabilità bassa: coincide solo il nome). $a, $b, $c come sopra.
fatto-installatore-nome-versione-diversa = Stesso nome di { $a }, installato nella versione { $b }; questo installatore ha la versione { $c }
# Stesso ProductCode, ma l'installatore o la voce di disinstallazione non indicano la versione. $a programma.
fatto-installatore-codice-versione-non-nota = Stesso codice di prodotto di { $a }, installato; versione non confrontabile perché una delle due non è indicata
# Stesso nome, ma l'installatore o la voce di disinstallazione non indicano la versione (dedotto, affidabilità bassa). $a programma.
fatto-installatore-nome-versione-non-nota = Stesso nome di { $a }, installato; versione non confrontabile perché una delle due non è indicata
# Nessuna voce di disinstallazione con lo stesso codice o nome. $a codice (MSI con ProductCode) o nome (un programma può avere un altro nome).
fatto-installatore-non-installato = { $a ->
    [codice] Nessun programma installato con questo codice di prodotto o con questo nome
   *[other] Nessun programma installato con questo nome (confronto per nome)
}
# Il confronto con i programmi installati non si può fare. $a senza-dati quando mancano nome e codice.
fatto-installatore-non-confrontabile = { $a ->
    [senza-dati] Senza nome né codice di prodotto: nessun confronto con i programmi installati
   *[other] Confronto con i programmi installati non disponibile
}

## M4: programmi, uso noto e possibili residui (F-AP). Chiavi `fatto-programma-*`, `fatto-uso-*`,
## `fatto-residuo-*` e `funzione-programma-*`, `funzione-possibile-residuo-*`.

# Titolo della funzione della vista Programmi (campo "Regola" della scheda, al posto del nome di una regola).
funzione-programma = Programmi installati
# Cosa fa la funzione, sotto il titolo nella scheda (F-AP-1, F-AP-2). "Prova del percorso" detta in parole semplici.
funzione-programma-descrizione = Funzione di Quado. L'elenco viene dalle voci di disinstallazione, dai pacchetti di Microsoft Store e dai manifest di Steam ed Epic. Lo spazio di un programma si conta solo quando una fonte ne dichiara la cartella.
# Metodo per i programmi Win32 (04 §6.7, §6.8): la UI apre Impostazioni e copia il nome negli appunti.
funzione-programma-metodo-impostazioni = Impostazioni › App. Quado copia il nome del programma negli appunti: incollalo nella casella di ricerca.
# Metodo per le app di Microsoft Store: la pagina dell'app in Impostazioni.
funzione-programma-metodo-msix = Impostazioni › App, nella pagina dell'app
# Metodo per i giochi di Steam (steam://uninstall/<appid>): Steam chiede conferma.
funzione-programma-metodo-steam = Disinstalla con Steam, che chiede conferma.
# Metodo per i prodotti di Epic: non c'è un URI, si rimanda alla libreria del launcher. Percorso di menu.
funzione-programma-metodo-epic = Libreria di Epic Games Launcher › i tre puntini accanto al titolo › Disinstalla
# Alternativa per i programmi Win32 (F-AP-5): il comando appwiz.cpl si mostra e si copia. Percorso di menu.
funzione-programma-alternativa-pannello = Pannello di controllo › Programmi e funzionalità
# Rischio di ogni disinstallazione: Quado non sa cosa toglie il programma di disinstallazione.
funzione-programma-rischio-1 = Il programma di disinstallazione decide cosa eliminare: può lasciare dati o eliminare anche file nelle cartelle del programma.
# Casella del controllo manuale prima di aprire l'interfaccia, quando nella cartella ci sono tuoi file (F-AP-5). Parla l'utente.
funzione-programma-controllo-tuoi-file = Ho copiato altrove i miei file dalla cartella del programma
# Ipotesi quando la cartella viene dal comando di disinstallazione (solo un indizio di percorso).
funzione-programma-ipotesi-indizio = Ipotesi: la cartella del programma di disinstallazione è la cartella del programma.
# Ipotesi quando i tuoi file sono nella cartella che contiene il programma e i suoi dati (CapCut).
funzione-programma-ipotesi-ambito = Ipotesi: la cartella che contiene il programma e i suoi dati appartiene al programma, e la disinstallazione può toglierla.
# Titolo della funzione dei possibili residui. "Residuo" solo come possibilità (04 §6.8).
funzione-possibile-residuo = Possibili residui
# Cosa fa la funzione (F-AP-6), con il criterio del confronto per nome.
funzione-possibile-residuo-descrizione = Funzione di Quado. Elenca le cartelle in Local, Roaming, LocalLow e ProgramData che nessun programma installato e nessuna regola spiegano. Il confronto è per nome. Una cartella è spiegata se ogni parola del suo nome compare nel nome o nell'editore di uno stesso programma.

# Editore dalla voce di disinstallazione.
fatto-programma-editore = Editore: { $a }
# Versione dichiarata dal manifest (Epic).
fatto-programma-versione = Versione: { $a }
# Cartella del programma con la sua prova (InstallLocation, manifest, pacchetto). $a percorso.
fatto-programma-cartella = Cartella del programma: { $a }
# Cartella ricavata dal comando di disinstallazione, mai eseguito (F-AP-2). $a percorso.
fatto-programma-cartella-indizio = Cartella del programma di disinstallazione, presa come cartella del programma: { $a }
# Percorso dichiarato che non può essere la cartella di un programma. $a percorso.
fatto-programma-cartella-non-plausibile = La cartella dichiarata { $a } è la radice dell'unità o una cartella di Windows o dell'utente: nessuno spazio attribuito.
# Nessuna prova del percorso (F-AP-2): testo della specifica.
fatto-programma-posizione-non-dichiarata = Posizione non dichiarata
# Nessuna prova del percorso e nessun dato dalle regole: 0 byte attribuiti, mai per somiglianza del nome.
fatto-programma-nessuno-spazio = Nessuno spazio attribuito: nessuna fonte dichiara la cartella del programma.
# Spazio misurato del programma (dimensione allocata). $a già scritto.
fatto-programma-su-disco = Dimensioni su disco: { $a }
# Cartella dichiarata ma non nella scansione o non leggibile: mai 0.
fatto-programma-non-misurato = Dimensioni su disco non misurate: la cartella non è nella scansione o non si può leggere.
# Cartella non leggibile ma con dati dalle regole (app di Microsoft Store in WindowsApps): il misurato è solo dei dati.
fatto-programma-solo-dati = La cartella del programma non è nella scansione o non si può leggere: le dimensioni comprendono solo i dati riconosciuti dalle regole.
# Più programmi dichiarano la stessa cartella (Unreal Engine e plugin di Epic). $a altri programmi.
fatto-programma-cartella-condivisa = { $a ->
    [one] Anche un altro programma dichiara questa cartella: lo spazio si conta una volta sola.
   *[other] Anche altri { $a_testo } programmi dichiarano questa cartella: lo spazio si conta una volta sola.
}
# Dati del programma riconosciuti dalle regole (dentro e fuori dalla cartella). $a byte, $b numero di elementi.
fatto-programma-dati-regole = { $b ->
    [one] Dati del programma riconosciuti da una regola: { $a }
   *[other] Dati del programma riconosciuti dalle regole: { $a } in { $b_testo } cartelle
}
# Avviso prima della disinstallazione (04 §6.8). $a programma, $b dimensioni, $c sottocartella.
fatto-programma-tuoi-file = Nella cartella di { $a } ci sono tuoi file ({ $b } in { $c }). Copiali altrove prima di disinstallare.
# InstallDate dal registro. $a data.
fatto-programma-data-installazione = Data di installazione: { $a }
# SystemComponent=1: nascosto per default, nessun metodo di disinstallazione.
fatto-programma-componente-di-sistema = Componente di sistema: Impostazioni › App non lo elenca.
# UserAssist con il tracciamento attivo (F-AP-4), con la copertura. $a data.
fatto-uso-userassist = Ultimo avvio da Esplora file o Start: { $a } (UserAssist). Gli avvii da riga di comando, da altri launcher e da attività pianificate non sono registrati.
# UserAssist attivo ma senza voci per gli eseguibili della cartella. Mai "mai usato".
fatto-uso-userassist-nessun-avvio = Nessun avvio da Esplora file o Start per questa cartella, secondo UserAssist. Gli avvii da riga di comando, da altri launcher e da attività pianificate non sono registrati.
# UserAssist attivo ma il programma non ha una cartella con cui abbinarlo.
fatto-uso-senza-cartella = Nessun dato d'uso: senza la cartella del programma gli avvii registrati da UserAssist non si possono abbinare.
# Tracciamento attivo ma UserAssist non letto.
fatto-uso-userassist-non-letto = UserAssist non letto: nessun dato d'uso.
# Start_TrackProgs = 0 (04 §6.8): nessun valore UserAssist, anche se la chiave ha dati vecchi.
fatto-uso-tracciamento-disattivato = Tracciamento degli avvii di Windows disattivato: nessun dato d'uso
# Start_TrackProgs assente, con un valore inatteso o non leggibile.
fatto-uso-tracciamento-non-letto = Impostazione del tracciamento degli avvii non letta: nessun dato d'uso.
# App di Microsoft Store: nessun dato d'uso (F-AP-4).
fatto-uso-msix-nessun-dato = Windows non dice quando un'app di Microsoft Store è stata usata l'ultima volta.
# Possibile residuo (04 §6.8). $a data dell'ultima modifica nella cartella.
fatto-residuo-possibile = Nessun programma installato corrispondente (confronto per nome). Ultima modifica: { $a }. Potrebbe essere un residuo.
# Possibile residuo senza data nella scansione.
fatto-residuo-possibile-senza-data = Nessun programma installato corrispondente (confronto per nome). Ultima modifica non letta. Potrebbe essere un residuo.

## M4: vista Download (F-DL-1, F-DL-3..6). Chiavi `fatto-lente-*` e `funzione-*` delle voci della vista.

# Titolo delle voci di file con le stesse dimensioni, confrontati o no (card interventi, elenco della vista Download).
funzione-stessa-dimensione-nome = File con le stesse dimensioni
# Titolo delle voci di copie con lo stesso contenuto, dopo il confronto (F-DL-4).
funzione-copie-identiche-nome = Copie identiche
# Metodo delle copie identiche: spostamento nel Cestino dopo l'anteprima (M5). Quado è l'attore.
funzione-copie-identiche-metodo = Quado sposta nel Cestino le copie che scegli, dopo l'anteprima. La copia da tenere resta al suo posto.
# Effetto da dire sempre: chi apre una copia dal suo percorso non la trova più.
funzione-copie-identiche-rischio-1 = Un collegamento o un programma che apre una copia dal suo percorso non la trova più. Lo stesso contenuto resta nella copia tenuta.
# Titolo delle voci di archivi zip con una cartella dello stesso nome accanto (F-DL-5).
funzione-archivio-con-cartella-nome = Archivi con una cartella dello stesso nome
# Titolo della voce delle immagini disco.
funzione-immagine-disco-nome = Immagini disco
# Titolo delle voci di download incompleti (F-DL-6).
funzione-download-incompleto-nome = Download incompleti
# Metodo dei download incompleti che nessun processo usa: spostamento nel Cestino dopo l'anteprima (M5).
funzione-download-incompleto-metodo = Quado sposta nel Cestino i download incompleti, dopo l'anteprima.
# Effetto da dire sempre.
funzione-download-incompleto-rischio-1 = Il browser non può più riprendere questi download: per avere il file, scaricalo di nuovo.

# Tipo secondo l'estensione (F-IN-3): primo fatto delle voci; per un installatore letto viene dopo l'esito della lettura. $a codice della famiglia della lente, $b numero di file.
fatto-lente-famiglia = { $a ->
    [installatori] { $b ->
        [one] Un installatore
       *[other] { $b_testo } installatori
    }
    [immagini-disco] { $b ->
        [one] Un'immagine disco
       *[other] { $b_testo } immagini disco
    }
    [download-incompleti] { $b ->
        [one] Un download incompleto
       *[other] { $b_testo } download incompleti
    }
   *[other] { $b ->
        [one] Un file
       *[other] { $b_testo } file
    }
}, secondo l'estensione
# File con le stesse dimensioni logiche, dalla scansione. $a numero di file, $b dimensioni di ciascuno.
fatto-lente-stessa-dimensione = { $a ->
    [one] Un file da { $b }
   *[other] { $a_testo } file da { $b } ciascuno
}
# Contenuto non ancora confrontato (03 §6.1): non disponibile finché non si chiede il confronto.
fatto-lente-contenuto-non-confrontato = Contenuto non confrontato
# Confronto annullato a metà (06 §3.2): nessun risultato parziale. $a file letti, $b file del gruppo.
fatto-lente-letti-in-parte = Confronto non completato: { $a_testo } file letti su { $b_testo }
# File che hanno cambiato dimensioni tra la scansione e il confronto. $a numero di file.
fatto-lente-dimensione-cambiata = { $a ->
    [one] Le dimensioni di un file sono cambiate dopo la scansione: per confrontarlo serve una nuova scansione della cartella
   *[other] Le dimensioni di { $a_testo } file sono cambiate dopo la scansione: per confrontarli serve una nuova scansione della cartella
}
# Prova di gestibilità (F-DL-4): stessa impronta SHA-256. $a numero di file, $b dimensioni di ciascuno.
fatto-lente-contenuto-identico = Stesso contenuto (impronta SHA-256) in { $a_testo } file da { $b }
# Prova di gestibilità: la copia che resta, con la sua data di creazione.
fatto-lente-copia-tenuta = Resta la copia creata il { $a }
# Contenuto confrontato: nessuna copia. $a numero di file della voce.
fatto-lente-contenuto-diverso = { $a ->
    [one] Contenuto confrontato: nessun altro file con le stesse dimensioni ha questo contenuto
   *[other] Contenuto confrontato: i { $a_testo } file hanno contenuti diversi tra loro
}
# Installatori non ancora letti (F-DL-2): nome e versione si leggono su richiesta.
fatto-lente-installatori-non-letti = Dati di versione non letti
# Segnaposto cloud (05 D6): il contenuto non si legge, per non scaricarlo.
fatto-lente-solo-cloud = Solo nel cloud: non letto, per non scaricarlo
# Dati di versione non letti per un problema (06 §2.3), forma degli errori «Impossibile + infinito». $a codice del motivo; $b codice di errore di Windows o nome del campo.
fatto-lente-estrattore-non-riuscito = Impossibile leggere i dati di versione: { $a ->
    [non-presente] il file non c'è più
    [accesso-negato] Windows nega l'accesso
    [errore] errore di Windows (Codice { $b })
    [solo-cloud] il file è solo nel cloud
    [troncato] il file finisce a metà
    [inatteso] formato inatteso
    [vuoto] il file è vuoto
    [troppo-annidato] troppi livelli uno dentro l'altro
    [codifica] codifica dei caratteri non valida
    [troppo-grande] il file supera il limite di lettura
    [campo-mancante] manca il campo { $b }
    [tipo-diverso] il campo { $b } ha un tipo inatteso
    [valore] il campo { $b } ha un valore inatteso
   *[other] { $a }
}
# Zip con una cartella dello stesso nome accanto (F-DL-5).
fatto-lente-cartella-accanto = Accanto c'è una cartella con lo stesso nome dell'archivio
# Prova misurata (04 §6.6): file dell'indice presenti nella cartella con le stesse dimensioni. $a presenti, $b file dell'indice.
fatto-lente-voci-presenti = { $a ->
    [one] { $a_testo } file su { $b_testo } dell'archivio è nella cartella, con le stesse dimensioni
   *[other] { $a_testo } file su { $b_testo } dell'archivio sono nella cartella, con le stesse dimensioni
}
# File dell'archivio trovati nella cartella con dimensioni diverse. $a numero di file.
fatto-lente-voci-dimensione-diversa = { $a ->
    [one] Un file dell'archivio è nella cartella con dimensioni diverse
   *[other] { $a_testo } file dell'archivio sono nella cartella con dimensioni diverse
}
# File dell'archivio che non sono nella cartella, compresi i percorsi non validi. $a numero di file.
fatto-lente-voci-mancanti = { $a ->
    [one] Un file dell'archivio non è nella cartella
   *[other] { $a_testo } file dell'archivio non sono nella cartella
}
# File dell'archivio che cadono in cartelle non leggibili: né presenti né mancanti. $a numero di file.
fatto-lente-voci-non-lette = { $a ->
    [one] Un file dell'archivio sta in una cartella che Quado non ha potuto leggere
   *[other] { $a_testo } file dell'archivio stanno in cartelle che Quado non ha potuto leggere
}
# Indice dello zip non ancora letto: si legge su richiesta.
fatto-lente-indice-non-letto = Indice dell'archivio non letto
# Indice dello zip non leggibile (06 §3.3). $a codice del motivo; $b codice di errore di Windows o nome del campo.
fatto-lente-archivio-non-leggibile = Impossibile leggere l'indice dell'archivio: { $a ->
    [non-presente] il file non c'è più
    [accesso-negato] Windows nega l'accesso
    [errore] errore di Windows (Codice { $b })
    [solo-cloud] il file è solo nel cloud
    [troncato] il file finisce a metà
    [inatteso] formato inatteso
    [vuoto] il file è vuoto
    [troppo-annidato] troppi livelli uno dentro l'altro
    [codifica] codifica dei caratteri non valida
    [troppo-grande] il file supera il limite di lettura
    [campo-mancante] manca il campo { $b }
    [tipo-diverso] il campo { $b } ha un tipo inatteso
    [valore] il campo { $b } ha un valore inatteso
   *[other] { $a }
}
# Segnaposto cloud tra i file della voce, dalla scansione. $a numero di file.
fatto-lente-segnaposto = { $a ->
    [one] Un file è solo nel cloud: su questa unità occupa solo i byte locali
   *[other] { $a_testo } file sono solo nel cloud: su questa unità occupano solo i byte locali
}
# Prova di gestibilità (F-DL-6): il controllo dei file in uso non trova programmi che usano i file. $a numero di file, $b data del controllo, $b_ora la sua ora.
fatto-lente-nessun-processo = { $a ->
    [one] Nessun programma lo usa, secondo Windows (controllo del { $b } alle { $b_ora })
   *[other] Nessun programma li usa, secondo Windows (controllo del { $b } alle { $b_ora })
}
# Download incompleto in uso secondo il controllo dei file in uso. $a data del controllo, $a_ora la sua ora.
fatto-lente-in-uso = In uso da un programma, secondo Windows (controllo del { $a } alle { $a_ora })
# Processo che usa il file, con il nome che riporta il controllo dei file in uso. $a nome del processo.
fatto-lente-processo = Lo usa { $a }
# Il controllo dei file in uso non ha risposto: l'uso non è noto e non vale come "non in uso". $a codice di errore, $b data, $b_ora ora.
fatto-lente-uso-non-letto = Uso non noto: il controllo del { $b } alle { $b_ora } non è riuscito (Codice { $a })
# Uso non ancora controllato: il controllo dei file in uso si interroga su richiesta.
fatto-lente-uso-non-controllato = Uso non controllato
