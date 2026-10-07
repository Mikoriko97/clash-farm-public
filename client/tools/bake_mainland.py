"""Пече карту висот материка з client/blender/mainland.blend (ADR-021).

    blender -b client/blender/mainland.blend --python client/tools/bake_mainland.py

Обʼєкт `Terrain` (сітка 1024 × 1024 м, Blender Z-up, Y = -Z Godot) семплюється
променями зверху на сітці 1 м → client/blender/height_1024.f32 (float32 LE,
рядок = z Godot від -512 до 511). Далі `godot --headless --script
res://tools/import_height.gd` кладе його в assets/mainland/height.res.
Біоми (biome.res) цей крок не чіпає.
"""

import struct
import sys
import time
from pathlib import Path

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

SIZE = 1024
HALF = 512.0
OUT = Path(bpy.path.abspath("//")) / "height_1024.f32"
SEA = -8.0


def main() -> int:
    terrain = bpy.data.objects.get("Terrain")
    if terrain is None:
        print("bake_mainland: no object named Terrain")
        return 1
    depsgraph = bpy.context.evaluated_depsgraph_get()
    tree = BVHTree.FromObject(terrain, depsgraph)
    inv = terrain.matrix_world.inverted()
    up = Vector((0.0, 0.0, -1.0))
    started = time.time()
    rows = []
    misses = 0
    for pz in range(SIZE):
        gz = pz - HALF
        row = [0.0] * SIZE
        for px in range(SIZE):
            gx = px - HALF
            # The mesh spans x -512..511 and y -511..512: pixels on its edges are nudged inside.
            bx = min(max(gx, -HALF + 0.001), HALF - 1.001)
            by = min(max(-gz, -HALF + 1.001), HALF - 0.001)
            origin = inv @ Vector((bx, by, 500.0))
            hit = tree.ray_cast(origin, up, 2000.0)
            if hit[0] is None:
                misses += 1
                row[px] = SEA
            else:
                row[px] = (terrain.matrix_world @ hit[0]).z
        rows.append(struct.pack("<%df" % SIZE, *row))
    OUT.write_bytes(b"".join(rows))
    print("bake_mainland: wrote %s in %.1fs, misses=%d" % (OUT, time.time() - started, misses))
    return 0


if __name__ == "__main__":
    sys.exit(main())
