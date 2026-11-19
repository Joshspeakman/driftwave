"""Renders a rain-soaked city street at night in Blender (Cycles), for the Night Market and Neon Noir backdrops.

Original 3D scene: a narrow street, buildings with lit windows, neon blade signs (Yu Gothic), shopfronts, lanterns,
people with umbrellas, a glossy wet road with puddles, volumetric haze. Rain is not rendered: the app animates it.
Usage: blender --background --factory-startup --python windows/tools/render-city.py -- <variant> <out.png> [samples]
Variants: market (Japanese signs, warm and pink), noir (sparser, amber and cyan).
"""
import math
import random
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
VARIANT = argv[0] if argv else 'market'
OUT = argv[1] if len(argv) > 1 else 'C:/tmp/city.png'
SAMPLES = int(argv[2]) if len(argv) > 2 else 96
rnd = random.Random(7 if VARIANT == 'market' else 23)

JP_FONT = 'C:/Windows/Fonts/YuGothB.ttc'
LATIN_FONT = 'C:/Windows/Fonts/bahnschrift.ttf'

PALETTES = {
    'market': {'neon': [(1.0, 0.12, 0.45), (0.1, 0.85, 1.0), (0.75, 0.3, 1.0), (1.0, 0.7, 0.15), (0.3, 1.0, 0.55), (1.0, 0.35, 0.2)],
               'signs': ['ラーメン', '居酒屋', 'カラオケ', '電脳', 'ホテル', '薬', '寿司', '夜市', '酒', '焼鳥', '喫茶', '質屋', '電話', '二十四時'],
               'haze': (0.55, 0.35, 0.75), 'sign_density': 1.0, 'lanterns': True},
    'noir': {'neon': [(1.0, 0.6, 0.15), (0.15, 0.85, 1.0), (1.0, 0.35, 0.1)],
             'signs': ['HOTEL', 'BAR', 'NOODLES', '電話', 'OPEN', 'TAXI', '酒'],
             'haze': (0.35, 0.5, 0.65), 'sign_density': 0.55, 'lanterns': False},
}
P = PALETTES[VARIANT]

# MARK: scene and render settings
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
prefs = bpy.context.preferences.addons['cycles'].preferences
for backend in ('OPTIX', 'CUDA'):
    try:
        prefs.compute_device_type = backend
        prefs.get_devices()
        for d in prefs.devices:
            d.use = True
        scene.cycles.device = 'GPU'
        break
    except Exception:
        continue
scene.cycles.samples = SAMPLES
scene.cycles.use_denoising = True
scene.cycles.max_bounces = 6
scene.cycles.volume_bounces = 1
scene.render.resolution_x, scene.render.resolution_y = 1920, 1080
scene.render.film_transparent = False
try:
    scene.view_settings.view_transform = 'AgX'
    scene.view_settings.look = 'AgX - Punchy'
except Exception:
    pass
scene.view_settings.exposure = -0.4

world = bpy.data.worlds.new('Night')
scene.world = world
world.use_nodes = True
wn = world.node_tree.nodes
wn.clear()
bg = wn.new('ShaderNodeBackground')
bg.inputs['Color'].default_value = (0.004, 0.006, 0.014, 1)
bg.inputs['Strength'].default_value = 1.0
vol = wn.new('ShaderNodeVolumeScatter')
vol.inputs['Density'].default_value = 0.012
vol.inputs['Color'].default_value = (*P['haze'], 1)
out = wn.new('ShaderNodeOutputWorld')
world.node_tree.links.new(bg.outputs[0], out.inputs['Surface'])
world.node_tree.links.new(vol.outputs[0], out.inputs['Volume'])


def mat(name, build):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    o = nt.nodes.new('ShaderNodeOutputMaterial')
    nt.links.new(build(nt), o.inputs['Surface'])
    return m


def emission(color, strength):
    def b(nt):
        e = nt.nodes.new('ShaderNodeEmission')
        e.inputs['Color'].default_value = (*color, 1)
        e.inputs['Strength'].default_value = strength
        return e.outputs[0]
    return mat(f'em{color}{strength}', b)


