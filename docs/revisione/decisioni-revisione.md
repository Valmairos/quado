# Decisioni della revisione (12/10/2026)

Promemoria delle decisioni prese dal revisore principale sulle criticità emerse. Fonti: la revisione avversariale indipendente, riassunta con le sue sigle (C*, V*, CH*, R*, RS*, IP*, U*, M*, N*) in `../10-revisione-critica.md`, e `prove-windows.md` (ricerca sulle prove disponibili in Windows). Ogni decisione va riportata in modo coerente in tutti i documenti toccati.

## 1. Cornice

- Quado è nato come strumento personale, per un PC con Windows 10 oggi e probabilmente Windows 11 in futuro. Che altri lo usino è un vantaggio, non un obiettivo.
- Tolti dalla v1, perché costano senza un beneficio proporzionato:
  - MSI, firma del codice, winget, Store;
  - benchmark pubblicati;
  - CI privilegiata multi file system;
  - ARM64 e Linux (restano solo come nota di portabilità);
  - modalità portable (una sola sede dei dati, `%APPDATA%\Rilievo`);
  - palette comandi (passa alla v1.1);
  - "Esporta diagnostica".
- Distribuzione: eseguibile compilato in locale, tenuto in una cartella dell'utente (`%LOCALAPPDATA%\Programs\Rilievo`). Senza Mark-of-the-Web SmartScreen non interviene.
- Ha senso costruirlo rispetto agli strumenti esistenti? Sì, ma solo per lo strato che spiega e verifica. Individuare è già risolto (WizTree, gratuito per uso personale, più le Impostazioni di Windows). Comprendere, Valutare, Agire con controlli e Verificare non lo fa nessuno senza AI nel cloud. Lo scanner costa poco: la sonda da circa 300 righe ha fatto 16 s.
  - Alternativa considerata e scartata: costruire Quado sopra l'export CSV di WizTree. Strumento chiuso, serve l'amministratore per la velocità, il CSV non ha ID file né attributi, e si finirebbe con due app.

## 2. Modello delle raccomandazioni (risolve R1, U5, C1, C8)

Cinque stati visibili, con un vocabolario unico usato in card, ispettore, conferma e risultato:

