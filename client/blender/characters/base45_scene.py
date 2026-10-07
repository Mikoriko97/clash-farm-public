"""Сцена бази персонажа 4,5 голови в Blender (ADR-032): арматура `Base45`, тіло `Base45_Body` (ваги, підошва, зони палітри,
матеріал-палітра для прев'ю) і наліпка обличчя `Base45_Face`. Спільна для генераторів бази, частин голови, тіл і вбрань:
`ensure()` будує сцену, якщо її ще нема, і повертає (арматура, тіло, обличчя)."""

import bpy

import base45_face
import common
import parts_base45

ATLAS_PNG = "S:/clashofstaking/client/blender/characters/face_atlas.png"  # у GLB вбудовується (Godot: base45_face_atlas.png)
#: Кольори вигляду за замовчуванням (`CharacterLook.DEFAULTS`) — лише для прев'ю в Blender; у грі палітру будує Godot.
PREVIEW = {"skin": "#d9a878", "shirt": "#5f8f4a", "pants": "#6b5a3e", "shoes": "#3a2f28"}
CELL_SLOTS = {14: "skin", 1: "skin", 0: "skin", 2: "skin", 3: "skin", 13: "shirt", 12: "shirt",
              11: "pants", 10: "pants", 9: "pants", 8: "pants", 7: "shoes", 6: "shoes", 5: "shoes", 4: "shoes"}


def palette_material():
    """16 × 1 текстура з кольорами клітинок (як будує `CharacterLook`), найближча вибірка."""
    name = "base45_palette"
    img = bpy.data.images.get(name) or bpy.data.images.new(name, 16, 1, alpha=False)
    px = []
    for cell in range(16):
        h = PREVIEW[CELL_SLOTS.get(cell, "skin")].lstrip("#")
        px += [int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4)] + [1.0]  # пікселі зображення вже в sRGB
    img.pixels = px
    mat = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    tex = nodes.get("palette") or nodes.new("ShaderNodeTexImage")
    tex.name = "palette"
    tex.image = img
    tex.interpolation = "Closest"
    mat.node_tree.links.new(tex.outputs["Color"], nodes["Principled BSDF"].inputs["Base Color"])
    nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.9
    return mat


def attach_to_head(obj, arm):
    """Наліпка обличчя повністю на кістці `Head` (скінна, як тіло)."""
    obj.parent = arm
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    group = obj.vertex_groups.new(name="Head")
    group.add([v.index for v in obj.data.vertices], 1.0, "REPLACE")


def build():
    common.clear_scene()
    arm = parts_base45.make_armature()
    body = parts_base45.make_body()
    parts_base45.bind(body, arm)
    parts_base45.flatten_sole(body)
    parts_base45.paint_zones(body)
    body.data.materials.append(palette_material())
    face = base45_face.make_decal(body, base45_face.face_material(base45_face.atlas_image(ATLAS_PNG)))
    attach_to_head(face, arm)
    return arm, body, face


def ensure():
    objs = [bpy.data.objects.get(n) for n in ("Base45", "Base45_Body", "Base45_Face")]
    return tuple(objs) if all(objs) else build()
