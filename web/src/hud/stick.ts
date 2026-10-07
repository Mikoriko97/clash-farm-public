// Геометрія джойстика на екрані: зсув пальця від центру → вектор для Godot (`move_input`) і ознака бігу.
// Окремо від компонента, щоб рахунок можна було перевірити без DOM.

/** Вектор стіка від такої довжини (частка радіуса) Godot вважає бігом: те саме значення в `client/scripts/stick_run.gd` (`RUN_AT`). */
export const RUN_AT = 0.85;

export interface StickVector {
  x: number;
  y: number;
  /** Палець біля зовнішнього краю кола: персонаж біжить. */
  run: boolean;
}

/** Зсув пальця `dx, dy` (px) при радіусі ходу кнопки `radius` (px): довжина обмежується колом, вектор 0..1 з двома знаками. */
export function stickVector(dx: number, dy: number, radius: number): StickVector {
  const r = Math.max(radius, 1);
  const len = Math.hypot(dx, dy);
  const k = len > r ? r / len : 1;
  const x = Number(((dx * k) / r).toFixed(2));
  const y = Number(((dy * k) / r).toFixed(2));
  return { x, y, run: Math.hypot(x, y) >= RUN_AT };
}

/** Радіус ходу кнопки: від центру майданчика до краю мінус півтолщі кнопки (кнопка не вилазить за коло, будь-який розмір майданчика). */
export function travelRadius(padSize: number, knobSize: number): number {
  return Math.max(padSize / 2 - knobSize / 2, 1);
}
