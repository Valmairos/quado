## Rule and classification engine texts (05 D6, D13). Context comments are in it.ftl.

regola-capcut-versioni-nome = Previous versions of CapCut
regola-capcut-versioni-riga = { $n ->
    [one] { $n_testo } folder of a previous version (inferred)
   *[other] { $n_testo } folders of previous versions (inferred)
}
regola-capcut-versioni-metodo = Quado moves the folders you choose to the Recycle Bin, after the preview. The version in use and the previous one stay where they are.
regola-capcut-versioni-ipotesi-1 = Assumption: CapCut starts only the version in use, and the other folders are left over from updates.
regola-capcut-versioni-rischio-1 = If the assumption is wrong, CapCut doesn't start. You can restore the folders from the Recycle Bin until you empty it.
regola-capcut-versioni-rischio-2 = Close CapCut before the preview: while it's open, the check of files in use blocks the move.
regola-capcut-versioni-prova = Before you empty the Recycle Bin, open CapCut and check that it works.
regola-capcut-versioni-prova-fatta = I opened CapCut and it works

regola-capcut-progetti-nome = CapCut projects
regola-capcut-progetti-riga = Your video projects. They aren't cache: Quado doesn't suggest deleting them.

regola-capcut-cache-nome = CapCut cache
regola-capcut-cache-riga = CapCut cache: clear it from CapCut's settings.
regola-capcut-cache-metodo = In CapCut: Settings › Draft › Clear Cache (the trash bin icon next to the cache size)

regola-ibernazione-nome = Hibernation file
regola-ibernazione-riga = Windows hibernation file: you can reduce or remove it with powercfg.
regola-ibernazione-metodo = Reduce the hibernation file: keeps fast startup and turns off hibernation.
regola-ibernazione-alternativa-1 = Turn off hibernation: also turns off fast startup and hybrid sleep.
regola-ibernazione-rischio-1 = Both commands turn off hibernation and must run as administrator.
regola-ibernazione-rischio-batteria = When the battery is critical, the laptop can no longer hibernate: if the battery runs out, unsaved work can be lost.
regola-ibernazione-rischio-standby = This PC uses modern standby: while it sleeps, Windows hibernates it to save the battery. Without hibernation the battery can run out completely.

regola-cestino-nome = Recycle Bin
regola-cestino-riga = { $disco } on disk, { $windows } according to Windows
regola-cestino-metodo = Open the Recycle Bin and delete only the items you choose.
regola-cestino-alternativa-1 = Empty the Recycle Bin with the Windows dialog: it deletes every item.

regola-cestino-orfane-nome = Recycle Bin folders without an index
regola-cestino-orfane-riga = The Recycle Bin folder holds { $disco } that the Windows Recycle Bin doesn't list: folders without their index file. Emptying the Recycle Bin doesn't delete them: tested on a virtual disk.
regola-cestino-orfane-ipotesi-1 = Assumption: they're left over from interrupted moves to the Recycle Bin or from removed index files.

regola-winsxs-nome = Windows component store
regola-winsxs-riga = Windows components: Windows shrinks the store by itself. To shrink it sooner, use Disk Cleanup › Clean up system files › Windows Update Cleanup. Deleting files by hand can damage Windows.

regola-windows-installer-nome = Windows Installer cache
regola-windows-installer-riga = Packages needed to repair and uninstall programs: deleting them makes those operations fail.

regola-file-temporanei-nome = Windows temporary files
regola-file-temporanei-riga = Temporary files: delete them from Settings › Temporary files, never by hand.
regola-file-temporanei-metodo = Settings › System › Storage › Temporary files
regola-file-temporanei-alternativa-1 = Windows Disk Cleanup
regola-file-temporanei-rischio-1 = Leave Downloads and Recycle Bin unchecked if you want to keep them: Windows deletes the files without the Recycle Bin.

regola-npm-cache-nome = npm cache
regola-npm-cache-riga = npm cache: npm manages it with its own commands.
regola-npm-cache-metodo = Checks the cache and removes only data that isn't needed: it usually frees little space (inferred).
regola-npm-cache-alternativa-1 = Empties the whole cache: this is the command that frees the space.
regola-npm-cache-rischio-1 = After clean --force, npm downloads the packages again at the next install; offline, it can't find them.

