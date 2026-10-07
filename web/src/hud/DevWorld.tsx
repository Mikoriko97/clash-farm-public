import { useState } from "react";
import { sendToGodot } from "../bridge/host";

/** Dev-only world controls (menu): time of day, weather, effects. Never shipped: guarded by import.meta.env.DEV. */
export function DevWorld() {
  const [hour, setHour] = useState(12);
  const [live, setLive] = useState(true);
  const [dayLength, setDayLength] = useState(24);
  const [stats, setStats] = useState(false);

  const setTime = (h: number, isLive: boolean) => {
    setHour(h);
    setLive(isLive);
    sendToGodot("debug_world", isLive ? { hour: null } : { hour: h });
  };
  const weather = (w: string) => sendToGodot("debug_world", { weather: w });
  const fx = (kind: string) => sendToGodot("play_fx", { fx: kind, target: { plot_idx: 0 } });

  return (
    <div className="dev">
      <h3>World (dev)</h3>
      <label className="dev-row">
        <span>
          Hour <b className="num">{hour.toFixed(1)}</b>
        </span>
        <input type="range" min={0} max={24} step={0.5} value={hour} onChange={(e) => setTime(Number(e.target.value), false)} />
      </label>
      <label className="dev-row">
        <input type="checkbox" checked={live} onChange={(e) => setTime(hour, e.target.checked)} /> live time
      </label>
      <label className="dev-row">
        <input
          type="checkbox"
          checked={stats}
          onChange={(e) => {
            setStats(e.target.checked);
            sendToGodot("debug_world", { stats: e.target.checked });
          }}
        />{" "}
        frame stats (F3)
      </label>
      <label className="dev-row">
        <span>Day length, min</span>
        <input
          type="number"
          min={1}
          max={1440}
          value={dayLength}
          onChange={(e) => {
            const v = Math.max(1, Math.min(1440, Number(e.target.value) || 24));
            setDayLength(v);
            sendToGodot("debug_world", { day_length_min: v });
          }}
        />
      </label>
      <div className="dev-buttons">
        {[
          ["auto", "Auto"],
          ["clear", "Clear"],
          ["cloudy", "Cloudy"],
          ["fog", "Fog"],
          ["rain", "Rain"],
          ["storm", "Storm"],
        ].map(([id, label]) => (
          <button key={id} type="button" className="btn" onClick={() => weather(id!)}>
            {label}
          </button>
        ))}
        <button type="button" className="btn" onClick={() => sendToGodot("debug_world", { lightning: true })}>
          ⚡ Lightning
        </button>
      </div>
      <div className="dev-buttons">
        {[
          ["plant", "Plant"],
          ["harvest", "Harvest"],
          ["water", "Water"],
        ].map(([id, label]) => (
          <button key={id} type="button" className="btn" onClick={() => fx(id!)}>
            FX: {label}
          </button>
        ))}
      </div>
    </div>
  );
}
