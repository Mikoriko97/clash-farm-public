// Mainland map (docs/MAINLAND.md §3–4): the 16 × 16 chunk zones under a fog
// that lifts where the character has been, with the character's chunk marked.
import type { LandmarkView, MainlandMap } from "../api/mainland";
import { BIOME_COLOR, BIOME_LABEL, chunkKey, exploredCount, type Explored } from "../mainland/fog";
import { PanelFrame } from "./panels";

interface MapProps {
  map: MainlandMap | null;
  explored: Explored;
  here: { cx: number; cz: number } | null;
  zone: "island" | "mainland";
  landmarks?: LandmarkView[];
  onClose: () => void;
}

/** Chunk of a landmark in decimetres from the map centre (server exploration::chunk_of). */
export function landmarkChunk(l: LandmarkView, chunkM: number, originM: number): string {
  const cx = Math.floor((l.x_dm / 10 - originM) / chunkM);
  const cz = Math.floor((l.z_dm / 10 - originM) / chunkM);
  return chunkKey(cx, cz);
}

export function MapSheet({ map, explored, here, zone, landmarks = [], onClose }: MapProps) {
  const size = map?.chunks ?? 16;
  const marks = new Map<string, LandmarkView>();
  if (map) for (const l of landmarks) marks.set(landmarkChunk(l, map.chunk_m, map.origin_m), l);
  const land = map ? map.rows.join("").split("").filter((c) => c !== "W").length : 0;
  const seen = exploredCount(explored);
  return (
    <PanelFrame title="Mainland map" className="sheet map" onClose={onClose}>
      {!map && <p className="hint">Loading the map…</p>}
      {map && (
        <div className="map-grid" style={{ gridTemplateColumns: `repeat(${size}, 1fr)` }} role="img" aria-label="Mainland map with fog">
          {map.rows.map((row, cz) =>
            row.split("").map((letter, cx) => {
              const known = explored[chunkKey(cx, cz)] === true || letter === "W";
              const mine = here !== null && here.cx === cx && here.cz === cz && zone === "mainland";
              const mark = marks.get(chunkKey(cx, cz));
              return (
                <span
                  key={chunkKey(cx, cz)}
                  className={`cell${known ? "" : " fog"}${mine ? " me" : ""}`}
                  style={{ background: known ? BIOME_COLOR[letter] ?? "#888" : undefined }}
                  title={mark ? `${mark.name}${mark.discovered ? " ★" : ""}` : known ? `${BIOME_LABEL[letter] ?? letter} (${cx}, ${cz})` : "unexplored"}
                >
                  {mine ? "●" : mark ? (mark.discovered ? "★" : "📍") : ""}
                </span>
              );
            }),
          )}
        </div>
      )}
      <p className="hint">
        Explored {seen} of {land} land cells. The fog lifts where you have walked; {zone === "mainland" ? "the dot is you" : "take the boat to continue"}.
      </p>
      <p className="hint legend">
        {Object.entries(BIOME_LABEL).map(([k, label]) => (
          <span key={k}>
            <i style={{ background: BIOME_COLOR[k] }} /> {label}
          </span>
        ))}
      </p>
    </PanelFrame>
  );
}
