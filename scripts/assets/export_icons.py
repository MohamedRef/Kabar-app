#!/usr/bin/env python3
from __future__ import annotations

import platform
import subprocess
from pathlib import Path
from typing import Iterable

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "assets" / "src" / "icon"
OUT = ROOT / "assets" / "export"
ANDROID_RES = OUT / "android" / "res"
MACOS_DIR = OUT / "macos"
WEB_DIR = OUT / "web"
PREVIEW_PATH = OUT / "preview.png"

ADAPTIVE_SIZES = {
    "mipmap-mdpi": 108,
    "mipmap-hdpi": 162,
    "mipmap-xhdpi": 216,
    "mipmap-xxhdpi": 324,
    "mipmap-xxxhdpi": 432,
}
LEGACY_SIZES = {
    "mipmap-mdpi": 48,
    "mipmap-hdpi": 72,
    "mipmap-xhdpi": 96,
    "mipmap-xxhdpi": 144,
    "mipmap-xxxhdpi": 192,
}
NOTIFICATION_SIZES = {
    "drawable-mdpi": 24,
    "drawable-hdpi": 36,
    "drawable-xhdpi": 48,
    "drawable-xxhdpi": 72,
    "drawable-xxxhdpi": 96,
}
ICONSET = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]


def ensure_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def load_rgba(path: Path) -> Image.Image:
    return Image.open(path).convert("RGBA")


def save(img: Image.Image, path: Path) -> None:
    ensure_dir(path.parent)
    img.save(path)


def resized(img: Image.Image, size: int | tuple[int, int]) -> Image.Image:
    if isinstance(size, int):
        size = (size, size)
    return img.resize(size, Image.LANCZOS)


