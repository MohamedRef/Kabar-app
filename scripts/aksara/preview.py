#!/usr/bin/env python3
"""Draws content/aksara/strokes.json over the font glyphs, numbered in
stroke order with a dot at each start, for reviewing the draft.

Usage: python3 scripts/aksara/preview.py FONT.ttf content/aksara/strokes.json out.png [ids...]
"""

import json
import sys

from PIL import Image, ImageDraw, ImageFont

CELL, SCALE = 330, 0.2  # px per cell, px per 1/1000 em
COLORS = ["#d62828", "#1d4ed8", "#15803d", "#a16207", "#7e22ce", "#0e7490", "#be185d"]


def main():
    font = ImageFont.truetype(sys.argv[1], int(1000 * SCALE), layout_engine=ImageFont.Layout.RAQM)
    label = ImageFont.load_default(size=16)
    data = json.load(open(sys.argv[2], encoding="utf-8"))
    glyphs = [g for g in data["glyphs"] if len(sys.argv) < 5 or g["id"] in sys.argv[4:]]
    # Flow layout: each glyph gets a cell as wide as its strokes.
    widths = [max(x for st in g["strokes"] for x, _ in st) * SCALE + 70 for g in glyphs]
    page_w, place, x, y = 1400, [], 0, 0
    for w in widths:
        if x and x + w > page_w:
            x, y = 0, y + CELL
        place.append((x, y))
        x += max(w, 160)
    im = Image.new("RGB", (page_w, y + CELL + 120), "white")
    d = ImageDraw.Draw(im)
    for g, (cx, cy) in zip(glyphs, place):
        ox, oy = cx + 30, cy + 190
        d.text((ox, oy), g["text"], font=font, fill="#e5e7eb", anchor="ls")
        d.text((ox - 25, oy - 180), g["id"], font=label, fill="black")
        for n, s in enumerate(g["strokes"]):
            c = COLORS[n % len(COLORS)]
            pts = [(ox + x * SCALE, oy + y * SCALE) for x, y in s]
            d.line(pts, fill=c, width=3)
            x0, y0 = pts[0]
            d.ellipse((x0 - 6, y0 - 6, x0 + 6, y0 + 6), fill=c)
            d.text((x0 + 7, y0 - 20), str(n + 1), font=label, fill=c)
            if len(pts) > 1:
                (xa, ya), (xb, yb) = pts[-2], pts[-1]
                d.ellipse((xb - 3, yb - 3, xb + 3, yb + 3), outline=c)
    im.save(sys.argv[3])


if __name__ == "__main__":
    main()