regola-pnpm-store-nome = pnpm store
regola-pnpm-store-riga = pnpm store: pnpm manages it. It holds files with more than one name (hard links), also used by your projects.
regola-pnpm-store-metodo = Removes from the store the packages that no project references.

regola-steam-nome = Steam games
regola-steam-riga = { $nome }, Steam game
regola-steam-metodo = Uninstall from the Steam library.

regola-epic-nome = Epic installations
regola-epic-riga = { $n ->
    [one] { $nome }, installed by Epic Games Launcher
   *[other] { $nome } and other products in the same folder ({ $n_testo } in all), installed by Epic Games Launcher
}
regola-epic-metodo = Epic Games Launcher library › the three dots next to the title › Uninstall

regola-android-avd-nome = Android virtual devices
regola-android-avd-riga = Disks of Android Studio virtual devices: delete them from Device Manager.
regola-android-avd-metodo = Android Studio › Device Manager › device menu › Delete
regola-android-avd-rischio-1 = The apps and data inside the virtual device are lost.

regola-driverstore-nome = Driver store
regola-driverstore-riga = Packages of the installed drivers: Microsoft says not to remove or change them.

regola-paging-nome = Paging and swap files
regola-paging-riga = Windows virtual memory: let Windows manage its size. If you shrink it, Windows can stop responding when memory runs out.

regola-cache-chrome-nome = Chrome cache
regola-cache-chrome-riga = Chrome cache: clear it from Chrome.
regola-cache-chrome-metodo = In Chrome: menu at the top right › Delete browsing data. Check only "Cached images and files", with the time range "All time".
regola-cache-chrome-rischio-1 = Leave cookies and passwords unchecked: if you delete them, you're signed out of websites.

regola-cache-brave-nome = Brave cache
regola-cache-brave-riga = Brave cache: clear it from Brave.
regola-cache-brave-metodo = In Brave: enter brave://settings/clearBrowserData in the address bar. Check only "Cached images and files".
regola-cache-brave-rischio-1 = Leave cookies and passwords unchecked: if you delete them, you're signed out of websites.

regola-cache-edge-nome = Edge cache
regola-cache-edge-riga = Edge cache: clear it from Edge.
regola-cache-edge-metodo = In Edge: Settings › Privacy › Clear browsing data. Check only "Cached images and files".
regola-cache-edge-rischio-1 = Leave cookies and passwords unchecked: if you delete them, you're signed out of websites.

regola-progetti-sviluppo-nome = Project dependencies and builds
regola-progetti-sviluppo-riga = { $nome } with { $lockfile } next to it: usually rebuilt with { $comando } (inferred)
regola-progetti-sviluppo-metodo = Quado moves the folder to the Recycle Bin if you choose to, after the preview.
regola-progetti-sviluppo-alternativa-1 = For a Rust target folder: run cargo clean in the project folder.
regola-progetti-sviluppo-rischio-1 = It can be rebuilt only if the project still installs or compiles.
regola-progetti-sviluppo-rischio-2 = Close the project's editor and terminals before the preview: if they use the folder, the check of files in use blocks the move.
regola-progetti-sviluppo-prova = Before you empty the Recycle Bin, check that the project still installs or compiles.

regola-programmi-registrati-nome = Installed programs
regola-programmi-registrati-riga = Installed program: { $nome }
regola-programmi-registrati-metodo = Uninstall from Settings › Apps.

regola-pacchetti-msix-nome = Microsoft Store app data
regola-pacchetti-msix-riga = Data of the { $nome } app: Windows deletes it when you uninstall the app.
regola-pacchetti-msix-metodo = Uninstall from Settings › Apps.

regola-cartelle-utente-nome = User folders
regola-cartelle-utente-riga = Your files: you decide what to keep.

regola-cartella-windows-nome = Windows folder
regola-cartella-windows-riga = System files protected by Windows

