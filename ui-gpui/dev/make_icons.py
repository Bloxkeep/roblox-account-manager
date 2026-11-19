"""Bloxkeep's compact two-layer icon family. Rounded block silhouettes echo the logo;
the accent identifies a functional part, never an unrelated badge. Drawings must
remain legible at 16, 20 and 24 px, in one colour as well as two.

Generates the three SVG variants, the Rust name table and the preview sheet.
Shared interface signs and other companies' marks stay in assets/icons.
"""
import os, sys

root = sys.argv[1]
out = os.path.join(root, 'assets', 'icons', 'duo')
os.makedirs(out, exist_ok=True)

HEAD = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">'

def block(x, y, w, h, rx):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="#000" stroke="none"/>'

def stroke(d, w=1.9):
    return f'<path d="{d}" fill="none" stroke="#000" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'

def fill(d):
    return f'<path d="{d}" fill="#000" stroke="none"/>'

SHIELD = '<path d="M12 3.2l7.500 2.800v5.500c0 4.600-3.100 8-7.500 9.300-4.400-1.300-7.500-4.700-7.500-9.300V6z"/>'
EYE = '<path d="M2.500 12S6 5.500 12 5.500 21.500 12 21.500 12 18 18.500 12 18.500 2.500 12 2.500 12z"/>'
TILE = '<rect x="3.500" y="3.500" width="17" height="17" rx="5"/>'
USER_LEFT = '<rect x="4" y="3.500" width="9" height="9" rx="2.600"/><path d="M2.500 20.500v-.500A3.500 3.500 0 0 1 6 16.500h5"/>'

