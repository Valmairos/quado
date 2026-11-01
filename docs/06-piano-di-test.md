# 06 · Piano di test (v2, dopo la revisione)

Stato: v2 del 12/10/2026, secondo `revisione/decisioni-revisione.md` (§5, §6, §9, §9 bis, §9 ter). La prima versione dei documenti non è inclusa.

Principi:
- Un requisito è chiuso quando è verificato nella forma in cui la difesa esiste: un controllo sul Cestino si prova con un Cestino vero e piccolo su un VHDX, non con un mock. La logica si può provare con dati simulati; la difesa si prova sul sistema vero.
- Ogni requisito F-* di `revisione/decisioni-revisione.md` §9 ter ha almeno un test: la tabella di copertura è al §13.
- I criteri che dipendono dallo stato del PC di sviluppo (numero di voci, cartelle presenti) si verificano su fixture versionate. I confronti con il PC reale sono controlli manuali con data, non test di regressione.
- Le misure citate qui sono state prese sul PC di sviluppo, in sola lettura, con la data.

## Dove si eseguono i test

| Livello | Dove | Privilegi |
|---|---|---|
| Unità e proprietà | `cargo test` sul PC di sviluppo | Nessuno |
| Alberi di prova | Cartella temporanea su NTFS di C: | Nessuno; i symlink richiedono la Modalità sviluppatore |
| VHDX locali | Volumi virtuali creati, montati e smontati da `cargo xtask vhdx` | Amministratore solo per creare, montare e smontare: una persona davanti al PC approva il prompt UAC a ogni sessione. I test girano poi non elevati, tranne quelli dell'helper |
| Interfaccia | `egui_kittest` offscreen | Nessuno |
| Controlli manuali | Process Monitor, ETW, confronto con Esplora file e Impostazioni | Nessuno, salvo dove indicato |

Non c'è una CI privilegiata multi file system (C10): per un PC solo costava senza un beneficio proporzionato. Con il parser MFT escono anche il fuzzing della MFT e gli oracoli `ntfs` e `mft` (05 D4).

## 1. Accuratezza della scansione

### 1.1 Oracolo indipendente

Lo scanner ha un secondo percorso di misura, scritto a parte e usato solo nei test. Apre ogni file con soli attributi, legge `FileStandardInfo` (allocata, logica, numero di link) e `FileIdInfo` (identità) e somma con le stesse regole. Lo scanner enumera le cartelle, l'oracolo apre i file uno per uno: sugli alberi di prova le due strade devono coincidere al byte.

Controllo più grossolano con strumenti di Windows: `Get-ChildItem` per le dimensioni logiche, `compact /q` per i file compressi, `fsutil hardlink list` per gli hard link.

La stima degli interventi rilegge gli attributi come l'oracolo; per questo la sua verità di riferimento non è l'oracolo ma la variazione di spazio libero su VHDX (§6.1).

### 1.2 Alberi di prova

Generati da `xtask` in una cartella temporanea su NTFS, senza privilegi salvo dove indicato.

| Caso | Come si crea | Cosa si verifica | Requisiti |
|---|---|---|---|
| Molti file piccoli | 1-3 milioni di file da 0-4 KiB | Velocità e memoria; i file residenti nella MFT hanno allocata pari alla logica arrotondata a 8 byte, come in 02 §3 (C6) | F-SC-2, F-SC-8 |
| File sparse enormi | 10 file da 10-50 GB sparse | Logica contro allocata | F-SC-8 |
| Hard link | `CreateHardLinkW`, anche tra cartelle lontane | Conteggio unico, nota "già contati una volta", attribuzione dichiarata | F-SC-7 |
| Junction che crea un ciclo | Mount point verso un antenato | Nessun ciclo, junction indicata | F-SC-7 |
| Symlink | `CreateSymbolicLinkW` (Modalità sviluppatore) | Non seguito, indicato | F-SC-7 |
| Compressione NTFS | `FSCTL_SET_COMPRESSION` | Allocata minore della logica | F-SC-8 |
| Compressione WOF | `compact /c /exe:lzx` | Allocata corretta con `FileStandardInfo` (02 §3) | F-SC-8 |
| Flussi alternativi | Scrittura su `file:stream` | Non contati e dichiarati; lo scarto resta nel non attribuito | F-SC-8, F-OV-1 |
| Percorsi lunghi | Oltre 260 e fino a circa 32.000 caratteri | Scansione, "Apri in Esplora file", "Copia percorso", "Proprietà" | F-SC-2, F-IN-5 |
| Nomi non validi in UTF-16 | Surrogati spaiati via `CreateFileW` | Nome mostrato in forma sicura, percorso conservato esatto | F-SC-2 |
| Cartella negata | ACL di negazione sulla propria cartella | Elencata con il motivo, spazio nel non attribuito | F-SC-6 |
| File in uso | Handle aperto con condivisione esclusiva | Misurato lo stesso dalla voce di cartella | F-SC-2 |
| Dimensioni pigre | File tenuto aperto in scrittura mentre cresce | La voce di cartella è vecchia; "Verifica questa voce" la corregge senza leggere il contenuto | F-IN-6 |
| Modifiche durante la scansione | Thread che crea e cancella file | Nessun crash; deriva dell'usato mostrata come incertezza | F-SC-2, F-OV-1 |
| Profondità | 2.000 livelli di cartelle | Nessun overflow (pila esplicita) | F-SC-2 |
| Cartella larghissima | 1 milione di file in una cartella | Elenco virtualizzato entro i tempi di frame del §9 | F-EX-2 |
| Segnaposto cloud | Sync root registrata con la Cloud Filter API e un provider di prova | Due valori ("nel cloud", "su questo disco"); nessuna idratazione e nessun elenco scaricato, verificato dal provider | F-SC-9 |
| Annullamento | Annullare a metà scansione | Arresto entro 500 ms; risultato parziale con il banner "Scansione interrotta" | F-SC-5 |

### 1.3 VHDX locali e file system

