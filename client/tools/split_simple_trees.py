"""Нарізає куплений пак Simple Trees на моделі для гри (docs/ASSETS_NATURE.md):

    blender -b --python client/tools/split_simple_trees.py

Сирий пак лежить у `client/assets/Simple Trees/` (поза git і поза імпортом Godot,
там `.gdignore`): FBX з Unreal із ланцюжками LOD, колізійними оболонками `UCX_` і
масштабом 0.01 на кореневому вузлі, текстури 4K TGA. Для кожної моделі з
`Meshes_LODs` скрипт:
  - запікає масштаб кореня в меш (метри), прибирає `UCX_` і зайві UV-шари;
  - лишає три рівні: `lod0_near` (LOD1 паку), `lod1_mid` (LOD2, або LOD3, якщо
    LOD2 важчий за MID_MAX_TRIS) і `lod2_far` (останній LOD);
  - перейменовує матеріали на `bark` / `leaves` / `far` без текстур — справжні
    матеріали дає `tools/post_import_simple_trees.gd` (одна текстура на весь пак);
  - будує `shadow_crown`: гладку крону з метаболів навколо листкових карток
    середнього рівня, спрощену до CROWN_FACES граней. Листя в грі не кидає тінь від
    сонця (дірки в картках, що гойдаються, мерехтіли на карті тіней), тінь дає ця
    крона — суцільна, мʼяка, гойдається разом із деревом;
  - пише `client/assets/simple_trees/pieces/<name>.glb` (Y-up, основа в нулі).
Текстури: `leaves_albedo.png` 1024² з прозорістю і `bark_albedo.png` 512×1024
(правило web: ≤ 1024). Решта карт паку (normal, AO, translucency) у грі не потрібні.
"""

import json
import re
from pathlib import Path

import bpy

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "assets" / "Simple Trees"
OUT = ROOT / "assets" / "simple_trees"
MID_MAX_TRIS = 6000
## Грані гладкої крони-тіні та найбільша кількість метаболів на модель.
CROWN_FACES = 320
CROWN_BALLS = 900
LOD_RE = re.compile(r"_LOD(\d+)$")


def reset() -> None:
    bpy.ops.wm.read_factory_settings(use_empty=True)


def tris(obj: bpy.types.Object) -> int:
    return sum(len(p.vertices) - 2 for p in obj.data.polygons)


def role(material_name: str) -> str:
    name = material_name.lower()
    if "bark" in name:
        return "bark"
    if "leaves" in name:
        return "leaves"
    return "far"


def shared_materials() -> dict:
    out = {}
    for key in ("bark", "leaves", "far"):
        out[key] = bpy.data.materials.new(key)
    return out


def pick_lods(lods: list) -> dict:
    """lods: objects sorted by LOD index. Returns {near, mid, far} objects."""
    near = lods[1] if len(lods) > 1 else lods[0]
    mid = lods[2] if len(lods) > 2 else lods[-1]
    if tris(mid) > MID_MAX_TRIS and len(lods) > 3:
        mid = lods[3]
    return {"lod0_near": near, "lod1_mid": mid, "lod2_far": lods[-1]}


def prepare(obj: bpy.types.Object, new_name: str, materials: dict) -> None:
    mw = obj.matrix_world.copy()
    obj.parent = None
    obj.matrix_world = mw
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    while len(obj.data.uv_layers) > 1:
        obj.data.uv_layers.remove(obj.data.uv_layers[-1])
    for attr in list(obj.data.color_attributes):
        obj.data.color_attributes.remove(attr)
    for slot in obj.material_slots:
        slot.material = materials[role(slot.material.name if slot.material else "")]
    obj.name = new_name
    obj.data.name = new_name


