/** Іконка предмета (UX етап 3): клітинка атласу, відрендереного з моделей гри
 *  (`client/tools/render_icons.gd` → `pack_icons.gd`), розміром 1.5em від контейнера.
 *  Для id без клітинки — емодзі з `actions.ts`. Декоративна: назва завжди поруч текстом. */
import "./icons/items.css";
import { iconOf } from "./actions";
import { ITEM_ICON_IDS } from "./icons/items";

/** Номер дозволу на розширення поверх спільного сувою. */
const BADGE: Record<string, string> = { island_expand_1: "I", island_expand_2: "II", island_expand_3: "III", island_expand_4: "IV" };

export function hasItemIcon(id: string | null | undefined): id is string {
  return !!id && ITEM_ICON_IDS.has(id);
}

export function ItemIcon({ id }: { id: string }) {
  if (!hasItemIcon(id)) return <span aria-hidden="true">{iconOf(id)}</span>;
  const badge = BADGE[id];
  return (
    <span className={`item-icon ii-${id}`} aria-hidden="true" data-item={id}>
      {badge && <span className="ii-badge">{badge}</span>}
    </span>
  );
}
