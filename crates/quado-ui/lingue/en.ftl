### Quado: interface text in English. The reference language is Italian (it.ftl), where every key
### has a comment explaining where it appears. Keep the meaning of states and warnings (RS29).

## General

lingua-nome = English
conteggio-file = { $n ->
    [one] { $testo } file
   *[other] { $testo } files
}
conteggio-cartelle = { $n ->
    [one] { $testo } folder
   *[other] { $testo } folders
}

## Units

unita-byte = { $n ->
    [one] byte
   *[other] bytes
}
unita-kb = KB
unita-mb = MB
unita-gb = GB
unita-tb = TB
unita-pb = PB
unita-si-kb = kB
unita-secondi = s

## Drive list

volume-titolo = { $etichetta } ({ $lettera })
volume-senza-etichetta = Local Disk
volume-uso = { $usato } of { $totale } · { $fs }
unita-fissa = fixed drive
unita-rimovibile = removable drive
unita-di-rete = network drive
unita-ottica = optical drive
unita-ram = RAM disk
unita-sconosciuta = unknown drive type
volumi-titolo = Drives
volumi-meta = Choose a drive to scan. The scan lists file names, sizes and dates and doesn't read file contents.
volumi-scansiona = Scan { $lettera }
volumi-uso = { $usato } used of { $totale }
volumi-di-sistema = system drive
volumi-nota-fat = On { $fs }, sizes on disk are rounded to the cluster. This file system has no compression and no files with more than one name (hard links).
volumi-senza-garanzie = Measurements without guarantees: Quado's totals are tested only on NTFS, exFAT and FAT32.
volumi-nota = Partitions without a drive letter, such as another operating system's, stay out of this list: Quado doesn't read them.
volumi-nessuno = Windows lists no drive with a letter. Connect the disk or assign it a letter, then restart Quado.
volumi-torna = Back to Overview

## Title bar and sidebar

barra-volumi = Choose a drive
barra-cambia-volume = Opens the list of drives.
barra-riesamina = Rescan
barra-riesamina-suggerimento = Scans drive { $volume } again and updates all totals (F5).
ricerca-suggerimento = Search folders and files
ricerca-scorciatoia = Ctrl+F
nav-panoramica = Overview
nav-esplora = Explore
nav-file-grandi = Large files
nav-volumi = Drives
nav-impostazioni = Settings
nav-nota-sola-lettura = The scan only reads. Quado moves files to the Recycle Bin only after the preview and your confirmation.
barra-comprimi = Collapse the sidebar
barra-espandi = Expand the sidebar
barra-palette = Go to
palette-segnaposto = Go to a view, folder or command
palette-gruppo-viste = Views
palette-gruppo-elementi = Folders and files
palette-gruppo-comandi = Commands
palette-sposta = to choose
palette-apri = to open
palette-chiudi = to close
palette-nessuno = Nothing matches this text. Search also finds part of a name: try fewer letters.
tasto-esc = Esc
tasto-invio = Enter

## Status bar

stato-scansione-in-corso = Scanning… { $file } files read
stato-non-leggibili = { $n ->
    [one] { $testo } unreadable folder
   *[other] { $testo } unreadable folders
}
stato-metodo = Size on disk, each file counted once
stato-senza-privilegi = Quado isn't running as administrator
stato-amministratore = Running as administrator

## Banners

banner-interrotta = Scan interrupted: the totals don't include the folders that weren't read. To complete them, use "Rescan".
banner-fat = { $fs } drive: sizes on disk are rounded to the cluster. This file system has no compression and no files with more than one name (hard links).
banner-senza-garanzie = { $fs } file system: measurements without guarantees. Quado's totals are tested only on NTFS, exFAT and FAT32.
banner-errore-scansione = Can't read { $volume }. Check that the drive is connected, then try again from the list of drives. (Code { $codice })

## Scan in progress

scansione-titolo = Scanning { $volume }
scansione-meta = Read-only; Quado isn't running as administrator. If you cancel, what was already read stays visible.
scansione-meta-amministratore = Read-only, as administrator. If you cancel, what was already read stays visible.
scansione-file = files read
scansione-cartelle = folders read
scansione-letti = measured so far
scansione-tempo = elapsed
scansione-annulla = Cancel
scansione-annullo = Cancelling…
scansione-esc = Esc cancels.
scansione-primo-livello = Top level of the drive
scansione-primo-livello-nota = Totals grow as the scan proceeds.
scansione-attesa = Reading the root…
scansione-altri = { $n ->
    [one] and one more item
   *[other] and { $testo } more items
}
scansione-completata = { $n ->
    [0] Scan completed in { $durata }.
    [one] Scan completed in { $durata }. Windows denied access to one folder: its content stays in the space unaccounted for.
   *[other] Scan completed in { $durata }. Windows denied access to { $testo } folders: their content stays in the space unaccounted for.
}

## Overview

panoramica-meta-cluster = { $fs } · { $cluster } clusters
panoramica-scansione = Scanned on { $data } at { $ora } · { $durata } · not as administrator
panoramica-scansione-amministratore = Scanned on { $data } at { $ora } · { $durata } · as administrator
bilancio-titolo = How the space is used
bilancio-unita-esplora = GB as in File Explorer (1 GB = 1024³ bytes)
bilancio-unita-si = GB in SI units (1 GB = 10⁹ bytes)
bilancio-usati = used
bilancio-liberi = free
bilancio-su = of { $totale }
bilancio-usati-su = used of { $totale }
bilancio-file-letti = Files read
bilancio-metadati = NTFS metadata
bilancio-riservato = Space reserved by Windows
bilancio-non-attribuito = Unaccounted for
bilancio-libero = Free
bilancio-deriva = { $valore } ± { $deriva }
bilancio-quota-volume = { $quota } of the drive
bilancio-nota-deriva = Used space changed by { $deriva } during the scan: that is the uncertainty on the space unaccounted for.
bilancio-nota-quota = A disk quota reserves { $quota } of the free space for other accounts.
bilancio-non-disponibile = Available only after scanning a whole drive.
top-titolo = Where the space goes
top-sottotitolo = Largest items, of any kind
top-spiegazione-cartella = In { $percorso } · { $file }
top-spiegazione-file = File in { $percorso }
top-suggerimento = { $quota } of used space. Click to open the item in Explore.
top-vuoto = No item takes space on disk in this scan.
top-legenda-fonte = Source of each fact:
conti-titolo = Why the numbers don't fully add up
conti-usato-windows = Used space according to Windows
conti-file-letti = Files read, each file counted once
conti-metadati = Measured NTFS metadata: $MFT { $mft }, folder indexes { $indici }, journal { $journal }
conti-non-attribuito = Unaccounted for
conti-hard-link = Files with more than one name (hard links): { $testo } extra names for the same data, already counted once. Counting every name would add { $byte }.
conti-osservato = Observed
conti-non-leggibili = { $n ->
    [one] { $testo } unreadable folder: { $esempi }
   *[other] { $testo } unreadable folders, including { $esempi }
}
conti-cestini-altri = { $n ->
    [one] One Recycle Bin of another account, unreadable
   *[other] { $testo } Recycle Bins of other accounts, unreadable
}
conti-profili-altri = { $n ->
    [one] One profile of another account, unreadable
   *[other] { $testo } profiles of other accounts, unreadable
}
conti-osservato-nulla = All folders were read.
conti-possibile = Possible, measurable only as administrator
conti-copie-shadow = Restore points and shadow copies
conti-archiviazione-riservata = Reserved storage
conti-altri-metafile = Other NTFS metafiles ($LogFile, $Bitmap, $Secure)
conti-riservato = Space reserved by Windows, reserved storage included
conti-lette-amministratore = { $n ->
    [one] One folder read as administrator: { $dimensione }
   *[other] { $testo } folders read as administrator: { $dimensione }
}
conti-cestino-altro = Recycle Bin of another account ({ $sid }): { $dimensione }
conti-cestino-orfano = Recycle Bin of an account that no longer exists on this PC ({ $sid }): { $dimensione }
conti-profili-misurati = { $n ->
    [one] One profile of another account: { $dimensione } in { $file }
   *[other] { $testo } profiles of other accounts: { $dimensione } in { $file }
}
conti-copie-shadow-misurate = Restore points and shadow copies: { $usato } used, { $allocato } allocated, maximum { $massimo }. This space is inside System Volume Information and isn't added to these numbers.
conti-copie-shadow-senza-limite = Restore points and shadow copies: { $usato } used, { $allocato } allocated, no maximum. This space is inside System Volume Information and isn't added to these numbers.
conti-copie-shadow-nessuna = Restore points and shadow copies: no space assigned on this drive.
conti-copie-shadow-servizio-spento = Restore points and shadow copies: the Volume Shadow Copy service is disabled on this PC. Without the service, Windows doesn't report the assigned space.
conti-copie-shadow-errore = Can't read the space used by restore points and shadow copies. (Code { $codice })
helper-spiegazione = { $n ->
    [one] As administrator, Quado reads only the folder with access denied and the space used by restore points and shadow copies. It doesn't read file contents. For profiles and Recycle Bins of other accounts it shows only totals. Windows asks you to confirm: for a program without a digital signature, it shows the publisher as unknown.
   *[other] As administrator, Quado reads only the { $testo } folders with access denied and the space used by restore points and shadow copies. It doesn't read file contents. For profiles and Recycle Bins of other accounts it shows only totals. Windows asks you to confirm: for a program without a digital signature, it shows the publisher as unknown.
}
helper-ripeti = Retry as administrator
helper-attesa = Waiting for confirmation from Windows…
helper-lettura = { $n ->
    [one] Reading as administrator… { $fatte } of { $testo } folder
   *[other] Reading as administrator… { $fatte } of { $testo } folders
}
helper-annulla = Cancel
helper-fine = { $n ->
    [one] Reading as administrator complete: { $lette } of { $testo } folder.
   *[other] Reading as administrator complete: { $lette } of { $testo } folders.
}
helper-fine-non-lette = { $n ->
    [one] One folder didn't open even as administrator and stays among the unreadable ones. (Code { $codici })
   *[other] { $testo } folders didn't open even as administrator and stay among the unreadable ones. (Codes { $codici })
}
helper-fine-ignorate = { $n ->
    [one] One reply concerns a folder that is no longer in the list: Quado didn't apply it.
   *[other] { $testo } replies concern folders that are no longer in the list: Quado didn't apply them.
}
helper-fine-codice = Reading as administrator ended with an error: the replies received before it are applied. (Code { $codice })
helper-senza-privilegio = Windows didn't grant the backup privilege: the folders with access denied remain unread.
helper-rifiutato = You answered No to the Windows prompt: nothing changed.
helper-annullato = Reading as administrator cancelled: nothing changed.
helper-errore = Can't read as administrator. Nothing changed: you can try again. { $motivo }
helper-motivo-codice = (Code { $codice })
helper-motivo-uscita = The reading process closed before the end. (Code { $codice })
helper-motivo-tempo = No answer within { $secondi } seconds.
helper-motivo-processo = A process other than the one started answered.
helper-motivo-protocollo = The reading process sent an invalid message.
helper-motivo-chiuso = The connection closed before the end.
helper-posizione = Can't read as administrator from this folder. For security, Quado doesn't start as administrator from Downloads or a temporary folder. Move quado.exe to a folder of its own, for example %LOCALAPPDATA%\Programs\Quado, and open it from there.