def principled(color, rough=0.6, metal=0.0):
    def b(nt):
        p = nt.nodes.new('ShaderNodeBsdfPrincipled')
        p.inputs['Base Color'].default_value = (*color, 1)
        p.inputs['Roughness'].default_value = rough
        p.inputs['Metallic'].default_value = metal
        return p.outputs[0]
    return mat(f'pr{color}{rough}', b)


def facade_material(seed):
    """Concrete facade with a grid of windows, each lit (or not) in its own colour."""
    def b(nt):
        n = nt.nodes
        coord = n.new('ShaderNodeTexCoord')
        sep = n.new('ShaderNodeSeparateXYZ')
        nt.links.new(coord.outputs['Object'], sep.inputs[0])
        yz = n.new('ShaderNodeCombineXYZ')
        nt.links.new(sep.outputs['Y'], yz.inputs['X'])
        nt.links.new(sep.outputs['Z'], yz.inputs['Y'])
        nt.links.new(sep.outputs['X'], yz.inputs['Z'])
        brick = n.new('ShaderNodeTexBrick')
        brick.offset = 0.0
        brick.squash = 1.0
        brick.inputs['Scale'].default_value = 1.0
        brick.inputs['Mortar Size'].default_value = 0.45 + (seed % 3) * 0.08
        brick.inputs['Mortar Smooth'].default_value = 0.0
        brick.inputs['Brick Width'].default_value = 1.9 + (seed % 3) * 0.3
        brick.inputs['Row Height'].default_value = 3.2
        brick.inputs['Bias'].default_value = -0.55
        brick.inputs['Color1'].default_value = (1.0, 0.72, 0.42, 1)
        brick.inputs['Color2'].default_value = (0.0, 0.0, 0.0, 1)
        brick.inputs['Mortar'].default_value = (0.0, 0.0, 0.0, 1)
        nt.links.new(yz.outputs[0], brick.inputs['Vector'])
        # Tint: some windows cool, some pink
        noise = n.new('ShaderNodeTexWhiteNoise')
        noise.noise_dimensions = '3D'
        snap = n.new('ShaderNodeVectorMath')
        snap.operation = 'SNAP'
        snap.inputs[1].default_value = (1.9, 3.2, 100)
        nt.links.new(yz.outputs[0], snap.inputs[0])
        nt.links.new(snap.outputs[0], noise.inputs['Vector'])
        ramp = n.new('ShaderNodeValToRGB')
        cr = ramp.color_ramp
        cr.interpolation = 'CONSTANT'
        cr.elements[0].position = 0.0
        cr.elements[0].color = (1.0, 0.78, 0.5, 1)
        cr.elements[1].position = 0.6
        cr.elements[1].color = (0.6, 0.85, 1.0, 1)
        e3 = cr.elements.new(0.85)
        e3.color = (1.0, 0.55, 0.75, 1)
        nt.links.new(noise.outputs['Value'], ramp.inputs['Fac'])
        mix = n.new('ShaderNodeMix')
        mix.data_type = 'RGBA'
        mix.blend_type = 'MULTIPLY'
        mix.inputs['Factor'].default_value = 1.0
        nt.links.new(brick.outputs['Color'], mix.inputs['A'])
        nt.links.new(ramp.outputs['Color'], mix.inputs['B'])
        em = n.new('ShaderNodeEmission')
        em.inputs['Strength'].default_value = 0.9
        nt.links.new(mix.outputs['Result'], em.inputs['Color'])
        wall = n.new('ShaderNodeBsdfPrincipled')
        wall.inputs['Base Color'].default_value = (0.03, 0.03, 0.04, 1)
        wall.inputs['Roughness'].default_value = 0.8
        add = n.new('ShaderNodeAddShader')
        nt.links.new(wall.outputs[0], add.inputs[0])
        nt.links.new(em.outputs[0], add.inputs[1])
        return add.outputs[0]
    return mat(f'facade{seed}', b)


