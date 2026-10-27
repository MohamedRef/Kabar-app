#!/usr/bin/env python3
"""Draw Kabar's souvenir cards in code, one scene per unit (assets/souvenirs.md).

Every card is flat SVG geometry rendered natively at 1024x1536, in the frame,
palette and paper tooth of the approved set (the frame and texture helpers
come from the Codex generator that drew the first cards). A card is named by
the unit whose souvenir it shows: assets/src/souvenirs/<unit id>.png.

    pip install cairosvg Pillow
    python3 scripts/assets/draw_souvenirs.py              # every scene below
    python3 scripts/assets/draw_souvenirs.py bipa1.u09    # one or more units
    python3 scripts/assets/export_souvenirs.py            # then export

Units whose approved master came from elsewhere (bipa4.u03, bipa6.u03,
bipa6.u09, j3.u03, j3.u05, j6.u01) have no scene here and are never touched.
No text, letters or numbers are drawn; no gold (anything gold is orange).
"""
from __future__ import annotations

import io
import math
import random
import sys
from pathlib import Path

import cairosvg
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / "assets" / "src" / "souvenirs"

P = "#F5EFE3"  # warm paper
C = "#22201E"  # Merapi charcoal
G = "#4F8A3C"  # sawah green
O = "#EE8A5A"  # Bromo orange
K = "#E9A3A0"  # Bromo pink
I = "#2B3A70"  # batik indigo
T1, T2, T3 = "#7FC4BD", "#5AA9A6", "#3E8C8F"  # flat turquoise sea tones (raja ampat, bays)
D = "#8A5A3C"  # wood brown (stilts, boats, stalls)
S = "#D9C4A0"  # sand / stone

# Scene panel: x 126–898, y 122–972.
X0, Y0, X1, Y1 = 126, 122, 898, 972
W, H = X1 - X0, Y1 - Y0

# ---------------------------------------------------------------- primitives


def a(**kw) -> str:
    return " ".join(f'{k.replace("_", "-")}="{v}"' for k, v in kw.items())


def path(d, fill="none", stroke="none", sw=3, op=1, extra="", dash=""):
    if dash:
        extra += f' stroke-dasharray="{dash}"'
    return (
        f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" '
        f'stroke-linejoin="round" stroke-linecap="round" opacity="{op}" {extra}/>'
    )


def ellipse(x, y, rx, ry, fill, stroke="none", sw=2, op=1, extra=""):
    return f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{op}" {extra}/>'


def circle(x, y, r, fill, stroke="none", sw=2, op=1):
    return ellipse(x, y, r, r, fill, stroke, sw, op)


def rect(x, y, w, h, fill, rx=0, stroke="none", sw=2, op=1, extra=""):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{op}" {extra}/>'


def line(x1, y1, x2, y2, color=C, sw=4, op=1, dash=""):
    ds = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color}" stroke-width="{sw}" opacity="{op}" stroke-linecap="round"{ds}/>'


def poly(points, fill, stroke="none", sw=3, op=1):
    pts = " ".join(f"{x},{y}" for x, y in points) if not isinstance(points, str) else points
    return f'<polygon points="{pts}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" opacity="{op}"/>'


def group(body, tx=0, ty=0, s=1, rot=0):
    return f'<g transform="translate({tx} {ty}) rotate({rot}) scale({s})">{body}</g>'


def kawung(cx, cy, scale=1, op=0.20):
    els = []
    for dx, dy, rot in [(-9, 0, -17), (9, 0, 17), (0, -9, 0), (0, 9, 0)]:
        els.append(
            ellipse(cx + dx * scale, cy + dy * scale, 6.5 * scale, 11 * scale, "none", I, 1.6 * scale, op,
                    extra=f'transform="rotate({rot} {cx + dx * scale} {cy + dy * scale})"')
        )
    els.append(ellipse(cx, cy, 2.5 * scale, 2.5 * scale, I, op=op))
    return "".join(els)


def leaf(x, y, scale=1, rot=0, color=G):
    return (
        f'<g transform="translate({x} {y}) rotate({rot}) scale({scale})">'
        + path("M0 0 C-28 -38 -20 -85 0 -118 C18 -78 28 -34 0 0Z", color, I, 2, op=0.95)
        + path("M0 -7 Q0 -58 0 -105", stroke=P, sw=2, op=0.6)
        + "</g>"
    )


# ---------------------------------------------------------------- backdrops


def sky(sun=(744, 302), r=94, glow=True):
    z = [rect(X0, Y0, W, H, K)]
    if glow:
        z.append(path("M126 190 Q340 262 590 192 Q730 158 898 210 L898 470 Q660 410 440 470 Q250 515 126 460Z", O, op=0.22))
    if sun:
        z.append(circle(sun[0], sun[1], r, P, op=0.38))
    return "".join(z)


def hills(far=True):
    z = []
    if far:
        z.append(path("M126 660 Q245 520 382 607 Q500 475 617 576 Q773 466 898 550 L898 972 L126 972Z", I, op=0.26))
    z.append(path("M126 732 Q252 625 394 696 Q529 592 674 680 Q801 616 898 683 L898 972 L126 972Z", G, op=0.55))
    z.append(path("M126 838 Q305 759 442 821 Q632 731 898 788 L898 972 L126 972Z", G, op=0.9))
    return "".join(z)


def ground(y=820, color=G, op=0.9):
    return path(f"M126 {y} Q400 {y - 30} 650 {y + 4} Q790 {y + 18} 898 {y - 8} L898 972 L126 972Z", color, op=op)


def sea(y=700, tones=(T1, T2, T3)):
    z = [rect(X0, y, W, Y1 - y, tones[0])]
    z.append(path(f"M126 {y + 70} Q350 {y + 50} 560 {y + 80} Q760 {y + 105} 898 {y + 75} L898 972 L126 972Z", tones[1]))
    z.append(path(f"M126 {y + 170} Q360 {y + 150} 600 {y + 185} Q780 {y + 205} 898 {y + 180} L898 972 L126 972Z", tones[2]))
    for i in range(5):
        yy = y + 40 + i * 48
        xx = 170 + (i * 137) % 560
        z.append(path(f"M{xx} {yy} q22 -10 44 0", stroke=P, sw=5, op=0.55))
    return "".join(z)


def city_far(y=650, op=0.35):
    """A soft skyline band behind a scene."""
    z = []
    xs = [126, 180, 230, 300, 350, 420, 470, 560, 610, 690, 740, 820, 860]
    hs = [70, 120, 60, 150, 90, 60, 170, 80, 130, 70, 110, 60, 140]
    for x, h in zip(xs, hs):
        z.append(rect(x, y - h, 46, h + 40, I, op=op))
    return "".join(z)


# ---------------------------------------------------------------- objects


def palm(x, y, h=260, lean=12, s=1.0):
    top = (x + lean, y - h)
    z = [path(f"M{x} {y} Q{x + lean * 0.2} {y - h * 0.55} {top[0]} {top[1]}", stroke=C, sw=16 * s)]
    for ang in (-150, -115, -70, -35, 10, 200):
        rad = math.radians(ang)
        ex, ey = top[0] + math.cos(rad) * 120 * s, top[1] + math.sin(rad) * 60 * s + 30 * s
        cx, cy = top[0] + math.cos(rad) * 60 * s, top[1] - 30 * s
        z.append(path(f"M{top[0]} {top[1]} Q{cx} {cy} {ex} {ey} Q{cx} {cy + 22 * s} {top[0]} {top[1]}Z", G, C, 4))
    z.append(circle(top[0], top[1] + 6, 10 * s, D, C, 3))
    return "".join(z)


def tree(x, y, s=1.0, color=G):
    z = [path(f"M{x} {y} L{x} {y - 120 * s}", stroke=C, sw=20 * s)]
    for dx, dy, rx, ry in [(-60, -150, 75, 50), (35, -185, 85, 58), (80, -130, 60, 44), (-10, -110, 80, 46)]:
        z.append(ellipse(x + dx * s, y + dy * s, rx * s, ry * s, color, C, 5))
    return "".join(z)


def banyan(x, y, s=1.0):
    z = [path(f"M{x - 30 * s} {y} Q{x - 10 * s} {y - 150 * s} {x} {y - 230 * s} Q{x + 10 * s} {y - 150 * s} {x + 30 * s} {y}Z", D, C, 6)]
    for dx in (-140, -100, -60, 70, 110, 150):
        z.append(line(x + dx * s, y - 200 * s, x + dx * s, y, C, 4, 0.7))
    for dx, dy, rx, ry in [(-120, -260, 110, 60), (0, -300, 140, 75), (120, -250, 110, 60), (0, -220, 170, 55)]:
        z.append(ellipse(x + dx * s, y + dy * s, rx * s, ry * s, G, C, 5))
    return "".join(z)


def person(x, y, s=1.0, body=I, head=C, scarf=None):
    """A small faceless figure standing at (x, y) feet."""
    z = [path(f"M{x - 18 * s} {y} L{x - 14 * s} {y - 70 * s} Q{x} {y - 95 * s} {x + 14 * s} {y - 70 * s} L{x + 18 * s} {y}Z", body, C, 4)]
    z.append(circle(x, y - 108 * s, 17 * s, head if not scarf else scarf, C, 4))
    return "".join(z)


def windows(x, y, cols, rows, w=34, h=60, gx=24, gy=36, fill=I, arch=False, shutter=False):
    z = []
    for r in range(rows):
        for c in range(cols):
            xx, yy = x + c * (w + gx), y + r * (h + gy)
            if arch:
                z.append(path(f"M{xx} {yy + h} L{xx} {yy + w / 2} A{w / 2} {w / 2} 0 0 1 {xx + w} {yy + w / 2} L{xx + w} {yy + h}Z", fill, C, 4))
            else:
                z.append(rect(xx, yy, w, h, fill, 3, C, 4))
            if shutter:
                z.append(line(xx + w / 2, yy + 4, xx + w / 2, yy + h - 4, P, 3, 0.6))
    return "".join(z)


def boat(x, y, s=1.0, hull=D, roof=None, outriggers=False):
    z = []
    if outriggers:
        z.append(line(x - 150 * s, y + 20 * s, x + 150 * s, y + 20 * s, C, 6 * s))
        for dx in (-90, 90):
            z.append(line(x + dx * s, y - 10 * s, x + dx * s, y + 20 * s, C, 5 * s))
    z.append(path(f"M{x - 120 * s} {y - 30 * s} Q{x} {y + 20 * s} {x + 130 * s} {y - 40 * s} L{x + 100 * s} {y} Q{x} {y + 30 * s} {x - 100 * s} {y}Z", hull, C, 6))
    z.append(path(f"M{x - 112 * s} {y - 24 * s} Q{x} {y + 12 * s} {x + 120 * s} {y - 32 * s}", stroke=O, sw=6 * s))
    if roof:
        z.insert(0, rect(x - 60 * s, y - 62 * s, 110 * s, 58 * s, roof, 6, C, 5))
        z.append(path(f"M{x - 75 * s} {y - 62 * s} L{x + 65 * s} {y - 62 * s} L{x + 55 * s} {y - 82 * s} L{x - 65 * s} {y - 82 * s}Z", O, C, 5))
        for i in range(3):
            z.append(rect(x - 48 * s + i * 34 * s, y - 52 * s, 22 * s, 22 * s, P, 3, C, 3))
    return "".join(z)


def island(x, y, rx, ry, palms=0, sand=True):
    z = []
    if sand:
        z.append(ellipse(x, y + ry * 0.35, rx * 1.15, ry * 0.6, S, C, 4))
    z.append(path(f"M{x - rx} {y + ry * 0.3} Q{x - rx * 0.6} {y - ry} {x} {y - ry * 1.1} Q{x + rx * 0.7} {y - ry} {x + rx} {y + ry * 0.3}Z", G, C, 5))
    for i in range(palms):
        z.append(palm(x - rx * 0.3 + i * rx * 0.5, y - ry * 0.4, 90, 8, 0.45))
    return "".join(z)


def lantern(x, y, s=1.0):
    return (
        line(x, y - 40 * s, x, y - 18 * s, C, 3)
        + ellipse(x, y, 26 * s, 22 * s, "#C8483A", C, 4)
        + rect(x - 12 * s, y - 24 * s, 24 * s, 6 * s, C)
        + rect(x - 12 * s, y + 18 * s, 24 * s, 6 * s, C)
        + line(x, y + 24 * s, x, y + 40 * s, O, 3)
    )


def steam(x, y, n=3, gap=34):
    return "".join(path(f"M{x + i * gap} {y} q-14 -24 0 -48 q14 -24 0 -48", stroke=P, sw=7, op=0.8) for i in range(n))


# ---------------------------------------------------------------- scenes
# Each returns the SVG body of the picture panel. Subjects come from the
# unit's souvenir note (content/lessons/.../<unit>.lNN.yaml, `souvenir:`).

SCENES: dict[str, callable] = {}


def scene(unit_id):
    def deco(fn):
        SCENES[unit_id] = fn
        return fn

    return deco


@scene("bipa1.u01")
def fatahillah():
    """Taman Fatahillah: Batavia's old city hall facing a cobbled square."""
    z = [sky(sun=(780, 260), r=80)]
    z.append(ground(760, S, 1))
    for i in range(7):  # cobbles
        z.append(path(f"M{150 + i * 110} 860 q30 -8 60 0", stroke=C, sw=3, op=0.25))
        z.append(path(f"M{200 + i * 110} 920 q30 -8 60 0", stroke=C, sw=3, op=0.25))
    # city hall: wide white block, red-tiled hipped roof, cupola
    z.append(rect(200, 500, 624, 270, P, 4, C, 8))
    z.append(poly([(180, 505), (844, 505), (780, 420), (244, 420)], O, C, 8))
    z.append(rect(472, 330, 80, 95, P, 4, C, 7))
    z.append(path("M462 334 Q512 270 562 334Z", O, C, 7))
    z.append(line(512, 280, 512, 250, C, 5))
    z.append(windows(236, 530, 4, 2, 36, 70, 22, 34, I, shutter=True))
    z.append(windows(578, 530, 4, 2, 36, 70, 22, 34, I, shutter=True))
    z.append(rect(482, 640, 60, 130, C, 4))
    z.append(windows(494, 540, 1, 1, 36, 70, 0, 0, I, arch=True))
    # old bicycles in the square
    for bx in (300, 720):
        z.append(circle(bx - 34, 880, 26, "none", C, 6) + circle(bx + 34, 880, 26, "none", C, 6))
        z.append(path(f"M{bx - 34} 880 L{bx - 5} 845 L{bx + 34} 880 M{bx - 5} 845 L{bx + 20} 845 M{bx + 20} 832 L{bx + 34} 880", stroke=O, sw=6))
    return "".join(z)


@scene("bipa1.u02")
def proklamasi():
    """The house in Menteng where the proclamation was drafted."""
    z = [sky(sun=(250, 270), r=80), hills(False)]
    z.append(tree(205, 760, 0.9) + tree(820, 770, 1.0))
    z.append(rect(290, 520, 450, 250, P, 4, C, 8))
    z.append(poly([(260, 525), (770, 525), (690, 420), (340, 420)], O, C, 8))
    z.append(rect(420, 560, 190, 30, P, 2, C, 5))  # porch beam
    for x in (430, 475, 555, 600):
        z.append(rect(x - 8, 590, 16, 180, P, 2, C, 5))
    z.append(windows(320, 590, 2, 1, 40, 90, 30))
    z.append(windows(640, 590, 2, 1, 40, 90, 20))
    z.append(rect(490, 640, 50, 130, I, 3, C, 5))
    z.append(path("M140 860 Q500 820 898 860 L898 972 L126 972Z", G))  # lawn
    # flagpole with a plain red-over-white flag
    z.append(line(820, 860, 820, 470, C, 7))
    z.append(rect(822, 475, 70, 26, "#C8483A", 0, C, 3) + rect(822, 501, 70, 26, P, 0, C, 3))
    return "".join(z)


@scene("bipa1.u03")
def taman_ismail_marzuki():
    """Taman Ismail Marzuki: the planetarium dome, a family taking a photo."""
    z = [sky(sun=(770, 250), r=78)]
    z.append(ground(800, S, 1))
    z.append(rect(150, 560, 300, 240, P, 6, C, 7))  # theatre block
    z.append(windows(178, 600, 4, 2, 44, 46, 22, 30, I))
    z.append(path("M430 800 L430 560 A190 190 0 0 1 810 560 L810 800Z", P, C, 8))  # dome on drum
    z.append(path("M430 560 A190 190 0 0 1 810 560Z", I, C, 8))
    z.append(path("M480 520 A140 120 0 0 1 760 520", stroke=P, sw=5, op=0.5))
    z.append(rect(430, 560, 380, 30, O, 0, C, 6))
    z.append(tree(855, 800, 0.7))
    # family photo: two adults and a child, one holding a phone
    z.append(person(300, 900, 0.9, O) + person(360, 900, 0.95, I, scarf=K) + person(420, 900, 0.6, G))
    z.append(person(600, 905, 0.95, C, scarf=G))
    z.append(rect(560, 800, 26, 40, C, 4))
    return "".join(z)


@scene("bipa1.u04")
def tanah_abang():
    """Pasar Tanah Abang: stalls stacked with bolts of cloth."""
    z = [rect(X0, Y0, W, H, K)]
    z.append(poly([(126, 122), (898, 122), (898, 240), (126, 240)], I, op=0.9))  # market roof
    for x in range(140, 898, 70):
        z.append(line(x, 122, x + 30, 240, P, 4, 0.35))
    z.append(rect(X0, 860, W, 112, S))  # aisle floor
    cloths = [O, I, G, K, P, "#C8483A"]
    for side, x0 in ((0, 140), (1, 600)):
        z.append(rect(x0, 300, 290, 560, D, 6, C, 7))
        for r in range(6):
            for c in range(4):
                col = cloths[(r * 4 + c + side) % len(cloths)]
                z.append(rect(x0 + 14 + c * 68, 316 + r * 80, 60, 64, col, 8, C, 4))
                z.append(line(x0 + 20 + c * 68, 348 + r * 80, x0 + 68 + c * 68, 348 + r * 80, C, 2, 0.3))
    # hanging lengths of cloth across the aisle
    for i, col in enumerate([O, G, I, K]):
        x = 470 + i * 34
        z.append(path(f"M{x} 250 L{x + 26} 250 L{x + 26} {560 + i * 40} Q{x + 13} {590 + i * 40} {x} {560 + i * 40}Z", col, C, 4))
    # upright rolls in front
    for i, col in enumerate([I, O, G, K, "#C8483A"]):
        z.append(rect(180 + i * 40, 720, 30, 150, col, 12, C, 4))
    return "".join(z)


@scene("bipa1.u05")
def ancol():
    """Pantai Ancol: a seafront promenade and pier at sunset."""
    z = [sky(sun=(640, 560), r=90)]
    z.append(sea(600))
    z.append(path("M126 760 Q420 720 898 800 L898 972 L126 972Z", S, C, 6))  # promenade
    z.append(path("M126 800 Q420 760 898 840", stroke=C, sw=4, op=0.3))
    for x in (230, 420, 610, 800):
        z.append(line(x, 770 + (x - 126) * 0.05, x, 690 + (x - 126) * 0.05, C, 6))
        z.append(circle(x, 682 + (x - 126) * 0.05, 12, P, C, 4))
    # pier
    z.append(rect(560, 640, 330, 22, D, 2, C, 5))
    for x in range(580, 890, 50):
        z.append(line(x, 662, x, 720, C, 6))
    z.append(palm(170, 780, 280, 20, 0.9) + palm(860, 820, 240, -16, 0.8))
    z.append(person(380, 860, 0.8, O) + person(430, 865, 0.6, G))
    return "".join(z)


@scene("bipa1.u06")
def blok_m():
    """Warung makan Blok M: a street food cart at dawn."""
    z = [sky(sun=(250, 330), r=86)]
    z.append(city_far(640, 0.3))
    z.append(ground(820, S, 1))
    # cart with glass case
    z.append(rect(300, 610, 420, 200, D, 8, C, 8))
    z.append(rect(320, 520, 380, 100, P, 6, C, 6))
    z.append(rect(334, 532, 352, 76, T1, 4, C, 4, op=0.55))
    for i, col in enumerate([O, G, P, O]):
        z.append(ellipse(380 + i * 85, 585, 32, 12, P, C, 3) + ellipse(380 + i * 85, 578, 22, 10, col, C, 3))
    # striped awning
    for i in range(7):
        col = O if i % 2 == 0 else P
        z.append(poly([(280 + i * 66, 420), (346 + i * 66, 420), (346 + i * 66, 480), (280 + i * 66, 480)], col, C, 4))
    z.append(line(300, 480, 300, 820, C, 8) + line(720, 480, 720, 820, C, 8))
    for x in (380, 640):
        z.append(circle(x, 830, 34, "none", C, 7))
    # pot with steam
    z.append(path("M740 700 L860 700 L850 790 L750 790Z", I, C, 6) + steam(765, 690))
    # stools
    for x in (200, 820):
        z.append(rect(x - 34, 860, 68, 16, "#C8483A", 4, C, 4) + line(x - 24, 876, x - 30, 930, C, 6) + line(x + 24, 876, x + 30, 930, C, 6))
    return "".join(z)


