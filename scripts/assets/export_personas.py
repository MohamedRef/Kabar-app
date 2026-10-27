#!/usr/bin/env python3
"""Export the persona portraits (docs/VISUAL-ASSETS.md, prompt 6a-6e).

Masters are assets/src/personas/<cast id>[-<expression>].png at 1024x1024.
Writes assets/export/personas/<file>.webp (1024), <file>-256.webp and
<file>-96.webp (avatars; the app crops them to a circle), and
assets/src/personas/cast-sheet.png. Names must match assets/portraits.yaml.

    pip install Pillow PyYAML
    python3 scripts/assets/export_personas.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "personas"
DEST = ROOT / "assets" / "export" / "personas"
EXPRESSIONS = ("listening", "laughing", "thinking", "explaining")


def main() -> None:
    entries = {p["id"]: p for p in yaml.safe_load((ROOT / "assets" / "portraits.yaml").read_text())["portraits"]}
    masters = sorted(p for p in SRC.glob("*.png") if p.stem != "cast-sheet")
    errors = []
    for m in masters:
        match = re.fullmatch(r"(.+?)(?:-(" + "|".join(EXPRESSIONS) + r"))?", m.stem)
        cid, expr = match.group(1), match.group(2)
        if cid not in entries:
            errors.append(f"{m.name}: {cid} is not in assets/portraits.yaml")
        elif expr and not entries[cid].get("expressions"):
            errors.append(f"{m.name}: {cid} has no expressions in assets/portraits.yaml")
        if Image.open(m).size != (1024, 1024):
            errors.append(f"{m.name}: not 1024x1024")
    if errors:
        sys.exit("\n".join(errors))
    DEST.mkdir(parents=True, exist_ok=True)
    keep = set()
    for m in masters:
        img = Image.open(m).convert("RGB")
        for suffix, size in (("", 1024), ("-256", 256), ("-96", 96)):
            out = DEST / f"{m.stem}{suffix}.webp"
            img.resize((size, size), Image.LANCZOS).save(out, "WEBP", quality=86, method=6)
            keep.add(out.name)
    for old in DEST.glob("*.webp"):
        if old.name not in keep:
            old.unlink()
    if masters:
        neutral = [m for m in masters if m.stem in entries]
        cols = 6
        sheet = Image.new("RGB", (cols * 200, ((len(masters) + cols - 1) // cols) * 200), "#F5EFE3")
        order = [x for n in neutral for x in masters if x.stem == n.stem or x.stem.startswith(n.stem + "-")]
        for i, m in enumerate(order):
            sheet.paste(Image.open(m).convert("RGB").resize((196, 196)), ((i % cols) * 200 + 2, (i // cols) * 200 + 2))
        sheet.save(SRC / "cast-sheet.png")
    print(f"Exported {len(masters)} portraits to {DEST.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
