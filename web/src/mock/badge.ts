// Позначка «Mock data» у кутку, поки мок-режим увімкнено: фейковий стан не сплутати з реальним.
import "./mock.css";

export function installMockBadge(): void {
  const badge = document.createElement("div");
  badge.className = "mock-badge";
  badge.textContent = "Mock data";
  badge.title = "Fake data for design review: nothing here is real. Remove ?mock from the address to leave.";
  document.body.append(badge);
}
