# SPDX-License-Identifier: GPL-3.0-or-later
"""Rigenera le grafiche del README di Quado: testata, schermate incorniciate, anteprima social.

La testata usa la schermata della Panoramica, quindi senza `--schermate` si rifà solo l'anteprima social.

Uso (dalla cartella che contiene questo file, o da qualunque altra):

    python -I genera.py [--fonts DIR] [--schermate DIR] [--uscita DIR] [--chrome EXE] [--solo NOME ...]

- `--fonts`: cartella con Inter-*.ttf e Inter-*-tnum.ttf (assets/fonts del repository). Se manca, la cerca
  risalendo dalle cartelle superiori.
- `--schermate`: cartella con le schermate con i dati di prova, {panoramica,esplora,spostamento}-en-{chiaro,scuro}.png.
  Senza questa opzione salta le cornici: non le cerca da solo, perché `quado --immagini` salva schermate con i
  nomi dei file del PC.
- `--uscita`: dove scrivere i PNG (predefinita: la cartella sopra questa).

Cosa fa:
1. compone ogni modello HTML di questa cartella con i token (comune.css), i font locali (file://) e la
   geometria del marchio (brand.rs e gli SVG di docs/brand);
2. lo apre in Chrome o Edge senza finestra (--headless=new), con un profilo temporaneo nuovo, senza estensioni
   e con un proxy inesistente, così nessuna richiesta esce dal PC; la prima apertura misura l'altezza,
   controlla che i font siano caricati e che nessun testo esca dal riquadro, la seconda fotografa a 2x;
3. salva il PNG ottimizzato: con la tavolozza solo se nessun pixel cambia di più di 3 livelli su 255,
   altrimenti a colori pieni;
4. nelle cornici controlla che i pixel della schermata siano uguali all'originale;
5. se accanto alla cartella di questo file c'è tavola.html (la tavola di controllo, fuori dal repository), rifà
   tavola.png con i PNG appena scritti: tutte le grafiche sui due fondi di GitHub.

Serve Pillow (e NumPy per i controlli); nient'altro da installare.
"""
import argparse
import html
import io
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

QUI = Path(__file__).resolve().parent

# --------------------------------------------------------------------------------------------------
# Geometria del marchio

# brand.rs: colori del segno e bordo interno della tessera
GRAPHITE, LIGHT, RIM = "#1D1D1F", "#F5F5F7", "#4A4A4D"
TILE_EXPONENT = 5.0
# brand::TILES: lato dell'immagine -> (lato della tessera, inizio del segno, metà, taglio)
TILES = {16: (15, 3, 4, 1), 20: (19, 4, 5, 1), 24: (23, 5, 6, 1), 32: (32, 7, 8, 2), 40: (40, 10, 9, 2),
         48: (48, 12, 11, 2), 64: (63, 15, 15, 3), 256: (256, 64, 58, 12)}


def f(v):
    """Numero per SVG senza zeri inutili."""
    s = f"{v:.4f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def rrect(x0, y0, x1, y1, r):
    """Rettangolo con i raggi in senso orario da alto-sinistra (come brand::Part)."""
    if not isinstance(r, (list, tuple)):
        r = (r, r, r, r)
    tl, tr, br, bl = r
    return (f"M{f(x0 + tl)} {f(y0)}H{f(x1 - tr)}A{f(tr)} {f(tr)} 0 0 1 {f(x1)} {f(y0 + tr)}"
            f"V{f(y1 - br)}A{f(br)} {f(br)} 0 0 1 {f(x1 - br)} {f(y1)}"
            f"H{f(x0 + bl)}A{f(bl)} {f(bl)} 0 0 1 {f(x0)} {f(y1 - bl)}"
            f"V{f(y0 + tl)}A{f(tl)} {f(tl)} 0 0 1 {f(x0 + tl)} {f(y0)}Z")


def mark_in(x0, half, cut):
    """brand::mark_in: le tre parti del segno nel quadrato da (x0, x0)."""
    side = 2 * half + cut
    outer, inner = 0.2 * side, 0.075 * side
    x1, a, b = x0 + side, x0 + half, x0 + half + cut
    return [(x0, x0, x1, a, (outer, outer, inner, inner)),
            (x0, b, a, x1, (inner, inner, inner, outer)),
            (b, b, x1, x1, (inner, inner, outer, inner))]


