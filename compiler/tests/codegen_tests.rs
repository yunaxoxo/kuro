//! Code generator tests that inspect the binary **without running it**:
//! `cargo test --test codegen_tests`
//!
//! Covers LEB128 encoding and the module sections. Implementation notes:
//! `src/codegen.rs`.
//!
//! Suggested order (each group needs a little more of `codegen.rs`):
//!
//! 1. `*leb128*`, `write_name*`, `write_section*`, `wasm_type*` — the
//!    encoding primitives, no AST involved.
//! 2. `module_starts_with_magic_and_version`
//! 3. everything else — needs all six sections.
//!
//! `wasmparser` is the validator used by wasmtime and most wasm tooling.
//! When it rejects your module the message contains the byte offset of the
//! problem; look that offset up in the playground's hex view or with
//! `xxd`.

mod common;

use common::compile_ok;
use kuro::ast::Type;
use kuro::codegen::{self, FIRST_USER_FN, FN_ALLOC};
use kuro::wasm_emit::{write_i32_leb128, write_name, write_section, write_u32_leb128};
use wasmparser::{ExternalKind, FuncType, Parser, Payload, TypeRef, ValType};

const MINIMAL: &str = "fn main() { }";

fn uleb(value: u32) -> Vec<u8> {
    let mut out = Vec::new();
    write_u32_leb128(&mut out, value);
    out
}

fn sleb(value: i32) -> Vec<u8> {
    let mut out = Vec::new();
    write_i32_leb128(&mut out, value);
    out
}

/// Validate with wasmparser, turning a failure into a readable panic.
fn assert_valid(wasm: &[u8], what: &str) {
    if let Err(err) = wasmparser::validate(wasm) {
        panic!(
            "{what}: wasmparser rejected the module: {err}\n\
             (the offset is a byte position in the binary; {} bytes total)\n{}",
            wasm.len(),
            hex_dump(wasm)
        );
    }
}

fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:06x}: ", row * 16));
        for byte in chunk {
            out.push_str(&format!("{byte:02x} "));
        }
        out.push('\n');
    }
    out
}

/// Everything the structural tests want to know about a module.
#[derive(Default)]
struct ModuleInfo {
    /// Ids of the known sections, in the order they appear.
    section_ids: Vec<u8>,
    types: Vec<FuncType>,
    /// `(module, field, type index)` of every imported function.
    func_imports: Vec<(String, String, u32)>,
    /// Type index of every function defined in the module.
    func_type_indices: Vec<u32>,
    /// `(initial pages, maximum pages)` of every memory.
    memories: Vec<(u64, Option<u64>)>,
    /// `(name, kind, index)` of every export.
    exports: Vec<(String, ExternalKind, u32)>,
    code_bodies: usize,
}

impl ModuleInfo {
    fn parse(wasm: &[u8]) -> ModuleInfo {
        let mut info = ModuleInfo::default();
        for payload in Parser::new(0).parse_all(wasm) {
            match payload.expect("malformed module (run the validation tests first)") {
                Payload::TypeSection(reader) => {
                    info.section_ids.push(1);
                    for ty in reader.into_iter_err_on_gc_types() {
                        info.types.push(ty.expect("bad type entry"));
                    }
                }
                Payload::ImportSection(reader) => {
                    info.section_ids.push(2);
                    for import in reader.into_imports() {
                        let import = import.expect("bad import entry");
                        match import.ty {
                            TypeRef::Func(type_index) => info.func_imports.push((
                                import.module.to_string(),
                                import.name.to_string(),
                                type_index,
                            )),
                            other => panic!("unexpected non-function import: {other:?}"),
                        }
                    }
                }
                Payload::FunctionSection(reader) => {
                    info.section_ids.push(3);
                    for type_index in reader {
                        info.func_type_indices.push(type_index.expect("bad function entry"));
                    }
                }
                Payload::MemorySection(reader) => {
                    info.section_ids.push(5);
                    for memory in reader {
                        let memory = memory.expect("bad memory entry");
                        info.memories.push((memory.initial, memory.maximum));
                    }
                }
                Payload::ExportSection(reader) => {
                    info.section_ids.push(7);
                    for export in reader {
                        let export = export.expect("bad export entry");
                        info.exports.push((export.name.to_string(), export.kind, export.index));
                    }
                }
                Payload::CodeSectionStart { .. } => info.section_ids.push(10),
                Payload::CodeSectionEntry(_) => info.code_bodies += 1,
                Payload::TableSection(_) => info.section_ids.push(4),
                Payload::GlobalSection(_) => info.section_ids.push(6),
                Payload::StartSection { .. } => info.section_ids.push(8),
                Payload::ElementSection(_) => info.section_ids.push(9),
                Payload::DataSection(_) => info.section_ids.push(11),
                _ => {}
            }
        }
        info
    }

