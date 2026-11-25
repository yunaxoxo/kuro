# kuro

A small statically typed math and algorithms language that compiles straight to WebAssembly bytecode, with no LLVM and no `wasm-encoder`. Every byte of the module is written by this crate. A browser workbench runs the compiler and lets you step through the program's linear memory.

```
fn main() -> int {
    let a: int[] = alloc(3);
    a[2] = 9;
    return a[2] + len(a);      // 12
}
```

**Status:** the shared types, the pipeline glue, the test suite and the workbench are complete. The compiler stages (lexer, parser, type checker, code generator) are typed stubs ending in `todo!()` while they are implemented. Until they are, the workbench replays recorded runs of the sample programs and says so.

## Architecture

```
                         compiler/ (Rust, cdylib + rlib)
 source ──lexer──▶ tokens ──parser──▶ AST ──typeck──▶ typed AST ──codegen + wasm_emit──▶ .wasm bytes
   │                                                                                         │
   │            compile_trace(source) → ExecutionTrace JSON (tokens, ast, wasm_bytes)         │
   ▼                                                                                         ▼
 web/ (Vite + React + TypeScript)                                              Web Worker runs the module,
   Editor ─▶ trace/backend.ts ─▶ Rust wasm build ─────────────────────────────▶ snapshots linear memory
   Memory · Tokens & AST · WASM Bytecode views ◀────────── ExecutionTrace ◀──── (trace/worker.ts, trace/live.ts)
```

| Path | What it is |
|---|---|
| `compiler/src/{lexer,parser,typeck,codegen,wasm_emit}.rs` | The compiler stages (in progress). |
| `compiler/src/{token,ast,error,trace,lib}.rs` | Shared types, the pipeline glue and the `wasm-bindgen` API. |
| `compiler/tests/` | 139 tests: lexer, parser, type checker, codegen (binary structure), end-to-end (executed under `wasmtime`). |
| `examples/*.kr` | The three sample programs, used by the tests and the workbench. |
| `web/` | The workbench. Every design token is a CSS custom property in `web/src/index.css`. |

## Getting started

Requirements: a Rust toolchain for the compiler, Node.js for the workbench, and (for the live compiler in the browser) the `wasm32-unknown-unknown` target and [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```sh
# Compiler
cargo test --manifest-path compiler/Cargo.toml --no-fail-fast
cargo check --manifest-path compiler/Cargo.toml

# Workbench
cd web
npm install
npm run dev          # http://localhost:5173
npm run build        # type-check and production build into web/dist
npm run wasm         # build the Rust compiler for the browser into web/public/pkg
```

## The workbench

Open a sample (Bubble Sort, Array Sum, Fibonacci) and press **Compile & Run** (`⌘↵` / `Ctrl↵`). Three views share one step cursor:

- **Memory Visualizer:** the program's linear memory as labelled Int32 cells, one snapshot per print call. Written cells turn vermilion; the scrubber, `←`/`→` and `Space` step and play.
- **Tokens & AST:** the token stream and the syntax tree, filterable, with click-to-source into the editor.
- **WASM Bytecode:** the module's sections, a hex dump and the disassembled instructions, linked byte for byte.

The sample, the view and the filter live in the URL, so any state can be shared or reloaded. The header shows which of two modes produced what you see:

1. **Recorded Run** (until the compiler stages work). `web/src/trace/fixtures/*.json` are recorded traces of the three samples, labelled `mode: "mock"`. Edits are not compiled while in this mode, and the workbench says so.
2. **Live WASM.** After `npm run wasm`, **Compile & Run** loads `web/public/pkg/kuro.js`, calls the compiler, runs the module it emitted and renders the result with `mode: "live"`.

## Compiler ↔ workbench contract

| Piece | Where | Contract |
|---|---|---|
| `compile_trace(source) → string` | `compiler/src/lib.rs` (`wasm_api`) | JSON `ExecutionTrace` with the compile-time half filled: `tokens`, `ast_json`, `wasm_bytes`, `diagnostics` (at most one error). `steps` and `stdout` are empty. |
| `compile_json(source, up_to) → string` | same | The pipeline output up to `"lex"`, `"parse"`, `"typecheck"` or `"codegen"`, with every artifact produced before a failure still present. |
| `take_last_panic() → string \| undefined` | same | The message of the last Rust panic. A `todo!()` reaches JavaScript as a thrown `WebAssembly.RuntimeError`; the workbench reads this message, shows which stage is unfinished and falls back to the recorded trace. |
| Spans | `compiler/src/token.rs` → `web/src/lib/ast.ts` | `start`/`end` are UTF-8 byte offsets, `line`/`col` are 1-based. The workbench converts byte offsets to string offsets once, when it builds the tree; `line`/`col` cross unchanged. |
| Host imports | `web/src/trace/worker.ts` | `env.print_int(i32)` and `env.print_float(f64)`. |
| Memory snapshots | the same worker | One snapshot before `main`, at each print call, and after `main` returns or traps. Cell *i* is the little-endian `i32` at byte `4i`, up to the heap pointer. A run is stopped after 3 seconds. |
| TypeScript mirror | `web/src/trace/types.ts` | Field for field the same as `compiler/src/trace.rs`; changing one means changing the other. |
