"""Writes icon.ico: a flat Dockyard red-on-black tile. No logo, no wordmark (docs/design/README.md).

Run: python app/src-tauri/icons/make_icon.py
"""
from pathlib import Path

from PIL import Image, ImageDraw

BLACK = (9, 9, 11, 255)  # #09090B, the app background
RED = (224, 32, 46, 255)  # #E0202E, the brand red

here = Path(__file__).resolve().parent
big = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
draw = ImageDraw.Draw(big)
draw.rounded_rectangle((8, 8, 247, 247), radius=40, fill=BLACK)
draw.rounded_rectangle((64, 64, 191, 191), radius=16, fill=RED)
big.save(here / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
big.save(here / "icon.png")
print("wrote", here / "icon.ico")