def superellipse(side, steps=360):
    """Contorno della tessera (superellisse di esponente 5) di lato `side`, dall'origine."""
    c = side / 2
    e = 2 / TILE_EXPONENT
    pts = []
    for k in range(steps):
        t = 2 * math.pi * k / steps
        ct, st = math.cos(t), math.sin(t)
        pts.append((c + c * math.copysign(abs(ct) ** e, ct), c + c * math.copysign(abs(st) ** e, st)))
    return "M" + "L".join(f"{x:.3f} {y:.3f}" for x, y in pts) + "Z"


def tessera_px(px):
    """La tessera disegnata per `px` pixel (brand::Tile::for_pixels), in coordinate di pixel."""
    s = float(px)
    side, x0, half, cut = TILES.get(px, (s, s / 4, 0.225 * s, 0.05 * s))
    rim = max(s / 128, 1.0)
    tile = superellipse(side)
    parts = "".join(f'<path d="{rrect(*p)}"/>' for p in mark_in(x0, half, cut))
    cid = f"tessera{px}"
    return (f'<svg viewBox="0 0 {px} {px}" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">'
            f'<clipPath id="{cid}"><path d="{tile}"/></clipPath>'
            f'<path d="{tile}" fill="{GRAPHITE}"/>'
            f'<path d="{tile}" fill="none" stroke="{RIM}" stroke-width="{f(2 * rim)}" clip-path="url(#{cid})"/>'
            f'<g fill="{LIGHT}">{parts}</g></svg>')


# --------------------------------------------------------------------------------------------------
CAPTION = {  # pulsanti della barra del titolo di Windows 10: riduci, ingrandisci, chiudi
    "min": '<svg viewBox="0 0 10 10"><path d="M0 5.5h10"/></svg>',
    "max": '<svg viewBox="0 0 10 10"><rect x=".5" y=".5" width="9" height="9"/></svg>',
    "close": '<svg viewBox="0 0 10 10"><path d="M.5.5l9 9M9.5.5l-9 9"/></svg>',
}

# --------------------------------------------------------------------------------------------------
# Rendering


def trova_su(rel, file):
    for d in [QUI, *QUI.parents]:
        p = d / rel
        if (p / file).is_file():
            return p
    return None


def trova_chrome(arg):
    cand = [arg] if arg else []
    for env in ("PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"):
        base = os.environ.get(env)
        if base:
            cand += [os.path.join(base, "Google", "Chrome", "Application", "chrome.exe"),
                     os.path.join(base, "Microsoft", "Edge", "Application", "msedge.exe")]
    cand += [shutil.which(n) for n in ("chrome", "google-chrome", "chromium", "msedge")]
    for c in cand:
        if c and os.path.isfile(c):
            return c
    sys.exit("Chrome o Edge non trovato: passa il percorso con --chrome.")


def font_css(fonts: Path):
    out = []
    for fam, suff in (("Q Inter", ""), ("Q Inter Tnum", "-tnum")):
        for w, name in ((300, "Light"), (400, "Regular"), (500, "Medium"), (600, "SemiBold")):
            p = fonts / f"Inter-{name}{suff}.ttf"
            if not p.is_file():
                sys.exit(f"Font mancante: {p}")
            out.append(f'@font-face{{font-family:"{fam}";font-weight:{w};font-style:normal;'
                       f'src:url("{p.resolve().as_uri()}") format("truetype")}}')
    return "\n".join(out)


