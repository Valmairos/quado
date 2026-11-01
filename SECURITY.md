# Security

## Reporting a vulnerability

Report vulnerabilities privately on GitHub: open the Security tab of this repository and select "Report a vulnerability". Do not open a public issue.

Include the Quado version (Settings › About), the Windows version, the steps that reproduce the problem and what an attacker gains by exploiting it. Leave out paths and file names that could identify you or your PC.

## Scope

Quado reads a whole drive, part of the registry and the configuration files of a few programs. It can start a second copy of itself as administrator to read folders that Windows denies. It moves files to the Recycle Bin and opens the Windows dialog that empties it. A report is a security issue when Quado does one of the following:

- Quado moves or deletes an item without confirmation.
- Quado moves an item that it should refuse: the Windows folder, System Volume Information, the paging, hibernation and swap files, or the folder of an installed program other than an old CapCut version.
- An item is permanently deleted outside the Windows dialog that empties the Recycle Bin, for example because Windows deleted it instead of moving it.
- A `quado.exe` built without the `misure` feature, such as the one on the Releases page, writes files outside `%APPDATA%\Quado` and the Recycle Bin. The shader caches of Direct3D and the graphics driver, described in the README, are excluded.
- Quado opens a network connection, or `quado.exe` imports a network function that `cargo xtask importazioni` does not detect.
- Another process communicates with the copy that reads as administrator, or makes it read more than the denied folders.
- That copy discloses file names from the profiles and Recycle Bins of other accounts, which it should report only as totals.
- `quado.exe` loads a DLL placed next to it.
- A crafted file name, registry value, zip archive, installer, shortcut or configuration file (of Steam, Epic, CapCut, npm or Dropbox) makes Quado read outside the data it opened, or run code. A crash on such input is also in scope.

A wrong size, estimate or explanation is an ordinary bug for a public issue, unless it makes Quado move an item that it should refuse. The SmartScreen warning, and the Windows prompt that shows the publisher as unknown, are expected because `quado.exe` is not signed.

## Known limitation

`%LOCALAPPDATA%\Programs\Quado` is writable by the current account, so a program running under that account could replace `quado.exe`, and the prompt for an unsigned program would look the same. This is risk RS11 in `docs/08-analisi-dei-rischi.md`.

## Response

Quado has a single maintainer and no bug bounty program. Replies are given in the private report.
