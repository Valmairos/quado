# Release notes template

The release workflow puts the first block in a draft release and fills in the version, the SHA-256 of quado.exe and the commit. Before you publish the draft, replace the other parts in angle brackets and drop a section when there's nothing to put in it. Below the template, the notes for 0.1.0 show it filled in.

````markdown
## Quado <version>

<One or two sentences on what changes for someone who already uses Quado.>

### Download

`quado.exe`, 64-bit Windows, tested on <Windows versions>.

SHA-256: `<hash of quado.exe>`

The zip contains the same quado.exe with the licenses and a README. SHA256SUMS.txt lists the hashes of the zip, quado.exe and THIRD-PARTY-LICENSES.html.

The file isn't signed, so Windows may show "Windows protected your PC" the first time you open it. The [README](https://github.com/Valmairos/quado#windows-protected-your-pc) explains how to check the file and run it.

Quado comes with no warranty (GPL-3.0-or-later). The README section [Before you use it](https://github.com/Valmairos/quado#before-you-use-it) lists what to check before you confirm an action.

### Changes

- <What you notice, with the names used in the interface>

### Fixes

- <What was wrong, and what Quado does now>

### Rules

- <New rule, or rule verified again, with the date of the check>

### Known limits

- <Only limits that are new or still apply>

### Checks on this build

- Built from commit `<commit>` with the `dist` profile
- `cargo test --workspace`: <N> passed, 0 failed, <M> ignored
- `cargo xtask importazioni`: no network functions and no `CreateProcess`
````

## Example: 0.1.0

````markdown
## Quado 0.1.0

First public release. Until now Quado has been built and used on one PC, with Windows 10 22H2.

### Download

`quado.exe`, 64-bit Windows, tested on Windows 10 22H2. Windows 11 hasn't been tried.

SHA-256: `<filled in by the release workflow>`

The zip contains the same quado.exe with the licenses and a README. SHA256SUMS.txt lists the hashes of the zip, quado.exe and THIRD-PARTY-LICENSES.html.

The file isn't signed, so Windows may show "Windows protected your PC" the first time you open it. The [README](https://github.com/Valmairos/quado#windows-protected-your-pc) explains how to check the file and run it.

Quado comes with no warranty (GPL-3.0-or-later). The README section [Before you use it](https://github.com/Valmairos/quado#before-you-use-it) lists what to check before you confirm an action.

### What's in it

- A read-only scan of a drive; you don't need to run Quado as administrator. "Retry as administrator" reads only the folders that Windows denied and the space used by restore points and shadow copies, after you confirm.
- In the Overview, the card "How the space is used", whose parts add up to the used space Windows reports: files read, NTFS metadata, space reserved by Windows and what remains unaccounted for.
- In the Overview, the card "Possible actions": for each entry, the space it uses and, on a separate line, when that space is freed.
- The views Explore (map and table of folders), Large files, Downloads, Programs, Windows and system, Rules (each rule with its source and date) and Actions (the history of what you confirmed).
- 26 rules. Two of them lead to the Recycle Bin: old CapCut versions, and `node_modules`, `.next` or `target` folders with the lockfile of their type next to them.
- Moves to the Recycle Bin after a preview with checks. The confirmation shows the rule's assumptions and risks before the list of items. For "To verify", and for "You decide" with more than one item, the check boxes start empty and "Select all" ticks them.
- The confirmation recognizes synced folders: those registered with Windows, such as OneDrive's, and those of Dropbox, Google Drive for desktop (when `DefaultMountPoint` is set), Syncthing and Resilio Sync.
- After the move, Quado says how to restore the items and what to try before you empty the Recycle Bin. For old CapCut versions, "Empty the Recycle Bin…" stays disabled until you tick "I opened CapCut and it works". After you empty the Recycle Bin from Quado, it measures the change in free space and compares it with the estimate.
- Settings › About, with the copyright, the license, the warranty notice, the license text and the address of the source code.
- Italian and English. A `.ftl` file in `%APPDATA%\Quado\lingue` changes the texts without recompiling.

### Known limits

- Move `quado.exe` out of Downloads before you use "Retry as administrator": it doesn't start from Downloads or from a temporary folder.
- Quado doesn't recognize Google Drive folders in mirror mode, or folders synced by other programs such as MEGA.
- Quado looks for shortcuts, registry entries and scheduled tasks that point to an item only before moving old CapCut versions or project folders.
- Totals are tested on NTFS, exFAT and FAT32 only.
- The first scan after a restart hasn't been measured, and a usability test hasn't been done.

### Checks on this build

- Built from commit `<filled in by the release workflow>` with the `dist` profile
- `cargo test --workspace`: `<fill in>` passed, 0 failed, `<fill in>` ignored
- `cargo xtask importazioni`: no network functions and no `CreateProcess`
````