MISURA = """
<script>
(async () => {
  const root = document.documentElement;
  try {
    await Promise.all([...document.fonts].map(ff => ff.load()));
    await document.fonts.ready;
    root.dataset.fonts = [...document.fonts].every(ff => ff.status === 'loaded') ? 'ok' : 'errore';
  } catch (e) { root.dataset.fonts = 'errore'; }
  const box = document.querySelector('.tile').getBoundingClientRect();
  root.dataset.w = Math.round(box.width);
  root.dataset.h = Math.ceil(box.height);
  // Testo tagliato o sovrapposto: ogni testo sta dentro il riquadro meno il suo margine interno, non esce
  // dalla propria scatola e non tocca un altro testo.
  const tile = document.querySelector('.tile');
  const tcs = getComputedStyle(tile);
  // 4 px di tolleranza in orizzontale: le correzioni ottiche (la spalla della Q) spostano la scatola, non il disegno.
  const inner = {left: box.left + parseFloat(tcs.paddingLeft) - 4.5, right: box.right - parseFloat(tcs.paddingRight) + 4.5,
                 // in verticale l'area del carattere sporge dalla riga: basta che resti nel riquadro
                 top: box.top - .5, bottom: box.bottom + .5};
  const bad = [];
  const texts = [];
  for (const el of document.querySelectorAll('.tile *')) {
    const own = [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim());
    if (!own) continue;
    const range = document.createRange();
    range.selectNodeContents(el);
    const rects = [...range.getClientRects()].filter(r => r.width > 0 && r.height > 0);
    for (const r of rects) texts.push([el, r]);
    const out = rects.some(r => r.left < inner.left || r.right > inner.right || r.top < inner.top || r.bottom > inner.bottom);
    const block = getComputedStyle(el).display !== 'inline';
    // In verticale l'interlinea stretta dei titoli fa sporgere le lettere senza tagliarle: conta il riquadro.
    const over = block && el.scrollWidth > el.clientWidth + 1;
    if (out || over) bad.push(el.textContent.trim().slice(0, 40));
  }
  for (let i = 0; i < texts.length; i++) for (let j = i + 1; j < texts.length; j++) {
    const [ea, a] = texts[i], [eb, b] = texts[j];
    if (ea === eb || ea.contains(eb) || eb.contains(ea)) continue;
    const w = Math.min(a.right, b.right) - Math.max(a.left, b.left), h = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
    if (w > .5 && h > 2) bad.push(`${ea.textContent.trim().slice(0, 20)} / ${eb.textContent.trim().slice(0, 20)}`);
  }
  // Glifi e icone: non toccano il testo vicino.
  for (const g of document.querySelectorAll('.tile svg')) {
    const a = g.getBoundingClientRect();
    for (const [el, b] of texts) {
      const w = Math.min(a.right, b.right) - Math.max(a.left, b.left), h = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
      if (w > .5 && h > .5) bad.push(`glifo / ${el.textContent.trim().slice(0, 20)}`);
    }
  }
  root.dataset.clip = bad.join(' | ');
})();
</script>
"""


class Chrome:
    def __init__(self, exe):
        self.exe = exe

    def run(self, url, w, h, scale, extra):
        with tempfile.TemporaryDirectory(prefix="quado-grafiche-profilo-") as prof:
            cmd = [self.exe, "--headless=new", f"--user-data-dir={prof}", "--no-first-run",
                   "--disable-extensions", "--no-default-browser-check", "--disable-background-networking",
                   "--disable-component-update", "--disable-sync", "--disable-default-apps",
                   "--dns-prefetch-disable", "--proxy-server=http://127.0.0.1:9", "--hide-scrollbars",
                   "--allow-file-access-from-files", f"--force-device-scale-factor={scale}",
                   f"--window-size={w},{h}", "--default-background-color=00000000",
                   "--virtual-time-budget=5000", *extra, url]
            r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
            return r

    def measure(self, url, w, scale):
        r = self.run(url, w, 3000, scale, ["--dump-dom"])
        m = {k: re.search(rf'data-{k}="([^"]*)"', r.stdout) for k in ("w", "h", "fonts", "clip")}
        if not m["h"]:
            sys.exit(f"Misura non riuscita per {url}\n{r.stderr[-2000:]}")
        vals = {k: html.unescape(v.group(1)) if v else "" for k, v in m.items()}
        if vals["fonts"] != "ok":
            sys.exit(f"Font non caricati in {url}")
        if vals["clip"]:
            sys.exit(f"Testo tagliato in {url}: {vals['clip']}")
        return int(vals["w"]), int(vals["h"])

    def shot(self, url, out, w, h, scale):
        r = self.run(url, w, h, scale, [f"--screenshot={out}"])
        if not Path(out).is_file():
            sys.exit(f"Fotografia non riuscita per {url}\n{r.stderr[-2000:]}")


