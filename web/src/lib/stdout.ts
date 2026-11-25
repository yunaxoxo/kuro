import type { ExecutionTrace } from "../trace/types";

/** Every line the program printed, in order. */
export function stdoutLines(trace: ExecutionTrace): string[] {
  return trace.stdout === "" ? [] : trace.stdout.split("\n");
}

/** How many of those lines had been printed once step `index` has run. */
export function printedThrough(trace: ExecutionTrace, index: number): number {
  if (trace.steps.length === 0) return stdoutLines(trace).length;
  let count = 0;
  for (let i = 0; i <= index && i < trace.steps.length; i++) if (/^call print\w*\(/.test(trace.steps[i].op)) count++;
  return count;
}
