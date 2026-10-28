"""Rasterize the app's code-native TV mark for the Windows executable."""
from pathlib import Path
from PIL import Image, ImageDraw

base = Path(__file__).resolve().parents[1] / 'src-tauri' / 'icons'
base.mkdir(parents=True, exist_ok=True)
img = Image.new('RGBA', (256, 256), (0, 0, 0, 0))
draw = ImageDraw.Draw(img)
draw.rounded_rectangle((8, 8, 248, 248), radius=58, fill='#1675f9')
draw.rounded_rectangle((48, 63, 208, 169), radius=14, outline='white', width=12)
draw.line((128, 171, 128, 190), fill='white', width=12)
draw.line((94, 195, 162, 195), fill='white', width=12)
img.save(base / 'icon.png')
img.save(base / 'icon.ico', sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
