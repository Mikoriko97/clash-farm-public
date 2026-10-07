/** Що відкривається за дверима будівлі-вітрини. У демо-режимі (demoMode.ts) три
 *  вітрини показують готові екрани партнерів на тестових даних — храм стейкінгу,
 *  обмінник, скарбниця стейблів — з перемикачем між будівлями, щоб показати місто
 *  без прогулянки. Поза демо й для решти вітрин — чесне «ще ніким не зайнято». */
import { useEffect, useRef, useState } from "react";
import type { CityBuilding } from "../api/mainland";
import { MonumentSheet, ShowcaseSheet } from "../hud/CitySheets";
import { Icon } from "../hud/Icon";
import { PanelFrame } from "../hud/panels";
import { DEMO_BUILDINGS, type DemoBuilding, demoBuilding } from "./demoData";
import { isCityDemo } from "./demoMode";
import { useDemoStore } from "./demoStore";
import { StableVaults } from "./StableVaults";
import { StakeTemple } from "./StakeTemple";
import { SwapHouse } from "./SwapHouse";
import "./city.css";

const INTEGRATION_LABEL = { partner_api: "Partner SDK in the player's wallet", own_pool: "Own pool" } as const;

function View({ building }: { building: DemoBuilding }) {
  if (building.view === "stake") return <StakeTemple />;
  if (building.view === "swap") return <SwapHouse />;
  return <StableVaults />;
}

export function CityDoor({ building, onClose }: { building: CityBuilding | null; onClose: () => void }) {
  const entered = isCityDemo() ? demoBuilding(building?.id) : null;
  const [activeId, setActiveId] = useState(entered?.id ?? null);
  const reset = useDemoStore((s) => s.reset);
  const nav = useRef<HTMLElement>(null);
  // Інша будівля — з початку екрана, а не з місця, де прокрутку лишив попередній.
  useEffect(() => {
    nav.current?.scrollIntoView({ block: "start" });
  }, [activeId]);
  const active = DEMO_BUILDINGS.find((b) => b.id === activeId) ?? entered;
  if (!active) return building?.kind === "monument" ? <MonumentSheet building={building} onClose={onClose} /> : <ShowcaseSheet building={building} onClose={onClose} />;
  return (
    <PanelFrame title={active.name} className="sheet city" onClose={onClose}>
      <nav ref={nav} className="city-nav" aria-label="City buildings">
        {DEMO_BUILDINGS.map((b) => (
          <button key={b.id} type="button" aria-current={b.id === active.id} onClick={() => setActiveId(b.id)}>
            <Icon name={b.icon} size={18} />
            {b.short}
          </button>
        ))}
      </nav>
      <div className="city-head">
        <span className="city-ic">
          <Icon name={active.icon} size={26} />
        </span>
        <div>
          <p className="purpose">{active.purpose}</p>
          <p className="chips">
            <span className="tag demo">
              <Icon name="flask" size={13} /> Demo · test data
            </span>
            <span className="tag">{active.partner}</span>
            {active.integration.map((kind) => (
              <span key={kind} className={`tag ${kind}`}>
                {INTEGRATION_LABEL[kind]}
              </span>
            ))}
          </p>
        </div>
      </div>
      <View key={active.id} building={active} />
      <div className="city-foot">
        <p className="hint">
          <Icon name="trend" size={14} /> Project revenue: {active.revenue}
        </p>
        <p className="hint">Interface preview on made-up numbers: no wallet is connected, no transactions are sent, and rates are not a promise of yield. Signing always happens in the player's wallet — the game never holds tokens.</p>
        <button type="button" className="reset" onClick={reset}>
          Reset demo balances
        </button>
      </div>
    </PanelFrame>
  );
}
