"""Render the existing RSS brand mark into a multi-resolution ICO (requires Pillow)."""
from pathlib import Path
from PIL import Image, ImageDraw

size = 256
image = Image.new("RGBA", (size, size))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((0, 0, 255, 255), radius=64, fill="#0f8f82")
# Same white dot and quarter-circle waves as the Reader RSS mark.
draw.ellipse((61, 167, 87, 193), fill="white")
for radius in (66, 117):
    draw.arc((74-radius, 180-radius, 74+radius, 180+radius), 270, 360, fill="white", width=15)
image.save(Path(__file__).resolve().parents[1] / "web/public/favicon.ico", sizes=[(16,16),(32,32),(48,48),(64,64)])
