# Privacy

[Italiano](PRIVACY.it.md)

Quado does not use the network. It sends no data to the maintainer or to anyone else, and it has no telemetry, update check or account. It does not open web pages: through Windows, it opens only Settings, File Explorer and the Steam dialog that uninstalls a game. Everything it reads and saves remains on the PC.

## Data that Quado reads

The scan reads names, sizes, dates and attributes, and does not require Quado to run as administrator. Windows does not allow the current account to read some folders, such as the profiles of other accounts. Their content is counted in the space marked "Unaccounted for". "Retry as administrator" reads only those folders and the space used by restore points and shadow copies, after Windows asks for confirmation. It does not read file contents, and for the profiles and Recycle Bins of other accounts it reports totals without names.

To recognize folders and programs, Quado also reads:

- from the registry, the installed programs and the settings of the Recycle Bin, Storage Sense, restore points and power options;
- if Windows tracks app launches, the programs started from Start or File Explorer, with the number of launches and the date of the last one (UserAssist, in the registry);
- the files in which Steam, Epic Games Launcher and CapCut list installed games and versions, the last-played dates of Steam games and the CapCut projects folder;
- the `.npmrc` file in the user profile, of which Quado uses only the folders set for the npm cache and the pnpm store;
- the app packages installed for the current account, such as Microsoft Store apps.

Before a move to the Recycle Bin, the preview asks Windows whether the files are in use, and by which programs. It also checks whether the item is in a synced folder:

- it asks Windows whether the folder is registered as synced, as OneDrive folders are;
- it reads the Dropbox file `info.json`, of which it uses only the folder paths, and the `DefaultMountPoint` registry value of Google Drive for desktop;
- it looks for the markers of Syncthing (`.stfolder`) and Resilio Sync (`.sync`) in the item and in the folders that contain it, reading only their attributes.

For old CapCut versions and project folders, the preview also looks for references to them. It reads the shortcuts in Start and on the desktop, the scheduled tasks, and the registry entries for installed programs, app paths and startup.

Quado reads the content of other files only on request, in the Downloads view:

- "Compare the content" reads files of the same size in full, to find identical copies.
- "Read version data" reads the version information of installers without running them.
- "Read archive indexes" reads the list of files in a zip archive without extracting it.

As with any program that opens a file, these reads can update the last-access date of the file. Files stored only in the cloud are skipped, so they are not downloaded.

## Changes to files

Quado changes files in one way only: it moves them to the Recycle Bin, after the preview and confirmation. It does not delete files: "Empty the Recycle Bin…" opens the Windows dialog, and Windows deletes the items only if the deletion is confirmed there.

Quado leaves an item in place when the move could lead Windows to delete files permanently:

- the drive has no Recycle Bin;
- the Recycle Bin is set to delete files immediately;
- the Recycle Bin is too small for the selection, or Quado cannot read its capacity;
- the path of the item, or of a file inside it, is 260 characters or longer.

Quado does not move the Windows folder, System Volume Information or the paging, hibernation and swap files. It also leaves the folders of installed programs in place, with one exception: old CapCut versions.

All other changes go through Windows or the program that installed the files. The uninstall buttons open Settings or Steam. When the official method is a command, such as `pnpm store prune`, Quado displays it and copies it to the clipboard without running it.

## Files that Quado saves

Quado saves only the following files, in `%APPDATA%\Quado`:

- `impostazioni.txt`, the settings;
- `interventi.jsonl`, the log of confirmed actions, created with the first action;
- the `lingue` folder, with any added language files and, if created, the template to translate.

Settings › Data and privacy lists these files with their sizes and can delete the action log. The command-line options that measure Quado, and write files elsewhere, exist only in builds with the `misure` feature. The feature is disabled by default, so the executables on the Releases page do not include them.

When Quado opens its window, Direct3D and the graphics driver may write their own caches, such as shader caches in `%LOCALAPPDATA%`. These writes are outside the control of Quado's code.

## Verify the absence of network code

Two checks verify that Quado contains no network code:

- `deny.toml` blocks HTTP and WebSocket client crates such as reqwest, ureq and hyper. Run `cargo deny check` to apply it (requires cargo-deny).
- `cargo xtask importazioni` reads the import table of the executable. It fails if it finds a network library (winhttp, wininet, urlmon, dnsapi, websocket) or the connect and name lookup functions of ws2_32. It also fails on functions that start a process, such as `CreateProcess`.

The second check also works on a downloaded `quado.exe`:

```powershell
cargo xtask importazioni path\to\quado.exe
```

The import table does not show functions that a program looks up by name at run time. The Rust standard library does this for a few optional Windows functions. To cover that case, `cargo xtask sessione-prova` records a guided session and samples the connections of Quado. On the development PC, with the network connected, it counted zero connections in 216 samples.

## Reports on GitHub

Issues and reports opened on GitHub are handled by GitHub under its privacy statement. Leave out names and paths that could identify you.
