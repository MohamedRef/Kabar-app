#!/usr/bin/env python3
"""Write the Codex portrait prompts (docs/VISUAL-ASSETS.md, prompt 6a-6e).

Each prompt is self-contained and built from content/cast.yaml (who the
character is) and assets/portraits.yaml (how they look). Files are named by
the cast id, so a portrait can never drift from the character it shows.

    python3 scripts/assets/portrait_prompts.py     # writes assets/portrait-prompts.md
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
CAST = ROOT / "content" / "cast.yaml"
PORTRAITS = ROOT / "assets" / "portraits.yaml"
OUT = ROOT / "assets" / "portrait-prompts.md"
LEARNER = "mohamed"

BATCHES = {
    "6a": "the people the learner talks to: the coach, the chat partners and the two Radio hosts",
    "6b": "the cast of the Sinetron, a serialized story about a warung family in Madiun and their friends in Cairo",
    "6c": "the people in the Indonesian lessons, from Jakarta to Papua",
    "6d": "the people in the Javanese lessons and the politeness game, from Surabaya to Kotagede",
    "6e": "the people in Kabar Pagi, short morning stories set in Yogyakarta",
}

EXPRESSIONS = ["listening", "laughing", "thinking", "explaining"]

STYLE = """\
HOW TO GENERATE (strict):
- Use your image generation tool. Do NOT draw the portraits with code (no SVG,
  Pillow, Cairo or canvas drawing).
- Generate EACH image as its own separate request, directly at 1024x1024. Never
  generate a sheet, grid or collage and split it. Never upscale a smaller image; only
  crop, never enlarge. No post-processing recolour, hue shift or filter.

STYLE (every portrait):
- Calm, warm, grown-up illustration. Flat vector-like shapes, soft layered depth,
  gentle paper grain, warm soft light. Not photorealistic, not anime, not cartoonish,
  no glossy 3D.
- Palette led by: Merapi charcoal #22201E, sawah green #4F8A3C, Bromo orange #EE8A5A,
  Bromo pink #E9A3A0, batik indigo #2B3A70, warm paper #F5EFE3, plus natural skin and
  hair tones.
- Head and shoulders, three-quarter view, friendly natural expression, eyes toward the
  viewer. Face centred with room around it so a circle crop never cuts the head.
- Drawn with dignity and respect: realistic proportions, no caricature, no exaggerated
  features, no stereotypes. Must not resemble any real person, celebrity or existing
  character. Do not imitate any named artist.
- Background: a soft, blurred hint of the setting, in the palette.
- If assets/src/personas/ already holds approved portraits, match their hand, light
  and framing exactly, so every batch looks like one cast.

HARD RULES: no text, letters, numbers, logos, name tags or cards with writing,
watermarks or signatures. Square 1024x1024 PNG."""


def load() -> tuple[dict, list]:
    cast = {c["id"]: c for c in yaml.safe_load(CAST.read_text())["characters"]}
    portraits = yaml.safe_load(PORTRAITS.read_text())["portraits"]
    ids = [p["id"] for p in portraits]
    errors = [f"{i}: listed twice" for i in sorted({i for i in ids if ids.count(i) > 1})]
    errors += [f"{i}: not in content/cast.yaml" for i in ids if i not in cast]
    errors += [f"{i}: no portrait entry" for i in cast if i not in ids and i != LEARNER]
    errors += [f"{p['id']}: unknown batch {p['batch']}" for p in portraits if p["batch"] not in BATCHES]
    if errors:
        sys.exit("assets/portraits.yaml:\n  " + "\n  ".join(errors))
    return cast, portraits


def prompt(batch: str, cast: dict, people: list) -> str:
    lines = [
        f'Create character portraits for "Kabar", an Indonesian + Javanese learning app.',
        f"This batch is {BATCHES[batch]}.",
        "These are ORIGINAL fictional characters. Draw all of them in this one session",
        "with the same hand so they look like one cast.",
        "",
        STYLE,
        "",
        "CHARACTERS. Each file is named by the character's id; never rename a file.",
    ]
    for p in people:
        c = cast[p["id"]]
        lines.append(f"- {p['id']}.png ({c['name']}, {c['home']}): {p['look']}. Background: {p['setting']}.")
        about = re.sub(r"\s*\([^)]*\)", "", c["about"])  # drop notes like "(renamed from …)"
        lines.append(f"  Who they are: {about}")
    expressive = [p["id"] for p in people if p.get("expressions")]
    if expressive:
        names = ", ".join(expressive)
        lines += [
            "",
            f"THEN, for {names}: 4 extra expressions each, with the SAME face, hair,",
            "clothes, framing and background: <id>-listening.png, <id>-laughing.png,",
            "<id>-thinking.png, <id>-explaining.png (a light hand gesture). Keep the identity",
            "consistent; regenerate any that drift.",
        ]
    count = len(people) + 4 * len(expressive)
    lines += [
        "",
        f"SAVE: all {count} masters to assets/src/personas/ with exactly the file names above.",
        "Nothing else: Kabar's own script (scripts/assets/export_personas.py) makes the",
        "web sizes and the cast sheet.",
        "",
        "Do not commit. List the files when done, and confirm each image was generated",
        "natively at 1024x1024 with the image tool.",
    ]
    return "\n".join(lines)


def main() -> None:
    cast, portraits = load()
    out = [
        "# Portrait prompts (Codex, prompt 6a-6e)\n",
        "Generated from `content/cast.yaml` and `assets/portraits.yaml` by "
        "`scripts/assets/portrait_prompts.py`; do not edit by hand. Send one batch at a "
        "time; each is self-contained.\n",
    ]
    for batch in BATCHES:
        people = [p for p in portraits if p["batch"] == batch]
        out.append(f"\n## Prompt {batch} ({len(people)} characters)\n\n```text\n{prompt(batch, cast, people)}\n```\n")
    OUT.write_text("".join(out))
    print(f"wrote {OUT.relative_to(ROOT)}: {len(portraits)} characters in {len(BATCHES)} batches")


if __name__ == "__main__":
    main()
