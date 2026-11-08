#!/usr/bin/env python3
"""Renders the drawn Kabar mark (assets/src/icon/svg/*.svg) to the 1024 px PNG
masters in assets/src/icon/, which export_icons.py then cuts to every size.

    pip install cairosvg pillow
    python3 scripts/assets/render_icon_svgs.py && python3 scripts/assets/export_icons.py
"""
from __future__ import annotations

import io
from pathlib import Path

import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "icon"
MASTER = 1024


def main() -> None:
    for name in ("background", "foreground", "full", "monochrome"):
        png = cairosvg.svg2png(url=str(SRC / "svg" / f"{name}.svg"), output_width=MASTER, output_height=MASTER)
        # No timestamps or text chunks, so re-running gives byte-identical files.
        Image.open(io.BytesIO(png)).convert("RGBA").save(SRC / f"{name}.png", optimize=True)
    print("wrote assets/src/icon/*.png")


if __name__ == "__main__":
    main()
