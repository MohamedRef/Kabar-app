#!/usr/bin/env python3
"""Synthesize Kabar's lesson sounds (SPEC §9; docs/VISUAL-ASSETS.md, prompt 8).

Nothing is downloaded, so there is no licence to track: a gong for the end of
a lesson and a short rising chime ("ding") for a right answer, three
variations of each. The kendang taps were the first right-answer sound; they
stay as alternatives.
Renders 48 kHz mono, normalizes to about -16 LUFS (measured with ffmpeg's
EBU R128 filter), and writes 16-bit WAV to assets/sounds/ (WAV plays in every
web view Kabar runs in) plus an OGG preview of each next to it.

    pip install numpy      # and ffmpeg on PATH
    python3 scripts/assets/synth_sounds.py
"""
from __future__ import annotations

import re
import subprocess
import wave
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / "assets" / "sounds"
RATE = 48_000
TARGET_LUFS = -16.0


def envelope(n: int, attack: float, decay: float) -> np.ndarray:
    t = np.arange(n) / RATE
    rise = np.clip(t / attack, 0, 1) ** 2
    return rise * np.exp(-t / decay)


def gong(base: float, length: float, glide: float, brightness: float, seed: int) -> np.ndarray:
    """A few inharmonic partials with slow decay, a soft attack and a slight glide."""
    rng = np.random.default_rng(seed)
    n = int(length * RATE)
    t = np.arange(n) / RATE
    out = np.zeros(n)
    partials = [(1.0, 1.0, 1.0), (1.48, 0.55, 0.75), (2.03, 0.42 * brightness, 0.55), (2.71, 0.26 * brightness, 0.4), (3.56, 0.12 * brightness, 0.28)]
    for ratio, amp, life in partials:
        f0 = base * ratio
        # pitch sinks a little as the gong settles
        f = f0 * (1 + glide * np.exp(-t / 0.35))
        phase = 2 * np.pi * np.cumsum(f) / RATE + rng.uniform(0, 2 * np.pi)
        beat = 1 + 0.08 * np.sin(2 * np.pi * (0.9 + rng.uniform(0, 0.6)) * t)  # slow shimmer
        out += amp * np.sin(phase) * beat * envelope(n, 0.012, length * 0.32 * life)
    # the mallet: a short, soft thump under the tone
    out += 0.25 * np.sin(2 * np.pi * base * 0.5 * t) * envelope(n, 0.004, 0.08)
    fade = np.ones(n)
    fade[-int(0.3 * RATE):] = np.linspace(1, 0, int(0.3 * RATE))
    return out * fade


def kendang(pitch: float, length: float, noise: float, bend: float, seed: int) -> np.ndarray:
    """A short pitched membrane thump with a little filtered noise."""
    rng = np.random.default_rng(seed)
    n = int(length * RATE)
    t = np.arange(n) / RATE
    f = pitch * (1 + bend * np.exp(-t / 0.02))  # the skin's pitch drops right after the hit
    body = np.sin(2 * np.pi * np.cumsum(f) / RATE) * envelope(n, 0.0015, length * 0.28)
    body += 0.35 * np.sin(2 * np.pi * np.cumsum(f * 1.59) / RATE) * envelope(n, 0.0015, length * 0.12)
    hiss = rng.standard_normal(n)
    k = 24  # a gentle moving-average low-pass keeps the noise warm, not harsh
    hiss = np.convolve(hiss, np.ones(k) / k, mode="same")
    body += noise * hiss * envelope(n, 0.001, 0.012)
    fade = np.ones(n)
    fade[-int(0.02 * RATE):] = np.linspace(1, 0, int(0.02 * RATE))
    return body * fade


