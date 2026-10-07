"""Інструменти в руці бази 4,5 голови (ADR-034): моделі `client/assets/tools/*.fbx` (держак уздовж локальної +Z, голова вгорі) і
згенерована лійка стають частиною персонажа — меш із вагою 1 на кістці `RightHand`. Хват задано тут, у спокійній позі: держак
проходить крізь кулак правої руки (вісь отвору кулака — вздовж великого пальця, у Т-позі це «вперед», −Y), удар інструмента
дивиться в бік пальців. Тож у будь-якій анімації інструмент сидить у кулаці так, як задумано (а не «як вийшло» з кріплення до
зап'ястя), і Godot нічого не підбирає: `HandTools` лише показує потрібний меш.

`Tool` описує модель у її власних координатах: точку хвату правої руки й лівої (на держаку), вісь удару (лезо, бойок, полотно).
`frame(P, D, U)` — світова матриця інструмента, коли кулак правої руки в точці P, держак дивиться в D (до голови), а удар — в U."""

import math

import bmesh
import bpy
from mathutils import Matrix, Vector

from common import ellipsoid, hex_rgb, material
from parts_base45 import bone_table

TOOLS_DIR = "S:/clashofstaking/client/assets/tools/"


class Tool:
    """`grip_r`/`grip_l` — висоти хвату на держаку (локальна z), `strike` — локальна вісь удару, `scale` — масштаб моделі."""

    def __init__(self, name, source, grip_r, grip_l=None, strike=(1.0, 0.0, 0.0), scale=1.0):
        self.name, self.source = name, source
        self.grip_r, self.grip_l = grip_r, grip_l
        self.strike = Vector(strike)
        self.scale = scale

    def local_basis(self):
        z = Vector((0.0, 0.0, 1.0))
        s = self.strike.normalized()
        return Matrix((s, z.cross(s), z)).transposed()  # стовпці: удар, z×удар, держак

    def frame(self, P, D, U):
        """Матриця, що ставить точку хвату правої руки в P, держак (+z) у D, удар у U (U ортогоналізується до D)."""
        d = Vector(D).normalized()
        u = (Vector(U) - d * Vector(U).dot(d)).normalized()
        world = Matrix((u, d.cross(u), d)).transposed()
        rot = (world @ self.local_basis().transposed()).to_4x4()
        grip = rot @ Vector((0.0, 0.0, self.grip_r * self.scale))
        return Matrix.Translation(Vector(P) - grip) @ rot @ Matrix.Diagonal((self.scale,) * 3 + (1.0,))


#: Хвати двома руками близько один до одного: руки бази короткі (≈0,45 до кінчиків пальців), ширший хват не досягти. Задня рука —
#: біля кінця держака: інакше вільний кінець за кулаком (у кайла й коси ~0,3) у горизонтальній фазі замаху входить у живіт.
TOOLS = {
    "axe": Tool("axe", "AxeV1", grip_r=0.02, grip_l=-0.12),
    "pickaxe": Tool("pickaxe", "PickAxeV1", grip_r=-0.3, grip_l=-0.46),
    "hoe": Tool("hoe", "HoeV1", grip_r=-0.05, grip_l=-0.22, strike=(0.0, -1.0, 0.0)),
    "scythe": Tool("scythe", "ScytheV1", grip_r=0.07, grip_l=-0.13),
    "spade": Tool("spade", "SpadeV1", grip_r=-0.37, grip_l=-0.25, strike=(0.0, 1.0, 0.0)),
    "hammer": Tool("hammer", "HammerV1", grip_r=-0.17),
    "can": Tool("can", None, grip_r=0.0),
}

#: Отвір кулака правої руки в Т-позі: трохи нижче долоні, посередині пальців; вісь — уздовж великого пальця (−Y).
FIST_R = Vector((-0.668, -0.004, 1.236))
THUMB_R = Vector((0.0, -1.0, 0.0))
FINGERS_R = Vector((-1.0, 0.0, 0.0))