| Stato | Quando | Cosa abilita |
|---|---|---|
| Gestibile | Prova di gestibilità diretta (non solo di identità) e metodo a basso rischio: copie identiche verificate con hash, file di download incompleti non in uso | Azione con simulazione |
| Da verificare | Gestibilità plausibile ma prova parziale. I controlli sono elencati: quelli automatici devono passare e quelli manuali vanno spuntati dall'utente | Azione disabilitata finché i controlli non sono soddisfatti |
| Decidi tu | Ruolo noto, utilità non determinabile dal software: tuoi file, installatori, immagini disco, programmi grandi | Strumenti per decidere (raggruppamenti, corrispondenze, confronti); azione solo su scelta esplicita, con simulazione |
| Lo gestisce X | Si gestisce solo con lo strumento o la procedura di X (Windows, Steam, npm, Docker, l'app). Variante "Nessuna procedura: non toccare" (Windows\Installer) | Rinvio alla procedura (URI, comando mostrato e copiato); mai rimozione manuale |
| Non sappiamo | Nessuna prova sul ruolo | Solo misure e "Apri in Esplora file" |

Ogni raccomandazione è un record con campi obbligatori:
- stato;
- azione;
- prova di identità e prova di gestibilità, tenute distinte;
- ipotesi;
- affidabilità, con un criterio;
- rischi ed effetti;
- metodo preferito e alternative;
- controlli preventivi, automatici e manuali;
- stima, con il suo metodo e il suo stato (esatta, intervallo, non stimabile);
- reversibilità (cosa e fino a quando);
- privilegi richiesti;
- comportamento in caso di errore o esito parziale;
- verifica dopo l'azione;
- regola, con la sua fonte e la data di verifica.

Gli stati Gestibile e Da verificare sono ammessi solo con una prova di gestibilità (test di proprietà).

Altre regole del modello:
- La provenienza vale per singola affermazione, non per elemento: una riga può avere più fatti con glifi diversi.
- Criteri di affidabilità dei fatti dedotti:

  | Livello | Criterio |
  |---|---|
  | alta | due fonti concordi, oppure una fonte ufficiale con struttura conforme |
  | media | una sola fonte non ufficiale, oppure struttura riconosciuta senza fonte |
  | bassa | solo nome o somiglianza |

- Il colore del gestore nella mappa e nel bilancio si assegna solo da fatti "da fonte" o "dedotto alta". Negli altri casi si usa il grigio con l'etichetta "ipotesi".

## 3. Casi specifici

- **CapCut, versioni precedenti.**
  - Stato: Da verificare. Non esiste una procedura ufficiale: la disinstallazione rischierebbe `User Data\Projects`.
  - Eccezione esplicita e scritta a F-AC-4, consentita solo tramite una regola con stato Da verificare e con controlli dedicati:
    - nessun processo usa i file (Restart Manager su un campione e sugli eseguibili);
    - nessun riferimento alle versioni da spostare in registro (`UninstallString`, `DisplayIcon`, App Paths, Run), collegamenti `.lnk` di Start e Desktop, attività pianificate;
    - si tengono di default le ultime due versioni (in uso e precedente).
  - Testi:
    - Panoramica: "{n} cartelle di versioni precedenti (dedotto)", senza dire "non più in uso";
    - conferma: "Prova CapCut prima di svuotare il Cestino".
  - La stima esclude la versione in uso e la precedente.
- **Cestino orfano.** È un'anomalia spiegata (F-SY-2), non un intervento: niente cifra recuperabile nella card interventi. Prima dell'implementazione (M1) si prova su un VHDX se lo svuotamento di Windows rimuove un `$R` senza `$I`. La riga della top list si limita al misurato: "{x} GB su disco, {y} MB secondo Windows".
- **Ibernazione.**
  - Stato: Decidi tu.
  - Opzioni affiancate:
    - `powercfg /hibernate /type reduced`: mantiene l'avvio rapido e toglie l'ibernazione;
    - `powercfg /h off`: toglie entrambi, più la sospensione ibrida.
  - Entrambe richiedono l'amministratore.
  - Lettura preventiva in sola lettura: `powercfg /a` e l'azione su batteria critica. Su un portatile si avvisa se l'azione per batteria critica è l'ibernazione.
- **npm.** Lo gestisce npm. Si preferisce `npm cache verify`; `npm cache clean --force` è indicato come alternativa con le sue conseguenze (nuovi download, installazioni offline). Il percorso viene da `npm config get cache`, oppure da `.npmrc` o dalla variabile d'ambiente: lettura di configurazione, mai esecuzione.
- **pnpm.** Lo gestisce pnpm (`pnpm store prune`). La stima è "non stimabile senza contare i link", per via degli hard link verso `node_modules`.
- **Docker e WSL.** Lo gestisce Docker / WSL.
  - Stima su C: "0 fino alla compattazione del disco virtuale".
  - `--volumes` non va mai mostrato.
  - La compattazione è documentata con le sue condizioni (amministratore, servizi fermi, `Optimize-VHD` assente su Home). `--set-sparse` è segnalato come rischioso sulle versioni recenti.
- **WinSxS.** Lo gestisce Windows. `StartComponentCleanup` senza `/ResetBase`; `/ResetBase` solo come alternativa, con la conseguenza scritta: gli aggiornamenti non si possono più disinstallare.
- **SoftwareDistribution, Temp, miniature.** Solo tramite Impostazioni › File temporanei, mai a mano.
- **node_modules, target, .next, build.** Decidi tu, con controlli: il lockfile è presente, e `cargo clean` si usa per `target`. Ricostruibili solo se il progetto si può ancora ricostruire.

## 4. Cestino come rete di sicurezza (RS1, RS2, RS3, CH3, U10)

Prima di ogni spostamento nel Cestino:
- **Capacità.** Si legge la capacità del Cestino del volume, `MaxCapacity` nel registro BitBucket (in sola lettura), e la si confronta con il contenuto attuale (`SHQueryRecycleBinW`) più la selezione. Se il totale supera la capacità, l'azione si blocca: Windows eliminerebbe da solo gli elementi più vecchi.
- **Sensore memoria.** Se lo svuotamento automatico del Cestino è attivo, la conferma lo dice con il numero di giorni.

Per liberare davvero lo spazio non si usa mai "svuota tutto" come strada implicita. Si offrono due procedure di Windows:
- "Apri il Cestino", per eliminare solo gli elementi spostati da Quado: il registro dice quali;
- "Svuota il Cestino", con il dialogo di Windows, dichiarando cos'altro contiene (numero e GB).

Testi: "Puoi ripristinare dal Cestino finché non viene svuotato, da te o da Sensore memoria". Mai "puoi ripristinare tutto".

## 5. Stima e verifica (CH1, CH2, V7)

**Stima** (in simulazione, rileggendo gli attributi dei file selezionati con `FileStandardInfo` e `FileIdInfo`, solo attributi):
- si contano le allocate dei file i cui nomi stanno tutti nella selezione, deduplicate per ID;
- un file con altri nomi fuori dalla selezione è escluso e citato;
- nomi non conoscibili (cartelle negate) fanno dire "intervallo";
- segnaposto cloud: solo i byte locali;
- file residenti nella MFT: 0, perché la MFT non si restringe;
- contenuto dei dischi virtuali: 0 fino alla compattazione;
- nel Cestino: 0 subito;
- con i punti di ripristino attivi: nota "parte può essere riassorbita" (dettaglio in `prove-windows.md`).
- Stato della stima: esatta, intervallo o non stimabile.

**Verifica** (nuovi requisiti F-VE):
1. Prima dell'azione si registrano spazio libero, dimensione del Cestino e stima con il suo metodo.
2. Si rimisura dopo l'azione, dopo lo svuotamento del Cestino e dopo "Ho eseguito la procedura" (per comandi e procedure esterne): spazio libero più riscansione dei sottoalberi toccati, poi stimato, ottenuto e differenza.
3. Oltre il 5% di differenza, la differenza si spiega voce per voce oppure si dichiara non spiegata.
4. Il registro degli interventi conserva stimato e ottenuto.
5. Il modello in memoria si aggiorna riscansionando i sottoalberi toccati.

Test su VHDX: stima contro delta reale.

## 6. Bilancio (C3, C4, V1, U1, U2)

- Definizione:
  - "usato" = totale meno libero da `GetDiskFreeSpaceExW`, misurato a fine scansione;
  - l'usato si misura anche a inizio scansione, e la deriva tra i due valori si mostra come incertezza;
  - non attribuito = usato a fine scansione - allocato unico letto - metadati misurati ($MFT, indici, journal).
- Card dei conti:
  - due liste: "Osservato" (cartelle negate con il loro numero, cestini di altri SID, profili altrui) e "Possibile, non misurabile senza amministratore" (copie shadow, archiviazione riservata, altri metafile);
  - nessuna frase causale;
  - gli hard link compaiono come nota ("già contati una volta"), non come riga da sottrarre.
- Test:
  - su un VHDX fermo: residuo ≤ metafile attesi + tolleranza;
  - errore iniettato (hard link non deduplicati) che deve spostare il residuo.
- Le misure citate nei documenti sono state prese sul PC di sviluppo, in sola lettura, con la data; i test usano fixture versionate.
- Unità:
  - nell'interfaccia GB = 1024³, come Esplora file;
  - nei documenti si scrive sempre l'unità; le misure della sonda in base 10 portano l'indicazione "10^9".

## 7. Nuove funzioni v1 (N1-N5, N9)

**Lente Download** (F-DL), su `Downloads` per default e applicabile a qualunque cartella dell'utente:
- famiglie per estensione: installatori, archivi, immagini disco, video e audio, documenti e immagini, download incompleti;
- installatori:
  - nome prodotto e versione dalla risorsa di versione PE e, per gli MSI, dalla tabella Property letta in sola lettura (vedi `prove-windows.md` per i limiti);
  - corrispondenza con i programmi installati: stessa versione, versione diversa, non installato. È dedotta, con affidabilità secondo il criterio del §2;
- stesso nome con suffisso "(1)": solo un filtro, mai una prova;
- copie identiche:
  - hash calcolato solo su richiesta, solo tra file della stessa dimensione;
  - mai sui segnaposto cloud;
  - dopo il confronto lo stato diventa Gestibile (si tiene il più vecchio);
- archivio con una cartella omonima accanto:
  - si confronta la directory centrale dello zip (nomi e dimensioni) con la cartella: "N voci su M presenti";
  - stato Decidi tu, con prova misurata;
- download incompleti (`.crdownload`, `.part`) non in uso: Gestibile.

**Programmi** (F-AP-4, F-AP-5, F-AP-6):
- dimensione misurata, `InstallDate` e "ultimo avvio noto" solo dalle fonti giudicate affidabili in `prove-windows.md`, ognuna con la sua copertura dichiarata;
- mai "inutilizzato": stato Decidi tu.
- Disinstallazione solo con le procedure ufficiali:
  - Impostazioni › App (`ms-settings:appsfeatures`, con il nome copiato negli appunti);
  - `steam://uninstall/<appid>`;
  - il launcher Epic.
  
  Quado non esegue mai `UninstallString` in v1.
- Precontrollo: se una regola indica tuoi file dentro la cartella del programma, avvisa di copiarli prima.
- Residui (F-AP-6): cartelle in AppData e ProgramData non rivendicate da alcun programma installato né da una regola. Stato Non sappiamo, con il testo "Nessun programma installato la rivendica". Nessuna proposta.

**"Verifica questa voce"** (F-IN-6): rilegge gli attributi di un file o di una cartella per correggere le dimensioni pigre.

**Lettura della configurazione in sola lettura** (F-SY-3): ibernazione e avvio rapido, ripristino configurazione (se leggibile), Sensore memoria, capacità del Cestino. Serve per raccomandazioni condizionali, senza sorveglianza.

**Vista "File grandi"**: requisito F-EX-5, i file più grandi del volume con filtri. **Vista "Interventi"**: storico e risultati (F-VE).

## 8. Architettura (IP4, IP5, C16, RS11)

- **Modalità con privilegi.** Niente parser MFT in v1. Un helper elevato su richiesta enumera solo le cartelle negate con `SeBackupPrivilege` e `FILE_FLAG_BACKUP_SEMANTICS`, usando lo stesso codice dello scanner. In più:
  - interroga lo spazio delle copie shadow (`vssadmin list shadowstorage` o WMI) e lo stato dell'archiviazione riservata (comando da verificare in M1);
  - raccoglie i cestini di altri SID;
  - restituisce i profili altrui in forma aggregata.
  
  Il guadagno di velocità della MFT non serve. `FSCTL_QUERY_FILE_LAYOUT` e la MFT passano alla v2, solo se servirà. Spariscono il crate `quado-mft`, il fuzzing, gli oracoli `ntfs` e `mft` e i rischi R5 e IP3.
- **Helper e DLL.** Linker con `/DEPENDENTLOADFLAG:0x800`, da verificare in M1, oltre a `SetDefaultDllDirectories`. Eseguibile in `%LOCALAPPDATA%\Programs\Rilievo`, mai avviato da `Downloads`.
- **Memoria.**
  - Durante la scansione, un array laterale di ID per la deduplica.
  - Dopo, si tengono solo gli ID con più nomi (115.231 sul PC di sviluppo).
  - Per la stima si rileggono gli attributi dei file selezionati.
  - D5 va ricalcolato con nomi lunghi. Obiettivo ≤ 400 MB per 3,3 milioni di voci, da misurare in M1.
- **M0 ricalibrato per uso personale.** Criteri bloccanti:
  - tabella da 3,5 milioni di righe con scroller a indice, frame p95 e p99 ≤ 16 e 33 ms;
  - mappa con 20.000 aree, frame p95 ≤ 16 ms;
  - testo a 100% e 150% confrontato con ClearType nativo tramite screenshot affiancati da approvare;
  - tastiera completa;
  - ripiego sul renderer WARP funzionante.
  
  Assistente vocale e RDP non sono cancelli: il proprietario del progetto deve confermare che non li usa. Il piano B (WinUI 3, con Slint come alternativa da valutare) si decide solo se M0 fallisce, e si abbandona se egui passa.
- **Milestone.** Il bilancio per gestore e le righe "spiegate" passano a M3 (C9).

## 9. Test aggiunti

- **Usabilità a compiti.** Con il proprietario del progetto e, se possibile, una persona non esperta. Cinque compiti:
  1. trova cosa occupa più spazio;
  2. spiega perché AppData è grande;
  3. decidi sui file di Download;
  4. trova un programma grande e dici cosa sai del suo uso;
  5. esegui un intervento e verificalo.
  
  Criteri: completamento senza aiuto, tempo, e interpretazione corretta degli stati, verificata con domande di comprensione (es. "Da verificare significa che puoi cancellare?" deve avere risposta no).
- **Protezione delle operazioni distruttive.**
  - Un piano confermato si costruisce solo dalla conferma ed è un tipo dedicato (V6).
  - Test di proprietà con selezioni casuali che includono elementi protetti: devono essere sempre respinte.
- Altri test:
  - stima contro delta reale su VHDX;
  - Cestino oltre la capacità;
  - lettura di Sensore memoria;
  - Restart Manager con file in uso;
  - riferimenti a versioni spostate;
  - corrispondenza installatori e programmi;
  - hash solo su richiesta;
  - confronto zip e cartella;
  - nessuna idratazione nella lente Download.
- Via: CI multi file system, fuzzing della MFT, test della privacy a ogni release. Restano `cargo-deny`, il controllo delle importazioni e una prova di sessione senza rete prima della v1.

## 9 bis. Decisioni dalle prove raccolte in Windows (`prove-windows.md`)

- **Uso dei programmi: si usano solo due fonti.**
  - LastPlayed di Steam: sul PC di sviluppo c'è per quasi tutti i giochi installati e concorda con gli `.acf`.
  - UserAssist: si legge solo se `Start_TrackProgs` = 1. Sul PC di sviluppo vale 0 e i dati sono fermi a una data di oltre un anno prima, quindi la UI dice "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso" senza dedurre altro.
  - Sono esclusi BAM, Prefetch, Amcache e SRUM, perché richiedono l'amministratore e hanno un sapore forense; è escluso anche LastAccess, che è rumore.
  - Epic non ha un campo di ultimo utilizzo. Per MSIX non esiste un'API.
- **Ultimo accesso.** Su C: l'aggiornamento è attivo, anche se `fsutil` scrive "Disabled": leggere i file ne aggiorna la data di ultimo accesso. Per questo le letture di contenuto (risorse di versione, hash, directory centrale degli zip) avvengono solo su richiesta e la UI le dichiara ("aggiorna la data di ultimo accesso di questi file"). Quado non apre handle in scrittura per evitarlo. Effetto collaterale noto: Sensore memoria potrebbe rinviare la pulizia di quei file da Download.
- **Costo dei metadati.** Leggere la versione di alcune decine di installatori costa 18-25 s, quasi tutto per la scansione all'apertura di Defender. Si fa in background, su richiesta, con una cache per (percorso, dimensione, data di modifica, ID file).
- **MSI.** Si leggono con `MsiOpenDatabase` in sola lettura e una query sulla tabella Property; mai `MsiOpenPackage`. Il ProductCode coincide con la chiave di disinstallazione nella maggior parte dei casi provati: corrispondenza "da fonte".
- **Fuori dalla v1:**
  - firma Authenticode: richiede un'apertura per file e, se verificata, rischia la rete;
  - Zone.Identifier con HostUrl: rivela da dove si è scaricato e copre solo circa un installatore su otto;
  - marcatori NSIS e Inno: costano letture e restano ambigui.
- **Copie identiche.** Con l'hash, su tutta la cartella Download si trovano oltre cento gruppi e alcuni GiB recuperabili, circa un gruppo su cinque con nomi diversi. Il suffisso "(N)" da solo non vale come prova: delle centinaia di copie con il suffisso, circa tre su quattro hanno una dimensione diversa dall'originale. L'hash di tutta Download richiede circa 74 s, ed è accettabile su richiesta.
- **Cestino sul PC di sviluppo.** `MaxCapacity` di C: vale circa il 5% dell'unità, in `HKCU\...\BitBucket\Volume\{GUID}`. Gli orfani sono alcune cartelle `$R` per decine di GiB, che `SHQueryRecycleBin` non conta. Sensore memoria, attivo con la regola sul Cestino, non rimuove gli orfani. Prima di M5 va verificato con un VHDX se la capacità usata da Windows per eliminare gli elementi più vecchi conta solo le coppie `$I`/`$R`.
- **Copie shadow.** Il copy-on-write è documentato: lo spazio liberato può essere riassorbito [I]. Le loro statistiche richiedono l'amministratore e quindi stanno solo nell'helper.
- **Residui.** Con l'euristica per nome, circa un terzo delle cartelle non corrisponde a nessun programma, ma una ventina di queste sono state modificate negli ultimi 90 giorni: falsi positivi. Lo stato resta "Non sappiamo", mostrando la data dell'ultima modifica e il testo "Potrebbe essere un residuo".
- **Disinstallazione.**
  - Per i programmi Win32 non esiste un URI documentato che apra la pagina della singola app: si usa `ms-settings:appsfeatures` copiando il nome negli appunti, oppure `appwiz.cpl`.
  - Per i pacchetti MSIX: `ms-settings:appsfeatures-app?<PFN>`.
  - Steam: `steam://uninstall/<appid>`. Epic: la libreria del launcher.
  - Mai `UninstallString` né `QuietUninstallString`: qualche decina di voci in HKCU sono comandi scrivibili dall'utente, e più di dieci voci `MsiExec /I` aprono la manutenzione invece di disinstallare.
- **Verifica.** Si mostrano due numeri distinti: "stimato" (somma delle allocate) e "variazione del volume". Il secondo include l'attività di altri processi; sul PC di sviluppo Sensore memoria, SilentCleanup e Defender sono girati il 12/10/2026, il giorno stesso delle misure.

## 9 ter. Elenco canonico dei requisiti v1 (ID da usare in tutti i documenti)

**Scansione (F-SC)**

| ID | Requisito |
|---|---|
| F-SC-1 | Elenco dei volumi |
| F-SC-2 | Scansione non elevata in sola lettura |
| F-SC-3 | Completamento con privilegi su richiesta: helper che enumera le cartelle negate con `SeBackupPrivilege`, raccoglie lo spazio delle copie shadow e i cestini di altri SID, e restituisce i profili altrui in forma aggregata |
| F-SC-4 | Avanzamento |
| F-SC-5 | Annullamento e risultato parziale |
| F-SC-6 | Cartelle negate elencate |
| F-SC-7 | Hard link e reparse point |
| F-SC-8 | Dimensione logica e allocata, con correzione dei file WOF |
| F-SC-9 | Segnaposto cloud |
| F-SC-10 | File system: NTFS completo; exFAT e FAT32 sulle unità rimovibili; gli altri "senza garanzie", dichiarato |

**Panoramica (F-OV)**

| ID | Requisito |
|---|---|
| F-OV-1 | Bilancio secondo la definizione in 03 §6.4 |
| F-OV-2 | Top list di soli elementi, con regola di non sovrapposizione |
| F-OV-3 | Card dei conti: osservato e possibile |
| F-OV-4 | Card interventi, solo da record di raccomandazione |

**Esplora (F-EX)**

| ID | Requisito |
|---|---|
| F-EX-1 | Mappa |
| F-EX-2 | Tabella virtualizzata |
| F-EX-3 | Navigazione e tastiera |
| F-EX-4 | Ricerca e filtri |
| F-EX-5 | Vista File grandi |

**Ispettore (F-IN)**

| ID | Requisito |
|---|---|
| F-IN-1 | Fatti con provenienza ciascuno |
| F-IN-2 | Perché è grande |
| F-IN-3 | Tipo dall'estensione, senza leggere il contenuto |
| F-IN-4 | Cosa puoi fare, dal record di raccomandazione |
| F-IN-5 | Apri in Esplora file, Copia percorso, Proprietà |
| F-IN-6 | Verifica questa voce |

**Lente Download (F-DL)**

| ID | Requisito |
|---|---|
| F-DL-1 | Famiglie per tipo, su qualunque cartella dell'utente |
| F-DL-2 | Metadati degli installatori su richiesta (risorsa di versione PE, Property degli MSI in sola lettura), in background e con cache; corrispondenza con i programmi installati |
| F-DL-3 | Suffisso "(N)" solo come filtro |
| F-DL-4 | Copie identiche con hash su richiesta, poi Gestibile |
| F-DL-5 | Archivio confrontato con la cartella estratta (directory centrale dello zip) |
| F-DL-6 | Download incompleti non in uso |

**Programmi (F-AP)**

| ID | Requisito |
|---|---|
| F-AP-1 | Elenco: registro, MSIX, Steam, Epic |
| F-AP-2 | Spazio solo con prova del percorso |
| F-AP-3 | Dichiarato contro misurato |
| F-AP-4 | Uso noto: Steam LastPlayed; UserAssist solo se il tracciamento è attivo; InstallDate; mai "inutilizzato" |
| F-AP-5 | Disinstallazione tramite interfaccia ufficiale, mai `UninstallString`; avviso se nella cartella ci sono tuoi file |
| F-AP-6 | Residui: stato "Non sappiamo" con data di ultima modifica |

**Windows e sistema (F-SY)**

| ID | Requisito |
|---|---|
| F-SY-1 | Vista Windows e sistema |
| F-SY-2 | Cestino misurato in due modi, orfani come anomalia |
| F-SY-3 | Configurazione in sola lettura: ibernazione e avvio rapido, Sensore memoria, capacità del Cestino, tracciamento degli avvii, ripristino se leggibile |

**Interventi (F-AC)**

| ID | Requisito |
|---|---|
| F-AC-1 | Tipi di intervento |
| F-AC-2 | Simulazione obbligatoria, con piano confermato come tipo dedicato |
| F-AC-3 | Conferma, Esc e riepilogo |
| F-AC-4 | Elementi protetti, con eccezione documentata solo via regola Da verificare con controlli |
| F-AC-5 | Mai eliminazione definitiva (`PreDeleteItem`) |
| F-AC-6 | Registro con stimato e ottenuto |
| F-AC-7 | Capacità del Cestino e Sensore memoria |
| F-AC-8 | Restart Manager |
| F-AC-9 | Riferimenti nel registro, nei collegamenti e nelle attività pianificate, quando la regola lo richiede |

**Verifica (F-VE)**

| ID | Requisito |
|---|---|
| F-VE-1 | Misura di partenza |
| F-VE-2 | Nuove misure dopo l'azione, dopo lo svuotamento e dopo "Ho eseguito la procedura" |
| F-VE-3 | Stimato contro ottenuto, con spiegazione oltre il 5% |
| F-VE-4 | Aggiornamento del modello con riscansione dei sottoalberi |
| F-VE-5 | Vista Interventi |

**Generali (F-GE)**

| ID | Requisito |
|---|---|
| F-GE-1 | Interfaccia in più lingue: italiano e inglese, altre aggiungibili (05 D13) |
| F-GE-2 | Temi e contrasto |
| F-GE-3 | Unità come Esplora file sul singolo file, SI a scelta |

La palette dei comandi passa alla v1.1. La modalità portable è tolta. La lingua inglese, tolta in revisione, è tornata con la decisione del 13/10/2026, insieme alla possibilità di aggiungere altre lingue (05 D13).

**Milestone v2**

| Milestone | Contenuto |
|---|---|
| M0 | Verifica dello stack |
| M1 | Scansione, modello, bilancio, misure di riferimento e verifiche tecniche: Cestino su VHDX (orfani, `PreDeleteItem`, conteggio della capacità), `/DEPENDENTLOADFLAG`, comando dell'archiviazione riservata, memoria con nomi reali |
| M2 | UI di base: Panoramica con bilancio non classificato e top list, Esplora, File grandi, ispettore delle misure, temi |
| M3 | Classificazione, record di raccomandazione e cinque stati, vista Windows, prime regole, bilancio per gestore |
| M4 | Lente Download e Programmi |
| M5 | Interventi e verifica |
| M6 | Helper con privilegi, rifinitura, test di usabilità, prova senza rete, build locale |

## 10. Coerenza da sistemare nei documenti

Correggere C5 (Pulizia disco: ricontrollare la fonte S38 e preferire Impostazioni), C6, C10-C15, C17, C18, U3, U7, U8, U9 e i riferimenti incrociati di C12. Tutte le contraddizioni elencate nella revisione indipendente devono risultare risolte o motivate in `10-revisione-critica.md`.
