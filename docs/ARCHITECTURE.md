# Architecture

Quado is a Cargo workspace with seven crates and a development tool. Code comments and the names of the development commands are in Italian. This page gives the overall structure; each module starts with a comment that explains its part.

## Crates

The following table lists the crates in the order in which data passes through them.

| Crate | Role | Depends on |
|---|---|---|
| `quado-scan-win` | Lists the volumes, scans a drive or a folder, runs the elevated helper and reads file content when the Downloads view asks for it | `quado-model` |
| `quado-model` | Scan tree, hard links, folder totals, the balance of used space, and the types of recommendations and actions | None |
| `quado-kb` | Rules in `regole/*.toml`, the classification engine and the read-only extractors | `quado-model` |
| `quado-actions` | Preview, confirmed plan, move to the Recycle Bin, Windows procedures, measurements after an action, action log | `quado-model`, `quado-scan-win` |
| `quado-ui` | The egui interface, its views and the Fluent catalogs in `lingue/` | All of the above |
| `quado` | The executable: window, renderer, and the helper mode | `quado-ui` and the core crates |
| `quado-testkit` | Test support only: an independent measurement oracle and generators of test trees | None |

`unsafe` code is forbidden in the workspace. Only `quado-scan-win`, `quado-actions`, `quado-testkit` and the Windows modules of `quado-kb` (`win`, `installers_win`) allow it. Clippy requires a written invariant for each `unsafe` block.

Only `quado-actions` moves files, empties the Recycle Bin or opens other programs. The interface writes only its own files in `%APPDATA%\Quado`: the settings and the template to translate. An architecture test, `crates/quado-actions/tests/processi.rs`, looks for the calls that start processes or change files in the sources of the other crates. Another test, `crates/quado-model/tests/architettura.rs`, checks that the core crates return codes and values: all text the user reads comes from the language files.

## From scan to action

1. Scan. Threads of a rayon pool list the folders and send one batch per folder to a single builder, which adds the nodes to the tree. Folders are opened with `FILE_LIST_DIRECTORY` and files only with `FILE_READ_ATTRIBUTES`. Reparse points are not followed, and cloud files are not downloaded (`FILE_FLAG_OPEN_NO_RECALL`). The scan reads no file content.
2. Model. The tree stores nodes in parallel arrays with `u32` indices and the names in one UTF-16 pool. A file with several names (hard links) counts once: one name, chosen in a fixed order, carries its size. The balance splits the used space reported by Windows into allocated space read, measured metadata and space that remains unaccounted for.
3. Rules. Each rule file states where it applies and how it recognizes a folder. It also lists its sources with a quoted excerpt, the versions it was checked on, the date of the last check and its test fixture. The extractors read uninstall entries, app packages, Steam, Epic, CapCut, UserAssist and configuration files such as `.npmrc`. The engine combines their data with the rules and the tree. The result is a record with one of five states and the source of each fact. The engine computes the reliability of inferred facts, not the rule.
4. Interface. `App::show` draws one frame; eframe calls it in the app, and `egui_kittest` calls it in the tests. Texts come from `crates/quado-ui/lingue/*.ftl` and `crates/quado-kb/regole/*.ftl`. Numbers and dates use the regional format of Windows.
5. Actions. A move starts from a preview (`simulate`), which reads the items again and runs the checks without changing anything. Only `plan::conferma` turns the preview and the choice made in the dialog into a `PianoConfermato`, and `execute::esegui` consumes it, so a plan runs once. Procedures of Windows and of other programs go through a typed launcher: Settings URIs, `steam://uninstall/` with a number, a validated package name, the Recycle Bin. Quado never runs a command or an `UninstallString` read from the registry; it shows the command and copies it.

## Protections before a move

The preview runs these checks on each item, in this order. A failed check blocks the move of that item.

