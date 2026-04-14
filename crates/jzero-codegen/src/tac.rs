use crate::address::Address;

/// A single three-address code instruction.
///
/// Each instruction has an opcode and 0–3 operands.
/// Not all opcodes use all operands; unused ones are `None`.
#[derive(Debug, Clone)]
pub struct Tac {
    pub op:  Op,
    pub op1: Option<Address>,
    pub op2: Option<Address>,
    pub op3: Option<Address>,
}

impl Tac {
    pub fn new0(op: Op) -> Self {
        Self { op, op1: None, op2: None, op3: None }
    }

    pub fn new1(op: Op, op1: Address) -> Self {
        Self { op, op1: Some(op1), op2: None, op3: None }
    }

    pub fn new2(op: Op, op1: Address, op2: Address) -> Self {
        Self { op, op1: Some(op1), op2: Some(op2), op3: None }
    }

    pub fn new3(op: Op, op1: Address, op2: Address, op3: Address) -> Self {
        Self { op, op1: Some(op1), op2: Some(op2), op3: Some(op3) }
    }
}

impl std::fmt::Display for Tac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.op1, &self.op2, &self.op3) {
            (None, _, _)             => write!(f, "{}", self.op),
            (Some(a), None, _)       => write!(f, "{} {}", self.op, a),
            (Some(a), Some(b), None) => write!(f, "{} {},{}", self.op, a, b),
            (Some(a), Some(b), Some(c)) =>
                write!(f, "{} {},{},{}", self.op, a, b, c),
        }
    }
}

/// All opcodes in the Jzero intermediate instruction set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    // ── Arithmetic ──────────────────────────────────────────────────────────
    /// op1 = op2 + op3
    Add,
    /// op1 = op2 - op3
    Sub,
    /// op1 = op2 * op3
    Mul,
    /// op1 = op2 / op3
    Div,
    /// op1 = op2 % op3
    Mod,
    /// op1 = -op2
    Neg,
    /// op1 = op2 ++ op3 (string concatenation)
    Sadd,

    // ── Data movement ───────────────────────────────────────────────────────
    /// op1 = op2  (assignment / copy)
    Asn,
    /// op1 = &op2 (address of)
    Addr,

    // ── Array / object ──────────────────────────────────────────────────────
    /// op1 = sizeof(array at op2)
    Asize,
    /// op1 = op2[op3]  (array load)
    Load,
    /// op1[op2] = op3  (array store)
    Store,
    /// op1 = alloc(op2 * wordsize)  (heap array allocation)
    NewArray,

    // ── Control flow ────────────────────────────────────────────────────────
    /// Unconditional jump to op1 (a label address)
    Goto,
    /// Define label op1 at this point in the instruction stream
    Lab,
    /// if op2 < op3 goto op1
    Blt,
    /// if op2 <= op3 goto op1
    Ble,
    /// if op2 > op3 goto op1
    Bgt,
    /// if op2 >= op3 goto op1
    Bge,
    /// if op2 == op3 goto op1
    Beq,
    /// if op2 != op3 goto op1
    Bne,

    // ── Method calls ────────────────────────────────────────────────────────
    /// Push parameter op1 onto the call stack
    Parm,
    /// Call method at op1 with op2 parameters
    Call,
    /// Return (from current method); op1 is optional return value
    Ret,

    /// op1 = String.valueOf(op2)  — convert integer to string pool key
    Itos,

    // ── Declarations (pseudo-instructions) ──────────────────────────────────
    /// Declare a global variable: name at address op1
    Global,
    /// Declare a string literal: op1 is the strings-region address
    StringDecl,
    /// Begin a procedure: name, param count, local size
    Proc,
    /// End of procedure
    End,
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Op::Add        => "ADD",
            Op::Sub        => "SUB",
            Op::Mul        => "MUL",
            Op::Div        => "DIV",
            Op::Mod        => "MOD",
            Op::Neg        => "NEG",
            Op::Sadd       => "SADD",
            Op::Asn        => "ASN",
            Op::Addr       => "ADDR",
            Op::Asize      => "ASIZE",
            Op::Load       => "LOAD",
            Op::Store      => "STORE",
            Op::NewArray   => "NEWARRAY",
            Op::Goto       => "GOTO",
            Op::Lab        => "LAB",
            Op::Blt        => "BLT",
            Op::Ble        => "BLE",
            Op::Bgt        => "BGT",
            Op::Bge        => "BGE",
            Op::Beq        => "BEQ",
            Op::Bne        => "BNE",
            Op::Parm       => "PARM",
            Op::Call       => "CALL",
            Op::Ret        => "RET",
            Op::Itos       => "ITOS",
            Op::Global     => "global",
            Op::StringDecl => "string",
            Op::Proc       => "proc",
            Op::End        => "end",
        };
        write!(f, "{s}")
    }
}