# Prove disponibili in Windows per i nuovi scenari (12/10/2026)

Sintesi del dossier di ricerca della revisione. Legenda: [V] documentazione ufficiale, [F] fonte non ufficiale, [M] misurato sul PC di sviluppo, in sola lettura, [I] deduzione, [?] non trovato o ambiguo.

Contesto delle misure: Windows 10 Home 22H2, utente non elevato, Defender attivo, C: NTFS da circa 1 TB. Le letture di contenuto fatte durante la ricerca (risorse di versione, hash) hanno aggiornato la data di ultimo accesso dei file del Download letti, vedi §A, riga "Ultimo accesso NTFS". Nessun contenuto è stato modificato.

## A. Uso dei programmi

| Fonte | Cosa registra | Chi la legge | Copertura e limiti | Sul PC di sviluppo [M] | Uso in Quado |
|---|---|---|---|---|---|
| UserAssist (HKCU …\Explorer\UserAssist\{GUID}\Count) | Percorso in ROT13, numero di avvii, ultimo avvio (FILETIME) [F winreg-kb, Cyber Triage] | utente corrente, senza privilegi | Avvii da Esplora file e Start; non da riga di comando, da altri launcher, da attività pianificate [F Securelist]. Si azzera con le sessioni lunghe; si ferma se "Tieni traccia degli avvii delle app" è spento (`Start_TrackProgs` = 0) [F] | `Start_TrackProgs` = 0; qualche decina di voci, dati fermi a una data di oltre un anno prima | Solo se `Start_TrackProgs` = 1, con la copertura dichiarata |
| BAM (HKLM\SYSTEM\…\bam\State\UserSettings) | Ultima esecuzione per percorso | amministratore | Da Windows 10 1709; non registra le esecuzioni da unità rimovibili [F] | accesso negato | Escluso |
| Prefetch (C:\Windows\Prefetch) | Fino a 8 ultimi avvii [F] | amministratore | | elenco negato; `EnablePrefetcher` = 3, SysMain attivo anche su SSD | Escluso |
| Amcache.hve | Presenza di file, non esecuzione [F Cyber Triage, ANSSI] | amministratore | | accesso negato | Escluso |
| SRUM | Circa 30 giorni di uso delle risorse per app [F Elcomsoft] | amministratore | | accesso negato | Escluso |
| Jump list, RecentDocs | Elementi recenti per app | utente | Solo app che usano la shell per i documenti; contengono nomi di documenti (privacy) | alcune decine di file AutomaticDestinations | Escluso |
| Ultimo accesso NTFS | Data di ultimo accesso | utente | Rumore: le letture di massa la aggiornano | Attivo su C: anche se `fsutil` scrive "Disabled" (registro `0x80000002`); il 97% degli exe in Program Files letto oggi | Escluso |
| Data di modifica delle cartelle dati | Attività nei dati dell'app | utente | Cambia anche per updater e telemetria [I] | segnali discordanti in `%LOCALAPPDATA%\Packages` | Solo come "attività recente nei dati", mai come uso |
| Steam `localconfig.vdf` e `appmanifest_*.acf` | `LastPlayed` (secondi Unix) e tempo di gioco | utente | Formato non documentato [?]; avvii fuori da Steam probabilmente esclusi [I] | LastPlayed presente per quasi tutti i giochi installati; le due fonti concordano entro un giorno | Sì |
| Epic `.item` | Nessun campo di ultimo utilizzo | utente | | pochi manifest | Nessun dato d'uso |
| MSIX | Nessuna API documentata per l'ultimo avvio di altri pacchetti [V AppDiagnosticInfo] | | | un centinaio di pacchetti | Nessun dato d'uso |

Cosa si può scrivere nella UI: "Ultima sessione registrata da Steam: data"; "Ultimo avvio da Esplora file o Start: data (UserAssist)" solo con il tracciamento attivo; altrimenti "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso". Mai "mai usato", "non usato da N giorni" come fatto, "sicuro da disinstallare".

## B. Installatori e file in Download

Inventario [M] (primo livello più un livello di sottocartelle): qualche migliaio di file, decine di GiB. Installatori .exe e .msi: alcune decine di file per alcuni GiB, circa metà con più di un anno.

