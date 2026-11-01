# 02 · Studio tecnico e fattibilità

Stato al 12/10/2026. Legenda: [V] documentazione ufficiale (Microsoft Learn, specifiche MS-*, documentazione del tool), [F] fonte non ufficiale (blog, produttore, community), [M] misurato sul PC di sviluppo, in sola lettura, [I] mia deduzione, [?] non trovato o ambiguo.

Rivisto dopo la revisione del 12/10/2026 (`10-revisione-critica.md`): riferimenti incrociati corretti, §8 e §14 allineati alle decisioni in `revisione/decisioni-revisione.md`, aggiunta la §15 sulle prove per i nuovi scenari. Le misure citate qui sono state prese il 12/10/2026 sul PC di sviluppo, in sola lettura.

Macchina di prova: Windows 10 Home 22H2, portatile a 6 core e 12 thread, C: NTFS da circa 1 TB su SSD NVMe, Defender attivo, sessione non elevata. Nessuna impostazione è stata modificata; i file di prova stanno solo in una cartella temporanea.

## 1. Sintesi

1. Senza privilegi l'unica strategia completa è enumerare cartella per cartella. Conviene `GetFileInformationByHandleEx` con `FileIdExtdDirectoryInfo`, che restituisce dimensione logica, allocata, attributi, tag di reparse e ID file a 128 bit senza aprire i file [V]. Misurato: quasi 3 milioni di file in 16-26 s con 8-12 thread [M].
2. La lettura diretta della MFT e `FSCTL_ENUM_USN_DATA` richiedono privilegi di amministratore: da utente standard restituiscono accesso negato [M].
3. Gli hard link gonfiano la somma ingenua: 13,6 GB (10^9) sul volume, il 22% su `C:\Windows` (35,77 contro 28,04 GB 10^9) [M].
4. I file compressi WOF compaiono nella voce di cartella con allocata 0 e reparse point nascosto. Capita anche con app di terzi: i `.pak` di Chrome e Brave [M]. Nessuna documentazione descrive questo comportamento [?].
5. Le dimensioni nelle voci di cartella possono essere vecchie per i file aperti in scrittura [M] [F Raymond Chen 2011].
6. Senza privilegi si misurano anche la `$MFT` (3,29 GB 10^9) e gli indici delle cartelle (1,38 GB 10^9). Il residuo davvero non attribuito è circa 26,9 GB (10^9), il 3,4% dell'usato [M]. La definizione formale è in 03 §6.4.
7. Nella cartella del Cestino dell'utente ci sono cartelle orfane per decine di GiB, senza il file indice `$I`: sono alcune cartelle `$R` (§15.4). Il Cestino di Windows non le vede: `SHQueryRecycleBinW` riporta pochi MB [M].

## 2. Come ottenere i metadati

| Strategia | Logica | Allocata | Hard link | Tag reparse | Privilegi | Velocità |
|---|---|---|---|---|---|---|
| `FindFirstFileExW` (Basic, `LARGE_FETCH`) | sì | no | no | sì (`dwReserved0`) | no | 3,28 M voci in 86 s, 1 thread [M] |
| `GetFileInformationByHandleEx(FileIdExtdDirectoryInfo)` o `NtQueryDirectoryFileEx` | sì | sì | ID file (dedup), non il numero di link | sì, campo dedicato | no | 79 s con 1 thread, 16,6 s con 8 [M] |
| `FSCTL_ENUM_USN_DATA` | no | no | FRN e FRN del genitore | solo attributi | sì [M] | circa 1 M file al minuto per Everything [F] |
| Lettura diretta della `$MFT` | sì | sì, anche flussi e residenti | sì | sì | sì [M] | la più veloce [F] |
| `FSCTL_QUERY_FILE_LAYOUT` | sì, per stream | sì, per stream | sì | parziale | handle di volume, di fatto sì [I] | non misurata |
| USN journal | no | no | variazioni | variazioni | sì per la documentazione [V] | incrementale |
| `ReadDirectoryChangesExW` | sì | sì | no | sì | no | osservazione dal vivo |

Dettagli che contano:
- `FILE_ID_EXTD_DIR_INFORMATION` ha EndOfFile, AllocationSize, FileAttributes, ReparsePointTag e FileId a 128 bit; "No specific access rights are required" [V].
- `std::fs::read_dir` di Rust non espone l'allocata; `number_of_links` e `file_index` restano `None` dalle voci di cartella [V]. Servono chiamate Win32 dirette.
- `FileStandardInfo` sull'handle della cartella, già aperto per l'enumerazione, dà l'allocazione dell'indice `$I30` quasi gratis (17,2 s contro 16,6 s) [M]. L'enumerazione riporta 0 per le cartelle.
- Aprire ogni file costa caro: 11.800 file/s contro 50.000 dell'enumerazione a cache calda, 6.400 sul profilo utente con Defender attivo, che analizza i file all'apertura [M] [V].
- `USN_RECORD_V2/V3` non hanno campi di dimensione [V]: con USN si costruisce l'albero dei nomi, non si misura.
- Lettura della MFT: `FSCTL_GET_NTFS_VOLUME_DATA` per posizione e dimensione, `FSCTL_GET_RETRIEVAL_POINTERS` sul file `$MFT`, poi i record (firma "FILE", fixup) con `$STANDARD_INFORMATION`, `$ATTRIBUTE_LIST`, `$FILE_NAME`, `$DATA` [F flatcap]. Le dimensioni dentro `$FILE_NAME` si aggiornano solo al rinomina e vanno lette da `$DATA` [F].
- `FSCTL_QUERY_FILE_LAYOUT` (da Windows 8.1 Update, non su ReFS) restituisce nomi, stream e allocazione per stream [V]. È l'alternativa documentata al parsing diretto per la modalità con privilegi [I].
- USN per gli aggiornamenti incrementali: salvare (numero di serie del volume, JournalID, NextUsn). Alla ripartenza, se il JournalID è lo stesso e il NextUsn salvato è ancora nel journal si applica il delta, altrimenti si riscansiona [V] [I]. Sul PC di sviluppo il journal da 32 MB copre circa 27 ore [M]: un indice più vecchio di un giorno va ricostruito. È uno dei motivi per cui l'indice persistente è rinviato alla v2.
- `FSCTL_QUERY_USN_JOURNAL` e `FSCTL_READ_UNPRIVILEGED_USN_JOURNAL` funzionano da utente standard (il secondo senza nomi), anche se la documentazione dice che il journal richiede amministratori [M] [V].