@scene("bipa1.u07")
def kemang():
    """Kafe-kafe Kemang: a corner café, two coffees and a teapot outside."""
    z = [sky(sun=(780, 260), r=80)]
    z.append(ground(800, S, 1))
    z.append(rect(180, 360, 560, 440, P, 6, C, 8))
    z.append(rect(220, 440, 300, 240, T1, 6, C, 6, op=0.6))  # big open window
    z.append(line(370, 440, 370, 680, C, 5))
    z.append(rect(560, 500, 120, 300, I, 4, C, 6))  # door
    z.append(poly([(160, 370), (760, 370), (720, 300), (200, 300)], G, C, 7))
    for i in range(9):  # string lights
        z.append(circle(220 + i * 60, 400 + (8 if i % 2 else 0), 9, O, C, 2))
    z.append(path("M210 395 Q480 425 730 395", stroke=C, sw=2))
    z.append(tree(820, 800, 1.0))
    # table with two cups and a teapot
    z.append(ellipse(470, 830, 170, 30, D, C, 6))
    z.append(line(470, 860, 470, 950, C, 10))
    z.append(path("M380 800 L420 800 L415 830 L385 830Z", P, C, 4) + path("M520 800 L560 800 L555 830 L525 830Z", P, C, 4))
    z.append(ellipse(470, 805, 40, 30, I, C, 5) + path("M505 800 q30 -6 30 -24", stroke=C, sw=6) + circle(470, 772, 7, C))
    z.append(steam(392, 790, 1) + steam(532, 790, 1))
    return "".join(z)


@scene("bipa1.u08")
def glodok():
    """Kelenteng di Pecinan Glodok: a temple front, lanterns, incense burner."""
    z = [sky(sun=(250, 260), r=78)]
    z.append(ground(820, S, 1))
    z.append(rect(230, 520, 560, 300, "#C8483A", 4, C, 8))
    z.append(path("M170 530 Q510 470 850 530 L790 450 Q510 400 230 450Z", I, C, 8))  # curved roof
    z.append(path("M170 530 Q150 500 135 470 M850 530 Q870 500 885 470", stroke=C, sw=10))
    z.append(path("M250 440 Q510 380 770 440 L720 380 Q510 340 300 380Z", G, C, 7))  # upper roof
    for x in (280, 400, 620, 740):
        z.append(rect(x - 14, 540, 28, 280, "#A0372D", 2, C, 5))
    z.append(rect(440, 600, 140, 220, C, 6))
    z.append(rect(455, 615, 110, 205, D, 4))
    for x in (230, 350, 670, 790):
        z.append(lantern(x, 590, 1.2))
    # incense burner
    z.append(path("M430 900 L590 900 L570 850 L450 850Z", D, C, 6) + rect(420, 836, 180, 18, C, 4))
    for x in (480, 510, 540):
        z.append(line(x, 836, x, 790, O, 4) + path(f"M{x} 785 q-12 -20 0 -40 q12 -20 0 -40", stroke=P, sw=4, op=0.7))
    return "".join(z)


@scene("bipa1.u09")
def monas():
    """Monas: a slender white obelisk on its cup-shaped base, flame in orange."""
    z = [sky(sun=(250, 300), r=80)]
    z.append(hills(False))
    z.append(tree(200, 880, 0.7) + tree(830, 880, 0.75) + tree(300, 900, 0.5))
    # base platform (the "cup")
    z.append(poly([(330, 760), (694, 760), (650, 700), (374, 700)], P, C, 8))
    z.append(rect(300, 760, 424, 50, P, 4, C, 8))
    z.append(line(330, 785, 694, 785, C, 3, 0.3))
    # obelisk
    z.append(poly([(486, 700), (538, 700), (528, 300), (496, 300)], P, C, 8))
    z.append(line(512, 690, 512, 310, C, 3, 0.25))
    # small observation deck and flame
    z.append(rect(482, 286, 60, 18, P, 3, C, 6))
    z.append(path("M512 286 Q480 250 500 220 Q508 240 512 230 Q520 200 512 175 Q548 215 540 250 Q536 270 512 286Z", O, C, 6))
    return "".join(z)


@scene("bipa1.u10")
def kepulauan_seribu():
    """Kapal ke Kepulauan Seribu: a roofed boat crossing to small islands."""
    z = [sky(sun=(760, 280), r=86)]
    z.append(sea(560))
    z.append(island(300, 560, 150, 60, palms=2))
    z.append(island(660, 545, 110, 45, palms=1))
    z.append(island(820, 600, 70, 26))
    z.append(boat(520, 820, 1.6, roof=P))
    z.append(path("M300 860 Q400 840 470 850", stroke=P, sw=6, op=0.7))
    return "".join(z)


@scene("j4.u02")
def malioboro():
    """Dalan Malioboro, full of becak: an Indonesian becak, seat in front."""
    z = [sky(sun=(250, 270), r=80)]
    z.append(city_far(660, 0.3))
    z.append(ground(800, S, 1))
    z.append(tree(780, 800, 1.1))
    # becak facing right: passenger seat + hood at the front, rider behind.
    rear = (300, 860)
    front = (640, 870)
    z.append(circle(*rear, 62, "none", C, 12) + circle(*front, 66, "none", C, 12))
    for c, r in ((rear, 62), (front, 66)):
        for ang in range(0, 180, 30):
            dx, dy = math.cos(math.radians(ang)) * (r - 6), math.sin(math.radians(ang)) * (r - 6)
            z.append(line(c[0] - dx, c[1] - dy, c[0] + dx, c[1] + dy, C, 3, 0.6))
        z.append(circle(*c, 10, C))
    # seat body (box with bench) over the front wheel
    z.append(path("M500 820 L520 650 L760 650 L790 820Z", "#C8483A", C, 8))
    z.append(rect(520, 700, 240, 34, P, 6, C, 5))  # bench cushion
    z.append(path("M790 820 L860 840 L860 860 L780 850Z", D, C, 6))  # footrest
    # folding hood
    z.append(path("M500 655 Q520 470 650 450 Q760 450 770 650 L740 650 Q730 490 650 486 Q545 500 530 655Z", I, C, 8))
    for x in (560, 610, 660, 710):
        z.append(line(x, 470 + abs(x - 640) * 0.25, x + 10, 650, P, 3, 0.4))
    # rider frame behind: saddle over the rear wheel, joined to the seat back
    z.append(path(f"M{rear[0]} {rear[1]} L360 760 L505 760 M360 760 L340 690 M505 760 L505 690", stroke=C, sw=12))
    z.append(path("M310 680 Q340 668 372 680 L366 692 L316 692Z", C, C, 4))  # saddle
    z.append(path("M480 690 L540 690", stroke=C, sw=10))  # handlebar behind the hood
    z.append(circle(360, 800, 22, "none", C, 7) + line(345, 785, 375, 815, C, 6))  # pedal crank
    z.append(path(f"M360 800 L{rear[0]} {rear[1]}", stroke=C, sw=4, op=0.6))  # chain
    return "".join(z)


@scene("bipa2.u01")
def gamelan_bali():
    """Gamelan Bali: a reyong (one row of kettles) and a gangsa in front."""
    z = [sky(sun=(780, 260), r=78)]
    z.append(rect(X0, 640, W, 332, "#C47A55", op=0.35))  # pavilion floor
    z.append(rect(X0, 640, W, 8, C, op=0.4))
    # reyong frame with upswept carved ends
    z.append(path("M160 560 Q140 470 200 450 Q215 500 230 520 L794 520 Q809 500 824 450 Q884 470 864 560Z", O, C, 8))
    z.append(path("M190 470 q20 30 0 60 M834 470 q-20 30 0 60", stroke=I, sw=7))
    z.append(rect(200, 520, 624, 60, "#C8483A", 4, C, 7))
    for x in range(240, 800, 60):
        z.append(path(f"M{x} 545 l12 14 l12 -14", stroke=O, sw=4))
    z.append(line(210, 600, 210, 650, C, 10) + line(814, 600, 814, 650, C, 10))
    z.append(rect(200, 580, 624, 22, D, 4, C, 6))
    z.append(line(200, 508, 824, 508, C, 3, 0.6))  # cord
    for i in range(12):  # twelve kettles, solid knob on top
        x = 237 + i * 50
        z.append(ellipse(x, 500, 23, 11, "#C8743F", C, 5))
        z.append(path(f"M{x - 23} 500 Q{x - 22} 476 {x} 474 Q{x + 22} 476 {x + 23} 500", "#D9895A", C, 5))
        z.append(circle(x, 470, 8, "#B9622F", C, 4))
    # gangsa in front: bars over a carved case, bamboo tubes below
    z.append(path("M250 760 L774 760 L744 900 L280 900Z", O, C, 8))
    for x in range(300, 740, 50):
        z.append(line(x, 790, x + 8, 890, C, 6, 0.7))
    for i in range(7):
        x = 300 + i * 66
        z.append(rect(x, 712, 52, 54 + i * 0, "#C8743F", 6, C, 5))
        z.append(circle(x + 10, 724, 4, C) + circle(x + 42, 724, 4, C))
    z.append(path("M210 900 L814 900", stroke=C, sw=6))
    # mallets
    z.append(line(560, 700, 700, 650, D, 8) + ellipse(560, 702, 16, 10, D, C, 4))
    z.append(line(600, 706, 740, 680, D, 8) + ellipse(600, 708, 16, 10, D, C, 4))
    return "".join(z)


@scene("bipa7.u05")
def wayag():
    """Pulau-pulau karst Wayag, from directly above: islets in turquoise water."""
    z = [rect(X0, Y0, W, H, T2)]
    z.append(path("M126 122 L898 122 L898 350 Q600 300 420 380 Q250 450 126 400Z", T1, op=0.7))
    z.append(path("M126 760 Q380 700 560 780 Q760 850 898 800 L898 972 L126 972Z", T3, op=0.8))
    rnd = random.Random(7)
    islets = [(300, 300, 95, 75), (560, 240, 70, 55), (720, 420, 120, 90), (420, 520, 80, 66), (230, 650, 110, 80),
              (640, 700, 75, 60), (800, 860, 60, 45)]
    for x, y, rx, ry in islets:
        pts = []
        for k in range(14):
            ang = k / 14 * 2 * math.pi
            rr = 1 + rnd.uniform(-0.16, 0.16)
            pts.append((x + math.cos(ang) * rx * rr, y + math.sin(ang) * ry * rr))
        ring = [(x + (px - x) * 1.22, y + (py - y) * 1.22) for px, py in pts]
        z.append(poly(ring, T1))
        z.append(poly([(x + (px - x) * 1.1, y + (py - y) * 1.1) for px, py in pts], S, op=0.85))
        z.append(poly(pts, "#3F7A33", C, 6))
        z.append(poly([(x + (px - x) * 0.62 - rx * 0.08, y + (py - y) * 0.62 - ry * 0.1) for px, py in pts], G))
    # a small boat with its wake
    z.append(path("M470 860 Q530 880 600 905", stroke=P, sw=8, op=0.6))
    z.append(ellipse(460, 855, 20, 9, P, C, 3, extra='transform="rotate(20 460 855)"'))
    return "".join(z)



# ---------------------------------------------------------------- more objects


def candi_bentar(cx, y, h=330, w=110, gap=50, color="#B5654A"):
    """A Balinese split gate: two mirrored stepped halves with a gap between."""
    z = []
    for side in (-1, 1):
        x = cx + side * (gap / 2)
        far = x + side * w
        steps = []
        for k in range(5):
            hh = h * (1 - k * 0.17)
            inset = k * w * 0.1
            steps.append((x + side * 0, y - hh, far - side * inset, y - hh + h * 0.17))
        z.append(poly([(x, y), (far, y), (far - side * w * 0.12, y - h * 0.45), (x + side * w * 0.05, y - h)], color, C, 7))
        for k in range(1, 6):
            yy = y - h * k * 0.16
            z.append(line(x + side * 6, yy, far - side * (w * 0.12 * k / 3), yy, C, 3, 0.45))
    return "".join(z)


def meru(cx, y, tiers=5, w=220, color=C):
    """A Balinese pagoda: a stone base with stacked thatched roofs."""
    z = [rect(cx - w * 0.35, y - 70, w * 0.7, 70, S, 4, C, 6)]
    yy = y - 70
    for k in range(tiers):
        ww = w * (1 - k * 0.14)
        z.append(poly([(cx - ww / 2, yy), (cx + ww / 2, yy), (cx + ww * 0.32, yy - 46), (cx - ww * 0.32, yy - 46)], color, C, 5))
        z.append(rect(cx - ww * 0.18, yy - 60, ww * 0.36, 16, D, 2, C, 3))
        yy -= 60
    z.append(line(cx, yy, cx, yy - 30, C, 5))
    return "".join(z)


def mosque(cx, y, s=1.0, dome=G):
    z = [rect(cx - 140 * s, y - 160 * s, 280 * s, 160 * s, P, 4, C, 7)]
    z.append(path(f"M{cx - 100 * s} {y - 160 * s} Q{cx - 100 * s} {y - 290 * s} {cx} {y - 300 * s} Q{cx + 100 * s} {y - 290 * s} {cx + 100 * s} {y - 160 * s}Z", dome, C, 7))
    z.append(line(cx, y - 300 * s, cx, y - 340 * s, C, 5) + circle(cx, y - 345 * s, 8 * s, O, C, 3))
    for side in (-1, 1):
        mx = cx + side * 190 * s
        z.append(rect(mx - 20 * s, y - 320 * s, 40 * s, 320 * s, P, 3, C, 6))
        z.append(path(f"M{mx - 26 * s} {y - 320 * s} Q{mx} {y - 380 * s} {mx + 26 * s} {y - 320 * s}Z", dome, C, 5))
    z.append(windows(cx - 110 * s, y - 120 * s, 4, 1, 34 * s, 70 * s, 26 * s, 0, I, arch=True))
    return "".join(z)


def waterfall(x, top, bottom, w=120):
    z = [path(f"M{x - w / 2} {top} L{x + w / 2} {top} L{x + w * 0.7} {bottom} L{x - w * 0.7} {bottom}Z", T1, C, 5)]
    for k in range(-2, 3):
        z.append(line(x + k * w * 0.18, top + 20, x + k * w * 0.26, bottom - 20, P, 6, 0.75))
    z.append(ellipse(x, bottom + 10, w * 1.2, 30, T2, C, 5))
    for k in (-1, 0, 1):
        z.append(circle(x + k * 50, bottom - 6, 18, P, op=0.8))
    return "".join(z)


def turtle(x, y, s=1.0, rot=0):
    body = (
        ellipse(0, 0, 70, 52, "#3F7A33", C, 6)
        + ellipse(0, 0, 44, 32, G, C, 3)
        + "".join(path(f"M{dx} {dy} l{ex} {ey}", stroke=C, sw=3, op=0.5) for dx, dy, ex, ey in [(-20, -20, 40, 40), (20, -20, -40, 40)])
        + ellipse(86, 0, 22, 16, G, C, 5)
        + ellipse(30, -58, 34, 14, G, C, 5, extra='transform="rotate(-35 30 -58)"')
        + ellipse(30, 58, 34, 14, G, C, 5, extra='transform="rotate(35 30 58)"')
        + ellipse(-50, -44, 20, 10, G, C, 4)
        + ellipse(-50, 44, 20, 10, G, C, 4)
    )
    return group(body, x, y, s, rot)


def dolphin(x, y, s=1.0, rot=0):
    body = (
        path("M-90 10 Q-40 -60 50 -40 Q90 -30 110 -10 Q80 -6 60 0 Q0 20 -60 30 Z", "#6E7FA8", C, 5)
        + path("M-10 -50 L10 -90 L30 -45Z", "#6E7FA8", C, 5)
        + path("M-88 10 L-130 -10 L-120 30Z", "#6E7FA8", C, 5)
        + circle(70, -24, 5, C)
    )
    return group(body, x, y, s, rot)


def ferry(x, y, s=1.0):
    z = [path(f"M{x - 220 * s} {y - 40 * s} L{x + 230 * s} {y - 40 * s} L{x + 190 * s} {y + 30 * s} L{x - 200 * s} {y + 30 * s}Z", P, C, 7)]
    z.append(rect(x - 220 * s, y - 40 * s, 450 * s, 18 * s, "#C8483A", 0, C, 5))
    z.append(rect(x - 150 * s, y - 120 * s, 300 * s, 80 * s, P, 4, C, 6))
    z.append(windows(x - 130 * s, y - 104 * s, 6, 1, 30 * s, 26 * s, 18 * s, 0, I))
    z.append(rect(x - 80 * s, y - 170 * s, 160 * s, 50 * s, P, 4, C, 6))
    z.append(rect(x + 30 * s, y - 220 * s, 34 * s, 54 * s, O, 3, C, 5))
    return "".join(z)


def volcano(cx, y, w=520, h=300, color=I, op=0.55, crater=True):
    z = [path(f"M{cx - w / 2} {y} L{cx - w * 0.09} {y - h} Q{cx} {y - h - 14} {cx + w * 0.09} {y - h} L{cx + w / 2} {y}Z", color, op=op)]
    if crater:
        z.append(path(f"M{cx - w * 0.09} {y - h} Q{cx} {y - h + 18} {cx + w * 0.09} {y - h}", stroke=C, sw=4, op=0.5))
    return "".join(z)


