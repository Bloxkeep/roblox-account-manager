"""Generates the static scenery behind the glass themes (RobloxManager/wwwroot/scene-*.svg).
Deterministic: the same output every run. Usage: python dev/make-scenes.py RobloxManager/wwwroot"""
import random, sys, os

OUT = sys.argv[1]
W, H = 1600, 1000


def svg(body, defs=''):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" '
            f'preserveAspectRatio="xMidYMid slice"><defs>{defs}</defs>{body}</svg>\n')


def grad(id, stops, x2=0, y2=1):
    s = ''.join(f'<stop offset="{o}" stop-color="{c}"{"" if op == 1 else f" stop-opacity=\"{op}\""}/>' for o, c, op in stops)
    return f'<linearGradient id="{id}" x1="0" y1="0" x2="{x2}" y2="{y2}">{s}</linearGradient>'


def blur(id, sd):
    return f'<filter id="{id}" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="{sd}"/></filter>'


def f(v):
    return f'{v:.0f}'


def ridge(rng, base, amp, step=70, jag=1.0):
    """A mountain ridge: random walk around `base`, closed along the bottom."""
    pts, y = [], base
    for x in range(-60, W + 140, step):
        y += rng.uniform(-amp, amp) * jag
        y = max(base - amp * 2.2, min(base + amp * 1.2, y))
        pts.append((x, y))
    d = f'M -60 {H} ' + ' '.join(f'L {f(x)} {f(y)}' for x, y in pts) + f' L {W + 140} {H} Z'
    return d, pts


def smooth(points, bottom=H):
    """Catmull-Rom through the points, as cubic Béziers, closed along the bottom."""
    d = f'M {f(points[0][0])} {bottom} L {f(points[0][0])} {f(points[0][1])} '
    for i in range(len(points) - 1):
        p0 = points[max(i - 1, 0)]; p1 = points[i]; p2 = points[i + 1]; p3 = points[min(i + 2, len(points) - 1)]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        d += f'C {f(c1[0])} {f(c1[1])} {f(c2[0])} {f(c2[1])} {f(p2[0])} {f(p2[1])} '
    return d + f'L {f(points[-1][0])} {bottom} Z'


def hills(rng, base, amp, n=6):
    xs = [-100 + i * (W + 200) / (n - 1) for i in range(n)]
    return smooth([(x, base + rng.uniform(-amp, amp)) for x in xs])


def stars(rng, n, top=0, bottom=620, rmax=1.6):
    return ''.join(f'<circle cx="{f(rng.uniform(0, W))}" cy="{f(rng.uniform(top, bottom))}" r="{rng.uniform(.5, rmax):.1f}" '
                   f'fill="#fff" opacity="{rng.uniform(.35, .95):.2f}"/>' for _ in range(n))


def pines(rng, base, hmin, hmax, color, count, jitter=30):
    out = []
    for i in range(count):
        x = -40 + i * (W + 80) / count + rng.uniform(-14, 14)
        h = rng.uniform(hmin, hmax)
        w = h * .42
        y0 = base + rng.uniform(-jitter, jitter)
        top = y0 - h
        # three stacked tiers
        d = ''
        for k in range(3):
            ty = top + k * h * .26
            tw = w * (.55 + k * .25)
            d += f'M {f(x)} {f(ty)} L {f(x - tw / 2)} {f(ty + h * .42)} L {f(x + tw / 2)} {f(ty + h * .42)} Z '
        out.append(d)
    # The ground under the row hides where the trees stand.
    out.append(f'M -60 {f(base + jitter * .5)} H {W + 60} V {H} H -60 Z')
    return f'<path fill="{color}" d="{" ".join(out)}"/>'


scenes = {}

# Aurora over dark mountains
r = random.Random(1)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>' + stars(r, 170)
body += ('<g filter="url(#b)" opacity=".75" fill="none" stroke-linecap="round">'
         '<path d="M -100 420 C 250 210 520 470 860 300 S 1420 180 1750 330" stroke="url(#ag)" stroke-width="130"/>'
         '<path d="M -100 300 C 300 140 640 330 980 190 S 1450 120 1750 210" stroke="url(#av)" stroke-width="90" opacity=".7"/>'
         '</g>')