def wet_road():
    """Dark asphalt with puddles: rough where dry, mirror-like where wet."""
    def b(nt):
        n = nt.nodes
        p = n.new('ShaderNodeBsdfPrincipled')
        p.inputs['Base Color'].default_value = (0.012, 0.012, 0.016, 1)
        noise = n.new('ShaderNodeTexNoise')
        noise.inputs['Scale'].default_value = 0.35
        noise.inputs['Detail'].default_value = 6
        ramp = n.new('ShaderNodeValToRGB')
        ramp.color_ramp.elements[0].position = 0.45
        ramp.color_ramp.elements[0].color = (0.1, 0.1, 0.1, 1)
        ramp.color_ramp.elements[1].position = 0.62
        ramp.color_ramp.elements[1].color = (0.55, 0.55, 0.55, 1)
        nt.links.new(noise.outputs['Fac'], ramp.inputs['Fac'])
        nt.links.new(ramp.outputs['Color'], p.inputs['Roughness'])
        fine = n.new('ShaderNodeTexNoise')
        fine.inputs['Scale'].default_value = 60
        bump = n.new('ShaderNodeBump')
        bump.inputs['Strength'].default_value = 0.08
        nt.links.new(fine.outputs['Fac'], bump.inputs['Height'])
        nt.links.new(bump.outputs['Normal'], p.inputs['Normal'])
        return p.outputs[0]
    return mat('road', b)


def box(name, loc, size, material):
    bpy.ops.mesh.primitive_cube_add(location=loc)
    o = bpy.context.object
    o.name = name
    o.scale = (size[0] / 2, size[1] / 2, size[2] / 2)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)   # object coordinates in metres
    o.data.materials.append(material)
    return o


def text(body, loc, rot, size, material, font_path, vertical=False, extrude=0.03):
    curve = bpy.data.curves.new('t', 'FONT')
    curve.body = '\n'.join(body) if vertical else body
    curve.font = bpy.data.fonts.load(font_path, check_existing=True)
    curve.size = size
    curve.extrude = extrude
    curve.align_x = 'CENTER'
    curve.align_y = 'CENTER'
    if vertical:
        curve.space_line = 0.95
    o = bpy.data.objects.new('t', curve)
    scene.collection.objects.link(o)
    o.location = loc
    o.rotation_euler = rot
    o.data.materials.append(material)
    return o


# MARK: build the street
street_half = 4.2
road = wet_road()
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 60, 0))
ground = bpy.context.object
ground.scale = (40, 140, 1)
ground.data.materials.append(road)
kerb = principled((0.05, 0.05, 0.06), 0.5)
for side in (-1, 1):
    box('kerb', (side * (street_half + 0.6), 60, 0.08), (1.2, 140, 0.16), kerb)

