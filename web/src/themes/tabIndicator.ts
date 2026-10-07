// Sliding highlight behind the selected button of segmented controls (.tabs, .city-nav, .cc-tabs).
// The pill is a ::before of the container (controls.css); here only its position is measured into
// --ind-x / --ind-w and re-measured when the selection, the content or the size changes.
const GROUPS = ".tabs, .city-nav, .cc-tabs";
const SELECTED = '[aria-selected="true"], [aria-current="true"]';

function place(group: HTMLElement): void {
  const sel = group.querySelector<HTMLElement>(`button${SELECTED.replaceAll(", ", ", button")}`);
  if (!sel) {
    delete group.dataset["ind"];
    return;
  }
  const first = group.dataset["ind"] === undefined;
  group.style.setProperty("--ind-x", `${sel.offsetLeft}px`);
  group.style.setProperty("--ind-y", `${sel.offsetTop}px`);
  group.style.setProperty("--ind-w", `${sel.offsetWidth}px`);
  group.style.setProperty("--ind-h", `${sel.offsetHeight}px`);
  group.toggleAttribute("data-ind-c", sel.classList.contains("c"));
  group.dataset["ind"] = "on";
  // no slide on the very first placement: the pill appears in place, later changes animate
  if (first) requestAnimationFrame(() => requestAnimationFrame(() => (group.dataset["ind"] = "ready")));
}

export function installTabIndicator(): void {
  const all = () => document.querySelectorAll<HTMLElement>(GROUPS).forEach(place);
  let queued = false;
  const schedule = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      all();
    });
  };
  new MutationObserver(schedule).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["aria-selected", "aria-current", "class"] });
  window.addEventListener("resize", schedule);
  document.fonts?.ready.then(schedule).catch(() => undefined);
  schedule();
}