## Explore

esplora-indietro = Back (Alt+Left arrow)
esplora-avanti = Forward (Alt+Right arrow)
esplora-su = Go to parent folder (Backspace)
esplora-dimensione-quota = { $dimensione } · { $quota } of used space
esplora-mappa-ed-elenco = Map and list
esplora-solo-elenco = List only
esplora-vuoto = This folder is empty.
esplora-vuoto-non-letta = Quado didn't read the content of this folder: the reason is in the inspector. "Rescan" reads the whole drive again.
esplora-filtri-nessuno = No item in this folder matches these filters. Try "Clear filters".
gestore-non-classificato = Not classified
gestore-app = Programs and app data
gestore-tuoi = Your files
gestore-windows = Windows
legenda-non-leggibile = Unreadable
mappa-suggerimento = { $dimensione } · { $quota } of used space
colonna-nome = Name
colonna-dimensioni-su-disco = Size on disk
colonna-quota = Share of folder
colonna-file = Files
colonna-ultima-modifica = Date modified
colonna-gestito-da = Managed by
colonna-cartella = Folder
tabella-non-leggibile = unreadable
tabella-non-classificato = not classified
tag-collegamento = link, not followed
tag-non-leggibile = unreadable
tag-cloud = in the cloud
tag-cloud-non-elencata = cloud, not listed

accessibile-mappa = Folder map
accessibile-tabella = Table of folders and files
accessibile-elenco = List of folders and files

## Search and filters

ricerca-titolo = Results for "{ $testo }"
ricerca-in-corso = Searching…
ricerca-risultati = { $n ->
    [one] { $testo } item · { $ms } ms
   *[other] { $testo } items · { $ms } ms
}
ricerca-nessun-risultato = No folder or file with this name. Search also finds part of a name: try fewer letters.
ricerca-nessun-risultato-filtri = No folder or file with this name and these filters. Try "Clear filters".
ricerca-chiudi = Close search
filtro-dimensione-minima = Minimum size
filtro-dimensione-non-valida = Write the minimum size as a number, for example 1.5. Until then, the filter doesn't apply.
filtro-tipo = Type
filtro-tipo-tutti = All types
filtro-modificato = Date modified
filtro-data-qualsiasi = Any date
filtro-data-30-giorni = In the last 30 days
filtro-data-ultimo-anno = In the last year
filtro-data-oltre-un-anno = More than a year ago
filtro-data-oltre-tre-anni = More than three years ago
filtro-azzera = Clear filters
filtro-nota-estensione = Types by file extension: Quado doesn't read file contents.
famiglia-video = Video
famiglia-audio = Audio
famiglia-immagini = Images
famiglia-documenti = Documents
famiglia-archivi = Compressed archives
famiglia-immagini-disco = Disk images
famiglia-installatori = Programs and installers
famiglia-componenti = Program components
famiglia-codice = Code and configuration
famiglia-database = Databases and data files
famiglia-log = Logs and dumps
famiglia-altro = Other extensions
famiglia-senza-estensione = No extension

## Large files

file-grandi-titolo = Large files
file-grandi-meta = The largest files on { $volume } by size on disk, each file counted once: { $testo } listed.
file-grandi-calcolo = Building the list…
file-grandi-nessuno = No file matches these filters. To see all files, use "Clear filters".
file-grandi-vuoto = No file was measured in this scan. "Rescan" reads the whole drive again.

## Inspector

ispettore-nessuna-selezione = Select a folder or a file to see its measurements.
tipo-unita = Drive
tipo-cartella = Folder
tipo-collegamento = Link to another folder
tipo-file = { $estensione } file · { $famiglia }, by extension
tipo-file-senza-estensione = File · { $famiglia }
ispettore-non-leggibile = Unreadable
ispettore-quota-usato = { $quota } of used space
fatto-dimensioni = Size: { $dimensione } ({ $byte } bytes)
fatto-dimensioni-su-disco = Size on disk: { $dimensione } ({ $byte } bytes)
fatto-ultima-modifica = Date modified: { $data } { $ora }
fatto-non-leggibile = Unreadable: its content stays in the space unaccounted for. { $motivo }
fatto-cloud = Cloud placeholder: { $nel-cloud } in the cloud, { $locale } on this disk.
fatto-cloud-non-elencata = Cloud folder not listed: listing it can start a download from the cloud.
fatto-collegamento = Link not followed: its content is counted in the folder it points to.
fatto-hard-link = File with more than one name (hard link): its bytes are counted once, here.
fatto-hard-link-secondario = File with more than one name (hard link): its bytes are already counted elsewhere, under another name.
fatto-allocata-corretta = Size on disk read from the file: Windows compresses it.
fatto-attributi = Attributes: { $elenco }
attributo-compresso = compressed
attributo-sparse = sparse
attributo-cifrato = encrypted
attributo-nascosto = hidden
attributo-sistema = system
motivo-accesso-negato = Windows denies access unless Quado runs as administrator.
motivo-accesso-negato-amministratore = Windows denies access even with Quado running as administrator.
motivo-in-uso = It's in use by another program.
motivo-non-trovata = Windows can't find it anymore: maybe it was deleted during the scan.
motivo-interrotta = The listing stopped halfway. (Code { $codice })
motivo-annullata = The scan was cancelled before reaching it.
motivo-altro = Windows returned an error. (Code { $codice })
ispettore-parti-principali = Largest parts
ispettore-tipi-di-file = File types
ispettore-secondo-estensione = Types by file extension: Quado doesn't read file contents.
ispettore-calcolo = Calculating file types…
ispettore-nessun-file = No files in this folder.
ispettore-azioni = Actions
azione-apri-in-esplora = Show in File Explorer
azione-copia-percorso = Copy path
azione-proprieta = Properties
azione-proprieta-suggerimento = Opens the Windows Properties window, which can load extensions from other programs.
azione-verifica = Reread from disk
azione-verifica-suggerimento = Reads the sizes again from disk, read-only, and updates the totals.
verifica-in-corso = Reading again…
verifica-invariata = Read again at { $ora }: size on disk unchanged.
verifica-cambiata = Read again at { $ora }: size on disk went from { $prima } to { $dopo }.
verifica-errore = Can't read the item again. The scan measurements stay: try again, or use "Rescan". (Code { $codice })
fonte-misurato = measured
fonte-da-fonte = from source
fonte-dedotto = inferred
fonte-non-disponibile = not available

## Short notices

avviso-percorso-copiato = Path copied to the clipboard
avviso-shell-errore = Can't open the window. Try again. (Code { $codice })
avviso-lingue-ricaricate = Language files reloaded
avviso-modello-creato = Translation template created in the languages folder
avviso-errore-file = Can't create the template. Check that you can write to the languages folder. (Code { $codice })
avviso-rilievo-copiati = Rilievo data copied to { $cartella }
avviso-rilievo-errore = Can't copy the Rilievo data: Quado starts with the default settings and tries again at the next start. (Code { $codice })

## Settings

