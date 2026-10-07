"""Перегляд частин: Workbench-рендер голови з чотирьох боків в один аркуш PNG."""

import math
import bpy
import numpy as np
from mathutils import Vector

PART_PREFIXES = ("hair_", "face_", "beard_", "hat_", "glasses_", "outfit_")
VIEWS = [("front", 0.0), ("three_quarter", 40.0), ("side", 90.0), ("back", 180.0)]


def _camera(scale):
    cam_data = bpy.data.cameras.new("PreviewCam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = scale
    cam = bpy.data.objects.new("PreviewCam", cam_data)
    bpy.context.scene.collection.objects.link(cam)
    return cam


def render_sheet(path, target=Vector((0.0, -0.037, 1.66)), scale=0.62, size=360, elev=8.0, hide=(), color_type="MATERIAL"):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.render.resolution_x = size
    scene.render.resolution_y = size
    scene.render.film_transparent = False
    shading = scene.display.shading
    shading.light = "STUDIO"
    shading.color_type = color_type
    shading.show_object_outline = False
    world = scene.world or bpy.data.worlds.new("W")
    scene.world = world
    world.color = (0.62, 0.72, 0.62)
    cam = _camera(scale)
    scene.camera = cam
    tiles = []
    for name, yaw in VIEWS:
        a = math.radians(yaw)
        # обличчя дивиться в −Y; камера стоїть перед ним і обертається довкола цілі
        offset = Vector((math.sin(a) * 2.0, -math.cos(a) * 2.0, math.sin(math.radians(elev)) * 2.0))
        cam.location = target + offset
        direction = target - cam.location
        cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = f"{path}.{name}.png"
        bpy.ops.render.render(write_still=True)
        img = bpy.data.images.load(f"{path}.{name}.png")
        tiles.append(np.array(img.pixels[:]).reshape(size, size, 4))
        bpy.data.images.remove(img)
    sheet = np.hstack(tiles)
    out = bpy.data.images.new("sheet", sheet.shape[1], sheet.shape[0], alpha=True)
    out.pixels = sheet.flatten().tolist()
    out.filepath_raw = path
    out.file_format = "PNG"
    out.save()
    bpy.data.images.remove(out)
    bpy.data.objects.remove(cam, do_unlink=True)
    import os
    for name, _ in VIEWS:
        try:
            os.remove(f"{path}.{name}.png")
        except OSError:
            pass


def render_grid(objs, path, yaw=40.0, cols=3, size=300, scale=0.62, target=Vector((0.0, -0.037, 1.66)), elev=8.0, color_type="MATERIAL"):
    """Кожна частина окремо, один ракурс, усе в одну картинку сіткою."""
    scene = bpy.context.scene
    tiles = []
    everything = [o for o in bpy.data.objects if o.name.startswith(PART_PREFIXES)]
    saved = [(o, o.hide_render) for o in everything]
    for o in objs:
        for other in everything:
            other.hide_render = other is not o
        global VIEWS
        old = VIEWS
        VIEWS = [("v", yaw)]
        render_sheet(f"{path}.tmp.png", target=target, scale=scale, size=size, elev=elev, color_type=color_type)
        VIEWS = old
        img = bpy.data.images.load(f"{path}.tmp.png")
        tiles.append(np.array(img.pixels[:]).reshape(size, size, 4))
        bpy.data.images.remove(img)
    for o, h in saved:
        o.hide_render = h
    while len(tiles) % cols:
        tiles.append(np.zeros_like(tiles[0]))
    rows = [np.hstack(tiles[i:i + cols]) for i in range(0, len(tiles), cols)]
    sheet = np.vstack(rows[::-1])  # піксельні ряди Blender знизу вгору
    out = bpy.data.images.new("grid", sheet.shape[1], sheet.shape[0], alpha=True)
    out.pixels = sheet.flatten().tolist()
    out.filepath_raw = path
    out.file_format = "PNG"
    out.save()
    bpy.data.images.remove(out)
    import os
    try:
        os.remove(f"{path}.tmp.png")
    except OSError:
        pass


def render_body_views(path, size=520, yaws=(0.0, 45.0, 90.0, 180.0), target=Vector((0.0, -0.03, 0.95)), scale=2.05, elev=4.0,
                      color_type="MATERIAL"):
    """Ціле тіло з кількох боків, один рядок."""
    scene = bpy.context.scene
    global VIEWS
    old = VIEWS
    VIEWS = [(f"y{int(y)}", y) for y in yaws]
    try:
        render_sheet(path, target=target, scale=scale, size=size, elev=elev, color_type=color_type)
    finally:
        VIEWS = old
