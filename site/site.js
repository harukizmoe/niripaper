"use strict";
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const assets = document.body.dataset.assets;
const clamp = (n, lo, hi) => Math.min(hi, Math.max(lo, n));

// The daemon's defaults (docs/SPEC.md 2.16): scale 1.1, both spans 6, overview zoom 0.96.
const SCALE = 1.1, SPAN = 6, OVERVIEW_ZOOM = 0.96;

// Swap a picture inside a `.stack` with the iris transition: a circle of the new
// picture opens over the old one. Same idea as the daemon's, drawn with clip-path.
async function setWall(stack, name) {
  const img = new Image();
  img.src = `${assets}wall-${name}.webp`;
  img.alt = "";
  await img.decode().catch(() => {});
  const old = [...stack.children];
  stack.append(img);
  if (!reduced) {
    const at = "50% 45%";
    await img.animate(
      { clipPath: [`circle(0% at ${at})`, `circle(150% at ${at})`] },
      { duration: 1200, easing: "cubic-bezier(0.215, 0.61, 0.355, 1)" },
    ).finished.catch(() => {});
  }
  old.forEach((o) => o.remove());
}

// A miniature niri: columns scroll sideways inside a workspace, workspaces stack
// vertically. The wallpaper is positioned with the daemon's own formula
// (docs/SPEC.md 4.1): progress = index / (span - 1), x = overflow * (0.5 - progress).
function demo(root) {
  const LAYOUT = [
    [["terminal"], ["editor"], ["browser", "chat"], ["files"], ["mpv"]],
    [["notes"], ["browser"], ["terminal"], ["editor"]],
    [["chat"], ["music"], ["files"]],
  ];
  const WIDTHS = [46, 34, 40];
  const BARS = [62, 44, 78, 36, 56, 70, 40];
  const screen = $("[data-screen]", root);
  const canvas = $("[data-canvas]", root);
  const strip = $("[data-strip]", root);
  const out = Object.fromEntries($$("[data-o]", root).map((o) => [o.dataset.o, o]));
  const state = { ws: 0, focus: LAYOUT.map(() => 0), rx: LAYOUT.map(() => 0), overview: false, parallax: true };

  const rows = LAYOUT.map((cols, w) => {
    const ws = Object.assign(document.createElement("div"), { className: "ws" });
    ws.style.setProperty("--i", w);
    const row = Object.assign(document.createElement("div"), { className: "row" });
    cols.forEach((tiles, c) => {
      const col = Object.assign(document.createElement("div"), { className: "col" });
      col.style.setProperty("--w", `${WIDTHS[(c + w) % WIDTHS.length]}cqw`);
      tiles.forEach((title, t) => {
        const win = Object.assign(document.createElement("div"), { className: "win" });
        win.dataset.ws = w;
        win.dataset.col = c;
        win.dataset.tile = t;
        win.dataset.h = (c + w) % 5;
        win.innerHTML = `<b>${title}</b>` + Array.from({ length: tiles.length > 1 ? 2 : 5 }, (_, k) => `<i style="--bar:${BARS[(c + k + w) % BARS.length]}%"></i>`).join("");
        col.append(win);
      });
      row.append(col);
    });
    ws.append(row);
    strip.append(ws);
    return row;
  });

  // Scroll the row the least that keeps the focused column fully in view, as niri does.
  function reveal(w) {
    const col = rows[w].children[state.focus[w]];
    const pad = parseFloat(getComputedStyle(rows[w]).paddingLeft);
    const left = col.offsetLeft, right = left + col.offsetWidth, view = screen.clientWidth;
    if (left < -state.rx[w] + pad) state.rx[w] = -left + pad;
    else if (right > -state.rx[w] + view - pad) state.rx[w] = -(right - view + pad);
    state.rx[w] = Math.min(0, state.rx[w]);
    rows[w].style.setProperty("--rx", `${state.rx[w]}px`);
  }

  function render() {
    const n = LAYOUT[state.ws].length, f = state.focus[state.ws];
    const h = !state.parallax ? 0.5 : n === 1 ? 0 : clamp(f / (SPAN - 1), 0, 1);
    const v = !state.parallax ? 0.5 : clamp(state.ws / (SPAN - 1), 0, 1);
    // Percentages of the canvas: overflow / canvas size = (scale - 1) / scale.
    const k = (SCALE - 1) / SCALE * 100;
    canvas.style.setProperty("--tx", `${k * (0.5 - h)}%`);
    canvas.style.setProperty("--ty", `${k * (0.5 - v)}%`);
    canvas.style.setProperty("--zoom", state.overview ? OVERVIEW_ZOOM : 1);
    screen.classList.toggle("overview", state.overview);
    strip.style.setProperty("--ws", state.ws);
    rows.forEach((_, w) => {
      reveal(w);
      rows[w].parentElement.classList.toggle("active", w === state.ws);
    });
    $$(".win", strip).forEach((win) => {
      const on = +win.dataset.ws === state.ws && +win.dataset.col === f && win.dataset.tile === "0";
      win.classList.toggle("focused", on);
    });
    out.col.value = f + 1;
    out.cols.value = n;
    out.ws.value = state.ws + 1;
    out.h.value = h.toFixed(2);
    out.v.value = v.toFixed(2);
    $("[data-overview]", root).setAttribute("aria-pressed", state.overview);
  }

  function move(dc, dw) {
    if (dw) state.ws = clamp(state.ws + dw, 0, LAYOUT.length - 1); // each workspace remembers its own focus
    if (dc) state.focus[state.ws] = clamp(state.focus[state.ws] + dc, 0, LAYOUT[state.ws].length - 1);
    render();
  }

  // Any real interaction ends the self-running tour.
  let tour = null;
  const stopTour = () => { clearInterval(tour); tour = null; };

  $$("[data-move]", root).forEach((b) => b.addEventListener("click", () => {
    stopTour();
    move(...b.dataset.move.split(",").map(Number));
  }));
  $("[data-overview]", root).addEventListener("click", () => { stopTour(); state.overview = !state.overview; render(); });
  $("[data-parallax]", root).addEventListener("change", (e) => { stopTour(); state.parallax = e.target.checked; render(); });
  $$("[data-wall]", root).forEach((b) => b.addEventListener("click", () => {
    stopTour();
    $$("[data-wall]", root).forEach((o) => o.setAttribute("aria-pressed", o === b));
    setWall(canvas, b.dataset.wall);
    setWall($("[data-backdrop]"), b.dataset.wall);
  }));
  strip.addEventListener("click", (e) => {
    const win = e.target.closest(".win");
    if (!win) return;
    stopTour();
    state.ws = +win.dataset.ws;
    state.focus[state.ws] = +win.dataset.col;
    state.overview = false;
    render();
  });
  screen.addEventListener("keydown", (e) => {
    const keys = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    if (e.key in keys) move(...keys[e.key]);
    else if (e.key.toLowerCase() === "o") state.overview = !state.overview, render();
    else return;
    stopTour();
    e.preventDefault();
  });
  new ResizeObserver(render).observe(screen);
  render();

  // Show the motion without being asked: a short loop while the demo is on screen.
  const STEPS = ["r", "r", "r", "d", "r", "r", "d", "o", "o", "u", "u", "l", "l", "l"];
  let i = 0, visible = false;
  const tick = () => {
    if (document.hidden || !visible) return;
    const s = STEPS[i++ % STEPS.length];
    if (s === "o") { state.overview = !state.overview; render(); }
    else move(...{ r: [1, 0], l: [-1, 0], u: [0, -1], d: [0, 1] }[s]);
  };
  if (!reduced) {
    new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { threshold: 0.5 }).observe(screen);
    tour = setInterval(tick, 1700);
  }
}

