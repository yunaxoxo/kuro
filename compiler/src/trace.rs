//! The execution-trace contract shared with the playground.
//!
//! These types mirror `web/src/trace/types.ts` field for field. They are plain
//! data plus one mapping function ([`ExecutionTrace::from_output`]) from the
//! compiler's pipeline output; there is no compiler logic here.
//!
//! `steps` and `stdout` come from *running* the emitted module, which is the
//! host's job. The compiler leaves both empty.

use crate::CompileOutput;
use crate::error::CompileError;
use serde::Serialize;

/// Where a trace came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceMode {
    /// Produced by this compiler.
    Live,
    /// A bundled fixture. The compiler never emits this; the playground does.
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub line: u32,
    pub col: u32,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceToken {
    /// Variant name of the token kind, e.g. `"Ident"`, `"Arrow"`.
    pub kind: String,
    /// The exact source text of the token.
    pub lexeme: String,
    pub line: u32,
    pub col: u32,
}

/// One observable moment of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceStep {
    pub step_index: u32,
    /// Source line the step is attributed to (1-based), or 0 when unknown.
    pub pc: u32,
    pub op: String,
    /// One plain-English sentence describing what changed.
    pub narration: String,
    /// The used prefix of linear memory as little-endian `i32` cells.
    pub memory_i32: Vec<i32>,
    /// Indices into `memory_i32` whose value differs from the previous step.
    pub mutated_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExecutionTrace {
    pub mode: TraceMode,
    pub stdout: String,
    pub diagnostics: Vec<Diagnostic>,
    pub tokens: Vec<TraceToken>,
    pub ast_json: serde_json::Value,
    pub wasm_bytes: Vec<u8>,
    pub steps: Vec<TraceStep>,
}

impl Diagnostic {
    pub fn from_error(error: &CompileError) -> Self {
        Diagnostic {
            line: error.span.line,
            col: error.span.col,
            severity: Severity::Error,
            message: error.message.clone(),
        }
    }
}

impl ExecutionTrace {
    /// Map pipeline output to the trace shape. Missing artifacts become empty.
    pub fn from_output(source: &str, output: &CompileOutput) -> Self {
        let tokens = output
            .tokens
            .iter()
            .flatten()
            .map(|token| TraceToken {
                kind: serde_json::to_value(&token.kind)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|t| t.as_str().map(str::to_owned)))
                    .unwrap_or_default(),
                lexeme: source.get(token.span.start..token.span.end).unwrap_or("").to_owned(),
                line: token.span.line,
                col: token.span.col,
            })
            .collect();

        ExecutionTrace {
            mode: TraceMode::Live,
            stdout: String::new(),
            diagnostics: output.error.iter().map(Diagnostic::from_error).collect(),
            tokens,
            ast_json: output
                .ast
                .as_ref()
                .and_then(|ast| serde_json::to_value(ast).ok())
                .unwrap_or(serde_json::Value::Null),
            wasm_bytes: output.wasm.clone().unwrap_or_default(),
            steps: Vec::new(),
        }
    }
}
