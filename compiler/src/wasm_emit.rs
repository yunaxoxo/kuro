//! WebAssembly binary primitives: constants from the binary format and the
//! byte-level encoders.
//!
//! LEB128 integers, module structure and instruction encodings. The constants
//! below are facts of the WebAssembly MVP specification, not design decisions.
//! Tests: `cargo test --test codegen_tests`.

#![allow(dead_code, unused_variables)]

pub const MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];
pub const VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

pub mod section {
    pub const TYPE: u8 = 1;
    pub const IMPORT: u8 = 2;
    pub const FUNCTION: u8 = 3;
    pub const MEMORY: u8 = 5;
    pub const EXPORT: u8 = 7;
    pub const CODE: u8 = 10;
}

pub mod valtype {
    pub const I32: u8 = 0x7F;
    pub const F64: u8 = 0x7C;
}

pub const FUNC_TYPE: u8 = 0x60;
pub const BLOCK_TYPE_EMPTY: u8 = 0x40;

pub mod extern_kind {
    pub const FUNC: u8 = 0x00;
    pub const MEMORY: u8 = 0x02;
}

pub mod opcode {
    pub const UNREACHABLE: u8 = 0x00;
    pub const NOP: u8 = 0x01;
    pub const BLOCK: u8 = 0x02;
    pub const LOOP: u8 = 0x03;
    pub const IF: u8 = 0x04;
    pub const ELSE: u8 = 0x05;
    pub const END: u8 = 0x0B;
    pub const BR: u8 = 0x0C;
    pub const BR_IF: u8 = 0x0D;
    pub const RETURN: u8 = 0x0F;
    pub const CALL: u8 = 0x10;
    pub const DROP: u8 = 0x1A;
    pub const LOCAL_GET: u8 = 0x20;
    pub const LOCAL_SET: u8 = 0x21;
    pub const LOCAL_TEE: u8 = 0x22;
    pub const I32_LOAD: u8 = 0x28;
    pub const I32_STORE: u8 = 0x36;
    pub const I32_CONST: u8 = 0x41;
    pub const F64_CONST: u8 = 0x44;
    pub const I32_EQZ: u8 = 0x45;
    pub const I32_EQ: u8 = 0x46;
    pub const I32_NE: u8 = 0x47;
    pub const I32_LT_S: u8 = 0x48;
    pub const I32_GT_S: u8 = 0x4A;
    pub const I32_LE_S: u8 = 0x4C;
    pub const I32_GE_S: u8 = 0x4E;
    pub const F64_EQ: u8 = 0x61;
    pub const F64_NE: u8 = 0x62;
    pub const F64_LT: u8 = 0x63;
    pub const F64_GT: u8 = 0x64;
    pub const F64_LE: u8 = 0x65;
    pub const F64_GE: u8 = 0x66;
    pub const I32_ADD: u8 = 0x6A;
    pub const I32_SUB: u8 = 0x6B;
    pub const I32_MUL: u8 = 0x6C;
    pub const I32_DIV_S: u8 = 0x6D;
    pub const I32_REM_S: u8 = 0x6F;
    pub const F64_NEG: u8 = 0x9A;
    pub const F64_ADD: u8 = 0xA0;
    pub const F64_SUB: u8 = 0xA1;
    pub const F64_MUL: u8 = 0xA2;
    pub const F64_DIV: u8 = 0xA3;
}

/// Append `value` as unsigned LEB128.
pub fn write_u32_leb128(out: &mut Vec<u8>, value: u32) {
    todo!("wasm_emit: write_u32_leb128")
}

/// Append `value` as signed LEB128.
pub fn write_i32_leb128(out: &mut Vec<u8>, value: i32) {
    todo!("wasm_emit: write_i32_leb128")
}

/// Append a wasm name.
pub fn write_name(out: &mut Vec<u8>, name: &str) {
    todo!("wasm_emit: write_name")
}

/// Append a framed section.
pub fn write_section(out: &mut Vec<u8>, id: u8, payload: &[u8]) {
    todo!("wasm_emit: write_section")
}
