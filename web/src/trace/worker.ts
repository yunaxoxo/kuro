/// <reference lib="webworker" />
import type { WorkerMessage } from "./protocol";

/**
 * Runs a compiled module off the main thread so a non-terminating `while` can
 * be stopped by terminating the worker. Memory is captured as `i32` cells at
 * the moments a host can observe: before `main`, at every `print_*` call, and
 * after `main` returns or traps.
 */

const MAX_SNAPSHOTS = 400;
const post = (message: WorkerMessage) => (self as DedicatedWorkerGlobalScope).postMessage(message);

self.onmessage = async (event: MessageEvent<{ bytes: Uint8Array }>) => {
  let memory: WebAssembly.Memory | null = null;
  let taken = 0;
  let dropped = 0;

  const snapshot = (label: string, force = false) => {
    if (!memory) return;
    if (!force && taken >= MAX_SNAPSHOTS) {
      dropped++;
      return;
    }
    const view = new DataView(memory.buffer);
    const heap = Math.max(view.getInt32(0, true), 0);
    const cells = Math.max(2, Math.min(memory.buffer.byteLength, heap) >> 2);
    taken++;
    post({ type: "snapshot", label, cells: Array.from(new Int32Array(memory.buffer, 0, cells)) });
  };

  const print = (text: string) => {
    post({ type: "print", text });
    snapshot(`call print(${text})`);
  };

  try {
    const { instance } = await WebAssembly.instantiate(event.data.bytes as BufferSource, {
      env: {
        print_int: (value: number) => print(String(value)),
        print_float: (value: number) => print(String(value)),
      },
    });
    memory = instance.exports.memory as WebAssembly.Memory;
    const main = instance.exports.main as (() => number | undefined) | undefined;
    if (!main) throw new Error("the module does not export `main`");

    snapshot("call main", true);
    let returned: number | null = null;
    let trap: string | null = null;
    try {
      const result = main();
      returned = typeof result === "number" ? result : null;
    } catch (err) {
      trap = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
    }
    snapshot(trap ? "trap" : returned === null ? "return" : `return ${returned}`, true);
    post({ type: "done", returned, trap, dropped });
  } catch (err) {
    post({ type: "done", returned: null, trap: `could not instantiate: ${err instanceof Error ? err.message : String(err)}`, dropped });
  }
};
