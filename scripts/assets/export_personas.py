#!/usr/bin/env python3
"""Import and export the persona portraits (docs/VISUAL-ASSETS.md, prompt 6a-6e).

Masters are assets/src/personas/<cast id>[-<expression>].webp, 1024x1024 at
high quality (Codex draws larger PNGs; keeping them as PNG would add ~2 MB
each to the repo). Names must match assets/portraits.yaml.

    pip install Pillow PyYAML
    # 1. bring a reviewed Codex batch in (square PNGs, at least 1024 px):
    python3 scripts/assets/export_personas.py --import ~/Downloads/codex-6a
    # 2. (re)write the app exports and the contact sheet:
    python3 scripts/assets/export_personas.py

Step 2 writes assets/export/personas/<file>.webp (256 px; the app crops it to
a circle or a rounded square and build.rs embeds every file) and
assets/src/personas/cast-sheet.webp.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import yaml
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "personas"
DEST = ROOT / "assets" / "export" / "personas"
EXPRESSIONS = ("listening", "laughing", "thinking", "explaining")
MASTER, EXPORT = 1024, 256


def entries() -> dict[str, dict]:
    return {p["id"]: p for p in yaml.safe_load((ROOT / "assets" / "portraits.yaml").read_text())["portraits"]}


def name_problem(stem: str, known: dict[str, dict]) -> str | None:
    match = re.fullmatch(r"(.+?)(?:-(" + "|".join(EXPRESSIONS) + r"))?", stem)
    cid, expr = match.group(1), match.group(2)
    if cid not in known:
        return f"{stem}: {cid} is not in assets/portraits.yaml"
    if expr and not known[cid].get("expressions"):
        return f"{stem}: {cid} has no expressions in assets/portraits.yaml"
    return None


def import_batch(folder: Path) -> None:
    known = entries()
    files = sorted(folder.rglob("*.png"))
    errors = [p for f in files if (p := name_problem(f.stem, known))]
    for f in files:
        w, h = Image.open(f).size
        if w != h or w < MASTER:
            errors.append(f"{f.name}: {w}x{h}, needs a square of at least {MASTER} px")
    if not files:
        errors.append(f"no PNGs in {folder}")
    if errors:
        sys.exit("\n".join(errors))
    SRC.mkdir(parents=True, exist_ok=True)
    for f in files:
        img = Image.open(f).convert("RGB").resize((MASTER, MASTER), Image.LANCZOS)
        img.save(SRC / f"{f.stem}.webp", "WEBP", quality=95, method=6)
    print(f"Imported {len(files)} portraits into {SRC.relative_to(ROOT)}")


def export() -> None:
    known = entries()
    masters = sorted(p for p in SRC.glob("*.webp") if p.stem != "cast-sheet")
    errors = [p for m in masters if (p := name_problem(m.stem, known))]
    errors += [f"{m.name}: not {MASTER}x{MASTER}" for m in masters if Image.open(m).size != (MASTER, MASTER)]
    if errors:
        sys.exit("\n".join(errors))
    DEST.mkdir(parents=True, exist_ok=True)
    keep = set()
    for m in masters:
        out = DEST / f"{m.stem}.webp"
        Image.open(m).convert("RGB").resize((EXPORT, EXPORT), Image.LANCZOS).save(out, "WEBP", quality=84, method=6)
        keep.add(out.name)
    for old in DEST.glob("*.webp"):
        if old.name not in keep:
            old.unlink()
    if masters:
        # One row per character, in portraits.yaml order: neutral, then expressions.
        stems = {m.stem for m in masters}
        rows = [[s for s in (cid, *(f"{cid}-{e}" for e in EXPRESSIONS)) if s in stems] for cid in known]
        rows = [r for r in rows if r]
        cell = 200
        sheet = Image.new("RGB", (5 * cell, len(rows) * cell), "#F5EFE3")
        for y, row in enumerate(rows):
            for x, stem in enumerate(row):
                img = Image.open(SRC / f"{stem}.webp").convert("RGB").resize((cell - 4, cell - 4), Image.LANCZOS)
                sheet.paste(img, (x * cell + 2, y * cell + 2))
        sheet.save(SRC / "cast-sheet.webp", "WEBP", quality=80)
    print(f"Exported {len(masters)} portraits to {DEST.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--import", dest="folder", type=Path, help="a folder of reviewed Codex PNGs to bring in first")
    args = parser.parse_args()
    if args.folder:
        import_batch(args.folder)
    export()


if __name__ == "__main__":
    main()
