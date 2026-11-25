//! Parser: tokens → [`Program`].
//!
//! Tests: `cargo test --test parser_tests`.

#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use crate::ast::{BinaryOp, Block, Expr, ExprKind, Function, Param, Program, Stmt, Type, UnaryOp};
use crate::error::{CompileError, ErrorKind, Result};
use crate::token::{Span, Token, TokenKind};

/// Parser state.
pub struct Parser {
    tokens: Vec<Token>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        todo!("parser: Parser::new")
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        todo!("parser: Parser::parse_program")
    }
}

/// Entry point used by `lib.rs` and the tests.
pub fn parse(tokens: Vec<Token>) -> Result<Program> {
    Parser::new(tokens).parse_program()
}

/// Parse one standalone expression that must be followed by end of input
/// Used by the precedence tests.
pub fn parse_expression(tokens: Vec<Token>) -> Result<Expr> {
    todo!("parser: parse_expression")
}
