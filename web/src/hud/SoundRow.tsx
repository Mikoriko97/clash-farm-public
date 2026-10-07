import { sendToGodot } from "../bridge/host";
import { noteVolume, useGodotSettings } from "./godotSettings";
import { Icon } from "./Icon";
import { playUi, setUiVolume } from "./sound";

/** Гучність у меню: «effects» (дії у світі + звуки інтерфейсу) і «ambience» (море, ліс, печера).
 *  Значення йдуть у Godot (`set_volume`), який зберігає їх у user://settings.cfg і відповідає
 *  `volume_changed`; web бере з нього гучність UI-звуків. Стан читаємо зі спільного стору
 *  `godotSettings` (слухає Godot ще до відкриття меню). */
export function SoundRow() {
  const { sfx, ambient } = useGodotSettings().volume;
  const send = (next: { sfx: number; ambient: number }) => {
    noteVolume(next);
    setUiVolume(next.sfx);
    sendToGodot("set_volume", next);
  };
  return (
    <div className="row sound">
      <span className="em"><Icon name="speaker" /></span>
      <div>
        <div className="t">Sound</div>
        <div className="d">
          <label>
            effects
            <input type="range" min={0} max={100} step={5} value={sfx} aria-label="Effects volume" onChange={(e) => send({ sfx: Number(e.target.value), ambient })} onPointerUp={() => playUi("click")} />
            <span>{sfx === 0 ? "off" : `${sfx}%`}</span>
          </label>
          <label>
            ambience
            <input type="range" min={0} max={100} step={5} value={ambient} aria-label="Ambience volume" onChange={(e) => send({ sfx, ambient: Number(e.target.value) })} />
            <span>{ambient === 0 ? "off" : `${ambient}%`}</span>
          </label>
        </div>
      </div>
      <button type="button" className="price quiet" onClick={() => send(sfx === 0 && ambient === 0 ? { sfx: 80, ambient: 60 } : { sfx: 0, ambient: 0 })}>
        {sfx === 0 && ambient === 0 ? "Unmute" : "Mute"}
      </button>
    </div>
  );
}
