//! Code generator: annotated [`Program`] → WebAssembly binary.
//!
//! Byte-level encoders and opcode constants live in `wasm_emit.rs`.
//! Tests: `cargo test --test codegen_tests`.

#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use crate::ast::{BinaryOp, Block, Expr, ExprKind, Function, Program, Stmt, Type, UnaryOp};
use crate::error::{CompileError, ErrorKind, Result};
use crate::token::Span;

// ABI constants: host import indices and the linear-memory layout.
pub const FN_PRINT_INT: u32 = 0;
pub const FN_PRINT_FLOAT: u32 = 1;
pub const FN_ALLOC: u32 = 2;
pub const FIRST_USER_FN: u32 = 3;

pub const HEAP_PTR_ADDR: i32 = 0;
pub const HEAP_BASE: i32 = 8;
pub const ARRAY_HEADER_BYTES: i32 = 4;
pub const ELEM_BYTES: i32 = 4;
pub const MEMORY_PAGES: u32 = 1;

/// The wasm value type (`wasm_emit::valtype`) that represents a kuro type,
/// or `None` for `Void` (value-type lowering).
pub fn wasm_type(ty: Type) -> Option<u8> {
    todo!("codegen: wasm_type")
}

/// Code-generator state.
pub struct Codegen {}

impl Codegen {
    pub fn new() -> Self {
        todo!("codegen: Codegen::new")
    }

    /// Produce the complete binary for a type-checked program.
    pub fn generate(self, program: &Program) -> Result<Vec<u8>> {
        todo!("codegen: Codegen::generate")
    }
}

/// Entry point used by `lib.rs` and the tests.
pub fn generate(program: &Program) -> Result<Vec<u8>> {
    Codegen::new().generate(program)
}
