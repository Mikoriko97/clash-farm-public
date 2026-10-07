import { useEffect, useState } from "react";
import type { ItemView } from "../api/world";
import { sendToGodot } from "../bridge/host";
import { ITEM_LABEL } from "../island/labels";
import { useAppStore } from "../store/appStore";
import { Icon } from "./Icon";
import { levelMin } from "./shopInfo";
import { playUi } from "./sound";

const SHOW_MS = 7000;

/** Що відкрив цей рівень: предмети з `meta.level_min` і грядка на парних рівнях
 *  (6 + ⌊рівень/2⌋, docs/ECONOMY.md). Порожньо — просто вітаємо. */
export function unlocksFor(level: number, catalog: ItemView[]): string[] {
  const items = catalog.filter((c) => levelMin(c) === level).map((c) => ITEM_LABEL[c.id] ?? c.id);
  return level % 2 === 0 ? ["+1 plot", ...items] : items;
}

/** Святкування рівня (UX етап 2): помітний тост із тим, що відкрилось, і фанфари.
 *  Монтується заново на кожен рівень (key = seq) і ховає себе через SHOW_MS. */
export function LevelUp() {
  const levelUp = useAppStore((s) => s.levelUp);
  if (!levelUp) return null;
  return <LevelUpToast key={levelUp.seq} level={levelUp.level} />;
}

function LevelUpToast({ level }: { level: number }) {
  const catalog = useAppStore((s) => s.catalog);
  const [gone, setGone] = useState(false);
  useEffect(() => {
    playUi("levelup");
    // персонаж радіє разом з тостом (ADR-035; Godot чекає, поки він доробить дію, що дала рівень)
    sendToGodot("play_fx", { fx: "cheer" });
    const t = window.setTimeout(() => setGone(true), SHOW_MS);
    return () => window.clearTimeout(t);
  }, []);
  if (gone) return null;
  const unlocked = unlocksFor(level, catalog);
  return (
    <div className="toast glass big levelup" role="status">
      <Icon name="star" />
      <span>
        <b>Level {level}!</b>
        {unlocked.length > 0 ? ` Unlocked: ${unlocked.slice(0, 4).join(", ")}${unlocked.length > 4 ? "…" : ""}` : " Keep it up."}
      </span>
    </div>
  );
}
