"""A take from record.mjs -> a looping animated WebP at its real pace: frames closer than 1/30 s merged (at most
30 fps), cropped to the part that matters, made 900 px wide. python encode.py <frames> <out.webp> [x y w h, in CSS px]
An out.gif instead (the README's tour): 960 px wide, at most 8 fps, frames that don't change dropped, to stay small."""
import json, os, sys
from PIL import Image

src, out = sys.argv[1], sys.argv[2]
meta = json.load(open(os.path.join(src, 'frames.json')))
frames, end = meta['frames'], meta['end']
gif = out.lower().endswith('.gif')
# The page is 1280 CSS px wide (the tour 1320); the frames may come at that size or bigger (the screen's scale).
scale = Image.open(os.path.join(src, frames[0]['file'])).width / (1320 if gif else 1280)
crop = [float(v) * scale for v in sys.argv[3:7]] if len(sys.argv) >= 7 else None
kept = []
for f in frames:
    if kept and f['t'] - kept[-1]['t'] < (1 / 8 if gif else 1 / 30):
        kept[-1] = {**f, 't': kept[-1]['t']}
    else:
        kept.append(dict(f))
imgs, durs = [], []
for i, f in enumerate(kept):
    nxt = kept[i + 1]['t'] if i + 1 < len(kept) else end
    im = Image.open(os.path.join(src, f['file'])).convert('RGB')
    if crop:
        im = im.crop((round(crop[0]), round(crop[1]), round(crop[0] + crop[2]), round(crop[1] + crop[3])))
    width = 960 if gif else 900
    im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    dur = max(20, round((nxt - f['t']) * 1000))
    if gif and imgs and im.tobytes() == imgs[-1].tobytes():
        durs[-1] += dur
        continue
    imgs.append(im)
    durs.append(dur)
if gif:
    # One palette for each stretch of look-alike frames (a new one when the picture changes a lot, as with a theme), so
    # what didn't move keeps the very same colours and each frame stores only the part that changed.
    from PIL import ImageChops, ImageStat
    far = lambda a, b: sum(ImageStat.Stat(ImageChops.difference(a, b)).mean) / 3 > 12
    small = [im.resize((96, 61)) for im in imgs]
    # A theme fading into the next is a whole new picture every frame: only its end is kept (the GIF stays small).
    def whole(a, b):
        box = ImageChops.difference(a, b).convert('L').point(lambda v: 255 if v > 6 else 0).getbbox()
        return bool(box) and (box[2] - box[0]) * (box[3] - box[1]) > .6 * a.width * a.height
    i = 1
    while i < len(imgs) - 1:
        if whole(small[i - 1], small[i]) and whole(small[i], small[i + 1]):
            durs[i - 1] += durs.pop(i)
            del imgs[i], small[i]
        else:
            i += 1
    runs, start = [], 0
    for i in range(1, len(imgs) + 1):
        if i == len(imgs) or far(small[start], small[i]):
            runs.append((start, i))
            start = i
    pal = []
    for a, b in runs:
        picks = imgs[a:b][::max(1, (b - a) // 6)][:6]
        sheet = Image.new('RGB', (picks[0].width, picks[0].height * len(picks)))
        for k, im in enumerate(picks):
            sheet.paste(im, (0, k * im.height))
        ref = sheet.quantize(colors=255, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
        pal += [im.quantize(palette=ref, dither=Image.Dither.NONE) for im in imgs[a:b]]
    pal[0].save(out, save_all=True, append_images=pal[1:], duration=durs, loop=0, optimize=False, disposal=1)
else:
    imgs[0].save(out, save_all=True, append_images=imgs[1:], duration=durs, loop=0, quality=78, method=6)
print(out, len(imgs), 'frames', round(sum(durs) / 1000, 1), 's', os.path.getsize(out) // 1024, 'KB')