### 2.1 La sonda di questa fase

Per avere numeri nostri ho scritto una sonda usa e getta in Rust (non fa parte del progetto) che enumera C: con `GetFileInformationByHandleEx(FileIdExtdDirectoryInfo)` e `rayon` su 12 thread, buffer da 64 KiB, senza seguire i reparse point e deduplicando gli hard link per ID file.

| Esecuzione | Tempo | Picco di memoria |
|---|---|---|
| Prima esecuzione | 26,32 s | 97 MB |
| Seconda esecuzione | 17,13 s | 102 MB |
| Terza esecuzione | 16,12 s | |
| `std::fs::read_dir` con gli stessi thread (solo dimensione logica) | 19,81 s | 17 MB |

Risultati comuni:
- quasi 3 milioni di file e circa mezzo milione di cartelle;
- 113 cartelle negate e 8 altri errori;
- 4.473 cartelle con reparse point non seguite (4.470 junction o mount point, 180 symlink in totale tra file e cartelle, 26 APPEXECLINK);
- somma logica e somma allocata oltre 700 GB, con l'allocata più alta di 4,16 GB; hard link 115.231 file con più nomi e 174.932 nomi in più (13,56 GB contati due volte);
- allocata deduplicata pari a circa il 96% dell'usato del volume, che supera i 700 GB (unità 10^9);
- deduplicazione: 0,3-0,5 s per ordinare quasi 3 milioni di ID.

La memoria della sonda include il vettore degli ID per la deduplicazione: l'app avrà in più nomi e struttura dell'albero (stima in 05 D5).

## 3. Cosa significa "dimensione"

- Logica (EndOfFile): byte del flusso principale. Allocata: cluster assegnati.
- `GetCompressedFileSizeW` restituisce la dimensione compressa per i file compressi, la parte allocata per gli sparse e la dimensione logica per tutti gli altri [V]. Non è quindi la "dimensione su disco" di un file normale.
- Esplora file usa `GetCompressedFileSize` solo per compressi e sparse, altrove arrotonda al cluster [F], e da Windows 8.1 include i flussi alternativi [F].

Matrice misurata su file da 1.000.000 di byte, cluster 4 KB [M]:

| File | Voce di cartella (logica / allocata) | `FileStandardInfo` | `GetCompressedFileSizeW` |
|---|---|---|---|
| normale | 1.000.000 / 1.003.520 | 1.003.520 | 1.000.000 |
| compresso NTFS | 1.000.000 / 126.976 | 126.976 | 126.976 |
| WOF XPRESS4K | 1.000.000 / 0, tag nascosto | 73.728 | 73.728 |
| 100 byte, residente nella MFT | 100 / 104 | 104 | 100 |
| 4 byte con flusso alternativo da 500.000 | 4 / 8 | 8 | 4 |
| sparse da 100 MB con 64 KB scritti | 104.857.600 / 0 subito, 65.536 dopo il flush | idem | idem |
| hard link con 2 nomi | identici, stesso ID | 2 link | - |

Conseguenze:
- I file residenti hanno allocata pari alla logica arrotondata a 8 byte, ma lo spazio reale è nel record della MFT. Su C: sono circa 709.000 [M]. Sommarli insieme alla MFT è un piccolo doppio conteggio da dichiarare [I].
- I flussi alternativi sono invisibili sia all'enumerazione sia a `FileStandardInfo` [V]. Sul profilo utente: decine di migliaia di file con flussi alternativi, quasi tutti `Zone.Identifier`, per meno di 2 MB in tutto [M]. Scoprirli costa un'apertura per file: li conta solo la modalità con privilegi.
- WOF: `wof.sys` nasconde attributo e tag di reparse anche a chi apre con `FILE_FLAG_OPEN_REPARSE_POINT` [M]. Lo rivelano solo `FileStandardInfo`, `GetCompressedFileSizeW` o `WofIsExternalFile` [V]. Con CompactOS spento, sul PC di sviluppo sono 123 file di Chrome e Brave (`af.pak`: 563.263 byte logici, 192.512 allocati) [M]. Correzione: per logica > 0, allocata 0 e niente sparse, offline o cloud, aprire con `FILE_READ_ATTRIBUTES` e leggere `FileStandardInfo` [I], verificato su tre file.

## 4. Hard link, reparse point e cicli

- Al massimo 1023 nomi per file, sullo stesso volume. Dimensione e attributi si aggiornano solo nella voce del nome usato per la modifica [V].
- Identità: (numero di serie del volume, ID a 128 bit). Su ReFS l'ID a 64 bit non è garantito unico [V].
- L'ID restituito da `FileIdExtdDirectoryInfo` è lo stesso per tutti i nomi: la deduplicazione non richiede aperture [M].
- Fonti di hard link: WinSxS (Esplora la sovrastima, la misura ufficiale è `DISM /AnalyzeComponentStore` [V]), pnpm dallo store a `node_modules` [V], file identici tra pacchetti MSIX [V].
- Regola di attribuzione: i totali si calcolano sugli ID unici. Il dato condiviso si attribuisce al percorso con priorità più alta (per esempio System32 prima di WinSxS) e lo si dichiara come condiviso [I].

