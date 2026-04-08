//! Bytecode instruction representation (Chapter 13).
//!
//! Each `Byc` encodes one instruction in the Jzero bytecode format:
//! a fixed-width 8-byte word laid out as:
//!   [opcode: 1 byte][region: 1 byte][operand: 6 bytes, little-endian signed]
//!
//! `Op` constants mirror Table 12.1. `BycRegion` mirrors the R_* region codes
//! from Chapter 12.

use crate::address::{Address, Region};

// ---------------------------------------------------------------------------
// Opcodes (Table 12.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    Halt   = 1,
    Noop   = 2,
    Add    = 3,
    Sub    = 4,
    Mul    = 5,
    Div    = 6,
    Mod    = 7,
    Neg    = 8,
    Push   = 9,
    Pop    = 10,
    Call   = 11,
    Return = 12,
    Goto   = 13,
    Bif    = 14,
    Lt     = 15,
    Le     = 16,
    Gt     = 17,
    Ge     = 18,
    Eq     = 19,
    Neq    = 20,
    Local  = 21,
    Load   = 22,
    Store  = 23,
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Halt   => "halt",
            Op::Noop   => "noop",
            Op::Add    => "add",
            Op::Sub    => "sub",
            Op::Mul    => "mul",
            Op::Div    => "div",
            Op::Mod    => "mod",
            Op::Neg    => "neg",
            Op::Push   => "push",
            Op::Pop    => "pop",
            Op::Call   => "call",
            Op::Return => "return",
            Op::Goto   => "goto",
            Op::Bif    => "bif",
            Op::Lt     => "lt",
            Op::Le     => "le",
            Op::Gt     => "gt",
            Op::Ge     => "ge",
            Op::Eq     => "eq",
            Op::Neq    => "neq",
            Op::Local  => "local",
            Op::Load   => "load",
            Op::Store  => "store",
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1  => Some(Op::Halt),
            2  => Some(Op::Noop),
            3  => Some(Op::Add),
            4  => Some(Op::Sub),
            5  => Some(Op::Mul),
            6  => Some(Op::Div),
            7  => Some(Op::Mod),
            8  => Some(Op::Neg),
            9  => Some(Op::Push),
            10 => Some(Op::Pop),
            11 => Some(Op::Call),
            12 => Some(Op::Return),
            13 => Some(Op::Goto),
            14 => Some(Op::Bif),
            15 => Some(Op::Lt),
            16 => Some(Op::Le),
            17 => Some(Op::Gt),
            18 => Some(Op::Ge),
            19 => Some(Op::Eq),
            20 => Some(Op::Neq),
            21 => Some(Op::Local),
            22 => Some(Op::Load),
            23 => Some(Op::Store),
            _  => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Operand regions (Chapter 12, R_* constants)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BycRegion {
    None  = 0,  // no operand
    Abs   = 1,  // absolute word offset from magic word
    Imm   = 2,  // immediate value
    Stack = 3,  // word offset relative to bp (base pointer)
    Heap  = 4,  // word offset relative to hp (heap pointer)
}

impl BycRegion {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(BycRegion::None),
            1 => Some(BycRegion::Abs),
            2 => Some(BycRegion::Imm),
            3 => Some(BycRegion::Stack),
            4 => Some(BycRegion::Heap),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Byc — one bytecode instruction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Byc {
    pub op:     Op,
    pub region: BycRegion,
    pub opnd:   i64,
    /// True when this instruction's operand is an unresolved label id.
    /// Cleared by the second pass in `bytecode.rs` once the labeltable is built.
    pub needs_patch: bool,
}

impl Byc {
    /// Construct a `Byc` from an opcode and an optional TAC `Address`.
    /// Maps TAC address regions onto bytecode regions.
    pub fn new(op: Op, addr: Option<&Address>) -> Self {
        let (region, opnd, needs_patch) = match addr {
            None => (BycRegion::None, 0, false),
            Some(a) => map_address(a),
        };
        Byc { op, region, opnd, needs_patch }
    }

    /// Construct a no-operand instruction.
    pub fn no_operand(op: Op) -> Self {
        Byc { op, region: BycRegion::None, opnd: 0, needs_patch: false }
    }

    /// Construct with an immediate integer value (e.g. for `LOCAL n` or
    /// pushing the `-1` sentinel for `PrintStream__println`).
    pub fn imm(op: Op, val: i64) -> Self {
        Byc { op, region: BycRegion::Imm, opnd: val, needs_patch: false }
    }

    // -----------------------------------------------------------------------
    // Output
    // -----------------------------------------------------------------------

