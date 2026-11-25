import type { ExecutionTrace } from "./types";
import fibonacci from "./fixtures/fibonacci.json";
import arraySum from "./fixtures/array_sum.json";
import bubbleSort from "./fixtures/bubble_sort.json";
import fibonacciSrc from "../../../examples/fibonacci.kr?raw";
import arraySumSrc from "../../../examples/array_sum.kr?raw";
import bubbleSortSrc from "../../../examples/bubble_sort.kr?raw";

export interface Sample {
  id: string;
  number: string;
  title: string;
  source: string;
  /** Recorded trace, used whenever the Rust backend cannot run. */
  fixture: ExecutionTrace;
}

export const SAMPLES: Sample[] = [
  // Ordered so the first tab writes memory: the program that never allocates comes last.
  { id: "bubble_sort", number: "01", title: "Bubble Sort", source: bubbleSortSrc, fixture: bubbleSort as ExecutionTrace },
  { id: "array_sum", number: "02", title: "Array Sum", source: arraySumSrc, fixture: arraySum as ExecutionTrace },
  { id: "fibonacci", number: "03", title: "Fibonacci", source: fibonacciSrc, fixture: fibonacci as ExecutionTrace },
];

const DEFAULT_SAMPLE = SAMPLES[0];

const normalise = (text: string) => text.replace(/\r\n/g, "\n").trimEnd();

export function sampleMatching(source: string): Sample | undefined {
  const wanted = normalise(source);
  return SAMPLES.find((sample) => normalise(sample.source) === wanted);
}

type ViewId = "memory" | "tokens" | "bytecode";
const VIEWS: ViewId[] = ["memory", "tokens", "bytecode"];

/** Shareable UI state read from the address bar: `?sample=array_sum&view=tokens&q=while`. */
export function readUrlState(): { sample: Sample; view: ViewId; filter: string } {
  const params = new URLSearchParams(window.location.search);
  const view = params.get("view");
  return {
    sample: SAMPLES.find((s) => s.id === params.get("sample")) ?? DEFAULT_SAMPLE,
    view: VIEWS.find((v) => v === view) ?? "memory",
    filter: params.get("q") ?? "",
  };
}