| Tag di reparse | Valore | Politica |
|---|---|---|
| MOUNT_POINT | 0xA0000003 | non seguire; un volume montato si scansiona a parte |
| SYMLINK, LX_SYMLINK | 0xA000000C, 0xA000001D | non seguire |
| APPEXECLINK | 0x8000001B | file da 0 byte |
| CLOUD, CLOUD_1…F | 0x9000001A, 0x9000x01A | segnaposto, vedi §5 |
| WOF | 0x80000017 | nascosto dal filtro, vedi §3 |
| DEDUP | 0x80000013 | logica molto maggiore dell'allocata |
| WCI, PROJFS | 0x80000018, 0x9000001C | contenitori e proiezioni: elencare può far comparire contenuti |
| AF_UNIX | 0x80000023 | ignorare |

Valori: [V MS-FSCC]. Regola generale: mai seguire i tag con il bit "name surrogate" (`IsReparseTagNameSurrogate`) [V]. Le junction di compatibilità come "Default User" hanno un'ACE "Everyone Deny" proprio per fermare chi le percorre [V] [M]. Difese contro i cicli: non seguire i surrogati, insieme delle cartelle visitate per identità, pila esplicita invece della ricorsione.

## 5. Segnaposto cloud

| Attributo | Valore | Significato |
|---|---|---|
| OFFLINE | 0x1000 | dati spostati altrove |
| RECALL_ON_OPEN | 0x40000 | elemento virtuale, solo in enumerazione |
| PINNED / UNPINNED | 0x80000 / 0x100000 | da tenere / da non tenere in locale |
| RECALL_ON_DATA_ACCESS | 0x400000 | leggere il file, o elencare la cartella, richiama dati dal remoto |

Fonte: [V file attribute constants].
- `CfGetPlaceholderStateFromAttributeTag` dà lo stato usando solo i dati dell'enumerazione [V]. `CfGetPlaceholderInfo` su un handle con soli attributi dà i byte presenti in locale [V].
- La politica di richiamo si decide all'apertura per i dati, e ai filtri è chiesto di non trasformare le aperture "solo attributi" in letture [V]. Quindi enumerare e aprire con soli attributi non idrata [I]. Mai aprire con `FILE_READ_DATA`, mai calcolare hash, mai leggere i flussi alternativi di un segnaposto.
- Eccezione: elencare una cartella con `RECALL_ON_DATA_ACCESS` può scaricarne l'elenco [V]. Per default non le elenchiamo.
- `FILE_FLAG_OPEN_NO_RECALL` è documentato, ma non è documentato che il filtro Cloud Files lo rispetti [?]: lo usiamo comunque come protezione aggiuntiva.
- All'avvio si imposta `RtlSetProcessPlaceholderCompatibilityMode(PHCM_EXPOSE_PLACEHOLDERS)` [V].
- PINNED e UNPINNED da soli non indicano un segnaposto: sul PC di sviluppo qualche migliaio di file installati da programmi comuni li hanno senza essere nel cloud [M].
- Il PC di sviluppo non usa OneDrive, quindi il test di non idratazione si fa con un provider di prova (06).

## 6. Cartelle negate e spazio non attribuito

Negate all'utente standard sul PC di sviluppo [M]: WindowsApps, System32\config, Windows\Temp, SystemTemp, Prefetch, Minidump, LiveKernelReports, Search\Data, ServiceState, Network\Downloader, la junction "Default User", le cartelle del Cestino di altri SID (in parte SID orfani), System Volume Information. In tutto 113 cartelle.
- System Volume Information è negata anche agli amministratori [V]; si legge con `SeBackupPrivilege` più `FILE_FLAG_BACKUP_SEMANTICS` [V], privilegio da abilitare esplicitamente [V].
- Leggibili invece: Windows\Installer (4,07 GB 10^9), WinSxS (9,69 GB 10^9 unici), DriverStore\FileRepository (6,33 GB 10^9), WER, SoftwareDistribution [M].

Calcolo del non attribuito: (totale - libero) da `GetDiskFreeSpaceExW` [V], meno allocate uniche, indici e metadati misurati. È approssimato perché il volume cambia durante la scansione [I]: per questo l'usato si misura all'inizio e alla fine della scansione e la differenza si mostra come incertezza (definizione in 03 §6.4). La tabella usa un usato letto in un momento diverso da quello di §2.1 (0,32 GB in meno): il volume cambia tra una lettura e l'altra.

| Voce, PC di sviluppo | Byte (10^9) |
|---|---|
| Usato dal volume | oltre 700 GB |
| File unici per ID | circa il 96% dell'usato |
| Indici delle cartelle | 1,38 GB |
| $MFT | 3,29 GB |
| $UsnJrnl | circa 0,04 GB |
| Residuo non attribuito | circa 26,9 GB (3,4%) |

Il residuo può contenere copie shadow, il contenuto delle 113 cartelle negate, cestini di altri SID, archiviazione riservata e altri metafile [I]. Nessuna di queste voci è misurabile senza privilegi [M]. Nell'interfaccia non compaiono come cause ma in due liste: "Osservato" (cartelle negate, cestini di altri SID) e "Possibile, non misurabile senza amministratore" (copie shadow, archiviazione riservata, altri metafile), come stabilito per F-OV-3.

## 7. Perché lo spazio libero non torna

