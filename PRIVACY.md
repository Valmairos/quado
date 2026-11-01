# Privacy

[Italiano](PRIVACY.it.md)

Quado does not use the network. It sends no data to the maintainer or to anyone else, and it has no telemetry, update check or account. It does not open web pages: through Windows, it opens only Settings, File Explorer and the Steam dialog that uninstalls a game.

On the PC where it runs, Quado reads the names, sizes, dates and attributes of the files on the scanned drive. It also reads part of the registry, the files in which Steam, Epic Games Launcher and CapCut list games, versions and folders, and the `.npmrc` file in the user profile. Before a move to the Recycle Bin, it checks whether the items are in use or in a synced folder, and for some items it also reads the shortcuts and scheduled tasks that could point to them. It reads the content of other files only on request, in the Downloads view. The complete list is in the README section [Data access and file changes](README.md#data-access-and-file-changes).

Quado saves its settings, the log of confirmed actions and any added language files in `%APPDATA%\Quado`. Settings › Data and privacy lists these files and can delete the action log. Details are in the README section [Privacy](README.md#privacy).

Issues and reports opened on GitHub are handled by GitHub under its privacy statement. Leave out names and paths that could identify you.
