// SVG charts for the admin (no library, no inline styles — colours come from CSS classes,
// every dynamic shape is an attribute). A fixed viewBox keeps text proportional; the
// `<title>` on each mark is the hover tooltip.
import { groupThousands } from "../format";

export interface Point {
  date: string;
  value: number;
}

export type Tone = "moss" | "sun" | "sky" | "rose";

const W = 600;
const H = 200;
const PAD = { l: 44, r: 12, t: 12, b: 26 };

/** Axis ceiling that reads well: 1, 2, 2.5, 5 × 10^k just above `max`. */
export function niceMax(max: number): number {
  if (max <= 0) return 4;
  const p = 10 ** Math.floor(Math.log10(max));
  for (const m of [1, 2, 2.5, 4, 5, 10]) if (m * p >= max) return m * p;
  return 10 * p;
}

function fmt(n: number): string {
  return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 10_000 ? `${Math.round(n / 1000)}k` : groupThousands(Math.round(n));
}

function xLabels(points: Point[]): number[] {
  const n = points.length;
  if (n <= 8) return points.map((_, i) => i);
  const step = n > 20 ? 7 : 3;
  const out: number[] = [];
  for (let i = 0; i < n; i += step) out.push(i);
  // The last day always gets a label; drop a neighbour that would sit on top of it.
  if (n - 1 - (out[out.length - 1] ?? 0) < step / 2) out.pop();
  if (out[out.length - 1] !== n - 1) out.push(n - 1);
  return out;
}

function Grid({ top, points, format }: { top: number; points: Point[]; format: (n: number) => string }) {
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((f) => f * top);
  const y = (v: number) => PAD.t + (H - PAD.t - PAD.b) * (1 - v / top);
  const innerW = W - PAD.l - PAD.r;
  return (
    <g>
      {ticks.map((t) => (
        <g key={t}>
          <line x1={PAD.l} x2={W - PAD.r} y1={y(t)} y2={y(t)} className="grid" />
          <text x={PAD.l - 6} y={y(t) + 4} textAnchor="end" className="tick">{format(t)}</text>
        </g>
      ))}
      {xLabels(points).map((i) => (
        <text key={i} x={PAD.l + (innerW * (i + 0.5)) / points.length} y={H - 8} textAnchor="middle" className="tick">{points[i]?.date.slice(5) ?? ""}</text>
      ))}
    </g>
  );
}

/** One series as bars (`kind="bars"`) or an area with a line (`kind="area"`). */
export function Series({ points, tone = "moss", kind = "bars", format = fmt, label }: { points: Point[]; tone?: Tone; kind?: "bars" | "area"; format?: (n: number) => string; label?: string }) {
  return <MultiSeries series={[{ name: label ?? "", points, tone }]} kind={kind} format={format} legend={false} />;
}