impostazioni-titolo = Settings
impostazioni-lingua = Language
impostazioni-lingua-file-utente = from your file
impostazioni-lingua-nota = You can fix a text or add a language with an .ftl file in { $cartella }. The file name is the language code, for example en.ftl or fr.ftl. Then choose "Reload files".
impostazioni-lingua-apri-cartella = Open the languages folder
impostazioni-lingua-modello = Create a template to translate
impostazioni-lingua-ricarica = Reload files
impostazioni-lingua-errori = { $n ->
    [one] Quado ignores one line in the language files and uses all the others. Fix it and choose "Reload files".
   *[other] Quado ignores { $n } lines in the language files and uses all the others. Fix them and choose "Reload files".
}
impostazioni-tema = Theme
tema-sistema = Same as Windows
tema-chiaro = Light
tema-scuro = Dark
impostazioni-tema-nota = Quado uses no animations when "Animation effects" is turned off in Windows.
impostazioni-unita = Units
unita-scelta-esplora = As in File Explorer (1 GB = 1024³ bytes)
unita-scelta-si = Decimal (1 GB = 10⁹ bytes)
impostazioni-unita-nota = With "As in File Explorer", a single file has the same size you see in File Explorer. A folder total can differ because Quado counts files with more than one name (hard links) only once.
impostazioni-informazioni = About
info-versione = Version
info-grafica = Graphics
info-dati = Data folder
info-nome-nota = Quado was called Rilievo until October 26, 2026.
info-licenza-titolo = License
info-licenza = Quado is free software: you can redistribute and change it under the GNU General Public License, version 3 or (at your option) any later version.
info-garanzia = Quado comes with no warranty, to the extent the law allows: see sections 15 and 16 of the license.
info-terze-parti = The licenses of the libraries and fonts by other authors are in the release zip: THIRD-PARTY-LICENSES.html and the licenses folder.
info-testo-licenza = License text
info-sorgente = Source code
impostazioni-aspetto = Appearance
impostazioni-indice = Settings sections
impostazioni-accento = Accent color
impostazioni-accento-nota = Buttons, selection and focus. Data colors stay the same.
accento-grafite = Graphite
accento-viola = Violet
accento-rosa = Rose
impostazioni-colori-dati = Data colors
impostazioni-colori-dati-nota = Colors of the four managers in the bar, the legends and the map. Shape and name stay the same.
colori-dati-predefiniti = Default
colori-dati-contrasto = High contrast
colori-dati-discromatopsie = For color blindness
impostazioni-testo = Text size
impostazioni-testo-nota = Makes text, spacing and controls in Quado larger or smaller. Shortcuts: Ctrl+Plus and Ctrl+Minus.
impostazioni-densita = Density
impostazioni-densita-nota = Height of rows in tables and lists.
densita-comoda = Comfortable
densita-compatta = Compact
impostazioni-animazioni = Animations
impostazioni-animazioni-nota = Off when "Animation effects" is turned off in Windows.
animazioni-windows = As in Windows
animazioni-spente = Off
impostazioni-esplora-vista = Opens with
impostazioni-colonne = Table columns
impostazioni-colonne-nota = Name and Size on disk are always shown.
impostazioni-barra-laterale = Sidebar
barra-laterale-espansa = Expanded
barra-laterale-compressa = Collapsed
impostazioni-anteprima = Preview

## M3: classification, states, recommendation records, Windows and system, rules.

nav-sistema = Windows and system
nav-regole = Rules
nav-download = Downloads
nav-programmi = Programs

banner-regole-da-riverificare = { $n ->
    [one] { $testo } rule to check again: it was verified more than six months ago. Twelve months after verification, it no longer suggests actions.
   *[other] { $testo } rules to check again: they were verified more than six months ago. Twelve months after verification, they no longer suggest actions.
}
banner-regole-apri = Open the rules

bilancio-non-classificato-ipotesi = Not classified or assumption
legenda-non-classificato-ipotesi = Not classified or assumption
etichetta-ipotesi = assumption

interventi-titolo = Possible actions
interventi-sottotitolo = Each entry comes from a rule, with its source and verification date. Select an entry to see in the inspector what you can do, with assumptions and risks. Quado changes nothing on the disk from this card.
interventi-occupa = Uses
interventi-libera-cestino = Freed when you empty the Recycle Bin: { $valore }
interventi-libera-cestino-intervallo = Freed when you empty the Recycle Bin: between { $minimo } and { $massimo }
interventi-libera-cestino-anteprima = Freed when you empty the Recycle Bin: the preview calculates it
interventi-libera-procedura = Freed: Quado measures it after the steps
interventi-gruppo = { $nome }: { $n ->
    [one] { $testo } item
   *[other] { $testo } items
}
interventi-altri = { $n ->
    [one] One more entry is in the inspector of its item.
   *[other] { $testo } more entries are in the inspectors of their items.
}
interventi-vuoto = No rule suggests an action on this drive. Identical copies in Downloads appear here after "Compare the content".
interventi-cosa-puoi-fare = See what you can do

filtro-gestore = Managed by
filtro-gestore-tutti = All

stato-gestibile = Manageable
stato-da-verificare = To verify
stato-decidi-tu = You decide
stato-lo-gestisce = Managed by { $chi }
stato-lo-gestisce-nessuna-procedura = Managed by { $chi }. No steps to follow: don't touch
stato-non-sappiamo = We don't know
motivo-stato-nessuna-regola = No rule recognizes this item. You can see its measurements and use "Show in File Explorer".
motivo-stato-versione = The installed version is outside the versions verified for the rule, or it's unknown. Quado suggests no action.
motivo-stato-regola-scaduta = The rule was verified on { $data }, more than a year ago. Quado shows the explanation as an assumption and suggests no action.
motivo-stato-conflitto = Two rules describe this item differently. Quado shows both and suggests no action.
motivo-stato-solo-nome = The role of this item comes only from its name. Quado suggests no action.
motivo-stato-senza-prova = The rule finds no proof that acting on this item has known effects. Quado suggests no action.

affidabilita-alta = high
affidabilita-media = medium
affidabilita-bassa = low
affidabilita-riga = Reliability of inferred facts: { $livello } ({ $criterio })
criterio-due-fonti = two sources that agree
criterio-fonte-ufficiale = official source and structure as described
criterio-una-fonte = a single unofficial source
criterio-struttura = structure recognized without an official source
criterio-nome = only the name or a similarity
criterio-versione = version outside those verified for the rule
criterio-scaduta = rule verified more than a year ago
criterio-conflitto = conflicting rules

azione-tipo-cestino = move to the Recycle Bin, after the preview
azione-tipo-procedura = steps from Windows or the program that manages it
azione-tipo-comando = official command, which Quado shows and copies without running it
azione-tipo-disinstallazione = uninstall from the Programs view, through Settings › Apps or the launcher
azione-tipo-nessuna = none, besides those always available
privilegi-nessuno = administrator rights not needed
privilegi-amministratore = the command must run as administrator, so Quado shows and copies it without running it
controllo-esito-simulazione = checked in the preview
controllo-esito-superato = passed
controllo-esito-non-superato = not passed
controllo-esito-da-spuntare = for you to tick
stima-esatta = Exact estimate: { $valore }
stima-intervallo = Estimate: between { $minimo } and { $massimo }
stima-non-stimabile-link = Not estimable: the files have other names (hard links) outside this folder
stima-non-stimabile-procedura = Not estimable: the result is measured after the steps
stima-nessuna-azione = No estimate: no action suggested
stima-nella-simulazione = Calculated in the preview, when Quado reads the files again

ispettore-cos-e = What it is
ispettore-nessuna-regola = No rule recognizes it
ispettore-fa-parte-di = Part of { $nome }
ispettore-vai-elemento = Go to that item
ispettore-prova-gestibilita = Proof of manageability
ispettore-gestito-da = Managed by: { $gestore }
ispettore-schemi = Recognized by the rules
ispettore-schema-regola = Rule: { $regola }
ispettore-cosa-puoi-fare = What you can do
ispettore-azioni-elemento = Select the item that contains it to see what you can do.
ispettore-regola = Rule
ispettore-anche = Also recognized by: { $regole }
ispettore-vedi-regola = Open the rule
ispettore-eta = Age of the files
ispettore-eta-nota = Age based on the date modified.
eta-30-giorni = Last 30 days
eta-1-anno = From 30 days to one year
eta-3-anni = From one to three years
eta-oltre = More than three years
record-azione = Action
record-copia = Copy
record-metodo = Method
record-alternative = Alternatives
record-controlli = Checks before acting
record-privilegi = Privileges
record-reversibilita = Reversibility
record-errore = If it stops halfway
record-verifica = Verification
avviso-comando-copiato = Copied to the clipboard
avviso-link-copiato = Address copied to the clipboard

regole-titolo = Rules
regole-sottotitolo = { $n ->
    [one] Quado uses { $testo } rule. Each rule has a source with an excerpt, a verification date and a test case in Quado's tests. Select a rule to see its sources.
   *[other] Quado uses { $testo } rules. Each rule has a source with an excerpt, a verification date and a test case in Quado's tests. Select a rule to see its sources.
}
regole-vuoto = Can't read the rules included in Quado. Without rules, each item keeps only its measurements and "Show in File Explorer".
regole-verificata = Verified on { $data }
regole-versioni = { $programma } from { $min } to { $max }
regole-scadenza-riverificare = To check again: the { $data } verification is more than six months old.
regole-scadenza-annuncio = The { $data } verification is valid until { $limite }. If the rule isn't checked again, from that day it no longer suggests actions.
regole-scadenza-scaduta = The { $data } verification is more than a year old. The rule no longer suggests actions and its explanation counts as an assumption.
regole-fonte-ufficiale = official source
regole-fonte-non-ufficiale = unofficial source
regole-copia-link = Copy address
regole-fonte-file = A program file on your PC
regole-fonte-prova = A test made with Quado, described in this document of the source code
regole-fixture = Test case: { $nome }
regole-fonti = { $n ->
    [one] { $testo } source
   *[other] { $testo } sources
}