// Transition clips: play on hover or focus, tap to toggle on touch, poster when idle.
function gallery() {
  $$(".tr figure").forEach((fig) => {
    const v = $("video", fig);
    const play = () => v.play().catch(() => {});
    const stop = () => { v.pause(); v.load(); };
    fig.addEventListener("pointerenter", (e) => e.pointerType === "mouse" && play());
    fig.addEventListener("pointerleave", (e) => e.pointerType === "mouse" && stop());
    fig.addEventListener("focus", play);
    fig.addEventListener("blur", stop);
    fig.addEventListener("click", (e) => { if (e.detail && matchMedia("(hover: none)").matches) v.paused ? play() : stop(); });
  });
}

function copyButtons() {
  $$("[data-copy]").forEach((b) => {
    const label = b.textContent;
    b.addEventListener("click", async () => {
      const text = b.parentElement.querySelector("pre").textContent;
      try { await navigator.clipboard.writeText(text); } catch {
        const r = document.createRange();
        r.selectNodeContents(b.parentElement.querySelector("pre"));
        getSelection().removeAllRanges();
        getSelection().addRange(r);
        return;
      }
      b.textContent = b.dataset.copied;
      setTimeout(() => (b.textContent = label), 1500);
    });
  });
}

// Release download: asset names follow niripaper-<tag>-x86_64-linux-gnu-<mpv>.tar.gz.
// The tag comes from the data-cmd markup, which is updated whenever a release ships.
function download() {
  const cmd = $("[data-cmd]"), link = $("[data-dl]");
  if (!cmd) return;
  const tag = cmd.textContent.match(/download\/(v[^/]+)\//)[1];
  const render = () => {
    const mpv = $('input[name="mpv"]:checked').value;
    const file = `niripaper-${tag}-x86_64-linux-gnu-${mpv}.tar.gz`;
    const url = `https://github.com/harukizmoe/niripaper/releases/download/${tag}/${file}`;
    link.href = url;
    cmd.textContent = `curl -LO ${url}\nmkdir niripaper-release\ntar xzf ${file} -C niripaper-release\ninstall -Dm755 niripaper-release/niripaper niripaper-release/niripaper-app -t ~/.local/bin`;
  };
  $$('input[name="mpv"]').forEach((r) => r.addEventListener("change", render));
  render();
}
download();
const d = $("[data-demo]");
if (d) demo(d);
gallery();
copyButtons();
