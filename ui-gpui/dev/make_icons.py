"""Bloxkeep's own icons, in two layers (the owner, 2026-11-15: stock icons "trông slop"; like Medal's: a line drawing
with one detail in the accent colour). One language for all of them, from the logo (a rounded block with a face, a
second block behind it): 24 px grid, 1.8 px rounded strokes, corners rounded like the logo's block, and in each icon
ONE small part set apart as the accent: a filled rounded block (a door, a knob, a pupil), or a four-point spark.
Writes assets/icons/duo/<name>.svg (the line) and <name>.a.svg (the accent)."""
import os, sys

out = os.path.join(sys.argv[1], 'assets', 'icons', 'duo')
os.makedirs(out, exist_ok=True)

LINE = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">%s</svg>'
# The four-point spark, centred at x,y with arm r: the set's second motif (a thing that is new, liked, or at night).
def spark(x, y, r):
    k = r * 0.32
    return f'<path d="M{x} {y-r}C{x} {y-k} {x+k} {y} {x+r} {y}C{x+k} {y} {x} {y+k} {x} {y+r}C{x} {y+k} {x-k} {y} {x-r} {y}C{x-k} {y} {x} {y-k} {x} {y-r}z" fill="#000" stroke="none"/>'
def block(x, y, w, h, rx):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="#000" stroke="none"/>'

ICONS = {
    # A house whose roof is the logo's tilted block; its door the accent block.
    'home': ('<path d="M4 11 12 4.2 20 11v7.8a1.7 1.7 0 0 1-1.7 1.7H5.7A1.7 1.7 0 0 1 4 18.8z"/>', block(9.7, 13, 4.6, 7.5, 1.3)),
    # Two accounts as two of the logo's blocks: the front one's face is the accent.
    'users': ('<rect x="3" y="4" width="10" height="10" rx="2.8"/><path d="M2.5 20.5v-.6a3.4 3.4 0 0 1 3.4-3.4h4.2a3.4 3.4 0 0 1 3.4 3.4v.6"/><path d="M16.2 4.6h1.6A2.7 2.7 0 0 1 20.5 7.300v3.6a2.700 2.700 0 0 1-2.700 2.700h-1.600M17.800 16.500a3.400 3.400 0 0 1 3.400 3.400v.6"/>',
              '<circle cx="6.4" cy="8.1" r="1" fill="#000" stroke="none"/><circle cx="9.7" cy="8.1" r="1" fill="#000" stroke="none"/><path d="M6.200 10.800c1.100.9 2.600.9 3.700 0" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round"/>'),
    # Presets: one block on another (the logo's pair), the front one set to play.
    'repeat': ('<path d="M8.5 4h8.3A3.200 3.200 0 0 1 20 7.200v8.300"/><rect x="4" y="7.500" width="12.500" height="12.500" rx="3.200"/>', '<path d="M8.800 11.200v5.100a.600.600 0 0 0 .9.500l4.200-2.500a.600.600 0 0 0 0-1.100l-4.200-2.500a.600.600 0 0 0-.900.500z" fill="#000" stroke="none"/>'),
    # Statistics: a clock in a block; its hands the accent.
    'clock': ('<rect x="3.500" y="3.500" width="17" height="17" rx="5"/>', '<path d="M12 7.600V12l3 1.900" fill="none" stroke="#000" stroke-width="1.900" stroke-linecap="round" stroke-linejoin="round"/>'),
    # Settings: three tracks, their knobs the accent blocks.
    'sliders': ('<path d="M5 3.500v5.700M5 15.800v4.700M12 3.500v2.200M12 12.300v8.200M19 3.500v8.300M19 18.400v2.100"/>', block(2.7, 10.2, 4.6, 4.6, 1.4) + block(9.7, 6.7, 4.6, 4.6, 1.4) + block(16.7, 12.8, 4.6, 4.6, 1.4)),
    'lock': ('<rect x="4.500" y="10.500" width="15" height="10" rx="3"/><path d="M8 10.500V8a4 4 0 0 1 8 0v2.500"/>', block(10.8, 13.7, 2.4, 3.8, 1.2)),
    # Seen: an eye whose pupil is a block.
    'eye': ('<path d="M2.500 12S6 5.500 12 5.500 21.500 12 21.500 12 18 18.500 12 18.500 2.500 12 2.500 12z"/>', block(9.3, 9.3, 5.4, 5.4, 1.8)),
    # Favourites: a star with a spark off its shoulder.
    'star': ('<path d="M11 5.200l2.300 4.700 5.200.700-3.800 3.700.900 5.200L11 17l-4.600 2.500.900-5.200-3.800-3.700 5.200-.700z"/>', spark(19.6, 5, 2.9)),
    # Liked: a thumb up, a spark over it.
    'like': ('<path d="M7.500 10.800v9.200H5A1.500 1.500 0 0 1 3.500 18.500v-6.200A1.500 1.500 0 0 1 5 10.800z"/><path d="M7.500 10.800l3.200-5.900a1.900 1.900 0 0 1 3.500 1.300l-.800 3.800h4.100a2.200 2.200 0 0 1 2.100 2.800l-1.400 5.500a2.200 2.200 0 0 1-2.100 1.700H7.500"/>', spark(18.9, 3.9, 2.7)),
    'info': ('<rect x="3.500" y="3.500" width="17" height="17" rx="5"/><path d="M12 11.300v5.200"/>', block(10.8, 6.800, 2.4, 2.4, 0.9)),
    # Quiet hours: a moon and its spark.
    'moon': ('<path d="M19.500 14.600A8 8 0 1 1 9.400 4.500a6.300 6.300 0 0 0 10.100 10.100z"/>', spark(17.4, 6, 3)),
    # Files: a tray; the arrow into it the accent.
    'download': ('<path d="M4 15.500v2.300A2.700 2.700 0 0 0 6.700 20.500h10.600A2.700 2.700 0 0 0 20 17.800v-2.300"/>', '<path d="M12 3.500v10.800M7.700 10.200 12 14.500l4.300-4.300" fill="none" stroke="#000" stroke-width="1.900" stroke-linecap="round" stroke-linejoin="round"/>'),
    # The wiki, About: a book with its ribbon.
    'book': ('<path d="M5 19.200V5.700A2.700 2.700 0 0 1 7.700 3H19v13.500H7.700a2.500 2.500 0 0 0 0 5H19"/>', '<path d="M10.200 3v6.600l2.100-1.600 2.100 1.600V3z" fill="#000" stroke="none"/>'),
    # Inventory: a kept thing, the block inside it.
    'bookmark': ('<path d="M6.700 3.500h10.600A1.700 1.700 0 0 1 19 5.200v15.300l-7-4.400-7 4.400V5.200a1.700 1.700 0 0 1 1.700-1.700z"/>', block(9.5, 7, 5, 5, 1.5)),
}
for name, (line, accent) in ICONS.items():
    open(os.path.join(out, name + '.svg'), 'w', encoding='utf-8', newline='\n').write(LINE % line)
    open(os.path.join(out, name + '.a.svg'), 'w', encoding='utf-8', newline='\n').write('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">%s</svg>' % accent)
print(len(ICONS), 'icons:', ' '.join(ICONS))