estrattore-non-presente = not present
estrattore-accesso-negato = readable only as administrator
estrattore-segnaposto-cloud = not read, the file is only in the cloud
estrattore-errore = not read (Code { $codice })
estrattore-non-riuscito = not read, { $motivo }
formato-troncato = the file ends halfway
formato-inatteso = unexpected format
formato-vuoto = the file is empty
formato-troppo-annidato = too many nested levels
formato-codifica = invalid character encoding
formato-troppo-grande = the file exceeds the reading limit
formato-campo-mancante = the { $campo } field is missing
formato-tipo-diverso = the { $campo } field has an unexpected type
formato-valore = the { $campo } field has an unexpected value

valore-attivo = on
valore-non-attivo = off
valore-non-registrato = not set

sistema-titolo = Windows and system
sistema-sottotitolo = Space used by Windows on this drive, with the measurement and state of each item.
sistema-voci = System items
sistema-voce-paging = Paging file (pagefile.sys)
sistema-voce-swap = Swap file (swapfile.sys)
sistema-voce-ibernazione = Hibernation file (hiberfil.sys)
sistema-voce-winsxs = Component store (WinSxS)
sistema-voce-softwaredistribution = Windows Update files (SoftwareDistribution)
sistema-voce-installer = Windows Installer cache
sistema-voce-driverstore = Driver store (DriverStore)
sistema-voce-cestino = Recycle Bin
sistema-voce-windows-old = Previous Windows installation (Windows.old)
sistema-voce-copie-shadow = Restore points and shadow copies
sistema-voce-archiviazione-riservata = Reserved storage
sistema-voce-mft = File table ($MFT)
sistema-voce-indici = Folder indexes
sistema-voce-usn = File change journal ($UsnJrnl)
sistema-non-presente = not present
sistema-cestino-windows = { $valore }, { $n ->
    [one] { $testo } item
   *[other] { $testo } items
}
sistema-cestino-secondo-windows = According to Windows
sistema-cestino-su-disco = On disk
sistema-cestino-orfane = Folders without an index
sistema-non-disponibile-admin = measurable only as administrator
sistema-non-disponibile-fs = not available on this file system
sistema-nota-procedure = Select an item to see the Microsoft steps or command in the inspector, with the source in Rules. Quado shows and copies commands without running them.

configurazione-titolo = Configuration read
configurazione-sottotitolo = Read during the scan from Windows and from program configuration files, without changing anything.
configurazione-ibernazione = Hibernation
configurazione-avvio-rapido = Fast startup
configurazione-batteria-critica = Critical battery action
configurazione-batteria-0 = do nothing
configurazione-batteria-1 = sleep
configurazione-batteria-2 = hibernate
configurazione-batteria-3 = shut down
configurazione-batteria-4 = other value
configurazione-alimentazione = Power
configurazione-sensore = Storage Sense
configurazione-sensore-frequenza = Storage Sense frequency
configurazione-sensore-poco-spazio = when free space is low
configurazione-giorni = { $n ->
    [one] { $testo } day
   *[other] { $testo } days
}
configurazione-sensore-cestino = Recycle Bin items deleted after
configurazione-sensore-download = Downloads files deleted after
configurazione-mai = never
configurazione-mai-configurato = never configured
configurazione-cestino-capacita = Recycle Bin capacity on this drive
configurazione-capacita-predefinita = Windows default
configurazione-cestino-elimina-subito = Delete without the Recycle Bin
configurazione-tracciamento = App launch tracking (Start_TrackProgs)
configurazione-tracciamento-spento = Windows launch tracking is off: no usage data
configurazione-ripristino = System Protection (restore points)
configurazione-cache-npm = npm cache
configurazione-store-pnpm = pnpm store
configurazione-origine-ambiente = from the environment variable
configurazione-origine-npmrc = from .npmrc
configurazione-origine-predefinito = default path from the documentation

fonti-titolo = Sources read during the scan
fonti-disinstallazione = Uninstall entries
fonti-voci = { $n ->
    [one] { $testo } entry
   *[other] { $testo } entries
}{ $saltate ->
    [0] {""}
    [one] , { $saltate_testo } skipped for an unexpected format
   *[other] , { $saltate_testo } skipped for an unexpected format
}
fonti-msix = Microsoft Store packages
fonti-pacchetti = { $n ->
    [one] { $testo } package
   *[other] { $testo } packages
}
fonti-steam = Steam
fonti-giochi = { $n ->
    [one] { $testo } game
   *[other] { $testo } games
}
fonti-epic = Epic Games Launcher
fonti-installazioni = { $n ->
    [one] { $testo } installation
   *[other] { $testo } installations
}
fonti-capcut = CapCut
fonti-versione = version { $versione }
fonti-userassist = Launches recorded by Windows (UserAssist)
fonti-userassist-non-usato = not read, launch tracking is off
fonti-voci-semplici = { $n ->
    [one] { $testo } entry
   *[other] { $testo } entries
}

origine-scansione = scan
origine-disinstallazione-hklm = uninstall entry for all accounts (HKLM)
origine-disinstallazione-wow = 32-bit uninstall entry for all accounts (HKLM, WOW6432Node)
origine-disinstallazione-hkcu = uninstall entry for your account (HKCU)
origine-msix = Microsoft Store package
origine-steam-manifest = Steam installation data (appmanifest)
origine-steam-localconfig = Steam account data on this PC (localconfig.vdf)
origine-epic-manifest = Epic Games Launcher installation data (Manifests)
origine-capcut-productinfo = CapCut version data (ProductInfo.xml)
origine-capcut-config = CapCut settings
origine-cestino-api = Windows Recycle Bin count
origine-alimentazione = Windows Power Options
origine-sensore-memoria = Storage Sense settings
origine-bitbucket = Recycle Bin Properties
origine-npm-config = npm configuration
origine-cartella-nota = path set by Windows
origine-regola = rule
origine-hash-contenuto = content comparison (SHA-256)
origine-indice-zip = list of files in the zip archive
origine-risorsa-versione = file version data (Properties › Details)
origine-proprieta-msi = MSI installer data (Property table)
origine-restart-manager = files in use according to Windows
origine-userassist = launches recorded by Windows (UserAssist)
gestore-app-breve = Programs
gestore-tuoi-breve = Your files
gestore-windows-breve = Windows
gestore-non-classificato-breve = Not classified

## M4: Downloads view.

download-titolo = Downloads
download-metodo = Quado identifies the type from the extension and reads installer versions without running them.
download-meta = { $percorso } · { $dimensione } · { $file }
download-meta-minimo = { $n ->
    [one] One folder wasn't read: the totals leave out its content.
   *[other] { $testo } folders weren't read: the totals leave out their content.
}
download-cartella = Folder
download-cartella-nota = { $codice ->
    [documenti] Documents
    [download] Downloads
    [desktop] Desktop
    [immagini] Pictures
    [video] Videos
    [musica] Music
   *[other] Folder
}
download-costruzione = Grouping the files… Quado uses the scan data and doesn't open the files.
download-non-trovata = The scan doesn't include the folder { $percorso }.
download-non-indicata = Windows didn't report the Downloads folder for this account.
download-scegli = Choose another folder from the "Folder" menu, or from Explore with "Open in the Downloads view".

lente-famiglia-installatori = Installers
lente-famiglia-archivi = Archives
lente-famiglia-immagini-disco = Disk images
lente-famiglia-video-audio = Video and audio
lente-famiglia-documenti-immagini = Documents and pictures
lente-famiglia-download-incompleti = Incomplete downloads
lente-famiglia-altro = Other files
download-riquadro = { $famiglia }: { $dimensione }, { $file }
download-altri-file = Other files: { $dimensione } in { $file }

download-filtro-suffisso = With "(N)" in the name: { $n ->
    [one] one file
   *[other] { $testo } files
}
download-filtro-titolo = Files with "(N)" in the name
download-filtro-nota = The filter shows files with "(N)" in the name and leaves entries and states as they are. The suffix doesn't prove that two files are copies: only the content comparison does.
download-filtro-vuoto = No files with "(N)" in the name in this folder.

download-comando-confronta = Compare the content
download-comando-installatori = Read version data
download-comando-archivi = Read archive indexes
download-comando-uso = Check use again
download-comando-uso-suggerimento = Asks Windows which programs have these files open, without reading their content.
download-motivo-gia-confrontati = All files of the same size have already been compared.
download-motivo-cloud = The files of the same size are only in the cloud: Quado doesn't read them, so they aren't downloaded.
download-motivo-nessun-gruppo = There are no files of the same size to compare.
download-motivo-nessun-installatore = No exe or msi installers to read in this folder.
download-motivo-nessun-archivio = No zip archive with a folder of the same name next to it.
download-motivo-lettura-in-corso = Quado runs one reading at a time: this command becomes available when the other one ends or you stop it.

download-confronta-domanda = { $n ->
    [one] The comparison reads the file in full ({ $byte }) and computes its fingerprint, on your PC. It updates the file's last access date.
   *[other] The comparison reads the { $testo } files in full ({ $byte }) and computes their fingerprints, on your PC. It updates the last access date of these files.
}
download-confronta-cloud = { $n ->
    [one] Quado skips one file that is only in the cloud, so it isn't downloaded.
   *[other] Quado skips { $testo } files that are only in the cloud, so they aren't downloaded.
}
download-installatori-domanda = { $n ->
    [one] The reading opens the installer ({ $byte }) only where its name and version are stored, without running it. It updates the file's last access date.
   *[other] The reading opens the { $testo } installers ({ $byte } in all) only where name and version are stored, without running them. It updates the last access date of these files.
}
download-archivi-domanda = { $n ->
    [one] The reading opens the archive ({ $byte }) only at the end, where the file index is, without decompressing it. It updates the file's last access date.
   *[other] The reading opens the { $testo } archives ({ $byte } in all) only at the end, where the file index is, without decompressing them. It updates the last access date of these files.
}
download-conferma-confronta = Compare
download-conferma-leggi = Read
download-annulla = Cancel
download-annulla-lettura = Stop reading