for base, amp, color in [(640, 40, '#0e1b30'), (720, 34, '#0a1424'), (820, 26, '#060c17')]:
    body += f'<path fill="{color}" d="{ridge(r, base, amp)[0]}"/>'
defs = grad('sky', [(0, '#040814', 1), (.55, '#0a1a30', 1), (1, '#123050', 1)]) + \
       grad('ag', [(0, '#2ff5a8', 0), (.3, '#2ff5a8', 1), (.7, '#38e1c8', 1), (1, '#38e1c8', 0)], 1, 0) + \
       grad('av', [(0, '#a26bff', 0), (.5, '#a26bff', 1), (1, '#4f8cff', 0)], 1, 0) + blur('b', 38)
scenes['aurora'] = svg(body, defs)

# Sunset over layered hills
r = random.Random(2)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>'
body += '<circle cx="1060" cy="640" r="230" fill="#ffd28a" opacity=".45" filter="url(#b)"/><circle cx="1060" cy="640" r="105" fill="#ffe7b0"/>'
for base, amp, color in [(600, 46, '#b04f7a'), (680, 40, '#7d3570'), (770, 34, '#4b1f55'), (870, 24, '#25103a')]:
    body += f'<path fill="{color}" d="{ridge(r, base, amp, 90)[0]}"/>'
defs = grad('sky', [(0, '#24174a', 1), (.42, '#a53f7a', 1), (.72, '#f08a5d', 1), (1, '#fbc98a', 1)]) + blur('b', 60)
scenes['sunset'] = svg(body, defs)

# Night city skyline
r = random.Random(3)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>' + stars(r, 90, 0, 420, 1.2)
body += '<circle cx="1250" cy="190" r="140" fill="#f4eedc" opacity=".18" filter="url(#b)"/><circle cx="1250" cy="190" r="46" fill="#f4eedc"/>'
for color, hmin, hmax, lit, wop in [('#161a3a', 260, 520, .18, .45), ('#0a0b1d', 140, 380, .28, .9)]:
    x = -20
    rects, windows = [], []
    while x < W + 20:
        w = r.uniform(60, 150)
        h = r.uniform(hmin, hmax)
        rects.append(f'M {f(x)} {H} V {f(H - h)} h {f(w)} V {H} Z')
        for wy in range(int(H - h + 24), H - 30, 26):
            for wx in range(int(x + 12), int(x + w - 14), 18):
                if r.random() < lit:
                    windows.append(f'M {wx} {wy} h 8 v 11 h -8 Z')
        x += w + r.uniform(4, 18)
    body += f'<path fill="{color}" d="{" ".join(rects)}"/><path fill="#ffd27a" opacity="{wop}" d="{" ".join(windows)}"/>'
defs = grad('sky', [(0, '#05071a', 1), (.6, '#18134a', 1), (1, '#45205f', 1)]) + blur('b', 40)
scenes['city'] = svg(body, defs)

# Deep space with a ringed planet
r = random.Random(4)
body = '<rect width="100%" height="100%" fill="#03020b"/>'
body += ('<g filter="url(#b)">'
         '<ellipse cx="420" cy="380" rx="420" ry="220" fill="#5b21b6" opacity=".45"/>'
         '<ellipse cx="760" cy="520" rx="380" ry="170" fill="#1d4ed8" opacity=".35"/>'
         '<ellipse cx="300" cy="620" rx="260" ry="140" fill="#db2777" opacity=".22"/></g>')
body += stars(r, 260, 0, H, 1.8)
body += ('<g transform="rotate(-18 1220 700)">'
         '<ellipse cx="1220" cy="700" rx="330" ry="70" fill="none" stroke="#f6c46b" stroke-opacity=".35" stroke-width="18"/>'
         '<circle cx="1220" cy="700" r="170" fill="url(#pl)"/>'
         '<circle cx="1220" cy="700" r="170" fill="url(#sh)"/>'
         '<path d="M 890 700 A 330 70 0 0 0 1550 700" fill="none" stroke="#f6c46b" stroke-opacity=".55" stroke-width="18"/></g>')
defs = blur('b', 70) + grad('pl', [(0, '#f6b04f', 1), (.5, '#c2622d', 1), (1, '#5a2414', 1)], 0, 1) + \
       '<radialGradient id="sh" cx=".3" cy=".3" r=".9"><stop offset=".45" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".75"/></radialGradient>'
