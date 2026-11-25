//! Lexer: source text → tokens.
//!
//! Tests: `cargo test --test lexer_tests`.

#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use crate::error::{CompileError, ErrorKind, Result};
use crate::token::{Span, Token, TokenKind};

/// Lexer state.
pub struct Lexer<'src> {
    source: &'src str,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        todo!("lexer: Lexer::new")
    }

    /// Consume the lexer and produce the token stream (terminated by `Eof`).
    pub fn tokenize(self) -> Result<Vec<Token>> {
        todo!("lexer: Lexer::tokenize")
    }
}

/// Entry point used by `lib.rs` and the tests.
pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).tokenize()
}