download-avanzamento-confronto = Comparing… { $file } of { $totale } files read ({ $letti } of { $byte })
download-avanzamento-installatori = Reading version data… { $file } of { $totale } files
download-avanzamento-archivi = Reading indexes… { $file } of { $totale } archives
download-avanzamento-attributi = Checking the installers… { $file } of { $totale }. Quado looks only at size and date, without reading the content.

download-esito-confronto = Comparison complete: { $letti } of { $totale } files read.
download-esito-confronto-interrotto = Comparison stopped: { $letti } of { $totale } files read. Groups read only in part stay with their content not compared.
download-esito-installatori = Version data read: { $letti } of { $totale } files.
download-esito-installatori-interrotto = Version data reading stopped: { $letti } of { $totale } files read. The others are still to be read.
download-esito-installatori-cache = The version data comes from the reading already done in this session: the files haven't changed since.
download-esito-archivi = Indexes read: { $letti } of { $totale }.
download-esito-archivi-interrotto = Index reading stopped: { $letti } of { $totale } indexes read.
download-esito-non-letti = { $n ->
    [one] One file wasn't read: the reason is in the details of its entry.
   *[other] { $testo } files weren't read: the reasons are in the details of their entries.
}

download-elenco-titolo = What Quado recognizes in this folder
download-elenco-regola = Name, date or size alone aren't enough to suggest an action.
download-elenco-vuoto = Quado doesn't recognize the role of any file in this folder.
download-elenco-accessibile = Folder entries
download-legenda-gestisce = Managed by a program or Windows

download-voce-stessa-dimensione = Same-size files, content not compared
download-voce-copie = Identical copies
download-voce-contenuto-diverso = Same size, different content
download-voce-installatori-non-letti = { $n ->
    [one] Installer with unread version data
   *[other] Installers with unread version data
}
download-voce-installatori-cloud = { $n ->
    [one] Installer only in the cloud
   *[other] Installers only in the cloud
}
download-voce-installatori-non-riusciti = { $n ->
    [one] Installer with unreadable version data
   *[other] Installers with unreadable version data
}
download-voce-installatori-stessa-versione = { $n ->
    [one] Installer of a program already installed at the same version
   *[other] Installers of programs already installed at the same version
}
download-voce-installatori-altra-versione = { $n ->
    [one] Installer of a program installed in a different version
   *[other] Installers of programs installed in a different version
}
download-voce-installatori-non-installati = { $n ->
    [one] Installer of a program that isn't installed
   *[other] Installers of programs that aren't installed
}
download-voce-installatori-senza-confronto = { $n ->
    [one] Installer not compared with the installed programs
   *[other] Installers not compared with the installed programs
}
download-voce-archivio = Archive with a folder of the same name next to it
download-voce-immagini-disco = { $n ->
    [one] Disk image
   *[other] Disk images
}
download-voce-incompleti-liberi = { $n ->
    [one] Incomplete download not in use
   *[other] Incomplete downloads not in use
}
download-voce-incompleti-in-uso = { $n ->
    [one] Incomplete download in use
   *[other] Incomplete downloads in use
}
download-voce-incompleti-uso-non-noto = { $n ->
    [one] Incomplete download, use unknown
   *[other] Incomplete downloads, use unknown
}
download-voce-incompleti-non-controllati = { $n ->
    [one] Incomplete download, use not checked
   *[other] Incomplete downloads, use not checked
}
download-nomi = { $primo } and { $altri ->
    [one] one more file
   *[other] { $altri_testo } more files
}
download-riga-gruppi = { $n ->
    [one] One group
   *[other] { $testo } groups
}, { $file }

download-dettaglio-vuoto = Select an entry to see the facts, the files and what you can do.
download-dettaglio-sottotitolo = { $file } · { $dimensione }
download-dettaglio-altre-voci = { $n ->
    [one] and one more entry
   *[other] and { $testo } more entries
}
download-altre-voci = { $n ->
    [one] Show one more entry
   *[other] Show more entries ({ $testo } left)
}
download-dettaglio-altri-file = { $n ->
    [one] and one more file
   *[other] and { $testo } more files
}
download-non-letto = { $nome } not read: { $motivo }
download-non-letto-assente = the file no longer exists
download-non-letto-accesso = Windows denies access
download-non-letto-in-uso = it's in use by another program, which keeps it open exclusively
download-non-letto-errore = Windows error (Code { $codice })
download-non-letto-cloud = it's only in the cloud and Quado doesn't read it, so it isn't downloaded
download-non-letto-cambiato = it changed during the reading, so the fingerprint isn't valid
download-non-letto-non-file = it isn't a file (link or folder)
download-non-letto-annullato = reading stopped
download-origine = Quado built this entry from the scan measurements and the readings asked for in this session, without using a rule.
download-installatori-altri-tipi = Quado doesn't read version data from MSIX and APPX packages or MSP patches: they stay without comparison.
download-caselle-dopo = After "Compare the content", if the files turn out identical, choose which copy to keep.
download-tieni-nota = Quado keeps the copy created first unless you choose another one. The others go to the Recycle Bin only with "Move to the Recycle Bin…", after the preview and your confirmation.
download-file-creato = Created on { $data }
download-file-modificato = Modified on { $data }
download-tenuta = kept
download-da-spostare = to move to the Recycle Bin, after the preview
download-casella = Select { $nome }
download-tieni = Keep { $nome }
download-famiglia-nota = Type recognized from the extension. The largest files:
download-apri-lente = Open in the Downloads view
download-apri-lente-suggerimento = Shows file types and entries of this folder from the scan data, without opening the files.
interventi-sottotitolo-lente = Each entry comes from a rule, with its source and verification date, or from evidence measured in the Downloads view. Select an entry to see in the inspector what you can do. Quado changes nothing on the disk from this card.

## M4: Programs.

