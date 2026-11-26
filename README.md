# kuro

**A deterministic WebAssembly compiler and memory-forensics workbench, written in Rust.**

kuro is a small statically typed language whose Rust compiler emits WebAssembly MVP bytecode by hand: no LLVM, no `wasm-encoder`, every byte written by the crate itself. A browser workbench compiles and runs a program, then lets you step through its linear memory, tokens, syntax tree and module bytes.

![The kuro workbench mid-sort: source on the left, linear memory on the right as labelled Int32 cells, the two cells just swapped in vermilion](docs/workbench.webp)

```
source (.kr) ─▶ lexer ─▶ parser ─▶ type checker ─▶ codegen ─▶ wasm_emit ─▶ .wasm ─▶ workbench
                tokens   recursive   two passes:     stack      LEB128,               run in a
                + spans  descent,    signatures,     machine,   sections              Web Worker,
                         precedence  then bodies     linear                           memory snapshot
                         climbing                    memory                           at every print
```

**Status.** The shared types, pipeline glue, 139 tests and the workbench are complete. The five compiler stages are typed `todo!()` stubs while they are implemented; until then the workbench replays recorded runs of the sample programs and says so. The forensics profile below is specified and not built yet.

## Commands

```sh
cargo test  --manifest-path compiler/Cargo.toml --no-fail-fast
cargo check --manifest-path compiler/Cargo.toml

cd web
npm install
npm run dev      # http://localhost:5173
npm run build    # type-check, then build into web/dist
npm run wasm     # compile the Rust compiler for the browser (wasm32-unknown-unknown + wasm-pack)
```

## Repository

```
compiler/src/   lexer.rs · parser.rs · typeck.rs · codegen.rs · wasm_emit.rs    the five stages
                token.rs · ast.rs · error.rs · trace.rs · lib.rs              types, glue, wasm-bindgen API
compiler/tests/ lexer · parser · type checker · codegen · end to end (under wasmtime)
examples/       bubble_sort.kr · array_sum.kr · fibonacci.kr
web/src/        components/ · trace/ (compile, run, worker, recorded runs) · styles/ · index.css (tokens)
docs/           README assets
```

## Language

```rust
fn sum(a: int[]) -> int {
    let total = 0;
    let i = 0;
    while i < len(a) {
        total = total + a[i];
        i = i + 1;
    }
    return total;
}
```

| | |
|---|---|
| Types | `int` (i32) · `float` (f64) · `bool` · `int[]` (an address into linear memory). No implicit conversions. |
| Operators, loosest first | `\|\|` · `&&` · `==` `!=` · `<` `<=` `>` `>=` · `+` `-` · `*` `/` `%` · unary `-` `!` · `a[i]`. All binary operators are left-associative. |
| Builtins | `print_int(int)` · `print_float(float)` · `alloc(n) -> int[]` · `len(int[]) -> int` |
| Statements | `let` (always initialised) · assignment · `if` / `else if` / `else` · `while` · `return`. Braces are mandatory. |
| Semantics | Integer arithmetic wraps; division truncates toward zero and traps on zero. `&&` and `\|\|` short-circuit, left to right. Arrays are references. Indexing is unchecked. |

## Linear memory

One zeroed 64 KiB page. Arrays are bump-allocated and never freed; each is a length header followed by its elements, all little-endian `i32`.

| Address | Cell | Meaning |
|---|---|---|
| `0x0000` | `PTR` | heap pointer: the next free byte (0 until the first `alloc`) |
| `0x0004` | `RSV` | reserved, always 0 |
| `0x0008` | `LEN` | the first array's length |
| `0x000C` | `[0]` | its first element; `[i]` is at `ptr + 4 + 4i` |

The module always has six sections (Type, Import, Function, Memory, Export, Code) and fixed function indices: `0` `print_int`, `1` `print_float`, `2` `alloc`, then user functions in source order.

## Compile & Run

```mermaid
sequenceDiagram
    actor You
    participant UI as Workbench
    participant C as Compiler (Rust → wasm)
    participant W as Web Worker

    You->>UI: Compile & Run (⌘↵)
    alt the compiler is built and every stage works
        UI->>C: compile_trace(source)
        C-->>UI: tokens, AST, module bytes
        UI->>W: run the module (3 s limit)
        W-->>UI: stdout and a memory snapshot at each print
        UI-->>You: Live WASM
    else a stage still hits todo!()
        UI->>C: compile_trace(source)
        C--xUI: panic
        UI->>C: take_last_panic()
        C-->>UI: which stage is unfinished
        UI-->>You: Recorded Run, naming the unfinished stage
    else the compiler is not built yet
        UI-->>You: Recorded Run of the sample, labelled as such
    end
```