regola-windows-old-nome = Previous Windows installation
regola-windows-old-riga = Windows from before the last upgrade: Windows deletes it by itself 10 days after the upgrade.
regola-windows-old-metodo = Settings › System › Storage › Temporary files, the previous version of Windows check box
regola-windows-old-rischio-1 = After you delete it, you can't go back to the previous version of Windows.

regola-ipotesi-dal-nome-nome = Assumption from the name
regola-ipotesi-dal-nome-riga = Assumption from the name: { $nome }. No installation entry confirms it.

controllo-restart-manager = Executables, libraries and most recently changed files not in use, according to Windows
controllo-riferimenti = No references in the registry, Start menu or desktop shortcuts, or scheduled tasks
controllo-capacita-cestino = The drive's Recycle Bin can hold the selection
controllo-batteria-critica = The laptop doesn't hibernate when the battery is critical
controllo-lockfile = The project's lock file is next to the folder

reversibilita-cestino = You can restore the items from the Recycle Bin until you or Storage Sense empty it. Open the Recycle Bin, select the items and choose "Restore the selected items".
reversibilita-nessuna-azione = Nothing to restore.
reversibilita-cache = The program downloads or rebuilds the data when it needs it.
reversibilita-ibernazione = You can undo it with powercfg /hibernate /type full after reducing, and with powercfg /h on after turning it off.
reversibilita-svuotamento = Items deleted from the Recycle Bin can't be restored.
reversibilita-windows-old = It can't be undone: the previous version of Windows can't be restored.
reversibilita-procedura-windows = Windows deletes the files without the Recycle Bin: they can't be restored.
reversibilita-disinstallazione = An uninstall can't be undone: to get the program back, install it again.
reversibilita-procedura-app = The app deletes the data without the Recycle Bin: it can't be restored.
errore-per-elemento = Items move one at a time. The summary says which ones are in the Recycle Bin and which stayed in place.
errore-nessuna-azione = Not applicable.
errore-procedura-esterna = Another program runs the steps and Quado can't see them: the next measurement shows what changed.
errore-comando = You run the command: if it stops, read the message in the terminal.
verifica-riscansione = After the action, Quado measures free space again and rereads the items it touched.
verifica-nessuna-azione = Not applicable.
verifica-dopo-procedura = When you choose "I've completed the steps", Quado measures free space again and rereads the folder.

fatto-percorso-documentato = Path given by the rule's source
fatto-ibernazione-attiva = { $a ->
    [1] Hibernation on
   *[other] Hibernation off
}
fatto-avvio-rapido = { $a ->
    [1] Fast startup on
   *[other] Fast startup off
}
fatto-standby-moderno = Modern standby on
fatto-batteria-critica = { $a ->
    [0] Critical battery: do nothing
    [1] Critical battery: sleep
    [2] Critical battery: hibernate
    [3] Critical battery: shut down
   *[other] Critical battery: action code { $a_testo }
}
fatto-alimentazione-non-letta = Power settings not read
fatto-cartelle-di-versione = { $a ->
    [one] { $a_testo } version folder
   *[other] { $a_testo } version folders
}
fatto-versione-in-uso = Version in use: { $a }
fatto-versioni-precedenti = { $a ->
    [one] { $a_testo } previous version
   *[other] { $a_testo } previous versions
}
fatto-non-versione-in-uso = A version other than the one in use ({ $a })
fatto-versioni-tenute = { $a ->
    [one] { $a_testo } version stays: the one in use
   *[other] { $a_testo } versions stay: the one in use and the previous one
}
fatto-cestino-su-disco = On disk: { $a }
fatto-cestino-secondo-windows = According to Windows: { $a } in { $b ->
    [one] one item
   *[other] { $b_testo } items
}
fatto-cestino-secondo-windows-non-letto = According to Windows: count not read
fatto-sensore-memoria-cestino = Storage Sense automatically deletes items that stay in the Recycle Bin for more than { $a_testo } days.
fatto-cestino-capacita = Recycle Bin capacity: { $a }
fatto-cartelle-orfane = { $a ->
    [one] { $a_testo } folder without an index file, { $b }
   *[other] { $a_testo } folders without an index file, { $b }
}
fatto-gioco-steam = { $a } (Steam, app ID { $b })
fatto-dimensione-dichiarata = Declared size: { $a }
fatto-ultima-sessione-steam = Last session recorded by Steam: { $a }
fatto-uso-non-registrato = No session recorded by Steam
fatto-installazione-epic = { $a } (Epic)
fatto-epic-nessun-dato-uso = Epic doesn't record when a game was last used.
fatto-programma-installato = Installed: { $a } { $b }
fatto-programma-indizio = Folder of the uninstaller of { $a } { $b }
fatto-ipotesi-dal-nome = Same name as the publisher { $a }
fatto-pacchetto-msix = Package { $a }
fatto-percorso-cache-npm = { $b ->
    [ambiente] npm cache set by the environment variable: { $a }
    [npmrc] npm cache set by .npmrc: { $a }
   *[other] npm cache in the default path: { $a }
}
fatto-percorso-store-pnpm = { $b ->
    [ambiente] pnpm store set by the environment variable: { $a }
    [npmrc] pnpm store set by .npmrc: { $a }
   *[other] pnpm store in the default path: { $a }
}
fatto-cartella-ricostruibile = { $a } next to { $b }
fatto-lockfile-presente = { $a } present next to the folder
fatto-cartella-nota = { $a ->
    [documenti] Windows Documents folder
    [download] Windows Downloads folder
    [desktop] Windows Desktop folder
    [immagini] Windows Pictures folder
    [video] Windows Videos folder
    [musica] Windows Music folder
   *[other] Windows known folder
}
fatto-cartella-bozze = Drafts folder according to CapCut: { $a }