programmi-titolo = Programs
programmi-sottotitolo = Programs registered in Windows, Microsoft Store apps, Steam games and Epic products. Quado counts a program's space only when a source declares its folder.
programmi-tessera-programmi = Programs listed
programmi-tessera-senza-componenti = Without system components
programmi-tessera-con-componenti = With system components
programmi-tessera-spazio = Measured space
programmi-tessera-spazio-nota = Size on disk. A folder declared by several programs counts once. Declared sizes stay out of the total.
programmi-tessera-non-dichiarata = Location not declared
programmi-tessera-non-dichiarata-nota = Quado attributes space only with proof of the path, never by a similar name.
programmi-tracciamento-spento = Windows launch tracking is off: no usage data
programmi-tracciamento-attivo = Windows launch tracking is on: the last launch comes from UserAssist. UserAssist counts launches from File Explorer and Start. Launches from the command line, other launchers and scheduled tasks are left out.
programmi-tracciamento-non-letto = Launch tracking setting not read: no usage data from UserAssist.
programmi-uso-copertura = Steam records the last session of its games; Epic and Microsoft Store don't record usage.
programmi-fonte-errore = { $fonte }: { $motivo }
programmi-fonte-errore-elemento = { $fonte }, { $percorso }: { $motivo }
programmi-sezione-programmi = Programs ({ $testo })
programmi-sezione-residui = Possible leftovers ({ $testo })
programmi-mostra-componenti = Show system components ({ $testo })
programmi-cerca = Search programs by name
programmi-colonna-fonte = Source
programmi-colonna-versione = Version
programmi-colonna-dichiarata = Declared size
programmi-colonna-uso = Last known use
programmi-colonna-installazione = Installed on
programmi-colonna-base = Base folder
programmi-colonna-stato = State
programmi-fonte-hklm = Registry, all accounts
programmi-fonte-wow = Registry, 32-bit
programmi-fonte-hkcu = Registry, your account
programmi-fonte-msix = Microsoft Store
programmi-fonte-steam = Steam
programmi-fonte-epic = Epic
programmi-cella-non-dichiarata = location not declared
programmi-cella-non-misurato = not measured
programmi-cella-non-plausibile = no space attributed
programmi-cella-dichiarata = { $dimensione } ({ $fonte })
programmi-dichiarata-registro = registry
programmi-dichiarata-steam = Steam
programmi-uso-steam = { $data } (Steam)
programmi-uso-steam-nessuna = no session recorded
programmi-uso-userassist = { $data } (UserAssist)
programmi-uso-userassist-nessuno = no launch in UserAssist
programmi-uso-nessun-dato = no usage data
programmi-tag-componente = system component
programmi-tag-condivisa = folder in common
programmi-accessibile-tabella = Program list
programmi-accessibile-residui = List of possible leftovers
programmi-vuoto-senza-fonti = This scan didn't read the program sources: the list is empty.
programmi-vuoto-nessuno = The sources that were read list no programs.
programmi-vuoto-ricerca = No program has this text in its name. Clear the search to return to the list.
programmi-residui-nessuno = No possible leftovers in Local, Roaming, LocalLow and ProgramData.
programmi-nessuna-selezione = Select a program to see its source, space and known use.
programmi-residuo-nessuna-selezione = Select a folder to see its measurements.
programmi-residui-regola = Folders in Local, Roaming, LocalLow and ProgramData that Quado doesn't match to an installed program or a rule. The comparison is by name only and can be wrong, so Quado suggests no action. To decide, look at the folder with "Show in File Explorer".
programmi-residuo-base = Base folder: { $base }
programmi-tipo-hklm = Program registered in Windows for all accounts (HKLM)
programmi-tipo-wow = 32-bit program registered in Windows (HKLM, WOW6432Node)
programmi-tipo-hkcu = Program registered in Windows for your account (HKCU)
programmi-tipo-msix = Microsoft Store app
programmi-tipo-steam = Steam game
programmi-tipo-epic = Epic Games Launcher product
programmi-misurato-nota = Size on disk, measured in this scan
programmi-titolo-non-dichiarata = Location not declared
programmi-titolo-non-misurato = Not measured
programmi-titolo-non-plausibile = No space attributed
programmi-sezione-posizione = Location
programmi-sezione-spazio = Space
programmi-sezione-uso = Known use
programmi-sezione-dati = Data recognized by the rules
programmi-sezione-funzione = Function
programmi-prova-installlocation = Proof of the path: InstallLocation of the uninstall entry.
programmi-prova-indizio = Path hint: the folder of the executable named by the uninstall command or the icon. Quado reads that command and never runs it.
programmi-prova-msix = Proof of the path: InstalledLocation of the package.
programmi-prova-steam = Proof of the path: libraryfolders.vdf and the game manifest.
programmi-prova-epic = Proof of the path: InstallLocation of the Epic manifest.
programmi-provenienza = Origin of the folder: { $classe }
programmi-condivisa-con = Same folder as: { $nomi }
programmi-dati-dentro = { $n ->
    [one] One item in the program folder
   *[other] { $n_testo } items in the program folder
}
programmi-dati-fuori = { $n ->
    [one] One item outside the folder, managed by this program
   *[other] { $n_testo } items outside the folder, managed by this program
}
programmi-dichiarato-nota = The declared size comes from the installer or from Steam and can be out of date. Quado shows it next to the measured size and keeps it out of totals.
programmi-copertura-steam = Steam records sessions of games started from Steam; launches outside Steam might be missing.
programmi-disinstalla = Uninstall…
programmi-mostra-in-esplora = View in Explore
programmi-dialogo-titolo = Open Settings to uninstall { $programma }?
programmi-dialogo-titolo-steam = Open Steam to uninstall { $programma }?
programmi-dialogo-titolo-guida = How to uninstall { $programma }
programmi-dialogo-sottotitolo = Uninstalling happens in the official interface, where you confirm it. Quado never runs the uninstall command stored in the registry.
programmi-dialogo-impostazioni = Quado copies the name "{ $programma }" to the clipboard and opens Settings › Apps, where you paste it in the search box.
programmi-dialogo-msix = Quado opens the page of { $programma } in Settings › Apps.
programmi-dialogo-steam = Quado opens Steam, which asks for confirmation before uninstalling { $programma }.
programmi-dialogo-epic = To uninstall { $programma }, open the library in the Epic launcher, choose the three dots next to its name, then Uninstall. Quado can't open that page.
programmi-dialogo-esc = Esc cancels.
programmi-dialogo-annulla = Cancel
programmi-dialogo-apri-impostazioni = Open Settings
programmi-dialogo-apri-steam = Open Steam
programmi-dialogo-chiudi = Close
programmi-dialogo-aperto-impostazioni = Settings › Apps opens. The name "{ $programma }" is on the clipboard: paste it in the search box.
programmi-dialogo-aperto-msix = Settings › Apps opens, on the page of { $programma }.
programmi-dialogo-aperto-steam = Steam opens: confirm the uninstall of { $programma } there.
programmi-dialogo-errore = Can't open the page. Try again, or open it yourself. (Code { $codice })
programmi-dialogo-non-valido = Can't open the page: the package name or the game number isn't valid. Open it yourself from Settings › Apps or from Steam.

## M5: actions, common parts (03 F-AC, F-VE).

nav-interventi = Actions
azione-prepara-cestino = Move to the Recycle Bin…
azione-apri-procedura = Open the steps
azione-ho-eseguito = I've completed the steps

## M5: move to the Recycle Bin, confirmation and result (agent E1).