def rest_frame(tool):
    """Інструмент у спокійній позі: держак крізь кулак уздовж великого пальця, удар — у бік пальців."""
    return tool.frame(FIST_R, THUMB_R, FINGERS_R)


def import_tool(tool, mats_cache):
    """Модель → один меш `tool_<name>` у власних координатах (держак +z, хват у z = grip), трансформації застосовані."""
    for o in list(bpy.data.objects):
        if o.name == f"tool_{tool.name}":
            bpy.data.objects.remove(o, do_unlink=True)
    if tool.source is None:
        return make_can(mats_cache)
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=TOOLS_DIR + tool.source + ".fbx")
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    for o in new:
        if o.type != "MESH":
            bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.name = obj.data.name = f"tool_{tool.name}"
    return obj


def make_can(mats_cache):
    """Лійка: корпус, носик уперед-угору, дужка зверху (у кулаці), розетка. Дужка — уздовж z через початок координат,
    корпус висить у бік пальців (+x)."""
    green = mats_cache.setdefault("can_green", material("can_green", hex_rgb("#4f7f3f"), rough=0.6))
    dark = mats_cache.setdefault("can_dark", material("can_dark", hex_rgb("#2f4f2a"), rough=0.6))
    bm = bmesh.new()
    res = bmesh.ops.create_cone(bm, cap_ends=True, segments=18, radius1=0.09, radius2=0.08, depth=0.16)
    rot = Matrix.Rotation(math.radians(90.0), 4, "Y")
    bmesh.ops.transform(bm, matrix=Matrix.Translation((0.14, 0.0, 0.0)) @ rot, verts=res["verts"])
    # носик: від боку корпусу вперед (+z, бік великого пальця) і вгору (−x, коли рука опущена й пальці дивляться вниз)
    spout = bmesh.ops.create_cone(bm, cap_ends=True, segments=10, radius1=0.022, radius2=0.012, depth=0.24)
    tilt = Matrix.Rotation(math.radians(-40.0), 4, "Y")
    bmesh.ops.transform(bm, matrix=Matrix.Translation((0.085, 0.0, 0.175)) @ tilt, verts=spout["verts"])
    ellipsoid(bm, (0.005, 0.0, 0.27), (0.022, 0.03, 0.03), u=10, v=6)  # розетка
    body_faces = len(bm.faces)
    bmesh.ops.create_cone(bm, cap_ends=True, segments=8, radius1=0.012, radius2=0.012, depth=0.11)  # дужка крізь кулак (вздовж z)
    for side in (-0.05, 0.05):
        arm = bmesh.ops.create_cone(bm, cap_ends=True, segments=8, radius1=0.01, radius2=0.01, depth=0.07)
        bmesh.ops.transform(bm, matrix=Matrix.Translation((0.035, 0.0, side)) @ rot, verts=arm["verts"])
    mesh = bpy.data.meshes.new("tool_can")
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(green)
    mesh.materials.append(dark)
    for i, poly in enumerate(mesh.polygons):
        poly.material_index = 0 if i < body_faces else 1
        poly.use_smooth = True
    obj = bpy.data.objects.new("tool_can", mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def place_rest(obj, tool, arm):
    """Ставить меш інструмента в спокійну позу (кулак правої руки) і вішає на кістку `RightHand` з вагою 1."""
    obj.data.transform(rest_frame(tool))
    obj.matrix_world = Matrix.Identity(4)
    obj.parent = arm
    obj.vertex_groups.clear()
    group = obj.vertex_groups.new(name="RightHand")
    group.add([v.index for v in obj.data.vertices], 1.0, "REPLACE")
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    return obj


def hand_tail_rest(side="Right"):
    """Кінець кістки кисті в спокої (мета IK): положення й орієнтація кістки, перенесені на її хвіст."""
    h, t, _ = bone_table()[side + "Hand"]
    return Vector(h), Vector(t)
