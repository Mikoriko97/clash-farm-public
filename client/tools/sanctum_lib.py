"""Примітиви для build_sanctum.py: матеріали, плити, усічені блоки, циліндри,
арки-кільця (отвір без булевих операцій — лофт між двома профілями), кристал,
тор. Кожна деталь — окремий обʼєкт, який реєструється для фінального join.
Тільки квади/трикутники з примітивів; bevel — модифікатор із 2 сегментами.
"""

import math

import bmesh
import bpy
from mathutils import Vector

# key → (colour, emission strength | None, alpha | None)
PALETTE = {
    "white": ((0.93, 0.93, 0.96, 1.0), None, None),
    "blue": ((0.24, 0.40, 0.88, 1.0), None, None),
    "dark_blue": ((0.14, 0.24, 0.62, 1.0), None, None),
    "glow_blue": ((0.40, 0.70, 1.0, 1.0), 1.2, None),
    "cream": ((1.0, 0.90, 0.70, 1.0), 1.3, None),
    "crystal": ((0.55, 0.82, 1.0, 1.0), 2.5, None),
    "glass": ((0.55, 0.78, 1.0, 0.35), None, 0.35),
    "hidden": ((0.0, 0.0, 0.0, 0.0), None, 0.0),
}
_materials = {}
parts = []


def material(key):
    if key in _materials:
        return _materials[key]
    color, glow, alpha = PALETTE[key]
    mat = bpy.data.materials.new(key)
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = 0.15 if alpha is not None else 0.85
    if glow:
        bsdf.inputs["Emission Color"].default_value = color
        bsdf.inputs["Emission Strength"].default_value = glow
    if alpha is not None:
        bsdf.inputs["Alpha"].default_value = alpha
        if hasattr(mat, "surface_render_method"):
            mat.surface_render_method = "BLENDED"
        if hasattr(mat, "blend_method"):
            mat.blend_method = "BLEND"
        mat.use_backface_culling = False
    _materials[key] = mat
    return mat


def finish(obj, key, bevel=0.0, segments=2):
    obj.data.materials.append(material(key))
    for poly in obj.data.polygons:
        poly.use_smooth = False
    if bevel > 0.0:
        mod = obj.modifiers.new("bevel", "BEVEL")
        mod.width = bevel
        mod.segments = segments
        mod.limit_method = "ANGLE"
        mod.angle_limit = math.radians(60)
    parts.append(obj)
    return obj


def box(name, key, size, at, bevel=0.0):
    """Cube of `size` (x, y, z); `at` is the centre of its bottom face."""
    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(at[0], at[1], at[2] + size[2] * 0.5))
    obj = bpy.context.active_object
    obj.name = name
    obj.scale = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    return finish(obj, key, bevel)


def slab(name, key, x0, x1, y0, y1, z0, z1, bevel=0.0):
    """Axis-aligned block by its extents — walls, floors, bands."""
    return box(name, key, (x1 - x0, y1 - y0, z1 - z0), ((x0 + x1) * 0.5, (y0 + y1) * 0.5, z0), bevel)


def frustum(name, key, bottom, top, height, at, bevel=0.0):
    obj = box(name, key, (bottom[0], bottom[1], height), at, bevel)
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    zmax = max(v.co.z for v in bm.verts)
    for v in bm.verts:
        if abs(v.co.z - zmax) < 1e-4:
            v.co.x *= top[0] / bottom[0]
            v.co.y *= top[1] / bottom[1]
    bm.to_mesh(obj.data)
    bm.free()
    return obj


def cylinder(name, key, radius, depth, at, verts=24, axis="Z"):
    rot = (0.0, 0.0, 0.0) if axis == "Z" else (math.radians(90), 0.0, 0.0)
    loc = (at[0], at[1], at[2] + depth * 0.5) if axis == "Z" else at
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts, radius=radius, depth=depth, location=loc, rotation=rot)
    obj = bpy.context.active_object
    obj.name = name
    return finish(obj, key)


