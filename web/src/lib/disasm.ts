/**
 * A small WebAssembly disassembler covering exactly the instruction set
 * kuro emits. It does not validate: it decodes as far as it can and
 * reports where it gave up, so it is useful while your code generator still
 * produces broken modules.
 */

export interface SectionInfo {
  id: number;
  name: string;
  /** Offset of the section id byte. */
  offset: number;
  /** Payload size in bytes (the value of the size field). */
  size: number;
  /** Offset of the first payload byte. */
  payloadOffset: number;
}

export interface Instr {
  offset: number;
  depth: number;
  bytes: string;
  text: string;
}

export interface DisFunction {
  index: number;
  name: string;
  signature: string;
  localsText: string;
  instrs: Instr[];
  /** Offset of the body-size field. */
  offset: number;
  error?: string;
}

export interface Disassembly {
  sections: SectionInfo[];
  functions: DisFunction[];
  /** First problem found while decoding, with its byte offset. */
  error?: { offset: number; message: string };
}

const SECTION_NAMES: Record<number, string> = {
  0: "Custom",
  1: "Type",
  2: "Import",
  3: "Function",
  4: "Table",
  5: "Memory",
  6: "Global",
  7: "Export",
  8: "Start",
  9: "Element",
  10: "Code",
  11: "Data",
};

const VALTYPES: Record<number, string> = { 0x7f: "i32", 0x7e: "i64", 0x7d: "f32", 0x7c: "f64" };

type Imm = "none" | "block" | "u32" | "mem" | "i32" | "f64" | "memidx";
const OPS: Record<number, [string, Imm]> = {
  0x00: ["unreachable", "none"],
  0x01: ["nop", "none"],
  0x02: ["block", "block"],
  0x03: ["loop", "block"],
  0x04: ["if", "block"],
  0x05: ["else", "none"],
  0x0b: ["end", "none"],
  0x0c: ["br", "u32"],
  0x0d: ["br_if", "u32"],
  0x0f: ["return", "none"],
  0x10: ["call", "u32"],
  0x1a: ["drop", "none"],
  0x1b: ["select", "none"],
  0x20: ["local.get", "u32"],
  0x21: ["local.set", "u32"],
  0x22: ["local.tee", "u32"],
  0x23: ["global.get", "u32"],
  0x24: ["global.set", "u32"],
  0x28: ["i32.load", "mem"],
  0x2b: ["f64.load", "mem"],
  0x36: ["i32.store", "mem"],
  0x39: ["f64.store", "mem"],
  0x3f: ["memory.size", "memidx"],
  0x40: ["memory.grow", "memidx"],
  0x41: ["i32.const", "i32"],
  0x44: ["f64.const", "f64"],
  0x45: ["i32.eqz", "none"],
  0x46: ["i32.eq", "none"],
  0x47: ["i32.ne", "none"],
  0x48: ["i32.lt_s", "none"],
  0x49: ["i32.lt_u", "none"],
  0x4a: ["i32.gt_s", "none"],
  0x4b: ["i32.gt_u", "none"],
  0x4c: ["i32.le_s", "none"],
  0x4d: ["i32.le_u", "none"],
  0x4e: ["i32.ge_s", "none"],
  0x4f: ["i32.ge_u", "none"],
  0x61: ["f64.eq", "none"],
  0x62: ["f64.ne", "none"],
  0x63: ["f64.lt", "none"],
  0x64: ["f64.gt", "none"],
  0x65: ["f64.le", "none"],
  0x66: ["f64.ge", "none"],
  0x6a: ["i32.add", "none"],
  0x6b: ["i32.sub", "none"],
  0x6c: ["i32.mul", "none"],
  0x6d: ["i32.div_s", "none"],
  0x6e: ["i32.div_u", "none"],
  0x6f: ["i32.rem_s", "none"],
  0x70: ["i32.rem_u", "none"],
  0x71: ["i32.and", "none"],
  0x72: ["i32.or", "none"],
  0x74: ["i32.shl", "none"],
  0x99: ["f64.abs", "none"],
  0x9a: ["f64.neg", "none"],
  0x9c: ["f64.floor", "none"],
  0x9f: ["f64.sqrt", "none"],
  0xa0: ["f64.add", "none"],
  0xa1: ["f64.sub", "none"],
  0xa2: ["f64.mul", "none"],
  0xa3: ["f64.div", "none"],
  0xaa: ["i32.trunc_f64_s", "none"],
  0xb7: ["f64.convert_i32_s", "none"],
};

