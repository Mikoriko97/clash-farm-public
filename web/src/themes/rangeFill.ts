// Keeps --fill of every <input type="range"> equal to its position so a themed track can show the filled part
// (CSS alone cannot do that in Chromium). Style properties are set through the CSSOM, which the CSP allows.
function sync(input: HTMLInputElement): void {
  const min = Number(input.min || 0);
  const max = Number(input.max || 100);
  const pct = max > min ? ((Number(input.value) - min) / (max - min)) * 100 : 0;
  input.style.setProperty("--fill", `${pct}%`);
}

export function installRangeFill(): void {
  const scan = (root: ParentNode) => root.querySelectorAll<HTMLInputElement>('input[type="range"]').forEach(sync);
  document.addEventListener("input", (e) => {
    if (e.target instanceof HTMLInputElement && e.target.type === "range") sync(e.target);
  });
  new MutationObserver(() => scan(document)).observe(document.body, { childList: true, subtree: true });
  scan(document);
}