def arch_profile(half_width, straight, apex, steps=4):
    """Pointed arch outline in XZ, counter-clockwise, bottom edge open (used as a wall)."""
    pts = [(-half_width, 0.0), (half_width, 0.0), (half_width, straight)]
    for i in range(1, steps):
        t = i / steps
        pts.append((half_width * (1.0 - t), straight + (apex - straight) * math.sin(t * math.pi / 2.0)))
    pts.append((0.0, apex))
    for i in range(steps - 1, 0, -1):
        t = i / steps
        pts.append((-half_width * (1.0 - t), straight + (apex - straight) * math.sin(t * math.pi / 2.0)))
    pts.append((-half_width, straight))
    return pts


def _link(name, bm, at):
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = at
    return obj


def arch_ring(name, key, outer, inner, depth, at):
    """Wall around a pointed-arch opening: quads lofted between the outer and inner
    outlines (same point count), extruded `depth` along Y. `outer`/`inner` =
    (half_width, straight, apex); `at` = (x, y_front, z_bottom)."""
    po = arch_profile(*outer)
    pi = arch_profile(*inner)
    bm = bmesh.new()
    vo = [bm.verts.new((x, 0.0, z)) for x, z in po]
    vi = [bm.verts.new((x, 0.0, z)) for x, z in pi]
    faces = []
    n = len(po)
    for i in range(1, n):  # skip the open bottom edge (index 0 → 1)
        j = (i + 1) % n
        if j == 0:
            j = n - 1
            faces.append(bm.faces.new((vo[i], vo[0], vi[0], vi[i])))
            break
        faces.append(bm.faces.new((vo[i], vo[j], vi[j], vi[i])))
    res = bmesh.ops.extrude_face_region(bm, geom=faces)
    moved = [g for g in res["geom"] if isinstance(g, bmesh.types.BMVert)]
    bmesh.ops.translate(bm, verts=moved, vec=Vector((0.0, depth, 0.0)))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return finish(_link(name, bm, at), key)


def diamond(name, key, radius, height, at):
    bm = bmesh.new()
    ring = [bm.verts.new((radius * math.cos(a), radius * math.sin(a), 0.0)) for a in (0.0, math.pi / 2, math.pi, 3 * math.pi / 2)]
    top = bm.verts.new((0.0, 0.0, height * 0.55))
    bottom = bm.verts.new((0.0, 0.0, -height * 0.45))
    for i in range(4):
        bm.faces.new((ring[i], ring[(i + 1) % 4], top))
        bm.faces.new((ring[(i + 1) % 4], ring[i], bottom))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    obj = _link(name, bm, at)
    obj.rotation_euler = (0.0, 0.0, math.radians(45))
    return finish(obj, key)


def torus(name, key, major, minor, at, tilt=0.0):
    bpy.ops.mesh.primitive_torus_add(major_radius=major, minor_radius=minor, major_segments=28, minor_segments=8, location=at, rotation=(math.radians(tilt), 0.0, 0.0))
    obj = bpy.context.active_object
    obj.name = name
    return finish(obj, key)


def wedge(name, key, x0, x1, y_low, y_high, z_low, z_high):
    """Ramp: a box whose top slopes from z_low at y_low to z_high at y_high (front is −Y)."""
    obj = slab(name, key, x0, x1, min(y_low, y_high), max(y_low, y_high), min(z_low, 0.0) - 0.5, z_high)
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    ymin = min(v.co.y for v in bm.verts)
    ymax = max(v.co.y for v in bm.verts)
    zmax = max(v.co.z for v in bm.verts)
    low_y = ymin if y_low < y_high else ymax
    for v in bm.verts:
        if abs(v.co.z - zmax) < 1e-4 and abs(v.co.y - low_y) < 1e-4:
            v.co.z = z_low
    bm.to_mesh(obj.data)
    bm.free()
    return obj


def glass_band(prefix, x0, x1, y, z0, z1, mullion=1.6, post=0.22, thick=0.08):
    """Glass pane with white posts every `mullion` metres, in the XZ plane at depth `y`."""
    slab(f"{prefix}_glass", "glass", x0, x1, y - thick * 0.5, y + thick * 0.5, z0, z1)
    x = x0
    while x <= x1 + 1e-3:
        slab(f"{prefix}_post_{x:.1f}", "white", x - post * 0.5, x + post * 0.5, y - 0.18, y + 0.18, z0 - 0.05, z1 + 0.05)
        x += mullion


