// Translates server snapshots into `world_state` bridge messages
// (docs/BRIDGE_PROTOCOL.md). Godot only ever sees public plot state.
import type { IslandSnapshot, IslandView, PlotView, ResourceView, WeatherView } from "../api/world";

export interface GodotPlot {
  idx: number;
  col: number;
  row: number;
  state: PlotView["state"];
  crop_id?: string;
  planted_at?: string;
  ready_at?: string;
}

export function toGodotPlot(plot: PlotView): GodotPlot {
  const out: GodotPlot = { idx: plot.idx, col: plot.col, row: plot.row, state: plot.state };
  if (plot.crop_id) out.crop_id = plot.crop_id;
  if (plot.planted_at) out.planted_at = plot.planted_at;
  if (plot.ready_at) out.ready_at = plot.ready_at;
  return out;
}

export function toGodotResource(node: ResourceView): Record<string, unknown> {
  const out: Record<string, unknown> = { idx: node.idx, kind: node.kind, hits_left: node.hits_left };
  if (node.respawn_at) out.respawn_at = node.respawn_at;
  return out;
}

export function fullWorldState(snapshot: IslandSnapshot): Record<string, unknown> {
  return {
    full: true,
    plots: snapshot.plots.map(toGodotPlot),
    decor: snapshot.decor.map(toGodotDecor),
    resources: (snapshot.resources ?? []).map(toGodotResource),
    cleared: snapshot.cleared ?? [],
    expand: toGodotExpand(snapshot.island),
    ...(snapshot.weather ? { weather: toGodotWeather(snapshot.weather) } : {}),
    level: snapshot.island.level,
    now: snapshot.now,
  };
}

export function plotPatch(plot: PlotView, now: string): Record<string, unknown> {
  return plotsPatch([plot], now);
}

/** The cleared-bush spots after a clearing; a patch, so nothing else in the world changes. */
export function clearedPatch(cleared: [number, number][], now: string): Record<string, unknown> {
  return { full: false, plots: [], decor: [], cleared, now };
}

/** Several plots at once (bed actions); a patch never removes plots, send `fullWorldState` for that. */
export function plotsPatch(plots: PlotView[], now: string): Record<string, unknown> {
  return { full: false, plots: plots.map(toGodotPlot), decor: [], now };
}

/** Strip counts per side (ADR-015); Godot rebuilds the island rectangle from them. */
export function toGodotExpand(island: IslandView): Record<string, number> {
  return { n: island.expand_n, e: island.expand_e, s: island.expand_s, w: island.expand_w };
}

export function toGodotWeather(weather: WeatherView): Record<string, unknown> {
  return { state: weather.state, until: weather.until };
}

export function weatherPatch(weather: WeatherView, now: string): Record<string, unknown> {
  return { full: false, plots: [], decor: [], weather: toGodotWeather(weather), now };
}

export function expandPatch(island: IslandView, now: string): Record<string, unknown> {
  return { full: false, plots: [], decor: [], expand: toGodotExpand(island), now };
}

export function resourcePatch(node: ResourceView, now: string): Record<string, unknown> {
  return { full: false, plots: [], decor: [], resources: [toGodotResource(node)], now };
}

/** Decor changes are sent as the full decor list so Godot can clear emptied slots. */
export function decorPatch(decor: IslandSnapshot["decor"], now: string): Record<string, unknown> {
  return { full: false, plots: [], decor: decor.map(toGodotDecor), decor_full: true, now };
}

export function toGodotDecor(d: IslandSnapshot["decor"][number]): Record<string, unknown> {
  return { id: d.id, item_id: d.item_id, x_dm: d.x_dm, z_dm: d.z_dm, yaw_deg: d.yaw_deg };
}
