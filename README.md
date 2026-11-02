<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/hero-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/hero-light.png">
  <img src="docs/images/hero-light.png" width="880" alt="Quado, disk space analyzer for Windows. Below the name, the top of the Overview window with test data: how the space on the drive is used, split by who manages it.">
</picture>

<p align="center">
  <a href="https://github.com/Valmairos/quado/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Valmairos/quado?style=flat-square&labelColor=1D1D1F&color=3A3A3C&label=release"></a>
  <a href="https://github.com/Valmairos/quado/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/Valmairos/quado/ci.yml?branch=main&label=CI&style=flat-square&labelColor=1D1D1F"></a>
  <a href="#requirements"><img alt="Platform: Windows x64" src="https://img.shields.io/badge/platform-Windows%20x64-3A3A3C?style=flat-square&labelColor=1D1D1F"></a>
  <a href="LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-3A3A3C?style=flat-square&labelColor=1D1D1F"></a>
</p>

<p align="center"><b>English</b> · <a href="README.it.md">Italiano</a></p>

<p align="center">Quado is a disk space analyzer for Windows. It shows which folders use the most space on a drive and, where it can, what those folders are.</p>

<p align="center"><a href="#features">Features</a> · <a href="#requirements">Requirements</a> · <a href="#download">Download</a> · <a href="#privacy">Privacy</a> · <a href="#build-from-source">Build from source</a> · <a href="#contributing">Contributing</a></p>

## Features

- Scans NTFS, exFAT and FAT32 drives without changing any file.
- Shows folder sizes as a list and as a map, with views for large files, Downloads, installed programs, and Windows and system.
- Recognizes folders created by common programs and by Windows, with rules that cite their sources. Folders that no rule matches are marked "We don't know".
- Moves selected folders to the Recycle Bin after a preview that checks for files in use, synced folders and the capacity of the Recycle Bin.
- For other folders, points to the Windows setting or the program that manages them, or copies the official command.
- On request, reads as administrator the folders that the normal scan cannot open.
- Contains no network code.

## Screenshots

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/overview-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/overview-light.png">
  <img src="docs/images/overview-light.png" width="880" alt="Overview: how the space on the drive is used, and the largest items">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/explore-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="docs/images/explore-light.png">
  <img src="docs/images/explore-light.png" width="880" alt="Explore: map and table of the folders">
</picture>

The screenshots show test data.

## Requirements

The following table separates what has been tested, on one PC, from what should work and what is not supported.

| | Tested | Should work but not tested | Not supported |
|---|---|---|---|
| Windows | Windows 10 22H2 | Windows 10 1809 to 21H2, Windows 11 | Before Windows 10 1809, 32-bit, S mode |
| Other systems | None | None | macOS, Linux |
| Processor | x64 | Arm with Windows 11 | Arm with Windows 10 |
| Graphics | Direct3D 12 | WARP, OpenGL | None |
| File systems | NTFS, exFAT, FAT32 | ReFS, Dev Drive | None |
| Drives | Internal drives, VHDX | USB drives, memory cards, mapped network drives | Network paths without a drive letter |
| Languages | Italian, English | None | Right-to-left text |

- The minimum version is inferred, not tested. `quado.exe` imports `GetDiskSpaceInformationW`, which Windows provides from Windows 10, version 1809.
- Without a Direct3D 12 driver, Quado uses WARP, the software renderer of Windows, and the window responds slowly. If Direct3D 12 does not start, Quado uses OpenGL.
- On file systems other than NTFS, exFAT and FAT32, and on network drives, Quado marks its measurements "without guarantees".

## Download