def chime(notes: list[float], gap: float, partials: list[tuple[float, float, float]], tail: float, seed: int) -> np.ndarray:
    """Quick rising struck notes (a bell or a saron bar): each note is a few
    partials with a hard attack and a fast decay, the last one rings longer."""
    rng = np.random.default_rng(seed)
    n = int((gap * (len(notes) - 1) + tail) * RATE)
    t = np.arange(n) / RATE
    out = np.zeros(n)
    for i, f0 in enumerate(notes):
        start = int(i * gap * RATE)
        m = n - start
        tt = t[:m]
        life = tail if i == len(notes) - 1 else gap * 2.2
        note = np.zeros(m)
        for ratio, amp, decay in partials:
            note += amp * np.sin(2 * np.pi * f0 * ratio * tt + rng.uniform(0, 2 * np.pi)) * envelope(m, 0.002, life * decay)
        out[start:] += note * (0.8 if i < len(notes) - 1 else 1.0)
    fade = np.ones(n)
    fade[-int(0.05 * RATE):] = np.linspace(1, 0, int(0.05 * RATE))
    return out * fade


def write_wav(path: Path, x: np.ndarray) -> None:
    pcm = (np.clip(x, -1, 1) * 32767).astype("<i2")
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())


def loudness(x: np.ndarray, tmp: Path) -> float:
    """Integrated loudness in LUFS; short sounds are padded so the meter can gate."""
    padded = np.concatenate([x, np.zeros(max(0, RATE - len(x)))])
    write_wav(tmp, padded * 0.5)
    res = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", str(tmp), "-af", "ebur128", "-f", "null", "-"], capture_output=True, text=True)
    value = float(re.findall(r"I:\s+(-?[\d.]+) LUFS", res.stderr)[-1])
    return value - 20 * np.log10(0.5)


def finish(name: str, x: np.ndarray) -> None:
    x = x / np.max(np.abs(x))
    tmp = DEST / f".{name}.tmp.wav"
    gain = 10 ** ((TARGET_LUFS - loudness(x, tmp)) / 20)
    x = x * gain
    peak = np.max(np.abs(x))
    if peak > 0.89:  # keep 1 dB of headroom even if that leaves it a little quieter
        x *= 0.89 / peak
    tmp.unlink(missing_ok=True)
    wav = DEST / f"{name}.wav"
    write_wav(wav, x)
    subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(wav), "-c:a", "libvorbis", "-q:a", "5", str(DEST / f"{name}.ogg")], check=True)
    print(f"{wav.relative_to(ROOT)}: {len(x) / RATE:.2f} s, {loudness(x, tmp):.1f} LUFS")
    tmp.unlink(missing_ok=True)


VARIANTS = {
    "gong-1": lambda: gong(110, 2.6, 0.012, 1.0, 1),  # warm, the classic
    "gong-2": lambda: gong(98, 3.0, 0.018, 0.7, 2),  # deeper and darker
    "gong-3": lambda: gong(130, 2.2, 0.008, 1.2, 3),  # smaller and brighter
    "kendang-1": lambda: kendang(200, 0.15, 0.25, 0.35, 1),  # round "dhung"
    "kendang-2": lambda: kendang(180, 0.17, 0.18, 0.5, 2),  # lower, more bend
    "kendang-3": lambda: kendang(220, 0.13, 0.32, 0.25, 3),  # tighter "tak"
    # right-answer chimes: short, bright, rising
    "ding-1": lambda: chime([784, 1047], 0.09, [(1, 1, 0.35), (2, 0.25, 0.2), (3, 0.08, 0.12)], 0.45, 1),  # soft two-note rise
    "ding-2": lambda: chime([1047, 1568], 0.08, [(1, 1, 0.4), (2.76, 0.3, 0.18), (5.4, 0.12, 0.08)], 0.6, 2),  # sparkly bell, a fifth up
    "ding-3": lambda: chime([587, 660, 880], 0.07, [(1, 1, 0.35), (2.4, 0.35, 0.15), (3.9, 0.12, 0.08)], 0.5, 3),  # saron bars, slendro-ish run
}


def main() -> None:
    DEST.mkdir(parents=True, exist_ok=True)
    for name, make in VARIANTS.items():
        finish(name, make())


if __name__ == "__main__":
    main()