    fn export(&self, name: &str) -> (ExternalKind, u32) {
        self.exports
            .iter()
            .find(|(export_name, _, _)| export_name == name)
            .map(|(_, kind, index)| (*kind, *index))
            .unwrap_or_else(|| {
                let names: Vec<&str> = self.exports.iter().map(|(n, _, _)| n.as_str()).collect();
                panic!("no export named `{name}`; the module exports {names:?}")
            })
    }

    /// The signature of an exported function, resolved through the function
    /// index space: imports first, then the Function section.
    fn signature_of_export(&self, name: &str) -> &FuncType {
        let (kind, func_index) = self.export(name);
        assert_eq!(kind, ExternalKind::Func, "`{name}` should be a function export");
        let func_index = func_index as usize;
        let type_index = if func_index < self.func_imports.len() {
            self.func_imports[func_index].2
        } else {
            self.func_type_indices[func_index - self.func_imports.len()]
        };
        &self.types[type_index as usize]
    }
}

// ============================================================================
// 1. Encoding primitives
// ============================================================================

#[test]
fn unsigned_leb128_known_vectors() {
    assert_eq!(uleb(0), [0x00]);
    assert_eq!(uleb(1), [0x01]);
    assert_eq!(uleb(63), [0x3F]);
    assert_eq!(uleb(64), [0x40], "unsigned: 64 fits in one byte");
    assert_eq!(uleb(127), [0x7F]);
    assert_eq!(uleb(128), [0x80, 0x01]);
    assert_eq!(uleb(255), [0xFF, 0x01]);
    assert_eq!(uleb(300), [0xAC, 0x02]);
    assert_eq!(uleb(16383), [0xFF, 0x7F]);
    assert_eq!(uleb(16384), [0x80, 0x80, 0x01]);
    assert_eq!(uleb(624485), [0xE5, 0x8E, 0x26]);
    assert_eq!(uleb(u32::MAX), [0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
}

#[test]
fn signed_leb128_known_vectors() {
    assert_eq!(sleb(0), [0x00]);
    assert_eq!(sleb(1), [0x01]);
    assert_eq!(sleb(63), [0x3F]);
    assert_eq!(sleb(64), [0xC0, 0x00], "signed: 0x40 alone would decode as -64");
    assert_eq!(sleb(127), [0xFF, 0x00]);
    assert_eq!(sleb(128), [0x80, 0x01]);
    assert_eq!(sleb(8191), [0xFF, 0x3F]);
    assert_eq!(sleb(8192), [0x80, 0xC0, 0x00]);
    assert_eq!(sleb(-1), [0x7F]);
    assert_eq!(sleb(-2), [0x7E]);
    assert_eq!(sleb(-64), [0x40]);
    assert_eq!(sleb(-65), [0xBF, 0x7F]);
    assert_eq!(sleb(-128), [0x80, 0x7F]);
    assert_eq!(sleb(-129), [0xFF, 0x7E]);
    assert_eq!(sleb(-123456), [0xC0, 0xBB, 0x78]);
    assert_eq!(sleb(i32::MAX), [0xFF, 0xFF, 0xFF, 0xFF, 0x07]);
    assert_eq!(sleb(i32::MIN), [0x80, 0x80, 0x80, 0x80, 0x78]);
}

#[test]
fn leb128_round_trips_through_a_reference_decoder() {
    // An independent decoder, so the encoder is checked against the format
    // rather than against a table.
    fn decode_unsigned(bytes: &[u8]) -> u32 {
        let mut result: u64 = 0;
        for (i, byte) in bytes.iter().enumerate() {
            result |= u64::from(byte & 0x7F) << (7 * i);
            assert_eq!(byte & 0x80 != 0, i + 1 < bytes.len(), "continuation bit on the wrong byte");
        }
        result as u32
    }
    fn decode_signed(bytes: &[u8]) -> i32 {
        let mut result: i64 = 0;
        let mut shift = 0;
        for (i, byte) in bytes.iter().enumerate() {
            result |= i64::from(byte & 0x7F) << shift;
            shift += 7;
            assert_eq!(byte & 0x80 != 0, i + 1 < bytes.len(), "continuation bit on the wrong byte");
        }
        if bytes.last().unwrap() & 0x40 != 0 {
            result |= -1i64 << shift; // sign-extend from the last byte's bit 6
        }
        result as i32
    }

    let mut samples: Vec<i32> = vec![0, 1, -1, i32::MAX, i32::MIN];
    for shift in 0..31 {
        let power = 1i32 << shift;
        samples.extend([power - 1, power, power + 1, -power - 1, -power, -power + 1]);
    }
    for value in samples {
        let signed = sleb(value);
        assert!((1..=5).contains(&signed.len()), "{value}: signed LEB128 of an i32 is 1 to 5 bytes");
        assert_eq!(decode_signed(&signed), value, "signed round trip of {value}: {signed:02X?}");

        let unsigned_value = value as u32;
        let unsigned = uleb(unsigned_value);
        assert!((1..=5).contains(&unsigned.len()));
        assert_eq!(decode_unsigned(&unsigned), unsigned_value, "unsigned round trip of {unsigned_value}: {unsigned:02X?}");
    }
}

#[test]
fn encoders_append_to_the_buffer() {
    let mut out = vec![0xAA];
    write_u32_leb128(&mut out, 128);
    write_i32_leb128(&mut out, -1);
    write_name(&mut out, "hi");
    assert_eq!(out, [0xAA, 0x80, 0x01, 0x7F, 0x02, b'h', b'i']);
}

#[test]
fn write_name_is_a_length_prefixed_utf8_string() {
    let mut out = Vec::new();
    write_name(&mut out, "main");
    assert_eq!(out, [0x04, 0x6D, 0x61, 0x69, 0x6E]);

    let mut out = Vec::new();
    write_name(&mut out, "");
    assert_eq!(out, [0x00]);

    // The prefix is the BYTE length, not the character count.
    let mut out = Vec::new();
    write_name(&mut out, "Σ");
    assert_eq!(out, [0x02, 0xCE, 0xA3]);

    // And it is LEB128, so long names get a multi-byte prefix.
    let long = "x".repeat(200);
    let mut out = Vec::new();
    write_name(&mut out, &long);
    assert_eq!(&out[..2], [0xC8, 0x01]);
    assert_eq!(out.len(), 202);
}

#[test]
fn write_section_prefixes_id_and_payload_size() {
    let mut out = Vec::new();
    write_section(&mut out, 5, &[0x01, 0x00, 0x01]);
    assert_eq!(out, [0x05, 0x03, 0x01, 0x00, 0x01]);

    let mut out = Vec::new();
    write_section(&mut out, 1, &[]);
    assert_eq!(out, [0x01, 0x00]);

    // 300 payload bytes → the size needs two LEB128 bytes (AC 02).
    let payload = vec![0u8; 300];
    let mut out = Vec::new();
    write_section(&mut out, 10, &payload);
    assert_eq!(&out[..3], [0x0A, 0xAC, 0x02]);
    assert_eq!(out.len(), 3 + 300);
}

#[test]
fn wasm_type_maps_kuro_types_to_value_types() {
    assert_eq!(codegen::wasm_type(Type::Int), Some(0x7F));
    assert_eq!(codegen::wasm_type(Type::Bool), Some(0x7F));
    assert_eq!(codegen::wasm_type(Type::IntArray), Some(0x7F));
    assert_eq!(codegen::wasm_type(Type::Float), Some(0x7C));
    assert_eq!(codegen::wasm_type(Type::Void), None);
}

// ============================================================================
// 2. Module header
// ============================================================================

#[test]
fn module_starts_with_magic_and_version() {
    let wasm = compile_ok(MINIMAL);
    assert!(wasm.len() >= 8, "a module is at least the 8-byte header");
    assert_eq!(&wasm[0..4], b"\0asm", "magic number");
    assert_eq!(&wasm[4..8], [0x01, 0x00, 0x00, 0x00], "version 1, little-endian");
}

// ============================================================================
// 3. Whole-module structure
// ============================================================================

#[test]
fn minimal_program_is_a_valid_module() {
    assert_valid(&compile_ok(MINIMAL), "minimal program");
}

#[test]
fn example_programs_are_valid_modules() {
    for (name, source) in common::EXAMPLES {
        assert_valid(&compile_ok(source), name);
    }
}

#[test]
fn every_statement_and_expression_form_validates() {
    // One program touching every construct, so a validation failure here
    // usually means a stack-discipline slip in one specific gen_* arm.
    let wasm = compile_ok(
        "fn helper(a: int, x: float, flag: bool, arr: int[]) -> float {\n\
             let local = a * 2;\n\
             if flag && local > 0 || !flag { arr[a] = local; } else { arr[0] = -a; }\n\
             while a > 0 { a = a - 1; if a == 3 { return x; } }\n\
             helper(a, x, flag, arr);\n\
             1 + 2;\n\
             print_int(arr[0] % 7);\n\
             print_float(-x / 2.0);\n\
             return x * 2.0 - 1.5;\n\
         }\n\
         fn pick(c: bool) -> int { if c { return 1; } else { return 2; } }\n\
         fn nothing() { return; }\n\
         fn main() -> int {\n\
             let arr: int[] = alloc(8);\n\
             let total = helper(4, 0.5, true, arr);\n\
             nothing();\n\
             if total >= 0.0 { return len(arr) + pick(total == 0.0); }\n\
             return pick(false);\n\
         }",
    );
    assert_valid(&wasm, "kitchen-sink program");
}

#[test]
fn sections_appear_in_canonical_order() {
    let info = ModuleInfo::parse(&compile_ok(MINIMAL));
    let required: Vec<u8> = info
        .section_ids
        .iter()
        .copied()
        .filter(|id| [1, 2, 3, 5, 7, 10].contains(id))
        .collect();
    assert_eq!(
        required,
        [1, 2, 3, 5, 7, 10],
        "expected Type, Import, Function, Memory, Export, Code exactly once each, in that order"
    );
}

#[test]
fn imports_the_two_host_functions() {
    let info = ModuleInfo::parse(&compile_ok(MINIMAL));

    // Both are imported even though this program calls neither: their
    // positions define function indices 0 and 1.
    let names: Vec<(&str, &str)> = info
        .func_imports
        .iter()
        .map(|(module, field, _)| (module.as_str(), field.as_str()))
        .collect();
    assert_eq!(names, [("env", "print_int"), ("env", "print_float")]);

    let print_int = &info.types[info.func_imports[0].2 as usize];
    assert_eq!(print_int.params(), [ValType::I32]);
    assert_eq!(print_int.results(), []);

    let print_float = &info.types[info.func_imports[1].2 as usize];
    assert_eq!(print_float.params(), [ValType::F64]);
    assert_eq!(print_float.results(), []);
}

#[test]
fn declares_one_page_of_memory() {
    let info = ModuleInfo::parse(&compile_ok(MINIMAL));
    assert_eq!(info.memories.len(), 1, "exactly one linear memory");
    let (initial, _maximum) = info.memories[0];
    assert!(initial >= 1, "at least one 64 KiB page");
}

#[test]
fn exports_memory_alloc_and_every_user_function() {
    let info = ModuleInfo::parse(&compile_ok(
        "fn fib(n: int) -> int { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }\n\
         fn main() -> int { return fib(10); }",
    ));

    assert_eq!(info.export("memory"), (ExternalKind::Memory, 0));
    assert_eq!(info.export("alloc"), (ExternalKind::Func, FN_ALLOC));
    // User functions take indices from FIRST_USER_FN upwards, in source order.
    assert_eq!(info.export("fib"), (ExternalKind::Func, FIRST_USER_FN));
    assert_eq!(info.export("main"), (ExternalKind::Func, FIRST_USER_FN + 1));

    let alloc = info.signature_of_export("alloc");
    assert_eq!(alloc.params(), [ValType::I32]);
    assert_eq!(alloc.results(), [ValType::I32]);
}

#[test]
fn one_code_body_per_defined_function() {
    let info = ModuleInfo::parse(&compile_ok("fn a() { } fn b() { } fn main() { }"));
    // alloc + three user functions.
    assert_eq!(info.func_type_indices.len(), 4, "Function section entries");
    assert_eq!(info.code_bodies, 4, "Code section bodies");
}

#[test]
fn function_signatures_map_to_wasm_types() {
    let info = ModuleInfo::parse(&compile_ok(
        "fn mixed(a: int, x: float, flag: bool, arr: int[]) -> float { return x; }\n\
         fn make(n: int) -> int[] { return alloc(n); }\n\
         fn test(x: float) -> bool { return x > 0.0; }\n\
         fn main() { }",
    ));

    let mixed = info.signature_of_export("mixed");
    assert_eq!(mixed.params(), [ValType::I32, ValType::F64, ValType::I32, ValType::I32]);
    assert_eq!(mixed.results(), [ValType::F64]);

    let make = info.signature_of_export("make");
    assert_eq!(make.params(), [ValType::I32]);
    assert_eq!(make.results(), [ValType::I32], "an int[] is an i32 address");

    let test = info.signature_of_export("test");
    assert_eq!(test.params(), [ValType::F64]);
    assert_eq!(test.results(), [ValType::I32], "a bool is an i32");

    let main = info.signature_of_export("main");
    assert_eq!(main.params(), []);
    assert_eq!(main.results(), [], "a function without `->` has no results");
}