    /// Human-readable assembler text, one line (no newline).
    pub fn text(&self) -> String {
        let operand = match self.region {
            BycRegion::None  => String::new(),
            BycRegion::Abs   => format!(" @{:x}", self.opnd),
            BycRegion::Imm   => format!(" {}", self.opnd),
            BycRegion::Stack => format!(" stack:{}", self.opnd),
            BycRegion::Heap  => format!(" heap:{}", self.opnd),
        };
        format!("\t{}{}", self.op.name(), operand)
    }

    /// Encode as a single 8-byte little-endian word.
    ///
    /// Layout: [op:1][region:1][opnd:6 bytes, little-endian]
    pub fn binary(&self) -> [u8; 8] {
        let mut word = [0u8; 8];
        word[0] = self.op as u8;
        word[1] = self.region as u8;
        // Write the low 6 bytes of opnd in little-endian order into bytes 2..8.
        let x = self.opnd;
        for i in 0..6 {
            word[2 + i] = ((x >> (8 * i)) & 0xff) as u8;
        }
        word
    }

    /// Decode an 8-byte little-endian word into a `Byc`.
    /// Returns `None` if the opcode byte is unrecognised.
    pub fn from_binary(word: &[u8; 8]) -> Option<Self> {
        let op     = Op::from_u8(word[0])?;
        let region = BycRegion::from_u8(word[1]).unwrap_or(BycRegion::None);
        // Sign-extend 6 bytes → i64.
        let mut x: i64 = 0;
        // Read bytes 2..8 as a 48-bit signed integer (little-endian).
        for i in (0..6).rev() {
            x = (x << 8) | (word[2 + i] as i64);
        }
        // Sign-extend bit 47.
        if x & (1 << 47) != 0 {
            x |= !0xFFFF_FFFF_FFFFi64;
        }
        Some(Byc { op, region, opnd: x, needs_patch: false })
    }
}

// ---------------------------------------------------------------------------
// Address mapping (TAC → bytecode)
// ---------------------------------------------------------------------------

/// Maps a TAC `Address` to `(BycRegion, opnd, needs_patch)`.
///
/// - `Loc`     → `R_STACK`  (bp-relative slot, offset in bytes)
/// - `Global`  → `R_ABS`   (absolute word offset in data section)
/// - `Strings` → `R_ABS`   (absolute word offset in data section)
/// - `Lab`     → `R_ABS`   (label id stored in opnd; needs_patch = true)
/// - `Imm`     → `R_IMM`   (immediate value)
/// - `Self_`   → `R_STACK` (offset 0, the implicit self pointer)
/// - `Class`   → `R_ABS`   (class-region offset)
/// - `Symbol`  → `R_IMM`   (-1 sentinel; caller handles named symbols)
fn map_address(a: &Address) -> (BycRegion, i64, bool) {
    match a {
        Address::Regional { region, offset } => match region {
            Region::Loc     => (BycRegion::Stack, *offset, false),
            Region::Global  => (BycRegion::Abs,   *offset, false),
            Region::Strings => (BycRegion::Imm,   *offset, false), // offset into data section
            Region::Lab     => (BycRegion::Abs,   *offset, true),  // patch later
            Region::Imm     => (BycRegion::Imm,   *offset, false),
            Region::Class   => (BycRegion::Abs,   *offset, false),
            Region::Self_   => (BycRegion::Stack,  0,      false),
        },
        Address::Symbol(_) => (BycRegion::Imm, -1, false),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_binary() {
        let b = Byc::imm(Op::Push, 42);
        let encoded = b.binary();
        let decoded = Byc::from_binary(&encoded).unwrap();
        assert_eq!(decoded.op,     Op::Push);
        assert_eq!(decoded.region, BycRegion::Imm);
        assert_eq!(decoded.opnd,   42);
    }

    #[test]
    fn roundtrip_negative_immediate() {
        let b = Byc::imm(Op::Push, -1);
        let encoded = b.binary();
        let decoded = Byc::from_binary(&encoded).unwrap();
        assert_eq!(decoded.opnd, -1);
    }

    #[test]
    fn no_operand_binary() {
        let b = Byc::no_operand(Op::Add);
        let encoded = b.binary();
        assert_eq!(encoded[0], Op::Add as u8);
        assert_eq!(encoded[1], BycRegion::None as u8);
        assert_eq!(&encoded[2..], &[0u8; 6]);
    }

    #[test]
    fn text_format() {
        assert_eq!(Byc::imm(Op::Push, 42).text(),           "\tpush 42");
        assert_eq!(Byc::no_operand(Op::Add).text(),          "\tadd");
        assert_eq!(Byc::imm(Op::Local, 3).text(),            "\tlocal 3");
    }

    #[test]
    fn opcode_roundtrip() {
        for v in 1u8..=23 {
            let op = Op::from_u8(v).unwrap();
            assert_eq!(op as u8, v);
        }
    }
}