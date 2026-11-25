//! Type checker: [`Program`] → annotated [`Program`].
//!
//! Tests: `cargo test --test typechecker_tests`.

#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use crate::ast::{BinaryOp, Block, Expr, ExprKind, Function, Program, Stmt, Type, UnaryOp};
use crate::error::{CompileError, ErrorKind, Result};
use crate::token::Span;

/// Type-checker state.
pub struct TypeChecker {}

impl TypeChecker {
    pub fn new() -> Self {
        todo!("typeck: TypeChecker::new")
    }

    /// Check a program and set `expr.ty` on every accepted expression.
    pub fn check_program(&mut self, program: &mut Program) -> Result<()> {
        todo!("typeck: TypeChecker::check_program")
    }
}

/// Entry point used by `lib.rs` and the tests.
pub fn check(program: &mut Program) -> Result<()> {
    TypeChecker::new().check_program(program)
}
