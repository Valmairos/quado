# Sorgenti delle grafiche del README

I PNG in `docs/images` escono da questi file:

- `hero.html`: testata, con la tessera, il nome, la descrizione e la parte alta della finestra della Panoramica;
- `cornice.html`: finestra di Windows intorno a una schermata dell'app;
- `social.html`: anteprima social del repository, 1280 × 640, con la barra dello spazio di un'unità con numeri
  inventati;
- `comune.css`: i token della variante A, copiati da `crates/quado-ui/src/tokens.rs` (chi cambia un token lo
  cambia anche qui);
- `genera.py`: compone le pagine, le fotografa a 2x e salva i PNG.

## Rigenerarle

Serve Python con Pillow e NumPy, e Chrome o Edge. Dalla radice del repository:

```
python -I docs/images/sorgenti/genera.py --schermate CARTELLA
```

`CARTELLA` contiene le schermate con i dati di prova: `panoramica-en-chiaro.png`, `panoramica-en-scuro.png`,
`esplora-en-chiaro.png`, `esplora-en-scuro.png`, `spostamento-en-chiaro.png` e `spostamento-en-scuro.png`.
Non usare quelle di `quado --immagini`: hanno i nomi dei file del PC su cui gira. La testata usa la Panoramica,
quindi senza `--schermate` lo script rifà solo l'anteprima social.

Lo script trova da solo i font in `assets/fonts` e scrive in `docs/images`; `--uscita` sceglie un'altra
cartella e `--solo hero` rifà solo i file con quel nome. Apre Chrome senza finestra, con un profilo temporaneo
nuovo, senza estensioni e con un proxy inesistente, così nessuna richiesta esce dal PC.

Prima di salvare controlla che i font siano caricati, che nessun testo esca dal riquadro o ne tocchi un altro e
che i pixel di ogni schermata restino quelli originali. Se un controllo fallisce si ferma e dice quale.

La testata taglia la finestra della Panoramica 496 px sotto l'inizio della schermata, appena sotto la card
dell'uso dell'unità. Se la Panoramica cambia disposizione, controlla che il taglio cada ancora prima degli
elenchi.

L'anteprima social non va nel README: si carica a mano in Settings › General › Social preview.