## Workbench

| View | Shows |
|---|---|
| Memory Visualizer | Linear memory as labelled Int32 cells, one snapshot per print. The cells a step writes turn vermilion; each step is narrated. |
| Tokens & AST | The token stream and syntax tree, filterable; click any row to jump to its source. |
| WASM Bytecode | Sections, a hex dump and the disassembly, linked byte for byte. |

`⌘↵` run · `←` `→` step (`Shift` ×5) · `Space` play · `1` `2` `3` views · `⌘F` filter · `Esc` leave the editor. State lives in the URL. It works offline, honours reduced motion, is keyboard-operable throughout and adapts from phones to wide screens.

## Forensics profile (planned)

kuro programs have no clock, no randomness and no input, and they store only `i32` in memory. So a run is fully deterministic, and that makes four tools cheap to build. All four are opt-in. With the default options, the compiler's output stays byte-for-byte the core contract below, so the tests, recorded runs and workbench are unaffected.

| Capability | What you get |
|---|---|
| Time-travel event log | An instrumented build reports every store, call, return and branch to one host import. The NDJSON log rebuilds memory and the call stack at any step, forwards or backwards, and answers "which line last wrote this cell?" |
| Gas counter | Exact per-line counts of ALU ops, loads, stores, branches and calls, with a fixed cost model. These are operation counts, not time, so two implementations compare by the work they do. |
| Headless CLI | `kuro build`, `kuro check`, `kuro run`, `kuro disasm` and `kuro bench`, behind a `cli` cargo feature, with editor-friendly diagnostics, stable exit codes and byte-identical output. |
| Heap layout 2 | Arrays carry `cap` and `len` and gain `push`, `pop` and `slice`. `int[,]` tables come from `grid`. Every index is bounds-checked, provably bad literals are rejected at compile time, and any other out-of-range index traps with its source line. |

### CLI & Unix pipeline composition (planned)

```sh
kuro run sort.kr                                   # stdout: the program's prints, nothing else
cat sort.kr | kuro run - --json | jq -e '.status == "ok"'          # stdin in, exit status out
kuro run sort.kr --json --profile | jq '.gas.lines | sort_by(-.gas) | .[:3]'   # hottest lines
kuro run cases/*.kr --json | jq -c 'select(.status != "ok") | {source, trap}'  # NDJSON batch
kuro run sort.kr --who-wrote 0x0014                # last write to a cell: line, step, old → new
kuro bench bubble.kr insertion.kr --n 16,64,256    # counts side by side, empirical growth
kuro build sort.kr --wat && wasm-tools validate sort.wasm   # .wasm plus annotated .wat
wasmtime run --preload env=env.wasm --invoke main sort.wasm # core module under plain wasmtime
```

With `--json`, every input produces exactly one document. Every key is always present, and floats are written as strings, so documents compare byte for byte. Exit codes: `0` ok, `1` compile error, `2` trap, `3` fuel or event limit. `--format quickfix` prints one `file:line:col: error: message` line per problem, so Neovim's `:make` and Zed's terminal jump straight to the error. The `wasmtime` line needs a small hand-written `env` shim. A module imports at most three host functions: `env.print_int`, `env.print_float` and, in instrumented builds, `kuro.event`. The browser, Node and `kuro run` all print values identically.

## Compiler ↔ workbench contract

| Piece | Where | Contract |
|---|---|---|
| `compile_trace(source)` | `compiler/src/lib.rs` | JSON `ExecutionTrace`: `tokens`, `ast_json`, `wasm_bytes`, at most one diagnostic. `steps` and `stdout` stay empty. |
| `compile_json(source, up_to)` | same | Output up to `lex`, `parse`, `typecheck` or `codegen`, keeping every artifact made before a failure. |
| `take_last_panic()` | same | The last Rust panic's message; a `todo!()` names the unfinished stage. |
| Spans | `token.rs` → `web/src/lib/ast.ts` | UTF-8 byte offsets; `line`/`col` 1-based. |
| Host imports | `web/src/trace/worker.ts` | `env.print_int(i32)`, `env.print_float(f64)`. |
| Snapshots | same | Before `main`, at each print, after return or trap; cell *i* is the `i32` at byte `4i`. |
| Types | `web/src/trace/types.ts` | Mirrors `compiler/src/trace.rs` field for field. |
