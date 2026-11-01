# Privacy

[English](PRIVACY.md)

Quado non usa la rete. Non invia dati al manutentore né ad altri, e non ha telemetria, controllo degli aggiornamenti o account. Non apre pagine web: tramite Windows apre solo Impostazioni, Esplora file e la finestra di Steam che disinstalla un gioco.

Sul PC in cui viene eseguito, Quado legge nomi, dimensioni, date e attributi dei file dell'unità scansionata. Legge anche una parte del registro, i file in cui Steam, Epic Games Launcher e CapCut elencano giochi, versioni e cartelle, e il file `.npmrc` nel profilo utente. Prima di uno spostamento nel Cestino controlla se gli elementi sono in uso o in una cartella sincronizzata, e per alcuni elementi legge anche i collegamenti e le attività pianificate che potrebbero puntarvi. Legge il contenuto degli altri file solo su richiesta, nella vista Download. L'elenco completo è nella sezione [Accesso ai dati e modifiche ai file](README.it.md#accesso-ai-dati-e-modifiche-ai-file) del README.

Quado salva in `%APPDATA%\Quado` le impostazioni, il registro degli interventi confermati e gli eventuali file di lingua aggiunti. Impostazioni › Dati e privacy elenca questi file e può cancellare il registro degli interventi. I dettagli sono nella sezione [Privacy](README.it.md#privacy) del README.

Le issue e le segnalazioni aperte su GitHub sono trattate da GitHub secondo la sua informativa sulla privacy. Omettere nomi e percorsi che possano identificare chi scrive.
