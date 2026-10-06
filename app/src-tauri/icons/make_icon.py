"""Writes icon.ico and icon.png from the I-Simpa Night Mode logo: the red tile with sound waves spreading
from one corner, made for the April release (main: isimpa_neon_*.png) and kept as the app's logo
(decision 62, Burhan 2026-10-06: "keep the logo as the i simpa red one we made a long time ago").

The red drawing is 128 px (logo-128.png, main's isimpa_neon_logo.png); April's 32 and 64 px files are a
cyan variant, not this logo, so every size is scaled from the red one (256 up, the rest down).

Run: python app/src-tauri/icons/make_icon.py
"""
from pathlib import Path

from PIL import Image

here = Path(__file__).resolve().parent
logo = Image.open(here / "logo-128.png").convert("RGBA")


def at(size: int) -> Image.Image:
    return logo if size == 128 else logo.resize((size, size), Image.LANCZOS)


sizes = [16, 24, 32, 48, 64, 128, 256]
images = [at(s) for s in sizes]
images[-1].save(here / "icon.ico", sizes=[(s, s) for s in sizes], append_images=images[:-1])
images[-1].save(here / "icon.png")
print("wrote", here / "icon.ico")
