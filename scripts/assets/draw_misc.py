#!/usr/bin/env python3
"""Draw Kabar's supporting illustrations in code (docs/VISUAL-ASSETS.md, prompt 7).

Flat SVG in the souvenir cards' palette, rendered with cairosvg. Masters go to
assets/src/misc/, WebP exports to assets/export/misc/. No text anywhere: the
app draws every word (the hero's greeting sits in its empty sky).

    pip install cairosvg Pillow
    python3 scripts/assets/draw_misc.py
"""
from __future__ import annotations

import io
import math
import random
from pathlib import Path

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "misc"
DEST = ROOT / "assets" / "export" / "misc"

P = "#F5EFE3"  # warm paper
C = "#22201E"  # Merapi charcoal
G = "#4F8A3C"  # sawah green
G2 = "#3F7A33"  # deeper green
O = "#EE8A5A"  # Bromo orange
K = "#E9A3A0"  # Bromo pink
I = "#2B3A70"  # batik indigo
GOLD = "#C9A23A"  # Bridge gold: the hero's glint only
T1, T2 = "#7FC4BD", "#5AA9A6"
D = "#8A5A3C"  # wood
S = "#D9C4A0"  # sand


def svg(w: int, h: int, body: str) -> str:
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{body}</svg>'


def path(d, fill="none", stroke="none", sw=3, op=1, extra=""):
    return f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round" opacity="{op}" {extra}/>'


def circle(x, y, r, fill, stroke="none", sw=2, op=1):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{op}"/>'


def ellipse(x, y, rx, ry, fill, stroke="none", sw=2, op=1):
    return f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{op}"/>'


def rect(x, y, w, h, fill, rx=0, stroke="none", sw=2, op=1):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{op}"/>'


def line(x1, y1, x2, y2, color=C, sw=4, op=1):
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="{sw}" opacity="{op}" stroke-linecap="round"/>'


def render(name: str, w: int, h: int, body: str, grain: bool = True) -> Image.Image:
    png = cairosvg.svg2png(bytestring=svg(w, h, body).encode(), output_width=w, output_height=h)
    img = Image.open(io.BytesIO(png)).convert("RGBA")
    if grain:
        img = add_grain(img, sum(map(ord, name)))
    SRC.mkdir(parents=True, exist_ok=True)
    img.save(SRC / f"{name}.png", optimize=True)
    print("drew", (SRC / f"{name}.png").relative_to(ROOT))
    return img


