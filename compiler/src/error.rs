//! # Compile errors
//!
//! One error type for the whole pipeline. This file is fully implemented.
//!
//! The test suite asserts on [`CompileError::kind`] (and sometimes on
//! `span.line` / `span.col`), never on the wording of `message`.

use crate::token::Span;
use serde::Serialize;
use std::fmt;

/// Shorthand used by every compiler module.
pub type Result<T> = std::result::Result<T, CompileError>;

/// The four phases of the compiler, in pipeline order.
///
/// The ordering (`Lex < Parse < TypeCheck < Codegen`) is meaningful:
/// [`crate::compile_pipeline`] runs every stage `<=` the one requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Lex,
    Parse,
    TypeCheck,
    Codegen,
}

impl Stage {
    /// All stages in pipeline order.
    pub const ALL: [Stage; 4] = [Stage::Lex, Stage::Parse, Stage::TypeCheck, Stage::Codegen];

    /// The lowercase name used in JSON and by the playground:
    /// `"lex"`, `"parse"`, `"typecheck"`, `"codegen"`.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Lex => "lex",
            Stage::Parse => "parse",
            Stage::TypeCheck => "typecheck",
            Stage::Codegen => "codegen",
        }
    }

    /// Inverse of [`Stage::name`].
    pub fn from_name(name: &str) -> Option<Stage> {
        Stage::ALL.into_iter().find(|stage| stage.name() == name)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// *What* went wrong, independent of the message text.
///
/// Each situation maps to exactly one kind. When two
/// kinds could plausibly apply, the more specific one wins (indexing an `int`
/// is `NotAnArray`, not `TypeMismatch`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum ErrorKind {
    // ----- lexer ------------------------------------------------------------
    /// A character that cannot start any token: `@`, `#`, a lone `&` or `|`,
    /// any non-ASCII character outside a comment.
    UnexpectedChar,
    /// A malformed numeric literal: an integer above `i32::MAX`
    /// (2147483647), or digits followed by `.` without a digit after it
    /// (`1.`, `1.x`).
    InvalidNumber,

    // ----- parser -----------------------------------------------------------
    /// The parser needed one thing and found another (including running into
    /// end-of-input early). The span is the offending token.
    UnexpectedToken,
    /// The left side of `=` is not a variable or an index expression,
    /// e.g. `1 + 2 = 3;` or `f(x) = 3;`.
    InvalidAssignTarget,

    // ----- type checker -----------------------------------------------------
    /// Use of a variable that is not in scope.
    UndefinedVariable,
    /// Call to a function that is neither user-defined nor a builtin.
    UndefinedFunction,
    /// Two functions with one name, a function named like a builtin, two
    /// parameters with one name, or two `let`s of one name in the same block.
    DuplicateDefinition,
    /// Operand / annotation / assignment / condition / argument / index of
    /// the wrong type. The general-purpose type error.
    TypeMismatch,
    /// The base of `base[index]` (read or write) is not an `int[]`.
    NotAnArray,
    /// A call passes the wrong number of arguments.
    ArgCountMismatch,
    /// A `return` whose value (or lack of one) does not match the function's
    /// declared return type.
    ReturnTypeMismatch,
    /// A function with a return type can reach the end of its body without
    /// executing a `return`.
    MissingReturn,
    /// The program does not define `fn main`.
    MissingMain,
    /// `main` is defined but declares parameters.
    InvalidMain,

    // ----- code generator ---------------------------------------------------
    /// A codegen invariant was broken (e.g. an expression reached codegen
    /// without a type annotation). Indicates a compiler bug, not a user error.
    Internal,
}

impl ErrorKind {
    /// The pipeline stage that reports this kind of error.
    pub fn stage(self) -> Stage {
        use ErrorKind::*;
        match self {
            UnexpectedChar | InvalidNumber => Stage::Lex,
            UnexpectedToken | InvalidAssignTarget => Stage::Parse,
            UndefinedVariable | UndefinedFunction | DuplicateDefinition | TypeMismatch
            | NotAnArray | ArgCountMismatch | ReturnTypeMismatch | MissingReturn
            | MissingMain | InvalidMain => Stage::TypeCheck,
            Internal => Stage::Codegen,
        }
    }
}

/// A diagnostic: what went wrong, a human explanation, and where.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompileError {
    pub kind: ErrorKind,
    /// Derived from `kind`; stored so it shows up in the JSON the UI reads.
    pub stage: Stage,
    pub message: String,
    pub span: Span,
}

impl CompileError {
    pub fn new(kind: ErrorKind, message: impl Into<String>, span: Span) -> Self {
        CompileError {
            kind,
            stage: kind.stage(),
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} error at {}:{}: {}",
            self.stage, self.span.line, self.span.col, self.message
        )
    }
}

impl std::error::Error for CompileError {}