| Causa | Come misurarla | Privilegi | Note |
|---|---|---|---|
| `$MFT` | `MftValidDataLength` da `FSCTL_GET_NTFS_VOLUME_DATA` sull'handle della radice | no [M] | non si restringe mai [V] |
| Indici delle cartelle | `FileStandardInfo` sull'handle di ogni cartella | no [M] | 1,38 GB (10^9) qui |
| `$UsnJrnl` | `FSCTL_QUERY_USN_JOURNAL` | no (misurato) | |
| `$LogFile`, `$Bitmap`, `$Secure` | MFT diretta | sì | `$Bitmap` circa 30 MB (10^6) qui [I] |
| Copie shadow e punti di ripristino | `vssadmin list shadowstorage`, `Win32_ShadowStorage` | sì [V] [M] | in Windows 11 per default 2%, minimo 2 GB, massimo 50 GB [V] |
| hiberfil, pagefile, swapfile | enumerazione della radice | no | circa 20 GB (10^9) qui; le API sul singolo file danno violazione di condivisione [M] |
| Archiviazione riservata | `DISM /Online /Get-ReservedStorageState` | sì (errore 740 qui) | circa 7 GB [V] |
| File eliminati ma ancora aperti | enumerazione degli handle | sì [I] | lo spazio resta occupato fino alla chiusura [V] |
| Dati di altri utenti, ACL | conteggio delle cartelle negate | - | mostrati come "non leggibile", mai come zero |
| Spreco di cluster | allocata - logica | no | 3,84 GB (10^9) qui |
| Hard link | deduplica per ID | no | -13,6 GB (10^9) qui |
| Compressione e WOF | allocata < logica | no, apertura per i WOF | §3 |
| Segnaposto cloud | attributi e byte locali | no | la logica non è spazio occupato |
| Cestino | `SHQueryRecycleBinW` per unità (cbSize 24 su x64) e scansione di `$Recycle.Bin` | no, solo utente corrente | le cartelle `$R` senza `$I` non sono contate da Windows: decine di GB qui, §15.4 |
| Windows.old | dimensione della cartella | sì per rimuoverla | si elimina da sola dopo 10 giorni [V] |
| Quote | `FreeBytesAvailableToCaller` contro `TotalNumberOfFreeBytes` | no | |

## 8. Consumi di Windows e gestione ufficiale

| Elemento | Procedura ufficiale | Note |
|---|---|---|
| WinSxS | `Dism /Online /Cleanup-Image /StartComponentCleanup`; `/ResetBase` solo come alternativa | mai a mano: il PC può non avviarsi [V]. Con `/ResetBase` gli aggiornamenti installati non si possono più disinstallare [V] |
| Windows\Installer | nessuna | "should not be deleted"; il ripristino può richiedere di reinstallare Windows [V] |
| SoftwareDistribution | Impostazioni › File temporanei (Pulizia di Windows Update) | il reset serve solo alla risoluzione dei problemi [V]; Quado non la tocca a mano |
| Ottimizzazione recapito | gestita da Windows, scadenza 3 giorni, massimo 20% [V] | negata all'utente standard [M] |
| Windows.old | Impostazioni › Archiviazione › File temporanei; `DISM /Set-OSUninstallWindow` | [V] |
| Cestino | dialogo di Windows "Svuota Cestino" (`SHEmptyRecycleBinW`) | capacità predefinita: 10% dei primi 40 GB più 5% del resto [F]; dettagli e misure in §15.4 |
| File temporanei, dump, WER, miniature | Impostazioni › File temporanei; in alternativa Pulizia disco | [V]; Quado non li cancella mai a mano |
| Indice di Windows Search | Opzioni di indicizzazione › Ricostruisci | [V] |
| Archivio driver | `pnputil /enum-drivers`, `/delete-driver oemNN.inf` | non toccare i file [V]; 6,33 GB (10^9) qui |
| Ibernazione | `powercfg /hibernate /type reduced` oppure `powercfg /h off`, entrambi da amministratore | `/type reduced` toglie l'ibernazione e mantiene l'avvio rapido; `/h off` toglie ibernazione e avvio rapido, e con loro la sospensione ibrida [V] |
| File di paging | Memoria virtuale | dimensionamento automatico documentato [V] |
| Punti di ripristino | `vssadmin resize shadowstorage` | amministratore [V] |
| Sensore memoria | Impostazioni o criteri di gruppo | [V] |
| Pulizia disco | `cleanmgr /d`, `/sageset`, `/sagerun` | mantenuta in Windows [V] (01 §2.6, fonte S38); Quado preferisce Impostazioni › File temporanei e la cita come alternativa |

I gestori di Pulizia disco (`IEmptyVolumeCache`, 29 registrati qui [M]) sono pensati per il gestore di pulizia di Windows [V]. Nessuna documentazione prevede che un'app esterna li ospiti [?]: farlo sarebbe un comportamento non documentato, con cancellazioni immediate. Quado mostra la stima e rimanda a Impostazioni › File temporanei, con Pulizia disco come alternativa.

## 9. Sviluppo e macchine virtuali

| Elemento | Dove | Comando ufficiale | Fonte |
|---|---|---|---|
| WSL 2 | `BasePath` in `HKCU\...\Lxss\{GUID}`, file `ext4.vhdx` | `wsl --manage <distro> --set-sparse true`; `diskpart compact vdisk` | [V]. La pagina WSL aggiornata il 02/06/2026 non tratta più la compattazione, e con WSL 2.5.8 `--set-sparse` risultava disabilitato per rischio di corruzione [F]: va presentato come da verificare sulla versione installata |
| Docker Desktop | `%LOCALAPPDATA%\Docker\wsl\disk\docker_data.vhdx` | `docker system df`, `docker system prune`, `docker builder prune`, "Clean up data" | [V]. Lo spazio liberato dentro il disco virtuale torna su C: solo dopo la compattazione del VHDX [V WSL]; Quado non mostra mai l'opzione `--volumes`, che elimina dati |
| Hyper-V | file `.vhdx` | `Optimize-VHD` (non disponibile su Home) | [V] |
| VirtualBox, VMware | `.vdi`, `.vmdk` | `VBoxManage modifymedium --compact`; Compact | [V] |
| Android | `%LOCALAPPDATA%\Android\Sdk`, `.android\avd` | `sdkmanager --uninstall`, Device Manager di Android Studio | [V]; alcuni GiB di AVD qui |
| npm | `%LocalAppData%\npm-cache`, oppure il percorso di `npm config get cache`, `.npmrc` o variabile d'ambiente | `npm cache verify` (preferito), `npm cache clean --force` | [V]; alcuni GB qui. Dopo `clean --force` npm riscarica i pacchetti e le installazioni offline non trovano più la copia locale [I] |
| pnpm | `%LOCALAPPDATA%\pnpm\store` | `pnpm store prune` | [V]; alcuni GB qui. Lo store è collegato con hard link ai `node_modules`: lo spazio recuperabile non si stima senza contare i link [V] [I] |
| Yarn, Cargo, pip, uv, conda, NuGet, Gradle, Maven | cache documentate | `yarn cache clean`, `cargo clean`, `pip cache purge`, `uv cache prune`, `conda clean --all`, `dotnet nuget locals all --clear` | [V] |
| Visual Studio | `%ProgramData%\Microsoft\VisualStudio\Packages` | `vs_installer --nocache` | [V]; alcuni GB qui |
| JetBrains | `%LOCALAPPDATA%\JetBrains\<prodotto><versione>` | Delete Leftover IDE Directories | [V] |