1. Folder contents: the walk gives the estimate and finds paths of 260 characters or longer. The shell does not move those to the Recycle Bin.
2. Volume: it must have a Recycle Bin that does not delete files immediately (`NukeOnDelete`), with a known capacity large enough for the selection.
3. Files in use: Restart Manager reports the programs that have them open.
4. References: shortcuts, registry entries and scheduled tasks that point to the item, for the rules that need this check.
5. Synced folders: those registered with Windows and those of Dropbox, Google Drive for desktop, Syncthing and Resilio Sync. The confirmation then asks for a manual check.
6. Protected items: the Windows folder, System Volume Information, the paging, hibernation and swap files and the folders of installed programs. A rule can make an exception only when all its other checks have passed.

The plan then moves one item at a time. Right before each move, it checks again that the item has the same ID, size and modification date. Each item is a separate `IFileOperation` with `FOFX_RECYCLEONDELETE` and `FOF_ALLOWUNDO`. No product code uses `FOF_NOCONFIRMATION`, `FOF_NO_UI` or `SHERB_NOCONFIRMATION`, so Windows asks before it deletes anything permanently. The sink reports whether the item reached the Recycle Bin. The plan stops at the first item that disappears without reaching it. Emptying the Recycle Bin opens the Windows dialog, and only for the root of one drive.

## Reading as administrator

"Retry as administrator" starts a second copy of `quado.exe` with `--helper <pipe> <pid>` and the `runas` verb of `ShellExecuteExW`. Windows then shows the UAC prompt. This is the only process that Quado creates; Settings, File Explorer and Steam are opened through the shell.

- The interface creates a named pipe with a random 128-bit name, a DACL that allows only the current user, `PIPE_REJECT_REMOTE_CLIENTS` and a single instance.
- Each side checks the process at the other end of the pipe. The helper also checks that the PID it received is its parent.
- The helper refuses to start when `quado.exe` is in Downloads or in a temporary folder, where a DLL saved next to it could be loaded.
- The helper reads one request and validates it with explicit limits before any allocation. It then enables `SeBackupPrivilege` and scans only the folders that the normal scan could not open.
- After the request, the helper only writes replies. It reports the profiles and Recycle Bins of other accounts as totals, without names, adds the space used by shadow copies and exits.

The executable limits the DLL search to System32 (`/DEPENDENTLOADFLAG:0x800` and `SetDefaultDllDirectories`) and links the C runtime statically.

## Window and saved data

Quado draws with wgpu on Direct3D 12, on the graphics card that drives the main monitor. Without a hardware adapter, wgpu uses WARP, the software renderer of Windows. `--renderer warp` forces WARP, and `--renderer gl` forces OpenGL, which Quado also uses when wgpu does not start.

Quado saves three things in `%APPDATA%\Quado`:

- `impostazioni.txt`: the settings, as lines of `key = value`, written only when the user changes something.
- `interventi.jsonl`: the action log. Each line is a JSON object with codes and values instead of sentences, so Quado shows the log in the current language.
- `lingue`: the language files that the user adds, and the template to translate.

## Privacy by design

Quado contains no network code. `deny.toml` blocks HTTP and WebSocket client crates, and `cargo xtask importazioni` reads the import table of `quado.exe`. It fails on any function of winhttp, wininet, urlmon, dnsapi or websocket and on the connect and name lookup functions of ws2_32. It also fails on `CreateProcess`, `WinExec` and `MsiOpenPackage`.

The import table does not show functions loaded by name at run time. `cargo xtask sessione-prova` covers that case: it samples the connections and file handles of Quado during a guided session. The command-line options that measure Quado exist only with the `misure` feature, which release builds do not enable. [PRIVACY.md](../PRIVACY.md) lists what Quado reads and writes.

## Tests and tools

- Unit tests sit next to the code, and integration tests in the `tests` folder of each crate. Run them with `cargo test --workspace`.
- `quado-testkit` has an oracle that measures the same trees by a second path (`std::fs::read_dir` and file attributes), used to check the scanner.
- Tests that need the VHDX test drives, administrator rights or a person at the PC are marked `#[ignore]`. `cargo xtask vhdx crea` creates the NTFS, exFAT and FAT32 test drives.
- Rule fixtures are anonymized trees and files in `crates/quado-kb/fixture`, and the expected records are in `crates/quado-kb/tests/attese`.
- `xtask` also has commands for the installed build (`installa`), the language files (`lingue`), the DLL checks (`verifica-dll`), the app icon (`icona`) and performance measurements.
