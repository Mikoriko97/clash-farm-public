"""Будівля партнера Sanctum за референсами власника (фото 2026-09-11 і відео з
обертанням моделі): один витягнутий корпус-капсула (суперелліпс у плані,
гранований синій дах-купол, суцільна скляна стрічка з білими імпостами,
біла окантовка, синя спідниця), вежа з емблемою-хмаркою і відкритим
стрілчастим порталом врізана спереду, шпиль із кільцем і кристалом. Усередині:
підлога, внутрішня оболонка стін, світло, постамент із кристалом. Сходи з
невидимим пандусом для колайдера.

    blender -b --python client/tools/build_sanctum.py [-- --render DIR]

Пише client/blender/sanctum_temple.blend і client/assets/partners/sanctum_temple.glb
(Y-up, півот у центрі підошви, фасад у −Y Blender = +Z Godot). Осі: X вправо,
−Y вперед, Z вгору; рівень підлоги — FLOOR.
"""

import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sanctum_lib import arch_ring, box, cylinder, diamond, frustum, loft, outline_with_cuts, parts, posts_along, slab, superellipse, torus, wedge  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
BLEND = ROOT / "blender" / "sanctum_temple.blend"
GLB = ROOT / "assets" / "partners" / "sanctum_temple.glb"
NAME = "sanctum_temple"
FLOOR = 1.0
WALL = 0.32
# Hull plan: superellipse half-axes measured from the owner's FBX (17 × 15 m, nearly round).
HA, HB, HC = 8.6, 7.4, (0.0, 0.6)
# Tower footprint (outer, 6.2 m wide); its front sticks out ~2 m past the hull.
TX, TY0, TY1, TH = 3.1, -8.9, -3.4, 12.6
# The hull outline gets vertices exactly on the tower's side walls, so the opening for the
# tower ends on those walls: no half-faces poking into the atrium, no open ring ends.
OUTLINE = outline_with_cuts(superellipse(HA, HB, 3.0, 32, HC), (-TX, TX), HC[1])
# Height profile of the hull (FBX bands): vertical walls to the rim, then the brim and the dome.
RINGS = [(FLOOR, 1.0), (FLOOR + 0.6, 1.0), (FLOOR + 1.5, 1.0), (FLOOR + 1.9, 1.0), (FLOOR + 4.0, 1.0), (FLOOR + 4.6, 1.0), (FLOOR + 5.4, 1.06), (FLOOR + 6.2, 0.96), (FLOOR + 7.4, 0.74), (FLOOR + 8.4, 0.4), (FLOOR + 8.9, 0.12)]
BANDS = ["blue", "blue", "white", "glass", "white", "white", "blue", "blue", "blue", "blue"]


def tower_gap(x, y, z0, z1):
    """Every hull face inside the tower footprint (all heights) is dropped: the tower's own
    walls and cap close that volume, so nothing of the hull or dome shows inside the atrium."""
    return -TX + 0.01 < x < TX - 0.01 and y < HC[1] - 3.0


def build_base():
    base = superellipse(HA + 1.3, HB + 1.3, 3.2, 32, HC)
    loft("base_lower", base, [(0.0, 1.0), (0.35, 1.0)], ["white"])
    loft("base_blue", base, [(0.35, 0.96), (0.7, 0.96)], ["blue"])
    loft("base_floor", base, [(0.7, 0.93), (FLOOR, 0.93)], ["white"])
    slab("porch", "white", -3.4, 3.4, TY0 - 1.2, HC[1] - HB, 0.7, FLOOR)
    slab("porch_lip", "blue", -3.6, 3.6, TY0 - 1.4, TY0 - 1.0, 0.35, 0.7)
    for i in range(5):
        top = FLOOR - 0.2 * (i + 1)
        slab(f"step_{i}", "white", -2.4, 2.4, TY0 - 1.2 - 0.45 * (i + 1), TY0 - 1.2 - 0.45 * i, 0.0, top)
    # Starts below ground so the capsule meets only the slope, never a vertical lip.
    wedge("collision_ramp", "hidden", -2.3, 2.3, TY0 - 4.6, TY0 - 1.2, -0.4, FLOOR)


