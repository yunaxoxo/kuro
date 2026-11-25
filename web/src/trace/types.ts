/**
 * The execution-trace contract between the Rust compiler / WASM runner and the
 * workbench. Mirrors `compiler/src/trace.rs` field for field; changing one
 * means changing the other.
 */

export type TraceMode = "live" | "mock";

export interface Diagnostic {
  line: number;
  col: number;
  severity: "error" | "warning";
  message: string;
}

export interface TraceToken {
  /** Variant name of the token kind, e.g. "Ident", "Arrow". */
  kind: string;
  /** Exact source text of the token. */
  lexeme: string;
  line: number;
  col: number;
}

export interface TraceStep {
  step_index: number;
  /** Source line (1-based) the step is attributed to; 0 when unknown. */
  pc: number;
  op: string;
  /** One plain-English sentence describing what changed. */
  narration: string;
  /** Used prefix of linear memory as little-endian i32 cells. */
  memory_i32: number[];
  /** Indices into `memory_i32` whose value differs from the previous step. */
  mutated_indices: number[];
}

export interface ExecutionTrace {
  mode: TraceMode;
  stdout: string;
  diagnostics: Diagnostic[];
  tokens: TraceToken[];
  ast_json: unknown;
  wasm_bytes: number[];
  steps: TraceStep[];
}

/** Why the workbench shows what it shows. */
export interface RunOutcome {
  trace: ExecutionTrace;
  /** One plain sentence explaining the mode; null when everything ran live. */
  note: string | null;
}
