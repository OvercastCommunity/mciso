const dec = new TextDecoder();
let wasm;
let memory;
const threadWorkers = [];
let nextWorker = 0;
const imports = { env: {
  mciso_console: (ptr, len) => {
    postMessage({ type: "log", msg: dec.decode(new Uint8Array(memory.buffer, ptr >>> 0, len).slice()) });
  },
  mciso_spawn: thread => {
    const stackSize = 2 << 20;
    const stackTop = ((wasm.mciso_alloc(stackSize + 16) >>> 0) + stackSize + 16) & ~15;
    const tlsAlign = wasm.__tls_align.value;
    const tlsPtr = ((wasm.mciso_alloc(wasm.__tls_size.value + tlsAlign) >>> 0) + tlsAlign - 1) & -tlsAlign;
    threadWorkers[nextWorker++].postMessage({ thread, stackTop, tlsPtr });
  },
}};

function push(bytes) {
  const ptr = wasm.mciso_alloc(bytes.length) >>> 0;
  new Uint8Array(memory.buffer).set(bytes, ptr);
  return ptr;
}

const ready = (async () => {
  if (!self.crossOriginIsolated) {
    throw new Error("SharedArrayBuffer unavailable - serve with COOP/COEP headers: python3 demo/serve.py");
  }
  const assetsPromise = fetch("assets.bin").then(r => {
    if (!r.ok) throw new Error("assets.bin missing - run: cargo run --release --bin bundle");
    return r.arrayBuffer();
  });
  memory = new WebAssembly.Memory({ initial: 1024, maximum: 65536, shared: true });
  imports.env.memory = memory;
  const module = await WebAssembly.compileStreaming(fetch("mciso.wasm"))
    .catch(async () => WebAssembly.compile(await (await fetch("mciso.wasm")).arrayBuffer()));
  const instance = await WebAssembly.instantiate(module, imports);
  wasm = instance.exports;
  const threads = Math.min(navigator.hardwareConcurrency || 4, 8);
  await Promise.all(Array.from({ length: threads - 1 }, () => new Promise((res, rej) => {
    const w = new Worker("thread.js");
    threadWorkers.push(w);
    w.addEventListener("message", e => {
      if (e.data.type === "booted") res();
      else if (e.data.type === "log") postMessage(e.data);
    });
    w.addEventListener("error", e => rej(new Error(e.message || "thread worker failed")));
    w.postMessage({ module, memory });
  })));
  if (!wasm.mciso_pool(threads)) throw new Error("could not start the WebAssembly worker pool");
  const assets = new Uint8Array(await assetsPromise);
  const n = wasm.mciso_init(push(assets), assets.length);
  if (!n) throw new Error("assets.bin is invalid or incompatible - rebuild it with: cargo run --release --bin bundle");
  return { assets: n, threads };
})();

onmessage = async e => {
  const m = e.data;
  try {
    const info = await ready;
    if (m.type === "init") {
      postMessage({ id: m.id, assets: info.assets, threads: info.threads });
    } else if (m.type === "world") {
      wasm.mciso_clear();
      for (const r of m.regions) {
        const bytes = new Uint8Array(r.buf);
        wasm.mciso_add_region(r.rx, r.rz, push(bytes), bytes.length);
      }
      postMessage({ id: m.id, blocks: wasm.mciso_build() });
    } else if (m.type === "surface") {
      wasm.mciso_clear();
      const bytes = new Uint8Array(m.buf);
      postMessage({ id: m.id, blocks: wasm.mciso_load_surface(push(bytes), bytes.length) });
    } else if (m.type === "size") {
      const s = wasm.mciso_view_size(m.turns, 16);
      postMessage({ id: m.id, nw: Number(s >> 32n) / 16, nh: Number(s & 0xffffffffn) / 16 });
    } else if (m.type === "render") {
      const t0 = performance.now();
      const s = wasm.mciso_view_size(m.turns, m.ht);
      const iw = Number(s >> 32n);
      const ih = Number(s & 0xffffffffn);
      const x1 = Math.min(m.x + m.w, iw);
      const y1 = Math.min(m.y + m.h, ih);
      const ix = Math.max(m.x, 0);
      const iy = Math.max(m.y, 0);
      let vw = Math.min(x1 - ix, 8192);
      let vh = Math.min(y1 - iy, 8192);
      const cap = 32 << 20;
      if (vw * vh > cap) {
        const f = Math.sqrt(cap / (vw * vh));
        vw = Math.floor(vw * f);
        vh = Math.floor(vh * f);
      }
      if (vw <= 0 || vh <= 0) {
        postMessage({ id: m.id, empty: true });
        return;
      }
      const len = wasm.mciso_render_viewport(m.turns, m.ht, ix, iy, vw, vh);
      if (len !== vw * vh * 4) {
        postMessage({ id: m.id, empty: true });
        return;
      }
      const data = new Uint8ClampedArray(memory.buffer, wasm.mciso_result_ptr() >>> 0, len).slice();
      const bmp = await createImageBitmap(new ImageData(data, vw, vh));
      const ms = performance.now() - t0;
      postMessage({ id: m.id, turns: m.turns, ht: m.ht, ix, iy, vw, vh, bmp, ms }, [bmp]);
    }
  } catch (err) {
    postMessage({ id: m.id, error: err.message });
  }
};