## M4: installers.

funzione-installatore-nome = Installers

fatto-installatore-prodotto = Product { $a }, version { $b }
fatto-installatore-prodotto-senza-versione = Product { $a }, version not stated
fatto-installatore-versione-senza-nome = Version { $a }, product name not stated
fatto-installatore-editore = Publisher: { $a }
fatto-installatore-codice-prodotto = Product code { $a }
fatto-installatore-versione-file = File version { $a }
fatto-installatore-senza-dati = { $a ->
    [msi] The package states no name, version or product code
   *[other] The file contains no version data
}
fatto-installatore-stesso-codice = Same product code as { $a } { $b }, installed
fatto-installatore-stesso-nome = Name and version match { $a } { $b }, installed
fatto-installatore-codice-versione-diversa = Same product code as { $a }, installed at version { $b }; this installer has version { $c }
fatto-installatore-nome-versione-diversa = Same name as { $a }, installed at version { $b }; this installer has version { $c }
fatto-installatore-codice-versione-non-nota = Same product code as { $a }, installed; the versions can't be compared because one of the two isn't stated
fatto-installatore-nome-versione-non-nota = Same name as { $a }, installed; the versions can't be compared because one of the two isn't stated
fatto-installatore-non-installato = { $a ->
    [codice] No installed program with this product code or this name
   *[other] No installed program with this name (name comparison)
}
fatto-installatore-non-confrontabile = { $a ->
    [senza-dati] No name or product code: no comparison with installed programs
   *[other] Comparison with installed programs not available
}

## M4: programs, known usage and possible leftovers.

funzione-programma = Installed programs
funzione-programma-descrizione = Quado function. The list comes from uninstall entries, Microsoft Store packages and the Steam and Epic manifests. A program's space counts only when a source declares its folder.
funzione-programma-metodo-impostazioni = Settings › Apps. Quado copies the program name to the clipboard: paste it in the search box.
funzione-programma-metodo-msix = Settings › Apps, on the app's page
funzione-programma-metodo-steam = Uninstall with Steam, which asks for confirmation.
funzione-programma-metodo-epic = Epic Games Launcher library › the three dots next to the title › Uninstall
funzione-programma-alternativa-pannello = Control Panel › Programs and Features
funzione-programma-rischio-1 = The uninstaller decides what to delete: it can leave data behind or also delete files in the program's folders.
funzione-programma-controllo-tuoi-file = I copied my files from the program's folder somewhere else
funzione-programma-ipotesi-indizio = Assumption: the uninstaller's folder is the program's folder.
funzione-programma-ipotesi-ambito = Assumption: the folder that holds the program and its data belongs to the program, and uninstalling can remove it.
funzione-possibile-residuo = Possible leftovers
funzione-possibile-residuo-descrizione = Quado function. It lists folders in Local, Roaming, LocalLow and ProgramData that no installed program and no rule explain. The comparison is by name. A folder is explained when every word of its name appears in the name or publisher of the same program.

