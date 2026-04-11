//! Jzero `.j0` binary file format writer (Chapter 12/13).
//!
//! # File layout (all values little-endian 8-byte words)
//!
//! ```text
//! Word 0:  magic   "Jzero!!\0"
//! Word 1:  version "1.0\0\0\0\0\0"
//! Word 2:  first-instruction word offset (relative to word 0)
//! Word 3…: data section  (string literals packed as NUL-padded 8-byte words)
//! Word N…: instructions  (each a Byc encoded as one 8-byte word)
//! ```
//!
//! The startup sequence is prepended to the instructions so main() is called
//! automatically:
//!
//! ```text
//! PUSH <addr-of-main>   (R_ABS, byte offset of the proc main)
//! CALL imm:0            (0 parameters)
//! HALT
//! ```
//!
//! Because main's address is not known until all instructions are laid out,
//! the startup sequence is patched in after the data section offset is fixed.

use std::collections::HashMap;

use crate::byc::{Byc, BycRegion, Op};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Assemble a complete `.j0` binary image.
///
/// * `bycs`       – translated bytecode (labels already patched by `translate`)
/// * `data`       – raw data section bytes (string pool + globals)
/// * `_labeltable` – label-id → byte offset within the instruction stream
/// * `main_abs`   – absolute byte offset of main (relative to magic word),
///                  or None to use 0 (will HALT immediately)
pub fn assemble(
    bycs: &[Byc],
    data: &[u8],
    _labeltable: &HashMap<i64, usize>,
    main_abs: Option<i64>,
    argc: i64,
) -> Vec<u8> {
    // -----------------------------------------------------------------------
    // Sizes
    // -----------------------------------------------------------------------
    let data_padded  = pad8(data);
    let data_words   = data_padded.len() / 8;
    let header_words = 3usize;
    let startup_words = 4usize;
    let first_instr_word_off = header_words + data_words;

    // Byte offset of the first program instruction (after startup sequence),
    // relative to word 0 (the magic word).
    let code_base_bytes = (first_instr_word_off + startup_words) * 8;

    // -----------------------------------------------------------------------
    // Relocate label references.
    // Labels were patched in `bytecode::translate` as offsets relative to the
    // start of the instruction stream (0 = first emitted Byc).  The VM's ip
    // is relative to word 0, so we must add `code_base_bytes` to every
    // R_ABS operand that represents a label (GOTO / BIF targets).
    // -----------------------------------------------------------------------
    let relocated: Vec<Byc> = bycs.iter().map(|b| {
        match b.op {
            Op::Goto | Op::Bif => {
                let mut r = b.clone();
                r.opnd += code_base_bytes as i64;
                r
            }
            _ => b.clone(),
        }
    }).collect();

    // -----------------------------------------------------------------------
    // Resolve main's absolute byte offset.
    // -----------------------------------------------------------------------
    let main_addr: i64 = main_abs.unwrap_or(0);

    // -----------------------------------------------------------------------
    // Build the startup sequence.
    // Pushes: fn_addr(main), argc, then calls main with 1 arg.
    // Stack layout after CALL:
    //   stack[fn_slot+0] = main_addr  (bp points here, loc:0)
    //   stack[fn_slot+1] = argc       (loc:8, argv param)
    //   stack[fn_slot+2] = saved_ip
    //   stack[fn_slot+3] = saved_bp
    //   stack[fn_slot+4] = saved_ret
    // -----------------------------------------------------------------------
    let startup: Vec<Byc> = vec![
        Byc { op: Op::Push, region: BycRegion::Imm, opnd: main_addr, needs_patch: false },
        Byc::imm(Op::Push, argc),
        Byc::imm(Op::Call, 1),
        Byc::no_operand(Op::Halt),
    ];

    // -----------------------------------------------------------------------
    // Assemble the binary image.
    // -----------------------------------------------------------------------
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"Jzero!!\0");
    out.extend_from_slice(b"1.0\0\0\0\0\0");
    write_i64(&mut out, first_instr_word_off as i64);
    out.extend_from_slice(&data_padded);
    for b in &startup    { out.extend_from_slice(&b.binary()); }
    for b in &relocated  { out.extend_from_slice(&b.binary()); }
    out
}

