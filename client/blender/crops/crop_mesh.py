"""Геометрія культур для генератора `make_crops.py`: картки з альфа-атласом (листя,
квіти, колоски) і низькополігональні тіла (плоди, стебла) з UV у палітру атласу.

Два матеріали на меш: 0 — картки (`crop_cards`, двобічні, альфа-відсік, вітер у Godot),
1 — тіла (`crop_solid`, непрозорі). Z угору в Blender; основа пучка в нулі.
"""

import math

import bpy
from mathutils import Matrix, Vector

from crop_atlas import palette_uv, tile_rect

CARDS, SOLID = 0, 1


class Builder:
    def __init__(self):
        self.verts, self.faces, self.uvs, self.mats = [], [], [], []

    # ------------------------------------------------------------ примітиви
    def _tri(self, a, b, c, uv, mat):
        base = len(self.verts)
        self.verts.extend([a, b, c])
        self.faces.append((base, base + 1, base + 2))
        self.uvs.append(uv)
        self.mats.append(mat)

    def quad(self, corners, rect, mat=CARDS):
        u0, v0, u1, v1 = rect
        a, b, c, d = corners
        self._tri(a, b, c, [(u0, v0), (u1, v0), (u1, v1)], mat)
        self._tri(a, c, d, [(u0, v0), (u1, v1), (u0, v1)], mat)

    def card(self, origin, height, width, yaw, tilt, tile, bend=0.0, roll=0.0):
        """Картка з основою в `origin`: `tilt` нахиляє від вертикалі (рад), `bend` вигинає верх,
        `roll` крутить навколо власної осі (листок, що лежить боком)."""
        rot = Matrix.Rotation(yaw, 3, "Z") @ Matrix.Rotation(tilt, 3, "X") @ Matrix.Rotation(roll, 3, "Z")
        pts = []
        for dx, dz in ((-width / 2, 0.0), (width / 2, 0.0), (width / 2, height), (-width / 2, height)):
            y = bend * (dz / height) ** 2 * height if height else 0.0
            pts.append(Vector(origin) + rot @ Vector((dx, y, dz)))
        self.quad(pts, tile_rect(tile))

    def cross(self, origin, height, width, yaw, tile, tilt=0.0, bend=0.0):
        """Дві картки хрестом: обʼємний кущик чи квітка з будь-якого боку."""
        for k in (0.0, math.pi / 2):
            self.card(origin, height, width, yaw + k, tilt, tile, bend)

    def flat(self, centre, size, yaw, tile, lift=0.01, tilt=0.0):
        """Картка, що лежить на землі (листок гарбуза, розетка полуниці), трохи піднята."""
        rot = Matrix.Rotation(yaw, 3, "Z") @ Matrix.Rotation(tilt, 3, "X")
        pts = [Vector(centre) + Vector((0, 0, lift)) + rot @ Vector(p) for p in
               ((-size / 2, -size * 0.1, 0.0), (size / 2, -size * 0.1, 0.0), (size / 2, size * 0.9, 0.0), (-size / 2, size * 0.9, 0.0))]
        self.quad(pts, tile_rect(tile))

    def stalk(self, origin, height, radius, colour, lean=(0.0, 0.0), sides=4, segments=2):
        """Призма-стебло з UV у колір палітри (темніше внизу); повертає вершину."""
        rings = []
        for s in range(segments + 1):
            t = s / segments
            sway = Vector((lean[0] * t * t, lean[1] * t * t, height * t))
            r = radius * (1.0 - 0.4 * t)
            rings.append([Vector(origin) + sway + Vector((math.cos(k * math.tau / sides) * r, math.sin(k * math.tau / sides) * r, 0))
                          for k in range(sides)])
        for s in range(segments):
            va, vb = s / segments, (s + 1) / segments
            for k in range(sides):
                a0, a1 = rings[s][k], rings[s][(k + 1) % sides]
                b0, b1 = rings[s + 1][k], rings[s + 1][(k + 1) % sides]
                self._tri(a0, a1, b1, [palette_uv(colour, va), palette_uv(colour, va), palette_uv(colour, vb)], SOLID)
                self._tri(a0, b1, b0, [palette_uv(colour, va), palette_uv(colour, vb), palette_uv(colour, vb)], SOLID)
        return sum(rings[-1], Vector()) / sides

    def blob(self, centre, radii, colour, segs=8, rings=5, ribs=0.0, rib_count=0, squash_top=0.0, stripes=None):
        """Низькополігональний еліпсоїд: плід, головка, бульба. `ribs` — глибина ребер
        (гарбуз), `squash_top` — вдавлена верхівка, `stripes` — (темний колір, кількість)."""
        cx, cy, cz = centre
        rx, ry, rz = radii
        grid = []
        for i in range(rings + 1):
            phi = math.pi * i / rings
            row = []
            for j in range(segs):
                th = math.tau * j / segs
                rib = 1.0 - ribs * (0.5 + 0.5 * math.cos(th * rib_count)) if rib_count else 1.0
                z = -math.cos(phi)
                if z > 0:
                    z *= 1.0 - squash_top
                row.append(Vector((cx + math.cos(th) * math.sin(phi) * rx * rib, cy + math.sin(th) * math.sin(phi) * ry * rib, cz + z * rz)))
            grid.append(row)
        for i in range(rings):
            v0, v1 = i / rings, (i + 1) / rings
            for j in range(segs):
                col = colour
                if stripes and (j * stripes[1] // segs) % 2 == 0:
                    col = stripes[0]
                a, b = grid[i][j], grid[i][(j + 1) % segs]
                c, d = grid[i + 1][(j + 1) % segs], grid[i + 1][j]
                if i > 0:
                    self._tri(a, b, c, [palette_uv(col, v0), palette_uv(col, v0), palette_uv(col, v1)], SOLID)
                if i < rings - 1:
                    self._tri(a, c, d, [palette_uv(col, v0), palette_uv(col, v1), palette_uv(col, v1)], SOLID)
        return Vector((cx, cy, cz + rz))

    def cone(self, base, height, radius, colour, segs=6, tip_down=False):
        """Конус: морква (вершиною вниз), ягода полуниці, перець."""
        b = Vector(base)
        tip = b + Vector((0, 0, -height if tip_down else height))
        ring = [b + Vector((math.cos(math.tau * k / segs) * radius, math.sin(math.tau * k / segs) * radius, 0)) for k in range(segs)]
        for k in range(segs):
            a, c = ring[k], ring[(k + 1) % segs]
            if tip_down:  # swap the winding so faces still point outwards
                a, c = c, a
            self._tri(a, c, tip, [palette_uv(colour, 0.9), palette_uv(colour, 0.9), palette_uv(colour, 0.2)], SOLID)
            self._tri(b, c, a, [palette_uv(colour, 0.95)] * 3, SOLID)

    def box(self, centre, size, colour, yaw=0.0):
        """Прямокутна коробка (пакетик насіння, скриня): 6 граней, UV у колір палітри."""
        cx, cy, cz = centre
        hx, hy, hz = size[0] / 2, size[1] / 2, size[2] / 2
        rot = Matrix.Rotation(yaw, 3, "Z")
        p = [Vector((cx, cy, cz)) + rot @ Vector((sx * hx, sy * hy, sz * hz))
             for sx, sy, sz in ((-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1))]
        for a, b, c, d, v in ((0, 1, 5, 4, 0.5), (1, 2, 6, 5, 0.6), (2, 3, 7, 6, 0.5), (3, 0, 4, 7, 0.6), (4, 5, 6, 7, 0.9), (3, 2, 1, 0, 0.2)):
            self._tri(p[a], p[b], p[c], [palette_uv(colour, v)] * 3, SOLID)
            self._tri(p[a], p[c], p[d], [palette_uv(colour, v)] * 3, SOLID)

    def log(self, centre, length, radius, colour, end, yaw=0.0, sides=7):
        """Колода вздовж осі Y (повернута на `yaw`): кора `colour`, торці `end` зі світлим центром."""
        rot = Matrix.Rotation(yaw, 3, "Z")
        c = Vector(centre)
        rings = [[c + rot @ Vector((math.cos(k * math.tau / sides) * radius, y, math.sin(k * math.tau / sides) * radius))
                  for k in range(sides)] for y in (-length / 2, length / 2)]
        for k in range(sides):
            a0, a1 = rings[0][k], rings[0][(k + 1) % sides]
            b0, b1 = rings[1][k], rings[1][(k + 1) % sides]
            v = 0.35 + 0.6 * (0.5 + 0.5 * math.sin((k + 0.5) * math.tau / sides))
            self._tri(a0, b0, b1, [palette_uv(colour, v)] * 3, SOLID)
            self._tri(a0, b1, a1, [palette_uv(colour, v)] * 3, SOLID)
        for s, ring in ((-1, rings[0]), (1, rings[1])):
            mid = c + rot @ Vector((0, s * length / 2, 0))
            for k in range(sides):
                a, b = ring[k], ring[(k + 1) % sides]
                if s < 0:
                    a, b = b, a
                self._tri(mid, b, a, [palette_uv(end, 0.95), palette_uv(end, 0.3), palette_uv(end, 0.3)], SOLID)

    # ------------------------------------------------------------ вихід
    def tris(self):
        return len(self.faces)

    def to_object(self, name, materials):
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([tuple(v) for v in self.verts], [], self.faces)
        uv = mesh.uv_layers.new(name="UVMap")
        for poly, tri in zip(mesh.polygons, self.uvs):
            for li, coord in zip(poly.loop_indices, tri):
                uv.data[li].uv = coord
        for m in materials:
            mesh.materials.append(m)
        for poly, mat in zip(mesh.polygons, self.mats):
            poly.material_index = mat
            poly.use_smooth = False
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        return obj