spostamento-titolo = { $n ->
    [one] Move one item of "{ $nome }" to the Recycle Bin?
   *[other] Move { $n_testo } items of "{ $nome }" to the Recycle Bin?
}
spostamento-sottotitolo = Preview: Quado moves the items only when you choose "Move to the Recycle Bin".
spostamento-chiudi = Close without moving
spostamento-elementi-scelti = Selected items
spostamento-simulazione-titolo = Preview of the move to the Recycle Bin: { $nome }
spostamento-simulazione-testo = Quado reads only the attributes of the items again, then runs the checks. Files stored only in the cloud aren't downloaded.
spostamento-simulazione-letti = { $n ->
    [one] One file or folder read
   *[other] { $testo } files and folders read
}
spostamento-riga-prove = Evidence: { $prove }
spostamento-riga-regola = Rule: { $nome } ({ $id }), verified on { $data }
spostamento-riga-funzione = Quado function: { $nome }. It uses only evidence measured or read in this session.
spostamento-numero-da-spostare = To move
spostamento-numero-subito = Freed right away
spostamento-numero-dopo = After emptying the Recycle Bin
spostamento-stima-esatta = exact estimate
spostamento-stima-intervallo-valore = from { $minimo } to { $massimo }
spostamento-stima-intervallo = range estimate: some folders can be read only as administrator
spostamento-stima-nessun-valore = can't be estimated
spostamento-numeri-nota = The Recycle Bin keeps the files: the space is freed when you or Storage Sense empty it.
spostamento-esclusi-nomi = { $n ->
    [one] One file left out of the estimate ({ $byte }): it has other names (hard links) outside the selection, so its space isn't freed.
   *[other] { $n_testo } files left out of the estimate ({ $byte }): they have other names (hard links) outside the selection, so their space isn't freed.
}
spostamento-cartelle-negate = { $n ->
    [one] One folder in the selection can be read only as administrator: the estimate doesn't include its content.
   *[other] { $n_testo } folders in the selection can be read only as administrator: the estimate doesn't include their content.
}
spostamento-elenco = Items
spostamento-selezionati = { $n ->
    [one] { $testo } of { $totale } selected
   *[other] { $testo } of { $totale } selected
}
spostamento-fisso-versione-in-uso = excluded: it's the version in use
spostamento-fisso-versione-tenuta = kept: it's the previous version
spostamento-fisso-copia = kept: it's the copy that stays
spostamento-blocco-protetto-windows = Can't be moved: it's a Windows folder.
spostamento-blocco-protetto-winsxs = Can't be moved: it's the Windows component store (WinSxS).
spostamento-blocco-protetto-installer = Can't be moved: it's the Windows Installer cache.
spostamento-blocco-protetto-svi = Can't be moved: it's System Volume Information, a Windows system folder.
spostamento-blocco-protetto-paging = Can't be moved: it's the Windows paging file.
spostamento-blocco-protetto-ibernazione = Can't be moved: it's the Windows hibernation file.
spostamento-blocco-protetto-swap = Can't be moved: it's the Windows swap file.
spostamento-blocco-protetto-programma = Can't be moved: it's in the folder of an installed program.
spostamento-blocco-senza-cestino = This drive has no Recycle Bin: Windows would delete the item permanently.
spostamento-blocco-elimina-subito = This drive's Recycle Bin is set to delete files right away: the item couldn't be restored.
spostamento-blocco-capacita-ignota = Quado doesn't know how much this drive's Recycle Bin can hold, so the item stays where it is.
spostamento-blocco-piu-grande = A { $dimensione } item in a { $capacita } Recycle Bin: Windows wouldn't move it to the Recycle Bin and would ask to delete it permanently.
spostamento-blocco-in-uso = In use by { $processi }
spostamento-processo = { $nome } (PID { $pid })
spostamento-blocco-restart-manager = Windows didn't say whether the files are in use, so the item stays where it is. (Code { $codice })
spostamento-blocco-riferimenti = Pointing to this item: { $elenco }
spostamento-riferimento = { $dove } ({ $tipo })
spostamento-rif-uninstall-string = uninstall command
spostamento-rif-display-icon = icon of an installed program
spostamento-rif-install-location = install folder declared by a program
spostamento-rif-app-paths = app path in the registry (App Paths)
spostamento-rif-run-utente = startup for your account
spostamento-rif-run-sistema = startup for all accounts
spostamento-rif-collegamento = shortcut
spostamento-rif-attivita = scheduled task
spostamento-blocco-percorso-lungo = Path of 260 characters or more: Windows wouldn't move it to the Recycle Bin and would ask to delete it permanently.
spostamento-blocco-contiene-lungo = It contains a path of 260 characters or more: Windows wouldn't move the folder to the Recycle Bin and would ask to delete it permanently.
spostamento-blocco-illeggibile = Can't read the item, so it stays where it is. (Code { $codice })
spostamento-blocco-mancante = It no longer exists.
spostamento-blocco-altra-unita = It's on a different drive from the other items.
spostamento-controlli = Checks
spostamento-rm-ok = No file is in use, according to Windows (checked at { $ora }).
spostamento-rm-in-uso = { $n ->
    [one] One item is in use according to Windows (checked at { $ora }) and stays where it is. To move it, close the program that uses it and choose "Move to the Recycle Bin…" again.
   *[other] { $n_testo } items are in use according to Windows (checked at { $ora }) and stay where they are. To move them, close the programs that use them and choose "Move to the Recycle Bin…" again.
}
spostamento-rm-errore = { $n ->
    [one] For one item Windows didn't say whether the files are in use: it stays where it is.
   *[other] For { $n_testo } items Windows didn't say whether the files are in use: they stay where they are.
}
spostamento-rif-ok = No shortcut, registry entry or scheduled task points to these items.
spostamento-rif-trovati = { $n ->
    [one] Shortcuts, registry entries or scheduled tasks point to one item: it stays where it is.
   *[other] Shortcuts, registry entries or scheduled tasks point to { $n_testo } items: they stay where they are.
}
spostamento-rif-parziale = Partial check. Sources not read in full: { $fonti }.
spostamento-capacita-ok = The Recycle Bin of { $volume } can hold this selection: it would reach { $totale } of { $capacita }.
spostamento-capacita-superata = Move blocked: the Recycle Bin of { $volume } can hold { $capacita } and with this selection it would reach { $totale }. Beyond that limit Windows deletes the oldest items on its own.
spostamento-capacita-ignota = Move blocked: can't read the capacity or the content of the Recycle Bin of { $volume }.
spostamento-sensore = { $giorni ->
    [one] Storage Sense automatically deletes items that stay in the Recycle Bin for more than one day.
   *[other] Storage Sense automatically deletes items that stay in the Recycle Bin for more than { $giorni_testo } days.
}
spostamento-controllo-regola = { $controllo }: { $esito }
spostamento-manuali = Tick these before moving
spostamento-manuale-elemento = { $nome }: { $testo }
spostamento-sincronizzato = This item is synced with { $provider }: moving it to the Recycle Bin also deletes it in the cloud and on your other devices. To free space without deleting it use "Free up space" in { $provider }.
spostamento-sincronizzato-cloud = This item is in the { $provider } folder: moving it to the Recycle Bin also deletes it in the cloud and on your other linked devices.
spostamento-sincronizzato-dispositivi = This item is in a folder synced with { $provider }: moving it to the Recycle Bin also deletes it on the other devices that sync the folder.
spostamento-sinc-ok = No item is in a synced folder that Quado recognizes. Quado recognizes the folders registered with Windows, such as OneDrive's, and those of Dropbox, Syncthing and Resilio Sync. Check folders synced by other programs yourself.
spostamento-sincronizzato-ripristino = Files that are only in the cloud don't reach this PC's Recycle Bin: you recover them from the { $provider } trash on the web.
spostamento-reversibilita = You can restore the items from the Recycle Bin until you or Storage Sense empty it: open the Recycle Bin, select the items and choose "Restore the selected items". If Windows asks to permanently delete an item, choose No: the item stays where it is.
spostamento-ipotesi-rischi = Assumptions and risks
spostamento-seleziona-tutti = Select all
spostamento-ripristina = To put them back: open the Recycle Bin, select the items and choose "Restore the selected items".
spostamento-punti-ripristino = With restore points on, part of the freed space may be taken again in the following days.
spostamento-punti-ripristino-ignoti = Can't read the state of restore points. If they're on, part of the freed space may be taken again in the following days.
spostamento-scheda = Recommendation details
spostamento-scheda-identita = Proof of identity
spostamento-scheda-regola = { $nome } · { $id }
spostamento-scheda-fonte = Source: { $titolo } · { $tipo }
spostamento-esc = Esc cancels.
spostamento-annulla = Cancel
spostamento-sposta = Move to the Recycle Bin
spostamento-manca-scelta = Select at least one item.
spostamento-manca-spunta = Tick the checks above to move the items.
spostamento-manca-capacita = The Recycle Bin capacity check blocks the move.
spostamento-manca-blocco = A selected item has a check that didn't pass.
spostamento-manca-azione = This recommendation doesn't suggest a move to the Recycle Bin.
spostamento-occupato = A move is already in progress: wait for it to finish.
spostamento-risultato-titolo = Result of the move
spostamento-risultato-quando = { $nome } · { $data } at { $ora }
spostamento-registro-scritto = Action recorded in the history below
spostamento-registro-in-corso = Recording…
spostamento-registro-errore = Can't write the action log: this action won't appear in the history. (Code { $codice })
spostamento-registro-nessuna-cartella = Action not recorded: the Quado data folder isn't available.
spostamento-passo-spostamento = Move
spostamento-in-corso = Moving… { $fatti } of { $totale }
spostamento-partenza-errore = Can't measure the free space before moving, so Quado moved nothing. (Code { $codice })
spostamento-spostati = { $n ->
    [one] One of { $totale } items moved to the Recycle Bin
   *[other] { $n_testo } of { $totale } items moved to the Recycle Bin
}
spostamento-nessuno-saltato = Quado checked each item again right before moving it: all of them matched the preview.
spostamento-ricontrollati = Quado checked each item again right before moving it. The items not moved are below, with the reason.
spostamento-esito-riga = { $nome }: { $esito }
spostamento-esito-cambiato = skipped because it changed after the preview. It's still where it was.
spostamento-esito-mancante = skipped because it was no longer there.
spostamento-esito-in-uso = skipped because it was in use at the second check. It's still where it was.
spostamento-esito-illeggibile = skipped because the second check couldn't read it. It's still where it was. (Code { $codice })
spostamento-esito-fallito = not moved by Windows. It's still where it was. (Code { $codice })
spostamento-esito-non-nel-cestino = it left its place without reaching the Recycle Bin. Quado stopped here and left the next items where they were. (Code { $codice })
spostamento-esito-non-raggiunto = not moved: Quado stopped before reaching it. It's still where it was.
spostamento-parziale = { $n ->
    [one] One item wasn't moved. The moved items ({ $m }) are in the Recycle Bin: to put them back, open the Recycle Bin, select them and choose "Restore".
   *[other] { $n_testo } items weren't moved. The moved items ({ $m }) are in the Recycle Bin: to put them back, open the Recycle Bin, select them and choose "Restore".
}
spostamento-nessuno = No item was moved to the Recycle Bin.
spostamento-passo-spazio = Free space
spostamento-spazio-prima = Before
spostamento-spazio-ora = Now
spostamento-spazio-differenza = Difference
spostamento-spazio-nota = Free space stays about the same while the Recycle Bin keeps the files: it's freed when you or Storage Sense empty it.
spostamento-misura-errore = Can't measure the free space: the comparison with the estimate is missing. (Code { $codice })
spostamento-piu = +{ $valore }
spostamento-meno = -{ $valore }
spostamento-passo-cestino = Deleting from the Recycle Bin
spostamento-apri-testo = { $n ->
    [one] Open the Recycle Bin and delete only the item moved by Quado on { $data } at { $ora }:
   *[other] Open the Recycle Bin and delete only the { $n_testo } items moved by Quado on { $data } at { $ora }:
}
spostamento-apri = Open the Recycle Bin
spostamento-svuota-testo = { $n ->
    [one] The Windows dialog also deletes one other item of { $byte }, according to the Windows count.
   *[other] The Windows dialog also deletes { $n_testo } other items of { $byte }, according to the Windows count.
}
spostamento-svuota-nessun-altro = According to the Windows count, the Recycle Bin holds only the items moved now.
spostamento-svuota-ignoto = The Windows dialog also deletes the other items in the Recycle Bin. Windows didn't say how many there are.
spostamento-svuota = Empty the Recycle Bin…
spostamento-svuota-in-corso = The Windows dialog is open: confirm or cancel there.
spostamento-svuota-errore = The Recycle Bin wasn't emptied: the dialog was cancelled or Windows reported an error. (Code { $codice })
spostamento-passo-verifica = Check after emptying
spostamento-verifica-attesa = When you empty the Recycle Bin with "Empty the Recycle Bin…", Quado measures the free space and the touched folders again, then compares the result with the estimate.
spostamento-confronto-dopo-azione = Right after the move
spostamento-confronto-dopo-svuotamento = After emptying the Recycle Bin
spostamento-stimato = Estimated
spostamento-variazione = Change in free space
spostamento-variazione-nota = The change also includes what other programs wrote or deleted in the meantime.
spostamento-differenza = Difference: { $valore } ({ $percento })
spostamento-differenza-senza-percento = Difference: { $valore }
spostamento-differenza-nessuna = The change falls within the estimate.
spostamento-solo-ottenuto = With no estimated value, only the change in free space counts.
spostamento-voce-saltati = { $n ->
    [one] One item not moved: { $byte } of the estimate stay where they were.
   *[other] { $n_testo } items not moved: { $byte } of the estimate stay where they were.
}
spostamento-voce-altri-nomi = { $n ->
    [one] One file has other names (hard links) outside the selection ({ $byte }): its space is freed only when no name is left. The estimate already leaves it out.
   *[other] { $n_testo } files have other names (hard links) outside the selection ({ $byte }): their space is freed only when no name is left. The estimate already leaves them out.
}
spostamento-voce-nel-cestino = Still in the Recycle Bin: { $byte }. This space is freed when the Recycle Bin is emptied.
spostamento-voce-altri-elementi = { $n ->
    [one] Emptying the Recycle Bin also deleted one other item, of { $byte } according to the Windows count.
   *[other] Emptying the Recycle Bin also deleted { $n_testo } other items, of { $byte } according to the Windows count.
}
spostamento-voce-file-aperti = { $n ->
    [one] One file of the selection was still in use when the Recycle Bin was emptied, according to Windows.
   *[other] { $n_testo } files of the selection were still in use when the Recycle Bin was emptied, according to Windows.
}
spostamento-voce-altri-processi = Activity of other programs: { $valore }. Rescanning the folders confirms that the moved items take no space anymore, so this part of the difference doesn't come from the action.
spostamento-non-spiegata = Unexplained difference: { $valore } ({ $percento }).
spostamento-regola-5 = Above a 5% difference, Quado explains it item by item or declares it unexplained.
spostamento-riscansione-in-corso = Rescanning the touched folders…
spostamento-riscansione-fatta = Touched folders rescanned: the totals are up to date.
spostamento-riscansione-radice = Some items were in the root of the drive. To update the totals choose "Rescan": Quado reads the whole drive again.
spostamento-riscansione-fallite = { $n ->
    [one] Can't rescan one touched folder: its totals stay as before. To update them choose "Rescan".
   *[other] Can't rescan { $n_testo } touched folders: their totals stay as before. To update them choose "Rescan".
}

