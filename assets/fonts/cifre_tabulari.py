"""Inter con le cifre tabulari fisse: Inter-{peso}-tnum.ttf da Inter-{peso}.ttf.

Perché: egui non applica le funzioni OpenType, quindi non attiva `tnum` e le cifre di Inter restano
proporzionali (l'1 è più stretto dello 0). Nelle cifre chiave, nelle colonne e nei contatori che cambiano
durante la scansione i numeri devono stare in colonna e non ballare. Qui la sostituzione `tnum` del GSUB
si "congela" nella tabella `cmap`: i codici U+0030..U+0039 puntano direttamente ai glifi tabulari.

Cosa fa, solo con la libreria standard:
1. legge `cmap` (sottotabelle formato 4 e 12) e la sostituzione semplice (lookup di tipo 1, anche dentro
   un lookup di estensione di tipo 7) della funzione `tnum` del GSUB;
2. ricostruisce `cmap` con le stesse codifiche e le cifre mappate sui glifi tabulari; le altre tabelle
   restano byte per byte uguali (GSUB compreso: `pnum` e `tnum` continuano a funzionare dove servono);
3. ricalcola le checksum delle tabelle e `checkSumAdjustment` di `head`;
4. verifica il risultato: stesse corrispondenze dell'originale tranne le cifre, le dieci cifre con la
   stessa larghezza di avanzamento in `hmtx`, checksum corrette.

Licenza: Inter è sotto SIL OFL 1.1 senza nomi riservati (Inter-LICENSE.txt), quindi i file modificati
mantengono il nome e la stessa licenza. Le impronte di originali e derivati sono in SHA256SUMS.

Uso, dalla radice del repository:  python -I assets/fonts/cifre_tabulari.py
"""
import os
import struct
import sys

PESI = ("Light", "Regular", "Medium", "SemiBold")
CIFRE = range(0x30, 0x3A)


def tabelle(font):
    """Direttorio delle tabelle: {tag: (checksum, offset, lunghezza)} e l'ordine originale."""
    n = struct.unpack(">H", font[4:6])[0]
    out, ordine = {}, []
    for i in range(n):
        tag, cs, off, ln = struct.unpack(">4sIII", font[12 + 16 * i:28 + 16 * i])
        out[tag.decode("latin-1")] = (cs, off, ln)
        ordine.append(tag.decode("latin-1"))
    return out, ordine


def dati(font, t, tag):
    _, off, ln = t[tag]
    return font[off:off + ln]


def u16(b, o):
    return struct.unpack(">H", b[o:o + 2])[0]


def i16(b, o):
    return struct.unpack(">h", b[o:o + 2])[0]


def u32(b, o):
    return struct.unpack(">I", b[o:o + 4])[0]


# ------------------------------------------------------------------ cmap