class DecodeError extends Error {
  constructor(
    public offset: number,
    message: string,
  ) {
    super(message);
  }
}

class Reader {
  constructor(
    private bytes: Uint8Array,
    public pos = 0,
    public end = bytes.length,
  ) {}

  get eof() {
    return this.pos >= this.end;
  }

  byte(): number {
    if (this.pos >= this.end) throw new DecodeError(this.pos, "unexpected end of data");
    return this.bytes[this.pos++];
  }

  u32(): number {
    let result = 0;
    let shift = 0;
    for (let i = 0; i < 5; i++) {
      const b = this.byte();
      result += (b & 0x7f) * 2 ** shift;
      if ((b & 0x80) === 0) return result;
      shift += 7;
    }
    throw new DecodeError(this.pos - 1, "LEB128 longer than 5 bytes");
  }

  s32(): number {
    let result = 0;
    let shift = 0;
    for (let i = 0; i < 5; i++) {
      const b = this.byte();
      result |= (b & 0x7f) << shift;
      shift += 7;
      if ((b & 0x80) === 0) {
        if (shift < 32 && (b & 0x40) !== 0) result |= -1 << shift;
        return result | 0;
      }
    }
    throw new DecodeError(this.pos - 1, "LEB128 longer than 5 bytes");
  }

  f64(): number {
    if (this.pos + 8 > this.end) throw new DecodeError(this.pos, "unexpected end of data");
    const value = new DataView(this.bytes.buffer, this.bytes.byteOffset + this.pos, 8).getFloat64(0, true);
    this.pos += 8;
    return value;
  }

  name(): string {
    const length = this.u32();
    if (this.pos + length > this.end) throw new DecodeError(this.pos, "name runs past the end");
    const text = new TextDecoder().decode(this.bytes.subarray(this.pos, this.pos + length));
    this.pos += length;
    return text;
  }
}

const hex = (bytes: Uint8Array, from: number, to: number) =>
  Array.from(bytes.subarray(from, to), (b) => b.toString(16).padStart(2, "0")).join(" ");

const valtype = (b: number) => VALTYPES[b] ?? `0x${b.toString(16)}?`;

export function disassemble(bytes: Uint8Array): Disassembly {
  const out: Disassembly = { sections: [], functions: [] };
  try {
    decodeModule(bytes, out);
  } catch (err) {
    if (err instanceof DecodeError) out.error = { offset: err.offset, message: err.message };
    else throw err;
  }
  return out;
}