## M5: Actions view, steps and commands, Data and privacy (agent E2).

procedura-apri-cestino = Open the Recycle Bin
procedura-apri-archiviazione = Open Settings › Storage
procedura-apri-file-temporanei = Open Settings › Temporary files
procedura-misura-partenza = Measure free space
procedura-misura-partenza-suggerimento = Reads the free space and the Recycle Bin content according to Windows. Then follow the steps in the app and choose "I've completed the steps".
procedura-svuota-cestino = Empty the Recycle Bin…
procedura-cestino-conteggio = The Windows dialog permanently deletes { $n ->
    [one] the only item in the Recycle Bin
   *[other] all { $testo } items in the Recycle Bin
}: { $dimensione } according to the Windows count. To delete only some of them use "Open the Recycle Bin".
procedura-cestino-lettura = Reading the Recycle Bin…
procedura-cestino-vuoto = The Recycle Bin is empty, according to Windows.
procedura-cestino-non-letto = Windows doesn't say how many items the Recycle Bin holds, so "Empty the Recycle Bin…" is unavailable.
procedura-cestino-errore = Can't read the Recycle Bin, so "Empty the Recycle Bin…" is unavailable. (Code { $codice })
procedura-misura-in-corso = Measuring… free space, Recycle Bin and touched folders
procedura-aperta = Page opened at { $ora }. Free space before the steps: { $libero } on { $volume }. When you're done, choose "I've completed the steps".
procedura-copiato = Command copied at { $ora }. Free space before the command: { $libero } on { $volume }. Quado doesn't run the command: after you run it, choose "I've completed the steps".
procedura-partenza = Free space at { $ora }, before the steps: { $libero } on { $volume }. When you're done, choose "I've completed the steps".
procedura-partenza-errore = Can't measure the free space before the steps: the change can't be computed. (Code { $codice })
procedura-pagina-errore = Can't open the page. Try again, or open it yourself. (Code { $codice })
procedura-pagina-non-valida = Can't open the page for these steps: its address isn't one that Quado opens.
procedura-comando-nota = Quado shows and copies the command, but doesn't run it. "Copy" also measures the starting free space. After you run the command, choose "I've completed the steps" to measure the result.
procedura-senza-partenza = Without a free-space measurement taken before the steps, "I've completed the steps" records only the current free space and doesn't compute the change.
procedura-misura-errore = Can't measure the free space: Quado recorded nothing. (Code { $codice })
procedura-solo-misura = Free space at { $ora }: { $libero }. Without the starting measurement the change isn't computed.
procedura-serve-riesamina = These items are in the root of the drive: to update the totals choose "Rescan".
procedura-riscansione-errore = Can't rescan { $n ->
    [one] one folder
   *[other] { $testo } folders
}: the totals stay as before. (Code { $codice })
procedura-svuota-codice = The Windows dialog closed with a code: the measurements show what changed. (Code { $codice })
procedura-registrato = Action recorded in the Actions view.
procedura-registro-errore = Can't write the action log: this measurement won't appear in the Actions view. (Code { $codice })
procedura-disinstallazione-dopo = When you're done, choose "I've completed the steps": Quado measures the free space and the program folder again. "Rescan" reads the whole drive again.

storico-piu = +{ $valore }
storico-meno = -{ $valore }
storico-intervallo = from { $minimo } to { $massimo }
storico-verifica = Estimated: { $stimato }. Change in free space: { $variazione }. The change also includes what other programs wrote or deleted in the meantime.
storico-verifica-solo-variazione = Change in free space: { $variazione }. The change also includes what other programs wrote or deleted in the meantime.
storico-differenza-oltre = Difference: { $differenza } ({ $percentuale }), above 5%.
storico-differenza-entro = Difference: { $differenza } ({ $percentuale }), within 5%.
storico-differenza = Difference: { $differenza }.
storico-non-spiegata = Unexplained difference: { $differenza } ({ $percentuale }).
storico-spiegazione-saltati = { $n ->
    [one] One item not moved
   *[other] { $testo } items not moved
}: { $dimensione } of the estimate stay where they were.
storico-spiegazione-altri-nomi = { $n ->
    [one] One file left out of the estimate because it has other names (hard links) outside the selection
   *[other] { $testo } files left out of the estimate because they have other names (hard links) outside the selection
}: { $dimensione }.
storico-spiegazione-nel-cestino = Still in the Recycle Bin: { $dimensione }.
storico-spiegazione-altri-elementi = Emptying the Recycle Bin also deleted { $n ->
    [one] one other item
   *[other] { $testo } other items
}: { $dimensione }.
storico-spiegazione-file-aperti = { $n ->
    [one] One file was still in use
   *[other] { $testo } files were still in use
} when the Recycle Bin was emptied.
storico-spiegazione-altri-processi = Activity of other programs: { $variazione }. The rescan confirms that the items take no space anymore.
storico-ancora-presenti = According to the rescan, the items still take up { $dimensione }.

storico-titolo = Action history
storico-sottotitolo = Measurements before and after each action, with the estimate and the change in free space. Log: { $percorso }
storico-non-disponibile = History not available in this session: the Quado data folder is missing.
storico-errore = Can't read the action log. The file stays as it is, and you can find it in Settings › Data and privacy. (Code { $codice })
storico-illeggibili = { $n ->
    [one] One line of the log can't be read, so the history leaves it out. It may be half written or come from another version of Quado.
   *[other] { $testo } lines of the log can't be read, so the history leaves them out. They may be half written or come from another version of Quado.
}
storico-vuoto = No actions recorded. Moves to the Recycle Bin and steps measured by Quado appear here.
storico-vuoto-azione = See possible actions
storico-numero = { $n ->
    [one] One action recorded
   *[other] { $testo } actions recorded
}, most recent first.
storico-voce = { $data } at { $ora } · { $tipo } · { $origine }
storico-tipo-spostamento = Move to the Recycle Bin
storico-tipo-procedura = Steps: { $nome }
storico-tipo-procedura-app = Steps in the app that manages the data
storico-tipo-comando = Official command, copied and not run by Quado
storico-procedura-cestino-apri = Open the Recycle Bin
storico-procedura-cestino-svuota = Empty the Recycle Bin
storico-procedura-archiviazione = Settings › Storage
storico-procedura-file-temporanei = Settings › Temporary files
storico-procedura-disinstallazione = uninstalling the program
storico-elementi = Items: { $n ->
    [one] one
   *[other] { $testo }
} ({ $esiti })
storico-elementi-nessuno = Items: none
storico-elementi-procedura = Items: none touched by Quado
storico-esito-nel-cestino = { $testo } in the Recycle Bin
storico-esito-saltati = { $n ->
    [one] { $testo } skipped
   *[other] { $testo } skipped
}
storico-esito-falliti = { $n ->
    [one] { $testo } not moved by Windows
   *[other] { $testo } not moved by Windows
}
storico-esito-non-nel-cestino = { $n ->
    [one] { $testo } didn't reach the Recycle Bin
   *[other] { $testo } didn't reach the Recycle Bin
}
storico-esito-non-raggiunti = { $n ->
    [one] { $testo } not reached
   *[other] { $testo } not reached
}
storico-metodo = Method: { $metodo } · drive { $volume }
storico-metodo-nessuno = none
storico-una-misura = Free space at { $ora }: { $libero }. Without the starting measurement the change isn't computed.
storico-nessuna-misura = No measurement recorded.
storico-senza-confronto = Measurements recorded without a comparison with the estimate.

privacy-titolo = Data and privacy
privacy-nota = Quado doesn't use the network. It saves only these files, in { $cartella } on your PC.
privacy-non-disponibile = The data folder is missing in this session, so Quado doesn't save settings or the action log.
privacy-niente = Quado hasn't saved anything yet.
privacy-file-impostazioni = Settings: { $dimensione }
privacy-file-registro = Action log: { $dimensione }
privacy-file-lingue = Language files: { $dimensione }
privacy-cancella-registro = Delete the action log…
privacy-registro-assente = There's no action log yet: it's created with the first action.
privacy-conferma = Delete the action log ({ $dimensione })? The history in the Actions view is cleared and can't be recovered. Settings, languages and items in the Recycle Bin stay as they are.
privacy-cancella-conferma = Delete the log
privacy-annulla = Cancel
privacy-registro-cancellato = Action log deleted.
privacy-registro-errore = Can't delete the action log: the file is still in the data folder. (Code { $codice })
privacy-rilievo-titolo = Rilievo data, the previous version
privacy-rilievo-nota = { $dimensione } in { $cartella }. Quado copies its settings, log and languages only on the first start, then stops using it.
privacy-rilievo-cancella = Delete the Rilievo folder…
privacy-rilievo-conferma = Delete the Rilievo folder ({ $dimensione })? The files can't be recovered. Quado's data stays as it is.
privacy-rilievo-cancella-conferma = Delete the folder
privacy-rilievo-cancellata = Rilievo folder deleted.
privacy-rilievo-errore = Can't delete the whole Rilievo folder: some files are still there. Close Rilievo if it's open, then try again. (Code { $codice })
