Quado {VERSION}
Disk space analyzer for Windows, 64-bit.
Analizzatore dello spazio su disco per Windows a 64 bit.

https://github.com/{REPOSITORY}


ENGLISH

quado.exe runs without installation. You don't need to run it as
administrator. Keep it in a folder of its own, for example
%LOCALAPPDATA%\Programs\Quado: "Retry as administrator" doesn't start
from Downloads or from a temporary folder.

quado.exe isn't signed. The first time you open it, SmartScreen may show
"Windows protected your PC". Select "More info", check that the app is
quado.exe, then select "Run anyway". Before that, you can compare the
SHA-256 of quado.exe with the one in the release notes:

    Get-FileHash .\quado.exe

Quado doesn't use the network. It saves its settings, the log of the
actions you confirm and the language files you add in %APPDATA%\Quado.
To remove Quado, delete quado.exe and that folder.

Quado is free software under the GNU General Public License, version 3
or later (LICENSE.txt). It comes with no warranty, to the extent the law
allows: see sections 15 and 16 of LICENSE.txt. The fonts built into
quado.exe, Inter and IBM Plex, are under the SIL Open Font License 1.1:
see the licenses folder. The licenses of the Rust crates built into
quado.exe are in THIRD-PARTY-LICENSES.html.


ITALIANO

quado.exe funziona senza installazione. Non serve avviarlo come
amministratore. Tienilo in una cartella sua, per esempio
%LOCALAPPDATA%\Programs\Quado: «Ripeti come amministratore» non parte da
Download né da una cartella temporanea.

quado.exe non è firmato. La prima volta che lo apri, SmartScreen può
fermarlo con l'avviso «Windows ha protetto il tuo PC». Seleziona il
collegamento che mostra i dettagli dell'app, controlla che l'app sia
quado.exe e seleziona «Esegui comunque». Prima puoi confrontare lo
SHA-256 di quado.exe con quello scritto nelle note di rilascio:

    Get-FileHash .\quado.exe

Quado non usa la rete. Salva in %APPDATA%\Quado le impostazioni, il
registro degli interventi che confermi e i file di lingua che aggiungi.
Per togliere Quado cancella quado.exe e quella cartella.

Quado è software libero, con licenza GNU General Public License,
versione 3 o successiva (LICENSE.txt). È distribuito senza garanzia, nei
limiti consentiti dalla legge: vedi le sezioni 15 e 16 di LICENSE.txt.
I font incorporati in quado.exe, Inter e IBM Plex, sono distribuiti con
la SIL Open Font License 1.1: vedi la cartella licenses. Le licenze dei
crate Rust compilati in quado.exe sono in THIRD-PARTY-LICENSES.html.

Copyright (C) 2026 Valmairos.
