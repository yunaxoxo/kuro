import { runModule } from "./live";
import { sampleMatching, SAMPLES, type Sample } from "./samples";
import type { Diagnostic, ExecutionTrace, RunOutcome } from "./types";

/** The module `wasm-pack build --target web` generates (see compiler/src/lib.rs `wasm_api`). */
interface RustPkg {
  default(init: { module_or_path: string }): Promise<unknown>;
  compile_trace(source: string): string;
  take_last_panic(): string | undefined;
}

const PKG_JS = "/pkg/kuro.js";
const PKG_WASM = "/pkg/kuro_bg.wasm";

/**
 * Load the Rust compiler if `npm run wasm` has been run. A fresh instance is
 * created per call: a Rust panic aborts the instance, so reusing one after a
 * `todo!()` would be unreliable.
 */
async function loadRust(): Promise<RustPkg | null> {
  try {
    const head = await fetch(PKG_WASM, { method: "HEAD", cache: "no-store" });
    const type = head.headers.get("content-type") ?? "";
    // Vite answers unknown paths with index.html, so check the type, not just `ok`.
    if (!head.ok || !type.includes("wasm")) return null;
    const stamp = `${head.headers.get("last-modified") ?? ""}${head.headers.get("content-length") ?? ""}`;
    const bust = encodeURIComponent(stamp || String(Date.now()));
    const pkg = (await import(/* @vite-ignore */ `${PKG_JS}?v=${bust}`)) as RustPkg;
    await pkg.default({ module_or_path: `${PKG_WASM}?v=${bust}` });
    return pkg;
  } catch (err) {
    console.warn("Could not load the Rust compiler build:", err);
    return null;
  }
}

/** "panicked at src/lexer.rs:12:9:\nnot yet implemented: lexer: Lexer::new" → "lexer: Lexer::new" */
function describePanic(raw: string | undefined): string {
  const message = raw ?? "the compiler trapped";
  const todo = /not yet implemented:?\s*(.*)/s.exec(message);
  if (todo) return `${todo[1].trim().split("\n")[0] || "todo!()"} is still a todo!()`;
  return message.replace(/^.*panicked at [^\n]*\n?/s, "").trim() || message;
}

/** The recorded trace for `sample`, flagged as mock; a warning explains edited source. */
function fixtureTrace(sample: Sample, edited: boolean): ExecutionTrace {
  const diagnostics: Diagnostic[] = edited
    ? [
        {
          line: 1,
          col: 1,
          severity: "warning",
          message: `Your edits were not compiled, so errors in them won't show. Showing the recorded trace of “${sample.title}” until the Rust compiler runs.`,
        },
      ]
    : [];
  return { ...sample.fixture, mode: "mock", diagnostics };
}

/**
 * Compile and run `source`. The Rust compiler is used when it is built and
 * does not panic; otherwise the bundled fixture trace of `fallback` (or of the
 * sample the source matches) is returned with `mode: "mock"`.
 */
export async function compileAndRun(source: string, fallback: Sample = SAMPLES[0]): Promise<RunOutcome> {
  const matched = sampleMatching(source);
  const mock = (note: string): RunOutcome => ({ trace: fixtureTrace(matched ?? fallback, !matched), note });

  const rust = await loadRust();
  if (!rust) return mock(import.meta.env.DEV ? "compiler not built (npm run wasm)" : "the live compiler is not bundled in this build");

  let compiled: ExecutionTrace;
  try {
    compiled = JSON.parse(rust.compile_trace(source)) as ExecutionTrace;
  } catch (err) {
    const reason = describePanic(rust.take_last_panic() ?? (err instanceof Error ? err.message : undefined));
    return mock(reason);
  }

  if (compiled.diagnostics.length > 0 || compiled.wasm_bytes.length === 0) {
    return { trace: { ...compiled, mode: "live" }, note: null };
  }

  const run = await runModule(Uint8Array.from(compiled.wasm_bytes));
  const diagnostics: Diagnostic[] = [...compiled.diagnostics];
  if (run.trap) diagnostics.push({ line: 1, col: 1, severity: "error", message: `The program trapped at run time: ${run.trap}` });
  if (run.timedOut) diagnostics.push({ line: 1, col: 1, severity: "error", message: "The program ran for more than 3 seconds and was stopped." });
  return { trace: { ...compiled, mode: "live", stdout: run.stdout, steps: run.steps, diagnostics }, note: null };
}