/// Render a human-readable assembler listing.
///
/// Useful for debugging before the VM is available.
pub fn disassemble_text(bycs: &[Byc], data: &[u8]) -> String {
    let mut s = String::new();

    // Data section
    if !data.is_empty() {
        s.push_str(".data\n");
        let mut i = 0;
        while i + 8 <= data.len() {
            let word = read_i64(&data[i..i + 8]);
            s.push_str(&format!("\t@{:04x}  {:016x}\n", i, word));
            i += 8;
        }
    }

    // Startup sequence header comment
    s.push_str(".startup\n");
    // (The actual startup bytes are baked by `assemble`; shown symbolically.)
    s.push_str("\tpush <main>\n");
    s.push_str("\tcall 0\n");
    s.push_str("\thalt\n");

    // Instructions
    s.push_str(".code\n");
    let mut byte_off: usize = 0;
    for b in bycs {
        s.push_str(&format!("{:04x}:{}\n", byte_off, b.text()));
        byte_off += 8;
    }

    s
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Pad `bytes` to the next multiple of 8.
fn pad8(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    let rem = v.len() % 8;
    if rem != 0 {
        v.resize(v.len() + (8 - rem), 0);
    }
    v
}

/// Write a little-endian i64 into `out`.
fn write_i64(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Read a little-endian i64 from an 8-byte slice.
fn read_i64(b: &[u8]) -> i64 {
    i64::from_le_bytes(b[..8].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_program_has_correct_header() {
        let out = assemble(&[], &[], &HashMap::new(), None, 0);
        // Word 0: magic
        assert_eq!(&out[0..8],  b"Jzero!!\0");
        // Word 1: version
        assert_eq!(&out[8..16], b"1.0\0\0\0\0\0");
        // Word 2: first-instr offset = 3 (header words, no data)
        let off = i64::from_le_bytes(out[16..24].try_into().unwrap());
        assert_eq!(off, 3);
        // Total size = 3 header words + 4 startup words = 7 × 8 = 56 bytes
        assert_eq!(out.len(), 56);
    }

    #[test]
    fn data_section_is_padded_to_8_bytes() {
        let data = b"hello\0\0\0"; // already 8 bytes
        let out = assemble(&[], data, &HashMap::new(), None, 0);
        // Word 2: first-instr offset = 3 header + 1 data word = 4
        let off = i64::from_le_bytes(out[16..24].try_into().unwrap());
        assert_eq!(off, 4);
    }

    #[test]
    fn instruction_count_matches() {
        use crate::byc::Byc;
        let bycs = vec![
            Byc::no_operand(Op::Add),
            Byc::no_operand(Op::Halt),
        ];
        let out = assemble(&bycs, &[], &HashMap::new(), None, 0);
        // 3 header + 4 startup + 2 program = 9 words × 8 bytes = 72
        assert_eq!(out.len(), 72);
    }

    #[test]
    fn startup_sequence_is_halt_call_push() {
        let out = assemble(&[], &[], &HashMap::new(), None, 0);
        // Startup begins at word 3 (byte 24).
        // Word 3: PUSH R_IMM <main_addr=0>
        assert_eq!(out[24], Op::Push as u8);
        assert_eq!(out[25], BycRegion::Imm as u8);
        // Word 4: PUSH imm:argc
        assert_eq!(out[32], Op::Push as u8);
        assert_eq!(out[33], BycRegion::Imm as u8);
        // Word 5: CALL imm:1
        assert_eq!(out[40], Op::Call as u8);
        assert_eq!(out[41], BycRegion::Imm as u8);
        // Word 6: HALT
        assert_eq!(out[48], Op::Halt as u8);
    }

    #[test]
    fn pad8_works() {
        assert_eq!(pad8(b"").len(), 0);
        assert_eq!(pad8(b"hello").len(), 8);
        assert_eq!(pad8(b"hello\0\0\0").len(), 8);
        assert_eq!(pad8(b"hello, world!").len(), 16);
    }
}