def crown_proxy(src: bpy.types.Object, materials: dict):
    """Smooth, closed crown for shadows: metaballs on the leaf cards of `src` (already in
    metres, materials renamed), polygonised, decimated. None when `src` has no leaves."""
    leaf_slots = {i for i, slot in enumerate(src.material_slots) if slot.material and slot.material.name in ("leaves", "far")}
    polys = [p for p in src.data.polygons if p.material_index in leaf_slots]
    if not polys:
        return None
    step = max(1, len(polys) // CROWN_BALLS)
    ball = bpy.data.metaballs.new("crown_ball")
    ball.resolution = 0.2
    ball.render_resolution = 0.2
    ball.threshold = 0.6
    holder = bpy.data.objects.new("crown_ball", ball)
    bpy.context.scene.collection.objects.link(holder)
    for poly in polys[::step]:
        element = ball.elements.new()
        element.co = poly.center
        element.radius = min(max(poly.area ** 0.5 * 1.15, 0.45), 1.5)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    mesh = bpy.data.meshes.new_from_object(holder.evaluated_get(depsgraph))
    bpy.data.objects.remove(holder, do_unlink=True)
    crown = bpy.data.objects.new("shadow_crown", mesh)
    bpy.context.scene.collection.objects.link(crown)
    faces = len(mesh.polygons)
    if faces > CROWN_FACES:
        mod = crown.modifiers.new("decimate", "DECIMATE")
        mod.ratio = CROWN_FACES / faces
        depsgraph = bpy.context.evaluated_depsgraph_get()
        reduced = bpy.data.meshes.new_from_object(crown.evaluated_get(depsgraph))
        crown.modifiers.clear()
        crown.data = reduced
        bpy.data.meshes.remove(mesh)
    crown.data.name = "shadow_crown"
    crown.data.materials.clear()
    crown.data.materials.append(materials["bark"])
    for poly in crown.data.polygons:
        poly.use_smooth = True
    return crown


def split_model(path: Path) -> dict:
    reset()
    materials = shared_materials()
    bpy.ops.import_scene.fbx(filepath=str(path))
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and LOD_RE.search(o.name)]
    lods = sorted(meshes, key=lambda o: int(LOD_RE.search(o.name).group(1)))
    chosen = pick_lods(lods)
    report = {}
    # Transforms first (they need the parent chain), then drop everything else.
    for new_name, obj in chosen.items():
        report[new_name] = {"source": obj.name, "tris": tris(obj)}
        prepare(obj, new_name, materials)
    keep = set(chosen.values())
    for o in list(bpy.data.objects):
        if o not in keep:
            bpy.data.objects.remove(o, do_unlink=True)
    crown = crown_proxy(chosen["lod1_mid"], materials)
    if crown is not None:
        keep.add(crown)
        report["shadow_crown"] = {"source": "metaballs", "tris": tris(crown)}
    bpy.ops.object.select_all(action="SELECT")
    name = path.stem.lower().replace("sm_plant_", "")
    target = OUT / "pieces" / f"{name}.glb"
    bpy.ops.export_scene.gltf(filepath=str(target), export_format="GLB", use_selection=True, export_yup=True,
                              export_apply=True, export_materials="EXPORT", export_animations=False)
    lo = [min(v.co[i] for o in keep for v in o.data.vertices) for i in range(3)]
    hi = [max(v.co[i] for o in keep for v in o.data.vertices) for i in range(3)]
    report["size_m"] = [round(hi[0] - lo[0], 2), round(hi[1] - lo[1], 2), round(hi[2] - lo[2], 2)]
    report["base_z"] = round(lo[2], 2)
    return {name: report}


def textures() -> None:
    (OUT / "textures").mkdir(parents=True, exist_ok=True)
    for src, dst, size in (("T_Plant_Tree_Simple_Leaves_BCO.tga", "leaves_albedo.png", (1024, 1024)),
                           ("T_Plant_Tree_Simple_Bark_BC.tga", "bark_albedo.png", (512, 1024))):
        img = bpy.data.images.load(str(SRC / "Textures" / src))
        img.scale(*size)
        img.filepath_raw = str(OUT / "textures" / dst)
        img.file_format = "PNG"
        img.save()
        print(f"texture {dst} {size} planes={img.depth}")


def main() -> None:
    (OUT / "pieces").mkdir(parents=True, exist_ok=True)
    report = {}
    for path in sorted((SRC / "Meshes_LODs").glob("*.fbx")):
        report.update(split_model(path))
    reset()
    textures()
    (OUT / "pieces" / "pieces.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    for name, info in report.items():
        lods = " ".join(f"{k}={v['tris']}" for k, v in info.items() if k.startswith("lod"))
        print(f"  {name:24s} {info['size_m']} base {info['base_z']}  {lods}")
    print(f"split_simple_trees: {len(report)} models -> {OUT}")


main()
