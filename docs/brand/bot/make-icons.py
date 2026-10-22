"""The pictures beside the bot's Roblox update posts (services/DiscordBot/worker/texts.js): a coloured disc with an update arrow
around a mark for what the build means for Multi-Roblox. Drawn four times larger, then scaled down for smooth edges.
Run from anywhere: python make-icons.py (needs Pillow)."""
import math
import os
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
SIZE, K = 256, 4
S = SIZE * K
WHITE = (255, 255, 255, 255)
# Discord's own status colours, a shade deeper than the message accents so white reads on them.
ICONS = {
    'same': ((35, 165, 90), 'check'),        # No change
    'changed': ((224, 145, 22), 'bang'),     # May be blocked, lock renamed
    'gone': ((218, 55, 60), 'cross'),        # Likely blocked
    'first': ((88, 101, 242), 'dot'),        # Watching (nothing to compare with)
    'checking': ((116, 121, 134), 'dots'),   # Not checked yet
}


def line(d, points, width):
    d.line(points, fill=WHITE, width=width, joint='curve')
    r = width / 2
    for x, y in (points[0], points[-1]):
        d.ellipse((x - r, y - r, x + r, y + r), fill=WHITE)


def icon(color, mark):
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = S / 2
    d.ellipse((0, 0, S - 1, S - 1), fill=color + (255,))
    # The update arrow: most of a ring, open at the top right, an arrowhead at its end pointing on round.
    R, W = S * 0.33, S * 0.075
    start, end = -50, 250  # degrees, clockwise from three o'clock (PIL's way)
    d.arc((c - R, c - R, c + R, c + R), start=start, end=end, fill=WHITE, width=int(W))
    a = math.radians(start)
    tip = (c + R * math.cos(a), c + R * math.sin(a))
    # Arrowhead at the ring's start, pointing anticlockwise (towards the gap's other side).
    tangent = (math.sin(a), -math.cos(a))
    normal = (math.cos(a), math.sin(a))
    h, w = S * 0.11, S * 0.105
    head = [
        (tip[0] + tangent[0] * h, tip[1] + tangent[1] * h),
        (tip[0] + normal[0] * w - tangent[0] * h * 0.25, tip[1] + normal[1] * w - tangent[1] * h * 0.25),
        (tip[0] - normal[0] * w - tangent[0] * h * 0.25, tip[1] - normal[1] * w - tangent[1] * h * 0.25),
    ]
    d.polygon(head, fill=WHITE)
    m = S * 0.13  # the mark's half size
    mw = int(S * 0.07)
    if mark == 'check':
        line(d, [(c - m, c + m * 0.05), (c - m * 0.25, c + m * 0.75), (c + m, c - m * 0.6)], mw)
    elif mark == 'bang':
        line(d, [(c, c - m), (c, c + m * 0.3)], mw)
        r = mw * 0.62
        d.ellipse((c - r, c + m * 0.95 - r, c + r, c + m * 0.95 + r), fill=WHITE)
    elif mark == 'cross':
        k = m * 0.8
        line(d, [(c - k, c - k), (c + k, c + k)], mw)
        line(d, [(c - k, c + k), (c + k, c - k)], mw)
    elif mark == 'dot':
        r = m * 0.55
        d.ellipse((c - r, c - r, c + r, c + r), fill=WHITE)
    elif mark == 'dots':
        r = mw * 0.55
        for x in (-m * 0.85, 0, m * 0.85):
            d.ellipse((c + x - r, c - r, c + x + r, c + r), fill=WHITE)
    return img.resize((SIZE, SIZE), Image.LANCZOS)


for name, (color, mark) in ICONS.items():
    icon(color, mark).save(os.path.join(HERE, f'update-{name}.png'), optimize=True)
print('made', ', '.join(f'update-{n}.png' for n in ICONS))