/** Several series on one scale; bars sit side by side per day, areas overlap. */
export function MultiSeries({ series, kind = "bars", format = fmt, legend = true }: { series: { name: string; points: Point[]; tone: Tone }[]; kind?: "bars" | "area"; format?: (n: number) => string; legend?: boolean }) {
  const first = series[0]?.points ?? [];
  const n = first.length;
  const top = niceMax(Math.max(0, ...series.flatMap((s) => s.points.map((p) => p.value))));
  const innerW = W - PAD.l - PAD.r;
  const innerH = H - PAD.t - PAD.b;
  const y = (v: number) => PAD.t + innerH * (1 - v / top);
  const slot = n > 0 ? innerW / n : innerW;
  const xc = (i: number) => PAD.l + slot * (i + 0.5);
  return (
    <div className="chart-wrap">
      {legend && (
        <div className="legend-row">
          {series.map((s) => (
            <span key={s.name}><i className={`swatch ${s.tone}`} />{s.name}</span>
          ))}
        </div>
      )}
      <svg className="chart" viewBox={`0 0 ${W} ${H}`} role="img" aria-label={series.map((s) => s.name).join(", ") || "графік"}>
        <Grid top={top} points={first} format={format} />
        {kind === "area" &&
          series.map((s) => {
            const pts = s.points.map((p, i) => `${xc(i).toFixed(1)},${y(p.value).toFixed(1)}`);
            const base = `${xc(n - 1).toFixed(1)},${y(0).toFixed(1)} ${xc(0).toFixed(1)},${y(0).toFixed(1)}`;
            return (
              <g key={s.name} className={`series ${s.tone}`}>
                <polygon className="area" points={`${pts.join(" ")} ${base}`} />
                <polyline className="line" points={pts.join(" ")} fill="none" />
                {s.points.map((p, i) => (
                  <circle key={p.date} cx={xc(i)} cy={y(p.value)} r={i === n - 1 ? 4 : 2.5} className="dot"><title>{`${p.date}: ${s.name} ${groupThousands(p.value)}`}</title></circle>
                ))}
              </g>
            );
          })}
        {kind === "bars" &&
          series.map((s, si) => {
            const bw = Math.max(2, (slot * 0.7) / series.length);
            return (
              <g key={s.name} className={`series ${s.tone}`}>
                {s.points.map((p, i) => {
                  const h = p.value > 0 ? Math.max(1.5, innerH * (p.value / top)) : 0;
                  const x = xc(i) - (bw * series.length) / 2 + si * bw;
                  return (
                    <rect key={p.date} x={x} y={y(0) - h} width={bw} height={h} rx={Math.min(3, bw / 2)} className={p.value > 0 ? "bar" : "bar zero"}>
                      <title>{`${p.date}: ${s.name ? `${s.name} ` : ""}${groupThousands(p.value)}`}</title>
                    </rect>
                  );
                })}
              </g>
            );
          })}
      </svg>
    </div>
  );
}

/** Horizontal bars: label, bar, value — for levels, funnels and top lists. */
export function HBars({ rows, tone = "moss", max, format = (n) => groupThousands(n), percentOf }: { rows: { label: string; value: number; note?: string }[]; tone?: Tone; max?: number; format?: (n: number) => string; percentOf?: number }) {
  const top = Math.max(1, max ?? Math.max(0, ...rows.map((r) => r.value)));
  return (
    <div className="hbars">
      {rows.map((r) => (
        <div key={r.label} className="hbar">
          <span className="hbar-label" title={r.label}>{r.label}</span>
          <svg viewBox="0 0 100 10" preserveAspectRatio="none" className={`hbar-track ${tone}`} aria-hidden="true">
            <rect x="0" y="0" width="100" height="10" rx="3" className="track" />
            <rect x="0" y="0" width={Math.max(r.value > 0 ? 1 : 0, (r.value / top) * 100)} height="10" rx="3" className="fill" />
          </svg>
          <span className="hbar-value">
            <b>{format(r.value)}</b>
            {percentOf !== undefined && percentOf > 0 && <small> {Math.round((r.value * 100) / percentOf)}%</small>}
            {r.note && <small> {r.note}</small>}
          </span>
        </div>
      ))}
    </div>
  );
}

/** Ring with a filled share in basis points; the caller writes the label next to it. */
export function Ring({ bp, tone = "moss" }: { bp: number; tone?: Tone }) {
  const r = 40;
  const c = 2 * Math.PI * r;
  const share = Math.max(0, Math.min(10_000, bp)) / 10_000;
  return (
    <svg className={`ring ${tone}`} viewBox="0 0 100 100" role="img" aria-label={`${Math.round(share * 100)}%`}>
      <circle cx="50" cy="50" r={r} className="track" />
      <circle cx="50" cy="50" r={r} className="fill" strokeDasharray={`${c * share} ${c}`} transform="rotate(-90 50 50)" />
      <text x="50" y="55" textAnchor="middle" className="ring-label">{Math.round(share * 100)}%</text>
    </svg>
  );
}
