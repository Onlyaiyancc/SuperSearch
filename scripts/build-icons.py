from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parent.parent
target = root / 'src-tauri/icons'
source = root / 'public/supersearch-icon-source.png'
image = Image.open(source).convert('RGBA')
# 仅裁去透明留白并缩放；品牌图形直接来自已选方案的生成素材。
bounds = image.getchannel('A').point(lambda value: 255 if value > 16 else 0).getbbox()
assert bounds, 'Icon source must contain visible pixels.'
art = image.crop(bounds)
canvas = Image.new('RGBA', (1024, 1024))
art.thumbnail((960, 960), Image.Resampling.LANCZOS)
canvas.alpha_composite(art, ((1024 - art.width) // 2, (1024 - art.height) // 2))
icon = canvas.resize((256, 256), Image.Resampling.LANCZOS)
icon.save(target / 'icon.png')
icon.save(root / 'public/supersearch.png')
icon.save(target / 'icon.ico', sizes=[(size, size) for size in [16, 20, 24, 32, 40, 48, 64, 128, 256]])
tray = canvas.resize((32, 32), Image.Resampling.LANCZOS)
(target / 'tray.rgba').write_bytes(tray.tobytes())
assert len((target / 'tray.rgba').read_bytes()) == 4096
assert icon.getpixel((0, 0))[3] == 0
assert {16, 32, 48, 256}.issubset({size[0] for size in Image.open(target / 'icon.ico').ico.sizes()})
sheet = Image.new('RGB', (640, 240), '#f2f4f7')
preview = ImageDraw.Draw(sheet)
for index, size in enumerate([16, 24, 32, 48, 64, 128]):
    sample = canvas.resize((size, size), Image.Resampling.LANCZOS)
    x = [30, 94, 162, 238, 328, 444][index]
    sheet.paste(sample, (x, 120 - size // 2), sample)
    preview.text((x, 195), str(size) + ' px', fill='#59616c')
preview_directory = root / 'test-results/icons'
preview_directory.mkdir(parents=True, exist_ok=True)
sheet.save(preview_directory / 'App-Icon-Preview.png')
print('Generated Silver icon: PNG, multi-resolution ICO, tray RGBA and size preview.')