scenes['space'] = svg(body, defs)

# Misty pine forest (light)
r = random.Random(5)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>'
layers = [(560, 120, 180, '#a9bdb4', 34), (660, 160, 240, '#86a196', 30), (780, 210, 320, '#5c7a6d', 26), (920, 260, 400, '#2f4a40', 20)]
for i, (base, hmin, hmax, color, count) in enumerate(layers):
    body += pines(r, base, hmin, hmax, color, count)
    if i < len(layers) - 1:
        body += f'<rect x="0" y="{base - 160}" width="{W}" height="260" fill="url(#fog)"/>'
defs = grad('sky', [(0, '#c3d1ca', 1), (1, '#e8eee9', 1)]) + grad('fog', [(0, '#eef3ef', 0), (.6, '#eef3ef', .75), (1, '#eef3ef', 0)])
scenes['mist'] = svg(body, defs)

# Snowy peaks (light)
r = random.Random(6)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>'
for base, amp, snow, rock in [(470, 70, '#f3f7fc', '#9bb2cc'), (600, 52, '#f7fafd', '#7f98b5')]:
    d, pts = ridge(r, base, amp, 80, 1.0)
    body += f'<path fill="{snow}" d="{d}"/>'
    lower = ' '.join(f'L {f(x)} {f(y + 100 + r.uniform(-8, 14))}' for x, y in pts)
    body += f'<path fill="{rock}" d="M -60 {H} {lower} L {W + 140} {H} Z"/>'
body += f'<path fill="#f6f9fc" d="{hills(r, 820, 30, 5)}"/>'
body += pines(r, 860, 70, 130, '#3e5873', 26, 20)
body += f'<path fill="#fbfdff" d="{hills(r, 900, 18, 5)}"/>'
defs = grad('sky', [(0, '#6fa3d6', 1), (1, '#d7e8f6', 1)])
scenes['snow'] = svg(body, defs)

# Desert dunes (light)
r = random.Random(7)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>'
body += '<circle cx="420" cy="330" r="200" fill="#fff4d6" opacity=".5" filter="url(#b)"/><circle cx="420" cy="330" r="80" fill="#fff6dc"/>'
for base, amp, color in [(600, 50, '#efc082'), (690, 60, '#e2a262'), (790, 55, '#cf8446'), (900, 40, '#b36632')]:
    body += f'<path fill="{color}" d="{hills(r, base, amp, 5)}"/>'
defs = grad('sky', [(0, '#f0b77a', 1), (.6, '#f8d9a8', 1), (1, '#fdebcf', 1)]) + blur('b', 50)
scenes['desert'] = svg(body, defs)

# Green meadow (light)
r = random.Random(8)
body = '<rect width="100%" height="100%" fill="url(#sky)"/>'
for cx, cy, s in [(300, 200, 1.0), (820, 140, .8), (1300, 240, 1.15)]:
    body += '<g fill="#fff" opacity=".92">' + ''.join(
        f'<circle cx="{f(cx + dx * s)}" cy="{f(cy + dy * s)}" r="{f(rr * s)}"/>'
        for dx, dy, rr in [(-70, 10, 42), (-20, -18, 58), (40, -4, 50), (90, 14, 36), (10, 22, 44)]) + '</g>'
for base, amp, color in [(620, 40, '#a8d784'), (700, 46, '#82c365'), (800, 40, '#5fa94d'), (900, 30, '#43903e')]:
    body += f'<path fill="{color}" d="{hills(r, base, amp, 5)}"/>'
flowers = ''.join(f'<circle cx="{f(r.uniform(0, W))}" cy="{f(r.uniform(900, 990))}" r="{r.uniform(3, 5.5):.1f}" fill="{r.choice(["#fff", "#ffe066", "#ff9ec4"])}"/>' for _ in range(90))
body += flowers
defs = grad('sky', [(0, '#6cbbec', 1), (1, '#d5effb', 1)])
scenes['meadow'] = svg(body, defs)

for name, text in scenes.items():
    path = os.path.join(OUT, f'scene-{name}.svg')
    open(path, 'w', encoding='utf-8', newline='\n').write(text)
    print(name, len(text))
