/** Лінія однієї серії за останні N днів: тонка лінія 2 px, мʼяка заливка до базової
 *  лінії, хрестовина й підказка під курсором чи пальцем. Значення пишуться текстовими
 *  токенами HUD, колір лінії несе лише саму лінію (одна серія — легенда не потрібна). */
import { useState } from "react";

const W = 300;
const H = 64;
const PAD = 4;

interface Props {
  points: number[];
  /** Назва серії для підказки й читалок екрана. */
  label: string;
  format: (value: number) => string;
}

export function Sparkline({ points, label, format }: Props) {
  const [hover, setHover] = useState<number | null>(null);
  const n = points.length;
  if (n < 2) return null;
  const min = Math.min(...points);
  const max = Math.max(...points);
  const span = Math.max(1, max - min);
  const x = (i: number) => (i * W) / (n - 1);
  const y = (v: number) => PAD + ((max - v) * (H - PAD * 2)) / span;
  const line = points.map((v, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)} ${y(v).toFixed(1)}`).join(" ");
  const at = hover ?? n - 1;
  const value = points[at] ?? 0;
  const ago = n - 1 - at;
  const pick = (clientX: number, box: DOMRect) => setHover(Math.max(0, Math.min(n - 1, Math.round(((clientX - box.left) / box.width) * (n - 1)))));
  return (
    <figure className="spark">
      <figcaption>
        <span>{label} · 30 days</span>
        <span className="num">
          {format(value)} <small>{ago === 0 ? "today" : `${ago}d ago`}</small>
        </span>
      </figcaption>
      <div
        className="spark-plot"
        role="img"
        aria-label={`${label}: now ${format(points[n - 1] ?? 0)}, min ${format(min)}, max ${format(max)} over 30 days`}
        onPointerMove={(e) => pick(e.clientX, e.currentTarget.getBoundingClientRect())}
        onPointerDown={(e) => pick(e.clientX, e.currentTarget.getBoundingClientRect())}
        onPointerLeave={() => setHover(null)}
      >
        <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden="true">
          <path className="area" d={`${line} L${W} ${H} L0 ${H} Z`} />
          <path className="line" d={line} vectorEffect="non-scaling-stroke" />
        </svg>
        <i className="cross" style={{ left: `${(x(at) * 100) / W}%` }} hidden={hover === null} />
        <i className="dot" style={{ left: `${(x(at) * 100) / W}%`, top: `${(y(value) * 100) / H}%` }} />
        <span className="axis hi num">{format(max)}</span>
        <span className="axis lo num">{format(min)}</span>
      </div>
    </figure>
  );
}