def circular_mask(size: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.ellipse((0, 0, size - 1, size - 1), fill=255)
    return mask


def squircle_mask(size: int, radius: float | None = None) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    inset = round(size * 0.02)
    if radius is None:
        radius = size * 0.225
    draw.rounded_rectangle((inset, inset, size - 1 - inset, size - 1 - inset), radius=radius, fill=255)
    return mask


def apply_mask(img: Image.Image, mask: Image.Image) -> Image.Image:
    masked = img.copy()
    masked.putalpha(mask)
    return masked


def alpha_bbox(img: Image.Image, threshold: int = 1) -> tuple[int, int, int, int]:
    alpha = img.getchannel("A")
    bbox = alpha.point(lambda p: 255 if p > threshold else 0).getbbox()
    if bbox is None:
        return (0, 0, img.width, img.height)
    return bbox


def fit_transparent_to_circle(img: Image.Image, canvas_size: int, diameter: int) -> Image.Image:
    bbox = alpha_bbox(img)
    cropped = img.crop(bbox)
    alpha = cropped.getchannel("A")
    pix = alpha.load()
    xs: list[int] = []
    ys: list[int] = []
    for y in range(cropped.height):
        for x in range(cropped.width):
            if pix[x, y] > 1:
                xs.append(x)
                ys.append(y)
    if not xs:
        return Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    cx = (cropped.width - 1) / 2.0
    cy = (cropped.height - 1) / 2.0
    max_r = max((((x - cx) ** 2 + (y - cy) ** 2) ** 0.5) for x, y in zip(xs, ys))
    scale = (diameter / 2.0) / max_r
    new_w = max(1, round(cropped.width * scale))
    new_h = max(1, round(cropped.height * scale))
    scaled = cropped.resize((new_w, new_h), Image.LANCZOS)
    out = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    out.alpha_composite(scaled, ((canvas_size - new_w) // 2, (canvas_size - new_h) // 2))
    return out


def fit_to_box(img: Image.Image, size: int, padding: float) -> Image.Image:
    """Crop to the visible glyph and scale it to fill `size` minus `padding` on each side."""
    cropped = img.crop(alpha_bbox(img))
    inner = size - 2 * round(size * padding)
    scale = inner / max(cropped.width, cropped.height)
    new_w = max(1, round(cropped.width * scale))
    new_h = max(1, round(cropped.height * scale))
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.alpha_composite(cropped.resize((new_w, new_h), Image.LANCZOS), ((size - new_w) // 2, (size - new_h) // 2))
    return out


def make_macos_master(full_img: Image.Image) -> Image.Image:
    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    body_size = 824
    radius = 185
    body = resized(full_img, body_size)
    mask = Image.new("L", (body_size, body_size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, body_size - 1, body_size - 1), radius=radius, fill=255)
    x = (1024 - body_size) // 2
    y = (1024 - body_size) // 2
    canvas.paste(body, (x, y), mask)
    return canvas


def solid_from_alpha(img: Image.Image, rgb: tuple[int, int, int]) -> Image.Image:
    out = Image.new("RGBA", img.size, rgb + (0,))
    out.putalpha(img.getchannel("A"))
    return out


def write_android_xml(anydpi_dir: Path) -> None:
    ensure_dir(anydpi_dir)
    xml = """<?xml version=\"1.0\" encoding=\"utf-8\"?>
<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">
    <background android:drawable=\"@mipmap/ic_launcher_background\" />
    <foreground android:drawable=\"@mipmap/ic_launcher_foreground\" />
    <monochrome android:drawable=\"@mipmap/ic_launcher_monochrome\" />
</adaptive-icon>
"""
    (anydpi_dir / "ic_launcher.xml").write_text(xml, encoding="utf-8")
    (anydpi_dir / "ic_launcher_round.xml").write_text(xml, encoding="utf-8")


def build_preview(full_img: Image.Image, macos_master: Image.Image, notif_icon: Image.Image) -> None:
    bg = (245, 239, 227, 255)
    canvas = Image.new("RGBA", (2300, 950), bg)
    draw = ImageDraw.Draw(canvas)

    def panel(box, outline=(34, 32, 30, 70)):
        draw.rounded_rectangle(box, radius=24, outline=outline, width=2, fill=(255, 255, 255, 30))

    panel((24, 24, 850, 430))
    panel((880, 24, 2276, 430))
    panel((24, 470, 420, 926))

    # Android masks: square, circle, squircle
    base = resized(full_img, 260)
    circle = apply_mask(base, circular_mask(260))
    squircle = apply_mask(base, squircle_mask(260))
    square = base
    for x, icon in zip((70, 320, 570), (square, circle, squircle)):
        canvas.alpha_composite(icon, (x, 97))

    # macOS icons: 1024/128/32/16 represented at varied display sizes
    specs = [
        (macos_master, 260, (940, 97)),
        (resized(macos_master, 128), 128, (1285, 163)),
        (resized(macos_master, 32), 80, (1505, 187)),
        (resized(macos_master, 16), 56, (1685, 199)),
    ]
    for img, shown_size, pos in specs:
        shown = resized(img, shown_size)
        canvas.alpha_composite(shown, pos)

    # Notification icon on dark background
    dark_tile = Image.new("RGBA", (280, 280), (34, 32, 30, 255))
    tile_icon = resized(notif_icon, 170)
    dark_tile.alpha_composite(tile_icon, ((280 - 170) // 2, (280 - 170) // 2))
    canvas.alpha_composite(dark_tile, (82, 558))

    save(canvas, PREVIEW_PATH)


def main() -> None:
    ensure_dir(OUT)
    background = load_rgba(SRC / "background.png")
    foreground = load_rgba(SRC / "foreground.png")
    monochrome = load_rgba(SRC / "monochrome.png")
    full = load_rgba(SRC / "full.png")

    # Android adaptive icon layers
    for dirname, px in ADAPTIVE_SIZES.items():
        d = ANDROID_RES / dirname
        save(resized(background, px), d / "ic_launcher_background.png")
        save(resized(foreground, px), d / "ic_launcher_foreground.png")
        save(resized(monochrome, px), d / "ic_launcher_monochrome.png")

    write_android_xml(ANDROID_RES / "mipmap-anydpi-v26")

    # Android legacy launcher icons
    for dirname, px in LEGACY_SIZES.items():
        d = ANDROID_RES / dirname
        square = resized(full, px)
        round_icon = apply_mask(square, circular_mask(px))
        save(square, d / "ic_launcher.png")
        save(round_icon, d / "ic_launcher_round.png")

    # Notification icons: glyph fills the 24dp grid with 2dp padding (Material spec)
    for dirname, px in NOTIFICATION_SIZES.items():
        d = ANDROID_RES / dirname
        save(fit_to_box(monochrome, px, 2 / 24), d / "ic_stat_kabar.png")

    # Splash icon, 960x960 with emblem inside central 2/3 circle
    splash = fit_transparent_to_circle(foreground, 960, int(round(960 * 2 / 3)))
    save(splash, ANDROID_RES / "drawable" / "splash_icon.png")

    # macOS iconset
    iconset_dir = MACOS_DIR / "Kabar.iconset"
    ensure_dir(iconset_dir)
    macos_master = make_macos_master(full)
    for filename, size in ICONSET:
        save(resized(macos_master, size), iconset_dir / filename)

    # Build icns on macOS, otherwise write README
    if platform.system() == "Darwin":
        try:
            subprocess.run(["iconutil", "-c", "icns", str(iconset_dir), "-o", str(MACOS_DIR / "Kabar.icns")], check=True)
        except Exception as exc:  # pragma: no cover
            (MACOS_DIR / "README.txt").write_text(
                f"Run on macOS: iconutil -c icns Kabar.iconset -o Kabar.icns\nGeneration note: {exc}\n",
                encoding="utf-8",
            )
    else:
        (MACOS_DIR / "README.txt").write_text(
            "Run on macOS: iconutil -c icns Kabar.iconset -o Kabar.icns\n",
            encoding="utf-8",
        )

    # Menu bar templates (black silhouette with alpha)
    # Glyph fills the 18pt template with 1pt padding
    menubar = solid_from_alpha(monochrome, (0, 0, 0))
    save(fit_to_box(menubar, 18, 1 / 18), MACOS_DIR / "menubar-template.png")
    save(fit_to_box(menubar, 36, 1 / 18), MACOS_DIR / "menubar-template@2x.png")

    # Web
    save(resized(full, 192), WEB_DIR / "icon-192.png")
    save(resized(full, 512), WEB_DIR / "icon-512.png")
    maskable = resized(full, 512)  # emblem already fits safely inside the central 80%
    save(maskable, WEB_DIR / "maskable-512.png")
    save(resized(full, 32), WEB_DIR / "favicon-32.png")

    # Preview
    notif_preview = fit_to_box(monochrome, 180, 2 / 24)
    build_preview(full, macos_master, notif_preview)


if __name__ == "__main__":
    main()
