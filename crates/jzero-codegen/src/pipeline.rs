//! High-level bytecode compilation pipeline (Chapters 12/13).
//!
//! Sits between `generate()` (TAC) and `jzero-vm` (interpreter).
//! Collects the flat TAC list, translates it to bytecode, serialises the
//! string pool into the data section, and assembles the `.j0` binary image.

use std::collections::HashMap;

use jzero_ast::tree::Tree;

use crate::{
    bytecode::translate,
    context::CodegenContext,
    j0file::{assemble, disassemble_text},
};

/// Result of the bytecode compilation step.
pub struct BytecodeOutput {
    pub binary:      Vec<u8>,
    pub text:        String,
    pub main_offset: usize,
}

/// Compile a fully-generated `CodegenContext` into a `.j0` binary image.
/// `argc` is the number of command-line arguments to pass to main().
pub fn compile_bytecode(tree: &Tree, ctx: &CodegenContext, argc: i64) -> BytecodeOutput {
    // ── 1. Collect flat TAC ──────────────────────────────────────────────────
    let icode = collect_icode(tree, ctx);

    // ── 2. Serialize string pool → data section bytes ────────────────────────
    let data_bytes = build_data_section(ctx);

    // ── 3. Translate TAC → bytecode ──────────────────────────────────────────
    let (bycs, labeltable) = translate(&icode);

    // DEBUG: dump icode and bytecode
    for (i, t) in icode.iter().enumerate() {
        eprintln!("  tac[{:02}] {}", i, t);
    }
    for (i, b) in bycs.iter().enumerate() {
        eprintln!("  byc[{:02}] {}", i, b.text().trim());
    }

    // ── 4. Compute main's absolute byte offset ───────────────────────────────
    // The TAC for Jzero programs has no Lab before `proc main` — main is simply
    // the first instruction in the code section.  Its absolute byte offset is:
    //   (3 header words + data_words + 3 startup words) * 8
    // We compute data_words here so assemble() can use the same value.
    let data_padded_len = (data_bytes.len() + 7) & !7;
    let data_words   = data_padded_len / 8;
    let header_words = 3usize;
    let startup_words = 4usize;
    let main_offset  = (header_words + data_words + startup_words) * 8;

    // ── 5. Assemble ──────────────────────────────────────────────────────────
    let binary = assemble(&bycs, &data_bytes, &labeltable, Some(main_offset as i64), argc);
    let text   = disassemble_text(&bycs, &data_bytes);

    BytecodeOutput { binary, text, main_offset }
}

// ---------------------------------------------------------------------------
// TAC collection
// ---------------------------------------------------------------------------

/// Walk the tree and concatenate all icode vecs from MethodDecl blocks.
/// Returns a single flat Vec<Tac> representing the whole program.
fn collect_icode(tree: &Tree, ctx: &CodegenContext) -> Vec<crate::tac::Tac> {
    let mut out = Vec::new();
    collect_icode_rec(tree, ctx, &mut out);
    out
}

fn collect_icode_rec(tree: &Tree, ctx: &CodegenContext, out: &mut Vec<crate::tac::Tac>) {
    if tree.sym == "MethodDecl" {
        // Find the highest local offset used in this method's icode so we can
        // emit LOCAL n to pre-allocate stack space and prevent overlap between
        // locals and the expression stack.
        if let Some(block) = tree.kids.get(1) {
            if let Some(info) = ctx.node(block.id) {
                let max_local = max_local_offset(&info.icode);
                if max_local > 0 {
                    // Emit LOCAL n where n = number of 8-byte slots needed.
                    let n = max_local / 8;
                    out.push(crate::tac::Tac::new1(
                        crate::tac::Op::Proc,
                        crate::address::Address::imm(n),
                    ));
                }
                out.extend(info.icode.iter().cloned());
            }
        }
        // Append explicit RET.
        out.push(crate::tac::Tac::new1(
            crate::tac::Op::Ret,
            crate::address::Address::imm(0),
        ));
        return;
    }
    for kid in &tree.kids {
        collect_icode_rec(kid, ctx, out);
    }
}

// ---------------------------------------------------------------------------
// Data section serialization
// ---------------------------------------------------------------------------

/// Build the binary data section from the string pool.
///
/// Each string is written at its `string_offset` as NUL-terminated UTF-8,
/// padded so the next entry starts on an 8-byte boundary.
fn build_data_section(ctx: &CodegenContext) -> Vec<u8> {
    if ctx.strings.is_empty() {
        return Vec::new();
    }

    // Total size: last entry's offset + its padded length.
    let total = ctx.strings.iter().map(|e| {
        let raw = e.value.len() + 1; // +1 for NUL
        e.string_offset as usize + pad8_size(raw)
    }).max().unwrap_or(0);

    let mut data = vec![0u8; total];

    for entry in &ctx.strings {
        let off   = entry.string_offset as usize;
        let bytes = entry.value.as_bytes();
        let end   = off + bytes.len();
        if end <= data.len() {
            data[off..end].copy_from_slice(bytes);
            // NUL terminator already present (vec initialised to 0).
        }
    }

    data
}

/// Round `n` up to the next multiple of 8.
fn pad8_size(n: usize) -> usize {
    (n + 7) & !7
}

/// Find the highest `loc:N` offset used in an icode list.
/// This tells us how many stack slots to pre-allocate with LOCAL.
fn max_local_offset(icode: &[crate::tac::Tac]) -> i64 {
    use crate::address::{Address, Region};
    let mut max = 0i64;
    for instr in icode {
        for addr in [&instr.op1, &instr.op2, &instr.op3] {
            if let Some(Address::Regional { region: Region::Loc, offset }) = addr {
                if *offset > max { max = *offset; }
            }
        }
    }
    max
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jzero_ast::tree::reset_ids;
    use jzero_parser::parse_tree;
    use jzero_semantic::analyze;
    use crate::generate;

    fn compile(src: &str) -> BytecodeOutput {
        reset_ids();
        let mut tree = parse_tree(src).expect("parse failed");
        let sem = analyze(&mut tree);
        let ctx = generate(&tree, &sem);
        compile_bytecode(&tree, &ctx, 0)
    }

    #[test]
    fn hello_binary_has_magic() {
        let out = compile(r#"public class hello {
            public static void main(String argv[]) {
                System.out.println("hello, jzero!");
            }
        }"#);
        assert_eq!(&out.binary[0..8], b"Jzero!!\0", "magic word missing");
        assert_eq!(&out.binary[8..16], b"1.0\0\0\0\0\0", "version missing");
    }

    #[test]
    fn hello_text_has_sections() {
        let out = compile(r#"public class hello {
            public static void main(String argv[]) {
                System.out.println("hello, jzero!");
            }
        }"#);
        assert!(out.text.contains(".data"),    "missing .data section");
        assert!(out.text.contains(".code"),    "missing .code section");
        assert!(out.text.contains(".startup"), "missing .startup section");
    }

    #[test]
    fn data_section_contains_string() {
        let out = compile(r#"public class hello {
            public static void main(String argv[]) {
                System.out.println("hello, jzero!");
            }
        }"#);
        // "hello, jzero!" should appear in the data section bytes.
        let needle = b"hello, jzero!";
        let found = out.binary.windows(needle.len()).any(|w| w == needle);
        assert!(found, "string literal not found in binary");
    }

    #[test]
    fn binary_size_is_multiple_of_8() {
        let out = compile(r#"public class hello {
            public static void main(String argv[]) {
                System.out.println("hello, jzero!");
            }
        }"#);
        assert_eq!(out.binary.len() % 8, 0, "binary not word-aligned");
    }
}