function decodeModule(bytes: Uint8Array, out: Disassembly) {
  if (bytes.length < 8) throw new DecodeError(0, "module is shorter than the 8-byte header");
  if (hex(bytes, 0, 4) !== "00 61 73 6d") throw new DecodeError(0, "bad magic number (expected 00 61 73 6d)");
  if (hex(bytes, 4, 8) !== "01 00 00 00") throw new DecodeError(4, "unsupported version (expected 01 00 00 00)");

  const r = new Reader(bytes, 8);
  const types: string[] = [];
  const importNames: string[] = [];
  const importTypes: number[] = [];
  const definedTypes: number[] = [];
  const exportNames = new Map<number, string>();

  while (!r.eof) {
    const offset = r.pos;
    const id = r.byte();
    const size = r.u32();
    const payloadOffset = r.pos;
    if (payloadOffset + size > bytes.length) {
      out.sections.push({ id, name: SECTION_NAMES[id] ?? `Unknown(${id})`, offset, size, payloadOffset });
      throw new DecodeError(offset + 1, `section ${SECTION_NAMES[id] ?? id} claims ${size} bytes but only ${bytes.length - payloadOffset} remain`);
    }
    out.sections.push({ id, name: SECTION_NAMES[id] ?? `Unknown(${id})`, offset, size, payloadOffset });
    const s = new Reader(bytes, payloadOffset, payloadOffset + size);

    switch (id) {
      case 1: {
        for (let n = s.u32(); n > 0; n--) {
          if (s.byte() !== 0x60) throw new DecodeError(s.pos - 1, "function type must start with 0x60");
          const params: string[] = [];
          for (let p = s.u32(); p > 0; p--) params.push(valtype(s.byte()));
          const results: string[] = [];
          for (let p = s.u32(); p > 0; p--) results.push(valtype(s.byte()));
          types.push(`(${params.join(", ")})${results.length ? " -> " + results.join(", ") : ""}`);
        }
        break;
      }
      case 2: {
        for (let n = s.u32(); n > 0; n--) {
          const module = s.name();
          const field = s.name();
          const kind = s.byte();
          if (kind !== 0) throw new DecodeError(s.pos - 1, "only function imports are supported");
          importNames.push(`${module}.${field}`);
          importTypes.push(s.u32());
        }
        break;
      }
      case 3: {
        for (let n = s.u32(); n > 0; n--) definedTypes.push(s.u32());
        break;
      }
      case 7: {
        for (let n = s.u32(); n > 0; n--) {
          const name = s.name();
          const kind = s.byte();
          const index = s.u32();
          if (kind === 0) exportNames.set(index, name);
        }
        break;
      }
      case 10: {
        const count = s.u32();
        for (let k = 0; k < count; k++) {
          const funcIndex = importNames.length + k;
          const fn: DisFunction = {
            index: funcIndex,
            name: exportNames.get(funcIndex) ?? `func${funcIndex}`,
            signature: types[definedTypes[k]] ?? "(?)",
            localsText: "",
            instrs: [],
            offset: s.pos,
          };
          out.functions.push(fn);
          const bodySize = s.u32();
          const bodyEnd = s.pos + bodySize;
          if (bodyEnd > s.end) throw new DecodeError(fn.offset, "function body runs past the end of the Code section");
          try {
            decodeBody(new Reader(bytes, s.pos, bodyEnd), bytes, fn, importNames);
          } catch (err) {
            if (err instanceof DecodeError) {
              fn.error = `${err.message} (at offset 0x${err.offset.toString(16)})`;
              if (!out.error) out.error = { offset: err.offset, message: err.message };
            } else throw err;
          }
          s.pos = bodyEnd;
        }
        break;
      }
      default:
        break;
    }
    r.pos = payloadOffset + size;
  }
}

function decodeBody(b: Reader, bytes: Uint8Array, fn: DisFunction, importNames: string[]) {
  const runs: string[] = [];
  for (let n = b.u32(); n > 0; n--) {
    const count = b.u32();
    runs.push(`${count} x ${valtype(b.byte())}`);
  }
  fn.localsText = runs.length ? `locals: ${runs.join(", ")}` : "no extra locals";

  let depth = 0;
  while (!b.eof) {
    const offset = b.pos;
    const opcode = b.byte();
    const entry = OPS[opcode];
    if (!entry) throw new DecodeError(offset, `unknown opcode 0x${opcode.toString(16).padStart(2, "0")}`);
    const [name, imm] = entry;
    let text = name;
    switch (imm) {
      case "block": {
        const t = b.byte();
        text += t === 0x40 ? "" : ` (result ${valtype(t)})`;
        break;
      }
      case "u32": {
        const value = b.u32();
        text += ` ${value}`;
        if (name === "call") text += ` ;; ${value < importNames.length ? importNames[value] : `func${value}`}`;
        break;
      }
      case "mem": {
        const align = b.u32();
        const offsetImm = b.u32();
        text += ` align=${align} offset=${offsetImm}`;
        break;
      }
      case "i32":
        text += ` ${b.s32()}`;
        break;
      case "f64":
        text += ` ${b.f64()}`;
        break;
      case "memidx":
        b.byte();
        break;
      case "none":
        break;
    }

    if (name === "end" || name === "else") depth = Math.max(0, depth - 1);
    fn.instrs.push({ offset, depth, bytes: hex(bytes, offset, b.pos), text });
    if (name === "block" || name === "loop" || name === "if" || name === "else") depth++;
  }
}