def build_hull():
    loft("hull", OUTLINE, RINGS, BANDS, skip=tower_gap)
    inner = [(z, s * 0.955) for z, s in RINGS]
    inner_bands = ["blue", "blue", "white", "hidden", "white", "white", "white", "white", "white", "white"]
    loft("hull_inner", OUTLINE, inner, inner_bands, skip=tower_gap, inside=True)
    # White sill under the glass and a rim above it, slightly proud of the wall.
    loft("sill", OUTLINE, [(FLOOR + 1.45, 1.02), (FLOOR + 1.65, 1.03), (FLOOR + 1.9, 1.02)], ["white", "white"], skip=tower_gap, cap=None)
    loft("rim", OUTLINE, [(FLOOR + 4.0, 1.02), (FLOOR + 4.25, 1.035), (FLOOR + 4.6, 1.01)], ["white", "white"], skip=tower_gap, cap=None)
    posts_along("mullion", OUTLINE, FLOOR + 1.9, FLOOR + 4.0, every=2, skip=tower_gap)
    # Roof ridge line: a thin white spine along the crest, like the reference.
    slab("ridge", "white", HC[0] - HA * 0.5, HC[0] + HA * 0.5, HC[1] - 0.25, HC[1] + 0.25, FLOOR + 8.55, FLOOR + 8.85, bevel=0.1)
    slab("ceiling_glow", "cream", HC[0] - 5.0, HC[0] + 5.0, HC[1] - 3.0, HC[1] + 3.0, FLOOR + 5.0, FLOOR + 5.15)


def build_tower_walls():
    slab("pilaster_l", "white", -TX, -1.95, TY0, TY0 + WALL + 0.15, FLOOR, TH)
    slab("pilaster_r", "white", 1.95, TX, TY0, TY0 + WALL + 0.15, FLOOR, TH)
    slab("lintel", "blue", -1.95, 1.95, TY0 + 0.1, TY0 + WALL, 6.2, TH)
    slab("tower_front_upper", "white", -TX, TX, TY1 - 0.3, TY1, FLOOR + 5.0, TH)
    arch_ring("portal_ring", "white", (1.95, 3.6, 6.2), (1.35, 3.2, 5.2), WALL + 0.3, (0.0, TY0 - 0.3, FLOOR))
    arch_ring("portal_glow", "cream", (1.35, 3.2, 5.2), (1.1, 3.0, 4.7), 0.2, (0.0, TY0 + 0.05, FLOOR))
    for sx, tag in ((-1.0, "l"), (1.0, "r")):
        # Wall spans from inside the atrium to 0.35 m outside the hull cut, swallowing ring ends.
        x0, x1 = (TX - WALL, TX + 0.35) if sx > 0 else (-TX - 0.35, -TX + WALL)
        slab(f"tower_side_{tag}", "white", x0, x1, TY0, TY1 + 0.6, FLOOR, TH)
        slab(f"tower_side_panel_{tag}", "blue", x0 - (0.12 if sx < 0 else 0.0), x1 + (0.12 if sx > 0 else 0.0), TY0 + 1.0, TY1 + 0.3, FLOOR + 9.3, TH - 0.6)


def build_tower_top():
    cy = (TY0 + TY1) * 0.5
    frustum("tower_cap", "white", (2 * TX + 0.6, TY1 - TY0 + 0.6), (2 * TX - 0.8, TY1 - TY0 - 0.8), 1.0, (0.0, cy, TH))
    frustum("tower_roof", "blue", (2 * TX - 0.8, TY1 - TY0 - 0.8), (2 * TX - 2.6, TY1 - TY0 - 2.6), 0.9, (0.0, cy, TH + 1.0))
    slab("tower_ceiling_glow", "cream", -2.2, 2.2, TY0 + 0.6, TY1 - 0.4, TH - 0.25, TH - 0.05)
    cylinder("spire_base", "white", 0.95, 0.5, (0.0, cy, TH + 1.9), verts=16)
    cylinder("spire", "white", 0.5, 1.6, (0.0, cy, TH + 2.4), verts=12)
    torus("halo", "white", 1.9, 0.18, (0.0, cy, TH + 4.5), tilt=14.0)
    diamond("crystal", "crystal", 0.95, 3.0, (0.0, cy, TH + 5.2))
    ey = TY0 - 0.2
    ez = TH - 2.6
    cylinder("emblem_ring", "white", 1.75, 0.35, (0.0, ey + 0.05, ez), verts=32, axis="Y")
    cylinder("emblem_disc", "blue", 1.5, 0.45, (0.0, ey, ez), verts=32, axis="Y")
    for dx, r, dz in ((-0.55, 0.42, -0.15), (0.0, 0.62, 0.05), (0.55, 0.42, -0.15)):
        cylinder(f"cloud_{dx:+.2f}", "white", r, 0.2, (dx, ey - 0.3, ez - 0.05 + dz), verts=16, axis="Y")
    box("cloud_base", "white", (1.7, 0.2, 0.5), (0.0, ey - 0.3, ez - 0.55))
    for dx in (-0.16, 0.16):
        box(f"cloud_eye_{dx:+.2f}", "dark_blue", (0.12, 0.12, 0.42), (dx, ey - 0.45, ez - 0.3))


