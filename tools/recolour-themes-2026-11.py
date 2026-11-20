"""The colour rule's pass over the built-in themes (Josh, 2026-11-20: no teal, no purple, no pink in any theme).

Edits themes/*.json in place (they are the source now; make-homage-themes.py is historical). Run once; running it
again changes nothing. rust/core/src/hues.rs tests every built-in theme against the rule.
"""
import json
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'themes'))


def edit(name, colors=None, description=None, fn=None):
    path = os.path.join(ROOT, name + '.json')
    with open(path, encoding='utf-8') as f:
        t = json.load(f)
    for k, v in (colors or {}).items():
        t['colors'][k] = v
    if description:
        t['description'] = description
    if fn:
        fn(t['effects'])
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(t, f, ensure_ascii=False, indent=2)
        f.write('\n')


def stops(colour, layer):
    """A radial light's fill, recoloured, keeping each stop's alpha."""
    layer['fill'] = [['#' + s[1:3] + colour, at] for s, at in layer['fill']]


# Retrowave: a midnight drive. A molten sun sinks into an electric-blue grid under a deep navy sky.
def retrowave(e):
    e['synthwave'] = {'sunTop': '#FFE066', 'sunBottom': '#FF5A1F', 'grid': '#2F7BFF', 'sky': '#10225C'}
    # A blue neon title, like the grid (white type in an orange glow fringes lavender).
    e['titleShadow']['color'] = '#2F7BFF'


edit('retrowave', {
    'background': '#070B1F', 'surface': '#B30C1433', 'card': '#B3121D45', 'cardStroke': '#552F7BFF',
    'control': '#CC18264F', 'controlStroke': '#66FF7A33', 'subtle': '#332F7BFF', 'divider': '#442F7BFF',
    'text': '#EEF2FF', 'textSecondary': '#A9B6DC', 'textTertiary': '#6F7CA3', 'accent': '#FF6A2B',
    'accentText': '#FFA55C', 'onAccent': '#070B1F', 'dialog': '#0E1636',
}, 'A midnight drive: a molten orange sun sinking into an electric-blue grid.', retrowave)


# Night Market: the lanterns and signs burn vermilion and amber, with cold blue tubes along the panels.
def night_market(e):
    e['windowTitle']['color'] = '#FFD7A8'
    # Amber: a red glow over the blue tubes would make magenta.
    e['titleShadow']['color'] = '#FFB347'
    e['neonEdges'].update({'content': '#3D8BFF', 'player': '#FF5A36', 'bar': '#FF5A36'})
    layers = e['scene']['layers']
    stops('FFB347', layers[4])
    stops('9CC2FF', layers[5])
    stops('FF7A52', layers[9])
    layers[14].update({'color': '#FFB58A', 'glow': '#FF5A36'})
    layers[15]['glow'] = '#FF5A36'


edit('night-market', {
    'cardStroke': '#403D8BFF', 'divider': '#4D3D8BFF', 'subtle': '#263D8BFF', 'controlStroke': '#59FF5A36',
    'accent': '#FF5A36', 'accentText': '#FFB45E', 'onAccent': '#120804',
}, fn=night_market)


def neon_noir(e):
    e['titleShadow']['color'] = '#4A90FF'
    e['windowTitle']['color'] = '#9CC2FF'


edit('neon-noir', {'cardStroke': '#334A90FF', 'controlStroke': '#444A90FF', 'divider': '#334A90FF', 'subtle': '#264A90FF'},
     'Rain over a real neon street at night in Tokyo, amber and steel blue, behind smoky glass.', neon_noir)


# DOS: CGA's yellow on blue (the file managers' look), and a sand-yellow hourglass.
def dos(e):
    e['tiles']['colors'] = ['#0000AA', '#AA5500', '#00AA00']
    e['scene']['layers'][1]['palette']['Y'] = '#FFFF55'
    e['visTint']['ink'] = '#FFFF55'