def stall(x, y, w=260, awning=O, goods=(O, G, K)):
    z = [rect(x, y - 120, w, 120, D, 4, C, 6)]
    for i in range(int(w // 70)):
        z.append(ellipse(x + 40 + i * 70, y - 128, 30, 16, goods[i % len(goods)], C, 4))
    z.append(line(x + 10, y - 120, x + 10, y - 260, C, 6) + line(x + w - 10, y - 120, x + w - 10, y - 260, C, 6))
    z.append(path(f"M{x - 20} {y - 250} L{x + w + 20} {y - 250} L{x + w} {y - 300} L{x} {y - 300}Z", awning, C, 6))
    return "".join(z)


def thatch_house(cx, y, w=260, h=150, roof=D, walls=S):
    z = [rect(cx - w / 2, y - h, w, h, walls, 3, C, 6)]
    z.append(path(f"M{cx - w * 0.7} {y - h + 10} L{cx} {y - h - 170} L{cx + w * 0.7} {y - h + 10}Z", roof, C, 7))
    for k in range(5):
        z.append(line(cx - w * 0.6 + k * w * 0.3, y - h + 4, cx, y - h - 160, C, 2, 0.35))
    z.append(rect(cx - 26, y - 100, 52, 100, C, 3))
    return "".join(z)


@scene("bipa2.u02")
def pasar_badung():
    """Pasar Badung: Denpasar's market, baskets of flowers and fruit."""
    z = [sky(sun=(760, 260), r=80)]
    z.append(rect(X0, 800, W, 172, S))
    z.append(stall(150, 800, 300, O, (O, G, K)) + stall(500, 800, 340, G, (K, O, P)))
    # offering baskets of flowers in front
    for i, col in enumerate([O, K, P, O]):
        x = 220 + i * 170
        z.append(ellipse(x, 900, 70, 26, D, C, 5) + path(f"M{x - 70} 900 L{x - 56} 950 L{x + 56} 950 L{x + 70} 900", D, C, 5))
        for k in range(5):
            z.append(circle(x - 44 + k * 22, 888 - (k % 2) * 10, 14, col, C, 3))
    z.append(person(470, 780, 0.9, I, scarf=O))
    return "".join(z)


@scene("bipa2.u03")
def ngurah_rai():
    """Arriving at Ngurah Rai: a plane landing over the sea past a split gate."""
    z = [sky(sun=(250, 260), r=80)]
    z.append(sea(640))
    z.append(path("M126 760 L898 700 L898 972 L126 972Z", S))
    z.append(path("M126 820 L898 760 L898 800 L126 860Z", C, op=0.85))  # runway
    for k in range(7):
        x = 170 + k * 110
        z.append(line(x, 838 - k * 8.6, x + 50, 834 - k * 8.6, P, 5))
    z.append(candi_bentar(240, 760, 200, 70, 34))
    # plane descending
    z.append(group(
        path("M-170 0 Q-160 -24 -120 -26 L150 -26 Q190 -20 200 0 Q190 14 150 16 L-120 16 Q-160 14 -170 0Z", P, C, 6)
        + path("M-20 -10 L60 -10 L-40 -110 L-80 -110Z", I, C, 6)
        + path("M-20 12 L60 12 L0 70 L-40 70Z", I, C, 5)
        + path("M150 -24 L190 -86 L210 -86 L190 -16Z", O, C, 5)
        + "".join(circle(-120 + k * 40, -6, 7, I) for k in range(7)),
        560, 420, 1.0, -8))
    return "".join(z)


@scene("bipa2.u04")
def sukawati():
    """Pasar Seni Sukawati: an art market, carvings, paintings, sarongs."""
    z = [sky(sun=(770, 250), r=78)]
    z.append(rect(X0, 820, W, 152, S))
    z.append(rect(150, 380, 724, 440, D, 6, C, 7))
    z.append(poly([(130, 390), (894, 390), (850, 300), (174, 300)], C, C, 6))
    # paintings
    for i, (col, sky_) in enumerate([(G, O), (I, K), (G, P)]):
        x = 190 + i * 150
        z.append(rect(x, 420, 120, 150, P, 4, C, 6) + rect(x + 10, 430, 100, 130, sky_) + path(f"M{x + 10} 530 Q{x + 60} 470 {x + 110} 520 L{x + 110} 560 L{x + 10} 560Z", col))
    # hanging sarongs
    for i, col in enumerate([O, I, K, G]):
        x = 650 + i * 52
        z.append(path(f"M{x} 410 L{x + 44} 410 L{x + 44} 640 Q{x + 22} 660 {x} 640Z", col, C, 4))
        z.append(line(x + 8, 450, x + 36, 450, P, 3, 0.6) + line(x + 8, 600, x + 36, 600, P, 3, 0.6))
    # wooden carvings on a shelf
    z.append(rect(180, 700, 660, 20, C))
    for i in range(6):
        x = 220 + i * 110
        z.append(path(f"M{x - 18} 700 L{x - 22} 640 Q{x} 590 {x + 22} 640 L{x + 18} 700Z", "#A9673F", C, 5) + circle(x, 620 - (i % 2) * 6, 16, "#A9673F", C, 4))
    return "".join(z)


@scene("bipa2.u05")
def warung_halal():
    """Babi guling vs. warung halal: a Muslim warung's rice plates, a Balinese gate behind."""
    z = [sky(sun=(250, 260), r=80), hills(False)]
    z.append(candi_bentar(700, 760, 260, 90, 40))
    z.append(rect(200, 560, 380, 260, P, 6, C, 7))
    z.append(path("M180 570 L600 570 L560 480 L220 480Z", G, C, 7))
    z.append(rect(230, 600, 320, 110, T1, 6, C, 5, op=0.55))
    for i, col in enumerate([O, G, P]):
        z.append(ellipse(290 + i * 100, 670, 40, 16, P, C, 3) + ellipse(290 + i * 100, 660, 28, 12, col, C, 3))
    # a plate of nasi campur on the table front
    z.append(rect(150, 840, 520, 22, D, 4, C, 5))
    z.append(ellipse(400, 830, 110, 26, P, C, 5) + ellipse(370, 820, 40, 22, P, C, 3) + ellipse(430, 822, 30, 14, O, C, 3) + ellipse(455, 812, 20, 10, G, C, 3))
    z.append(path("M330 840 L345 760 M460 840 L470 760", stroke=C, sw=0))
    return "".join(z)


@scene("bipa2.u06")
def rumah_bali():
    """A Balinese family compound: pavilions and the family shrine."""
    z = [sky(sun=(760, 250), r=80)]
    z.append(ground(800, S, 1))
    z.append(rect(126, 640, 772, 30, "#B5654A", 0, C, 5))  # compound wall
    z.append(rect(126, 670, 772, 140, "#C47A55", 0, C, 5))
    # pavilion (bale) with thatch roof on posts
    z.append(rect(220, 700, 300, 40, S, 4, C, 6))
    for x in (240, 330, 410, 500):
        z.append(line(x, 700, x, 600, C, 10))
    z.append(path("M190 610 L550 610 L470 470 L270 470Z", D, C, 7))
    # family shrine (sanggah): small meru shapes
    z.append(meru(700, 640, 3, 150))
    z.append(meru(820, 650, 2, 110))
    # canang offering on the steps
    z.append(rect(560, 860, 90, 30, G, 4, C, 4) + circle(585, 855, 10, O) + circle(605, 852, 10, K) + circle(625, 855, 10, P))
    z.append(palm(170, 820, 260, 10, 0.8))
    return "".join(z)


@scene("bipa2.u07")
def kuta():
    """Surfing at Kuta: a surfer on a wave, boards on the beach."""
    z = [sky(sun=(640, 470), r=90)]
    z.append(sea(560))
    # breaking wave
    z.append(path("M126 760 Q300 600 470 640 Q560 660 560 720 Q520 690 480 700 Q500 740 560 760 L126 800Z", T2, C, 6))
    z.append(path("M140 750 Q300 620 470 650", stroke=P, sw=10, op=0.8))
    # surfer on a board
    z.append(path("M300 700 Q360 680 430 690 Q380 712 300 700Z", O, C, 5))
    z.append(path("M360 690 L350 640 L380 600 M350 640 L330 610 M380 600 L410 620 M370 640 L392 688", stroke=C, sw=12))
    z.append(circle(382, 585, 16, C))
    z.append(path("M126 860 Q500 820 898 870 L898 972 L126 972Z", S, C, 5))
    for i, col in enumerate([O, I, G]):
        z.append(path(f"M{700 + i * 50} 900 L{700 + i * 50} 760 Q{720 + i * 50} 730 {740 + i * 50} 760 L{740 + i * 50} 900Z", col, C, 5))
    z.append(palm(200, 900, 240, 18, 0.8))
    return "".join(z)


@scene("bipa2.u08")
def ulun_danu():
    """Pura Ulun Danu Beratan: tiered shrines standing in the lake, mountains behind."""
    z = [sky(sun=(250, 250), r=78)]
    z.append(volcano(520, 640, 900, 260, I, 0.35, False))
    z.append(path("M126 640 Q300 580 450 620 Q650 560 898 610 L898 680 L126 680Z", G, op=0.7))
    z.append(sea(660, (T1, T2, T3)))
    z.append(ellipse(470, 730, 150, 26, S, C, 5))
    z.append(meru(470, 720, 11, 200))
    z.append(ellipse(720, 760, 90, 18, S, C, 5))
    z.append(meru(720, 752, 3, 120))
    # reflection
    z.append(path("M410 760 L530 760 L500 900 L440 900Z", C, op=0.15))
    return "".join(z)


@scene("bipa2.u09")
def lovina():
    """Dolphins at dawn, Lovina: dolphins arching beside an outrigger boat."""
    z = [sky(sun=(512, 520), r=100)]
    z.append(sea(560))
    z.append(dolphin(330, 650, 1.1, -18) + dolphin(560, 700, 0.9, -10))
    z.append(path("M230 690 Q330 640 440 690", stroke=P, sw=6, op=0.7))
    z.append(boat(640, 860, 1.4, hull=P, outriggers=True))
    z.append(line(640, 830, 640, 700, C, 6) + path("M644 700 L720 760 L644 800Z", O, C, 5))
    return "".join(z)


@scene("bipa2.u10")
def jamu():
    """Jamu: herbal tonic bottles, turmeric and ginger roots, a glass of jamu."""
    z = [rect(X0, Y0, W, H, K), circle(744, 302, 94, P, op=0.38)]
    z.append(rect(X0, 700, W, 272, D, 0, C, 6))  # table
    z.append(rect(X0, 700, W, 20, "#A9673F"))
    # basket carried by jamu sellers
    z.append(path("M180 690 L520 690 L490 470 L210 470Z", "#C89B5E", C, 7))
    for k in range(5):
        z.append(line(210 + k * 70, 480, 200 + k * 76, 690, C, 3, 0.4))
    for i, col in enumerate(["#E3A33B", "#C8743F", "#E7D7A8", O]):  # bottles
        x = 250 + i * 70
        z.append(rect(x - 22, 330, 44, 150, col, 10, C, 5) + rect(x - 10, 300, 20, 34, P, 3, C, 4))
    # glass of jamu
    z.append(path("M620 560 L700 560 L690 690 L630 690Z", "#E3A33B", C, 6))
    z.append(ellipse(660, 560, 40, 10, "#F0C060", C, 4))
    # turmeric and ginger roots
    for x, y, col in [(600, 760, "#E3A33B"), (720, 790, "#D9B48A"), (480, 800, "#E3A33B")]:
        z.append(path(f"M{x} {y} q30 -30 70 -10 q20 -30 50 -10 q10 30 -30 40 q-40 20 -90 -20Z", col, C, 5))
    z.append(leaf(820, 720, 0.8, 30) + leaf(780, 720, 0.7, -20))
    return "".join(z)


@scene("bipa3.u01")
def ketupat():
    """Lebaran ketupat: woven rice packets hanging, a mosque at dusk behind."""
    z = [sky(sun=(760, 300), r=70)]
    z.append(mosque(500, 760, 0.95))
    z.append(ground(760, G, 0.9))
    z.append(line(126, 220, 898, 260, C, 4))
    for i in range(6):
        x = 180 + i * 125
        y = 330 + (i % 2) * 30 + i * 4
        z.append(line(x, 225 + i * 6, x, y - 50, C, 3))
        z.append(poly([(x, y - 50), (x + 48, y), (x, y + 50), (x - 48, y)], "#9BB85A", C, 5))
        z.append(line(x - 24, y - 24, x + 24, y + 24, C, 3, 0.5) + line(x + 24, y - 24, x - 24, y + 24, C, 3, 0.5))
        z.append(path(f"M{x} {y + 50} q10 30 -6 50", stroke="#9BB85A", sw=5))
    return "".join(z)


@scene("bipa3.u02")
def gili_turtles():
    """Sea turtles of the Gilis: a turtle gliding over coral."""
    z = [rect(X0, Y0, W, H, T1)]
    z.append(path("M126 122 L898 122 L898 260 Q600 230 400 280 Q240 310 126 270Z", T1))
    for k in range(5):
        z.append(path(f"M{150 + k * 160} 180 q30 -14 60 0", stroke=P, sw=5, op=0.6))
    z.append(path("M126 800 Q380 760 560 820 Q760 870 898 830 L898 972 L126 972Z", S))
    for x, col in [(200, O), (320, K), (650, "#C8483A"), (800, O)]:  # coral
        z.append(path(f"M{x} 860 L{x} 780 M{x} 820 L{x - 30} 760 M{x} 810 L{x + 34} 750 M{x - 30} 760 L{x - 40} 730", stroke=col, sw=14))
    z.append(turtle(500, 520, 1.6, -15))
    for x, y in [(300, 380), (700, 700), (760, 400)]:
        z.append(circle(x, y, 10, P, C, 2, 0.8) + circle(x + 18, y - 26, 6, P, C, 2, 0.8))
    return "".join(z)


@scene("bipa3.u03")
def layar_tancap():
    """Open-air cinema on the beach: a white screen on poles, mats, night sky."""
    z = [rect(X0, Y0, W, H, I)]
    for x, y in [(200, 200), (330, 260), (520, 180), (700, 240), (820, 190), (620, 320), (260, 360)]:
        z.append(circle(x, y, 5, P, op=0.9))
    z.append(circle(760, 300, 50, P, op=0.85))
    z.append(sea(600, ("#3B5A8A", "#2F4C78", "#26406A")))
    z.append(path("M126 780 Q500 740 898 790 L898 972 L126 972Z", S, C, 5))
    z.append(rect(330, 420, 360, 220, P, 4, C, 7))
    z.append(line(330, 420, 330, 800, C, 10) + line(690, 420, 690, 800, C, 10))
    z.append(path("M380 600 Q440 520 500 580 Q560 500 640 590 L640 620 L380 620Z", K, op=0.6))
    for i, col in enumerate([O, G, K]):
        z.append(rect(220 + i * 200, 850, 160, 40, col, 6, C, 4))
    z.append(person(300, 850, 0.6, C) + person(500, 850, 0.6, O) + person(700, 850, 0.6, G))
    return "".join(z)


@scene("bipa3.u04")
def sembalun():
    """Sembalun strawberry fields at the foot of Rinjani."""
    z = [sky(sun=(250, 250), r=78)]
    z.append(volcano(560, 620, 760, 340, I, 0.45))
    z.append(path("M126 600 Q350 560 520 600 Q700 640 898 590 L898 972 L126 972Z", G, op=0.6))
    for r in range(5):  # field rows
        y = 660 + r * 62
        z.append(path(f"M126 {y} Q500 {y - 20} 898 {y}", stroke="#3F7A33", sw=30))
        for k in range(8):
            x = 160 + k * 95 + (r % 2) * 40
            z.append(path(f"M{x} {y - 4} q-14 -4 -12 12 q4 16 12 20 q10 -4 14 -20 q2 -16 -14 -12Z", "#C8483A", C, 3))
    z.append(path("M126 960 L898 960", stroke=D, sw=40))
    return "".join(z)


@scene("bipa3.u05")
def sendang_gile():
    """Sendang Gile waterfall in the Senaru forest."""
    z = [sky(sun=(760, 250), r=70)]
    z.append(path("M126 260 L360 300 L420 860 L126 972Z", G, C, 6))
    z.append(path("M898 260 L640 300 L590 860 L898 972Z", "#3F7A33", C, 6))
    z.append(path("M360 300 L640 300 L620 340 L380 340Z", D, C, 5))
    z.append(waterfall(510, 330, 830, 170))
    z.append(path("M126 880 Q500 840 898 900 L898 972 L126 972Z", T2))
    for x, y, r_ in [(200, 400, -20), (820, 420, 20), (180, 640, -40), (840, 650, 40)]:
        z.append(leaf(x, y, 1.0, r_) + leaf(x + 20, y + 30, 0.8, r_ + 30))
    return "".join(z)


@scene("bipa3.u06")
def mandalika():
    """The Mandalika circuit: a racetrack curving by the sea."""
    z = [sky(sun=(640, 300), r=86)]
    z.append(sea(520))
    z.append(path("M126 620 Q500 580 898 630 L898 972 L126 972Z", G))
    z.append(path("M140 940 Q140 700 400 690 Q640 680 700 760 Q760 840 860 800", stroke=C, sw=70))
    z.append(path("M140 940 Q140 700 400 690 Q640 680 700 760 Q760 840 860 800", stroke=P, sw=4, dash="24 18"))
    for k in range(10):  # kerbs
        z.append(circle(200 + k * 60, 690 - (k % 3) * 4, 7, "#C8483A" if k % 2 else P))
    z.append(rect(580, 600, 240, 50, P, 4, C, 5) + rect(570, 580, 260, 20, I, 0, C, 4))  # stand
    for k in range(5):
        z.append(line(600 + k * 50, 612, 600 + k * 50, 640, C, 3, 0.5))
    z.append(group(path("M-60 0 Q-50 -26 0 -26 Q50 -26 60 0Z", O, C, 5) + circle(-36, 4, 14, C) + circle(36, 4, 14, C), 420, 735, 0.9))
    return "".join(z)


@scene("bipa3.u07")
def sade():
    """Sade, a Sasak village: steep thatched rice barns (lumbung) and houses."""
    z = [sky(sun=(760, 250), r=78), hills(False)]
    z.append(ground(820, S, 1))
    # lumbung: thatched bonnet roof on stilts
    for cx in (320, 620):
        for dx in (-50, 50):
            z.append(line(cx + dx, 820, cx + dx, 680, C, 12))
        z.append(rect(cx - 70, 670, 140, 20, D, 2, C, 4))
        z.append(path(f"M{cx - 90} 680 Q{cx - 110} 520 {cx} 380 Q{cx + 110} 520 {cx + 90} 680Z", "#B28B57", C, 7))
        z.append(rect(cx - 30, 560, 60, 60, D, 6, C, 4))
        for k in range(4):
            z.append(path(f"M{cx - 80 + k * 10} {660 - k * 60} Q{cx} {640 - k * 64} {cx + 80 - k * 10} {660 - k * 60}", stroke=C, sw=2, op=0.35))
    z.append(thatch_house(800, 860, 140, 90, "#B28B57"))
    z.append(person(470, 880, 0.75, "#C8483A", scarf=I))
    return "".join(z)


@scene("bipa3.u08")
def segara_anak():
    """Segara Anak: Rinjani's crater lake with the small cone inside."""
    z = [sky(sun=(250, 240), r=74)]
    z.append(path("M126 420 L300 330 L460 380 L620 300 L898 400 L898 972 L126 972Z", I, op=0.55))
    z.append(path("M126 560 Q300 460 500 470 Q720 470 898 540 L898 972 L126 972Z", G, op=0.65))
    z.append(ellipse(520, 700, 330, 120, T2, C, 6))
    z.append(path("M420 700 Q470 600 520 590 Q560 600 600 700Z", "#6B5A57", C, 5))
    z.append(path("M500 590 q10 -30 -4 -60 M530 588 q10 -30 0 -50", stroke=P, sw=6, op=0.6))
    z.append(path("M126 860 Q500 800 898 870 L898 972 L126 972Z", "#3F7A33"))
    z.append(person(230, 880, 0.7, O) + person(270, 884, 0.6, I))
    return "".join(z)


@scene("bipa3.u09")
def putri_mandalika():
    """Legend of Putri Mandalika: Bau Nyale night, torches on the shore by the sea."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(circle(700, 300, 70, P, op=0.85))
    for x, y in [(220, 200), (380, 260), (560, 190), (840, 230), (300, 380)]:
        z.append(circle(x, y, 5, P))
    z.append(sea(560, ("#3B5A8A", "#2F4C78", "#26406A")))
    for k in range(9):  # glowing nyale worms in the water
        x, y = 200 + k * 75, 680 + (k % 3) * 40
        z.append(path(f"M{x} {y} q12 -10 24 0 q12 10 24 0", stroke="#E7A3E0", sw=6, op=0.8))
    z.append(path("M126 820 Q500 780 898 830 L898 972 L126 972Z", S, C, 5))
    for x in (250, 450, 650, 820):
        z.append(person(x, 900, 0.7, C))
        z.append(line(x + 14, 820, x + 26, 760, D, 6) + path(f"M{x + 26} 760 q-14 -24 2 -44 q12 18 10 44Z", O, C, 3))
    return "".join(z)


@scene("bipa3.u10")
def ferry_sumbawa():
    """Ferry to Sumbawa: a car ferry crossing the strait."""
    z = [sky(sun=(250, 260), r=80)]
    z.append(path("M560 560 Q700 470 898 520 L898 600 L560 600Z", I, op=0.4))
    z.append(sea(600))
    z.append(ferry(470, 770, 1.1))
    z.append(path("M180 820 Q260 800 300 812", stroke=P, sw=7, op=0.7))
    for x, y in [(700, 360), (760, 330)]:
        z.append(path(f"M{x} {y} q14 -14 28 0 q14 -14 28 0", stroke=C, sw=5))
    return "".join(z)

# ---------------------------------------------------------------- BIPA 4 · Sulawesi

R = "#C8483A"  # flag red, chillies, lanterns


def flag(x, y, h=120, w=70):
    """The red-and-white flag on a pole."""
    return line(x, y, x, y - h, C, 5) + rect(x, y - h, w, w * 0.32, R, 0, C, 3) + rect(x, y - h + w * 0.32, w, w * 0.32, P, 0, C, 3)


def fish(x, y, s=1.0, color=O, rot=0):
    body = ellipse(0, 0, 30, 14, color, C, 3) + path("M-28 0 L-48 -14 L-48 14Z", color, C, 3) + circle(16, -3, 3, C)
    return group(body, x, y, s, rot)


def coral(x, y, s=1.0, color=K):
    z = []
    for ang in (-50, -20, 10, 40):
        rad = math.radians(ang - 90)
        ex, ey = x + math.cos(rad) * 70 * s, y + math.sin(rad) * 80 * s
        z.append(path(f"M{x} {y} Q{(x + ex) / 2 - 10 * s} {(y + ey) / 2} {ex} {ey}", stroke=color, sw=12 * s))
    return "".join(z)


def underwater(top=Y0):
    z = [rect(X0, top, W, Y1 - top, T2)]
    z.append(path(f"M126 {top + 260} Q500 {top + 220} 898 {top + 280} L898 972 L126 972Z", T3, op=0.7))
    for k in range(5):
        x = 180 + k * 160
        z.append(path(f"M{x} {top} L{x + 70} {top} L{x + 150} {top + 420} L{x + 110} {top + 420}Z", P, op=0.08))
    return "".join(z)


def pinisi(x, y, s=1.0):
    """A pinisi: a high-sterned wooden hull, two masts, gaff sails and jibs."""
    z = [path(f"M{x - 240 * s} {y - 70 * s} Q{x - 220 * s} {y} {x - 120 * s} {y + 20 * s} L{x + 150 * s} {y + 20 * s} Q{x + 230 * s} {y} {x + 290 * s} {y - 50 * s} L{x + 200 * s} {y - 40 * s} L{x - 190 * s} {y - 40 * s}Z", D, C, 7)]
    z.append(path(f"M{x - 230 * s} {y - 52 * s} L{x + 270 * s} {y - 44 * s}", stroke=P, sw=6 * s))
    z.append(rect(x - 230 * s, y - 110 * s, 90 * s, 60 * s, D, 4, C, 5))  # stern house
    for mx, h in ((x - 70 * s, 420), (x + 90 * s, 380)):
        z.append(line(mx, y - 40 * s, mx, y - (40 + h) * s, C, 8 * s))
        z.append(path(f"M{mx + 6 * s} {y - (60 + h * 0.85) * s} L{mx + 150 * s} {y - (40 + h * 0.62) * s} L{mx + 6 * s} {y - 70 * s}Z", P, C, 5))
    z.append(line(x + 90 * s, y - 420 * s, x + 380 * s, y - 50 * s, C, 4 * s))  # bowsprit stay
    for k, (a, b) in enumerate(((0.15, 0.55), (0.35, 0.75), (0.55, 0.95))):
        x1, y1 = x + (90 + 290 * a) * s, y - (420 - 370 * a) * s
        x2, y2 = x + (90 + 290 * b) * s, y - (420 - 370 * b) * s
        z.append(path(f"M{x1} {y1} L{x2} {y2} L{x + (90 + 40 * k) * s} {y2 + 10 * s}Z", P, C, 5))
    z.append(line(x + 250 * s, y - 40 * s, x + 380 * s, y - 50 * s, C, 6 * s))
    return "".join(z)


@scene("bipa4.u01")
def kapal_pinisi():
    """Pinisi schooners moored at Makassar's old harbour, Paotere."""
    z = [sky(sun=(250, 300), r=80), sea(760)]
    z.append(pinisi(500, 820, 0.95))
    z.append(group(pinisi(0, 0, 1), 780, 760, 0.45))
    z.append(path("M126 960 L898 960", stroke=D, sw=40) + "".join(line(160 + k * 120, 940, 160 + k * 120, 972, C, 5) for k in range(7)))
    return "".join(z)


def karst(x, y, w, h, color=G):
    return path(f"M{x - w / 2} {y} Q{x - w * 0.55} {y - h * 0.6} {x - w * 0.3} {y - h} Q{x} {y - h * 1.08} {x + w * 0.3} {y - h} Q{x + w * 0.58} {y - h * 0.5} {x + w / 2} {y}Z", color, C, 6)


@scene("bipa4.u02")
def rammang_rammang():
    """The karst towers of Rammang-Rammang, a river and a narrow boat."""
    z = [sky(sun=(700, 260), r=80)]
    z.append(karst(250, 640, 240, 300, "#6E7FA8") + karst(500, 640, 200, 360, "#6E7FA8") + karst(760, 640, 260, 280, "#6E7FA8"))
    z.append(karst(180, 720, 180, 260) + karst(840, 720, 200, 300) + karst(560, 720, 160, 200))
    for k in range(6):
        z.append(path(f"M{230 + k * 110} {470 + (k % 3) * 40} q-10 60 0 120", stroke=C, sw=3, op=0.25))
    z.append(rect(X0, 700, W, 120, G))
    z.append(path("M126 820 Q500 780 898 830 L898 972 L126 972Z", T2))
    z.append(path("M126 860 Q500 830 898 870", stroke=P, sw=5, op=0.4))
    z.append(path("M380 900 Q500 930 640 895 L620 915 Q500 945 395 918Z", D, C, 5))
    z.append(person(520, 912, 0.55, O, scarf=I))
    return "".join(z)


@scene("bipa4.u04")
def habibie():
    """Parepare: Habibie and Ainun on their bench by the bay, a plane above."""
    z = [sky(sun=(720, 300), r=86), sea(640)]
    z.append(path("M126 760 L898 760 L898 972 L126 972Z", S))
    for k in range(8):
        z.append(line(126 + k * 110, 760, 126 + k * 110, 972, C, 2, 0.15))
    z.append(path("M126 760 L898 760", stroke=C, sw=5))
    # the bench and the two seated figures, seen from behind looking at the sea
    z.append(rect(360, 800, 300, 22, D, 4, C, 5) + rect(360, 740, 300, 18, D, 4, C, 5))
    for x in (380, 640):
        z.append(line(x, 822, x, 880, C, 8))
    z.append(path("M430 800 L440 740 Q470 720 500 740 L510 800Z", I, C, 5) + circle(470, 710, 26, C, C, 4))
    z.append(path("M520 800 L530 744 Q560 724 590 744 L600 800Z", K, C, 5) + circle(560, 714, 26, P, C, 4))
    # a small propeller airliner (Habibie built them)
    z.append(group(path("M-90 0 Q-60 -14 70 -10 Q100 -6 110 0 Q100 8 70 10 L-80 10Z", P, C, 4) + path("M-10 -6 L-40 -60 L0 -60 L30 -6Z", P, C, 4) + path("M-80 0 L-100 -36 L-80 -36 L-60 -4Z", P, C, 4), 380, 300, 1.1, -6))
    return "".join(z)


@scene("bipa4.u05")
def tujuh_belasan():
    """Agustusan games: panjat pinang, a sack race, red-and-white flags."""
    z = [sky(sun=(780, 240), r=70)]
    z.append(path("M126 300 Q512 360 898 290", stroke=C, sw=3))
    for k in range(12):
        x = 160 + k * 64
        y = 300 + 60 * math.sin(math.pi * (x - 126) / 772) - (2 if k % 2 else 0)
        z.append(poly([(x - 18, y), (x + 18, y), (x, y + 34)], R if k % 2 == 0 else P, C, 3))
    z.append(ground(800, G, 1))
    # the greased pole with prizes at the top
    z.append(rect(420, 380, 30, 440, D, 6, C, 5))
    z.append(ellipse(435, 380, 110, 22, "none", C, 5))
    for k, col in enumerate((O, K, I, G, P)):
        z.append(rect(335 + k * 40, 400, 26, 34, col, 4, C, 3) + line(348 + k * 40, 380, 348 + k * 40, 400, C, 2))
    z.append(person(445, 760, 0.7, R, scarf=None) + person(435, 830, 0.8, I))
    # sack race
    for k, x in enumerate((620, 740)):
        z.append(person(x, 880 - k * 10, 0.8, O))
        z.append(path(f"M{x - 34} {880 - k * 10} L{x - 30} {800 - k * 10} L{x + 30} {800 - k * 10} L{x + 34} {880 - k * 10}Z", S, C, 5))
    z.append(flag(220, 900, 260, 110))
    return "".join(z)


@scene("bipa4.u06")
def lore_lindu():
    """A Lore Lindu megalith standing in the grass of the Bada valley."""
    z = [sky(sun=(250, 280), r=76), hills(True)]
    z.append(path("M126 760 Q500 720 898 770 L898 972 L126 972Z", "#7FA65A"))
    # the stone figure: rounded body, big round head, simple carved features
    z.append(path("M410 900 Q380 640 430 560 L590 560 Q640 640 610 900Z", "#9C9488", C, 7))
    z.append(ellipse(510, 520, 110, 120, "#9C9488", C, 7))
    z.append(path("M455 490 q15 -12 30 0 M535 490 q15 -12 30 0", stroke=C, sw=5, op=0.7))
    z.append(path("M510 500 L500 560 L520 560", stroke=C, sw=4, op=0.6))
    z.append(path("M470 680 Q510 720 550 680", stroke=C, sw=4, op=0.5))
    z.append(path("M440 620 Q510 660 580 620", stroke=C, sw=4, op=0.5))
    z.append(ellipse(510, 905, 160, 22, C, op=0.18))
    for k in range(14):
        x = 150 + k * 55
        z.append(path(f"M{x} 960 q6 -40 14 -60 M{x + 10} 960 q-4 -30 -12 -46", stroke="#3F7A33", sw=5))
    z.append(person(760, 880, 0.75, O, scarf=I))
    return "".join(z)


def whale_shark(x, y, s=1.0):
    body = path("M-260 0 Q-200 -80 60 -70 Q200 -60 250 0 Q200 60 60 70 Q-200 80 -260 0Z", "#4E6A8C", C, 6)
    body += path("M-250 -4 Q-60 40 240 6", stroke="#D9DDE0", sw=0, op=0)
    body += path("M-240 10 Q-60 70 240 10 Q200 60 60 70 Q-200 80 -240 10Z", "#C9D2D8", op=0.9)
    body += path("M-20 -66 L40 -150 L80 -62Z", "#4E6A8C", C, 6)
    body += path("M-250 0 L-340 -90 L-320 0 L-340 80Z", "#4E6A8C", C, 6)
    body += path("M40 40 L0 120 L100 50Z", "#4E6A8C", C, 5)
    body += path("M190 -20 L190 20 M170 -26 L170 24 M150 -30 L150 28", stroke=C, sw=3, op=0.5)
    r = random.Random(7)
    for _ in range(46):
        px, py = r.uniform(-220, 140), r.uniform(-55, 25)
        body += circle(px, py, r.uniform(4, 7), P, op=0.85)
    body += path("M246 -2 Q230 10 210 8", stroke=C, sw=4)
    return group(body, x, y, s)


@scene("bipa4.u07")
def hiu_paus():
    """Whale sharks of Botubarani: a gentle giant spotted in blue water."""
    z = [underwater()]
    z.append(whale_shark(520, 560, 1.15))
    for k, (x, y) in enumerate(((240, 330), (300, 380), (760, 820), (700, 860), (820, 300))):
        z.append(fish(x, y, 0.7, (O, P, K)[k % 3]))
    z.append(path("M126 930 Q400 880 898 940 L898 972 L126 972Z", S))
    z.append(coral(250, 950, 0.8, K) + coral(780, 950, 0.7, O))
    for x, y in ((380, 300), (620, 260), (690, 340)):
        z.append(circle(x, y, 8, P, op=0.6))
    return "".join(z)


@scene("bipa4.u08")
def toar_lumimuut():
    """Toar and Lumimuut: the mother born from a rock, her son, the dawn."""
    z = [sky(sun=(512, 470), r=120)]
    z.append(path("M126 640 Q300 470 512 560 Q700 450 898 600 L898 972 L126 972Z", I, op=0.35))
    z.append(path("M126 760 Q330 680 512 720 Q720 660 898 740 L898 972 L126 972Z", G))
    # the rock on the hill, split open
    z.append(path("M380 760 Q370 620 470 600 L500 760Z", "#9C9488", C, 7) + path("M530 760 L550 600 Q660 620 650 760Z", "#9C9488", C, 7))
    z.append(path("M400 680 Q430 660 450 690 M590 650 Q610 640 630 660", stroke=C, sw=3, op=0.5))
    z.append(person(470, 880, 1.1, K, scarf=C) + person(560, 880, 0.85, I))
    for k in range(7):
        a = math.radians(-160 + k * 23)
        z.append(line(512 + math.cos(a) * 150, 470 + math.sin(a) * 150, 512 + math.cos(a) * 220, 470 + math.sin(a) * 220, P, 6, 0.5))
    return "".join(z)


@scene("bipa4.u09")
def bunaken():
    """Bunaken's coral walls under the sea, Manado Tua's cone above it."""
    z = [sky(sun=(250, 230), r=60)]
    z.append(path("M480 420 L640 210 Q680 196 720 210 L898 420Z", G, C, 6))
    z.append(rect(X0, 410, W, 20, T1))
    z.append(underwater(420))
    # the wall drops on the left, crowded with coral
    z.append(path("M126 420 L370 420 Q360 600 400 700 Q370 840 420 972 L126 972Z", "#6E5A50", C, 6))
    for k, (x, y, col) in enumerate(((330, 480, K), (360, 600, O), (380, 720, K), (370, 840, O), (300, 560, G), (320, 900, K))):
        z.append(coral(x, y, 0.6, col))
    z.append(ellipse(280, 680, 40, 30, "#B5654A", C, 4) + ellipse(250, 790, 34, 26, G, C, 4))
    # a diver and a school of fish
    z.append(group(ellipse(0, 0, 70, 22, I, C, 5) + circle(80, -4, 20, C) + rect(-30, -34, 60, 16, O, 6, C, 4) + path("M-70 0 L-120 -20 L-120 20Z", C), 640, 640, 1, -10))
    for k in range(9):
        z.append(fish(560 + (k % 3) * 70, 800 + (k // 3) * 40, 0.55, O if k % 2 else P))
    for k in range(4):
        z.append(circle(700 + k * 14, 600 - k * 50, 7 - k, P, op=0.7))
    return "".join(z)


def flower(x, y, r=22, color=K, centre=O):
    z = [circle(x + math.cos(math.radians(a)) * r * 0.8, y + math.sin(math.radians(a)) * r * 0.8, r * 0.55, color, C, 3) for a in range(0, 360, 72)]
    return "".join(z) + circle(x, y, r * 0.38, centre, C, 3)


@scene("bipa4.u10")
def tomohon():
    """Tomohon's flower festival: a float covered in flowers below Mount Lokon."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(volcano(620, 640, 560, 330, I, 0.45))
    z.append(path("M126 680 Q500 640 898 690 L898 972 L126 972Z", "#7FA65A"))
    z.append(rect(X0, 860, W, 112, S))
    # the float: a flower-covered bird on a wagon
    z.append(rect(270, 760, 480, 100, G, 16, C, 6))
    for k in range(4):
        z.append(circle(330 + k * 120, 870, 26, C) + circle(330 + k * 120, 870, 10, S))
    z.append(path("M330 760 Q300 600 420 560 Q520 520 600 580 Q700 520 760 460 Q740 580 690 640 Q640 720 620 760Z", K, C, 6))
    z.append(path("M420 560 Q380 480 430 440 Q470 470 450 560Z", O, C, 5))
    r = random.Random(11)
    for _ in range(40):
        x, y = r.uniform(340, 700), r.uniform(600, 750)
        z.append(flower(x, y, r.uniform(12, 18), r.choice([P, O, R, K]), r.choice([O, I])))
    for k in range(6):
        z.append(flower(180 + k * 130, 940, 24, (O, R, P, K)[k % 4], I))
    z.append(person(200, 860, 0.8, R, scarf=O) + person(830, 860, 0.8, I))
    return "".join(z)


# ---------------------------------------------------------------- BIPA 5 · Kalimantan


def rainforest(y=700, op=1.0):
    z = []
    for k in range(12):
        x = 126 + k * 72
        h = 120 + (k * 53) % 90
        z.append(ellipse(x, y - h * 0.5, 70, h * 0.6, "#3F7A33" if k % 2 else G, C, 4, op))
    return "".join(z)


@scene("bipa5.u01")
def balikpapan():
    """Balikpapan, the oil city: refinery towers and tanks on the bay."""
    z = [sky(sun=(260, 280), r=80), sea(760)]
    z.append(rect(X0, 640, W, 120, S))
    for x, h in ((300, 320), (380, 260), (640, 360)):
        z.append(rect(x - 22, 640 - h, 44, h, P, 4, C, 5))
        for k in range(1, 5):
            z.append(line(x - 22, 640 - h * k / 5, x + 22, 640 - h * k / 5, C, 3, 0.6))
    z.append(line(760, 640, 760, 300, C, 8) + path("M760 300 q-20 -40 0 -80 q20 40 0 80Z", O, C, 4))
    for x in (190, 480, 830):
        z.append(rect(x - 60, 560, 120, 80, P, 6, C, 5) + ellipse(x, 560, 60, 14, P, C, 5) + line(x - 60, 600, x + 60, 600, R, 6))
    z.append(group(ferry(0, 0, 1), 560, 880, 0.6))
    return "".join(z)


@scene("bipa5.u02")
def nusantara():
    """Nusantara, the new capital: the Garuda palace rising from the forest."""
    z = [sky(sun=(512, 300), r=90)]
    z.append(rainforest(760))
    z.append(rect(X0, 760, W, 212, "#3F7A33"))
    # the palace: a long low building and the fanned blades of the garuda
    z.append(rect(270, 680, 484, 80, P, 4, C, 6) + windows(300, 700, 8, 1, 34, 40, 22, 0, I))
    for k in range(-7, 8):
        a = math.radians(k * 11 - 90)
        r1, r2 = 60, 260 - abs(k) * 14
        z.append(path(f"M{512 + math.cos(a) * r1} {670 + math.sin(a) * r1} L{512 + math.cos(a) * r2} {670 + math.sin(a) * r2}", stroke=O, sw=16))
        z.append(path(f"M{512 + math.cos(a) * r1} {670 + math.sin(a) * r1} L{512 + math.cos(a) * r2} {670 + math.sin(a) * r2}", stroke=C, sw=2, op=0.5))
    z.append(path("M462 680 Q512 600 562 680Z", O, C, 5))
    z.append(path("M126 900 Q512 860 898 900", stroke=S, sw=60))
    z.append(flag(800, 880, 200, 90))
    return "".join(z)


@scene("bipa5.u03")
def mahakam():
    """The Mahakam river: a long river boat and a pesut dolphin surfacing."""
    z = [sky(sun=(720, 270), r=80)]
    z.append(rainforest(640, 0.9))
    z.append(path("M126 620 Q512 600 898 630 L898 972 L126 972Z", "#8C7A5A"))
    z.append(path("M126 700 Q512 680 898 710", stroke=P, sw=4, op=0.3) + path("M126 820 Q512 790 898 830", stroke=P, sw=4, op=0.3))
    z.append(group(path("M-200 0 L220 0 L190 40 L-180 40Z", P, C, 6) + rect(-150, -70, 280, 70, P, 4, C, 6) + windows(-130, -56, 6, 1, 28, 30, 18, 0, I) + path("M-170 -70 L150 -70 L130 -100 L-150 -100Z", G, C, 6), 420, 700, 0.9))
    z.append(group(path("M-90 10 Q-50 -60 40 -50 Q90 -40 100 -6 Q60 -10 40 0 Q-20 20 -60 30Z", "#8B95A8", C, 5) + path("M-10 -54 L0 -80 L20 -52Z", "#8B95A8", C, 4) + circle(70, -30, 5, C), 650, 870, 1.0))
    z.append(path("M520 900 Q650 880 780 900", stroke=P, sw=5, op=0.6))
    return "".join(z)


@scene("bipa5.u04")
def kutai():
    """The Kutai Kartanegara palace at Tenggarong, white above its lawn."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rainforest(620, 0.6))
    z.append(rect(X0, 760, W, 212, G))
    z.append(rect(220, 540, 584, 220, P, 4, C, 7))
    z.append(rect(400, 440, 224, 100, P, 4, C, 7) + path("M380 440 L644 440 L600 380 L424 380Z", "#C8C2B4", C, 6))
    z.append(path("M200 540 L824 540 L780 490 L244 490Z", "#C8C2B4", C, 6))
    for k in range(8):
        z.append(rect(250 + k * 72, 580, 16, 180, P, 2, C, 4))
    z.append(windows(440, 460, 3, 1, 30, 50, 37, 0, I, arch=True))
    z.append(rect(470, 640, 84, 120, I, 4, C, 5))
    z.append(rect(420, 760, 184, 212, S) + line(420, 760, 420, 972, C, 3, 0.4) + line(604, 760, 604, 972, C, 3, 0.4))
    z.append(tree(170, 820, 0.7) + tree(860, 820, 0.7))
    return "".join(z)


def orangutan(x, y, s=1.0):
    fur = "#B8582E"
    b = path("M-60 0 Q-80 -110 0 -130 Q80 -110 60 0 Q30 40 0 40 Q-30 40 -60 0Z", fur, C, 6)
    b += path("M-50 -100 Q-110 -200 -90 -300", stroke=fur, sw=34) + path("M-50 -100 Q-110 -200 -90 -300", stroke=C, sw=3, op=0.4)
    b += path("M50 -100 Q120 -60 130 20", stroke=fur, sw=30)
    b += path("M-40 30 Q-60 80 -30 110 M40 30 Q60 80 30 110", stroke=fur, sw=28)
    b += ellipse(0, -160, 48, 44, fur, C, 6) + ellipse(0, -150, 30, 26, "#E0A27A", C, 3)
    b += circle(-10, -160, 4, C) + circle(10, -160, 4, C)
    return group(b, x, y, s)


@scene("bipa5.u05")
def orangutan_camp():
    """Camp Leakey: an orangutan swinging down to the feeding platform."""
    z = [rect(X0, Y0, W, H, "#3F7A33")]
    z.append(rainforest(400, 0.7) + rainforest(560))
    z.append(path("M180 122 Q200 400 160 972", stroke=D, sw=60) + path("M180 122 Q200 400 160 972", stroke=C, sw=4, op=0.4))
    z.append(path("M126 230 Q400 260 898 200", stroke=D, sw=14))
    z.append(line(340, 245, 330, 520, C, 4))
    z.append(orangutan(500, 640, 1.2))
    z.append(rect(300, 800, 460, 30, D, 4, C, 5) + line(340, 830, 340, 972, C, 10) + line(720, 830, 720, 972, C, 10))
    for k in range(3):
        z.append(path(f"M{380 + k * 40} 800 q10 -40 30 -44", stroke="#F2C94C", sw=12))
    return "".join(z)


def hornbill(x, y, s=1.0, rot=0):
    b = path("M-120 0 Q-40 -50 60 -20 Q20 20 -120 0Z", C, C, 4)
    b += path("M-60 -10 L-180 -90 L-150 -10 L-190 30 L-60 10Z", C, C, 4) + path("M-170 -70 L-160 -40", stroke=P, sw=6)
    b += path("M-60 -10 L-200 -40 L-60 10", P, C, 3)
    b += circle(70, -30, 26, C) + path("M90 -36 Q160 -30 190 10 Q140 -10 92 -18Z", O, C, 4) + path("M88 -54 Q130 -66 150 -40 Q120 -40 90 -42Z", R, C, 3)
    return group(b, x, y, s, rot)


@scene("bipa5.u06")
def tari_enggang():
    """The Dayak hornbill dance: a dancer with fans of hornbill feathers."""
    z = [sky(sun=(512, 330), r=110), rainforest(720, 0.6)]
    z.append(rect(X0, 760, W, 212, D))
    for k in range(8):
        z.append(line(126 + k * 110, 760, 126 + k * 110, 972, C, 3, 0.3))
    z.append(hornbill(640, 280, 0.9, -8))
    # the dancer: beaded vest, feather headdress, a fan of feathers in each hand
    x, y = 512, 900
    z.append(path(f"M{x - 70} {y} L{x - 50} {y - 230} Q{x} {y - 270} {x + 50} {y - 230} L{x + 70} {y}Z", I, C, 6))
    for k in range(4):
        z.append(path(f"M{x - 50} {y - 200 + k * 40} L{x + 50} {y - 200 + k * 40}", stroke=O, sw=5, dash="8 8"))
    z.append(circle(x, y - 300, 40, C, C, 4))
    for k in range(5):
        a = math.radians(-130 + k * 20)
        z.append(path(f"M{x} {y - 320} L{x + math.cos(a) * 130} {y - 320 + math.sin(a) * 130}", stroke=P, sw=12) + path(f"M{x + math.cos(a) * 95} {y - 320 + math.sin(a) * 95} L{x + math.cos(a) * 130} {y - 320 + math.sin(a) * 130}", stroke=C, sw=12))
    for side in (-1, 1):
        hx, hy = x + side * 200, y - 280
        z.append(path(f"M{x + side * 50} {y - 220} Q{x + side * 130} {y - 240} {hx} {hy}", stroke=I, sw=22))
        for k in range(5):
            a = math.radians(-90 + side * (-40 + k * 20))
            z.append(path(f"M{hx} {hy} L{hx + math.cos(a) * 150} {hy + math.sin(a) * 150}", stroke=P, sw=16) + path(f"M{hx + math.cos(a) * 110} {hy + math.sin(a) * 110} L{hx + math.cos(a) * 150} {hy + math.sin(a) * 150}", stroke=C, sw=16))
    return "".join(z)


def jukung(x, y, s=1.0, goods=(O, G, "#F2C94C")):
    z = [path(f"M{x - 130 * s} {y - 20 * s} Q{x} {y + 30 * s} {x + 140 * s} {y - 26 * s} L{x + 110 * s} {y + 6 * s} Q{x} {y + 30 * s} {x - 100 * s} {y + 6 * s}Z", D, C, 5)]
    for k in range(4):
        z.append(circle(x - 70 * s + k * 34 * s, y - 14 * s, 14 * s, goods[k % len(goods)], C, 3))
    z.append(person(x + 60 * s, y - 6 * s, 0.7 * s, I))
    z.append(ellipse(x + 60 * s, y - 92 * s, 50 * s, 14 * s, S, C, 4))
    return "".join(z)


@scene("bipa5.u07")
def lok_baintan():
    """Lok Baintan floating market: wooden boats heaped with fruit at dawn."""
    z = [sky(sun=(700, 300), r=90)]
    z.append(rainforest(560, 0.6))
    z.append(path("M126 560 Q512 540 898 570 L898 972 L126 972Z", "#8C7A5A"))
    for k in range(4):
        z.append(path(f"M126 {620 + k * 90} Q512 {600 + k * 90} 898 {630 + k * 90}", stroke=P, sw=4, op=0.25))
    z.append(jukung(300, 650, 0.8) + jukung(720, 700, 0.85, (K, O, P)))
    z.append(jukung(360, 840, 1.05, ("#F2C94C", G, O)) + jukung(720, 900, 0.9, (O, R, G)))
    return "".join(z)


def gem(x, y, r, color):
    pts = [(x, y - r), (x + r, y - r * 0.3), (x + r * 0.6, y + r), (x - r * 0.6, y + r), (x - r, y - r * 0.3)]
    return poly(pts, color, C, 4) + path(f"M{x - r} {y - r * 0.3} L{x + r} {y - r * 0.3} M{x} {y - r} L{x - r * 0.3} {y - r * 0.3} L{x} {y + r} L{x + r * 0.3} {y - r * 0.3} Z", stroke=C, sw=2, op=0.5) + path(f"M{x - r * 0.4} {y - r * 0.6} L{x - r * 0.1} {y - r * 0.8}", stroke=P, sw=4, op=0.8)


@scene("bipa5.u08")
def martapura():
    """The Martapura gem market: a tray of cut stones and rings."""
    z = [rect(X0, Y0, W, H, "#B5654A")]
    z.append(rect(X0, Y0, W, 260, I, op=0.4))
    for k in range(4):
        z.append(lantern(220 + k * 190, 220, 1.0))
    z.append(rect(180, 360, 664, 520, D, 24, C, 7) + rect(210, 390, 604, 460, I, 16, C, 5))
    cols = ["#5BA1D6", R, G, P, "#9D6CC2", O]
    for r_ in range(3):
        for c in range(4):
            cx, cy = 290 + c * 150, 470 + r_ * 140
            z.append(gem(cx, cy, 38 - (r_ + c) % 3 * 6, cols[(r_ * 4 + c) % len(cols)]))
    z.append(ellipse(512, 930, 120, 22, C, op=0.2))
    z.append(circle(780, 920, 40, "none", O, 12) + gem(780, 870, 24, "#5BA1D6"))
    return "".join(z)


@scene("bipa5.u09")
def khatulistiwa():
    """The Equator Monument: four ironwood pillars holding a globe and arrow."""
    z = [sky(sun=(512, 260), r=70)]
    z.append(rect(X0, 820, W, 152, G) + path("M126 880 L898 880", stroke=R, sw=8, dash="30 20"))
    for x in (380, 450, 574, 644):
        z.append(rect(x - 16, 470, 32, 350, D, 4, C, 5))
    z.append(rect(360, 460, 304, 24, D, 4, C, 5))
    z.append(circle(512, 380, 90, "none", C, 8))
    z.append(ellipse(512, 380, 90, 30, "none", C, 6) + line(512, 290, 512, 470, C, 6))
    z.append(path("M400 300 L640 460", stroke=O, sw=10) + poly([(640, 460), (612, 458), (628, 434)], O, C, 3))
    z.append(path("M380 370 L644 390", stroke=C, sw=6))
    z.append(rect(330, 800, 364, 30, S, 4, C, 5))
    z.append(person(270, 880, 0.8, O) + person(760, 880, 0.8, I, scarf=K))
    return "".join(z)


def lion_head(x, y, s=1.0):
    b = ellipse(0, 0, 70, 56, R, C, 6) + path("M-60 -30 Q0 -90 60 -30", stroke=O, sw=10)
    b += circle(-26, -10, 12, P, C, 3) + circle(26, -10, 12, P, C, 3) + circle(-26, -10, 5, C) + circle(26, -10, 5, C)
    b += path("M-40 24 Q0 50 40 24", P, C, 4)
    b += path("M-40 -44 L-70 -100 L-26 -54 M40 -44 L70 -100 L26 -54", O, C, 4)
    b += path("M60 10 Q110 0 130 30 M60 20 Q100 30 116 60", stroke=O, sw=6)
    return group(b, x, y, s)


@scene("bipa5.u10")
def cap_go_meh():
    """Cap Go Meh in Singkawang: a dragon dance under rows of red lanterns."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(circle(760, 240, 60, P, op=0.6))
    for row, y in enumerate((230, 330)):
        z.append(path(f"M126 {y - 40} Q512 {y + 20} 898 {y - 40}", stroke=C, sw=3))
        for k in range(7):
            x = 170 + k * 115 + row * 55
            z.append(lantern(x, y - 40 + 60 * math.sin(math.pi * (x - 126) / 772) * 0.4 + 30, 0.9))
    z.append(rect(X0, 820, W, 152, "#5A4A60"))
    # the dragon: a wavy body of segments carried on poles
    pts = [(200 + k * 70, 600 + 70 * math.sin(k * 0.9)) for k in range(9)]
    for k, (x, y) in enumerate(pts):
        z.append(line(x, y, x, 860, C, 5))
        z.append(person(x, 900, 0.6, R if k % 2 else O))
    for k, (x, y) in enumerate(pts[:-1]):
        z.append(path(f"M{x - 30} {y - 30} l10 -26 l10 22 l10 -26 l10 22 l10 -26 l10 30", P, C, 3))
        z.append(ellipse(x, y, 56, 40, R, C, 5) + path(f"M{x - 50} {y + 14} Q{x} {y + 30} {x + 50} {y + 14}", stroke=O, sw=8))
        z.append(path(f"M{x - 20} {y - 10} q10 -10 20 0 q10 -10 20 0", stroke=C, sw=3, op=0.5))
    z.append(lion_head(pts[-1][0] + 20, pts[-1][1] - 10, 1.1))
    return "".join(z)


# ---------------------------------------------------------------- BIPA 6 · Sumatra

BRICK = "#B5654A"


@scene("bipa6.u01")
def tjong_a_fie():
    """The Tjong A Fie mansion in Medan: a two-storey house behind a Chinese gate."""
    z = [sky(sun=(760, 260), r=70)]
    z.append(rect(X0, 800, W, 172, S))
    z.append(rect(200, 420, 624, 380, "#8DB58A", 4, C, 7))
    z.append(path("M180 420 L844 420 L800 360 L224 360Z", G, C, 6))
    z.append(windows(240, 460, 6, 2, 50, 90, 46, 50, P, arch=True, shutter=True))
    z.append(rect(180, 740, 664, 60, P, 4, C, 6))
    # the gate: a curved roof with upswept ends
    z.append(rect(390, 640, 40, 200, R, 3, C, 5) + rect(594, 640, 40, 200, R, 3, C, 5))
    z.append(path("M340 650 Q360 620 380 625 L644 625 Q664 620 684 650 Q680 590 640 590 L384 590 Q344 590 340 650Z", G, C, 6))
    z.append(path("M330 640 Q300 600 320 580 M694 640 Q724 600 704 580", stroke=C, sw=6))
    z.append(rect(430, 660, 164, 180, C, 4) + lantern(470, 700, 0.8) + lantern(554, 700, 0.8))
    return "".join(z)


@scene("bipa6.u02")
def pantun_melayu():
    """A Malay wedding: the couple on the dais, palm-flower poles, pantun said aloud."""
    z = [rect(X0, Y0, W, H, "#7A3A4A")]
    for k in range(9):
        z.append(path(f"M{126 + k * 96} 122 Q{174 + k * 96} 260 {222 + k * 96} 122", O, C, 3, 0.85))
    z.append(rect(X0, 800, W, 172, R))
    z.append(rect(220, 760, 584, 50, O, 6, C, 6))
    z.append(path("M300 760 L300 420 Q512 320 724 420 L724 760Z", O, C, 7) + path("M340 760 L340 450 Q512 370 684 450 L684 760Z", "#7A3A4A", C, 5))
    z.append(rect(370, 640, 280, 70, R, 10, C, 5))
    z.append(person(450, 700, 1.05, O, head=C) + person(574, 700, 1.05, O, scarf=P))
    for x in (200, 824):
        z.append(line(x, 900, x, 420, C, 6))
        for k in range(10):
            a = math.radians(-90 + (k - 4.5) * 18)
            z.append(path(f"M{x} 440 Q{x + math.cos(a) * 50} {440 + math.sin(a) * 40} {x + math.cos(a) * 90} {470 + math.sin(a) * 80 + 40}", stroke=(O, P)[k % 2], sw=6))
    z.append(group(path("M0 0 Q30 -20 60 0 Q30 14 0 0Z", P, C, 3), 470, 880, 1) + group(path("M0 0 Q30 -20 60 0 Q30 14 0 0Z", P, C, 3), 520, 900, -1))
    return "".join(z)


def sriwijaya_ship(x, y, s=1.0):
    b = path("M-200 -20 Q0 40 220 -30 L190 10 Q0 50 -180 10Z", D, C, 6)
    b += line(-240, 20, 240, 20, C, 5) + line(-140, -4, -140, 20, C, 4) + line(140, -4, 140, 20, C, 4)
    for mx, h in ((-60, 220), (70, 200)):
        b += line(mx, 0, mx, -h, C, 7)
        b += path(f"M{mx - 70} {-h * 0.95} L{mx + 80} {-h + 10} L{mx + 60} {-20} L{mx - 90} {-30}Z", "#C9A97A", C, 5)
        b += "".join(line(mx - 80 + k * 4, -h * 0.95 + k * h * 0.22, mx + 70 + k * 2, -h + 10 + k * h * 0.22, C, 2, 0.4) for k in range(1, 4))
    return group(b, x, y, s)


@scene("bipa6.u04")
def musi():
    """The Musi river at Palembang: Ampera bridge, and a Sriwijaya ship of old."""
    z = [sky(sun=(250, 300), r=80)]
    z.append(city_far(560, 0.3))
    z.append(rect(X0, 560, W, 412, "#7C8E7A"))
    for k in range(5):
        z.append(path(f"M{170 + k * 150} {640 + k * 60} q30 -10 60 0", stroke=P, sw=5, op=0.4))
    # Ampera bridge: two red towers, a deck and lifting cables
    z.append(rect(X0, 520, W, 26, R, 0, C, 5))
    for tx in (380, 680):
        z.append(rect(tx - 24, 300, 48, 300, R, 4, C, 6) + rect(tx - 40, 290, 80, 20, R, 3, C, 5))
    z.append(line(380, 330, 680, 330, C, 6) + line(380, 360, 680, 360, C, 4))
    for tx, side in ((380, -1), (680, 1)):
        for k in range(5):
            z.append(line(tx, 320, tx + side * (60 + k * 50), 520, C, 3))
    for k in range(10):
        z.append(line(126 + k * 86, 546, 126 + k * 86, 600, C, 8))
    z.append(sriwijaya_ship(520, 850, 1.0))
    return "".join(z)


@scene("bipa6.u05")
def baiturrahman():
    """Baiturrahman Grand Mosque: white walls, black domes, the long pool."""
    z = [sky(sun=(780, 240), r=60)]
    z.append(rect(X0, 640, W, 332, S))
    z.append(rect(200, 470, 624, 170, P, 4, C, 7))
    z.append(windows(230, 520, 9, 1, 40, 90, 24, 0, I, arch=True))
    for cx, r in ((512, 110), (300, 70), (724, 70)):
        z.append(path(f"M{cx - r} 470 Q{cx - r} {470 - r * 1.6} {cx} {470 - r * 1.8} Q{cx + r} {470 - r * 1.6} {cx + r} 470Z", C, C, 5) + line(cx, 470 - r * 1.8, cx, 440 - r * 1.8, C, 4) + circle(cx, 436 - r * 1.8, 6, O))
    for mx in (160, 864):
        z.append(rect(mx - 18, 300, 36, 340, P, 3, C, 5) + path(f"M{mx - 22} 300 Q{mx} 250 {mx + 22} 300Z", C, C, 4))
    z.append(path("M380 660 L644 660 L760 972 L264 972Z", T1, C, 5))
    z.append(path("M480 700 L544 700 L580 900 L444 900Z", P, op=0.35))
    for x in (230, 800):
        z.append(line(x, 920, x, 780, C, 5) + path(f"M{x - 60} 790 Q{x} 730 {x + 60} 790Z", P, C, 4))
    z.append(palm(170, 960, 200, 10, 0.7) + palm(860, 960, 200, -10, 0.7))
    return "".join(z)


def rumah_bolon(cx, y, s=1.0):
    b = rect(-120, -110, 240, 90, D, 4, C, 6)
    for k in range(4):
        b += rect(-120 + k * 62, -110, 52, 90, "none", 2, C, 2, op=0.5)
    b += path("M-100 -20 L-100 40 M-40 -20 L-40 40 M40 -20 L40 40 M100 -20 L100 40", stroke=C, sw=10)
    b += path("M-130 -110 L130 -110 L200 -280 Q0 -200 -200 -280Z", "#6E5A50", C, 7)
    b += path("M-110 -120 L110 -120 L0 -200Z", P, C, 4) + path("M-60 -130 L0 -170 L60 -130", stroke=R, sw=6)
    b += path("M-80 -80 l20 -16 l20 16 l20 -16 l20 16 l20 -16 l20 16 l20 -16 l20 16", stroke=P, sw=4)
    return group(b, cx, y, s)


@scene("bipa6.u06")
def samosir():
    """Samosir in Lake Toba: a Batak house with its saddle roof by the water."""
    z = [sky(sun=(700, 260), r=70)]
    z.append(path("M126 520 Q300 420 520 500 Q700 410 898 480 L898 600 L126 600Z", "#6E7FA8"))
    z.append(sea(560, (T2, T3, I)))
    z.append(path("M126 760 Q400 700 898 760 L898 972 L126 972Z", G))
    z.append(rumah_bolon(420, 880, 1.25))
    z.append(group(path("M-80 -10 Q0 20 90 -14 L70 10 Q0 30 -70 10Z", D, C, 4), 760, 700, 1))
    z.append(tree(780, 900, 0.7))
    return "".join(z)


@scene("bipa6.u07")
def muaro_jambi():
    """The brick temples of Muaro Jambi among the trees."""
    z = [sky(sun=(250, 270), r=70)]
    z.append(rainforest(640, 0.55))
    z.append(rect(X0, 760, W, 212, G))
    for k, (w, h) in enumerate(((520, 90), (420, 80), (320, 80), (220, 70))):
        y = 760 - sum(hh for _, hh in ((520, 90), (420, 80), (320, 80), (220, 70))[:k + 1])
        z.append(rect(512 - w / 2, y, w, h, BRICK, 4, C, 6))
        for c in range(int(w // 40)):
            z.append(line(512 - w / 2 + c * 40, y + h / 2, 512 - w / 2 + c * 40 + 20, y + h / 2, C, 2, 0.35))
    z.append(path("M432 440 Q512 340 592 440Z", BRICK, C, 6))
    z.append(rect(472, 680, 80, 80, C, 6))
    z.append(path("M462 760 L562 760 L600 972 L424 972Z", S))
    z.append(tree(190, 860, 0.8) + tree(840, 860, 0.8) + banyan(780, 760, 0.5))
    return "".join(z)


@scene("bipa6.u08")
def rafflesia():
    """Rafflesia arnoldii: the giant five-petalled flower on the forest floor."""
    z = [rect(X0, Y0, W, H, "#2F5E2A")]
    z.append(rainforest(500, 0.6))
    z.append(rect(X0, 620, W, 352, "#5A4632"))
    for k in range(12):
        z.append(leaf(140 + k * 70, 980, 0.9, -40 + (k % 5) * 20, G))
    cx, cy = 512, 700
    for k in range(5):
        a = math.radians(-90 + k * 72)
        px, py = cx + math.cos(a) * 140, cy + math.sin(a) * 105
        z.append(ellipse(px, py, 120, 92, "#B23A2E", C, 7, extra=f'transform="rotate({k * 72} {px} {py})"'))
        r = random.Random(k)
        for _ in range(9):
            z.append(circle(px + r.uniform(-60, 60), py + r.uniform(-40, 40), r.uniform(8, 14), "#F2D8C9", op=0.9))
    z.append(ellipse(cx, cy, 110, 80, "#7A2A22", C, 7) + ellipse(cx, cy - 6, 76, 50, "#3A1A16", C, 4))
    for k in range(7):
        z.append(circle(cx - 45 + k * 15, cy - 10 + (k % 2) * 10, 6, "#F2D8C9"))
    return "".join(z)


@scene("bipa6.u10")
def sinabung():
    """Mount Sinabung over the Karo highlands, with orange groves at Berastagi."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(path("M480 420 q-40 -60 10 -110 q20 -60 80 -50 q60 -30 100 20 q60 0 60 60 q-30 40 -90 30Z", "#C9C2BA", C, 4, 0.9))
    z.append(path("M260 700 L470 430 Q512 410 554 430 L800 700Z", "#6E6A80", C, 6))
    z.append(path("M470 430 L440 520 L500 480 L530 540 L554 430", "#B8B4C8", op=0.7))
    z.append(path("M126 700 Q512 660 898 700 L898 972 L126 972Z", "#7FA65A"))
    for row in range(4):
        y = 740 + row * 60
        z.append(path(f"M126 {y} Q512 {y - 20} 898 {y}", stroke=G, sw=22))
    for k in range(5):
        x = 190 + k * 160
        z.append(ellipse(x, 900, 60, 50, G, C, 5) + line(x, 950, x, 972, C, 8))
        for j in range(4):
            z.append(circle(x - 30 + j * 20, 890 + (j % 2) * 16, 9, O, C, 2))
    return "".join(z)


# ---------------------------------------------------------------- BIPA 7 · Papua and Raja Ampat


def karst_islet(x, y, w, h, color=G):
    return path(f"M{x - w / 2} {y} Q{x - w * 0.62} {y - h * 0.7} {x - w * 0.25} {y - h} Q{x + w * 0.1} {y - h * 1.1} {x + w * 0.4} {y - h * 0.8} Q{x + w * 0.62} {y - h * 0.4} {x + w / 2} {y}Z", color, C, 6) + path(f"M{x - w / 2} {y} Q{x} {y - h * 0.12} {x + w / 2} {y}", stroke=C, sw=4, op=0.4)


def canoe(x, y, s=1.0, rowers=2, color=D):
    z = [path(f"M{x - 160 * s} {y - 24 * s} Q{x} {y + 24 * s} {x + 170 * s} {y - 30 * s} L{x + 140 * s} {y + 4 * s} Q{x} {y + 26 * s} {x - 130 * s} {y + 4 * s}Z", color, C, 5)]
    z.append(path(f"M{x + 160 * s} {y - 28 * s} q20 -40 50 -40 q-10 20 -30 46", color, C, 4))  # carved prow
    for k in range(rowers):
        px = x - 70 * s + k * 90 * s
        z.append(person(px, y - 4 * s, 0.6 * s, (O, R, I)[k % 3]))
        z.append(line(px + 14 * s, y - 50 * s, px - 30 * s, y + 30 * s, C, 4))
    return "".join(z)


@scene("bipa7.u01")
def sorong():
    """Sorong harbour: a passenger ship, cranes and stacked containers."""
    z = [sky(sun=(250, 260), r=70), sea(700)]
    z.append(rect(X0, 600, W, 100, "#9C9488"))
    for k, col in enumerate((R, I, G, O, I, R, G)):
        z.append(rect(150 + (k % 4) * 90, 520 - (k // 4) * 50, 86, 48, col, 2, C, 4))
    for cx in (640, 800):
        z.append(line(cx, 600, cx, 300, C, 10) + line(cx, 310, cx - 160, 360, C, 8) + line(cx - 140, 352, cx - 140, 460, C, 3) + rect(cx - 160, 460, 40, 24, O, 2, C, 3))
    z.append(ferry(512, 830, 1.05))
    z.append(canoe(230, 930, 0.6, 1))
    return "".join(z)


def manta(x, y, s=1.0, rot=0):
    b = path("M0 -30 Q-80 -40 -200 30 Q-80 10 0 50 Q80 10 200 30 Q80 -40 0 -30Z", "#34405E", C, 5)
    b += path("M-20 -30 q-10 -26 -26 -30 M20 -30 q10 -26 26 -30", stroke=C, sw=6) + path("M0 50 L0 140", stroke=C, sw=4)
    b += path("M-60 0 Q0 30 60 0", P, op=0.4)
    return group(b, x, y, s, rot)


@scene("bipa7.u02")
def raja_ampat_reef():
    """The reefs of Raja Ampat: soft corals, a manta and a swirl of small fish."""
    z = [underwater()]
    z.append(manta(560, 330, 1.0, -8))
    r = random.Random(3)
    for k in range(16):
        a = k * 0.5
        z.append(fish(300 + math.cos(a) * (60 + k * 6), 560 + math.sin(a) * (40 + k * 4), 0.55, (O, P, "#F2C94C")[k % 3], math.degrees(a) + 90))
    z.append(path("M126 820 Q300 760 512 800 Q720 750 898 810 L898 972 L126 972Z", S))
    for k, col in enumerate((K, O, "#9D6CC2", R, K, O)):
        z.append(coral(170 + k * 130, 880 + (k % 2) * 30, 1.0, col))
    for k in range(5):
        x = 200 + k * 150
        z.append(ellipse(x, 940, 50, 26, (G, "#9D6CC2", O)[k % 3], C, 4))
    return "".join(z)


@scene("bipa7.u03")
def origin_myth():
    """A Papuan origin myth: the morning star rising over the sea and a carved canoe."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(path("M126 600 Q512 520 898 600 L898 640 L126 640Z", O, op=0.5))
    r = random.Random(5)
    for _ in range(40):
        z.append(circle(r.uniform(140, 880), r.uniform(140, 520), r.uniform(1.5, 3.5), P, op=0.8))
    sx, sy = 640, 300
    z.append(path(" ".join(f"{'M' if k == 0 else 'L'}{sx + math.cos(math.radians(k * 45 - 90)) * (70 if k % 2 == 0 else 22)} {sy + math.sin(math.radians(k * 45 - 90)) * (70 if k % 2 == 0 else 22)}" for k in range(8)) + "Z", P, C, 4))
    z.append(circle(sx, sy, 110, P, op=0.12))
    z.append(sea(640, (T3, I, "#22305A")))
    z.append(path(f"M{sx - 30} 650 L{sx + 30} 650 L{sx + 10} 900 L{sx - 10} 900Z", P, op=0.2))
    z.append(canoe(420, 820, 1.2, 1))
    return "".join(z)


@scene("bipa7.u04")
def piaynemo():
    """Piaynemo lookout: two friends chatting above the karst islets."""
    z = [sky(sun=(780, 240), r=60)]
    z.append(rect(X0, 460, W, 512, T1))
    for x, y, w, h in ((220, 560, 160, 90), (420, 520, 140, 70), (640, 600, 220, 120), (300, 700, 220, 130), (780, 740, 200, 120), (520, 820, 240, 140)):
        z.append(karst_islet(x, y, w, h))
    z.append(path("M126 972 L126 820 Q260 800 340 880 L420 972Z", "#6E5A50", C, 6))
    for k in range(5):
        z.append(line(150 + k * 50, 970, 170 + k * 50, 830 + k * 14, D, 8))
    z.append(person(200, 880, 0.9, O, scarf=None) + person(300, 920, 0.9, R))
    for bx, by in ((160, 700), (330, 740)):
        z.append(path(f"M{bx} {by} q0 -50 60 -50 q60 0 60 50 q0 50 -60 50 q-20 0 -36 -6 l-24 14 l8 -24 q-8 -14 -8 -34Z", P, C, 4))
        z.append(circle(bx + 36, by, 6, C) + circle(bx + 60, by, 6, C) + circle(bx + 84, by, 6, C))
    return "".join(z)


@scene("bipa7.u06")
def misool():
    """Misool: ochre hand stencils on a karst cliff above clear water."""
    z = [sky(sun=(250, 230), r=60)]
    z.append(path("M380 122 L898 122 L898 760 L420 760 Q400 600 460 520 Q380 420 420 300Z", "#B8A48A", C, 6))
    r = random.Random(9)
    for k in range(7):
        x, y = 520 + (k % 4) * 90 + r.uniform(-10, 10), 320 + (k // 4) * 140 + r.uniform(-10, 10)
        hand = path("M0 40 Q-30 30 -30 0 L-34 -40 M-14 2 L-16 -56 M0 0 L2 -62 M14 2 L18 -54 M26 10 L44 -20", stroke="#B8582E", sw=14, op=0.85) + ellipse(0, 20, 30, 30, "#B8582E", op=0.85)
        z.append(group(hand, x, y, 0.8, r.uniform(-20, 20)))
    z.append(path("M640 520 L700 470 L740 540 M600 600 Q650 560 700 600", stroke="#B8582E", sw=8, op=0.7))
    z.append(rect(X0, 740, W, 232, T1) + path("M126 820 Q512 790 898 830 L898 972 L126 972Z", T2))
    z.append(karst_islet(220, 760, 200, 220) + karst_islet(340, 780, 120, 120, "#3F7A33"))
    z.append(group(path("M-60 -10 Q0 14 70 -14 L56 6 Q0 24 -50 6Z", D, C, 4), 560, 880, 1.2))
    return "".join(z)


@scene("bipa7.u07")
def mansinam():
    """Mansinam island off Manokwari: a green island, a small white church, a boat."""
    z = [sky(sun=(700, 270), r=80), sea(640)]
    z.append(path("M180 660 Q300 480 520 500 Q760 520 840 660Z", G, C, 6))
    z.append(path("M180 660 Q512 690 840 660", S, C, 4))
    z.append(rect(450, 560, 110, 80, P, 3, C, 5) + path("M440 560 L505 500 L570 560Z", R, C, 5) + line(505, 500, 505, 456, C, 5) + line(488, 474, 522, 474, C, 5) + rect(490, 600, 30, 40, C, 3))
    for x in (280, 360, 650, 740):
        z.append(palm(x, 640, 110, 6, 0.5))
    z.append(path("M126 880 Q300 840 420 860 L420 972 L126 972Z", S))
    z.append(boat(640, 860, 0.9, D, None, True))
    return "".join(z)


@scene("bipa7.u08")
def teluk_jayapura():
    """Jayapura bay: the red arch of the Youtefa bridge between green hills."""
    z = [sky(sun=(512, 260), r=70)]
    z.append(path("M126 640 Q200 420 360 560 L360 680 L126 680Z", G, C, 5) + path("M898 620 Q800 400 660 560 L660 680 L898 680Z", G, C, 5))
    for k in range(8):
        z.append(rect(150 + k * 26, 590 - (k % 3) * 18, 18, 18, P, 2, C, 2) + rect(700 + k * 24, 580 - (k % 3) * 18, 18, 18, P, 2, C, 2))
    z.append(sea(660))
    z.append(path("M126 680 L898 680", stroke=C, sw=10) + path("M126 680 L898 680", stroke=P, sw=4))
    for cx, w, h in ((380, 260, 200), (650, 260, 200)):
        z.append(path(f"M{cx - w / 2} 680 Q{cx} {680 - h * 2} {cx + w / 2} 680", stroke=R, sw=14) + path(f"M{cx - w / 2} 680 Q{cx} {680 - h * 2} {cx + w / 2} 680", stroke=C, sw=2, op=0.5))
        for k in range(1, 8):
            t = k / 8
            xx = cx - w / 2 + w * t
            yy = 680 - 4 * h * t * (1 - t)
            z.append(line(xx, yy, xx, 680, C, 3))
    for x in (250, 515, 780):
        z.append(rect(x - 10, 686, 20, 70, "#9C9488", 2, C, 4))
    z.append(boat(260, 880, 0.7, D, None, True))
    return "".join(z)


@scene("bipa7.u09")
def sentani():
    """Lake Sentani festival: dancers on joined canoes among the lake's islands."""
    z = [sky(sun=(250, 270), r=70)]
    z.append(path("M126 560 Q300 420 512 520 Q700 400 898 520 L898 600 L126 600Z", "#6E7FA8"))
    z.append(sea(580, (T2, T3, I)))
    z.append(island(760, 640, 90, 40, 0, False) + island(260, 620, 70, 28, 0, False))
    for k in range(4):
        z.append(rect(560 + k * 40, 600, 30, 40, D, 2, C, 3) + path(f"M{555 + k * 40} 600 L{575 + k * 40} 580 L{595 + k * 40} 600Z", "#6E5A50", C, 3))
    # two canoes lashed together with a platform of dancers
    z.append(canoe(460, 880, 1.3, 0) + canoe(560, 830, 1.1, 0))
    z.append(rect(300, 780, 420, 20, D, 3, C, 4))
    for k, x in enumerate((360, 450, 540, 630)):
        z.append(person(x, 780, 0.9, (R, I, O, R)[k]))
        for j in range(3):
            z.append(line(x, 680, x - 20 + j * 20, 640, (C, P, C)[j], 5))
    return "".join(z)


def honai(cx, y, s=1.0):
    b = rect(-70, -100, 140, 100, D, 4, C, 5) + rect(-20, -60, 40, 60, C, 4)
    b += path("M-100 -90 Q-100 -230 0 -240 Q100 -230 100 -90Z", "#8C7A5A", C, 6)
    b += "".join(path(f"M{-90 + k * 30} -96 Q{-60 + k * 20} -180 0 -236", stroke=C, sw=2, op=0.35) for k in range(7))
    return group(b, cx, y, s)


@scene("bipa7.u10")
def baliem():
    """The Baliem valley festival: honai huts, high peaks and dancers with plumes."""
    z = [sky(sun=(700, 250), r=60)]
    z.append(path("M126 560 L260 340 L360 460 L500 280 L640 450 L760 330 L898 520 L898 640 L126 640Z", "#6E7FA8", C, 5))
    z.append(path("M480 310 L500 280 L520 310Z M245 365 L260 340 L275 365Z", P))
    z.append(path("M126 640 Q512 600 898 650 L898 972 L126 972Z", "#7FA65A"))
    z.append(honai(220, 760, 0.9) + honai(360, 720, 0.7) + honai(800, 740, 0.8))
    for k, x in enumerate((420, 520, 620, 720)):
        y = 920 - (k % 2) * 20
        z.append(person(x, y, 1.0, C, head=C))
        z.append(path(f"M{x - 8} {y - 126} L{x - 30} {y - 200} M{x} {y - 126} L{x} {y - 210} M{x + 8} {y - 126} L{x + 30} {y - 200}", stroke=(P, C, P, C)[k], sw=8))
        z.append(line(x + 30, y - 10, x + 50, y - 240, D, 6))
        z.append(rect(x - 16, y - 80, 32, 8, O, 2))
    return "".join(z)


# ---------------------------------------------------------------- Javanese · shared shapes


def joglo(cx, y, w=420, h=120, roof="#6E5A50", posts=6):
    """A Javanese pendopo: open pillars under a high-peaked joglo roof."""
    z = [rect(cx - w / 2, y - 24, w, 24, S, 3, C, 5)]
    for k in range(posts):
        x = cx - w * 0.42 + k * (w * 0.84 / (posts - 1))
        z.append(rect(x - 8, y - 24 - h, 16, h, D, 2, C, 4))
    top = y - 24 - h
    z.append(path(f"M{cx - w * 0.6} {top + 10} L{cx + w * 0.6} {top + 10} L{cx + w * 0.3} {top - 60} L{cx - w * 0.3} {top - 60}Z", roof, C, 6))
    z.append(path(f"M{cx - w * 0.3} {top - 60} L{cx + w * 0.3} {top - 60} L{cx + w * 0.12} {top - 170} L{cx - w * 0.12} {top - 170}Z", roof, C, 6))
    z.append(line(cx - w * 0.14, top - 170, cx + w * 0.14, top - 170, C, 8))
    return "".join(z)


def tajug(cx, y, w=420, tiers=3, roof="#4A4A50", walls=P):
    """A old Javanese mosque: square hall under a stacked pyramid roof."""
    z = [rect(cx - w * 0.4, y - 130, w * 0.8, 130, walls, 3, C, 6)]
    z.append(windows(cx - w * 0.32, y - 100, 4, 1, 36, 70, (w * 0.64 - 144) / 3, 0, I, arch=False))
    yy = y - 130
    for k in range(tiers):
        ww = w * (1 - k * 0.28)
        hh = 70 - k * 8
        z.append(path(f"M{cx - ww / 2} {yy + 6} L{cx + ww / 2} {yy + 6} L{cx + ww * 0.3} {yy - hh} L{cx - ww * 0.3} {yy - hh}Z", roof, C, 6))
        if k < tiers - 1:
            z.append(rect(cx - ww * 0.3, yy - hh - 30, ww * 0.6, 30, walls, 2, C, 4))
            yy -= hh + 30
        else:
            yy -= hh
    z.append(path(f"M{cx - 20} {yy} L{cx} {yy - 40} L{cx + 20} {yy}Z", roof, C, 4))
    return "".join(z)


def train(x, y, s=1.0, body="#C8483A", cars=2):
    z = []
    for k in range(cars):
        cx = x + k * 250 * s
        z.append(rect(cx, y - 110 * s, 240 * s, 100 * s, body if k == 0 else P, 14, C, 6))
        z.append(windows(cx + 20 * s, y - 92 * s, 4, 1, 36 * s, 34 * s, 20 * s, 0, I))
        z.append(rect(cx, y - 40 * s, 240 * s, 10 * s, I, 0))
        for dx in (40, 200):
            z.append(circle(cx + dx * s, y - 4 * s, 16 * s, C))
    return "".join(z)


def rails(y):
    return path(f"M126 {y} L898 {y} M126 {y + 20} L898 {y + 20}", stroke=C, sw=5) + "".join(line(140 + k * 40, y - 6, 140 + k * 40, y + 26, D, 6, 0.8) for k in range(20))


# ---------------------------------------------------------------- J1 · Surabaya


@scene("j1.u01")
def suroboyo_bus():
    """Suroboyo Bus: the red city bus you pay for with plastic bottles."""
    z = [sky(sun=(760, 250), r=70), city_far(640, 0.3)]
    z.append(rect(X0, 760, W, 212, "#7A7A80") + path("M126 870 L898 870", stroke=P, sw=8, dash="50 40"))
    z.append(rect(180, 520, 620, 240, R, 30, C, 7))
    z.append(windows(220, 560, 6, 1, 70, 80, 22, 0, "#9CC9D9"))
    z.append(rect(180, 680, 620, 18, P, 0) + rect(700, 560, 70, 190, "#9CC9D9", 6, C, 5))
    for x in (290, 680):
        z.append(circle(x, 760, 46, C) + circle(x, 760, 18, "#9C9488"))
    # a bag of plastic bottles: the fare
    z.append(group(path("M-50 0 L-40 -110 L40 -110 L50 0Z", P, C, 5, 0.9) + "".join(rect(-34 + k * 24, -96, 18, 60, "#9CC9D9", 6, C, 3) for k in range(3)), 250, 950, 0.9))
    z.append(person(340, 960, 0.8, O, scarf=I))
    return "".join(z)


@scene("j1.u02")
def gubeng():
    """Gubeng station: a long train hall and a train at the platform."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rect(160, 420, 704, 300, P, 4, C, 7))
    z.append(path("M140 420 Q512 300 884 420Z", "#6E7FA8", C, 7))
    z.append(windows(200, 470, 9, 1, 50, 90, 23, 0, I, arch=True))
    z.append(rect(X0, 720, W, 40, S, 0, C, 4) + rect(X0, 760, W, 212, "#9C9488"))
    z.append(train(180, 850, 1.0, R, 3))
    z.append(rails(860))
    z.append(path("M480 600 L544 600 L544 720 L480 720Z", I, C, 5))
    return "".join(z)


@scene("j1.u03")
def tunjungan():
    """Jalan Tunjungan at night: old shopfronts lit up, a scooter's streak of light."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(circle(760, 230, 50, P, op=0.7))
    z.append(rect(140, 380, 300, 420, "#E3C9A8", 4, C, 6) + rect(250, 300, 80, 80, "#E3C9A8", 4, C, 6) + path("M250 300 L290 250 L330 300Z", O, C, 5))
    z.append(windows(170, 420, 4, 3, 46, 60, 22, 40, "#F2C94C"))
    z.append(rect(500, 440, 360, 360, P, 4, C, 6) + windows(530, 470, 5, 3, 44, 56, 22, 34, "#F2C94C", arch=True))
    z.append(rect(X0, 800, W, 172, "#3A3A48"))
    for k in range(5):
        x = 180 + k * 170
        z.append(line(x, 800, x, 600, C, 5) + circle(x, 594, 16, "#F2C94C", C, 3) + circle(x, 594, 40, "#F2C94C", op=0.2))
    z.append(path("M126 900 Q512 870 898 900", stroke=O, sw=10, op=0.8) + path("M126 920 Q512 890 898 920", stroke=R, sw=6, op=0.6))
    z.append(person(300, 860, 0.7, K) + person(340, 865, 0.7, O))
    return "".join(z)


@scene("j1.u04")
def ampel():
    """The Sunan Ampel mosque: a tiered roof, the tall old minaret, pilgrims' lane."""
    z = [sky(sun=(250, 270), r=70)]
    z.append(rect(X0, 820, W, 152, S))
    z.append(tajug(430, 820, 480, 3, "#4A4A50"))
    z.append(rect(720, 340, 70, 480, P, 4, C, 6) + rect(706, 340, 98, 30, P, 3, C, 5) + path("M714 340 Q755 270 796 340Z", G, C, 5))
    z.append(windows(740, 400, 1, 4, 30, 50, 0, 50, I, arch=True))
    for k, col in enumerate((P, I, O, K)):
        z.append(person(220 + k * 120, 950, 0.8, col, scarf=P if k % 2 else None))
    return "".join(z)


def suramadu(y):
    z = [path(f"M126 {y} L898 {y}", stroke=C, sw=6)]
    for tx in (380, 640):
        z.append(rect(tx - 10, y - 140, 20, 160, "#9C9488", 2, C, 3))
        for k in range(5):
            z.append(line(tx, y - 130, tx - 30 - k * 24, y, C, 2) + line(tx, y - 130, tx + 30 + k * 24, y, C, 2))
    return "".join(z)


@scene("j1.u05")
def kenjeran():
    """Kenjeran beach: painted fishing boats on the sand, Suramadu bridge beyond."""
    z = [sky(sun=(700, 300), r=80), sea(620)]
    z.append(group(suramadu(0), 0, 600, 1))
    z.append(path("M126 800 Q500 760 898 800 L898 972 L126 972Z", S))
    z.append(boat(330, 860, 1.0, "#3F7AB0") + boat(680, 900, 0.9, G))
    z.append(person(520, 930, 0.7, O, scarf=I))
    return "".join(z)


def komodo(x, y, s=1.0):
    col = "#7A7A5A"
    b = path("M-200 0 Q-120 -50 0 -40 Q100 -36 160 -10 Q200 0 230 -6 Q200 16 150 14 Q60 30 -60 20 Q-200 30 -320 10 Q-220 6 -200 0Z", col, C, 6)
    for lx, d in ((-120, 1), (-60, -1), (60, 1), (110, -1)):
        b += path(f"M{lx} 10 q{d * 10} 30 {d * 30} 40", stroke=col, sw=22) + path(f"M{lx} 10 q{d * 10} 30 {d * 30} 40", stroke=C, sw=2, op=0.5)
    b += circle(190, -14, 5, C) + path("M230 0 l30 -6 l-10 6 l10 6Z", O)
    r = random.Random(2)
    for _ in range(18):
        b += circle(r.uniform(-180, 140), r.uniform(-30, 10), 4, C, op=0.25)
    return group(b, x, y, s)


@scene("j1.u06")
def kebun_binatang():
    """The Surabaya zoo: a Komodo dragon basking, the shark-and-crocodile statue behind."""
    z = [sky(sun=(250, 270), r=70)]
    # the city's emblem statue: a shark and a crocodile circling each other
    z.append(rect(580, 560, 160, 120, "#9C9488", 4, C, 5))
    z.append(path("M600 560 Q580 420 660 360 Q680 420 640 470 Q620 520 650 560Z", "#9C9488", C, 5))
    z.append(path("M720 560 Q760 440 680 380 Q690 450 700 480 Q700 520 690 560Z", "#B8B4C8", C, 5))
    z.append(path("M126 700 Q512 660 898 710 L898 972 L126 972Z", "#B8A070"))
    z.append(tree(200, 720, 0.8))
    z.append(komodo(470, 850, 1.1))
    z.append(path("M126 960 L898 960", stroke=D, sw=12) + "".join(line(160 + k * 90, 900, 160 + k * 90, 972, D, 8) for k in range(9)))
    return "".join(z)


@scene("j1.u07")
def pasar_atom():
    """Pasar Atom: a tall market block, bolts of cloth and shoppers outside."""
    z = [sky(sun=(780, 250), r=60)]
    z.append(rect(180, 300, 664, 500, P, 4, C, 7))
    for r_ in range(4):
        z.append(rect(180, 340 + r_ * 110, 664, 14, O, 0, C, 3))
        z.append(windows(210, 370 + r_ * 110, 9, 1, 50, 50, 22, 0, "#9CC9D9"))
    z.append(rect(420, 700, 184, 100, C, 6))
    z.append(rect(X0, 800, W, 172, S))
    z.append(stall(150, 940, 220, R, (P, K, O)) + stall(660, 940, 220, G, (O, I, P)))
    z.append(person(460, 920, 0.8, K, scarf=I) + person(560, 925, 0.8, I))
    return "".join(z)


# ---------------------------------------------------------------- J2 · Madiun line


def bench(x, y, w=220):
    return rect(x, y - 50, w, 16, D, 3, C, 4) + line(x + 14, y - 34, x + 14, y, C, 6) + line(x + w - 14, y - 34, x + w - 14, y, C, 6)


def glass(x, y, fill="#B8582E"):
    return path(f"M{x - 18} {y - 50} L{x + 18} {y - 50} L{x + 14} {y} L{x - 14} {y}Z", fill, C, 4, 0.95) + rect(x - 18, y - 52, 36, 6, P, 2, C, 2)


@scene("j2.u01")
def warung_pak_budi():
    """Pak Budi's warung: a small roadside stall, glasses of sweet tea, a bench."""
    z = [sky(sun=(760, 260), r=70), hills(True)]
    z.append(rect(X0, 800, W, 172, S))
    z.append(rect(220, 520, 580, 280, D, 6, C, 7) + rect(250, 560, 520, 120, P, 4, C, 5))
    z.append(path("M190 530 L830 530 L780 430 L240 430Z", G, C, 6) + path("M190 530 L830 530", stroke=P, sw=8, dash="40 40"))
    for k in range(5):
        z.append(rect(280 + k * 96, 590, 70, 70, "none", 30, C, 3, 0.6) + circle(315 + k * 96, 625, 24, (O, "#F2C94C", K, G, O)[k], C, 3))
    z.append(rect(220, 690, 580, 26, P, 3, C, 5))
    z.append(glass(330, 690) + glass(380, 690) + glass(620, 690, "#F2C94C"))
    z.append(bench(260, 900, 220) + bench(560, 900, 220))
    z.append(person(380, 850, 0.6, I) + person(660, 850, 0.6, O, scarf=K))
    return "".join(z)


@scene("j2.u02")
def nasi_pecel():
    """Nasi pecel Madiun on a banana leaf: greens, peanut sauce, a rempeyek cracker."""
    z = [rect(X0, Y0, W, H, D)]
    for k in range(10):
        z.append(line(126, 160 + k * 90, 898, 160 + k * 90, C, 2, 0.25))
    z.append(ellipse(512, 560, 340, 300, "#5A8F3E", C, 7) + path("M200 560 L824 560", stroke="#3F7A33", sw=6))
    for k in range(8):
        z.append(path(f"M{240 + k * 75} 400 L{280 + k * 70} 560", stroke="#3F7A33", sw=3, op=0.6))
    z.append(ellipse(420, 560, 120, 90, P, C, 5))  # rice
    for k in range(6):
        z.append(path(f"M{560 + k * 18} {470 + (k % 2) * 30} q30 30 0 60 q-20 20 10 40", stroke=G, sw=14))
    z.append(path("M520 560 Q600 520 680 560 Q700 640 600 660 Q520 650 520 560Z", "#9C5A2A", C, 5))  # peanut sauce
    z.append(path("M330 660 Q420 640 520 690 Q480 760 380 750 Q300 720 330 660Z", "#E8C178", C, 5))  # rempeyek
    for k in range(6):
        z.append(circle(360 + k * 26, 700 + (k % 2) * 14, 7, "#9C5A2A"))
    z.append(ellipse(640, 420, 40, 26, "#B8582E", C, 4) + ellipse(700, 440, 34, 22, "#E8C178", C, 4))
    z.append(path("M780 880 L860 700", stroke=P, sw=14) + ellipse(870, 680, 22, 36, P, C, 4, extra='transform="rotate(25 870 680)"'))
    return "".join(z)


@scene("j2.u03")
def bawang_merah():
    """Nganjuk shallots: bundles drying on racks above the green field."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(path("M126 600 Q512 560 898 610 L898 972 L126 972Z", G))
    for row in range(6):
        y = 640 + row * 52
        z.append(path(f"M126 {y} Q512 {y - 20} 898 {y + 6}", stroke="#3F7A33", sw=12))
    z.append(line(180, 760, 840, 760, D, 10) + line(220, 760, 220, 972, D, 10) + line(800, 760, 800, 972, D, 10))
    for k in range(12):
        x = 240 + k * 48
        z.append(line(x, 760, x, 790, "#C8B060", 4))
        for j in range(4):
            z.append(ellipse(x - 12 + j * 8, 810 + (j % 2) * 8, 14, 18, "#B8485A", C, 3))
    z.append(path("M126 972 L898 972 L898 920 Q512 880 126 920Z", S))
    for k in range(9):
        z.append(ellipse(180 + k * 80, 940, 22, 26, "#B8485A", C, 3))
    z.append(person(700, 920, 0.8, I, scarf=O))
    return "".join(z)


@scene("j2.u04")
def reog():
    """Reog Ponorogo: the tiger mask under its towering fan of peacock feathers."""
    z = [sky(sun=None, glow=True)]
    z.append(rect(X0, 860, W, 112, S))
    cx, cy = 512, 700
    for k in range(23):
        a = math.radians(180 + k * (180 / 22))
        ex, ey = cx + math.cos(a) * 400, cy + math.sin(a) * 440
        z.append(path(f"M{cx} {cy} L{ex} {ey}", stroke=G, sw=20) + path(f"M{cx} {cy} L{ex} {ey}", stroke=C, sw=2, op=0.5))
        z.append(ellipse(ex, ey, 26, 34, "#3F7AB0", C, 3, extra=f'transform="rotate({math.degrees(a) + 90} {ex} {ey})"') + circle(ex, ey, 10, I) + circle(ex, ey, 4, "#F2C94C"))
    for k in range(17):
        a = math.radians(180 + k * (180 / 16))
        z.append(ellipse(cx + math.cos(a) * 240, cy + math.sin(a) * 270, 16, 22, "#3F7AB0", C, 3) + circle(cx + math.cos(a) * 240, cy + math.sin(a) * 270, 6, I))
    # the tiger head
    z.append(ellipse(cx, cy + 30, 150, 120, O, C, 7))
    for d in (-1, 1):
        z.append(path(f"M{cx + d * 60} {cy - 60} q{d * 20} 40 {d * 50} 60 M{cx + d * 100} {cy - 20} q{d * 20} 40 {d * 40} 60", stroke=C, sw=8))
        z.append(ellipse(cx + d * 55, cy + 10, 26, 18, P, C, 4) + circle(cx + d * 55, cy + 10, 8, C))
        z.append(path(f"M{cx + d * 120} {cy - 70} l{d * 30} -40 l{d * 10} 50Z", O, C, 4))
    z.append(ellipse(cx, cy + 60, 30, 20, C) + path(f"M{cx - 70} {cy + 100} Q{cx} {cy + 150} {cx + 70} {cy + 100}", P, C, 5))
    z.append(path(f"M{cx - 60} {cy + 140} L{cx - 60} {cy + 160} M{cx + 60} {cy + 140} L{cx + 60} {cy + 160}", stroke=C, sw=6))
    z.append(path(f"M{cx - 110} 972 L{cx - 90} {cy + 150} L{cx + 90} {cy + 150} L{cx + 110} 972Z", C))
    return "".join(z)


@scene("j2.u05")
def sarangan():
    """Lake Sarangan below Mount Lawu: pine slopes, a horse, a little boat."""
    z = [sky(sun=(700, 260), r=70)]
    z.append(path("M126 600 Q300 340 512 300 Q720 340 898 600Z", "#6E7FA8"))
    z.append(path("M126 640 Q260 520 380 600 L126 700Z", G) + path("M898 640 Q760 520 640 600 L898 700Z", G))
    for k in range(7):
        x = 150 + k * 40
        z.append(poly([(x, 660 - (k % 3) * 20), (x - 20, 720), (x + 20, 720)], "#2F5E2A", C, 3))
        z.append(poly([(874 - k * 40, 660 - (k % 3) * 20), (854 - k * 40, 720), (894 - k * 40, 720)], "#2F5E2A", C, 3))
    z.append(ellipse(512, 760, 360, 110, T2, C, 5) + path("M300 760 q30 -10 60 0 M600 790 q30 -10 60 0", stroke=P, sw=5, op=0.6))
    z.append(group(path("M-60 -10 Q0 14 70 -14 L56 6 Q0 24 -50 6Z", P, C, 4) + path("M-30 -10 L-20 -40 L30 -40 L36 -12", R, C, 4), 560, 760, 1.1))
    z.append(path("M126 860 Q512 830 898 870 L898 972 L126 972Z", "#7FA65A"))
    horse = path("M-60 -40 Q0 -60 50 -40 L70 -90 L90 -80 L80 -40 L70 0 M-60 -40 L-60 0 M-40 -30 L-40 0 M30 -30 L30 0 M50 -30 L50 0", "#8A5A3C", C, 5) + path("M-60 -40 Q0 -60 50 -40 L50 -16 Q0 -10 -60 -16Z", "#8A5A3C", C, 5)
    z.append(group(horse, 300, 940, 1.2) + person(400, 940, 0.75, I, scarf=O))
    return "".join(z)


@scene("j2.u06")
def benteng_pendem():
    """Benteng Pendem Ngawi: the Dutch fort's brick arches behind its earth bank."""
    z = [sky(sun=(250, 270), r=70)]
    z.append(rect(160, 440, 704, 340, BRICK, 4, C, 7))
    z.append(windows(200, 540, 6, 1, 70, 140, 46, 0, "#4A3A30", arch=True))
    z.append(rect(160, 440, 704, 40, "#C88A6A", 0, C, 5))
    for k in range(12):
        z.append(line(160, 500 + k * 24, 864, 500 + k * 24, C, 1.5, 0.15))
    z.append(path("M160 440 L220 480 M864 440 L800 480", stroke=C, sw=3, op=0.4))
    for x in (260, 700):
        z.append(path(f"M{x} 440 q20 -50 60 -30 q30 -40 60 0", G, C, 4))
    z.append(path("M126 760 Q512 700 898 770 L898 840 L126 840Z", G))
    z.append(rect(X0, 840, W, 132, T3) + path("M126 880 q60 -10 120 0 M500 920 q60 -10 120 0", stroke=P, sw=5, op=0.5))
    return "".join(z)


# ---------------------------------------------------------------- J3 · Solo


@scene("j3.u01")
def solo_balapan():
    """Solo Balapan station: the green-roofed hall and a train pulling in."""
    z = [sky(sun=(760, 260), r=70)]
    z.append(rect(200, 420, 624, 280, P, 4, C, 7))
    z.append(path("M170 430 L854 430 L780 320 L244 320Z", G, C, 7) + path("M400 320 L624 320 L560 250 L464 250Z", G, C, 6))
    z.append(windows(240, 470, 8, 1, 50, 100, 25, 0, I, arch=True))
    z.append(rect(X0, 700, W, 60, S, 0, C, 4) + rect(X0, 760, W, 212, "#9C9488"))
    z.append(train(150, 850, 1.0, I, 3))
    z.append(rails(860))
    return "".join(z)


@scene("j3.u02")
def pasar_gede():
    """Pasar Gede: the market's broad gateway roof, fruit baskets in front."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rect(150, 500, 724, 300, P, 4, C, 7))
    z.append(path("M120 510 L904 510 L840 420 L184 420Z", "#6E5A50", C, 7))
    z.append(rect(380, 360, 264, 160, P, 4, C, 6) + path("M350 370 L674 370 L620 280 L404 280Z", "#6E5A50", C, 6))
    z.append(rect(440, 400, 144, 40, I, 4, C, 4))
    z.append(windows(190, 560, 3, 1, 50, 90, 26, 0, I) + windows(638, 560, 3, 1, 50, 90, 26, 0, I))
    z.append(rect(430, 560, 164, 240, C, 6))
    z.append(rect(X0, 800, W, 172, S))
    for k, col in enumerate((O, G, "#F2C94C", R, K)):
        x = 200 + k * 155
        z.append(ellipse(x, 900, 60, 22, D, C, 4) + path(f"M{x - 60} 900 L{x - 46} 950 L{x + 46} 950 L{x + 60} 900", D, C, 4))
        for j in range(4):
            z.append(circle(x - 30 + j * 20, 888 - (j % 2) * 10, 14, col, C, 3))
    return "".join(z)


@scene("j3.u04")
def mangkunegaran():
    """Pura Mangkunegaran: the great open pendopo under its joglo roof."""
    z = [sky(sun=(780, 250), r=60)]
    z.append(rect(X0, 760, W, 212, S))
    z.append(joglo(512, 780, 640, 160, "#4F6A50", 7))
    z.append(path("M200 780 L824 780 L860 840 L164 840Z", P, C, 5))
    for k in range(3):  # the pendopo's hanging lamps
        x = 360 + k * 152
        z.append(line(x, 470, x, 540, C, 3) + path(f"M{x - 34} 540 L{x + 34} 540 L{x + 18} 580 L{x - 18} 580Z", P, C, 4) + circle(x, 590, 8, O, C, 2))
    z.append(person(420, 940, 0.8, I, scarf=C) + person(600, 940, 0.8, "#6E5A50", scarf=C))
    return "".join(z)


@scene("j3.u06")
def laweyan():
    """Laweyan: a merchant's high-walled lane, batik cloth drying on bamboo poles."""
    z = [sky(sun=(512, 240), r=60)]
    z.append(path("M126 300 L380 360 L380 972 L126 972Z", P, C, 6) + path("M898 300 L644 360 L644 972 L898 972Z", "#E3C9A8", C, 6))
    z.append(path("M380 360 L644 360 L644 520 L380 520Z", "#6E5A50", C, 5) + path("M380 360 L512 300 L644 360", G, C, 5))
    z.append(path("M380 972 L460 600 L564 600 L644 972Z", S, C, 4))
    z.append(path("M160 420 L340 450 M160 500 L340 520", stroke=C, sw=3, op=0.3) + rect(220, 640, 100, 180, D, 40, C, 5))
    z.append(line(126, 560, 898, 560, D, 8))
    for k, (x, col) in enumerate(((220, I), (360, "#B8582E"), (560, I), (700, "#B8582E"))):
        z.append(rect(x, 560, 110, 220, col, 0, C, 5))
        z.append("".join(kawung(x + 30 + (j % 2) * 50, 600 + (j // 2) * 50, 0.4, 0.6) for j in range(8)))
    return "".join(z)


# ---------------------------------------------------------------- J4 · Yogyakarta


@scene("j4.u01")
def tugu():
    """Tugu Yogyakarta: the white pillar with its pointed top at the crossroads."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(path("M126 760 L898 760 L898 972 L126 972Z", "#7A7A80"))
    z.append(path("M512 760 L360 972 M512 760 L664 972 M126 860 L898 860", stroke=P, sw=6, dash="40 30"))
    z.append(rect(440, 700, 144, 60, P, 4, C, 6) + rect(460, 560, 104, 140, P, 4, C, 6))
    z.append(path("M470 560 Q470 480 512 470 Q554 480 554 560Z", P, C, 6))
    z.append(path("M490 470 L490 380 Q512 340 534 380 L534 470Z", P, C, 6) + path("M512 360 L512 280", stroke=O, sw=12) + path("M500 300 Q512 260 524 300", O, C, 3))
    z.append(path("M480 600 Q512 620 544 600 M480 640 Q512 660 544 640", stroke=O, sw=5))
    z.append(group(path("M-60 -20 L60 -20 L70 0 L-70 0Z", R, C, 4) + circle(-40, 4, 14, C) + circle(40, 4, 14, C), 280, 920, 1.0))
    z.append(person(760, 930, 0.8, I, scarf=O))
    return "".join(z)


@scene("j4.u03")
def kraton():
    """Kraton Yogyakarta: a pavilion of the palace and a palace servant in blangkon."""
    z = [sky(sun=(780, 250), r=60)]
    z.append(rect(X0, 720, W, 252, S))
    z.append(joglo(470, 760, 560, 150, "#3F5A42", 6))
    for k in range(6):
        z.append(rect(222 + k * 92, 590, 16, 146, "#3F5A42", 2, C, 4))
    z.append(path("M190 760 L750 760 L780 800 L160 800Z", P, C, 5))
    z.append(banyan(820, 760, 0.55))
    z.append(person(400, 930, 0.95, "#6E5A50", scarf=C) + rect(384, 860, 32, 40, I, 4))
    return "".join(z)


@scene("j4.u04")
def taman_sari():
    """Taman Sari, the water palace: bathing pools beneath a carved gate."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rect(230, 360, 564, 300, "#E3D3B5", 4, C, 7))
    z.append(path("M230 360 L330 300 L694 300 L794 360Z", "#E3D3B5", C, 6) + path("M430 300 L512 230 L594 300Z", "#E3D3B5", C, 6))
    z.append(path("M440 660 L440 470 Q512 400 584 470 L584 660Z", "#4A3A30", C, 6))
    z.append(windows(280, 440, 2, 1, 50, 90, 40, 0, "#4A3A30", arch=True) + windows(654, 440, 2, 1, 50, 90, 40, 0, "#4A3A30", arch=True))
    for x in (300, 724):
        z.append(path(f"M{x - 20} 380 q20 -30 40 0 q-20 20 -40 0", G, C, 3))
    z.append(rect(X0, 660, W, 312, "#C8B898"))
    z.append(rect(200, 720, 624, 200, T2, 6, C, 6) + rect(220, 740, 584, 160, T1, 4))
    for k in range(4):
        z.append(path(f"M{260 + k * 140} 800 q30 -10 60 0", stroke=P, sw=5, op=0.6))
    for x in (230, 794):
        z.append(rect(x - 20, 660, 40, 90, "#E3D3B5", 4, C, 5) + ellipse(x, 650, 34, 18, G, C, 4))
    return "".join(z)


@scene("j4.u05")
def kotabaru():
    """Kotabaru: Dutch-era villas and a church spire along a shady boulevard."""
    z = [sky(sun=(760, 250), r=60)]
    z.append(rect(560, 300, 140, 420, P, 4, C, 6) + path("M560 300 L630 160 L700 300Z", "#B5654A", C, 6) + line(630, 160, 630, 120, C, 5) + line(612, 136, 648, 136, C, 5))
    z.append(windows(600, 380, 1, 3, 60, 70, 0, 40, I, arch=True))
    for cx, col in ((300, "#E3C9A8"), (820, P)):
        z.append(rect(cx - 140, 560, 280, 200, col, 4, C, 6) + path(f"M{cx - 170} 570 L{cx} 450 L{cx + 170} 570Z", "#B5654A", C, 6))
        z.append(windows(cx - 110, 600, 3, 1, 50, 70, 30, 0, I, shutter=True))
    z.append(rect(X0, 760, W, 212, "#7A7A80") + path("M126 860 L898 860", stroke=P, sw=6, dash="40 30"))
    for x in (180, 470, 760):
        z.append(tree(x, 800, 0.75))
    z.append(group(path("M-50 0 L-30 -40 L30 -40 L50 0Z", G, C, 4) + circle(-30, 6, 14, C) + circle(30, 6, 14, C), 620, 920, 1))
    return "".join(z)


def prambanan_tower(cx, y, w, h, color="#5A5060"):
    z = []
    yy = y
    for k in range(7):
        ww = w * (1 - k * 0.12)
        hh = h * (0.22 if k == 0 else 0.11)
        z.append(rect(cx - ww / 2, yy - hh, ww, hh, color, 3, C, 4))
        yy -= hh
    z.append(path(f"M{cx - w * 0.15} {yy} L{cx} {yy - h * 0.12} L{cx + w * 0.15} {yy}Z", color, C, 4))
    return "".join(z)


@scene("j4.u06")
def ramayana():
    """The Ramayana ballet at Prambanan: dancers on the open stage before the temple."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(circle(760, 230, 50, P, op=0.7))
    z.append(prambanan_tower(512, 700, 220, 480) + prambanan_tower(300, 700, 150, 320) + prambanan_tower(724, 700, 150, 320))
    z.append(rect(X0, 700, W, 272, "#4A3A30") + rect(180, 740, 664, 120, "#6E5A50", 6, C, 6))
    z.append(path("M180 740 L320 600 L460 740Z", "#F2C94C", op=0.18) + path("M564 740 L704 600 L844 740Z", "#F2C94C", op=0.18))
    # Rama with his bow, Sinta, and a dancer with fire
    z.append(person(380, 760, 1.0, G, scarf=C) + path("M420 640 Q460 700 420 760", stroke=O, sw=6) + line(420, 640, 420, 760, P, 2))
    z.append(person(520, 760, 1.0, R, scarf=C) + path("M500 650 L470 620 M540 650 L570 620", stroke=C, sw=8))
    z.append(person(660, 760, 1.0, "#6E5A50", scarf=C) + path("M690 640 q-10 -40 10 -60 q10 30 0 60Z", O, C, 3))
    for k in range(7):
        z.append(circle(200 + k * 105, 940, 26, C, op=0.8))
    return "".join(z)


# ---------------------------------------------------------------- J5 · Semarang and the north coast


@scene("j5.u01")
def kota_lama():
    """Kota Lama Semarang: the domed Blenduk church among old warehouses."""
    z = [sky(sun=(780, 250), r=60)]
    for cx in (210, 814):
        z.append(rect(cx - 90, 520, 180, 260, "#E3C9A8", 4, C, 6) + path(f"M{cx - 100} 530 L{cx + 100} 530 L{cx + 80} 490 L{cx - 80} 490Z", "#B5654A", C, 5))
        z.append(windows(cx - 70, 560, 3, 2, 34, 60, 19, 40, G, shutter=True))
    z.append(rect(330, 500, 364, 280, P, 4, C, 7))
    z.append(path("M350 500 Q350 320 512 300 Q674 320 674 500Z", "#B5654A", C, 7) + path("M380 480 Q400 360 512 340", stroke=P, sw=6, op=0.4))
    z.append(rect(490, 270, 44, 40, P, 3, C, 5) + path("M486 270 Q512 230 538 270Z", "#B5654A", C, 4))
    for tx in (350, 674):
        z.append(rect(tx - 22, 400, 44, 120, P, 3, C, 5) + path(f"M{tx - 26} 400 L{tx} 360 L{tx + 26} 400Z", "#B5654A", C, 4))
    z.append(path("M400 620 L624 620 L512 560Z", P, C, 5))
    for k in range(4):
        z.append(rect(410 + k * 60, 620, 24, 160, P, 2, C, 4))
    z.append(rect(X0, 780, W, 192, "#8C8478"))
    for r_ in range(4):
        for c in range(12):
            z.append(rect(126 + c * 66 + (r_ % 2) * 33, 790 + r_ * 44, 60, 38, "none", 6, C, 2, 0.3))
    return "".join(z)


@scene("j5.u02")
def lawang_sewu():
    """Lawang Sewu, the 'thousand doors': long arcades and twin corner towers."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rect(126, 500, 772, 300, P, 4, C, 7))
    z.append(windows(150, 540, 12, 2, 40, 90, 22, 30, "#4A3A30", arch=True))
    z.append(path("M110 500 L914 500 L880 450 L144 450Z", "#B5654A", C, 6))
    for tx in (420, 604):
        z.append(rect(tx - 50, 300, 100, 500, P, 4, C, 7) + path(f"M{tx - 56} 300 Q{tx - 56} 220 {tx} 210 Q{tx + 56} 220 {tx + 56} 300Z", "#6E5A50", C, 6))
        z.append(windows(tx - 20, 340, 1, 2, 40, 70, 0, 30, I, arch=True))
    z.append(rect(X0, 800, W, 172, G) + rect(420, 800, 184, 172, S))
    z.append(tree(200, 900, 0.7) + tree(830, 900, 0.7))
    return "".join(z)


@scene("j5.u03")
def simpang_lima():
    """Simpang Lima at night: the square's lights and glowing pedal carts."""
    z = [rect(X0, Y0, W, H, I)]
    z.append(circle(250, 230, 46, P, op=0.7))
    z.append(city_far(560, 0.5) + "".join(rect(150 + k * 60, 470 + (k % 3) * 20, 10, 10, "#F2C94C", 0) for k in range(12)))
    z.append(tajug(640, 600, 300, 2, "#5A5060"))
    z.append(rect(X0, 600, W, 372, "#3A3A48"))
    z.append(ellipse(512, 760, 320, 100, "#4F7A4A", C, 5) + ellipse(512, 760, 320, 100, "none", P, 3, 0.4))
    for k, col in enumerate((R, "#F2C94C", G, K, O)):
        x = 200 + k * 150
        y = 880 + (k % 2) * 30
        z.append(rect(x - 60, y - 70, 120, 60, col, 10, C, 4) + circle(x - 60, y - 70, 40, col, op=0.18))
        z.append("".join(circle(x - 50 + j * 25, y - 76, 5, P) for j in range(5)))
        z.append(circle(x - 40, y - 6, 12, C) + circle(x + 40, y - 6, 12, C))
    return "".join(z)


def steam_loco(x, y, s=1.0):
    b = rect(-200, -150, 120, 130, C, 6, C, 5) + rect(-190, -136, 50, 50, "#F2C94C", 4, C, 3)
    b += rect(-80, -110, 240, 90, C, 30, C, 5) + rect(100, -180, 34, 80, C, 4) + rect(92, -190, 50, 16, C, 4)
    b += rect(-30, -150, 30, 40, C, 10) + rect(30, -140, 40, 30, O, 10, C, 3)
    b += line(-80, -60, 160, -60, R, 6)
    for wx, r in ((-150, 30), (-40, 40), (60, 40)):
        b += circle(wx, -10, r, R, C, 5) + circle(wx, -10, r * 0.3, C)
    b += line(-40, -10, 60, -10, P, 6) + path("M160 -20 L200 -6 L160 -6Z", "#9C9488", C, 3)
    return group(b, x, y, s)


@scene("j5.u04")
def ambarawa():
    """The Ambarawa railway museum: a steam locomotive puffing past the old station."""
    z = [sky(sun=(760, 250), r=60), hills(True)]
    z.append(rect(150, 520, 380, 220, P, 4, C, 6) + path("M130 530 L550 530 L500 450 L180 450Z", "#6E5A50", C, 6) + windows(180, 570, 4, 1, 50, 90, 38, 0, I, arch=True))
    z.append(rect(X0, 740, W, 232, "#9C9488"))
    z.append(path("M610 650 q-30 -40 0 -80 q-10 -50 50 -60 q20 -50 80 -30 q50 -20 80 30", P, C, 4, 0.9))
    z.append(steam_loco(480, 870, 1.15))
    z.append(rails(880))
    return "".join(z)


@scene("j5.u05")
def menara_kudus():
    """Menara Kudus: the red-brick minaret shaped like an old Javanese temple."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(rect(X0, 820, W, 152, S))
    cx = 512
    z.append(rect(cx - 160, 720, 320, 100, BRICK, 3, C, 6) + path(f"M{cx - 60} 820 L{cx - 60} 760 L{cx + 60} 760 L{cx + 60} 820Z", BRICK, C, 5))
    for k in range(5):
        z.append(rect(cx - 100 - (k % 2) * 6, 720 - (k + 1) * 60, 200 + (k % 2) * 12, 60, BRICK, 2, C, 5))
        for j in range(4):
            z.append(rect(cx - 80 + j * 46, 735 - (k + 1) * 60, 26, 26, "#E3D3B5", 2, C, 3))
    z.append(rect(cx - 120, 400, 240, 20, D, 3, C, 4))
    for d in (-1, 1):
        z.append(line(cx + d * 90, 420, cx + d * 90, 340, D, 10))
    z.append(path(f"M{cx - 150} 350 L{cx + 150} 350 L{cx + 80} 280 L{cx - 80} 280Z", "#4A4A50", C, 6) + path(f"M{cx - 80} 290 L{cx + 80} 290 L{cx} 220Z", "#4A4A50", C, 6))
    z.append(ellipse(cx, 380, 40, 30, D, C, 4))  # the bedug drum
    z.append(person(300, 950, 0.8, I, scarf=P) + person(720, 950, 0.8, P, head=C))
    return "".join(z)


@scene("j5.u06")
def masjid_demak():
    """The Demak Great Mosque: three stacked roofs, one of Java's oldest."""
    z = [sky(sun=(780, 250), r=60)]
    z.append(rect(X0, 800, W, 172, "#7FA65A") + rect(400, 800, 224, 172, S))
    z.append(tajug(512, 800, 600, 3, "#3A3A40", "#F5EFE3"))
    z.append(tree(170, 840, 0.7) + tree(860, 840, 0.7))
    return "".join(z)


# ---------------------------------------------------------------- J6 · Kotagede and Imogiri


def paduraksa(cx, y, w=180, h=280):
    """A roofed brick gate: a narrow doorway under a stepped top."""
    z = [rect(cx - w / 2, y - h * 0.65, w, h * 0.65, BRICK, 3, C, 6)]
    for k in range(4):
        ww = w * (1 - k * 0.18)
        z.append(rect(cx - ww / 2, y - h * 0.65 - (k + 1) * h * 0.09, ww, h * 0.09, BRICK, 2, C, 4))
    z.append(rect(cx - w * 0.2, y - h * 0.5, w * 0.4, h * 0.5, "#4A3A30", 3, C, 4))
    return "".join(z)


def brick_wall(x0, x1, y, h=120):
    z = [rect(x0, y - h, x1 - x0, h, BRICK, 2, C, 5)]
    for r_ in range(int(h // 24)):
        for c in range(int((x1 - x0) // 50)):
            z.append(rect(x0 + c * 50 + (r_ % 2) * 25, y - h + r_ * 24, 50, 24, "none", 0, C, 1.5, 0.25))
    return "".join(z)


@scene("j6.u02")
def masjid_gedhe_mataram():
    """Masjid Gedhe Mataram: the brick gate of Kotagede's mosque, its tiered roof behind."""
    z = [sky(sun=(250, 260), r=70)]
    z.append(tajug(512, 700, 460, 2, "#3A3A40"))
    z.append(rect(X0, 700, W, 272, S))
    z.append(brick_wall(126, 420, 840, 140) + brick_wall(604, 898, 840, 140))
    z.append(paduraksa(512, 860, 200, 340))
    z.append(banyan(200, 720, 0.45))
    z.append(person(700, 940, 0.8, "#6E5A50", scarf=C))
    return "".join(z)


def lurik(x, y, s=1.0):
    """A courtier in striped lurik and blangkon."""
    b = path("M-20 0 L-16 -76 Q0 -100 16 -76 L20 0Z", "#4A5A7A", C, 4)
    b += "".join(line(-16 + k * 8, -80, -18 + k * 8, 0, P, 2, 0.5) for k in range(5))
    b += circle(0, -112, 17, C, C, 4) + path("M-18 -116 Q0 -136 18 -116 L18 -108 L-18 -108Z", "#6E5A50", C, 3)
    return group(b, x, y, s)


@scene("j6.u03")
def makam_raja():
    """The royal graves of Mataram: a guarded gate, frangipani, courtiers in lurik."""
    z = [sky(sun=(780, 250), r=60)]
    z.append(rect(X0, 760, W, 212, S))
    z.append(brick_wall(126, 898, 780, 220))
    z.append(paduraksa(512, 800, 220, 400))
    z.append(path("M780 780 Q770 600 820 520 M820 520 Q760 470 720 480 M820 520 Q880 470 900 480", stroke="#6E5A50", sw=14))
    for fx, fy in ((720, 470), (760, 450), (860, 460), (900, 480), (820, 430)):
        z.append(flower(fx, fy, 20, P, "#F2C94C"))
    z.append(lurik(380, 900, 1.1) + lurik(650, 910, 1.1))
    return "".join(z)


@scene("j6.u04")
def jalan_rukunan():
    """Jalan Rukunan: a narrow lane between old walls, a child on a bicycle."""
    z = [sky(sun=(512, 230), r=50)]
    z.append(path("M126 260 L420 380 L420 900 L126 972Z", P, C, 6) + path("M898 260 L604 380 L604 900 L898 972Z", "#E3D3B5", C, 6))
    z.append(path("M126 260 L420 380 L420 400 L126 300Z", "#6E5A50", C, 4) + path("M898 260 L604 380 L604 400 L898 300Z", "#6E5A50", C, 4))
    z.append(path("M170 600 L380 560 L380 860 L170 900Z", "#4A3A30", C, 5) + rect(660, 560, 70, 200, D, 30, C, 5))
    z.append(path("M420 900 L604 900 L898 972 L126 972Z", "#B8A48A") + path("M420 380 L604 380 L604 900 L420 900Z", "#E9D9C0"))
    z.append(path("M126 300 L420 400 M898 300 L604 400", stroke=C, sw=2, op=0.3))
    z.append(group(circle(-40, 0, 26, "none", C, 5) + circle(40, 0, 26, "none", C, 5) + path("M-40 0 L-10 -30 L30 -30 L40 0 M-10 -30 L0 0", stroke=O, sw=5), 512, 880, 1.2))
    z.append(person(512, 850, 0.6, R))
    z.append(leaf(400, 400, 0.6, 30) + leaf(630, 400, 0.6, -30))
    return "".join(z)


@scene("j6.u05")
def imogiri():
    """Pasarean Imogiri: the long stone stairway up the hill to the royal graves."""
    z = [sky(sun=(250, 240), r=60)]
    z.append(path("M126 972 L126 520 Q512 260 898 520 L898 972Z", G))
    z.append(path("M126 972 L126 700 Q300 600 420 520 L604 520 Q720 600 898 700 L898 972Z", "#3F7A33", op=0.6))
    for k in range(10):
        x = 160 + k * 80
        z.append(poly([(x, 600 + abs(5 - k) * 40), (x - 26, 680 + abs(5 - k) * 40), (x + 26, 680 + abs(5 - k) * 40)], "#2F5E2A", C, 3))
    for k in range(28):
        t = k / 27
        y = 972 - t * 560
        w = 280 - t * 200
        z.append(rect(512 - w / 2, y - 20, w, 20, S if k % 2 else "#C8B898", 1, C, 2))
    z.append(paduraksa(512, 420, 120, 200))
    z.append(person(470, 860, 0.7, "#4A5A7A", scarf=C) + person(540, 760, 0.55, P, head=C))
    return "".join(z)


@scene("j6.u06")
def kitab_pegon():
    """A pesantren manuscript in Pegon script, open on its wooden book stand."""
    z = [rect(X0, Y0, W, H, "#5A4632")]
    z.append(rect(X0, 760, W, 212, D))
    # the folding book stand (rehal): two crossed boards
    z.append(path("M300 900 L700 600 L740 640 L340 940Z", "#8A5A3C", C, 6) + path("M724 900 L324 600 L284 640 L684 940Z", "#7A4A2C", C, 6))
    # the open book
    z.append(path("M200 560 Q360 520 512 580 L512 820 Q360 760 200 800Z", P, C, 6) + path("M824 560 Q664 520 512 580 L512 820 Q664 760 824 800Z", P, C, 6))
    for k in range(7):
        y = 600 + k * 28
        z.append(path(f"M240 {y - 8 + k * 0} q20 -10 40 0 t40 0 t40 0 t40 0 t40 0 t40 0", stroke=C, sw=3, op=0.75))
        z.append(path(f"M552 {y - 8} q20 -10 40 0 t40 0 t40 0 t40 0 t40 0 t40 0", stroke=C, sw=3, op=0.75))
        for j in range(3):
            z.append(circle(270 + j * 80 + k * 6, y + 6, 3, C, op=0.7) + circle(580 + j * 80 + k * 6, y + 6, 3, C, op=0.7))
    z.append(path("M512 580 L512 820", stroke=C, sw=4))
    # an oil lamp
    z.append(ellipse(800, 450, 40, 18, O, C, 4) + path("M800 432 q-14 -30 0 -60 q14 30 0 60Z", "#F2C94C", C, 3) + circle(800, 400, 70, "#F2C94C", op=0.15))
    return "".join(z)


# ---------------------------------------------------------------- frame


def frame(body: str) -> str:
    """The approved card frame: paper, two thin borders, kawung marks, the scene panel."""
    decor = [rect(0, 0, 1024, 1536, P)]
    decor.append(rect(44, 42, 936, 1452, "none", rx=42, stroke=I, sw=4, op=0.32))
    decor.append(rect(66, 64, 892, 1406, "none", rx=28, stroke=I, sw=2, op=0.12))
    for y in (201, 344, 487, 630, 773, 916):
        decor.append(kawung(82, y, 0.55, 0.18))
        decor.append(kawung(942, y, 0.55, 0.18))
    for x in (197, 359, 521, 683, 845):
        decor.append(kawung(x, 80, 0.54, 0.18))
    decor.append('<clipPath id="sceneclip"><rect x="126" y="122" width="772" height="850" rx="32"/></clipPath>')
    decor.append('<g clip-path="url(#sceneclip)">' + body + "</g>")
    decor.append(rect(126, 122, 772, 850, "none", rx=32, stroke=I, sw=4, op=0.55))
    return '<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1536" viewBox="0 0 1024 1536">' + "".join(decor) + "</svg>"


def texture(master: Image.Image, seed: int) -> Image.Image:
    """Deterministic paper tooth, subtle enough not to turn the artwork noisy."""
    r = random.Random(93407 + seed * 19)
    layer = Image.new("RGBA", master.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer, "RGBA")
    for _ in range(6200):
        x = r.randrange(64, 960)
        y = r.randrange(75, 1459)
        col = (43, 58, 112, r.choice([3, 4, 5])) if r.random() > 0.3 else (34, 32, 30, r.choice([2, 3, 4]))
        if r.random() < 0.20:
            draw.line((x, y, x + r.randint(2, 11), y + r.choice([0, 1, -1])), fill=col, width=1)
        else:
            draw.point((x, y), fill=col)
    return Image.alpha_composite(master.convert("RGBA"), layer).convert("RGB")


def render(unit_id: str) -> Path:
    svg = frame(SCENES[unit_id]())
    png = cairosvg.svg2png(bytestring=svg.encode("utf-8"), output_width=1024, output_height=1536)
    img = Image.open(io.BytesIO(png)).convert("RGBA")
    seed = sum(ord(ch) for ch in unit_id)
    out = DEST / f"{unit_id}.png"
    texture(img, seed).save(out, optimize=True)
    return out


def main() -> None:
    wanted = sys.argv[1:] or list(SCENES)
    unknown = [u for u in wanted if u not in SCENES]
    if unknown:
        raise SystemExit(f"no scene for {', '.join(unknown)}")
    DEST.mkdir(parents=True, exist_ok=True)
    for unit in wanted:
        print("drew", render(unit).relative_to(ROOT))


if __name__ == "__main__":
    main()
