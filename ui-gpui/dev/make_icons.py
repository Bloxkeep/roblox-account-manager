"""Bloxkeep's own icons, in two layers (the owner, 2026-11-15: stock icons "trông slop"; like Medal's: a line drawing
with one detail in the accent colour). One language for all of them, from the logo (a rounded block with a face, a
second block behind it): 24 px grid, 1.8 px rounded strokes, corners rounded like the logo's block, and in each icon
ONE small part set apart as the accent: a filled rounded block (a door, a knob, a pupil), a four-point spark, or the
stroke that says what the thing does (an arrow, a clock's hands).

Writes, for each icon: assets/icons/duo/<name>.svg (the lines), <name>.a.svg (the accent part) and <name>.full.svg
(both, for where an icon is drawn in one colour); src/duo_icons.rs (their names, for kit::icon); and dev/icons.html,
a sheet of them all to look at in a browser.

Left as they are in assets/icons: the plain signs every interface shares (check, the chevrons, x, plus, more, grip,
sort, pause, play, stop, record, the window buttons) and others' marks (Discord's, Roblox's, Robux)."""
import os, sys

root = sys.argv[1]
out = os.path.join(root, 'assets', 'icons', 'duo')
os.makedirs(out, exist_ok=True)

HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">'

def spark(x, y, r):
    """The four-point spark, centred at x,y with arm r: a thing that is new, liked, or at night."""
    k = r * 0.32
    return f'<path d="M{x} {y-r}C{x} {y-k} {x+k} {y} {x+r} {y}C{x+k} {y} {x} {y+k} {x} {y+r}C{x} {y+k} {x-k} {y} {x-r} {y}C{x-k} {y} {x} {y-k} {x} {y-r}z" fill="#000" stroke="none"/>'

def block(x, y, w, h, rx):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="#000" stroke="none"/>'

def stroke(d, w=1.9):
    return f'<path d="{d}" fill="none" stroke="#000" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'

def fill(d):
    return f'<path d="{d}" fill="#000" stroke="none"/>'

def face(x, y):
    """The logo's face (two eyes and a smile), its left eye at x,y."""
    return f'<circle cx="{x}" cy="{y}" r="1" fill="#000" stroke="none"/><circle cx="{x+3.3}" cy="{y}" r="1" fill="#000" stroke="none"/>' + stroke(f'M{x-.2} {y+2.7}c1.1.9 2.6.9 3.7 0', 1.5)

SHIELD = '<path d="M12 3.2l7.500 2.800v5.500c0 4.600-3.100 8-7.500 9.300-4.400-1.300-7.500-4.700-7.500-9.300V6z"/>'
EYE = '<path d="M2.500 12S6 5.500 12 5.500 21.500 12 21.500 12 18 18.500 12 18.500 2.500 12 2.500 12z"/>'
TILE = '<rect x="3.500" y="3.500" width="17" height="17" rx="5"/>'
USER_LEFT = '<rect x="4" y="3.500" width="9" height="9" rx="2.600"/><path d="M2.500 20.500v-.500A3.500 3.500 0 0 1 6 16.500h5"/>'