- `cargo xtask vhdx` crea con `diskpart` VHDX espandibili, li formatta e li monta (amministratore, con il consenso di chi esegue la prova). Su ogni volume genera l'albero, scansiona e confronta con l'oracolo.
- File system provati: NTFS (caso principale), exFAT e FAT32 come unità rimovibili (F-SC-10). Gli altri sono "senza garanzie": non si provano, si verifica solo che la UI lo dichiari.
- Se l'utente usa unità esterne, si aggiunge una prova in sola lettura sulle sue unità reali, con il file system che hanno davvero (07 §8).
- Smontare e rimontare un VHDX svuota la cache del file system per quel volume: è il modo ripetibile di misurare la scansione a freddo. Su C: la misura a freddo è la prima scansione dopo un riavvio (§9).
- Elenco dei volumi (F-SC-1): confronto con `Get-Volume` sul PC di sviluppo, VHDX montati compresi.

### 1.4 Bilancio del volume e Panoramica

Definizione del bilancio in 03 §6.4.

| Test | Dove | Criterio | Requisiti |
|---|---|---|---|
| Residuo su volume fermo | VHDX NTFS senza processi che vi scrivono | Non attribuito ≤ metafile attesi + tolleranza. Metafile attesi e tolleranza si fissano in M1 da tre misure e si scrivono nel test | F-OV-1 |
| Errore iniettato | Stesso VHDX, build di test che non deduplica gli hard link | Il residuo si sposta esattamente dei byte degli hard link: prova che il residuo non è calcolato per costruzione (V1) | F-OV-1 |
| Deriva | C: reale con attività normale | Usato a inizio e fine scansione registrati; la differenza compare come incertezza | F-OV-1 |
| Bilancio per gestore | Fixture classificate | Somme per gestore coerenti con la definizione; colore solo da fatti "da fonte" o "dedotto alta" (§2.2) | F-OV-1 |
| Top list | Alberi sintetici con antenati e discendenti grandi | Soli elementi; nessun elemento insieme a un suo antenato già elencato, salvo spiegazione diversa | F-OV-2 |
| Card dei conti | Fixture con cartelle negate, cestini di altri SID, profili altrui | Due liste, "Osservato" e "Possibile, non misurabile senza amministratore"; nessuna frase causale (controllo del testo); hard link come nota, non come riga da sottrarre | F-OV-3 |
| Card interventi | Record di fixture | Solo voci nate da record di raccomandazione; nessuna voce da sole euristiche (§2.2) | F-OV-4 |

### 1.5 Helper con privilegi (F-SC-3)

