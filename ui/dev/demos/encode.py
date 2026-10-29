"""A take from record.mjs -> a looping animated WebP at its real pace: frames closer than 1/30 s merged (at most
30 fps), cropped to the part that matters, made 900 px wide. python encode.py <frames> <out.webp> [x y w h, in CSS px]"""
import json, os, sys
from PIL import Image

src, out = sys.argv[1], sys.argv[2]
crop = [float(v) * 1.5 for v in sys.argv[3:7]] if len(sys.argv) >= 7 else None
meta = json.load(open(os.path.join(src, 'frames.json')))
frames, end = meta['frames'], meta['end']
kept = []
for f in frames:
    if kept and f['t'] - kept[-1]['t'] < 1 / 30:
        kept[-1] = {**f, 't': kept[-1]['t']}
    else:
        kept.append(dict(f))
imgs, durs = [], []
for i, f in enumerate(kept):
    nxt = kept[i + 1]['t'] if i + 1 < len(kept) else end
    im = Image.open(os.path.join(src, f['file'])).convert('RGB')
    if crop:
        im = im.crop((round(crop[0]), round(crop[1]), round(crop[0] + crop[2]), round(crop[1] + crop[3])))
    imgs.append(im.resize((900, round(im.height * 900 / im.width)), Image.LANCZOS))
    durs.append(max(20, round((nxt - f['t']) * 1000)))
imgs[0].save(out, save_all=True, append_images=imgs[1:], duration=durs, loop=0, quality=78, method=6)
print(out, len(imgs), 'frames', round(sum(durs) / 1000, 1), 's', os.path.getsize(out) // 1024, 'KB')