fatto-programma-editore = Publisher: { $a }
fatto-programma-versione = Version: { $a }
fatto-programma-cartella = Program folder: { $a }
fatto-programma-cartella-indizio = Folder of the uninstaller, taken as the program folder: { $a }
fatto-programma-cartella-non-plausibile = The declared folder { $a } is the root of the drive or a Windows or user folder: no space is attributed to it.
fatto-programma-posizione-non-dichiarata = Location not declared
fatto-programma-nessuno-spazio = No space attributed: no source declares the program's folder.
fatto-programma-su-disco = Size on disk: { $a }
fatto-programma-non-misurato = Size on disk not measured: the folder isn't in the scan or can't be read.
fatto-programma-solo-dati = The program folder isn't in the scan or can't be read: the size covers only the data recognized by the rules.
fatto-programma-cartella-condivisa = { $a ->
    [one] Another program also declares this folder: the space counts only once.
   *[other] { $a_testo } other programs also declare this folder: the space counts only once.
}
fatto-programma-dati-regole = { $b ->
    [one] Program data recognized by a rule: { $a }
   *[other] Program data recognized by the rules: { $a } in { $b_testo } folders
}
fatto-programma-tuoi-file = The { $a } folder holds your files ({ $b } in { $c }). Copy them elsewhere before uninstalling.
fatto-programma-data-installazione = Install date: { $a }
fatto-programma-componente-di-sistema = System component: Settings › Apps doesn't list it.
fatto-uso-userassist = Last launch from File Explorer or Start: { $a } (UserAssist). Launches from the command line, other launchers and scheduled tasks aren't recorded.
fatto-uso-userassist-nessun-avvio = No launches from File Explorer or Start for this folder, according to UserAssist. Launches from the command line, other launchers and scheduled tasks aren't recorded.
fatto-uso-senza-cartella = No usage data: without the program folder, the launches recorded by UserAssist can't be matched.
fatto-uso-userassist-non-letto = UserAssist not read: no usage data.
fatto-uso-tracciamento-disattivato = Windows launch tracking is off: no usage data
fatto-uso-tracciamento-non-letto = Launch tracking setting not read: no usage data.
fatto-uso-msix-nessun-dato = Windows doesn't say when a Microsoft Store app was last used.
fatto-residuo-possibile = No matching installed program (compared by name). Last modified: { $a }. It might be a leftover.
fatto-residuo-possibile-senza-data = No matching installed program (compared by name). Last modified date not read. It might be a leftover.

## M4: Downloads view.

funzione-stessa-dimensione-nome = Files of the same size
funzione-copie-identiche-nome = Identical copies
funzione-copie-identiche-metodo = Quado moves the copies you choose to the Recycle Bin, after the preview. The copy to keep stays where it is.
funzione-copie-identiche-rischio-1 = A shortcut or a program that opens a copy from its path won't find it anymore. The same content stays in the kept copy.
funzione-archivio-con-cartella-nome = Archives with a folder of the same name
funzione-immagine-disco-nome = Disk images
funzione-download-incompleto-nome = Incomplete downloads
funzione-download-incompleto-metodo = Quado moves the incomplete downloads to the Recycle Bin, after the preview.
funzione-download-incompleto-rischio-1 = The browser can't resume these downloads anymore: to get the file, download it again.