Download `quado.exe` from the [Releases](https://github.com/Valmairos/quado/releases) page. Quado is published only on that page; a copy from another site may contain different code. The program is a single executable and runs without installation.

Keep `quado.exe` in a folder of its own, for example `%LOCALAPPDATA%\Programs\Quado`. "Retry as administrator" does not start from Downloads or from a temporary folder. To remove Quado, see [Uninstall](#uninstall).

### Verify the file

Before running `quado.exe`, compare its SHA-256 hash with the one in the release notes. In PowerShell:

```powershell
Get-FileHash .\quado.exe
```

### "Windows protected your PC"

`quado.exe` is not code-signed. The first time a downloaded copy is opened, Microsoft Defender SmartScreen may block it with the message "Windows protected your PC". The message means that SmartScreen does not recognize the file; it does not report malware. To run the file:

1. Select "More info".
2. Check that the app is `quado.exe` and that the publisher is "Unknown publisher".
3. Select "Run anyway".

For the same reason, the Windows prompt of "Retry as administrator" shows the publisher as unknown. To avoid running a downloaded binary, [build Quado from source](#build-from-source).

## Before you use it

Quado is provided without warranty, to the extent permitted by applicable law. The person using Quado decides what to move and which commands to run, so check these points first:

- The rules were checked on one PC. On another PC, a rule can recognize a folder incorrectly or be out of date. Before confirming, read in the preview which rule applies.
- Moved items stay in the Recycle Bin until it is emptied, by hand or by Storage Sense. Emptying it deletes them permanently, together with everything else in the Recycle Bin of that drive. Back up anything that might still be needed first.
- Moving an item from a synced folder can delete it in the cloud and on the other devices. Quado recognizes the folders registered with Windows and those of Dropbox, Google Drive for desktop, Syncthing and Resilio Sync. Check other synced folders by hand.
- Files stored only in the cloud do not reach the Recycle Bin of the PC. They can be recovered from the trash of the service, on the web.
- Before running a command that Quado copies, read the risks that the inspector shows with it.
- On a PC managed by a company or a school, ask the IT department first.

## Privacy

Quado does not use the network. It has no update check, telemetry or account, and it sends no data. It saves its settings, the log of confirmed actions and any added language files in `%APPDATA%\Quado`. [PRIVACY.md](PRIVACY.md) lists what Quado reads, what it changes and saves, and how to check that it contains no network code.

## Languages

Quado is available in Italian and English. At first start, it uses the first Windows display language that it supports, otherwise English; Settings › Language changes it. To correct a text or add a language, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Build from source

Building Quado requires Rust, installed with rustup, and the Microsoft C++ build tools. `rust-toolchain.toml` pins the Rust version, which rustup installs the first time `cargo` runs in the repository.

```powershell
git clone https://github.com/Valmairos/quado.git
cd quado
cargo build --profile dist
cargo test --workspace
```

The executable is `target\dist\quado.exe`. The `dist` profile uses full link-time optimization; for development, use `cargo run --release -p quado`. Tests marked as ignored need the VHDX test drives or a person at the PC. `cargo xtask installa` builds Quado, checks its imports and copies it to `%LOCALAPPDATA%\Programs\Quado`, with a shortcut in the Start menu.

## Uninstall

Quado has no uninstaller. To remove it, delete the following files and folders:

- A downloaded copy: `quado.exe` and the folder `%APPDATA%\Quado`.
- A copy installed with `cargo xtask installa`: the folder `%LOCALAPPDATA%\Programs\Quado`, the shortcut `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Quado.lnk` and the folder `%APPDATA%\Quado`.

## Project status

Quado is at version 0.1.0. It has been built and used on one PC, a laptop with a 6-core processor and an NVMe SSD. On that PC, a full scan of about 3.1 million files takes about 12 seconds when the folder data is in the Windows cache. The main limitations:

- The rules cover a limited set of programs and parts of Windows. On other PCs, many folders appear as "Not classified".
- Quado has no installer, code signature or automatic updates.
- A usability test and a full session with the network disconnected have not been done.

## Use of AI

Quado's code, documents and interface texts were written with Claude Code, the coding agent developed by Anthropic. The maintainer, Valmairos, decided what Quado does and what it must never do, reviewed the results and requested changes. The maintainer also ran the tests that require a person at the PC, such as reading drive C: as administrator.

Quado itself contains no AI. The explanations it displays come from the rule files in `crates/quado-kb/regole`.

## Contributing

To report a bug or propose a rule, use the issue forms on GitHub. Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request; [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) describes the structure of the code. Report security vulnerabilities as described in [SECURITY.md](SECURITY.md).

## License

Quado is free software, distributed under the GNU General Public License, version 3 or (at your option) any later version. It is provided without warranty, to the extent permitted by applicable law; see sections 15 and 16 of [LICENSE](LICENSE). The fonts in `assets/fonts`, Inter and IBM Plex, are licensed under the SIL Open Font License 1.1. The license does not cover the name Quado. If you distribute a modified version, give it a different name.

Copyright (C) 2026 Valmairos.

## Trademarks

Windows, Steam and the other product names that appear in Quado and its rules are trademarks of their respective owners. Quado is not affiliated with or endorsed by them, and uses these names only to refer to their products.
