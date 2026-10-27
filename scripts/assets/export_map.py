#!/usr/bin/env python3
"""Rebuild Kabar Journey Map WebP exports and the 13-vignette contact sheet.

Requires Pillow: python -m pip install Pillow
Run from any directory: python scripts/assets/export_map.py
All paths are relative to the location of this script inside the project.
"""
from __future__ import annotations

from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "map"
OUT = ROOT / "assets" / "export" / "map"
BACKDROPS = ("paper-light", "paper-dark")
VIGNETTES = (
    "jakarta", "bali", "lombok", "sulawesi", "kalimantan", "sumatra",
    "raja-ampat", "surabaya", "madiun", "solo", "yogyakarta", "semarang",
    "kotagede",
)
RESAMPLE = Image.Resampling.LANCZOS


def check_master(name: str, transparent: bool) -> Image.Image:
    file = SRC / f"{name}.png"
    image = Image.open(file).convert("RGBA" if transparent else "RGB")
    if image.size != (1024, 1024):
        raise ValueError(f"Master {file} must be 1024x1024, got {image.size}")
    if transparent and image.getchannel("A").getextrema() == (255, 255):
        raise ValueError(f"Vignette {file} has no transparent background")
    return image


def save_webp(image: Image.Image, name: str, size: int) -> None:
    image.resize((size, size), RESAMPLE).save(
        OUT / f"{name}.webp", format="WEBP", quality=85, method=4
    )


def check_tileable(image: Image.Image, name: str) -> None:
    # The edge pixels must match exactly on both axes before making repeating tiles.
    left = image.crop((0, 0, 1, image.height)).tobytes()
    right = image.crop((image.width - 1, 0, image.width, image.height)).tobytes()
    top = image.crop((0, 0, image.width, 1)).tobytes()
    bottom = image.crop((0, image.height - 1, image.width, image.height)).tobytes()
    if left != right or top != bottom:
        raise ValueError(f"Backdrop {name} has a nonmatching seam")


def export_backdrops() -> Image.Image:
    light = None
    for name in BACKDROPS:
        image = check_master(name, transparent=False)
        check_tileable(image, name)
        save_webp(image, f"{name}-1024", 1024)
        save_webp(image, f"{name}-512", 512)
        if name == "paper-light":
            light = image
    assert light is not None
    return light


def export_vignettes() -> list[Image.Image]:
    masters = []
    for name in VIGNETTES:
        image = check_master(name, transparent=True)
        masters.append(image)
        save_webp(image, name, 512)
        save_webp(image, f"{name}-sm", 256)
    return masters


def create_contact_sheet(paper: Image.Image, vignettes: list[Image.Image]) -> None:
    # Exactly 2x2 repeats of the light paper backdrop, without any map drawing.
    sheet = Image.new("RGBA", (2048, 2048))
    tile = paper.convert("RGBA")
    for y in (0, 1024):
        for x in (0, 1024):
            sheet.alpha_composite(tile, (x, y))

    columns, cell, gap_x, gap_y = 4, 380, 70, 85
    rows = (len(vignettes) + columns - 1) // columns
    start_x = (2048 - (columns * cell + (columns - 1) * gap_x)) // 2
    start_y = (2048 - (rows * cell + (rows - 1) * gap_y)) // 2
    for index, vignette in enumerate(vignettes):
        x = start_x + (index % columns) * (cell + gap_x)
        y = start_y + (index // columns) * (cell + gap_y)
        sheet.alpha_composite(vignette.resize((cell, cell), RESAMPLE), (x, y))
    sheet.convert("RGB").save(SRC / "contact-sheet.png", optimize=True)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    paper = export_backdrops()
    vignettes = export_vignettes()
    create_contact_sheet(paper, vignettes)
    print(f"Exported {len(BACKDROPS)} backdrops and {len(VIGNETTES)} vignettes into {OUT}")


if __name__ == "__main__":
    main()