Su un VHDX NTFS preparato, con il consenso di chi esegue la prova:
- contenuto: una cartella con ACL di negazione, un profilo finto di un altro utente (cartella in `Users` con l'ACL di un altro SID), cartelle in `$Recycle.Bin` con SID diversi, uno dei quali non corrisponde a nessun account;
- l'helper enumera le cartelle negate e i totali coincidono con l'oracolo eseguito come amministratore;
- i profili altrui arrivano solo come totale e numero di file: la traccia dei messaggi sulla pipe non contiene nomi;
- i cestini di altri SID arrivano come totale per SID, con il SID orfano segnato;
- copie shadow e archiviazione riservata: su C: una lettura reale, con consenso; sul VHDX la risposta "non disponibile" senza errori.

I test di sicurezza dell'helper sono al §11.

### 1.6 Confronto con altri strumenti (facoltativo)

Solo se si decide di scaricarli: WizTree e WinDirStat sullo stesso volume e nella stessa sessione. Si registrano i totali e si spiega ogni differenza (hard link, flussi alternativi, aree non leggibili, unità). Non si pubblica nulla.

## 2. Raccomandazioni e regole

### 2.1 Tabella attesa per regola (V5)

Ogni regola ha una tabella: ingresso (fixture), stato atteso, azione, metodo preferito e alternative, controlli preventivi, stato della stima, privilegi. Il test confronta il record prodotto con la tabella campo per campo. I casi già decisi in `revisione/decisioni-revisione.md` §3 sono il minimo, per ciò che è presente sul PC di sviluppo: ogni regola richiede una fixture reale (07 §7), quindi Docker e WSL, che il PC di sviluppo non usa, entrano solo quando ce n'è una.

| Caso | Stato atteso | Azione e metodo | Controlli, stima, testi |
|---|---|---|---|
| CapCut, versioni precedenti | Da verificare | Spostamento nel Cestino delle versioni scelte, con l'eccezione a F-AC-4 dichiarata dalla regola | Restart Manager su un campione e sugli eseguibili; riferimenti in registro, `.lnk`, attività pianificate; si tengono le ultime due versioni e la stima le esclude; testo "(dedotto)", mai "non più in uso"; conferma con "Prova CapCut prima di svuotare il Cestino" |
| CapCut, `User Data\Projects` | Decidi tu, gestore "tu" | Nessuna proposta automatica | - |
| Cestino con `$R` orfani | Lo gestisce Windows, variante "Nessuna procedura: non toccare": anomalia spiegata (F-SY-2), nessun intervento | Nessuna cifra recuperabile nella card interventi | Riga della top list limitata al misurato: "N GB su disco, M MB secondo Windows" |
| Ibernazione | Decidi tu | `powercfg /hibernate /type reduced` e `powercfg /h off` affiancati, mostrati e copiati | Amministratore; avviso se l'azione per batteria critica è l'ibernazione |
| Cache npm | Lo gestisce npm | `npm cache verify`; alternativa `npm cache clean --force` con le conseguenze | Percorso dalla configurazione, mai eseguendo npm; con `verify` stima non stimabile |
| Store pnpm | Lo gestisce pnpm | `pnpm store prune` | Stima "non stimabile senza contare i link" |
| Docker, WSL (solo con una fixture reale; il PC di sviluppo non li usa, 07 §7) | Lo gestisce Docker / WSL | Comandi documentati; compattazione con le sue condizioni | Stima su C: 0 fino alla compattazione; `--volumes` mai mostrato |
| WinSxS | Lo gestisce Windows | `StartComponentCleanup` senza `/ResetBase`; `/ResetBase` solo come alternativa con la conseguenza scritta | Amministratore |
| SoftwareDistribution, Temp, miniature | Lo gestisce Windows | Impostazioni › File temporanei | Mai a mano |
| Windows\Installer | Lo gestisce Windows, variante "Nessuna procedura: non toccare" | Nessuna | - |
| node_modules, target, .next, build | Decidi tu | Spostamento nel Cestino su scelta esplicita; `cargo clean` per `target` | Lockfile presente |
| Copie identiche in Download | Decidi tu prima dell'hash, Gestibile dopo | Spostamento nel Cestino delle copie, si tiene il più vecchio | Hash su richiesta |
| Download incompleti non in uso | Gestibile | Spostamento nel Cestino | Restart Manager |
| Programma grande | Decidi tu | Disinstallazione dall'interfaccia ufficiale | Avviso se la regola indica tuoi file nella cartella |
| Residuo | Non sappiamo | Solo "Apri in Esplora file" | Data di ultima modifica, "Potrebbe essere un residuo" |
| Cartella senza regola | Non sappiamo | Solo misure e "Apri in Esplora file" | - |

### 2.2 Test di proprietà sul modello

- Gestibile e Da verificare esistono solo con una prova di gestibilità distinta dalla prova di identità. Il costruttore lo impone; il test lo verifica anche su record generati dal motore con ingressi casuali.
- Le euristiche non producono proposte: un elemento classificato solo da dimensione, età, nome o estensione non ottiene mai Gestibile o Da verificare e non compare nella card interventi.
- Ogni record ha tutti i campi obbligatori di 03 §6.1, non vuoti.
- Ogni fatto ha la sua provenienza; un fatto dedotto ha l'affidabilità calcolata con i criteri di 03 §6.2.
- Il colore del gestore viene solo da fatti "da fonte" o "dedotto alta"; negli altri casi grigio con "ipotesi".
- Versione fuori intervallo: affidabilità bassa, niente Gestibile né Da verificare.
- Due regole in conflitto: si mostrano entrambe, con l'affidabilità più bassa, e nessuna proposta.
- Base delle regole (RS10): ogni regola ha almeno una fonte con URL ed estratto, una fixture, una data di verifica e una tabella attesa; altrimenti il test fallisce.
- Regole da riverificare: con una data simulata oltre 6, 11 e 12 mesi compaiono il banner, l'annuncio e il cambio di stato dichiarato descritti in 07 §5.1, mai un cambio senza testo.

### 2.3 Fixture ed estrattori

- Fixture reali del PC di sviluppo, anonimizzate e versionate: `ProductInfo.xml` di CapCut, voci di disinstallazione con e senza `InstallLocation`, `libraryfolders.vdf`, `appmanifest_*.acf`, `localconfig.vdf`, manifest Epic `.item`, elenco MSIX, valori UserAssist e `Start_TrackProgs`, chiavi BitBucket e di Sensore memoria.
- Gli estrattori di registro leggono da una sorgente astratta: i test automatici usano una sorgente simulata, non il registro vero.
- Per ogni estrattore: caso valido, caso troncato, formato cambiato. Un formato inatteso produce "estrattore non riuscito" con il motivo; il test fallisce se l'estrattore restituisce un dato al posto dell'errore.
- Lettori di formati non fidati (VDF, JSON, XML, directory centrale degli zip): ingressi corrotti e casuali generati con `proptest`; nessun panic e nessuna lettura oltre i limiti di dimensione.

### 2.4 Configurazione in sola lettura (F-SY-3)

| Lettura | Test |
|---|---|
| Sensore memoria (attivo, frequenza, giorni per Cestino e Download) | Fixture per ogni combinazione; sul PC di sviluppo confronto manuale con Impostazioni › Archiviazione › Sensore memoria |
| `MaxCapacity` e `NukeOnDelete` per volume | Fixture; sul PC di sviluppo e su un VHDX confronto con le Proprietà del Cestino |
| Ibernazione, avvio rapido, azione per batteria critica | Confronto con `powercfg /a` e con le Opzioni risparmio energia, eseguiti a mano |
| Tracciamento degli avvii (`Start_TrackProgs`) | Fixture con 0, 1 e valore assente |
| Ripristino configurazione | Lettura senza privilegi se possibile; altrimenti "non disponibile" con il motivo |
| Nessuna scrittura | Process Monitor: nessuna chiave aperta in scrittura durante tutte le letture |

## 3. Lente Download (F-DL)

### 3.1 Famiglie e filtri

- Famiglie per estensione su `Downloads` e su un'altra cartella dell'utente scelta (F-DL-1): fixture con ogni famiglia (installatori, archivi, immagini disco, video e audio, documenti e immagini, download incompleti).
- Suffisso "(N)" (F-DL-3): fixture con `setup (1).exe` di dimensione diversa da `setup.exe`, e con due file di nome diverso e contenuto identico. "(N)" è solo un filtro: non raggruppa e non cambia lo stato; i gruppi nascono solo dall'hash.

### 3.2 Copie identiche (F-DL-4)

- Nessuna lettura di contenuto prima che l'utente chieda il confronto: Process Monitor non mostra `ReadFile` sui file della cartella durante scansione, apertura della lente e ispettore.
- Hash solo tra file della stessa dimensione: i file di dimensione unica non si leggono.
- Mai sui segnaposto cloud: nella sync root di prova (§1.2) il comando è disattivato con il motivo e il provider non riceve richieste.
- Dopo il confronto le copie sono Gestibile e si tiene il più vecchio per data di creazione; l'utente può sceglierne un altro.
- Annullamento a metà: nessun gruppo parziale presentato come completo.

### 3.3 Archivio e cartella estratta (F-DL-5)

- Fixture: zip con cartella omonima completa, parziale, con dimensioni diverse, con sottocartelle; zip64; zip corrotto.
- "N voci su M presenti" corretto in ogni caso; zip corrotto: "non leggibile", nessun crash.
- Si legge solo la directory centrale: nessuna decompressione (Process Monitor: letture solo in coda al file).
- Stato Decidi tu, con la prova misurata.

### 3.4 Download incompleti (F-DL-6)

- `.crdownload` e `.part` chiusi: Gestibile.
- Gli stessi file tenuti aperti da un processo di prova: bloccati con il nome del processo (Restart Manager).

### 3.5 Metadati degli installatori (F-DL-2)

- Risorsa di versione PE: fixture con e senza risorsa, e con risorsa malformata.
- MSI: `MsiOpenDatabaseW` in sola lettura; hash e date del file uguali prima e dopo.
- Corrispondenza: ProductCode uguale alla chiave di disinstallazione vale "da fonte"; nome e versione uguali per un exe valgono "dedotto", con l'affidabilità di 03 §6.2. Tre esiti: stessa versione, versione diversa, non installato.
- Background e cache: la UI resta entro i tempi di frame del §9 durante la lettura; una seconda richiesta sugli stessi file non li rilegge (Process Monitor); se cambia dimensione o data di un file, si rilegge solo quello.

### 3.6 Effetto sull'ultimo accesso

Su un VHDX NTFS con l'aggiornamento dell'ultimo accesso attivo:
- scansione, ispettore e "Verifica questa voce" non cambiano la data di ultimo accesso;
- risorsa di versione, MSI, zip e hash la cambiano, come previsto e dichiarato;
- prima di ogni lettura di contenuto la UI mostra "aggiorna la data di ultimo accesso di questi file" (snapshot);
- nessun handle in scrittura viene aperto per evitarlo (§11).

## 4. Programmi (F-AP)

| Test | Criterio | Requisiti |
|---|---|---|
| Elenco | Da fixture di registro (HKLM, WOW6432Node, HKCU), MSIX, Steam ed Epic: ogni voce con `DisplayName` considerata, `SystemComponent=1` nascoste per default; sul PC di sviluppo confronto con i conteggi di 02 §10 | F-AP-1 |
| Spazio | Senza prova di percorso: "posizione non dichiarata" e nessuno spazio per somiglianza di nome; un byte ha un solo proprietario (prefisso più lungo, caso di Unreal Engine e dei plugin Epic) | F-AP-2 |
| Dichiarato e misurato | `EstimatedSize` e `SizeOnDisk` sempre etichettati con la loro fonte, accanto al misurato | F-AP-3 |
| Uso noto | LastPlayed da `localconfig.vdf` e da `.acf` concordi sulle fixture; UserAssist letto solo con `Start_TrackProgs` = 1; con 0 compare "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso" e nessun valore UserAssist, anche se la chiave contiene dati vecchi (caso del PC di sviluppo); copertura delle fonti dichiarata | F-AP-4 |
| Parole vietate | Nessuna stringa del catalogo e nessun record contiene "inutilizzato", "mai usato", "non usato da", "sicuro da disinstallare" | F-AP-4 |
| Disinstallazione | Win32: apre `ms-settings:appsfeatures` e copia il nome negli appunti; MSIX: `ms-settings:appsfeatures-app?<PFN>`; Steam: `steam://uninstall/<appid>`; Epic: il launcher. Il lanciatore accetta solo valori tipizzati: un test di compilazione mostra che non si costruisce da una stringa del registro | F-AP-5 |
| Avviso tuoi file | Programma con una regola che indica tuoi file nella cartella (fixture CapCut): la UI chiede di copiarli prima | F-AP-5 |
| Residui | Fixture con cartelle rivendicate e non: le non rivendicate hanno stato Non sappiamo, data di ultima modifica e "Potrebbe essere un residuo"; test di proprietà: nessun residuo riceve un'azione diversa da "Apri in Esplora file" | F-AP-6 |

## 5. Interventi e protezione

### 5.1 Piano confermato come tipo (V6)

- Test di compilazione (`trybuild`): fuori dal modulo di conferma un `PianoConfermato` non si costruisce; l'API di esecuzione non accetta altri tipi; un piano già eseguito non si riusa.
- Test di architettura: le chiamate che modificano file o avviano procedure (`IFileOperation`, `MoveFileEx`, `DeleteFile`, `RemoveDirectory`, `SetFileInformationByHandle` con disposizione o rinomina, `CreateFile` con accesso in scrittura, `ShellExecuteExW`, `SHEmptyRecycleBinW`) compaiono solo in `quado-actions`, più `ShellExecuteExW` con `runas` nell'avvio dell'helper.
- Esc, Annulla e la X non eseguono mai l'azione (test UI).

### 5.2 Selezioni casuali (F-AC-4, F-AC-2)

`proptest` genera selezioni casuali su alberi di prova che includono elementi protetti (cartella Windows, WinSxS, Installer, System Volume Information, file di paging e ibernazione, cartelle di programmi registrati):
- ogni selezione con un elemento protetto è respinta dalla costruzione del piano;
- l'unica eccezione, quella di F-AC-4 via regola Da verificare, si prova a parte: un caso positivo con tutti i controlli soddisfatti e un caso negativo per ciascun controllo mancante;
- elementi cambiati tra simulazione ed esecuzione (dimensione, data, ID) si saltano e compaiono nel riepilogo;
- file sincronizzati (F-AC-2, R10): nella sync root di prova (§1.2) una selezione con un file sincronizzato mostra il controllo manuale "spostarlo nel Cestino lo elimina anche nel cloud"; senza la spunta la costruzione del piano la respinge, e il provider non riceve richieste di dati.

### 5.3 Cestino, su VHDX con Cestino piccolo

| Test | Criterio | Requisiti |
|---|---|---|
| Capacità superata | `MaxCapacity` del volume ridotta (dalle Proprietà del Cestino o dall'harness, ripristinata a fine test); una selezione che porterebbe oltre la capacità è bloccata prima di `IFileOperation`; gli elementi più vecchi già nel Cestino restano | F-AC-7 |
| Elemento più grande del Cestino | In `PreDeleteItem` manca `TSF_DELETE_RECYCLE_IF_POSSIBLE`: nessuna eliminazione definitiva, elemento saltato e riportato | F-AC-5 |
| `NukeOnDelete` e unità senza Cestino | Comando disattivato con la spiegazione | F-AC-1, F-AC-7 |
| Sensore memoria attivo | La conferma riporta i giorni letti | F-AC-7 |
| Cestino misurato in due modi | Un `$R` senza `$I` creato sul VHDX: "su disco" lo conta, "secondo Windows" no; nessuna cifra recuperabile nella card interventi | F-SY-2 |
| Svuota il Cestino | Compare il dialogo di Windows (nessun `SHERB_NOCONFIRMATION`); prima Quado dichiara numero e GB degli altri elementi presenti | F-AC-1 |
| Apri il Cestino | Il registro elenca gli elementi spostati da Quado | F-AC-1, F-AC-6 |

Verifiche una tantum di M1 (07 §3), con esito scritto in `m1/cestino-verifiche.md` e `m1/cestino-verifiche-bis.md`:
- cosa fa lo svuotamento del Cestino con un `$R` orfano (IP10);
- come si comporta `PreDeleteItem` e quale valore di ritorno annulla il singolo elemento (IP9);
- se Windows conta nella capacità la dimensione logica o allocata, e se conta gli orfani.

### 5.4 Restart Manager (F-AC-8)

- File tenuto aperto da un processo di prova: elemento bloccato con il nome del processo.
- File aperto e chiuso prima del controllo: elemento ammesso.
- File aperto dopo il controllo e prima dello spostamento: `IFileOperation` fallisce per quell'elemento, il riepilogo lo riporta come fallito e gli altri proseguono (RS6).
- Cartella grande: il campione comprende tutti gli eseguibili e le DLL.

### 5.5 Riferimenti (F-AC-9)

Una cartella di versione su VHDX referenziata da `DisplayIcon` di una voce di disinstallazione, da una chiave Run, da un collegamento `.lnk` nel menu Start dell'utente e da un'attività pianificata dell'utente:
- ciascun riferimento blocca l'elemento e compare nella conferma;
- i `.lnk` si leggono senza `Resolve` (Process Monitor: nessun accesso a percorsi di rete e nessuna ricerca);
- un'attività non visibile all'utente rende il controllo "parziale", e la conferma lo dice;
- i riferimenti di registro si provano con la sorgente simulata del §2.3; un solo controllo manuale usa HKCU vero, con valori che puntano a file inesistenti e che si rimuovono a fine prova.

### 5.6 Esecuzione, riepilogo e registro

- File in uso durante l'intervento: riepilogo con riusciti, saltati e falliti e il motivo; nessun file toccato oltre quelli confermati (F-AC-3).
- Esito parziale: il riepilogo dice cosa è nel Cestino e come ripristinarlo (RS8).
- Registro (F-AC-6): esiste solo dopo il primo intervento, contiene stimato e ottenuto, si cancella dalle impostazioni.

### 5.7 Procedure e comandi (F-AC-1)

- Ogni URI di Impostazioni usato apre la pagina attesa sul PC di sviluppo: controllo manuale con data, da ripetere al passaggio a Windows 11 (07 §5.3).
- I comandi ufficiali si copiano negli appunti con la fonte e non si eseguono: Quado non crea processi, a parte l'helper, e apre solo URI tipizzati (test di architettura del §5.1).
- `--volumes` non compare in nessuna stringa; `/ResetBase` compare solo tra le alternative, con la conseguenza.

## 6. Stima e verifica (F-VE)

### 6.1 Stima contro variazione reale, su VHDX

Per ogni scenario: stima in simulazione, spostamento nel Cestino, svuotamento con il dialogo di Windows, misura con `GetDiskFreeSpaceExW`, confronto. Tolleranza iniziale: 1% della stima più 1 MiB; si conferma o si corregge in M5 con le prime misure e si scrive nel test.

| Scenario | Stima attesa | Criterio |
|---|---|---|
| File normali | Esatta | Variazione entro la tolleranza |
| Hard link con tutti i nomi nella selezione | Contati una volta | Variazione entro la tolleranza |
| Hard link con un nome fuori dalla selezione | File escluso e citato | La variazione non include quel file |
| File compressi NTFS e WOF | Allocata | Variazione entro la tolleranza |
| File residenti nella MFT | 0 | La variazione non cresce per loro |
| File sparse | Allocata | Variazione entro la tolleranza |
| Selezione con una cartella negata | Intervallo | La variazione cade nell'intervallo |
| Segnaposto cloud | Solo i byte locali | Nessuna idratazione; variazione entro la tolleranza |
| Disco virtuale dentro il VHDX | 0 fino alla compattazione | Variazione circa 0 |
| Subito dopo lo spostamento, prima dello svuotamento | 0 | Variazione circa 0 |

### 6.2 Flusso di verifica

- Prima dell'azione si registrano spazio libero, dimensione del Cestino, stima con metodo e stato (F-VE-1).
- Nuove misure dopo l'azione, dopo lo svuotamento e dopo "Ho eseguito la procedura", ciascuna con spazio libero e riscansione dei sottoalberi toccati (F-VE-2).
- Differenza oltre il 5%: ogni voce della spiegazione corrisponde a un fatto misurato (elementi saltati, nomi rimasti, contenuto ancora nel Cestino); se non basta, compare "non spiegata" (F-VE-3).
- Attività concorrente: un processo di prova scrive sul VHDX durante la verifica. La variazione del volume cambia, la riscansione no; la differenza è dichiarata e non attribuita all'intervento (RS17).
- Modello: dopo la riscansione dei sottoalberi i totali coincidono con quelli di una scansione completa nuova (F-VE-4).
- Vista Interventi: mostra lo storico dal registro, con stimato, ottenuto e differenza (F-VE-5).
- Su C: un controllo manuale con data: un intervento reale a basso rischio, per osservare il comportamento con Sensore memoria, SilentCleanup e Defender attivi.

## 7. Interfaccia e accessibilità

- Snapshot offscreen (`egui_kittest`) di ogni schermata, nei due temi, a scala 100%, 150% e 200% (F-GE-2).
- Token: ogni coppia testo e superficie, in ogni stato, rispetta 4,5:1 (F-GE-2).
- Colori dei dati (F-GE-2, F-EX-1, 04 §6.2, RS23): un test automatico ricalcola dai token, nei due temi, il contrasto ≥ 3:1 di ogni colore `--c-*` con il fondo dei pannelli, il contrasto ≥ 4,5:1 di ogni coppia di riempimento ed etichetta della mappa e la distanza tra le coppie di tinte, grigio di "Non classificato" compreso, in visione normale e con la simulazione delle discromatopsie (Machado 2009, ΔE OKLab, soglia del validatore di 04 §6.2 scritta nel test: 3,0). Con i valori dell'anteprima falliva la coppia Windows / Non classificato in tema chiaro per la protanopia; corretta in M2.
- Percorsi completi solo da tastiera, in test automatici (F-EX-3).
- Con "Effetti di animazione" disattivati in Windows nessuna animazione.
- Etichette lunghe con puntini e testo completo nel tooltip.
- Lingue (F-GE-1, 05 D13):
  - completezza: `it.ftl` ed `en.ftl` hanno le stesse chiavi e gli stessi segnaposto, nessuna riga con errori, ogni chiave di `it.ftl` ha un commento di contesto;
  - pseudo-lingua di prova (testi più lunghi del 40%, caratteri accentati, segni di inizio e fine): ogni testo visibile negli snapshot è pseudo-localizzato, quindi nessuna stringa è scritta nel codice; nessun testo è tagliato;
  - snapshot di ogni schermata anche in inglese;
  - cambio di lingua in Impostazioni senza riavvio;
  - un file in `%APPDATA%\Quado\lingue` con una riga sbagliata: la riga si ignora, il resto vale, l'errore compare in Impostazioni;
  - il core non restituisce frasi: un test d'architettura cerca testo naturale nei tipi pubblici di errore e di stato del core.
- Vocabolario: i cinque stati compaiono con le stesse parole in card, ispettore, conferma e risultato. Assenti dal catalogo le parole vietate di 04 e in più "inutilizzato", "mai usato", "non più in uso", "puoi ripristinare tutto".
- Unità (F-GE-3): GB = 1024³ con la virgola decimale; sul singolo file "Dimensioni su disco" coincide con le Proprietà di Esplora file (controllo manuale su file di prova); scelta SI nelle impostazioni.
- Mappa, tabella, ricerca, filtri e File grandi (F-EX-1, F-EX-2, F-EX-4, F-EX-5): i file più grandi del volume coincidono con l'ordinamento dell'oracolo; i filtri danno gli stessi insiemi di una query sul modello; i tempi sono al §9.
- Ispettore (F-IN-5): "Apri in Esplora file" seleziona l'elemento senza aprirlo; "Copia percorso"; "Proprietà" solo su richiesta (RS14).
- Albero AccessKit di base verificato con snapshot. Prova manuale con un lettore di schermo solo se l'utente lo usa (05 D2).
- Testo a confronto con ClearType: screenshot affiancati approvati in M0 e di nuovo in M6.

## 8. Usabilità a compiti

Partecipanti: il proprietario del progetto e, se possibile, una persona non esperta. Senza aiuto, osservati, sul PC di sviluppo. I comportamenti attesi per ciascun compito sono quelli di `09-matrice-scenari.md`.

Compiti:
1. trova cosa occupa più spazio;
2. spiega perché AppData è grande;
3. decidi sui file di Download;
4. trova un programma grande e di' cosa sai del suo uso;
5. esegui un intervento e verificalo, su un caso a basso rischio scelto prima (per esempio copie identiche in Download) oppure su una cartella di prova.

Misure: completamento senza aiuto, tempo, errori, interpretazione degli stati.

Domande di comprensione dopo i compiti, con la risposta attesa:
1. "Da verificare" significa che puoi cancellare? No: prima vanno soddisfatti i controlli.
2. "Decidi tu" significa che Quado consiglia di eliminare? No.
3. Dopo lo spostamento nel Cestino, quanto spazio hai liberato? Nessuno, finché il Cestino non viene svuotato.
4. Cosa indica il tratteggio? Spazio occupato che non è stato misurato o attribuito.
5. "Lo gestisce Steam": come si libera quello spazio? Da Steam, con la procedura indicata.
6. "Nessun dato d'uso" significa che il programma non è mai stato usato? No.
7. "Potrebbe essere un residuo": Quado propone di eliminarlo? No.
8. Cosa vuol dire "dedotto"? Quado lo ricava da indizi e può sbagliare.

Criteri:
- il proprietario del progetto completa i cinque compiti senza aiuto;
- le domande 1, 3, 6 e 7 ricevono la risposta corretta da tutti i partecipanti;
- una risposta sbagliata porta a rivedere il testo e a ripetere la domanda dopo la correzione;
- i tempi si registrano come riferimento per le ripetizioni, senza soglia nella prima sessione.

## 9. Prestazioni

Protocollo:
- ogni risultato annota CPU, disco, RAM, versione di Windows, Defender attivo, versione dell'app e data;
- 5 esecuzioni per misura, con mediana e minimo-massimo;
- cache calda sulla C: reale; cache fredda con la prima scansione di C: dopo un riavvio (5 misure, ciascuna preceduta da un riavvio, 07 §3 riga 8) e sui VHDX smontati e rimontati (§1.3);
- metriche: tempo totale, tempo fino alla Panoramica, CPU utente e kernel, picco di working set, handle aperti, tempi di frame p50, p95 e p99.

| Misura | Obiettivo | Requisiti |
|---|---|---|
| Scansione di C: a cache calda | ≤ 22 s, mediana di 5 | F-SC-2 |
| Scansione a cache fredda, C: dopo il riavvio e VHDX rimontati | Da fissare in M1 dopo la misura (C11) | F-SC-2 |
| Picco di memoria per 3,3 milioni di voci | ≤ 400 MB con i nomi reali (05 D5) | F-SC-2 |
| Avanzamento | Almeno 4 aggiornamenti al secondo; frame della UI p95 ≤ 16 ms durante la scansione | F-SC-4 |
| Annullamento | ≤ 500 ms | F-SC-5 |
| Tabella | Frame p95 ≤ 16 ms e p99 ≤ 33 ms con 3,5 milioni di righe e con una cartella da 1 milione di figli | F-EX-2 |
| Mappa | Frame p95 ≤ 16 ms con 20.000 aree a 1080p | F-EX-1 |
| Ricerca | ≤ 300 ms su 3 milioni di nomi | F-EX-4 |
| Avvio | Primo frame interattivo dopo il riavvio: misurato in M2, obiettivo 1 s | - |
| Letture di contenuto in background | Frame della UI p95 ≤ 16 ms durante la lettura degli installatori e l'hash | F-DL-2, F-DL-4 |

Regola di onestà: nei documenti un numero di prestazioni compare solo se misurato con questo protocollo; un obiettivo resta scritto come obiettivo. Non si pubblicano benchmark.

Misure con questo protocollo sul PC di sviluppo (portatile a 6 core, SSD NVMe, Windows 10 22H2), 16-27/10/2026, profilo release, cache calda, unità C: con circa 3,1 milioni di file e 480.000 cartelle:

| Misura | Mediana (minimo-massimo) | Obiettivo |
|---|---|---|
| Scansione completa nell'app, 5 esecuzioni | 12,0 s (11,9-12,2) | ≤ 22 s |
| Picco di working set della sola scansione, 3,62 milioni di nodi | 359 MiB (358-360) | ≤ 400 MB con 3,3 milioni di voci |
| Memoria privata con la Panoramica ferma dopo la scansione | 515 MiB, 827 prima dell'ottimizzazione della memoria del 26-27/10 (font di ripiego caricati solo quando servono, dispositivo grafico più piccolo) | - |
| Primo frame interattivo all'avvio, a PC già acceso, 5 esecuzioni | 665 ms (612-1100) | 1 s dopo il riavvio, non ancora misurato |

Build: con il profilo `dist` (LTO "fat", una sola unità di codegen) una compilazione da zero con 4 job dura circa 10 minuti e mezzo, la ricompilazione dopo una modifica in `quado-ui` 7 min 21 s; con il profilo release la stessa ricompilazione dura 52 s. Il profilo `dist` accorcia del 10-30% i frame che dipendono dalla CPU dell'interfaccia; scansione e avvio non cambiano.

## 10. Privacy

| Livello | Verifica | Quando |
|---|---|---|
| Dipendenze | `cargo-deny`: divieto di crate di rete (client HTTP, TLS, DNS), licenze ammesse, avvisi di sicurezza | A ogni modifica delle dipendenze |
| Binario | Tabella delle importazioni: nessuna funzione di WinHTTP, WinINet, `urlmon` e `ws2_32` (`connect`, `WSAConnect`, `getaddrinfo`) | A ogni build di installazione (`cargo xtask importazioni`) |
| Sessione senza rete | Sessione completa guidata (scansione, Esplora, ricerca, lente Download con hash, Programmi, intervento simulato, impostazioni) con la rete scollegata: nessun errore e nessun comportamento diverso. La stessa sessione con la rete collegata e il campionamento delle connessioni per PID (`Get-NetTCPConnection`, `Get-NetUDPEndpoint`): zero connessioni | Una volta prima della v1 (M6) |
| Dati salvati | Process Monitor filtrato sul processo: scritture solo in `%APPDATA%\Quado` e, per gli interventi, nel Cestino | Nella stessa sessione |
| Zone.Identifier | Process Monitor: nessuna apertura di `:Zone.Identifier` (RS21) | Nella stessa sessione |

Limite dichiarato (M6): il sottosistema grafico gira nel processo di Quado e scrive le sue cache fuori da `%APPDATA%\Quado`, come in ogni programma Direct3D: la cache degli shader di Windows (`%LOCALAPPDATA%\D3DSCache`), quelle dei driver delle schede video (wgpu carica i driver di tutte le schede quando elenca gli adattatori) e il registro dell'overlay di cattura che il driver di una scheda video può caricare nei processi Direct3D. `cargo xtask sessione-prova` le elenca a parte con il motivo e non le conta come scritture di Quado.

Limite dichiarato: "Proprietà" di Windows e altri dialoghi della shell caricano estensioni di terzi nel processo, e quelle possono usare la rete. Per questo la cartella da scansionare si sceglie con un selettore nostro e "Proprietà" si apre solo su richiesta; la sessione di prova non la usa e il suo effetto si documenta a parte. I link alle fonti delle regole si aprono nel browser solo su clic: quel traffico è del browser.

## 11. Sicurezza

- Nessun handle in scrittura durante scansione, ispettore, letture di configurazione e letture di contenuto: traccia ETW `FileIo` o Process Monitor, controllando la maschera di accesso (F-SC-2).
- Nessuna lettura del contenuto dei file dell'utente fuori dalle funzioni su richiesta: durante scansione e ispettore nessun `ReadFile` su quei file (F-IN-3).
- MSI in sola lettura: `MsiOpenPackage` e `MsiOpenPackageEx` assenti dalle importazioni; file invariato dopo la lettura.
- `UninstallString` e `QuietUninstallString` mai eseguite: il lanciatore accetta solo valori tipizzati (§4) e Quado non crea processi tranne l'helper (F-AP-5).
- Helper (F-SC-3):
  - messaggio iniziale: ingressi casuali (`proptest`) e percorsi non validi (relativi, con `..`, di un altro volume, troppo lunghi, troppi) rifiutati;
  - DACL della pipe letta con `GetSecurityInfo`: solo il SID dell'utente; client remoti rifiutati;
  - un processo diverso da quello che ha lanciato l'helper viene rifiutato;
  - nessun file aperto in scrittura; il processo termina a fine lavoro;
  - rifiuta di partire da `Downloads` e dalle cartelle temporanee.
- DLL (RS11): il load config dell'eseguibile contiene `DependentLoadFlags` = 0x800 (`dumpbin /loadconfig` o `xtask`); una DLL esca con il nome di una dipendenza, messa nella cartella dell'eseguibile, non viene caricata (Process Monitor); `SetDefaultDllDirectories` è la prima chiamata.
- Codice `unsafe`: ogni blocco ha il commento con l'invariante e un test.

## 12. Test già eseguiti

Prove esplorative sul PC di sviluppo, in sola lettura, prima di scrivere codice. Non sono test di regressione: diventano fixture e controlli in M1 e M4. I numeri stanno nei documenti della colonna "Dove" e non si ripetono qui.

| Prova | Esito | Dove |
|---|---|---|
| Autenticità delle copie locali delle tre app di riferimento (un editor di foto, uno di grafica vettoriale e uno di PDF) | SHA256 identici a quelli delle release ufficiali | 04 §1 |
| Enumerazione non elevata di C: con `FileIdExtdDirectoryInfo` su 12 thread (sonda) | Tempi a freddo e a caldo, picco di memoria, numero di file e cartelle | 02 §2.1, anche per il metodo |
| Stessa enumerazione con `std::fs::read_dir` | Solo dimensione logica, tempi confrontabili | 02 §2.1 |
| Hard link del volume | File con più nomi e byte che una somma ingenua conterebbe due volte | 02 §2.1 |
| Scarto tra usato e letto | Non attribuito e cartelle negate | 02 §6; definizione in 03 §6.4 |
| Matrice delle dimensioni su file di prova | WOF con allocata 0 nella voce di cartella; residenti, sparse, flussi alternativi, hard link | 02 §3 |
| Metadati misurabili senza privilegi | `$MFT` e indici delle cartelle | 02 §6 e §7 |
| API che richiedono privilegi | Apertura del volume, `FSCTL_ENUM_USN_DATA`, `vssadmin`, `Win32_ShadowStorage`, `dism /get-reservedstoragestate` negate all'utente standard | 02 §2 |
| Voci di disinstallazione | Copertura di `InstallLocation` e dei percorsi utilizzabili; `UninstallString` problematiche | 02 §10; `revisione/prove-windows.md` §E |
| Cestino | Due misure, cartelle `$R` orfane, `MaxCapacity`, Sensore memoria attivo | `revisione/prove-windows.md` §C |
| Fonti dell'uso dei programmi | UserAssist fermo con `Start_TrackProgs` = 0; LastPlayed di Steam concorde con gli `.acf`; BAM, Prefetch, Amcache e SRUM negati | `revisione/prove-windows.md` §A |
| Ultimo accesso su C: | Aggiornamento attivo anche se `fsutil` scrive "Disabled" | `revisione/prove-windows.md` §A |
| Installatori in Download | Copertura e costo di risorsa di versione, MSI, firma, Zone.Identifier e marcatori; copie identiche e suffisso "(N)" | `revisione/prove-windows.md` §B |
| Residui per nome | Cartelle senza corrispondenza e falsi positivi modificati di recente | `revisione/prove-windows.md` §D |

Non ancora provati: modalità con privilegi, OneDrive (il PC di sviluppo non lo usa: i segnaposto si provano con il provider di prova di §1.2), svuotamento del Cestino con un `$R` orfano, egui sul PC di sviluppo.

## 13. Copertura dei requisiti

Ogni ID di `revisione/decisioni-revisione.md` §9 ter con le sezioni che lo verificano. I test di usabilità (§8) attraversano tutta la catena e non sono elencati per ID.

| ID | Requisito | Test |
|---|---|---|
| F-SC-1 | Elenco dei volumi | §1.3 |
| F-SC-2 | Scansione non elevata in sola lettura | §1.1, §1.2, §9, §11 |
| F-SC-3 | Completamento con privilegi (helper) | §1.5, §11 |
| F-SC-4 | Avanzamento | §9 |
| F-SC-5 | Annullamento e risultato parziale | §1.2, §9 |
| F-SC-6 | Cartelle negate elencate | §1.2, §1.4 |
| F-SC-7 | Hard link e reparse point | §1.2 |
| F-SC-8 | Logica e allocata, correzione WOF | §1.2 |
| F-SC-9 | Segnaposto cloud | §1.2, §3.2 |
| F-SC-10 | File system | §1.3 |
| F-OV-1 | Bilancio (03 §6.4) | §1.4, §2.2 |
| F-OV-2 | Top list con regola di non sovrapposizione | §1.4 |
| F-OV-3 | Card dei conti: osservato e possibile | §1.4 |
| F-OV-4 | Card interventi da record | §1.4, §2.2 |
| F-EX-1 | Mappa | §7, §9 |
| F-EX-2 | Tabella virtualizzata | §1.2, §7, §9 |
| F-EX-3 | Navigazione e tastiera | §7 |
| F-EX-4 | Ricerca e filtri | §7, §9 |
| F-EX-5 | Vista File grandi | §7 |
| F-IN-1 | Fatti con provenienza | §2.2 |
| F-IN-2 | Perché è grande | §2.1 |
| F-IN-3 | Tipo dall'estensione senza leggere il contenuto | §11 |
| F-IN-4 | Cosa puoi fare, dal record | §2.1, §2.2 |
| F-IN-5 | Apri in Esplora file, Copia percorso, Proprietà | §1.2, §7 |
| F-IN-6 | Verifica questa voce | §1.2, §3.6 |
| F-DL-1 | Famiglie per tipo | §3.1 |
| F-DL-2 | Metadati degli installatori e corrispondenza | §3.5, §9 |
| F-DL-3 | Suffisso "(N)" solo filtro | §3.1 |
| F-DL-4 | Copie identiche con hash su richiesta | §3.2, §9 |
| F-DL-5 | Archivio e cartella estratta | §3.3 |
| F-DL-6 | Download incompleti non in uso | §3.4 |
| F-AP-1 | Elenco dei programmi | §4 |
| F-AP-2 | Spazio solo con prova del percorso | §4 |
| F-AP-3 | Dichiarato contro misurato | §4 |
| F-AP-4 | Uso noto, mai "inutilizzato" | §4 |
| F-AP-5 | Disinstallazione tramite interfaccia ufficiale | §4, §11 |
| F-AP-6 | Residui | §4 |
| F-SY-1 | Vista Windows e sistema | §2.1, §7 |
| F-SY-2 | Cestino misurato in due modi | §2.1, §5.3 |
| F-SY-3 | Configurazione in sola lettura | §2.4 |
| F-AC-1 | Tipi di intervento | §5.3, §5.7 |
| F-AC-2 | Simulazione e piano confermato | §5.1, §5.2 |
| F-AC-3 | Conferma, Esc e riepilogo | §5.1, §5.6 |
| F-AC-4 | Elementi protetti ed eccezione via regola | §5.2 |
| F-AC-5 | Mai eliminazione definitiva (`PreDeleteItem`) | §5.3 |
| F-AC-6 | Registro con stimato e ottenuto | §5.3, §5.6 |
| F-AC-7 | Capacità del Cestino e Sensore memoria | §2.4, §5.3 |
| F-AC-8 | Restart Manager | §3.4, §5.4 |
| F-AC-9 | Riferimenti | §5.5 |
| F-VE-1 | Misura di partenza | §6.2 |
| F-VE-2 | Nuove misure | §6.2 |
| F-VE-3 | Stimato contro ottenuto | §6.1, §6.2 |
| F-VE-4 | Aggiornamento del modello | §6.2 |
| F-VE-5 | Vista Interventi | §6.2 |
| F-GE-1 | Lingue: italiano e inglese, altre aggiungibili | §7 |
| F-GE-2 | Temi e contrasto | §7 |
| F-GE-3 | Unità | §7 |
