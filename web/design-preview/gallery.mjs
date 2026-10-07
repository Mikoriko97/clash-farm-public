/* global process, console */
// Builds design-shots/gallery.html: every screen side by side — baseline (current dark design) vs the variants.
//   node design-preview/gallery.mjs baseline aaply [more-variants...]
import { existsSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../design-shots");
const dirs = process.argv.slice(2).filter((d) => existsSync(join(root, d)));
if (dirs.length === 0) throw new Error("usage: gallery.mjs <dir> [<dir>...] (folders inside design-shots)");
const names = [...new Set(dirs.flatMap((d) => readdirSync(join(root, d)).filter((f) => f.endsWith(".png"))))].sort();
const groups = [["desktop", "Desktop 1440×900"], ["mobile", "Mobile 390×844"]];
const title = (file) => file.replace(/^(desktop|mobile)-/, "").replace(/\.png$/, "").replace(/^\d+-/, "").replaceAll("-", " ");

const section = ([prefix, label]) => {
  const files = names.filter((n) => n.startsWith(prefix));
  const cards = files.map((f) => `
    <figure class="card">
      <figcaption>${f.replace(".png", "")} <span>${title(f)}</span></figcaption>
      <div class="pair" style="--n:${dirs.length}">${dirs.map((d) => existsSync(join(root, d, f)) ? `<a href="${d}/${f}" target="_blank"><img loading="lazy" src="${d}/${f}" alt="${d} ${f}"><i>${d}</i></a>` : `<div class="miss">${d}: —</div>`).join("")}</div>
    </figure>`).join("");
  return `<h2>${label} <small>${files.length} screens</small></h2><div class="grid ${prefix}">${cards}</div>`;
};

writeFileSync(join(root, "gallery.html"), `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Clash Farm design shots</title>
<style>
:root{color-scheme:light;--c:#f2f2f2;--y:#e6e51e}
body{margin:0;background:var(--c) radial-gradient(circle at 1px 1px,#d0d0d0 1.1px,transparent 1.4px) 0 0/20px 20px;font:15px/1.5 Inter,system-ui,sans-serif;color:#000;padding:32px 24px 80px}
h1{font:700 44px/1 Poppins,system-ui;letter-spacing:-.05em;margin:0 0 6px}
h2{font:700 28px/1.1 Poppins,system-ui;letter-spacing:-.03em;margin:48px 0 16px}h2 small{font:500 14px Inter;letter-spacing:0;color:#555;margin-left:8px}
p.lead{margin:0;color:#444;max-width:70ch}
.grid{display:grid;gap:20px}.grid.desktop{grid-template-columns:repeat(auto-fill,minmax(min(100%,860px),1fr))}.grid.mobile{grid-template-columns:repeat(auto-fill,minmax(min(100%,520px),1fr))}
.card{margin:0;background:#fff;border-radius:30px;padding:18px}
figcaption{font-weight:600;margin:0 0 10px 6px}figcaption span{font-weight:400;color:#666;margin-left:8px}
.pair{display:grid;grid-template-columns:repeat(var(--n),1fr);gap:10px}
.pair a{position:relative;display:block;border-radius:18px;overflow:hidden;background:var(--c)}.pair img{display:block;width:100%;height:auto}
.pair i{position:absolute;left:10px;top:10px;font:500 12px Inter;background:#000;color:#fff;border-radius:999px;padding:3px 10px}.pair a:nth-child(2) i{background:var(--y);color:#000}
.miss{display:grid;place-items:center;color:#888;background:var(--c);border-radius:18px;min-height:80px}
</style></head><body>
<h1>Clash Farm — design shots</h1>
<p class="lead">${dirs.join(" vs ")}. The 3D world is a painted placeholder (the Godot export is not in this checkout); the HUD on top is the real React app on mock data. Click an image to open it full size.</p>
${groups.map(section).join("")}
</body></html>`);
console.log(`gallery: ${names.length} screens × ${dirs.length} variants → design-shots/gallery.html`);