edit('dos-prompt', {'subtle': '#33AAAAAA', 'accent': '#FFFF55', 'accentText': '#FFFF55'},
     'A homage to DOS: grey text on black, yellow on blue, a prompt in the title bar and box-drawn borders.', dos)

edit('home-screen', {'background': '#0E1A33', 'textSecondary': '#ECF0F6', 'textTertiary': '#C3CAD6', 'accentText': '#6CB4FF'})
# Light blues right at teal's edge read aqua: plain sky blue.
edit('fluent', {'accentText': '#9CCBFF'})


# Super 16: the grey console of the 16-bit era, its four coloured buttons and a blue for everything you press.
def super16(e):
    e['tiles']['colors'] = ['#4A4D57', '#2E9E4F', '#D93D3D', '#3D5FD9', '#B8962A']
    e['windowTitle']['color'] = '#2F5BD3'
    e['scene']['layers'][5]['color'] = '#2F5BD3'


edit('super-16', {
    'background': '#C8C9CE', 'surface': '#E6E4E5E9', 'card': '#F3F3F5', 'cardStroke': '#8A8D99',
    'control': '#E9E9EC', 'controlStroke': '#8A8D99', 'subtle': '#332F5BD3', 'divider': '#A9AAB2',
    'text': '#25262E', 'textSecondary': '#4A4C58', 'textTertiary': '#767986', 'accent': '#2F5BD3',
    'accentText': '#2550C0', 'dialog': '#EDEDF0', 'pane': '#B9BAC0',
}, "A homage to the 16-bit era's grey console: soft greys, four coloured buttons and a star.", super16)


# Retro 64: the border and screen in true blues.
def retro64(e):
    e['frame']['color'] = '#5A7BD8'
    e['cursor']['color'] = '#A6B8F2'


edit('retro-64', {
    'background': '#1E3488', 'surface': '#1E3488', 'card': '#27409A', 'cardStroke': '#5A7BD8', 'control': '#27409A',
    'controlStroke': '#5A7BD8', 'subtle': '#405A7BD8', 'divider': '#5A7BD8', 'text': '#A6B8F2',
    'textSecondary': '#869DE0', 'textTertiary': '#5A7BD8', 'accent': '#5A7BD8', 'accentText': '#C2D0FF',
    'onAccent': '#0F1D52', 'dialog': '#1E3488',
}, fn=retro64)


def ninety_five(e):
    e['tiles']['colors'][2] = '#808000'


edit('ninety-five', {'background': '#3A6EA5', 'subtle': '#33002A9C'},
     'A homage to Windows 95: a blue desktop, grey 3D windows, and a navy-to-blue title bar.', ninety_five)


def program_manager(e):
    e['tiles']['colors'][1] = '#808000'


# Navy at a fifth over white is lavender: a bluer navy.
edit('program-manager', {'subtle': '#33002A9C'}, fn=program_manager)

edit('big-glass', {'surface': '#4D121620', 'card': '#731A1E28'})


def frosted(e):
    e['tiles']['colors'] = ['#7FB8E0', '#E8B87F', '#93C79A']
    e['aurora']['colors'] = ['#8EC5FF', '#FFD3A1', '#BFE3C0']


edit('frosted-glass', fn=frosted)

edit('luna', {'pane': '#C6D6E6F8'})


# Platinum: classic Mac OS's violet-grey accent turned slate blue (it read purple, if only just).
def platinum(e):
    e['tiles']['colors'] = ['#6F86B0', '#5F8EB0', '#8E8E8E']


edit('platinum', {'accent': '#5B7299', 'accentText': '#2E4F99', 'subtle': '#335B7299'}, fn=platinum)

# Aurora Glass: smoky glass that's neutral, not blue-black (a blue tint over the aurora's green edges is teal).
edit('aurora-glass', {'pane': '#8C0A0C10', 'surface': '#660A0D12', 'card': '#8C10141A', 'dialog': '#E610141A'})
print('themes recoloured')
