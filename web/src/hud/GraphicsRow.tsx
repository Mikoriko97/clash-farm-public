import { sendToGodot } from "../bridge/host";
import { noteQuality, useGodotSettings } from "./godotSettings";
import { Icon } from "./Icon";

const TIERS: Array<[string, string]> = [
  ["auto", "Auto"],
  ["low", "Low"],
  ["medium", "Medium"],
  ["high", "High"],
];
const NAMES: Record<string, string> = { low: "low", medium: "medium", high: "high" };

/** Рівень графіки в меню: вибір іде в Godot (`set_quality`), а `quality_changed`
 *  повертає, що діє насправді (в режимі «Auto» слабка машина сама сходить нижче).
 *  Зберігає сам клієнт Godot (user://settings.cfg); значення читаємо зі стору
 *  `godotSettings`, який слухає Godot ще до першого відкриття меню. */
export function GraphicsRow() {
  const { tier, effective } = useGodotSettings().quality;
  const pick = (t: string) => {
    noteQuality(t);
    sendToGodot("set_quality", { tier: t });
  };
  return (
    <div className="row">
      <span className="em"><Icon name="sliders" /></span>
      <div>
        <div className="t">Graphics</div>
        <div className="d">{tier === "auto" && effective ? `Auto: currently ${NAMES[effective] ?? effective}` : "Shadows, anti-aliasing, grass density"}</div>
      </div>
      <select className="price" value={tier} onChange={(e) => pick(e.target.value)} aria-label="Graphics quality">
        {TIERS.map(([id, label]) => (
          <option key={id} value={id}>
            {label}
          </option>
        ))}
      </select>
    </div>
  );
}