## 10. Attribuire cartelle ai programmi

| Livello | Fonte | Affidabilità |
|---|---|---|
| Da fonte, sistema | MSIX: `PackageManager.FindPackagesForUser("")` e `InstalledLocation`; dati in `%LOCALAPPDATA%\Packages\<PFN>` | alta [V]; un centinaio di pacchetti qui [M] |
| Da fonte, launcher | Steam `libraryfolders.vdf` e `appmanifest_*.acf`; Epic `Manifests\*.item` | alta, ma formati non documentati dai produttori [F] |
| Da fonte, registro | Uninstall in HKLM, WOW6432Node, HKCU; `InstallLocation` è facoltativa [V] | media: su alcune centinaia di voci con nome, solo circa un quarto ha `InstallLocation` e poco più di un terzo un percorso utilizzabile (anche da `UninstallString` o `DisplayIcon`) [M] |
| Dedotto | nome della cartella simile al nome del programma | bassa [F] |

Evidenze misurate:
- Steam: `SizeOnDisk` coincide al byte con la somma logica per quasi tutti i giochi confrontati; dove differisce, la differenza è di pochi MB [M]. È quindi una dimensione logica e può essere vecchia.
- Epic: Quixel Bridge e il plugin Fab hanno la stessa `InstallLocation` di Unreal Engine [M]. Regola: un solo proprietario per byte, prefisso più lungo.
- `EstimatedSize` è in KB e da Windows 7 non è più calcolata dal sistema [F]: solo come confronto, sempre etichettata.
- CapCut: la voce HKCU "CapCut" non ha `InstallLocation`, ma `UninstallString` punta a `AppData\Local\CapCut\Apps\uninst.exe`, e `Apps\ProductInfo.xml` dichiara la stessa versione della voce [M]. Con due fonti concordi la versione in uso è "da fonte"; che le cartelle delle versioni precedenti non servano più resta "dedotto". Dopo la revisione la regola tiene anche la versione precedente a quella in uso: le candidate sono tutte le cartelle di versione tranne le ultime due, con stato Da verificare e controlli dedicati (03 §6, 09).
- Cartelle note: `SHGetKnownFolderPath`, compreso `FOLDERID_UserProgramFiles` [V]. Lo spostamento delle cartelle note su OneDrive porta segnaposto dentro Desktop, Documenti e Immagini [V]; qui nessuna è reindirizzata [M].

## 11. File system diversi

| File system | Allocata | Hard link | Flussi alternativi | USN / MFT | Note |
|---|---|---|---|---|---|
| NTFS | sì | sì | sì | sì | caso principale |
| ReFS, Dev Drive | sì, ma il block cloning condivide i cluster [V]; come lo riporti `AllocationSize` non è documentato [?] | sì | sì | USN sì, MFT no | possibile sovrastima da dichiarare |
| FAT32, exFAT | solo arrotondamento al cluster | no | no | no | exFAT usa cluster da 32 o 128 KB [F] |
| SMB | trasportata dal server [V] | dipende | dipende | no | latenza alta, quote |
| BitLocker | trasparente [I] | | | | |

## 12. Robustezza

