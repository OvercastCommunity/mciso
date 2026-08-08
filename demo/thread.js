const dec = new TextDecoder();
let instance;
let memory;

onmessage = async e => {
  if (e.data.module) {
    ({ memory } = e.data);
    instance = await WebAssembly.instantiate(e.data.module, { env: {
      memory,
      mciso_console: (p, l) => postMessage({ type: "log", msg: dec.decode(new Uint8Array(memory.buffer, p >>> 0, l).slice()) }),
      mciso_spawn: () => {},
    }});
    postMessage({ type: "booted" });
  } else {
    const { thread, stackTop, tlsPtr } = e.data;
    instance.exports.__stack_pointer.value = stackTop;
    instance.exports.__wasm_init_tls(tlsPtr);
    instance.exports.mciso_worker_entry(thread);
  }
};
