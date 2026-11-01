# Brand · Marchio

[English](#english) · [Italiano](#italiano)

## English

Quado's mark is called «Metà» (half): a square divided into one half and two quarters.

| File | What it is |
|---|---|
| `segno-meta.svg` | The mark, graphite, on the 24-unit grid |
| `segno-meta-chiaro-su-scuro.svg` | The mark, light, for dark backgrounds |
| `segno-meta-16px.svg` | The drawing for 16 and 32 px |
| `icona-meta-grafite.svg` | The app icon: graphite tile, light mark, inner rim |
| `icona-meta-chiara.svg` | Light tile, graphite mark |
| `negativo-meta.svg` | The tile with the mark cut out, one color |
| `griglia-meta.svg`, `griglia-meta-16px.svg` | Construction grids |

Grid: 24 units. The mark fills the area from 2 to 22, the cuts are 2 units wide, the outer corners have radius 4 and the corners along the cuts radius 1.5. At 16 and 32 px the mark uses its own drawing on a 16 grid (area from 1 to 14, cuts of exactly one pixel, radii 2.5 and 1): the 24 drawing scaled to 16 px would give cuts of 1.33 px, which render grey.

Tile: a superellipse with exponent 5. The mark takes half of its side; below 32 px a little more, to stay readable.

Colors, one per mark:

- graphite `#1D1D1F`, on light backgrounds;
- light `#F5F5F7`, on dark backgrounds;
- inner rim of the tile `#4A4A4D`, which keeps the icon's outline visible on dark backgrounds.

Mark with the name: mark side 1.1 times the font size, gap 0.32 times, "Quado" in Inter SemiBold with letter spacing −2%.

The geometry lives in the code, in `quado_ui::brand` (`crates/quado-ui/src/brand.rs`); these SVGs draw the same geometry. The app icon, `crates/quado/icona/quado.ico`, is generated from `quado_ui::brand` with `cargo xtask icona`, with its own drawing at each size from 16 to 256 px. `cargo xtask icona --anteprima DIR` also writes the PNG of each size and a preview sheet.

## Italiano

Il segno di Quado si chiama «Metà»: un riquadro diviso in una metà e due quarti.

| File | Cos'è |
|---|---|
| `segno-meta.svg` | Il segno, grafite, sulla griglia di 24 unità |
| `segno-meta-chiaro-su-scuro.svg` | Il segno, chiaro, per i fondi scuri |
| `segno-meta-16px.svg` | Il disegno per 16 e 32 px |
| `icona-meta-grafite.svg` | L'icona dell'app: tessera grafite, segno chiaro, bordo interno |
| `icona-meta-chiara.svg` | Tessera chiara, segno grafite |
| `negativo-meta.svg` | La tessera con il segno ritagliato, un solo colore |
| `griglia-meta.svg`, `griglia-meta-16px.svg` | Griglie di costruzione |

Griglia: 24 unità. Il segno occupa l'area da 2 a 22, i tagli sono larghi 2 unità, gli angoli esterni hanno raggio 4 e quelli lungo i tagli 1,5. A 16 e 32 px il segno ha un disegno suo su una griglia di 16 (area da 1 a 14, tagli di un pixel esatto, raggi 2,5 e 1): il disegno a 24 ridotto a 16 px darebbe tagli di 1,33 px, cioè grigi.

Tessera: una superellisse di esponente 5. Il segno ne occupa metà del lato; sotto i 32 px un po' di più, per restare leggibile.

Colori, uno per segno:

- grafite `#1D1D1F`, sui fondi chiari;
- chiaro `#F5F5F7`, sui fondi scuri;
- bordo interno della tessera `#4A4A4D`, che tiene visibile la sagoma dell'icona sui fondi scuri.

Segno con il nome: lato del segno 1,1 volte il corpo del testo, distanza 0,32 volte, «Quado» in Inter SemiBold con le lettere avvicinate del 2%.

La geometria sta nel codice, in `quado_ui::brand` (`crates/quado-ui/src/brand.rs`); questi SVG disegnano la stessa geometria. L'icona dell'app, `crates/quado/icona/quado.ico`, si genera da `quado_ui::brand` con `cargo xtask icona`, con un disegno suo per ogni misura da 16 a 256 px. `cargo xtask icona --anteprima DIR` scrive anche il PNG di ogni misura e una tavola di controllo.
