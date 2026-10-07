// Fog of war over the mainland map (docs/MAINLAND.md §4): chunks the character
// has walked through (plus their neighbours) are revealed. Kept in the shell for
// now; the server-side `explored` mask arrives with the M3 migration.

export type Explored = Record<string, true>;

export function chunkKey(cx: number, cz: number): string {
  return `${cx}:${cz}`;
}

/** Reveals the chunk and everything within `radius` chunks of it (clamped to the map). */
export function revealAround(explored: Explored, cx: number, cz: number, radius: number, size: number): Explored {
  const next: Explored = { ...explored };
  for (let dz = -radius; dz <= radius; dz++) {
    for (let dx = -radius; dx <= radius; dx++) {
      const x = cx + dx;
      const z = cz + dz;
      if (x < 0 || z < 0 || x >= size || z >= size) continue;
      next[chunkKey(x, z)] = true;
    }
  }
  return next;
}

/** The server's 32-byte mask (hex) as the map's explored set. */
export function exploredFromHex(hex: string, size: number): Explored {
  const out: Explored = {};
  for (let cz = 0; cz < size; cz++) {
    for (let cx = 0; cx < size; cx++) {
      const bit = cz * size + cx;
      const byte = parseInt(hex.slice((bit >> 3) * 2, (bit >> 3) * 2 + 2), 16);
      if (!Number.isNaN(byte) && (byte & (1 << (bit & 7))) !== 0) out[chunkKey(cx, cz)] = true;
    }
  }
  return out;
}

export function exploredCount(explored: Explored): number {
  return Object.keys(explored).length;
}

/** Zone letters from shared/mainland_zones.json. */
export const BIOME_LABEL: Record<string, string> = {
  C: "city",
  B: "beach and pier",
  M: "meadows",
  F: "forest",
  D: "dark forest",
  R: "mountains",
  S: "swamp",
  W: "water",
};

export const BIOME_COLOR: Record<string, string> = {
  C: "#c9b585",
  B: "#e5d7a3",
  M: "#8cc36a",
  F: "#4f8a45",
  D: "#2f5d33",
  R: "#9a948c",
  S: "#6f8a5a",
  W: "#5f9fd6",
};
