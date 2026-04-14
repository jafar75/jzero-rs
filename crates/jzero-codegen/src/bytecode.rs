//! TAC → bytecode translation (Chapter 13).
//!
//! # Two-pass algorithm
//!
//! ## Pass 1 — emit
//! Walk the flat `&[Tac]` list produced by `gencode`. For each TAC instruction
//! emit one or more `Byc` instructions onto `rv`.  When a `Lab` address is used
//! as an operand (branch target, GOTO destination) the `Byc` is emitted with
//! `needs_patch = true` and `opnd` holding the *label id* (not a byte offset).
//! Simultaneously, whenever a `LAB` pseudo-instruction is encountered its label
//! id is mapped to the current byte offset and recorded in `labeltable`.
//!
//! ## Pass 2 — patch
//! Walk `rv` again.  Every `Byc` whose `needs_patch` flag is set has its `opnd`
//! replaced by the byte offset looked up from `labeltable`.

use std::collections::HashMap;

use crate::{
    address::{Address, Region},
    byc::{Byc, Op},
    tac::{Op as TacOp, Tac},
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Translate a slice of TAC instructions into bytecode.
///
/// Returns:
/// - `Vec<Byc>` — the bytecode instruction sequence (all labels resolved)
/// - `HashMap<i64, usize>` — labeltable mapping label-id → byte offset
pub fn translate(icode: &[Tac]) -> (Vec<Byc>, HashMap<i64, usize>) {
    let (mut bycs, labeltable) = pass1(icode);
    pass2(&mut bycs, &labeltable);
    (bycs, labeltable)
}

// ---------------------------------------------------------------------------
// Pass 1 — emit
// ---------------------------------------------------------------------------

fn pass1(icode: &[Tac]) -> (Vec<Byc>, HashMap<i64, usize>) {
    let mut rv: Vec<Byc> = Vec::new();
    let mut labeltable: HashMap<i64, usize> = HashMap::new();
    // Tracks whether the method address has been pushed ahead of the first
    // PARM in a call sequence (reset to false after each CALL).
    let mut method_addr_pushed = false;

    for (i, instr) in icode.iter().enumerate() {
        match instr.op {
            // ----------------------------------------------------------------
            // Arithmetic — binary: PUSH op2, PUSH op3, <op>, POP op1
            // ----------------------------------------------------------------
            TacOp::Add => emit_binary(&mut rv, Op::Add,  instr),
            TacOp::Sub => emit_binary(&mut rv, Op::Sub,  instr),
            TacOp::Mul => emit_binary(&mut rv, Op::Mul,  instr),
            TacOp::Div => emit_binary(&mut rv, Op::Div,  instr),
            TacOp::Mod => emit_binary(&mut rv, Op::Mod,  instr),

            // ----------------------------------------------------------------
            // Arithmetic — unary: PUSH op2, <op>, POP op1
            // ----------------------------------------------------------------
            TacOp::Neg => emit_unary(&mut rv, Op::Neg, instr),

            // SADD — string concatenation (Chapter 15).
            // SPUSH op2, SPUSH op3, SADD, SPOP op1
            TacOp::Sadd => {
                rv.push(Byc::new(Op::Spush, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Spush, instr.op3.as_ref()));
                rv.push(Byc::no_operand(Op::Sadd));
                rv.push(Byc::new(Op::Spop,  instr.op1.as_ref()));
            }

            // ----------------------------------------------------------------
            // Assignment: PUSH op2, POP op1
            // ----------------------------------------------------------------
            TacOp::Asn => {
                rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
            }

            // ----------------------------------------------------------------
            // Conditional branches — PUSH op2, PUSH op3, <cmp>, BIF op1
            // ----------------------------------------------------------------
            TacOp::Blt => emit_branch(&mut rv, Op::Lt, instr),
            TacOp::Ble => emit_branch(&mut rv, Op::Le, instr),
            TacOp::Bgt => emit_branch(&mut rv, Op::Gt, instr),
            TacOp::Bge => emit_branch(&mut rv, Op::Ge, instr),
            TacOp::Beq => emit_branch(&mut rv, Op::Eq, instr),
            TacOp::Bne => emit_branch(&mut rv, Op::Neq, instr),

            // ----------------------------------------------------------------
            // Unconditional branch
            // ----------------------------------------------------------------
            TacOp::Goto => {
                rv.push(Byc::new(Op::Goto, instr.op1.as_ref()));
            }

            // ----------------------------------------------------------------
            // Label pseudo-instruction — record byte offset, emit nothing
            // ----------------------------------------------------------------
            TacOp::Lab => {
                if let Some(Address::Regional { region: Region::Lab, offset }) =
                    &instr.op1
                {
                    labeltable.insert(*offset, rv.len() * 8);
                }
            }

            // ----------------------------------------------------------------
            // PARM / CALL — stack-machine calling convention.
            //
            // The bytecode stack machine requires the method address to sit
            // *below* the arguments.  TAC doesn't encode this, so on the first
            // PARM in a call sequence we look ahead to find the matching CALL
            // and push the method address first.
            // ----------------------------------------------------------------
            TacOp::Parm => {
                // Skip global-region PARMs — these are object receivers (e.g.
                // System) that the bytecode calling convention does not pass
                // explicitly; only the string/value arguments are pushed.
                if matches!(&instr.op1, Some(Address::Regional { region: Region::Global, .. })) {
                    continue;
                }
                if !method_addr_pushed {
                    if let Some(call_addr) = find_call_addr(icode, i) {
                        rv.push(call_addr);
                    }
                    method_addr_pushed = true;
                }
                rv.push(Byc::new(Op::Push, instr.op1.as_ref()));
            }

            TacOp::Call => {
                // op2 holds the arg count (as an Imm address).
                rv.push(Byc::new(Op::Call, instr.op2.as_ref()));
                method_addr_pushed = false;
            }

            // ----------------------------------------------------------------
            // Return
            // ----------------------------------------------------------------
            TacOp::Ret => {
                rv.push(Byc::new(Op::Return, instr.op1.as_ref()));
            }

            // ----------------------------------------------------------------
            // Proc — allocate frame slots with LOCAL n.
            // op2 carries the number of local words as an Imm.
            // ----------------------------------------------------------------
            TacOp::Proc => {
                let n = imm_value(instr.op1.as_ref());
                rv.push(Byc::imm(Op::Local, n));
            }

            // ----------------------------------------------------------------
            // End — no bytecode emitted
            // ----------------------------------------------------------------
            TacOp::End => {}

            // ----------------------------------------------------------------
            // Pointer / indirect ops
            // ----------------------------------------------------------------

            // ADDR: turn an address into a value (indirect push).
            TacOp::Addr => {
                rv.push(Byc::new(Op::Load, instr.op1.as_ref()));
            }

            // ASIZE: get array length. In our VM the argv slot holds the
            // length directly (not a pointer), so treat as a direct copy.
            TacOp::Asize => {
                rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
            }

            // NewArray: treated as ASN for now (address already computed
            // by the runtime; a real heap allocator would go here).
            TacOp::NewArray => {
                rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
            }

            // Load through pointer (LCON equivalent).
            TacOp::Load => {
                rv.push(Byc::new(Op::Load, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
            }

            // Store through pointer (SCON equivalent).
            TacOp::Store => {
                rv.push(Byc::new(Op::Store, instr.op2.as_ref()));
                rv.push(Byc::new(Op::Pop,   instr.op1.as_ref()));
            }

            // ----------------------------------------------------------------
            // Global / StringDecl — data-section declarations, no code emitted
            // ----------------------------------------------------------------
            TacOp::Global | TacOp::StringDecl => {}

            TacOp::Itos => {
                // PUSH the integer, ITOS converts TOS to a string pool key.
                rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
                rv.push(Byc::no_operand(Op::Itos));
                rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
            }
        }
    }

    (rv, labeltable)
}

// ---------------------------------------------------------------------------
// Pass 2 — patch label references
// ---------------------------------------------------------------------------

fn pass2(bycs: &mut Vec<Byc>, labeltable: &HashMap<i64, usize>) {
    for b in bycs.iter_mut() {
        if b.needs_patch {
            if let Some(&byte_off) = labeltable.get(&b.opnd) {
                b.opnd = byte_off as i64;
                b.needs_patch = false;
            }
            // If the label wasn't found (shouldn't happen in well-formed TAC)
            // leave needs_patch = true so callers can detect it.
        }
    }
}

// ---------------------------------------------------------------------------
// Emit helpers
// ---------------------------------------------------------------------------

/// Binary arithmetic: PUSH op2, PUSH op3, <bop>, POP op1
fn emit_binary(rv: &mut Vec<Byc>, bop: Op, instr: &Tac) {
    rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
    rv.push(Byc::new(Op::Push, instr.op3.as_ref()));
    rv.push(Byc::no_operand(bop));
    rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
}

/// Unary arithmetic: PUSH op2, <uop>, POP op1
fn emit_unary(rv: &mut Vec<Byc>, uop: Op, instr: &Tac) {
    rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
    rv.push(Byc::no_operand(uop));
    rv.push(Byc::new(Op::Pop,  instr.op1.as_ref()));
}

/// Conditional branch: PUSH op2, PUSH op3, <cmp>, BIF op1
fn emit_branch(rv: &mut Vec<Byc>, cmp: Op, instr: &Tac) {
    rv.push(Byc::new(Op::Push, instr.op2.as_ref()));
    rv.push(Byc::new(Op::Push, instr.op3.as_ref()));
    rv.push(Byc::no_operand(cmp));
    rv.push(Byc::new(Op::Bif,  instr.op1.as_ref()));
}

// ---------------------------------------------------------------------------
// Lookahead helpers
// ---------------------------------------------------------------------------

/// Scan forward from `start` to find the nearest CALL instruction and return
/// a `Byc` that pushes the method address.
///
/// - Named symbols (e.g. `PrintStream__println`) → `PUSH imm:-1`
/// - All other addresses → `PUSH <addr>`
fn find_call_addr(icode: &[Tac], start: usize) -> Option<Byc> {
    for instr in &icode[start + 1..] {
        if instr.op == TacOp::Call {
            // op1 is the method name/address in our TAC (see gencode.rs).
            return Some(match &instr.op1 {
                Some(Address::Symbol(_)) => Byc::imm(Op::Push, -1),
                other => Byc::new(Op::Push, other.as_ref()),
            });
        }
    }
    None
}

/// Extract the integer value from an `Imm` address, defaulting to 0.
fn imm_value(addr: Option<&Address>) -> i64 {
    match addr {
        Some(Address::Regional { region: Region::Imm, offset }) => *offset,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        address::Address,
        byc::BycRegion,
        tac::Tac,
    };

    fn lab(id: i64)  -> Address { Address::lab(id) }
    fn imm(v: i64)   -> Address { Address::imm(v) }
    fn loc(o: i64)   -> Address { Address::loc(o) }

    // Helper: build a minimal TAC instruction.
    fn tac(op: TacOp, op1: Option<Address>, op2: Option<Address>, op3: Option<Address>) -> Tac {
        Tac { op, op1, op2, op3 }
    }

    #[test]
    fn add_translates_to_four_instructions() {
        let icode = vec![
            tac(TacOp::Add, Some(loc(16)), Some(loc(8)), Some(imm(2))),
        ];
        let (bycs, _) = translate(&icode);
        assert_eq!(bycs.len(), 4);
        assert_eq!(bycs[0].op, Op::Push);
        assert_eq!(bycs[1].op, Op::Push);
        assert_eq!(bycs[2].op, Op::Add);
        assert_eq!(bycs[3].op, Op::Pop);
    }

    #[test]
    fn asn_translates_to_two_instructions() {
        let icode = vec![
            tac(TacOp::Asn, Some(loc(16)), Some(imm(5)), None),
        ];
        let (bycs, _) = translate(&icode);
        assert_eq!(bycs.len(), 2);
        assert_eq!(bycs[0].op, Op::Push);
        assert_eq!(bycs[1].op, Op::Pop);
    }

    #[test]
    fn label_recorded_in_labeltable() {
        let icode = vec![
            tac(TacOp::Lab, Some(lab(3)), None, None),
        ];
        let (bycs, labeltable) = translate(&icode);
        assert_eq!(bycs.len(), 0);
        assert_eq!(labeltable[&3], 0);
    }

    #[test]
    fn goto_patched_to_byte_offset() {
        // LAB 5 at instruction 0 (byte offset 0), then GOTO L5
        let icode = vec![
            tac(TacOp::Lab,  Some(lab(5)), None, None),
            tac(TacOp::Goto, Some(lab(5)), None, None),
        ];
        let (bycs, _) = translate(&icode);
        assert_eq!(bycs.len(), 1);
        assert_eq!(bycs[0].op,     Op::Goto);
        assert_eq!(bycs[0].region, BycRegion::Abs);
        assert_eq!(bycs[0].opnd,   0);   // byte offset 0
        assert!(!bycs[0].needs_patch);
    }

    #[test]
    fn branch_translates_to_four_instructions() {
        let icode = vec![
            tac(TacOp::Lab, Some(lab(1)), None, None),
            tac(TacOp::Bgt, Some(lab(1)), Some(loc(8)), Some(imm(3))),
        ];
        let (bycs, _) = translate(&icode);
        // LAB emits nothing; BGT emits PUSH, PUSH, GT, BIF
        assert_eq!(bycs.len(), 4);
        assert_eq!(bycs[2].op, Op::Gt);
        assert_eq!(bycs[3].op, Op::Bif);
        assert_eq!(bycs[3].opnd, 0); // patched to byte offset of LAB 1
    }

    #[test]
    fn parm_call_pushes_method_addr_first() {
        // println call: PARM strings:0, CALL PrintStream__println 1
        let icode = vec![
            tac(TacOp::Parm, Some(Address::strings(0)), None, None),
            tac(TacOp::Call,
                Some(Address::symbol("PrintStream__println")),
                Some(imm(1)),
                None),
        ];
        let (bycs, _) = translate(&icode);
        // Expected: PUSH imm:-1 (method addr), PUSH strings:0 (arg), CALL imm:1
        assert_eq!(bycs.len(), 3);
        assert_eq!(bycs[0].op,   Op::Push);
        assert_eq!(bycs[0].opnd, -1);           // sentinel for println
        assert_eq!(bycs[1].op,   Op::Push);     // the argument
        assert_eq!(bycs[2].op,   Op::Call);
    }
}