def superellipse(a, b, n=3.0, count=28, centre=(0.0, 0.0)):
    """Rounded-rectangle outline (superellipse) in XY, counter-clockwise, `count` points."""
    pts = []
    for i in range(count):
        t = 2.0 * math.pi * i / count
        c, s = math.cos(t), math.sin(t)
        x = a * math.copysign(abs(c) ** (2.0 / n), c)
        y = b * math.copysign(abs(s) ** (2.0 / n), s)
        pts.append((centre[0] + x, centre[1] + y))
    return pts


def loft(name, outline, rings, bands, skip=None, inside=False, cap="top"):
    """Faceted hull: `rings` = [(z, scale)], `bands` = material key per ring pair,
    `skip(x, y, z0, z1)` drops wall faces (openings), `inside` flips normals for an
    inner shell, `cap` closes the last ring with a fan. One object, one material
    slot per band, flat shaded — the look of the owner's reference."""
    bm = bmesh.new()
    levels = []
    for z, s in rings:
        levels.append([bm.verts.new((x * s, y * s, z)) for x, y in outline])
    keys = []
    for key in bands:
        if key not in keys:
            keys.append(key)
    n = len(outline)
    for li in range(len(rings) - 1):
        lo, hi = levels[li], levels[li + 1]
        z0, z1 = rings[li][0], rings[li + 1][0]
        for i in range(n):
            j = (i + 1) % n
            mx = (outline[i][0] + outline[j][0]) * 0.5
            my = (outline[i][1] + outline[j][1]) * 0.5
            if skip is not None and skip(mx, my, z0, z1):
                continue
            quad = (lo[i], lo[j], hi[j], hi[i]) if not inside else (lo[j], lo[i], hi[i], hi[j])
            face = bm.faces.new(quad)
            face.material_index = keys.index(bands[li])
    if cap == "top":
        top = levels[-1]
        zc = rings[-1][0]
        centre = bm.verts.new((sum(v.co.x for v in top) / n, sum(v.co.y for v in top) / n, zc))
        for i in range(n):
            j = (i + 1) % n
            tri = (top[i], top[j], centre) if not inside else (top[j], top[i], centre)
            face = bm.faces.new(tri)
            face.material_index = keys.index(bands[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    if inside:
        bmesh.ops.reverse_faces(bm, faces=bm.faces)
    obj = _link(name, bm, (0.0, 0.0, 0.0))
    for key in keys:
        obj.data.materials.append(material(key))
    for poly in obj.data.polygons:
        poly.use_smooth = False
    parts.append(obj)
    return obj


def posts_along(prefix, outline, z0, z1, every=2, width=0.22, depth=0.3, key="white", skip=None):
    """White mullions standing on the outline every `every` vertices, turned along the tangent."""
    n = len(outline)
    for i in range(0, n, every):
        x, y = outline[i]
        if skip is not None and skip(x, y, z0, z1):
            continue
        j, k = (i - 1) % n, (i + 1) % n
        angle = math.atan2(outline[k][1] - outline[j][1], outline[k][0] - outline[j][0])
        obj = box(f"{prefix}_{i}", key, (width, depth, z1 - z0 + 0.1), (0.0, 0.0, z0 - 0.05))
        obj.location = (x, y, z0 - 0.05 + (z1 - z0 + 0.1) * 0.5)
        obj.rotation_euler = (0.0, 0.0, angle)


def outline_with_cuts(outline, cuts, front_y):
    """Insert points where the outline's front side (y < front_y) crosses x = each cut,
    so a wall gap can end exactly on a neighbouring wall instead of mid-face."""
    pts = list(outline)
    for cut in cuts:
        out = []
        n = len(pts)
        for i in range(n):
            x0, y0 = pts[i]
            x1, y1 = pts[(i + 1) % n]
            out.append((x0, y0))
            if (y0 + y1) * 0.5 < front_y and (x0 - cut) * (x1 - cut) < 0.0:
                t = (cut - x0) / (x1 - x0)
                out.append((cut, y0 + (y1 - y0) * t))
        pts = out
    return pts