def leggi_cmap(cmap):
    """Restituisce le codifiche [(piattaforma, codifica, formato)] e la corrispondenza codice → glifo."""
    n = u16(cmap, 2)
    codifiche, mappa = [], {}
    for i in range(n):
        pid, eid, off = struct.unpack(">HHI", cmap[4 + 8 * i:12 + 8 * i])
        fmt = u16(cmap, off)
        codifiche.append((pid, eid, fmt))
        if fmt == 4:
            segx2 = u16(cmap, off + 6)
            ends = off + 14
            starts = ends + segx2 + 2
            deltas = starts + segx2
            ranges = deltas + segx2
            for s in range(segx2 // 2):
                end, start = u16(cmap, ends + 2 * s), u16(cmap, starts + 2 * s)
                delta, ro = i16(cmap, deltas + 2 * s), u16(cmap, ranges + 2 * s)
                for c in range(start, end + 1):
                    if c == 0xFFFF:
                        continue
                    if ro == 0:
                        g = (c + delta) & 0xFFFF
                    else:
                        g = u16(cmap, ranges + 2 * s + ro + 2 * (c - start))
                        g = (g + delta) & 0xFFFF if g else 0
                    if g:
                        mappa.setdefault(c, g)
        elif fmt == 12:
            ngr = u32(cmap, off + 12)
            for k in range(ngr):
                a, b, g0 = struct.unpack(">III", cmap[off + 16 + 12 * k:off + 28 + 12 * k])
                for c in range(a, b + 1):
                    mappa[c] = g0 + (c - a)
        else:
            raise SystemExit(f"cmap: formato {fmt} non gestito")
    return codifiche, mappa


def formato4(mappa):
    """Sottotabella formato 4 (solo BMP): segmenti di codici contigui, idDelta se i glifi sono
    consecutivi, altrimenti glyphIdArray."""
    codici = sorted(c for c in mappa if c < 0xFFFF)
    segmenti = []  # (inizio, fine)
    for c in codici:
        if segmenti and segmenti[-1][1] == c - 1:
            segmenti[-1][1] = c
        else:
            segmenti.append([c, c])
    # Dentro ogni segmento, le parti con glifi consecutivi lunghe almeno 4 codici usano idDelta; le parti
    # più corte vicine si uniscono in un segmento con glyphIdArray, per non moltiplicare i segmenti.
    uniti = []  # (inizio, fine, con idDelta)
    for a, b in segmenti:
        breve = None
        i = a
        while i <= b:
            j = i
            while j + 1 <= b and mappa[j + 1] == mappa[j] + 1:
                j += 1
            if j - i >= 3:
                if breve:
                    uniti.append((breve[0], breve[1], breve[0] == breve[1]))
                    breve = None
                uniti.append((i, j, True))
            else:
                breve = (breve[0], j) if breve else (i, j)
            i = j + 1
        if breve:
            uniti.append((breve[0], breve[1], breve[0] == breve[1]))
    uniti.append((0xFFFF, 0xFFFF, True))
    n = len(uniti)
    ends, starts, deltas, offs, array = [], [], [], [], []
    for k, (a, b, consecutivi) in enumerate(uniti):
        ends.append(b)
        starts.append(a)
        if a == 0xFFFF:
            deltas.append(1)
            offs.append(0)
        elif consecutivi:
            deltas.append((mappa[a] - a) & 0xFFFF)
            offs.append(0)
        else:
            deltas.append(0)
            # Distanza in byte dalla voce di idRangeOffset di questo segmento all'inizio dei suoi glifi.
            offs.append(2 * (n - k) + 2 * len(array))
            array.extend(mappa.get(c, 0) for c in range(a, b + 1))
    segx2 = 2 * n
    p = 1
    while p * 2 <= n:
        p *= 2
    search, selector = 2 * p, p.bit_length() - 1
    corpo = struct.pack(">HHHH", segx2, search, selector, segx2 - search)
    corpo += struct.pack(f">{n}H", *ends) + b"\0\0" + struct.pack(f">{n}H", *starts)
    corpo += struct.pack(f">{n}H", *deltas) + struct.pack(f">{n}H", *offs) + struct.pack(f">{len(array)}H", *array)
    lunghezza = 6 + len(corpo)
    if lunghezza > 0xFFFF:
        raise SystemExit("cmap formato 4 troppo grande")
    return struct.pack(">HHH", 4, lunghezza, 0) + corpo


def formato12(mappa):
    gruppi = []
    for c in sorted(mappa):
        g = mappa[c]
        if gruppi and gruppi[-1][1] == c - 1 and gruppi[-1][2] + (c - gruppi[-1][0]) == g:
            gruppi[-1][1] = c
        else:
            gruppi.append([c, c, g])
    corpo = b"".join(struct.pack(">III", a, b, g) for a, b, g in gruppi)
    return struct.pack(">HHIII", 12, 0, 16 + len(corpo), 0, len(gruppi)) + corpo


def scrivi_cmap(codifiche, mappa):
    sotto = {}
    for _, _, fmt in codifiche:
        if fmt not in sotto:
            sotto[fmt] = formato4(mappa) if fmt == 4 else formato12(mappa)
    testa = struct.pack(">HH", 0, len(codifiche))
    base = 4 + 8 * len(codifiche)
    offset, corpo = {}, b""
    for fmt in sorted(sotto):
        offset[fmt] = base + len(corpo)
        corpo += sotto[fmt]
        corpo += b"\0" * (-len(corpo) % 4)
    record = b"".join(struct.pack(">HHI", pid, eid, offset[fmt]) for pid, eid, fmt in sorted(codifiche))
    return testa + record + corpo


# ------------------------------------------------------------------ GSUB

def copertura(b, o):
    fmt = u16(b, o)
    if fmt == 1:
        n = u16(b, o + 2)
        return [u16(b, o + 4 + 2 * i) for i in range(n)]
    if fmt == 2:
        out = []
        for i in range(u16(b, o + 2)):
            a, z, _ = struct.unpack(">HHH", b[o + 4 + 6 * i:o + 10 + 6 * i])
            out.extend(range(a, z + 1))
        return out
    raise SystemExit(f"Coverage formato {fmt} non gestito")


def sostituzione_singola(b, o, out):
    fmt = u16(b, o)
    cov = copertura(b, o + u16(b, o + 2))
    if fmt == 1:
        delta = i16(b, o + 4)
        for g in cov:
            out.setdefault(g, (g + delta) & 0xFFFF)
    elif fmt == 2:
        for i, g in enumerate(cov):
            out.setdefault(g, u16(b, o + 6 + 2 * i))
    else:
        raise SystemExit(f"SingleSubst formato {fmt} non gestito")


def tnum(gsub):
    """Glifo → glifo tabulare, da tutte le voci `tnum` della FeatureList."""
    # Intestazione: versione (4 byte), offset di ScriptList, FeatureList, LookupList.
    features, lookups = u16(gsub, 6), u16(gsub, 8)
    indici = []
    for i in range(u16(gsub, features)):
        tag, fo = struct.unpack(">4sH", gsub[features + 2 + 6 * i:features + 8 + 6 * i])
        if tag == b"tnum":
            fo += features
            n = u16(gsub, fo + 2)
            indici.extend(u16(gsub, fo + 4 + 2 * k) for k in range(n))
    if not indici:
        raise SystemExit("il font non ha la funzione tnum")
    out = {}
    for li in sorted(set(indici)):
        lo = lookups + u16(gsub, lookups + 2 + 2 * li)
        tipo, _, n = struct.unpack(">HHH", gsub[lo:lo + 6])
        for s in range(n):
            so = lo + u16(gsub, lo + 6 + 2 * s)
            t = tipo
            if t == 7:  # estensione: tipo e offset a 32 bit
                t = u16(gsub, so + 2)
                so = so + u32(gsub, so + 4)
            if t != 1:
                raise SystemExit(f"lookup tnum di tipo {t} non gestito")
            sostituzione_singola(gsub, so, out)
    return out


# ------------------------------------------------------------------ font

def checksum(b):
    b = b + b"\0" * (-len(b) % 4)
    return sum(struct.unpack(f">{len(b) // 4}I", b)) & 0xFFFFFFFF


def avanzamenti(font, t):
    hhea, hmtx = dati(font, t, "hhea"), dati(font, t, "hmtx")
    n = u16(hhea, 34)

    def av(g):
        return u16(hmtx, 4 * min(g, n - 1))

    return av


def costruisci(font, t, ordine, nuove):
    """Font con le tabelle `nuove` al posto delle originali, checksum ricalcolate."""
    corpi = {tag: nuove.get(tag, dati(font, t, tag)) for tag in ordine}
    head = bytearray(corpi["head"])
    head[8:12] = b"\0\0\0\0"
    corpi["head"] = bytes(head)
    n = len(ordine)
    p = 1
    while p * 2 <= n:
        p *= 2
    testa = font[0:4] + struct.pack(">HHHH", n, 16 * p, p.bit_length() - 1, 16 * n - 16 * p)
    # Le tabelle si scrivono nell'ordine degli offset originali; il direttorio resta in ordine di tag.
    per_offset = sorted(ordine, key=lambda tag: t[tag][1])
    pos = 12 + 16 * n
    posizioni, corpo = {}, b""
    for tag in per_offset:
        posizioni[tag] = pos + len(corpo)
        corpo += corpi[tag] + b"\0" * (-len(corpi[tag]) % 4)
    direttorio = b"".join(struct.pack(">4sIII", tag.encode("latin-1"), checksum(corpi[tag]), posizioni[tag], len(corpi[tag])) for tag in sorted(ordine))
    out = bytearray(testa + direttorio + corpo)
    adj = (0xB1B0AFBA - checksum(bytes(out))) & 0xFFFFFFFF
    h = posizioni["head"]
    out[h + 8:h + 12] = struct.pack(">I", adj)
    return bytes(out)


def verifica(origine, risultato, sostituti):
    t0, _ = tabelle(origine)
    t1, _ = tabelle(risultato)
    _, m0 = leggi_cmap(dati(origine, t0, "cmap"))
    _, m1 = leggi_cmap(dati(risultato, t1, "cmap"))
    assert set(m0) == set(m1), "codici diversi"
    for c in m0:
        atteso = sostituti.get(m0[c], m0[c]) if c in CIFRE else m0[c]
        assert m1[c] == atteso, f"U+{c:04X}: {m1[c]} invece di {atteso}"
    av = avanzamenti(risultato, t1)
    larghezze = {av(m1[c]) for c in CIFRE}
    assert len(larghezze) == 1, f"cifre di larghezze diverse: {larghezze}"
    for tag, (cs, off, ln) in t1.items():
        corpo = risultato[off:off + ln]
        if tag == "head":
            corpo = corpo[:8] + b"\0\0\0\0" + corpo[12:]
        assert checksum(corpo) == cs, f"checksum di {tag}"
    assert checksum(risultato) == 0xB1B0AFBA, "checkSumAdjustment"
    for tag in t0:
        if tag not in ("cmap", "head"):
            assert dati(origine, t0, tag) == dati(risultato, t1, tag), f"{tag} cambiata"
    return larghezze.pop(), sorted({av(m0[c]) for c in CIFRE})


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    cartella = os.path.dirname(os.path.abspath(__file__))
    for peso in PESI:
        sorgente = os.path.join(cartella, f"Inter-{peso}.ttf")
        with open(sorgente, "rb") as f:
            font = f.read()
        t, ordine = tabelle(font)
        codifiche, mappa = leggi_cmap(dati(font, t, "cmap"))
        sostituti = tnum(dati(font, t, "GSUB"))
        nuova = dict(mappa)
        for c in CIFRE:
            g = mappa[c]
            if g not in sostituti:
                raise SystemExit(f"{peso}: U+{c:04X} senza glifo tabulare")
            nuova[c] = sostituti[g]
        risultato = costruisci(font, t, ordine, {"cmap": scrivi_cmap(codifiche, nuova)})
        tab, prima = verifica(font, risultato, sostituti)
        destinazione = os.path.join(cartella, f"Inter-{peso}-tnum.ttf")
        with open(destinazione, "wb") as f:
            f.write(risultato)
        print(f"Inter-{peso}-tnum.ttf: cifre larghe {tab} unità (prima {prima}), {len(sostituti)} glifi nella sostituzione tnum")
    return 0


if __name__ == "__main__":
    sys.exit(main())