def add_light(name, at, color, energy, radius):
    data = bpy.data.lights.new(name, "POINT")
    data.color = color
    data.energy = energy
    data.shadow_soft_size = 0.5
    data.use_custom_distance = True
    data.cutoff_distance = radius
    obj = bpy.data.objects.new(name, data)
    obj.location = at
    bpy.context.scene.collection.objects.link(obj)
    return obj


def build_interior():
    add_light("light_atrium", (0.0, -6.0, FLOOR + 5.5), (0.55, 0.75, 1.0), 900.0, 12.0)
    add_light("light_wing_l", (HC[0] - 4.5, HC[1] + 1.0, FLOOR + 4.2), (1.0, 0.9, 0.7), 600.0, 10.0)
    add_light("light_wing_r", (HC[0] + 4.5, HC[1] + 1.0, FLOOR + 4.2), (1.0, 0.9, 0.7), 600.0, 10.0)
    cylinder("pedestal", "white", 0.7, 0.9, (0.0, HC[1], FLOOR), verts=16)
    cylinder("pedestal_top", "blue", 0.8, 0.12, (0.0, HC[1], FLOOR + 0.9), verts=16)
    diamond("atrium_crystal", "crystal", 0.45, 1.4, (0.0, HC[1], FLOOR + 1.9))


def join_and_export():
    bpy.ops.object.select_all(action="DESELECT")
    # Ground level = lowest visible vertex; the buried ramp lead-in must not lift the building.
    lowest = min((o.matrix_world @ v.co).z for o in parts if o.name != "collision_ramp" for v in o.data.vertices)
    # The buried ramp stays its own object: Godot's export check and the post-import
    # script recognise it by name (collision_*), and it never enters the visible bounds.
    ramp = next(o for o in parts if o.name == "collision_ramp")
    for obj in parts:
        if obj is not ramp:
            obj.select_set(True)
    bpy.context.view_layer.objects.active = next(o for o in parts if o is not ramp)
    bpy.ops.object.convert(target="MESH")
    bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = NAME
    obj.data.name = NAME
    for o in (obj, ramp):
        o.location.z -= lowest
        o.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    for light in (o for o in bpy.data.objects if o.type == "LIGHT" and o.name.startswith("light_")):
        light.select_set(True)
    GLB.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(GLB), export_format="GLB", use_selection=True, export_yup=True, export_apply=True, export_materials="EXPORT", export_animations=False, export_lights=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(BLEND))
    print(f"build_sanctum: {tris} triangles, {len(obj.data.vertices)} vertices -> {GLB}")


def render_views(out_dir: Path):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x, scene.render.resolution_y = 960, 720
    world = bpy.data.worlds.new("w") if scene.world is None else scene.world
    scene.world = world
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.12, 0.12, 0.16, 1.0)
    world.node_tree.nodes["Background"].inputs[1].default_value = 1.2
    bpy.ops.object.light_add(type="SUN", location=(10, -10, 20))
    sun = bpy.context.active_object
    sun.rotation_euler = (math.radians(50), math.radians(10), math.radians(-35))
    sun.data.energy = 3.5
    bpy.ops.object.camera_add()
    cam = bpy.context.active_object
    cam.data.lens = 40
    scene.camera = cam
    views = {"hero": ((22.0, -28.0, 14.0), (0.0, -0.5, 6.5)), "junction": ((9.0, -14.0, 4.0), (3.4, -6.5, 4.0)), "atrium": ((0.0, -7.5, 1.9), (2.5, -3.0, 3.5)), "roof_join": ((10.0, -10.0, 12.0), (3.2, -5.0, 8.0))}
    for name, (loc, target) in views.items():
        cam.location = loc
        cam.rotation_euler = (Vector(target) - Vector(loc)).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = str(out_dir / f"sanctum_{name}.png")
        bpy.ops.render.render(write_still=True)
    print("build_sanctum: rendered", list(views))


def main():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    build_base()
    build_hull()
    build_tower_walls()
    build_tower_top()
    build_interior()
    join_and_export()
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if "--render" in argv:
        render_views(Path(argv[argv.index("--render") + 1]))


main()