- File creati o cancellati durante l'enumerazione possono comparire o no, anche due volte [F].
- Dimensioni vecchie: NTFS copia i metadati nella voce di cartella alla chiusura dell'ultimo handle e solo nella voce usata [F]. Misurato: la voce indicava 0 byte con 5 MB reali e 5 MB con 30 MB reali [M]. Aprire e chiudere un handle senza diritti aggiorna la voce [F]: lo fa "Verifica questa voce" nell'ispettore.
- Percorsi lunghi: prefisso `\\?\` fino a circa 32.767 caratteri; meglio aprire i figli relativi all'handle della cartella [V] [I].
- UTF-16 non valido: Windows accetta surrogati spaiati, verificato con U+D800 [M]. I nomi si tengono in UTF-16 grezzo e si convertono solo per la visualizzazione.
- Distinzione tra maiuscole per cartella (WSL) [V]: le chiavi non devono normalizzare le maiuscole.
- Antivirus: analizza all'apertura [V]; l'enumerazione è 4-8 volte più veloce dell'apertura [M].
- Parallelismo: 8 thread su NVMe danno un fattore 4,8 [M]; su HDD può peggiorare [I].
- Insidie di interfaccia binaria [M]: `SHQUERYRBINFO` vuole cbSize 24 su x64; `WIN32_FIND_DATAW` ha i FILETIME allineati a 4 byte e un binding sbagliato sposta gli offset.

## 13. Linux

| Windows | Linux |
|---|---|
| Allocata | `stx_blocks` in unità da 512 byte [V] |
| Deduplica per ID file | (`stx_dev`, `stx_ino`) con `stx_nlink` [V] |
| Punti di montaggio | `/proc/self/mountinfo`, `STATX_ATTR_MOUNT_ROOT` [V] |
| Block cloning di ReFS | reflink di btrfs e XFS, FIEMAP con `FIEMAP_EXTENT_SHARED` [V] |
| Compressione | `STATX_ATTR_COMPRESSED` [V] |
| `ReadDirectoryChangesW` | inotify, fanotify con `FAN_MARK_FILESYSTEM` [V] |
| MFT, USN | nessun equivalente portabile [I] |

Solo Windows: MFT e USN, WOF, Cloud Files, tag di reparse, cartelle note, registro, MSIX, Pulizia disco, VSS, archiviazione riservata. Portabile: modello ad albero, deduplica per identità, logica e allocata, provenienza dei dati, motore delle regole.

## 14. Implicazioni per il progetto

Strategia per situazione:

| Situazione | Metodo principale | Ripiego |
|---|---|---|
| Utente standard, NTFS locale | enumerazione parallela con `FileIdExtdDirectoryInfo`, `FileStandardInfo` della cartella per l'indice | `FindFirstFileExW` |
| Su richiesta, con privilegi (helper, 05 D3) | enumerazione delle sole cartelle negate con `SeBackupPrivilege` e `FILE_FLAG_BACKUP_SEMANTICS`, con lo stesso codice dello scanner; spazio delle copie shadow; cestini di altri SID | `FSCTL_QUERY_FILE_LAYOUT` e lettura della MFT rinviati alla v2, solo se serviranno |
| Logica > 0, allocata 0, niente sparse, offline o cloud | apertura con soli attributi, `FileStandardInfo` | marcare "non verificato" |
| Segnaposto cloud | attributi e stato dal tag | logica mostrata come "nel cloud" |
| ReFS, Dev Drive | enumerazione con ID a 128 bit | avviso di possibile sovrastima |
| FAT32, exFAT | enumerazione, allocata = arrotondamento al cluster | |
| SMB | parallelismo moderato, timeout | niente calcolo del non attribuito |

Provenienza obbligatoria di ogni numero, nelle quattro classi del brief:

| Classe | Esempi |
|---|---|
| Misurato | allocazione dalla voce di cartella o dal file, totale e libero del volume, `$MFT`, indici, journal |
| Da fonte | voci di registro, pacchetti MSIX, manifest dei launcher, file di configurazione noti delle app |
| Dedotto | attribuzione per regola o per somiglianza, scomposizione del residuo, totali deduplicati, "probabile WOF" |
| Non disponibile | copie shadow, archiviazione riservata, cestini di altri utenti, cartelle negate (contate, senza byte), file eliminati ma aperti: "richiede amministratore" o "non leggibile", mai zero |

Le regole che ne derivano sono già in 05 (D4, D5, D7) e nei requisiti F-SC e F-SY di 03.

Fonti principali, in ordine di sezione:
- `FILE_ID_EXTD_DIR_INFORMATION`: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-file_id_extd_dir_information
- `NtQueryDirectoryFileEx`: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntquerydirectoryfileex
- `FSCTL_ENUM_USN_DATA`: https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_enum_usn_data
- `USN_RECORD_V2`: https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-usn_record_v2
- `FSCTL_QUERY_FILE_LAYOUT`: https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/fsctl-query-file-layout
- Journal ID: https://learn.microsoft.com/en-us/windows/win32/fileio/using-the-change-journal-identifier
- `ReadDirectoryChangesExW`: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesexw
- `GetCompressedFileSizeW`: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getcompressedfilesizew
- CompactOS: https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/compact-os
- `WofIsExternalFile`: https://learn.microsoft.com/en-us/windows/win32/api/wofapi/nf-wofapi-wofisexternalfile
- `CreateHardLinkW`: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createhardlinkw
- WinSxS: https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/determine-the-actual-size-of-the-winsxs-folder
- Tag di reparse: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fscc/c8e77b37-3909-4fe6-a4ea-2b9d423b1ee4
- Attributi dei file: https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
- `CfGetPlaceholderStateFromAttributeTag`: https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfgetplaceholderstatefromattributetag
- Segnaposto, linee guida per i filtri: https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/placeholders_guidance
- `RtlSetProcessPlaceholderCompatibilityMode`: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-rtlsetprocessplaceholdercompatibilitymode
- Costanti dei privilegi: https://learn.microsoft.com/en-us/windows/win32/secauthz/privilege-constants
- `GetDiskFreeSpaceExW`: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw
- MFT: https://learn.microsoft.com/en-us/windows/win32/fileio/master-file-table
- Point-in-time restore: https://learn.microsoft.com/en-us/windows/configuration/point-in-time-restore
- Archiviazione riservata: https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/dism-storage-reserve
- `SHQueryRecycleBinW`: https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryrecyclebinw
- Pulizia di WinSxS: https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder
- Windows Installer cache: https://learn.microsoft.com/en-us/troubleshoot/windows-client/application-management/missing-windows-installer-cache
- powercfg: https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/powercfg-command-line-options
- Disk Cleanup: https://learn.microsoft.com/en-us/windows/win32/lwef/disk-cleanup
- WSL disk space: https://learn.microsoft.com/en-us/windows/wsl/disk-space
- Docker system prune: https://docs.docker.com/reference/cli/docker/system/prune/
- npm cache: https://docs.npmjs.com/cli/v11/commands/npm-cache
- pnpm store: https://pnpm.io/cli/store
- `PackageManager.FindPackagesForUser`: https://learn.microsoft.com/en-us/uwp/api/windows.management.deployment.packagemanager.findpackagesforuser
- Chiave Uninstall: https://learn.microsoft.com/en-us/windows/win32/msi/uninstall-registry-key
- `SHGetKnownFolderPath`: https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath
- Block cloning su ReFS: https://learn.microsoft.com/en-us/windows-server/storage/refs/block-cloning
- Limiti dei percorsi: https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation
- Raymond Chen sulle dimensioni nelle voci di cartella: https://devblogs.microsoft.com/oldnewthing/20111226-00/?p=8813
- statx: https://man7.org/linux/man-pages/man2/statx.2.html

## 15. Prove per i nuovi scenari (revisione)

Sintesi di `revisione/prove-windows.md`, che contiene le tabelle complete e le fonti. Stesse sigle del resto del documento: [V] documentazione ufficiale, [F] fonte non ufficiale, [M] misurato sul PC di sviluppo, in sola lettura, [I] deduzione, [?] non trovato o ambiguo. Le misure sono del 12/10/2026, in una sessione non elevata. Le decisioni che ne derivano sono in `revisione/decisioni-revisione.md` §9 bis.

### 15.1 Uso dei programmi

| Fonte | Privilegi | Sul PC di sviluppo | Uso in Quado |
|---|---|---|---|
| UserAssist (HKCU, `Explorer\UserAssist`) | utente | `Start_TrackProgs` = 0: tracciamento degli avvii spento, qualche decina di voci ferme a una data di oltre un anno prima [M] | Solo se `Start_TrackProgs` = 1. Copre gli avvii da Esplora file e Start, non quelli da riga di comando, da altri launcher o da attività pianificate [F] |
| Steam `LastPlayed` (`localconfig.vdf`, `appmanifest_*.acf`) | utente | presente per quasi tutti i giochi installati; le due fonti concordano entro un giorno [M]. Formato non documentato [?] | Sì |
| BAM, Prefetch, Amcache, SRUM | amministratore | accesso negato [M] | Esclusi: richiedono l'amministratore e sono fonti di uso forense |
| Jump list, RecentDocs | utente | alcune decine di file AutomaticDestinations [M] | Esclusi: coprono poche app e contengono nomi di documenti |
| Ultimo accesso NTFS | utente | vedi sotto | Escluso: è rumore |
| Data di modifica delle cartelle dati | utente | segnali discordanti in `%LOCALAPPDATA%\Packages` [M] | Solo come "attività recente nei dati", mai come uso |
| Epic (`.item`), pacchetti MSIX | utente | nessun campo di ultimo utilizzo [M]; nessuna API documentata per l'ultimo avvio di altri pacchetti [V] | Nessun dato d'uso |

Cosa si può scrivere: "Ultima sessione registrata da Steam: data"; "Ultimo avvio da Esplora file o Start: data (UserAssist)" solo con il tracciamento attivo; altrimenti "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso". Non si scrive mai "mai usato", "non usato da N giorni" o "sicuro da disinstallare" (F-AP-4).

Ultimo accesso: su C: l'aggiornamento della data di ultimo accesso è attivo, anche se `fsutil behavior query disablelastaccess` scrive "Disabled" (valore di registro `0x80000002`) [M]. Lo conferma il fatto che il 97% degli eseguibili in Program Files risulta letto il giorno stesso [M]. Conseguenze:
- la data di ultimo accesso non dice nulla sull'uso, perché antivirus, indicizzazione e letture di massa la aggiornano [I];
- leggere il contenuto di un file la aggiorna: le letture di contenuto di Quado (risorse di versione, hash, directory centrale degli zip) avvengono solo su richiesta e l'interfaccia lo dichiara;
- Quado non apre handle in scrittura per evitarlo;
- Sensore memoria potrebbe rinviare la pulizia dei file di Download letti in questo modo, perché decide in base a quando sono stati aperti l'ultima volta [I].

### 15.2 Metadati degli installatori

Inventario di Download [M] (primo livello più un livello di sottocartelle): alcune decine di installatori, quasi tutti `.exe`, gli altri `.msi`.

| Prova | Metodo | Copertura [M] | Costo [M] | Uso in Quado |
|---|---|---|---|---|
| Risorsa di versione PE | `GetFileVersionInfoW` [V] | ProductName su circa nove exe su dieci | 18-25 s per tutti gli exe | Sì, su richiesta (F-DL-2) |
| Proprietà MSI | `MsiOpenDatabase` in sola lettura e query sulla tabella Property [V] | ProductName e ProductCode su tutti gli MSI | 754 ms per tutti gli MSI | Sì, su richiesta |
| Corrispondenza con i programmi installati, MSI | ProductCode uguale al nome della chiave di disinstallazione [V] | la maggior parte degli MSI | trascurabile | "Da fonte" |
| Corrispondenza con i programmi installati, exe | confronto dei nomi | nome uguale per circa quattro exe su dieci, parziale per circa uno su dieci, diverso per circa un terzo | trascurabile | "Dedotto", affidabilità media o bassa secondo 03 §6.2 [I] |
| Firma Authenticode | `CryptQueryObject`; la verifica deve usare solo la cache per non andare in rete [V] | circa otto exe su dieci firmati | 35 s | Fuori dalla v1 |
| Zone.Identifier | lettura dello stream | circa un installatore su otto; HostUrl rivela da dove è stato scaricato il file | 3-9 s | Fuori dalla v1 |
| Marcatori NSIS, Inno, Squirrel | lettura dei primi 4 MB | poco più di metà degli exe riconosciuti | 21,5 s | Fuori dalla v1 |

Il costo della risorsa di versione è quasi tutto nell'apertura del file: Defender analizza i file quando vengono aperti [V] [M], come già visto in §2. Per questo la lettura avviene in background, solo su richiesta, con una cache indicizzata da percorso, dimensione, data di modifica e ID file. Per gli MSI non si usa mai `MsiOpenPackage`, il cui risultato dipende dallo stato della macchina [V].

### 15.3 Copie identiche e nomi con "(N)"

- Con dimensione uguale e poi SHA-256 su tutta la cartella Download: oltre cento gruppi di copie identiche, alcuni GiB recuperabili; in circa un gruppo su cinque le copie hanno nomi diversi [M]. Il calcolo richiede circa 74 s per tutta la cartella [M], un tempo accettabile solo su richiesta.
- Il suffisso " (N)" lo aggiunge il browser (Chromium) in base al nome, non al contenuto [F]: delle centinaia di copie con suffisso, circa tre su quattro hanno una dimensione diversa dall'originale [M]. Il suffisso è quindi solo un filtro (F-DL-3); l'identità si stabilisce con l'hash (F-DL-4).
- L'hash si calcola solo tra file della stessa dimensione e mai sui segnaposto cloud, perché leggerli li scaricherebbe (§5).

### 15.4 Cestino

- Capacità: per default il 10% dei primi 40 GB più il 5% del resto del volume [F Raymond Chen 2014]. Su C: `MaxCapacity` in `HKCU\...\BitBucket\Volume\{GUID}` vale circa il 5% dell'unità, decine di GiB [M]; la formula applicata a un volume da circa 1 TB dà circa lo stesso valore [I].
- Oltre la capacità: un elemento troppo grande per il Cestino viene eliminato definitivamente, previo avviso [F]; se il Cestino si riempie, Windows elimina da solo gli elementi più vecchi [F]. Per questo Quado confronta capacità, contenuto attuale (`SHQueryRecycleBinW`) e selezione prima di ogni spostamento (F-AC-7).
- Orfani: la shell costruisce l'elenco del Cestino dai file `$I` [F]. Su C: ci sono alcune cartelle `$R` senza `$I`, per decine di GiB; `SHQueryRecycleBinW` riporta pochi MB [M]. Nell'interfaccia sono un'anomalia spiegata (F-SY-2), senza cifra recuperabile.
- Sensore memoria, attivo con la regola sul Cestino, non ha rimosso gli orfani [M].
- Svuotamento con orfani: secondo una fonte non ufficiale lo svuotamento rimuove solo le coppie `$I`/`$R` complete [F voidtools], ma non è stato provato [?]. Non è provato neanche se la capacità usata da Windows per eliminare gli elementi più vecchi conti solo le coppie complete [?]. Entrambe le prove si fanno su un VHDX in M1, prima degli interventi di M5.

### 15.5 Spazio davvero recuperato

| Caso | Comportamento | Effetto sulla stima |
|---|---|---|
| Copie shadow | Il copy-on-write copia nella diff area i blocchi prima che vengano sovrascritti [V VSS]. Dopo un'eliminazione lo spazio libero cresce, ma le scritture successive sugli stessi cluster possono riassorbirlo [I]. Se la diff area non basta, Windows elimina la copia più vecchia [V] | Con i punti di ripristino attivi la verifica avvisa che parte dello spazio può essere riassorbita. Le statistiche delle copie shadow richiedono l'amministratore [M]: le legge solo l'helper (F-SC-3) |
| Segnaposto cloud | Eliminare un file solo online lo elimina anche nel cloud; "Libera spazio" invece lo toglie dal disco e mantiene la copia nel cloud [V OneDrive]. Il PC di sviluppo non usa OneDrive, quindi questo caso si prova con un provider di prova (§5) | Si contano solo i byte locali |
| Hard link | Lo spazio torna libero solo quando sparisce l'ultimo nome [V `CreateHardLinkW`] | Un file con altri nomi fuori dalla selezione è escluso e citato |
| File aperti | Un file eliminato mentre è aperto si cancella alla chiusura dell'ultimo handle [V `DeleteFileW`] | La verifica attende la chiusura degli handle |
| File compressi e sparse | Si libera lo spazio allocato, non la dimensione logica [I] | Si contano le allocate |
| Attività concorrente | Il giorno delle misure hanno girato Sensore memoria, SilentCleanup e la scansione rapida di Defender [M] | Si mostrano due numeri distinti: "stimato" e "variazione del volume" (`GetDiskFreeSpaceExW` prima e dopo) [V] [I] |

Il metodo completo di stima e verifica è in 03 §6.3 e nei requisiti F-VE.

### 15.6 Residui di software disinstallato

Bulk Crap Uninstaller cerca i residui partendo dal programma appena disinstallato, con un punteggio (cartella vuota +4, eseguibili presenti -4, nome ancora in uso -4, cartella in uso -7) [F codice BCU]. Senza quel punto di partenza la ricerca è molto più debole [I].

Prova sul PC di sviluppo [M]: circa un terzo delle cartelle in Local, Roaming, ProgramData e LocalLow non corrisponde per nome a nessun programma installato. Di queste circa un quarto sono vuote, quasi un terzo sono ferme da più di un anno e circa un quinto sono state modificate negli ultimi 90 giorni, quindi probabilmente in uso: falsi positivi del confronto per nome [I].

Per questo i residui hanno stato Non sappiamo, con la data dell'ultima modifica e il testo "Nessun programma installato corrispondente (confronto per nome). Ultima modifica: data. Potrebbe essere un residuo." Nessuna proposta di intervento (F-AP-6).

### 15.7 Disinstallazione

| Tipo di programma | Punto di ingresso ufficiale | Fonte |
|---|---|---|
| Win32 | `ms-settings:appsfeatures` (elenco delle app), con il nome copiato negli appunti; in alternativa `control /name Microsoft.ProgramsAndFeatures` (appwiz.cpl). Non esiste un URI documentato per la pagina di una singola app Win32 | [V] [?] |
| MSIX | `ms-settings:appsfeatures-app?<PFN>`; da riga di comando `Remove-AppxPackage` per l'utente corrente | [V] |
| Steam | `steam://uninstall/<appid>`: lo usano tutte le voci "Steam App N" del registro | [M] |
| Epic | libreria del launcher; i giochi non hanno voci di disinstallazione nel registro | [V] [M] |

Quado non esegue mai `UninstallString` né `QuietUninstallString` (F-AP-5). Sul PC di sviluppo [M]:
- qualche decina di voci sta in HKCU, dove l'utente e i programmi che girano con i suoi diritti possono scriverle: eseguirle vorrebbe dire eseguire un comando che chiunque con quei diritti può aver cambiato [I];
- più di dieci voci `MsiExec /I` aprono la manutenzione del programma invece di disinstallarlo;
- diversi percorsi con spazi non sono tra virgolette, quindi l'interpretazione del comando è ambigua;
- qualche decina di voci ha una variante silenziosa, che salta le conferme del programma di disinstallazione.

La strada più sicura per uno strumento di terzi è aprire l'interfaccia ufficiale e lasciare all'utente l'ultimo passo [I]. Prima di aprirla Quado avvisa se una regola indica tuoi file dentro la cartella del programma.

Le fonti di questa sezione sono elencate in fondo a `revisione/prove-windows.md`.
