import type { WorkerMessage } from "./protocol";
import type { TraceStep } from "./types";

const hex = (cell: number) => `0x${(cell * 4).toString(16).toUpperCase().padStart(4, "0")}`;

export interface LiveRun {
  stdout: string;
  steps: TraceStep[];
  trap: string | null;
  timedOut: boolean;
  dropped: number;
}

/** Execute `bytes` in a worker (killed after `timeoutMs`) and turn what the host saw into trace steps. */
export function runModule(bytes: Uint8Array, timeoutMs = 3000): Promise<LiveRun> {
  return new Promise((resolve) => {
    const worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
    const printed: string[] = [];
    const snapshots: { label: string; cells: number[] }[] = [];
    let trap: string | null = null;
    let timedOut = false;
    let dropped = 0;

    const finish = () => {
      clearTimeout(timer);
      worker.terminate();
      resolve({ stdout: printed.join("\n"), steps: toSteps(snapshots), trap, timedOut, dropped });
    };
    const timer = setTimeout(() => {
      timedOut = true;
      finish();
    }, timeoutMs);

    worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
      const message = event.data;
      if (message.type === "print") printed.push(message.text);
      else if (message.type === "snapshot") snapshots.push({ label: message.label, cells: message.cells });
      else {
        trap = message.trap;
        dropped = message.dropped;
        finish();
      }
    };
    worker.onerror = (event) => {
      trap = `worker error: ${event.message}`;
      finish();
    };
    worker.postMessage({ bytes });
  });
}

function toSteps(snapshots: { label: string; cells: number[] }[]): TraceStep[] {
  let previous: number[] = [];
  return snapshots.map((snap, index) => {
    const mutated: number[] = [];
    const length = Math.max(snap.cells.length, previous.length);
    for (let i = 0; i < length; i++) if ((snap.cells[i] ?? 0) !== (previous[i] ?? 0)) mutated.push(i);
    previous = snap.cells;
    const narration =
      mutated.length === 0
        ? `${describe(snap.label)} Linear memory is unchanged.`
        : `${describe(snap.label)} Linear memory changed at ${mutated
            .slice(0, 4)
            .map(hex)
            .join(", ")}${mutated.length > 4 ? ` and ${mutated.length - 4} more` : ""}.`;
    return { step_index: index, pc: 0, op: snap.label, narration, memory_i32: snap.cells, mutated_indices: mutated };
  });
}

function describe(label: string): string {
  if (label === "call main") return "main starts.";
  if (label === "trap") return "The program trapped.";
  if (label.startsWith("return")) return label === "return" ? "main returned." : `main returned ${label.slice(7)}.`;
  const printed = /^call print\((.*)\)$/.exec(label);
  return printed ? `Printed ${printed[1]}.` : `${label}.`;
}