dark = principled((0.02, 0.02, 0.025), 0.7)
board = principled((0.015, 0.013, 0.02), 0.35)
y = 2.0
for side in (-1, 1):
    y = 2.0
    k = 0
    while y < 120:
        depth = rnd.uniform(5, 11)
        height = rnd.uniform(9, 34) if y < 60 else rnd.uniform(18, 60)
        x = side * (street_half + 1.2 + 4)
        bld = box('bld', (x, y + depth / 2, height / 2), (8, depth, height), facade_material(k + (0 if side < 0 else 50)))
        # Shopfront glowing at street level
        shop = emission(rnd.choice([(1.0, 0.62, 0.32), (1.0, 0.82, 0.6), (0.55, 0.85, 1.0), (1.0, 0.5, 0.7)]), rnd.uniform(1.2, 2.6))
        box('shop', (side * (street_half + 1.21), y + depth / 2, 1.4), (0.05, depth * 0.8, 2.2), shop)
        box('awning', (side * (street_half + 0.9), y + depth / 2, 2.75), (0.8, depth * 0.85, 0.12), dark)
        # Neon blade signs sticking out into the street, facing the camera
        if rnd.random() < P['sign_density'] and y < 90:
            label = rnd.choice(P['signs'])
            col = rnd.choice(P['neon'])
            latin = all(ord(ch) < 128 for ch in label)
            vertical = not latin
            h = (len(label) * 0.95 + 0.6) if vertical else 1.4
            w = 1.3 if vertical else len(label) * 0.62 + 0.6
            sy = y + rnd.uniform(0.5, depth - 0.5)
            sz = rnd.uniform(3.6, 10)
            sx = side * (street_half + 1.2 - w / 2 - 0.15)
            box('board', (sx, sy, sz), (w, 0.18, h), board)
            text(label, (sx, sy - 0.12, sz), (math.radians(90), 0, 0), 0.92, emission(col, 5), LATIN_FONT if latin else JP_FONT, vertical)
            # Thin neon outline around the board
            for dz in (-h / 2, h / 2):
                box('tube', (sx, sy - 0.1, sz + dz), (w, 0.04, 0.04), emission(col, 3.5))
            light = bpy.data.lights.new('neon', 'POINT')
            light.energy = 45
            light.color = col
            light.shadow_soft_size = 0.6
            lo = bpy.data.objects.new('neon', light)
            scene.collection.objects.link(lo)
            lo.location = (sx, sy - 0.8, sz)
        y += depth + rnd.uniform(0.2, 1.4)
        k += 1

# Lanterns over a noodle stall on the left, and their warm light
if P['lanterns']:
    red = emission((1.0, 0.16, 0.05), 4)
    for i in range(6):
        ly = 9 + i * 1.3
        bpy.ops.mesh.primitive_uv_sphere_add(radius=0.17, location=(-street_half - 0.2, ly, 2.45))
        bpy.context.object.scale = (1, 1, 1.35)
        bpy.context.object.data.materials.append(red)
        light = bpy.data.lights.new('lantern', 'POINT')
        light.energy = 25
        light.color = (1.0, 0.35, 0.15)
        lo = bpy.data.objects.new('lantern', light)
        scene.collection.objects.link(lo)
        lo.location = (-street_half - 0.1, ly, 2.2)

# People with umbrellas
person = principled((0.01, 0.01, 0.012), 0.6)
for i in range(9):
    px, py = rnd.uniform(-3.2, 3.2), rnd.uniform(9, 45)
    bpy.ops.mesh.primitive_cylinder_add(radius=0.22, depth=1.55, location=(px, py, 0.78))
    bpy.context.object.data.materials.append(person)
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.13, location=(px, py, 1.66))
    bpy.context.object.data.materials.append(person)
    umbrella_col = rnd.choice([(0.02, 0.02, 0.03), (0.02, 0.02, 0.03), (0.6, 0.05, 0.08), (0.05, 0.1, 0.3)])
    bpy.ops.mesh.primitive_cone_add(radius1=0.7, radius2=0.02, depth=0.35, location=(px, py, 2.15))
    bpy.context.object.data.materials.append(principled(umbrella_col, 0.35))

# Overhead cables across the street
cable = principled((0.01, 0.01, 0.01), 0.5)
for i in range(7):
    cy = rnd.uniform(6, 50)
    bpy.ops.mesh.primitive_cylinder_add(radius=0.012, depth=street_half * 2 + 4, location=(0, cy, rnd.uniform(6, 9)), rotation=(0, math.radians(90), 0))
    bpy.context.object.data.materials.append(cable)

# Camera at eye height, looking down the street
cam_data = bpy.data.cameras.new('cam')
cam_data.lens = 26
cam = bpy.data.objects.new('cam', cam_data)
scene.collection.objects.link(cam)
cam.location = (0.4, -3.5, 1.65)
direction = Vector((0, 40, 4.5)) - cam.location
cam.rotation_euler = direction.to_track_quat('-Z', 'Y').to_euler()
scene.camera = cam

scene.render.filepath = OUT
scene.render.image_settings.file_format = 'PNG'
bpy.ops.render.render(write_still=True)
print('rendered', OUT)