ICONS = {
    # ── places
    'home': ('<path d="M4 11 12 4.200 20 11v7.800a1.700 1.700 0 0 1-1.700 1.700H5.700A1.700 1.700 0 0 1 4 18.800z"/>', block(9.7, 13, 4.6, 7.5, 1.3)),
    'users': ('<rect x="3" y="4" width="10" height="10" rx="2.800"/><path d="M2.500 20.500v-.600a3.400 3.400 0 0 1 3.400-3.400h4.200a3.400 3.400 0 0 1 3.400 3.400v.600"/><path d="M16.200 4.600h1.600A2.700 2.700 0 0 1 20.500 7.300v3.600a2.700 2.700 0 0 1-2.700 2.700h-1.600M17.800 16.500a3.400 3.400 0 0 1 3.400 3.400v.600"/>', face(6.4, 8.1)),
    'user': ('<rect x="7" y="3.500" width="10" height="10" rx="2.800"/><path d="M4.500 20.500V20A3.500 3.500 0 0 1 8 16.500h8a3.500 3.500 0 0 1 3.500 3.500v.500"/>', face(10.35, 7.6)),
    'userGo': (USER_LEFT, stroke('M14.500 17.500H21M18 14.500l3 3-3 3')),
    'userPlus': (USER_LEFT, stroke('M18 13.500v7M14.500 17h7')),
    'repeat': ('<path d="M8.500 4h8.300A3.200 3.200 0 0 1 20 7.200v8.300"/><rect x="4" y="7.500" width="12.500" height="12.500" rx="3.200"/>', fill('M8.800 11.200v5.100a.6.6 0 0 0 .9.500l4.200-2.500a.6.6 0 0 0 0-1.100l-4.200-2.500a.6.6 0 0 0-.9.500z')),
    'clock': (TILE, stroke('M12 7.600V12l3 1.900')),
    'history': ('<path d="M4.200 13.500A8 8 0 1 0 6.500 6.200"/><path d="M4 4.500v4.200h4.200"/>', stroke('M12 8.200v4.100l2.800 1.700')),
    'sliders': ('<path d="M5 3.500v5.700M5 15.800v4.700M12 3.500v2.200M12 12.300v8.200M19 3.500v8.300M19 18.400v2.100"/>', block(2.7, 10.2, 4.6, 4.6, 1.4) + block(9.7, 6.7, 4.6, 4.6, 1.4) + block(16.7, 12.8, 4.6, 4.6, 1.4)),
    'grid': ('<rect x="3.500" y="3.500" width="7" height="7" rx="2.200"/><rect x="13.500" y="3.500" width="7" height="7" rx="2.200"/><rect x="3.500" y="13.500" width="7" height="7" rx="2.200"/>', block(13.5, 13.5, 7, 7, 2.2)),
    'rows': ('<rect x="3.500" y="4" width="17" height="6.500" rx="2.200"/><rect x="3.500" y="13.500" width="17" height="6.500" rx="2.200"/>', block(6, 6, 2.5, 2.5, .9) + block(6, 15.5, 2.5, 2.5, .9)),
    'layers': ('<path d="M3.500 12.200l8.500 4.500 8.500-4.500"/><path d="M3.500 16.200l8.500 4.500 8.500-4.500"/><path d="M12 3.500 3.500 8l8.500 4.500L20.500 8z"/>', fill('M12 5.600 7.400 8l4.600 2.400L16.600 8z')),
    'gamepad': ('<rect x="2.500" y="6.500" width="19" height="11.500" rx="5.600"/><path d="M7.600 10.500v3.600M5.800 12.300h3.600"/>', block(13.9, 9.9, 2.3, 2.3, .9) + block(16.6, 12.4, 2.3, 2.3, .9)),
    'server': ('<rect x="3.500" y="4" width="17" height="7" rx="2.500"/><rect x="3.500" y="13" width="17" height="7" rx="2.500"/><path d="M12 7.500h5M12 16.500h5"/>', block(6.2, 6.4, 2.2, 2.2, .8) + block(6.2, 15.4, 2.2, 2.2, .8)),
    'screens': ('<rect x="2.500" y="5.500" width="12.500" height="9.500" rx="2.500"/><path d="M9 18.500h9.500A2.500 2.500 0 0 0 21 16V9.500"/>', block(5, 8, 4, 3.5, 1)),
    'monitor': ('<rect x="2.500" y="4" width="19" height="13" rx="3"/><path d="M8.500 20.500h7M12 17v3.500"/>', block(5.6, 7, 5, 4.4, 1.2)),
    # ── safety
    'lock': ('<rect x="4.500" y="10.500" width="15" height="10" rx="3"/><path d="M8 10.500V8a4 4 0 0 1 8 0v2.500"/>', block(10.8, 13.7, 2.4, 3.8, 1.2)),
    'key': ('<circle cx="8" cy="15.500" r="4.500"/><path d="M11.300 12.300 19.500 4M16 7.500l2.500 2.500"/>', block(6.6, 14.1, 2.8, 2.8, 1)),
    'shield': (SHIELD, block(9.8, 9.3, 4.4, 4.4, 1.3)),
    'shieldCheck': (SHIELD, stroke('M8.800 12.200l2.300 2.300 4.200-4.400')),
    'alert': ('<path d="M10.500 4.700 3.300 17.500a1.700 1.700 0 0 0 1.500 2.500h14.400a1.700 1.700 0 0 0 1.500-2.500L13.500 4.700a1.700 1.700 0 0 0-3 0z"/>', block(11, 9, 2, 5, 1) + block(11, 15.4, 2, 2, 1)),
    'eye': (EYE, block(9.3, 9.3, 5.4, 5.4, 1.8)),
    'eyeOff': (EYE, stroke('M4.500 4.500l15 15')),
    'cookie': ('<path d="M20.500 12.500A8.500 8.500 0 1 1 11.500 3.500a4 4 0 0 0 4.500 4.500 4 4 0 0 0 4.500 4.500z"/>', block(7.6, 9.4, 2.5, 2.5, .9) + block(11.4, 14.2, 2.5, 2.5, .9) + block(7.2, 14.6, 1.9, 1.9, .7)),
    # ── things said, marked, liked
    'bell': ('<path d="M6 16.500V11a6 6 0 0 1 12 0v5.500l1.500 2h-15z"/>', block(10, 19.7, 4, 2.3, 1.1)),
    'message': ('<path d="M4 6.500A2.500 2.500 0 0 1 6.500 4h11A2.500 2.500 0 0 1 20 6.500v8a2.500 2.500 0 0 1-2.500 2.500H10l-4.500 3.500V17A2.500 2.500 0 0 1 4 14.500z"/>', block(7.6, 9.4, 2.2, 2.2, .8) + block(10.9, 9.4, 2.2, 2.2, .8) + block(14.2, 9.4, 2.2, 2.2, .8)),
    'news': ('<path d="M4 5.500h12.500V18a2 2 0 0 0 2 2H6.500A2.500 2.500 0 0 1 4 17.500z"/><path d="M16.500 9H20v9a2 2 0 0 1-2 2"/><path d="M7.500 13.500h5.500M7.500 16.500h5.500"/>', block(7.2, 8.2, 6, 2.6, .9)),
    'star': ('<path d="M11 5.200l2.300 4.700 5.200.700-3.800 3.700.900 5.200L11 17l-4.600 2.500.900-5.200-3.800-3.700 5.200-.700z"/>', spark(19.6, 5, 2.9)),
    'like': ('<path d="M7.500 10.800V20H5a1.500 1.500 0 0 1-1.500-1.500v-6.200A1.500 1.500 0 0 1 5 10.800z"/><path d="M7.500 10.800l3.200-5.900a1.900 1.900 0 0 1 3.500 1.300l-.800 3.800h4.100a2.200 2.200 0 0 1 2.100 2.800l-1.400 5.500a2.200 2.200 0 0 1-2.100 1.700H7.500"/>', spark(18.9, 3.9, 2.7)),
    'bookmark': ('<path d="M6.700 3.500h10.600A1.700 1.700 0 0 1 19 5.200v15.300l-7-4.400-7 4.400V5.200a1.700 1.700 0 0 1 1.700-1.700z"/>', block(9.5, 7, 5, 5, 1.5)),
    'tag': ('<path d="M3.500 5.500a2 2 0 0 1 2-2h5.700a2 2 0 0 1 1.400.600l7.300 7.300a2 2 0 0 1 0 2.800l-5.700 5.700a2 2 0 0 1-2.800 0l-7.300-7.300a2 2 0 0 1-.600-1.400z"/>', block(6.6, 6.6, 3, 3, 1.1)),
    'info': (TILE + '<path d="M12 11.300v5.200"/>', block(10.8, 6.8, 2.4, 2.4, .9)),
    'moon': ('<path d="M19.500 14.600A8 8 0 1 1 9.400 4.500a6.300 6.300 0 0 0 10.100 10.100z"/>', spark(17.4, 6, 3)),
    'activity': ('<path d="M3 12h3.800l2.600-6.500 5.200 13 2.600-6.500H18"/>', block(18.4, 10.5, 3, 3, 1.1)),
    'gauge': ('<path d="M4.700 17.500a8.500 8.500 0 1 1 14.600 0"/><path d="M12 6.500v1.200M6.300 9l.900.800M17.700 9l-.900.800"/>', stroke('M12 14.200l3.300-4.200') + block(10.5, 12.8, 3, 3, 1.5)),
    # ── files, pictures, words
    'download': ('<path d="M4 15.500v2.300a2.700 2.700 0 0 0 2.700 2.700h10.600a2.700 2.700 0 0 0 2.700-2.700v-2.300"/>', stroke('M12 3.500v10.800M7.700 10.200 12 14.500l4.300-4.300')),
    'folder': ('<path d="M3.500 7A2.500 2.500 0 0 1 6 4.500h3.200l2.300 2.500H18A2.500 2.500 0 0 1 20.500 9.500V17a2.500 2.500 0 0 1-2.500 2.500H6A2.500 2.500 0 0 1 3.500 17z"/>', block(13.6, 12, 4, 4, 1.2)),
    'book': ('<path d="M5 19.200V5.700A2.700 2.700 0 0 1 7.700 3H19v13.500H7.700a2.500 2.500 0 0 0 0 5H19"/>', fill('M10.200 3v6.600l2.100-1.600 2.100 1.600V3z')),
    'image': ('<rect x="3.500" y="4.500" width="17" height="15" rx="3.500"/><path d="M4 16.500l4.500-4.500 3.500 3.500 2.500-2.500 5.500 5.500"/>', block(14, 7.4, 3.2, 3.2, 1.2)),
    'palette': ('<path d="M12 3.500a8.500 8.500 0 1 0 0 17c1.400 0 2-.900 2-1.900 0-1.300-1-1.700-1-2.900 0-1 .800-1.700 1.900-1.700h1.800a3.800 3.800 0 0 0 3.800-3.800C20.500 6.300 16.700 3.500 12 3.500z"/>', block(6.4, 10.1, 2.5, 2.5, .9) + block(9.2, 6.3, 2.5, 2.5, .9) + block(13.4, 6.300, 2.500, 2.500, .9)),
    'code': ('<path d="M8.500 7.500 4 12l4.500 4.500M15.500 7.500 20 12l-4.500 4.500"/>', stroke('M13.300 5.500l-2.600 13')),
    'languages': ('<path d="M3.500 6h9M8 3.500V6M5 6c.500 3.500 3 6.500 6.500 8M11.500 6c-.800 4-3.500 7.500-7.500 9"/>', stroke('M12.500 20.500l4-9 4 9M14 17.500h5')),
    'qr': ('<rect x="3.500" y="3.500" width="6.500" height="6.500" rx="1.800"/><rect x="14" y="3.500" width="6.500" height="6.500" rx="1.800"/><rect x="3.500" y="14" width="6.500" height="6.500" rx="1.800"/>', block(14, 14, 2.8, 2.8, .9) + block(17.7, 17.7, 2.8, 2.8, .9) + block(17.7, 14, 2.8, 2.8, .9)),
    'calendar': ('<rect x="3.500" y="5" width="17" height="15.500" rx="3.500"/><path d="M8 3v4M16 3v4M3.500 10.500h17"/>', block(13.2, 13.2, 4, 4, 1.2)),
    'link': ('<path d="M10.500 13.500a4 4 0 0 0 5.700 0l2.800-2.800a4 4 0 0 0-5.700-5.700l-1 1"/><path d="M13.500 10.500a4 4 0 0 0-5.700 0L5 13.300A4 4 0 0 0 10.700 19l1-1"/>', spark(18.6, 18.4, 2.4)),
    # ── doing
    'edit': ('<path d="M4 20l1-4.200L16.300 4.500a2 2 0 0 1 2.800 0l.400.400a2 2 0 0 1 0 2.800L8.200 19z"/>', fill('M4 20l1-4.200 3.200 3.200z')),
    'trash': ('<path d="M4 6.500h16M9 6.500V5a1.500 1.500 0 0 1 1.500-1.500h3A1.500 1.500 0 0 1 15 5v1.500M6 6.500l.800 12a2 2 0 0 0 2 2h6.400a2 2 0 0 0 2-2l.800-12"/>', stroke('M10 10.500v5.500M14 10.500v5.500')),
    'copy': ('<path d="M8 4h8.800A3.200 3.200 0 0 1 20 7.200V16"/><rect x="4" y="8" width="12" height="12" rx="3"/>', block(8, 12, 4, 4, 1.2)),
    'search': ('<circle cx="11" cy="11" r="6.500"/>', stroke('M16 16l4.500 4.500')),
    'refresh': ('<path d="M19.500 12a7.500 7.500 0 1 1-2.200-5.300"/>', stroke('M19.800 3.800v4.400h-4.400')),
    'external': ('<path d="M11 5.500H7A2.500 2.500 0 0 0 4.500 8v9A2.500 2.500 0 0 0 7 19.500h9a2.500 2.500 0 0 0 2.500-2.500v-4"/>', stroke('M14 4.500h5.500V10M19.500 4.500l-8 8')),
    'logIn': ('<path d="M14 4h3.500A2.500 2.500 0 0 1 20 6.500v11a2.500 2.500 0 0 1-2.500 2.500H14"/>', stroke('M4 12h10M10.500 8.200l3.800 3.800-3.800 3.800')),
    'power': ('<path d="M7 6.300a8 8 0 1 0 10 0"/>', stroke('M12 3v8.500')),
    'shuffle': ('<path d="M3.500 7h3a4 4 0 0 1 3.300 1.700l4.400 6.600a4 4 0 0 0 3.300 1.700h3M3.500 17h3a4 4 0 0 0 3.300-1.700l.700-1M13.500 9.700l.700-1A4 4 0 0 1 17.500 7h3"/>', stroke('M18.500 4.800 20.700 7l-2.200 2.200M18.500 14.800l2.200 2.200-2.200 2.200')),
    'branch': ('<path d="M7 6.500v11M17 6.500V8a5 5 0 0 1-5 5H7"/>', block(5, 2.500, 4, 4, 1.3) + block(15, 2.500, 4, 4, 1.3) + block(5, 17.500, 4, 4, 1.3)),
    'focus': ('<path d="M4 9V6.500A2.500 2.500 0 0 1 6.500 4H9M15 4h2.500A2.500 2.500 0 0 1 20 6.500V9M20 15v2.500a2.500 2.500 0 0 1-2.500 2.500H15M9 20H6.500A2.500 2.500 0 0 1 4 17.500V15"/>', block(9.5, 9.5, 5, 5, 1.5)),
    'wrench': ('<path d="M20 7.200a4.800 4.800 0 0 1-6.300 4.600l-6.800 6.800a1.900 1.900 0 0 1-2.700-2.700l6.800-6.800a4.800 4.800 0 0 1 4.600-6.300L13 5.500l.600 2.900 2.900.600z"/>', block(4.7, 16.8, 2.300, 2.300, 1.1)),
    # ── the PC
    'chip': ('<rect x="6" y="6" width="12" height="12" rx="2.800"/><path d="M9.500 3v3M14.500 3v3M9.500 18v3M14.500 18v3M3 9.500h3M3 14.500h3M18 9.500h3M18 14.500h3"/>', block(9.6, 9.6, 4.8, 4.8, 1.3)),
    'keyboard': ('<rect x="2.500" y="6" width="19" height="12" rx="3"/><path d="M8 14.500h8"/>', block(5.6, 9, 2, 2, .7) + block(9.2, 9, 2, 2, .7) + block(12.8, 9, 2, 2, .7) + block(16.4, 9, 2, 2, .7)),
    'mouse': ('<rect x="6.500" y="3" width="11" height="18" rx="5.500"/>', block(11, 6.5, 2, 4, 1)),
    'cloud': ('<path d="M7.500 19a4.500 4.500 0 0 1-.600-8.960A5.500 5.500 0 0 1 17.600 9.500 4.750 4.750 0 0 1 17 19z"/>', stroke('M12 16.500v-4.800M9.800 13.700 12 11.500l2.200 2.200')),
    'globe': ('<circle cx="12" cy="12" r="8.500"/><path d="M3.500 12h17M12 3.500c2.500 2.300 3.800 5.200 3.800 8.500s-1.300 6.200-3.800 8.500c-2.500-2.300-3.800-5.200-3.800-8.500s1.300-6.200 3.800-8.500z"/>', spark(18.9, 5.1, 2.5)),
}

