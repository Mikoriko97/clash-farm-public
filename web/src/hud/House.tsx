import { Icon } from "./Icon";
import { PanelFrame } from "./panels";

interface HouseProps {
  busy: boolean;
  onSleep: () => void;
  onCook: () => void;
  onClose: () => void;
}

/** Inside the house: sleep (XP once a day, the server counts the day) or cook (the craft panel). */
export function HouseSheet({ busy, onSleep, onCook, onClose }: HouseProps) {
  return (
    <PanelFrame title="House" className="sheet" onClose={onClose}>
      <p className="hint">Here you can sleep and cook something from what you gathered. Sleep gives a little XP once a day and leaves you rested: more XP from everything you do for a while.</p>
      <div className="rows">
        <div className="row shop-card">
          <span className="em"><Icon name="bed" /></span>
          <div>
            <div className="t">Go to sleep</div>
            <div className="d">XP and a rested bonus, once a day</div>
          </div>
          <button type="button" className="buy gold" disabled={busy} onClick={onSleep}>
            Sleep
          </button>
        </div>
        <div className="row shop-card">
          <span className="em"><Icon name="pot" /></span>
          <div>
            <div className="t">Cook</div>
            <div className="d">dishes from your produce at the table</div>
          </div>
          <button type="button" className="buy" disabled={busy} onClick={onCook}>
            To the table
          </button>
        </div>
      </div>
    </PanelFrame>
  );
}