def add_grain(img: Image.Image, seed: int) -> Image.Image:
    """Faint paper tooth, only where the picture is opaque."""
    r = random.Random(seed)
    px = img.load()
    w, h = img.size
    for _ in range(w * h // 160):
        x, y = r.randrange(w), r.randrange(h)
        cr, cg, cb, ca = px[x, y]
        if ca < 250:
            continue
        d = r.choice([-7, -5, 4, 6])
        px[x, y] = (max(0, min(255, cr + d)), max(0, min(255, cg + d)), max(0, min(255, cb + d)), ca)
    return img


# ---------------------------------------------------------------- hero


def bubble(cx, cy, s):
    """An empty speech bubble with a tiny gold glint."""
    b = path(
        f"M{cx - 110 * s} {cy} Q{cx - 110 * s} {cy - 80 * s} {cx} {cy - 80 * s} Q{cx + 110 * s} {cy - 80 * s} {cx + 110 * s} {cy} "
        f"Q{cx + 110 * s} {cy + 70 * s} {cx + 10 * s} {cy + 72 * s} L{cx - 40 * s} {cy + 110 * s} L{cx - 30 * s} {cy + 66 * s} "
        f"Q{cx - 110 * s} {cy + 56 * s} {cx - 110 * s} {cy}Z",
        P, I, 5 * s, 0.92,
    )
    gx, gy = cx + 70 * s, cy - 46 * s
    glint = path(f"M{gx} {gy - 16 * s} L{gx + 4 * s} {gy - 4 * s} L{gx + 16 * s} {gy} L{gx + 4 * s} {gy + 4 * s} L{gx} {gy + 16 * s} L{gx - 4 * s} {gy + 4 * s} L{gx - 16 * s} {gy} L{gx - 4 * s} {gy - 4 * s}Z", GOLD)
    return b + glint


def hero(w: int, h: int) -> str:
    """Dawn over Javanese rice terraces; a volcano far off; calm sky top centre."""
    z = [
        '<defs><linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">'
        f'<stop offset="0" stop-color="{K}"/><stop offset="0.75" stop-color="{O}"/></linearGradient></defs>',
        rect(0, 0, w, h, "url(#sky)"),
    ]
    horizon = h * (0.56 if w > h else 0.6)
    sun_x = w * (0.72 if w > h else 0.7)
    z.append(circle(sun_x, horizon - h * 0.04, min(w, h) * 0.11, P, op=0.5))
    z.append(circle(sun_x, horizon - h * 0.04, min(w, h) * 0.2, P, op=0.12))
    # long soft clouds
    for k, (cx, cy, rw) in enumerate(((0.18, 0.3, 0.22), (0.86, 0.22, 0.16), (0.3, 0.46, 0.18))):
        z.append(ellipse(w * cx, h * cy, w * rw, h * 0.018, P, op=0.35))
    # the volcano and a far ridge
    vx = w * 0.38
    vw, vh = w * (0.55 if w > h else 0.9), h * (0.24 if w > h else 0.18)
    z.append(path(f"M{vx - vw / 2} {horizon} L{vx - vw * 0.08} {horizon - vh} Q{vx} {horizon - vh - 10} {vx + vw * 0.08} {horizon - vh} L{vx + vw / 2} {horizon}Z", I, op=0.42))
    z.append(path(f"M{vx - vw * 0.08} {horizon - vh} Q{vx} {horizon - vh + 16} {vx + vw * 0.08} {horizon - vh} L{vx + vw * 0.05} {horizon - vh * 0.8} L{vx} {horizon - vh * 0.88} L{vx - vw * 0.05} {horizon - vh * 0.78}Z", P, op=0.35))
    z.append(path(f"M0 {horizon + 10} Q{w * 0.25} {horizon - h * 0.05} {w * 0.5} {horizon} Q{w * 0.75} {horizon + h * 0.03} {w} {horizon - h * 0.04} L{w} {h} L0 {h}Z", I, op=0.3))
    # terraces: stacked curved bands, water catching the sky between them
    bands = 9
    for k in range(bands):
        t = k / bands
        y = horizon + (h - horizon) * (t ** 1.25) * 1.02
        amp = h * 0.03 * (1 + t * 1.6)
        thick = (h - horizon) / bands * (0.9 + t * 0.9)
        col = (G, G2, "#5E9A48")[k % 3]
        top = f"M-20 {y} Q{w * 0.22} {y - amp} {w * 0.45} {y + amp * 0.4} Q{w * 0.72} {y + amp * 1.2} {w + 20} {y - amp * 0.3}"
        z.append(path(f"{top} L{w + 20} {h + 20} L-20 {h + 20}Z", col, op=0.96))
        z.append(path(top, stroke=C, sw=3, op=0.35))
        if k % 2 == 0 and k < bands - 1:  # flooded paddy reflecting the sky
            ry = y + thick * 0.35
            z.append(path(f"M{w * 0.08} {ry} Q{w * 0.25} {ry - amp * 0.8} {w * 0.42} {ry + amp * 0.3} L{w * 0.40} {ry + thick * 0.22} Q{w * 0.24} {ry + thick * 0.1} {w * 0.1} {ry + thick * 0.26}Z", K, op=0.55))
            z.append(path(f"M{w * 0.58} {ry + amp * 0.8} Q{w * 0.74} {ry + amp * 1.0} {w * 0.92} {ry - amp * 0.2} L{w * 0.9} {ry + thick * 0.2} Q{w * 0.74} {ry + thick * 0.4} {w * 0.6} {ry + thick * 0.3}Z", O, op=0.35))
        # rice tufts along the band's edge
        r = random.Random(k)
        for j in range(int(w / 60)):
            x = j * 60 + r.uniform(-12, 12)
            yy = y + (amp * math.sin((x / w) * math.pi * 1.6)) * 0.1 + 6
            z.append(path(f"M{x} {yy + 14} q-4 -12 -10 -18 M{x} {yy + 14} q2 -14 6 -20 M{x} {yy + 14} q8 -8 14 -10", stroke=G2, sw=3, op=0.6))
    # a pair of coconut palms on the left terraces
    for px, py, ph in ((w * 0.08, horizon + (h - horizon) * 0.35, h * 0.2), (w * 0.14, horizon + (h - horizon) * 0.42, h * 0.15)):
        z.append(path(f"M{px} {py} Q{px + 6} {py - ph * 0.5} {px + 14} {py - ph}", stroke=C, sw=8))
        tx, ty = px + 14, py - ph
        for ang in (-165, -125, -85, -50, -15, 25, 205):
            rad = math.radians(ang)
            ex, ey = tx + math.cos(rad) * ph * 0.5, ty + math.sin(rad) * ph * 0.22 + ph * 0.14
            cx, cy = tx + math.cos(rad) * ph * 0.26, ty - ph * 0.12
            z.append(path(f"M{tx} {ty} Q{cx} {cy} {ex} {ey} Q{cx} {cy + ph * 0.09} {tx} {ty}Z", G2, C, 3))
        z.append(circle(tx, ty + 4, ph * 0.035, D, C, 2))
    # the empty speech bubble floating in the sky (right of the greeting area)
    s = min(w, h) / 1024
    # low on the right, clear of the greeting the app draws top centre
    z.append(bubble(w * (0.86 if w > h else 0.8), h * (0.45 if w > h else 0.5), (0.85 if w > h else 0.9) * s))
    return "".join(z)


# ---------------------------------------------------------------- empty states


def shadow(cx, cy, rx):
    return ellipse(cx, cy, rx, rx * 0.12, C, op=0.12)


def flower(x, y, r, color=K, centre=O):
    petals = "".join(circle(x + math.cos(math.radians(a)) * r * 0.8, y + math.sin(math.radians(a)) * r * 0.8, r * 0.55, color, C, 3) for a in range(0, 360, 72))
    return petals + circle(x, y, r * 0.38, centre, C, 3)


def empty_suitcase() -> str:
    z = [shadow(512, 800, 330)]
    # the lid, open and tilted back
    z.append(path("M232 470 L262 250 Q266 226 292 226 L732 226 Q758 226 762 250 L792 470Z", D, C, 8))
    z.append(path("M282 450 L304 270 L720 270 L742 450Z", "#E3C9A8", C, 5))
    z.append(path("M380 300 Q512 330 644 300 M360 360 Q512 390 664 360", stroke=D, sw=4, op=0.4))
    # the case
    z.append(rect(222, 470, 580, 320, D, 26, C, 8))
    z.append(rect(252, 470, 520, 50, "#6E4A30", 8, C, 5))  # open rim, empty inside
    for x in (360, 664):
        z.append(rect(x - 22, 470, 44, 320, "#6E4A30", 4, C, 5) + rect(x - 30, 600, 60, 34, O, 6, C, 4))
    for x in (240, 784):
        z.append(circle(x, 490, 10, O, C, 3) + circle(x, 770, 10, O, C, 3))
    z.append(path("M462 470 Q462 430 512 430 Q562 430 562 470", stroke=C, sw=14))
    # one flower sticker
    z.append(circle(512, 680, 62, P, C, 4) + flower(512, 680, 36, K, O))
    return "".join(z)


def all_reviewed() -> str:
    z = []
    # the window at sunset
    z.append(rect(272, 120, 480, 420, P, 20, C, 8))
    z.append(rect(296, 144, 432, 372, K, 10))
    z.append(path("M296 380 Q512 340 728 390 L728 516 L296 516Z", O, op=0.6))
    z.append(circle(600, 330, 56, P, op=0.6))
    z.append(path("M296 450 Q420 400 520 440 Q620 470 728 430 L728 516 L296 516Z", I, op=0.35))
    z.append(line(512, 144, 512, 516, C, 8) + line(296, 330, 728, 330, C, 8))
    # the table
    z.append(rect(200, 600, 624, 34, D, 10, C, 7) + line(260, 634, 240, 860, C, 12) + line(764, 634, 784, 860, C, 12))
    z.append(shadow(512, 600, 260))
    # the closed notebook
    z.append(path("M300 600 L320 556 L560 556 L560 600Z", I, C, 6) + rect(320, 548, 240, 12, P, 4, C, 4) + line(350, 556, 350, 600, O, 5))
    # tea in a glass cup on a saucer, steam rising
    z.append(ellipse(660, 600, 80, 16, P, C, 5))
    z.append(path("M610 500 L710 500 L698 590 Q660 604 622 590Z", "#B8582E", C, 6, 0.95))
    z.append(path("M610 500 L710 500 L706 530 L614 530Z", P, op=0.3))
    z.append(path("M710 520 Q750 525 744 556 Q738 580 702 574", stroke=C, sw=6))
    for k in range(3):
        x = 630 + k * 30
        z.append(path(f"M{x} 486 q-14 -24 0 -48 q14 -24 0 -48", stroke=P, sw=7, op=0.85))
        z.append(path(f"M{x} 486 q-14 -24 0 -48 q14 -24 0 -48", stroke=C, sw=1.5, op=0.3))
    return "".join(z)


def offline() -> str:
    z = ['<clipPath id="moon"><circle cx="512" cy="512" r="380"/></clipPath>', '<g clip-path="url(#moon)">']
    z.append(rect(132, 132, 760, 760, I))
    r = random.Random(4)
    for _ in range(36):
        z.append(circle(r.uniform(180, 840), r.uniform(170, 520), r.uniform(2, 5), P, op=r.uniform(0.6, 0.95)))
    z.append(circle(680, 300, 44, P, op=0.85) + circle(700, 290, 40, I))  # crescent
    z.append(rect(132, 560, 760, 340, "#3E5C8A"))
    for k in range(5):
        y = 600 + k * 40
        x = 200 + (k * 151) % 480
        z.append(path(f"M{x} {y} q30 -10 60 0", stroke=P, sw=5, op=0.4))
    z.append(path("M132 760 Q420 700 892 780 L892 900 L132 900Z", S, C, 5))
    # the boat resting on the shore, a line to a post
    z.append(path("M330 720 Q470 760 610 712 L590 744 Q470 780 350 748Z", D, C, 6))
    z.append(path("M342 726 Q470 758 600 718", stroke=O, sw=6))
    z.append(line(470, 740, 470, 600, C, 6) + path("M474 610 L560 720 L474 720Z", P, C, 4, 0.9))
    z.append(line(700, 800, 700, 730, D, 12) + path("M600 735 Q650 760 700 745", stroke=C, sw=3))
    z.append("</g>")
    z.append(circle(512, 512, 380, "none", I, 6, 0.5))
    return "".join(z)


def gong() -> str:
    z = []
    # rings of sound: the app animates them; drawn faint and static here
    for k, r in enumerate((300, 360, 420)):
        z.append(circle(512, 560, r, "none", O, 6 - k, 0.35 - k * 0.08))
    # the carved frame: two posts, a beam with curled ends, a base
    z.append(rect(212, 300, 40, 560, D, 8, C, 7) + rect(772, 300, 40, 560, D, 8, C, 7))
    z.append(path("M160 300 Q170 250 222 260 L802 260 Q854 250 864 300 Q850 330 812 316 L212 316 Q174 330 160 300Z", D, C, 7))
    z.append(path("M180 296 q-10 -30 20 -36 M844 296 q10 -30 -20 -36", stroke=O, sw=6))
    z.append(path("M420 260 Q512 200 604 260", "#6E4A30", C, 6))
    z.append(rect(170, 860, 684, 40, D, 10, C, 7))
    # cords and the gong with its raised boss
    z.append(line(430, 316, 452, 380, C, 5) + line(594, 316, 572, 380, C, 5))
    z.append(circle(512, 560, 200, O, C, 8))
    z.append(circle(512, 560, 166, "none", C, 4, 0.35))
    z.append(circle(512, 560, 62, "#D9773F", C, 6) + circle(498, 544, 18, P, op=0.4))
    z.append(path("M370 470 Q400 420 450 400", stroke=P, sw=10, op=0.35))
    return "".join(z)


# ---------------------------------------------------------------- notebook paper


def notebook_paper(n: int = 1024) -> Image.Image:
    """Warm paper, faint fibres and a 5% indigo dot grid; tiles on every edge."""
    z = [rect(0, 0, n, n, P)]
    r = random.Random(7)
    fibres = []
    for _ in range(420):
        x, y = r.uniform(0, n), r.uniform(0, n)
        a = r.uniform(0, math.pi)
        l = r.uniform(10, 34)
        d = f"M{x} {y} q{math.cos(a) * l / 2 + r.uniform(-4, 4)} {math.sin(a) * l / 2 + r.uniform(-4, 4)} {math.cos(a) * l} {math.sin(a) * l}"
        col = r.choice(["#D9C4A0", "#C8B898", "#E7DCC8"])
        for dx in (-n, 0, n):  # wrap around so the edges meet
            for dy in (-n, 0, n):
                fibres.append(f'<path d="{d}" transform="translate({dx} {dy})" fill="none" stroke="{col}" stroke-width="1.6" opacity="0.55" stroke-linecap="round"/>')
    z += fibres
    step = 32
    for y in range(step // 2, n, step):
        for x in range(step // 2, n, step):
            z.append(circle(x, y, 1.8, I, op=0.05 * 2.2))
    png = cairosvg.svg2png(bytestring=svg(n, n, "".join(z)).encode(), output_width=n, output_height=n)
    img = Image.open(io.BytesIO(png)).convert("RGB")
    SRC.mkdir(parents=True, exist_ok=True)
    img.save(SRC / "notebook-paper.png", optimize=True)
    print("drew", (SRC / "notebook-paper.png").relative_to(ROOT))
    return img


# ---------------------------------------------------------------- export


def export(name: str, img: Image.Image, sizes, alpha: bool) -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    for suffix, size in sizes:
        w, h = img.size
        scale = size / max(w, h)
        out = img.resize((round(w * scale), round(h * scale)), Image.LANCZOS)
        if not alpha:
            out = out.convert("RGB")
        out.save(DEST / f"{name}{suffix}.webp", "WEBP", quality=86, method=6, lossless=False)


def main() -> None:
    export("hero-landscape", render("hero-landscape", 1536, 1024, hero(1536, 1024)), (("", 1536), ("-half", 768)), False)
    export("hero-portrait", render("hero-portrait", 1024, 1536, hero(1024, 1536)), (("", 1536), ("-half", 768)), False)
    for name, body in (("empty-suitcase", empty_suitcase()), ("all-reviewed", all_reviewed()), ("offline", offline()), ("celebrate-gong", gong())):
        export(name, render(name, 1024, 1024, body), (("", 640), ("-sm", 320)), True)
    export("notebook-paper", notebook_paper(), (("", 1024), ("-512", 512)), False)
    print(f"exported to {DEST.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
