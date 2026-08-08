export function createViewer({ canvas, bar, statusEl }) {
  const ctx = canvas.getContext("2d");
  const status = m => { statusEl.textContent = m; };

  const worker = new Worker("worker.js", { type: "module" });
  let nextId = 1;
  const waiting = new Map();
  function call(msg, transfer = []) {
    return new Promise((res, rej) => {
      msg.id = nextId++;
      waiting.set(msg.id, { res, rej });
      worker.postMessage(msg, transfer);
    });
  }
  worker.addEventListener("message", e => {
    const m = e.data;
    if (m.type === "log") {
      console.error("[mciso]", m.msg);
      status(m.msg);
      return;
    }
    const w = waiting.get(m.id);
    if (!w) return;
    waiting.delete(m.id);
    if (m.error) w.rej(new Error(m.error));
    else w.res(m);
  });

  const ready = call({ type: "init" });

  const SIDES = ["tl", "tr", "br", "bl"];
  const SIDE_LABELS = ["top left", "top right", "bottom right", "bottom left"];
  const TIERS = [2, 4, 8, 16, 32];
  const MAX_Z = 32;
  const RENDER_CAP = 32 << 20;
  const minZ = () => Math.max(0.25, 2 * Math.sqrt(canvas.width * canvas.height / RENDER_CAP));
  let blocks = 0;
  const view = { turns: 0, z: 8, cx: 0, cy: 0 };
  let cache = null;
  let prev = null;
  const bases = new Map();

  function dropFrames() {
    cache?.bmp.close();
    prev?.bmp.close();
    cache = null;
    prev = null;
  }

  function dropBases() {
    for (const b of bases.values()) b?.bmp.close();
    bases.clear();
  }

  async function requestBase() {
    const turns = view.turns;
    if (bases.has(turns)) return;
    bases.set(turns, null);
    try {
      const [nw, nh] = await normSize(turns);
      const iw = Math.ceil(nw * 2);
      const ih = Math.ceil(nh * 2);
      const w = Math.min(iw, 4096);
      const h = Math.min(ih, 4096);
      const m = await call({ type: "render", turns, ht: 2, x: (iw - w) >> 1, y: (ih - h) >> 1, w, h });
      if (m.empty) {
        bases.delete(turns);
        return;
      }
      bases.set(turns, { ht: 2, ix: m.ix, iy: m.iy, vw: m.vw, vh: m.vh, bmp: m.bmp });
      scheduleComposite();
    } catch {
      bases.delete(turns);
    }
  }

  const sideButtons = SIDES.map((side, i) => {
    const b = document.createElement("button");
    b.textContent = SIDE_LABELS[i];
    b.disabled = true;
    b.addEventListener("click", async () => {
      view.turns = i;
      dropFrames();
      scheduleComposite();
      await recenter();
      requestRender();
      requestBase();
    });
    bar.append(b);
    return b;
  });
  const zoomButtons = [["-", 0.5], ["+", 2]].map(([label, factor]) => {
    const b = document.createElement("button");
    b.textContent = label;
    b.disabled = true;
    b.addEventListener("click", () => zoomTo(view.z * factor, canvas.width / 2, canvas.height / 2));
    bar.append(b);
    return b;
  });
  bar.append(statusEl);

  async function normSize(turns = view.turns) {
    const m = await call({ type: "size", turns });
    return [m.nw, m.nh];
  }

  function sizeCanvas() {
    const dpr = devicePixelRatio || 1;
    const cssH = Math.max(innerHeight - canvas.getBoundingClientRect().top - 16, 300);
    canvas.width = Math.round(canvas.clientWidth * dpr);
    canvas.height = Math.round(cssH * dpr);
    canvas.style.height = `${cssH}px`;
  }

  async function recenter() {
    const [nw, nh] = await normSize();
    view.cx = nw / 2;
    view.cy = nh / 2;
  }

  async function fitView() {
    const [nw, nh] = await normSize();
    view.z = Math.min(MAX_Z, Math.max(minZ(), Math.min(canvas.width / nw, canvas.height / nh)));
    view.cx = nw / 2;
    view.cy = nh / 2;
  }

  function drawFrame(f) {
    const s = view.z / f.ht;
    const ox = view.cx * f.ht - canvas.width / (2 * s);
    const oy = view.cy * f.ht - canvas.height / (2 * s);
    ctx.drawImage(f.bmp, 0, 0, f.vw, f.vh, (f.ix - ox) * s, (f.iy - oy) * s, f.vw * s, f.vh * s);
  }

  function composite() {
    sideButtons.forEach((b, i) => { b.disabled = !blocks || i === view.turns; });
    zoomButtons[0].disabled = !blocks || view.z <= minZ() * 1.001;
    zoomButtons[1].disabled = !blocks || view.z >= MAX_Z * 0.999;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "medium";
    const base = bases.get(view.turns);
    if (base) drawFrame(base);
    if (prev) drawFrame(prev);
    if (cache) drawFrame(cache);
  }

  let pending = false;
  function scheduleComposite() {
    if (pending) return;
    pending = true;
    requestAnimationFrame(() => { pending = false; composite(); });
  }

  let inFlight = false;
  let dirty = false;
  async function requestRender() {
    if (!blocks) return;
    if (inFlight) {
      dirty = true;
      return;
    }
    inFlight = true;
    dirty = false;
    const ht = TIERS.find(t => t >= view.z) ?? 32;
    const s = view.z / ht;
    const padX = Math.ceil(canvas.width / s / 4);
    const padY = Math.ceil(canvas.height / s / 4);
    const ox = view.cx * ht - canvas.width / (2 * s);
    const oy = view.cy * ht - canvas.height / (2 * s);
    try {
      const m = await call({
        type: "render",
        turns: view.turns,
        ht,
        x: Math.floor(ox) - padX,
        y: Math.floor(oy) - padY,
        w: Math.ceil(canvas.width / s) + 2 * padX + 1,
        h: Math.ceil(canvas.height / s) + 2 * padY + 1,
      });
      if (m.empty) {
        dropFrames();
      } else if (m.turns === view.turns) {
        prev?.bmp.close();
        prev = cache;
        cache = { ht: m.ht, ix: m.ix, iy: m.iy, vw: m.vw, vh: m.vh, bmp: m.bmp };
        status(`${blocks.toLocaleString()} blocks - ${SIDE_LABELS[view.turns]}, zoom ${(view.z / MAX_Z * 100).toFixed(0)}%, tile ${m.ht * 2}px in ${m.ms.toFixed(0)}ms`);
      } else {
        m.bmp?.close();
      }
    } catch (e) {
      status(e.message === "unreachable" ? "render ran out of memory - try zooming in" : e.message);
      return;
    } finally {
      inFlight = false;
    }
    scheduleComposite();
    if (dirty) requestRender();
  }

  function viewChanged() {
    scheduleComposite();
    requestRender();
  }

  function zoomTo(z, cx, cy) {
    z = Math.min(MAX_Z, Math.max(minZ(), z));
    if (z === view.z) return;
    const dx = cx - canvas.width / 2;
    const dy = cy - canvas.height / 2;
    view.cx += dx / view.z - dx / z;
    view.cy += dy / view.z - dy / z;
    view.z = z;
    viewChanged();
  }

  let drag = null;
  canvas.addEventListener("pointerdown", e => {
    drag = { px: e.clientX, py: e.clientY, cx: view.cx, cy: view.cy };
    canvas.setPointerCapture(e.pointerId);
    canvas.classList.add("drag");
  });
  canvas.addEventListener("pointermove", e => {
    if (!drag) return;
    const dpr = devicePixelRatio || 1;
    view.cx = drag.cx - (e.clientX - drag.px) * dpr / view.z;
    view.cy = drag.cy - (e.clientY - drag.py) * dpr / view.z;
    viewChanged();
  });
  canvas.addEventListener("pointerup", () => { drag = null; canvas.classList.remove("drag"); });
  canvas.addEventListener("wheel", e => {
    e.preventDefault();
    const dy = e.deltaY * (e.deltaMode === 1 ? 40 : 1);
    const factor = Math.exp(-dy * (e.ctrlKey ? 0.01 : 0.002));
    const dpr = devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    zoomTo(view.z * factor, (e.clientX - rect.left) * dpr, (e.clientY - rect.top) * dpr);
  }, { passive: false });

  let resizeTimer;
  addEventListener("resize", () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      if (!blocks) return;
      sizeCanvas();
      composite();
      requestRender();
    }, 150);
  });

  let busy = false;
  async function load(fn) {
    if (busy) return;
    busy = true;
    try {
      blocks = 0;
      dropFrames();
      dropBases();
      const m = await fn(call, status);
      blocks = m.blocks;
      if (!blocks) return;
      canvas.style.display = "block";
      sizeCanvas();
      await fitView();
      requestRender();
      requestBase();
    } catch (e) {
      status(e.message);
    } finally {
      busy = false;
    }
  }

  function loadWorld(regions) {
    return load(async () => {
      status("building surface...");
      return call({ type: "world", regions }, regions.map(r => r.buf));
    });
  }

  function loadSurface(buf) {
    return load(async () => call({ type: "surface", buf }, [buf]));
  }

  return { ready, status, loadWorld, loadSurface };
}
