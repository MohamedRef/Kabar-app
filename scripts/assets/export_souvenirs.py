#!/usr/bin/env python3
"""Export the souvenir card masters (docs/VISUAL-ASSETS.md, prompt 4b).

Masters: assets/src/souvenirs/<name>.png, generated natively at 1024x1536.
Writes assets/export/souvenirs/<name>.webp (768x1152, quality 85) and
<name>-thumb.webp (256x384), plus assets/src/souvenirs/contact-sheet.png to
check that the cards still look like one set. Cards whose master is not in
the repo yet are listed and skipped.
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "souvenirs"
OUT = ROOT / "assets" / "export" / "souvenirs"

NAMES = [
    "batik",
    "wayang",
    "gamelan",
    "rendang",
    "angklung",
    "kawung",
    "becak",
    "kopi-tubruk",
    "sate",
    "tongkonan",
    "rumah-gadang",
    "raja-ampat",
    "kotagede-silver",
]
MASTER_SIZE = (1024, 1536)
CARD_SIZE = (768, 1152)
THUMB_SIZE = (256, 384)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    present, missing = [], []
    for name in NAMES:
        path = SRC / f"{name}.png"
        if not path.exists():
            missing.append(name)
            continue
        img = Image.open(path).convert("RGB")
        if img.size != MASTER_SIZE:
            raise SystemExit(f"{path.name} is {img.size}, expected a native {MASTER_SIZE} master")
        img.resize(CARD_SIZE, Image.LANCZOS).save(OUT / f"{name}.webp", "WEBP", quality=85, method=6)
        img.resize(THUMB_SIZE, Image.LANCZOS).save(OUT / f"{name}-thumb.webp", "WEBP", quality=85, method=6)
        present.append(img)

    cols, pad = 5, 16
    rows = max(1, -(-len(present) // cols))
    sheet = Image.new(
        "RGB",
        (cols * (THUMB_SIZE[0] + pad) + pad, rows * (THUMB_SIZE[1] + pad) + pad),
        (34, 32, 30),
    )
    for i, img in enumerate(present):
        x = pad + (i % cols) * (THUMB_SIZE[0] + pad)
        y = pad + (i // cols) * (THUMB_SIZE[1] + pad)
        sheet.paste(img.resize(THUMB_SIZE, Image.LANCZOS), (x, y))
    sheet.save(SRC / "contact-sheet.png", optimize=True)

    print(f"Exported {len(present)} of {len(NAMES)} cards to {OUT.relative_to(ROOT)}")
    if missing:
        print("No master yet:", ", ".join(missing))


if __name__ == "__main__":
    main()
