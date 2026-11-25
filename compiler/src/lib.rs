//! # kuro compiler
//!
//! Source text in, a WebAssembly binary out. No LLVM, no `wasm-encoder`:
//! every byte of the module is written by this crate.
//!
//! ```text
//!   &str ──lexer──▶ Vec<Token> ──parser──▶ Program ──typeck──▶ Program ──codegen──▶ Vec<u8>
//!                                          (ty: None)          (ty: Some)  └─wasm_emit─┘ (.wasm)
//! ```
//!
//! | Module        | Status                | Contents                                      |
//! |---------------|-----------------------|-----------------------------------------------|
//! | [`token`]     | complete (1 `todo!`)  | `Span`, `TokenKind`, `Token`                  |
//! | [`ast`]       | complete              | `Program`, `Function`, `Stmt`, `Expr`, ...    |
//! | [`error`]     | complete              | `CompileError`, `ErrorKind`, `Stage`          |
//! | [`trace`]     | complete              | execution-trace types shared with the UI      |
//! | [`lexer`]     | `todo!()`             | source text → tokens                          |
//! | [`parser`]    | `todo!()`             | tokens → AST                                  |
//! | [`typeck`]    | `todo!()`             | static semantics, `ty` annotations            |
//! | [`codegen`]   | `todo!()`             | AST → module bytes                            |
//! | [`wasm_emit`] | `todo!()`             | LEB128, names, sections; opcode constants     |
//!
//! This file is the glue and is complete: it chains the stages, records
//! partial results for the playground, and exposes the compiler to JavaScript
//! through `wasm-bindgen`. A `todo!()` panic inside a stage surfaces in the
//! playground as a thrown `WebAssembly.RuntimeError`, which switches it to its
//! bundled fixture traces.

pub mod ast;
pub mod codegen;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod trace;
pub mod typeck;
pub mod wasm_emit;

pub use error::{CompileError, ErrorKind, Result, Stage};

use serde::Serialize;

/// Everything the pipeline produced, including partial results when a stage
/// fails. This is what the playground's "IR / Pipeline" tab displays.
///
/// * `tokens` — present once lexing succeeded.
/// * `ast`    — present once parsing succeeded. If type checking also ran, it
///   is the *annotated* tree (possibly partially annotated when the type
///   checker stopped at an error).
/// * `wasm`   — present once code generation succeeded.
/// * `error`  — the first error encountered, if any. At most one stage fails
///   and no later stage runs after a failure.
#[derive(Debug, Default, Serialize)]
pub struct CompileOutput {
    pub tokens: Option<Vec<token::Token>>,
    pub ast: Option<ast::Program>,
    pub wasm: Option<Vec<u8>>,
    pub error: Option<CompileError>,
}

/// Run the pipeline up to and including `up_to`, keeping every intermediate
/// artifact.
///
/// The playground calls this once per stage (`Lex`, then `Parse`, ...) so
/// that an unfinished stage (its `todo!()` panics) cannot take
/// the earlier, working stages down with it.
pub fn compile_pipeline(source: &str, up_to: Stage) -> CompileOutput {
    let mut out = CompileOutput::default();

    let tokens = match lexer::tokenize(source) {
        Ok(tokens) => tokens,
        Err(err) => {
            out.error = Some(err);
            return out;
        }
    };
    out.tokens = Some(tokens.clone());
    if up_to == Stage::Lex {
        return out;
    }

    let mut program = match parser::parse(tokens) {
        Ok(program) => program,
        Err(err) => {
            out.error = Some(err);
            return out;
        }
    };
    if up_to == Stage::Parse {
        out.ast = Some(program);
        return out;
    }

    if let Err(err) = typeck::check(&mut program) {
        out.ast = Some(program);
        out.error = Some(err);
        return out;
    }
    if up_to == Stage::TypeCheck {
        out.ast = Some(program);
        return out;
    }

    match codegen::generate(&program) {
        Ok(bytes) => out.wasm = Some(bytes),
        Err(err) => out.error = Some(err),
    }
    out.ast = Some(program);
    out
}

/// Compile kuro source all the way to a `.wasm` binary.
///
/// This is the function the end-to-end tests call.
pub fn compile(source: &str) -> Result<Vec<u8>> {
    let tokens = lexer::tokenize(source)?;
    let mut program = parser::parse(tokens)?;
    typeck::check(&mut program)?;
    codegen::generate(&program)
}

/// [`compile_pipeline`] with the result serialised to JSON.
///
/// Shape (see `web/src/trace/types.ts` for the TypeScript mirror):
///
/// ```json
/// { "tokens": [..] | null, "ast": {..} | null, "wasm": [0, 97, 115, 109, ..] | null, "error": {..} | null }
/// ```
pub fn compile_to_json(source: &str, up_to: Stage) -> String {
    let output = compile_pipeline(source, up_to);
    serde_json::to_string(&output).unwrap_or_else(|err| {
        format!(r#"{{"tokens":null,"ast":null,"wasm":null,"error":{{"kind":"Internal","stage":"codegen","message":"failed to serialise compiler output: {err}","span":{{"start":0,"end":0,"line":1,"col":1}}}}}}"#)
    })
}

/// The compiler's contribution to the playground's execution trace: tokens,
/// diagnostics, AST and module bytes. `steps` and `stdout` stay empty because
/// they come from running the module in the host.
pub fn compile_to_trace_json(source: &str) -> String {
    let output = compile_pipeline(source, Stage::Codegen);
    let trace = trace::ExecutionTrace::from_output(source, &output);
    serde_json::to_string(&trace).unwrap_or_else(|_| "null".to_string())
}

/// The JavaScript-facing API, compiled only for `wasm32` (i.e. by
/// `wasm-pack build`). The playground loads it from `web/public/pkg/`.
#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use super::Stage;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    thread_local! {
        /// Message of the most recent Rust panic, so the playground can show
        /// *which* `todo!()` was hit instead of a bare "unreachable" trap.
        static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    /// Runs automatically when the module is instantiated.
    #[wasm_bindgen(start)]
    pub fn start() {
        std::panic::set_hook(Box::new(|info| {
            let message = info.to_string();
            LAST_PANIC.with(|slot| *slot.borrow_mut() = Some(message));
        }));
    }

    /// Run the pipeline up to `up_to` (`"lex"`, `"parse"`, `"typecheck"` or
    /// `"codegen"`) and return the `CompileOutput` as a JSON string.
    ///
    /// A Rust panic inside a stage (for example a `todo!()`) surfaces in
    /// JavaScript as a thrown `WebAssembly.RuntimeError`; the caller then
    /// reads the reason with [`take_last_panic`].
    #[wasm_bindgen]
    pub fn compile_json(source: &str, up_to: &str) -> String {
        let stage = Stage::from_name(up_to).unwrap_or(Stage::Codegen);
        super::compile_to_json(source, stage)
    }

    /// The whole pipeline as an `ExecutionTrace` JSON string (see `trace.rs`).
    /// Panics like [`compile_json`].
    #[wasm_bindgen]
    pub fn compile_trace(source: &str) -> String {
        super::compile_to_trace_json(source)
    }

    /// The message of the last panic, clearing it. `undefined` in JS if no
    /// panic happened since the last call.
    #[wasm_bindgen]
    pub fn take_last_panic() -> Option<String> {
        LAST_PANIC.with(|slot| slot.borrow_mut().take())
    }

    /// Crate version. Exported for hosts; the workbench does not display it yet.
    #[wasm_bindgen]
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}
