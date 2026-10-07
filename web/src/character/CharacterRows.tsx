// The menu's character rows: a new look (ADR-038) and a new name (ADR-039), both paid in CLASH.
import { Icon } from "../hud/Icon";

function Row({ icon, title, text, onOpen }: { icon: "smile" | "book"; title: string; text: string; onOpen: () => void }) {
  return (
    <div className="row">
      <span className="em">
        <Icon name={icon} />
      </span>
      <div>
        <div className="t">{title}</div>
        <div className="d">{text}</div>
      </div>
      <button type="button" className="price" onClick={onOpen}>
        Open
      </button>
    </div>
  );
}

export function CharacterRows({ onLook, onName }: { onLook: () => void; onName: () => void }) {
  return (
    <>
      <Row icon="smile" title="Change look" text="Hair, face, body and clothes; your name stays" onOpen={onLook} />
      <Row icon="book" title="Change name" text="A new name above your head; the old one stays yours for 30 days" onOpen={onName} />
    </>
  );
}
