import type { TraceStep } from "../trace/types";

/** 0x0000 to 0x0008: heap pointer, reserved word, first array length. */
export const RESERVED_CELLS = 3;

/** A few words that say what a step did, e.g. `Swap [1] ↔ [2]`. */
export function stepLabel(step: TraceStep): string {
  const data = step.mutated_indices.filter((i) => i >= RESERVED_CELLS).map((i) => i - RESERVED_CELLS);
  if (data.length === 2) return `Swap [${data[0]}] ↔ [${data[1]}]`;
  if (data.length === 1) return `Store [${data[0]}] = ${step.memory_i32[data[0] + RESERVED_CELLS]}`;
  if (data.length > 2) return `Write ×${data.length}`;
  const alloc = /^call alloc\((.*)\)$/.exec(step.op);
  if (alloc) return `Alloc ${alloc[1]}`;
  const print = /^call print\w*\((.*)\)$/.exec(step.op);
  if (print) return `Print ${print[1]}`;
  const ret = /^return(?: (.*))?$/.exec(step.op);
  if (ret) return ret[1] ? `Return ${ret[1]}` : "Return";
  if (step.op === "call main") return "Start";
  return step.op;
}
