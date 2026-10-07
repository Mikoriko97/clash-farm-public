import { sendToGodot } from "../bridge/host";
import { Icon } from "./Icon";

/** Кнопка стрибка для сенсорних екранів поруч із джойстиком; на клавіатурі — пробіл. */
export function JumpButton() {
  return (
    <button
      type="button"
      className="jump glass"
      aria-label="Jump"
      onPointerDown={(e) => {
        e.preventDefault();
        sendToGodot("jump", {});
      }}
    >
      <Icon name="jump" size={26} />
    </button>
  );
}