def salva_png(img: Image.Image, dest: Path):
    """PNG ottimizzato; tavolozza solo se l'errore massimo resta entro 3 livelli su 255."""
    img = img.convert("RGBA")
    a = np.asarray(img).astype(np.int16)
    if a[..., 3].min() == 255:
        img = img.convert("RGB")
        a = a[..., :3]
    q = img.quantize(256, method=Image.Quantize.FASTOCTREE if img.mode == "RGBA" else Image.Quantize.MEDIANCUT,
                     dither=Image.Dither.NONE)
    back = np.asarray(q.convert(img.mode)).astype(np.int16)
    err = int(np.abs(back - a).max())
    buf_q, buf_t = io.BytesIO(), io.BytesIO()
    q.save(buf_q, "PNG", optimize=True)
    img.save(buf_t, "PNG", optimize=True)
    if err <= 3 and len(buf_q.getvalue()) < len(buf_t.getvalue()):
        dest.write_bytes(buf_q.getvalue())
        return "tavolozza", err
    dest.write_bytes(buf_t.getvalue())
    return "colori pieni", err


def componi(nome, sost, build: Path, fonts_css, comune):
    src = (QUI / nome).read_text(encoding="utf-8")
    src = src.replace("/*@FONT*/", fonts_css).replace("/*@COMUNE*/", comune).replace("<!--@MISURA-->", MISURA)
    for k, v in sost.items():
        src = src.replace("{{" + k + "}}", v)
    left = re.findall(r"\{\{[A-Z0-9_]+\}\}", src)
    if left:
        sys.exit(f"{nome}: segnaposto senza valore {left}")
    out = build / nome
    out.write_text(src, encoding="utf-8")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fonts")
    ap.add_argument("--schermate")
    ap.add_argument("--uscita")
    ap.add_argument("--chrome")
    ap.add_argument("--solo", nargs="*")
    args = ap.parse_args()

    fonts = Path(args.fonts) if args.fonts else trova_su(Path("assets") / "fonts", "Inter-Regular.ttf")
    if not fonts:
        sys.exit("Font di Inter non trovati: passa --fonts con la cartella assets/fonts del repository.")
    screens = Path(args.schermate) if args.schermate else None
    uscita = Path(args.uscita) if args.uscita else QUI.parent
    uscita.mkdir(parents=True, exist_ok=True)
    chrome = Chrome(trova_chrome(args.chrome))
    solo = set(args.solo or [])

    sost = {
        "TESSERA16": tessera_px(16),
        "TESSERA_GRANDE": tessera_px(160),
        "CAP_MIN": CAPTION["min"],
        "CAP_MAX": CAPTION["max"],
        "CAP_CLOSE": CAPTION["close"],
    }
    fcss = font_css(fonts)
    comune = (QUI / "comune.css").read_text(encoding="utf-8")
    risultati = []

    with tempfile.TemporaryDirectory(prefix="quado-grafiche-") as tmp:
        build = Path(tmp)

        def fai(nome_png, modello, tema, extra=None, css_w=880, scale=2):
            if solo and not any(s in nome_png for s in solo):
                return
            page = componi(modello, {**sost, **(extra or {})}, build, fcss, comune)
            url = page.resolve().as_uri() + "#" + tema
            w, h = chrome.measure(url, css_w, scale)
            raw = build / ("raw-" + nome_png)
            chrome.shot(url, raw, w, h, scale)
            img = Image.open(raw)
            if img.size != (w * scale, h * scale):
                sys.exit(f"{nome_png}: dimensioni {img.size}, attese {(w * scale, h * scale)}")
            modo, err = salva_png(img, uscita / nome_png)
            risultati.append((nome_png, img.size, (uscita / nome_png).stat().st_size, modo, err))
            return img

        fai("social-preview.png", "social.html", "dark", css_w=640)

        def ritaglia(src_name):
            """La schermata senza il margine trasparente delle catture (l'ombra della finestra), salvata in build."""
            if not (screens / src_name).is_file():
                sys.exit(f"Schermata mancante: {screens / src_name}")
            src = Image.open(screens / src_name).convert("RGBA")
            crop = src.crop(src.getchannel("A").point(lambda v: 255 if v == 255 else 0).getbbox()).convert("RGB")
            cp = build / ("schermata-" + src_name)
            crop.save(cp)
            return crop, cp

        if not screens:
            print("Senza --schermate: salto la testata e le cornici.")
        else:
            # Testata: la finestra della Panoramica esce dal bordo inferiore (hero.html). La parte visibile, da 88 e
            # 672 px fino al bordo, deve restare identica alla schermata.
            for tema, src_name in (("light", "panoramica-en-chiaro.png"), ("dark", "panoramica-en-scuro.png")):
                if solo and not any(x in f"hero-{tema}.png" for x in solo):
                    continue
                crop, cp = ritaglia(src_name)
                img = fai(f"hero-{tema}.png", "hero.html", tema,
                          {"IMG": cp.resolve().as_uri(), "IMG_W": f(crop.width / 2), "IMG_H": f(crop.height / 2),
                           "WIN_THEME": tema})
                a = np.asarray(img.convert("RGB")).astype(np.int16)
                vis = a.shape[0] - 672
                d = int(np.abs(a[672:, 88:88 + crop.width] - np.asarray(crop)[:vis].astype(np.int16)).max())
                if d:
                    sys.exit(f"hero-{tema}.png: la schermata non è identica all'originale (differenza {d})")

            # Solo queste sei schermate, con i dati di prova: le altre possono avere i nomi dei file del PC.
            cornici = (("overview-light.png", "panoramica-en-chiaro.png", "light"),
                       ("overview-dark.png", "panoramica-en-scuro.png", "dark"),
                       ("explore-light.png", "esplora-en-chiaro.png", "light"),
                       ("explore-dark.png", "esplora-en-scuro.png", "dark"),
                       ("move-preview-light.png", "spostamento-en-chiaro.png", "light"),
                       ("move-preview-dark.png", "spostamento-en-scuro.png", "dark"))
            for out_name, src_name, tema in cornici:
                if solo and not any(s in out_name for s in solo):
                    continue
                if not (screens / src_name).is_file() and src_name == "esplora-en-chiaro.png":
                    # Senza la schermata chiara di Esplora: la scura su fondo chiaro.
                    print(f"{src_name} manca: {out_name} usa esplora-en-scuro.png su fondo chiaro")
                    src_name = "esplora-en-scuro.png"
                crop, cp = ritaglia(src_name)
                cw, ch = crop.size
                # La schermata sta a 1 px per 1 px dell'immagine, centrata in un riquadro largo 1760 px.
                pad = (1760 - cw) // 2
                if pad < 48:
                    sys.exit(f"{src_name}: larga {cw} px, troppo per il riquadro di 1760 px")
                win_theme = "dark" if "scuro" in src_name else "light"
                extra = {"IMG": cp.resolve().as_uri(), "IMG_W": f(cw / 2), "IMG_H": f(ch / 2),
                         "PAD": f(pad / 2), "WIN_THEME": win_theme}
                img = fai(out_name, "cornice.html", tema, extra)
                # Controllo: i pixel della schermata sono quelli originali, tranne i due angoli arrotondati in
                # basso (raggio 10 px). Posizione: margine + barra del titolo (32 px).
                a = np.asarray(img.convert("RGB")).astype(np.int16)
                b = np.asarray(crop).astype(np.int16)
                ox, oy = pad, pad + 32
                diff = np.abs(a[oy:oy + ch, ox:ox + cw] - b).max(axis=2)
                diff[ch - 12:, :12] = 0
                diff[ch - 12:, cw - 12:] = 0
                if diff.max():
                    sys.exit(f"{out_name}: la schermata non è identica all'originale (differenza {diff.max()})")

        # Tavola di controllo: sta accanto alla cartella di questo file solo fuori dal repository. Mostra i PNG
        # appena scritti in `uscita`, ovunque sia.
        tav = QUI.parent / "tavola.html"
        if (not solo or "tavola" in solo) and tav.is_file():
            base = f'<base href="{uscita.resolve().as_uri()}/">'
            t = tav.read_text(encoding="utf-8").replace("<head>", "<head>\n" + base, 1)
            page = build / "tavola.html"
            page.write_text(t, encoding="utf-8")
            url = page.resolve().as_uri()
            r = chrome.run(url, 1960, 3000, 1, ["--dump-dom"])
            h = int(re.search(r'data-h="(\d+)"', r.stdout).group(1))
            dest = QUI.parent / "tavola.png"
            chrome.shot(url, dest, 1960, h, 1)
            img = Image.open(dest)
            modo, err = salva_png(img, dest)
            risultati.append(("tavola.png", img.size, dest.stat().st_size, modo, err))

    for nome, size, peso, modo, err in risultati:
        print(f"{nome:28} {size[0]:>5} x {size[1]:<5} {peso / 1024:7.1f} KB  {modo} (errore max {err})")


if __name__ == "__main__":
    main()