fatto-lente-famiglia = { $a ->
    [installatori] { $b ->
        [one] One installer
       *[other] { $b_testo } installers
    }
    [immagini-disco] { $b ->
        [one] One disk image
       *[other] { $b_testo } disk images
    }
    [download-incompleti] { $b ->
        [one] One incomplete download
       *[other] { $b_testo } incomplete downloads
    }
   *[other] { $b ->
        [one] One file
       *[other] { $b_testo } files
    }
}, by file extension
fatto-lente-stessa-dimensione = { $a ->
    [one] One file of { $b }
   *[other] { $a_testo } files of { $b } each
}
fatto-lente-contenuto-non-confrontato = Content not compared
fatto-lente-letti-in-parte = Comparison not completed: { $a_testo } of { $b_testo } files read
fatto-lente-dimensione-cambiata = { $a ->
    [one] One file changed size after the scan: comparing it needs a new scan of the folder
   *[other] { $a_testo } files changed size after the scan: comparing them needs a new scan of the folder
}
fatto-lente-contenuto-identico = Same content (SHA-256 fingerprint) in { $a_testo } files of { $b }
fatto-lente-copia-tenuta = The copy created on { $a } stays
fatto-lente-contenuto-diverso = { $a ->
    [one] Content compared: no other file of the same size has this content
   *[other] Content compared: the { $a_testo } files all have different content
}
fatto-lente-installatori-non-letti = Version data not read
fatto-lente-solo-cloud = Only in the cloud: not read, so it isn't downloaded
fatto-lente-estrattore-non-riuscito = Can't read the version data: { $a ->
    [non-presente] the file no longer exists
    [accesso-negato] Windows denies access
    [errore] Windows error (Code { $b })
    [solo-cloud] the file is only in the cloud
    [troncato] the file ends halfway
    [inatteso] unexpected format
    [vuoto] the file is empty
    [troppo-annidato] too many nested levels
    [codifica] invalid character encoding
    [troppo-grande] the file exceeds the reading limit
    [campo-mancante] the { $b } field is missing
    [tipo-diverso] the { $b } field has an unexpected type
    [valore] the { $b } field has an unexpected value
   *[other] { $a }
}
fatto-lente-cartella-accanto = Next to it there is a folder with the same name as the archive
fatto-lente-voci-presenti = { $a ->
    [one] { $a_testo } of { $b_testo } archive files is in the folder, with the same size
   *[other] { $a_testo } of { $b_testo } archive files are in the folder, with the same size
}
fatto-lente-voci-dimensione-diversa = { $a ->
    [one] One archive file is in the folder with a different size
   *[other] { $a_testo } archive files are in the folder with a different size
}
fatto-lente-voci-mancanti = { $a ->
    [one] One archive file isn't in the folder
   *[other] { $a_testo } archive files aren't in the folder
}
fatto-lente-voci-non-lette = { $a ->
    [one] One archive file is in a folder Quado couldn't read
   *[other] { $a_testo } archive files are in folders Quado couldn't read
}
fatto-lente-indice-non-letto = Archive index not read
fatto-lente-archivio-non-leggibile = Can't read the archive index: { $a ->
    [non-presente] the file no longer exists
    [accesso-negato] Windows denies access
    [errore] Windows error (Code { $b })
    [solo-cloud] the file is only in the cloud
    [troncato] the file ends halfway
    [inatteso] unexpected format
    [vuoto] the file is empty
    [troppo-annidato] too many nested levels
    [codifica] invalid character encoding
    [troppo-grande] the file exceeds the reading limit
    [campo-mancante] the { $b } field is missing
    [tipo-diverso] the { $b } field has an unexpected type
    [valore] the { $b } field has an unexpected value
   *[other] { $a }
}
fatto-lente-segnaposto = { $a ->
    [one] One file is only in the cloud: on this drive it takes only the local bytes
   *[other] { $a_testo } files are only in the cloud: on this drive they take only the local bytes
}
fatto-lente-nessun-processo = { $a ->
    [one] No program is using it, according to Windows (checked on { $b } at { $b_ora })
   *[other] No program is using them, according to Windows (checked on { $b } at { $b_ora })
}
fatto-lente-in-uso = In use by a program, according to Windows (checked on { $a } at { $a_ora })
fatto-lente-processo = Used by { $a }
fatto-lente-uso-non-letto = Use not known: the check on { $b } at { $b_ora } failed (Code { $a })
fatto-lente-uso-non-controllato = Use not checked