for name, (line, accent) in ICONS.items():
    w = lambda file, body: open(os.path.join(out, file), 'w', encoding='utf-8', newline='\n').write(body)
    w(name + '.svg', HEAD + line + '</svg>')
    w(name + '.a.svg', '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">' + accent + '</svg>')
    w(name + '.full.svg', HEAD + line + accent + '</svg>')

names = sorted(ICONS)
open(os.path.join(root, 'src', 'duo_icons.rs'), 'w', encoding='utf-8', newline='\n').write(
    "//! The names of Bloxkeep's own two-layer icons (assets/icons/duo). Written by dev/make_icons.py: not edited by hand.\n"
    f"pub const DUO: [&str; {len(names)}] = [\n" + ''.join(f'    "{n}",\n' for n in names) + "];\n")

cells = ''.join(
    f'<figure><span class="i"><span class="l" style="-webkit-mask-image:url(../assets/icons/duo/{n}.svg);mask-image:url(../assets/icons/duo/{n}.svg)"></span>'
    f'<span class="a" style="-webkit-mask-image:url(../assets/icons/duo/{n}.a.svg);mask-image:url(../assets/icons/duo/{n}.a.svg)"></span></span><figcaption>{n}</figcaption></figure>'
    for n in ICONS)
open(os.path.join(root, 'dev', 'icons.html'), 'w', encoding='utf-8', newline='\n').write(
    '<!doctype html><meta charset="utf-8"><title>Bloxkeep icons</title><style>'
    'body{margin:0;padding:24px;background:#1a1b20;color:#9a9890;font:12px/1.3 system-ui}'
    'main{display:grid;grid-template-columns:repeat(10,1fr);gap:18px 8px}figure{margin:0;display:grid;justify-items:center;gap:6px}'
    '.i{position:relative;width:48px;height:48px}.i span{position:absolute;inset:0;-webkit-mask-size:100% 100%;mask-size:100% 100%}'
    '.l{background:#f7f6f2}.a{background:#f6c63a}</style><main>' + cells + '</main>')
print(len(ICONS), 'icons')