| Prova | Metodo | Copertura [M] | Costo [M] | Note |
|---|---|---|---|---|
| Risorsa di versione PE | `GetFileVersionInfoW` [V] | ProductName su circa nove exe su dieci | 18-25 s per tutti gli exe, quasi tutto apertura (Defender analizza all'apertura [V]) | Legge parte del file; aggiorna l'ultimo accesso |
| Proprietà MSI | `MsiOpenDatabase` in sola lettura e query sulla tabella Property [V] | tutti gli MSI con ProductName e ProductCode | 754 ms | Mai `MsiOpenPackage`, che dipende dallo stato della macchina [V] |
| Corrispondenza con i programmi installati | ProductCode MSI = nome della chiave di disinstallazione [V]; per gli exe confronto di nomi | MSI: la maggior parte per ProductCode. Exe: nome uguale per circa quattro su dieci, parziale per circa uno su dieci, diverso per circa un terzo | trascurabile | Alta per MSI, media o bassa per gli exe [I] |
| Firma Authenticode | `CryptQueryObject` senza verifica; la verifica richiede `WTD_CACHE_ONLY_URL_RETRIEVAL` per non usare la rete [V] | circa otto exe su dieci firmati | 35 s | Fuori dalla v1 |
| Zone.Identifier | lettura dello stream | presente su gran parte dei file, ma solo su circa un installatore su otto; HostUrl rivela la provenienza | 3-9 s | Fuori dalla v1 (privacy, copertura bassa) |
| Marcatori NSIS, Inno, Squirrel | lettura dei primi 4 MB | poco più di metà degli exe riconosciuti, gli altri sconosciuti | 21,5 s | Fuori dalla v1 |
| Copie identiche | dimensione uguale, poi SHA-256 | oltre cento gruppi identici in tutta Download, alcuni GiB; circa un gruppo su cinque con nomi diversi | 74 s per tutta la cartella | Il suffisso " (N)" lo aggiunge Chromium in base al nome [F]: delle centinaia di copie con suffisso, circa tre su quattro hanno dimensione diversa dall'originale |

## C. Spazio davvero recuperato

1. Cestino:
   - La capacità predefinita è il 10% dei primi 40 GB più il 5% del resto [F Raymond Chen 2014]. Su C: `MaxCapacity` vale circa il 5% dell'unità, decine di GiB, in `HKCU\…\BitBucket\Volume\{GUID}` [M].
   - Un elemento troppo grande per il Cestino porta all'eliminazione definitiva, previo avviso [F].
   - Sensore memoria è attivo, con la regola sul Cestino [M].
2. Cartelle `$R` senza `$I`:
   - La shell costruisce l'elenco del Cestino dai file `$I` [F]. Lo svuotamento rimuoverebbe solo le coppie complete [F voidtools, non verificato].
   - Su C: ci sono alcune cartelle orfane per decine di GiB. `SHQueryRecycleBin` riporta pochi MB. Sensore memoria non le ha rimosse [M].
   - L'effetto di "Svuota Cestino" non è provato [?].
3. Hard link: lo spazio torna libero solo quando sparisce l'ultimo nome [V `CreateHardLinkW`]. Un file eliminato mentre è aperto si cancella alla chiusura dell'ultimo handle [V `DeleteFileW`].
4. Copie shadow: il copy-on-write copia nella diff area i blocchi prima di sovrascriverli [V VSS]. Dopo un'eliminazione lo spazio libero cresce, ma le scritture successive sugli stessi cluster possono riassorbirlo [I]. Quando la diff area non basta, Windows elimina la copia più vecchia [V]. Le statistiche richiedono l'amministratore [M].
5. Segnaposto cloud: eliminare un file solo online lo elimina anche nel cloud, mentre "Libera spazio" mantiene la copia nel cloud [V OneDrive]. Il PC di sviluppo non usa OneDrive, quindi questo caso si prova con un provider di prova.
6. File compressi e sparse: si libera lo spazio allocato, non la dimensione logica [I].
7. Attività concorrente: oggi hanno girato Sensore memoria, SilentCleanup e la scansione rapida di Defender [M].
8. Misura robusta: lo spazio libero da `GetDiskFreeSpaceExW` prima e dopo, attendendo la chiusura degli handle, più la riscansione dei percorsi toccati. Si mostrano due numeri separati: stimato e variazione del volume [V `GetDiskFreeSpaceExW`] [I].

## D. Residui di software disinstallato

Bulk Crap Uninstaller cerca i residui partendo dall'app appena disinstallata, con un punteggio (cartella vuota +4, eseguibili presenti -4, nome ancora in uso -4, directory in uso -7) [F codice BCU]. Senza quel punto di partenza la ricerca è molto più debole [I].

Prova sul PC di sviluppo [M]: circa un terzo delle cartelle in Local, Roaming, ProgramData e LocalLow non corrisponde per nome a nessun programma installato. Di queste, circa un quarto sono vuote, circa un quinto sono state modificate negli ultimi 90 giorni (quindi in uso: falsi positivi) e quasi un terzo sono ferme da più di un anno (alcuni GB).

Testo onesto: "Nessun programma installato corrispondente (confronto per nome). Ultima modifica: data. Potrebbe essere un residuo."

## E. Disinstallazione

- `ms-settings:appsfeatures` apre l'elenco; `ms-settings:appsfeatures-app?<PFN>` vale solo per le app con pacchetto [V launch-settings]. Non c'è un URI documentato per la pagina di una singola app Win32 [?]. Alternativa: `control /name Microsoft.ProgramsAndFeatures` [V].
- `UninstallString` e `QuietUninstallString` [M]:
  - più di dieci voci `MsiExec /I` aprono la manutenzione, non la disinstallazione;
  - diversi percorsi con spazi non hanno le virgolette;
  - qualche decina di voci ha una variante silenziosa;
  - qualche decina di voci sta in HKCU, dove l'utente può scriverle.
- MSIX: `Remove-AppxPackage` per l'utente corrente [V].
- Steam: tutte le voci "Steam App N" usano `steam://uninstall/N` [M].
- Epic: libreria del launcher [V Epic], nessuna voce di disinstallazione per i giochi [M].
- La strada più sicura per uno strumento di terzi è aprire l'interfaccia ufficiale (Impostazioni, appwiz.cpl, Steam, Epic) e non eseguire mai i comandi del registro [I].

## Fonti principali

- MS-FSA, ultimo accesso: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/4e3695bd-7574-4f24-a223-b4679c065b63
- fsutil behavior: https://learn.microsoft.com/windows-server/administration/windows-commands/fsutil-behavior
- Sensore memoria e Download: https://support.microsoft.com/kb/5000679
- GetFileVersionInfoW: https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-getfileversioninfow
- Database MSI in sola lettura: https://learn.microsoft.com/en-us/windows/win32/Msi/a-database-and-patch-example
- MsiOpenPackageEx: https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msiopenpackageexw
- Chiave Uninstall: https://learn.microsoft.com/en-us/windows/win32/msi/uninstall-registry-key
- WINTRUST_DATA: https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data
- Defender, analisi all'apertura: https://learn.microsoft.com/en-us/defender-endpoint/mdav-scan-best-practices
- Capacità del Cestino (Raymond Chen): https://devblogs.microsoft.com/oldnewthing/20140701-00/?p=603
- SHEmptyRecycleBinW: https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shemptyrecyclebinw
- Cestino e file $I (it-connect): https://www.it-connect.tech/windows-forensics-part-9-what-the-recycle-bin-can-reveal/
- VSS: https://learn.microsoft.com/windows-server/storage/file-server/volume-shadow-copy-service
- OneDrive File su richiesta: https://support.microsoft.com/office/save-disk-space-with-onedrive-files-on-demand-for-windows-0e6860d3-d9f3-4971-b321-7092438fb38e
- GetDiskFreeSpaceExW: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw
- URI di Impostazioni: https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings
- Remove-AppxPackage: https://learn.microsoft.com/en-us/powershell/module/appx/remove-appxpackage
- Epic, disinstallare un gioco: https://help.epicgames.com/help/en-US/c-Category_EpicGamesStore/c-EpicGamesStore_LauncherSupport/how-to-uninstall-a-game-from-the-epic-games-launcher-a000087932
- UserAssist (winreg-kb): https://winreg-kb.readthedocs.io/en/latest/sources/explorer-keys/User-assist.html
- UserAssist (Securelist): https://securelist.com/userassist-artifact-forensic-value-for-incident-response/116911/
- BCU, residui: https://github.com/Klocman/Bulk-Crap-Uninstaller/blob/master/source/UninstallTools/Junk/ProgramFilesOrphans.cs