ICONS = {
    # ── places
    'home': ('<path d="M4 11 12 4.200 20 11v7.800a1.700 1.700 0 0 1-1.700 1.700H5.700A1.700 1.700 0 0 1 4 18.800z"/>', block(9.7, 13, 4.6, 7.5, 1.3)),
    'users': ('<rect x="3.5" y="4" width="8" height="8" rx="2.5"/><path d="M2.5 20v-1a4 4 0 0 1 4-4h2a4 4 0 0 1 4 4v1"/>', stroke('M15.5 4.5h1a3 3 0 0 1 3 3v1a3 3 0 0 1-3 3h-1M16 15h1a4 4 0 0 1 4 4v1')),
    'user': ('<rect x="8" y="3.5" width="8" height="8" rx="2.5"/>', stroke('M4.5 20v-1a4 4 0 0 1 4-4h7a4 4 0 0 1 4 4v1')),
    'userGo': (USER_LEFT, stroke('M14.500 17.500H21M18 14.500l3 3-3 3')),
    'userPlus': (USER_LEFT, stroke('M18 13.500v7M14.500 17h7')),
    'repeat': ('<path d="M4 9V8a3 3 0 0 1 3-3h13M20 15v1a3 3 0 0 1-3 3H4"/>', stroke('M17 2l3 3-3 3M7 16l-3 3 3 3')),
    'clock': ('<circle cx="12" cy="12" r="8.5"/>', stroke('M12 7v5l3.5 2')),
    'history': ('<path d="M4.200 13.500A8 8 0 1 0 6.500 6.200"/><path d="M4 4.500v4.200h4.200"/>', stroke('M12 8.200v4.100l2.800 1.700')),
    'sliders': ('<path d="M5 3.500v5.700M5 15.800v4.700M12 3.500v2.200M12 12.300v8.200M19 3.500v8.300M19 18.400v2.100"/>', block(2.7, 10.2, 4.6, 4.6, 1.4) + block(9.7, 6.7, 4.6, 4.6, 1.4) + block(16.7, 12.8, 4.6, 4.6, 1.4)),
    'grid': ('<rect x="3.500" y="3.500" width="7" height="7" rx="2.200"/><rect x="13.500" y="3.500" width="7" height="7" rx="2.200"/><rect x="3.500" y="13.500" width="7" height="7" rx="2.200"/>', block(13.5, 13.5, 7, 7, 2.2)),
    'rows': ('<path d="M9 6h11M9 12h11M9 18h11"/>', block(3, 4.5, 3, 3, 1) + block(3, 10.5, 3, 3, 1) + block(3, 16.5, 3, 3, 1)),
    'layers': ('<path d="M3.500 12.200l8.500 4.500 8.500-4.500"/><path d="M3.500 16.200l8.500 4.500 8.500-4.500"/><path d="M12 3.500 3.500 8l8.500 4.500L20.500 8z"/>', fill('M12 5.600 7.400 8l4.600 2.400L16.600 8z')),
    'gamepad': ('<rect x="2.500" y="6.500" width="19" height="11.500" rx="5.600"/><path d="M7.600 10.500v3.600M5.800 12.300h3.600"/>', block(13.9, 9.9, 2.3, 2.3, .9) + block(16.6, 12.4, 2.3, 2.3, .9)),
    'server': ('<rect x="3.500" y="4" width="17" height="7" rx="2.500"/><rect x="3.500" y="13" width="17" height="7" rx="2.500"/><path d="M12 7.500h5M12 16.500h5"/>', block(6.2, 6.4, 2.2, 2.2, .8) + block(6.2, 15.4, 2.2, 2.2, .8)),
    'screens': ('<rect x="2.5" y="9" width="14" height="10" rx="2"/><path d="M6 21.5h7M9.5 19v2.5"/>', stroke('M7.5 6V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2')),
    'monitor': ('<rect x="3" y="4" width="18" height="12" rx="2.5"/>', stroke('M12 16v4M8 20h8')),
    # ── safety
    'lock': ('<rect x="4.500" y="10.500" width="15" height="10" rx="3"/><path d="M8 10.500V8a4 4 0 0 1 8 0v2.500"/>', block(10.8, 13.7, 2.4, 3.8, 1.2)),
    'key': ('<circle cx="8" cy="15.500" r="4.500"/><path d="M11.300 12.300 19.500 4M16 7.500l2.500 2.500"/>', block(6.6, 14.1, 2.8, 2.8, 1)),
    'shield': (SHIELD, stroke('M12 7.5v8')),
    'shieldCheck': (SHIELD, stroke('M8.800 12.200l2.300 2.300 4.200-4.400')),
    'alert': ('<path d="M10.500 4.700 3.300 17.500a1.700 1.700 0 0 0 1.500 2.500h14.400a1.700 1.700 0 0 0 1.500-2.500L13.500 4.700a1.700 1.700 0 0 0-3 0z"/>', block(11, 9, 2, 5, 1) + block(11, 15.4, 2, 2, 1)),
    'eye': (EYE, '<circle cx="12" cy="12" r="3" fill="#000" stroke="none"/>'),
    'eyeOff': (EYE, stroke('M4.500 4.500l15 15')),
    'cookie': ('<path d="M20.500 12.500A8.500 8.500 0 1 1 11.500 3.500a4 4 0 0 0 4.500 4.500 4 4 0 0 0 4.500 4.500z"/>', block(7.6, 9.4, 2.5, 2.5, .9) + block(11.4, 14.2, 2.5, 2.5, .9) + block(7.2, 14.6, 1.9, 1.9, .7)),
    # ── things said, marked, liked
    'bell': ('<path d="M6 16.500V11a6 6 0 0 1 12 0v5.500l1.500 2h-15z"/>', block(10, 19.7, 4, 2.3, 1.1)),
    'message': ('<path d="M4 6.500A2.500 2.500 0 0 1 6.500 4h11A2.500 2.500 0 0 1 20 6.500v8a2.500 2.500 0 0 1-2.500 2.500H10l-4.500 3.500V17A2.500 2.500 0 0 1 4 14.500z"/>', block(7.6, 9.4, 2.2, 2.2, .8) + block(10.9, 9.4, 2.2, 2.2, .8) + block(14.2, 9.4, 2.2, 2.2, .8)),
    'news': ('<path d="M4 5.500h12.500V18a2 2 0 0 0 2 2H6.500A2.500 2.500 0 0 1 4 17.500z"/><path d="M16.500 9H20v9a2 2 0 0 1-2 2"/><path d="M7.500 13.500h5.500M7.500 16.500h5.500"/>', block(7.2, 8.2, 6, 2.6, .9)),
    'star': ('<path d="m12 3 2.8 5.7 6.2.9-4.5 4.4 1.1 6.2-5.6-3-5.6 3 1.1-6.2L3 9.6l6.2-.9z"/>', fill('m12 7.5 1.6 3.3 3.6.5-2.6 2.5.6 3.6-3.2-1.7-3.2 1.7.6-3.6-2.6-2.5 3.6-.5z')),
    'like': ('<path d="M8 10.5 11.5 4a2 2 0 0 1 3.5 1.5l-.8 4H18a2.5 2.5 0 0 1 2.4 3.2l-1.5 5.5a2.5 2.5 0 0 1-2.4 1.8H8z"/>', block(3, 10.5, 3, 9.5, 1)),
    'bookmark': ('<path d="M6 8v12l6-3.5 6 3.5V8"/>', stroke('M6 8V5a1.5 1.5 0 0 1 1.5-1.5h9A1.5 1.5 0 0 1 18 5v3')),
    'tag': ('<path d="M3.500 5.500a2 2 0 0 1 2-2h5.700a2 2 0 0 1 1.400.600l7.300 7.300a2 2 0 0 1 0 2.800l-5.700 5.700a2 2 0 0 1-2.800 0l-7.300-7.300a2 2 0 0 1-.600-1.400z"/>', block(6.6, 6.6, 3, 3, 1.1)),
    'gift': ('<rect x="4.500" y="11" width="15" height="9.500" rx="2"/><rect x="3" y="7.500" width="18" height="3.500" rx="1.300"/><path d="M12 7.500v13"/>', stroke('M12 7.500C10.800 4.300 7.300 3.600 7.100 5.600 6.900 7.300 10 7.500 12 7.500zM12 7.500c1.200-3.200 4.700-3.900 4.900-1.900.2 1.700-2.900 1.900-4.900 1.900z')),
    'info': (TILE + '<path d="M12 11.300v5.200"/>', block(10.8, 6.8, 2.4, 2.4, .9)),
    'moon': ('<path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>', ''),
    'activity': ('<path d="M3 12h4M17 12h4"/>', stroke('M7 12l3-7 4 14 3-7')),
    'gauge': ('<path d="M4.700 17.500a8.500 8.500 0 1 1 14.600 0"/><path d="M12 6.500v1.200M6.300 9l.900.800M17.700 9l-.900.800"/>', stroke('M12 14.200l3.300-4.200') + block(10.5, 12.8, 3, 3, 1.5)),
    # ── files, pictures, words
    'download': ('<path d="M4 15.500v2.300a2.700 2.700 0 0 0 2.700 2.700h10.600a2.700 2.700 0 0 0 2.700-2.700v-2.300"/>', stroke('M12 3.500v10.800M7.700 10.200 12 14.500l4.300-4.300')),
    'folder': ('<path d="M3.5 9V7A2.5 2.5 0 0 1 6 4.5h3l2.5 2.5H18A2.5 2.5 0 0 1 20.5 9.5V17a2.5 2.5 0 0 1-2.5 2.5H6A2.5 2.5 0 0 1 3.5 17V9z"/>', stroke('M4 10h16')),
    'book': ('<path d="M5 19.200V5.700A2.700 2.700 0 0 1 7.700 3H19v13.500H7.700a2.500 2.500 0 0 0 0 5H19"/>', fill('M10.200 3v6.600l2.100-1.600 2.100 1.600V3z')),
    'image': ('<rect x="3.500" y="4.500" width="17" height="15" rx="3.500"/><path d="M4 16.500l4.500-4.500 3.500 3.500 2.500-2.500 5.500 5.500"/>', block(14, 7.4, 3.2, 3.2, 1.2)),
    'palette': ('<path d="M12 3.500a8.500 8.500 0 1 0 0 17c1.400 0 2-.900 2-1.900 0-1.300-1-1.700-1-2.900 0-1 .800-1.700 1.900-1.700h1.800a3.800 3.800 0 0 0 3.800-3.800C20.500 6.300 16.700 3.500 12 3.500z"/>', block(6.4, 10.1, 2.5, 2.5, .9) + block(9.2, 6.3, 2.5, 2.5, .9) + block(13.4, 6.300, 2.500, 2.500, .9)),
    'code': ('<path d="M8.500 7.500 4 12l4.500 4.500M15.500 7.500 20 12l-4.500 4.500"/>', stroke('M13.300 5.500l-2.600 13')),
    'languages': ('<path d="M3.500 6h9M8 3.500V6M5 6c.500 3.500 3 6.500 6.500 8M11.500 6c-.800 4-3.500 7.500-7.500 9"/>', stroke('M12.500 20.500l4-9 4 9M14 17.500h5')),
    'qr': ('<rect x="3.500" y="3.500" width="6.500" height="6.500" rx="1.800"/><rect x="14" y="3.500" width="6.500" height="6.500" rx="1.800"/><rect x="3.500" y="14" width="6.500" height="6.500" rx="1.800"/>', block(14, 14, 2.8, 2.8, .9) + block(17.7, 17.7, 2.8, 2.8, .9) + block(17.7, 14, 2.8, 2.8, .9)),
    'calendar': ('<rect x="3.500" y="5" width="17" height="15.500" rx="3.500"/><path d="M8 3v4M16 3v4M3.500 10.500h17"/>', block(13.2, 13.2, 4, 4, 1.2)),
    'link': ('<path d="m10 7 2-2a5 5 0 0 1 7 7l-2 2M7 10l-2 2a5 5 0 0 0 7 7l2-2"/>', stroke('M8.5 15.5l7-7')),
    # ── doing
    'edit': ('<path d="M4 20l1-4.200L16.300 4.500a2 2 0 0 1 2.800 0l.400.400a2 2 0 0 1 0 2.800L8.200 19z"/>', fill('M4 20l1-4.200 3.200 3.200z')),
    'trash': ('<path d="M4 6.500h16M9 6.500V5a1.500 1.500 0 0 1 1.500-1.500h3A1.500 1.500 0 0 1 15 5v1.500M6 6.500l.800 12a2 2 0 0 0 2 2h6.400a2 2 0 0 0 2-2l.800-12"/>', stroke('M10 10.500v5.500M14 10.500v5.500')),
    'copy': ('<rect x="4" y="8" width="12" height="12" rx="3"/>', stroke('M8 4h9a3 3 0 0 1 3 3v9')),
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
    'globe': ('<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17"/>', stroke('M12 3.5c2.5 2.3 3.8 5.2 3.8 8.5s-1.3 6.2-3.8 8.5c-2.5-2.3-3.8-5.2-3.8-8.5s1.3-6.2 3.8-8.5z')),
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
