//! The Jzero bytecode interpreter machine (Chapters 12 + 15).
//!
//! # Memory model
//!
//! ```text
//! code        – the full .j0 image as bytes
//! data        – static data section (string literals, NUL-terminated)
//! stack       – fn_addr, args, locals, temporaries (integers and string keys)
//! string_pool – runtime string storage: maps i64 key ↔ String value
//!               String keys are negative integers (-1, -2, …) so they never
//!               collide with data-section byte offsets (which are ≥ 0).
//! ```
//!
//! # String representation
//!
//! String literals live in the data section as NUL-terminated UTF-8.
//! Their TAC address is a `Strings`-region offset (≥ 0).
//!
//! At runtime, `SPUSH` reads the raw string from the data section and interns
//! it in the `StringPool`, pushing the resulting negative key onto the stack.
//! `SADD` pops two keys, concatenates the strings, interns the result, and
//! pushes the new key.  `SPOP` pops a key and stores it back to a stack slot
//! (the string value stays in the pool).
//!
//! `do_println` checks whether the value on the stack is a pool key (< 0) or
//! a data-section offset (≥ 0) and reads the string accordingly.
//!
//! # Calling convention
//!
//! Before CALL n the stack looks like:
//!   ... | fn_addr | arg0 | … | argN |
//!         ↑ fn_slot              ↑ sp
//!
//! CALL saves (ip, bp, fn_slot) onto the off-stack `call_stack`, sets
//! bp = fn_slot, ip = fn_addr.
//!
//! RETURN pops (saved_ip, saved_bp, fn_slot), restores ip and bp, and sets
//! sp = fn_slot - 1 to clean up the entire frame.

use std::collections::HashMap;
use jzero_codegen::byc::{Byc, BycRegion, Op};

const STACK_WORDS: usize = 100_000;
const MAGIC:   &[u8; 8] = b"Jzero!!\0";
const VERSION: &[u8; 8] = b"1.0\0\0\0\0\0";

// ---------------------------------------------------------------------------
// String pool (Chapter 15)
// ---------------------------------------------------------------------------

/// Runtime string storage.
///
/// Keys are negative i64 values (-1, -2, …) so they are visually distinct
/// from data-section offsets (≥ 0) when inspecting the stack.
pub struct StringPool {
    by_key:    HashMap<i64, String>,
    by_value:  HashMap<String, i64>,
    next_key:  i64,   // decrements: -1, -2, …
}

impl StringPool {
    pub fn new() -> Self {
        StringPool {
            by_key:   HashMap::new(),
            by_value: HashMap::new(),
            next_key: -1,
        }
    }

    /// Intern a string, returning a unique negative key.
    /// If the string is already in the pool, returns the existing key.
    pub fn put(&mut self, s: String) -> i64 {
        if let Some(&key) = self.by_value.get(&s) {
            return key;
        }
        let key = self.next_key;
        self.next_key -= 1;
        self.by_key.insert(key, s.clone());
        self.by_value.insert(s, key);
        key
    }

    /// Retrieve a string by key.  Returns `None` if the key is unknown.
    pub fn get(&self, key: i64) -> Option<&str> {
        self.by_key.get(&key).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// J0Machine
// ---------------------------------------------------------------------------

pub struct J0Machine {
    code:        Vec<u8>,
    data:        Vec<u8>,
    stack:       Vec<i64>,
    ip:          usize,
    sp:          i64,
    bp:          i64,
    call_stack:  Vec<(usize, i64, i64)>,
    #[allow(dead_code)]
    hp:          i64,
    /// Runtime string pool (Chapter 15).
    pub spool:   StringPool,
    pub output:  String,
}

impl J0Machine {
    // -----------------------------------------------------------------------
    // Load
    // -----------------------------------------------------------------------

    pub fn load(bytes: &[u8], argc: i64) -> Result<Self, String> {
        if bytes.len() < 24 {
            return Err("image too small".into());
        }
        if &bytes[0..8] != MAGIC {
            return Err(format!("bad magic: {:?}", &bytes[0..8]));
        }
        if &bytes[8..16] != VERSION {
            return Err(format!("bad version: {:?}", &bytes[8..16]));
        }

        let first_word_off   = read_i64(bytes, 16) as usize;
        let first_instr_byte = first_word_off * 8;

        if first_instr_byte > bytes.len() {
            return Err("first-instruction offset out of range".into());
        }

        let data  = bytes[24..first_instr_byte].to_vec();
        let stack = vec![0i64; STACK_WORDS];
        let _ = argc; // argc passed via startup sequence

        Ok(J0Machine {
            code:       bytes.to_vec(),
            data,
            stack,
            ip:         first_instr_byte,
            sp:         -1,
            bp:         -1,
            call_stack: Vec::new(),
            hp:         0,
            spool:      StringPool::new(),
            output:     String::new(),
        })
    }

    // -----------------------------------------------------------------------
    // Fetch-decode-execute loop
    // -----------------------------------------------------------------------

    pub fn interp(&mut self) -> Result<String, String> {
        loop {
            let word = self.fetch()?;
            let byc  = Byc::from_binary(&word)
                .ok_or_else(|| format!("illegal opcode 0x{:02x} at ip={}", word[0], self.ip - 8))?;

            match byc.op {
                Op::Halt => break,
                Op::Noop => {}

                // ── Integer arithmetic ──────────────────────────────────
                Op::Add => { let (b,a) = self.pop2(); self.push(a + b); }
                Op::Sub => { let (b,a) = self.pop2(); self.push(a - b); }
                Op::Mul => { let (b,a) = self.pop2(); self.push(a * b); }
                Op::Div => {
                    let (b,a) = self.pop2();
                    if b == 0 { return Err("division by zero".into()); }
                    self.push(a / b);
                }
                Op::Mod => {
                    let (b,a) = self.pop2();
                    if b == 0 { return Err("modulo by zero".into()); }
                    self.push(a % b);
                }
                Op::Neg => { let a = self.pop(); self.push(-a); }

                // ── String operations (Chapter 15) ───────────────────────
                //
                // SPUSH: resolve the operand to a data-section byte offset
                //        (always R_IMM for Strings-region addresses), read
                //        the NUL-terminated string, intern it in the pool,
                //        and push the negative pool key.
                Op::Spush => {
                    let offset = self.deref(byc.region, byc.opnd)? as usize;
                    let s = self.read_string(offset)?;
                    let key = self.spool.put(s);
                    self.push(key);
                }

                // SPOP: pop a pool key from the stack and store it into the
                //       destination stack slot (the string stays in the pool).
                Op::Spop => {
                    let key = self.pop();
                    self.assign(byc.region, byc.opnd, key)?;
                }

                // SADD: pop two pool keys, concatenate the strings, intern
                //       the result, and push the new key.
                Op::Sadd => {
                    let key_b = self.pop();
                    let key_a = self.pop();
                    let s_a = self.spool.get(key_a)
                        .ok_or_else(|| format!("SADD: unknown key {}", key_a))?
                        .to_owned();
                    let s_b = self.spool.get(key_b)
                        .ok_or_else(|| format!("SADD: unknown key {}", key_b))?
                        .to_owned();
                    let result = s_a + &s_b;
                    let key = self.spool.put(result);
                    self.push(key);
                }

                // ── Comparisons ─────────────────────────────────────────
                Op::Lt  => { let (b,a) = self.pop2(); self.push((a <  b) as i64); }
                Op::Le  => { let (b,a) = self.pop2(); self.push((a <= b) as i64); }
                Op::Gt  => { let (b,a) = self.pop2(); self.push((a >  b) as i64); }
                Op::Ge  => { let (b,a) = self.pop2(); self.push((a >= b) as i64); }
                Op::Eq  => { let (b,a) = self.pop2(); self.push((a == b) as i64); }
                Op::Neq => { let (b,a) = self.pop2(); self.push((a != b) as i64); }

                // ── Stack ops ───────────────────────────────────────────
                Op::Push => {
                    let v = self.deref(byc.region, byc.opnd)?;
                    self.push(v);
                }
                Op::Pop => {
                    let v = self.pop();
                    self.assign(byc.region, byc.opnd, v)?;
                }

                // ── Frame allocation ────────────────────────────────────
                Op::Local => {
                    let n = byc.opnd as usize;
                    for _ in 0..n { self.push(0); }
                }

                // ── Indirect ops ────────────────────────────────────────
                Op::Load => {
                    let ptr = self.deref(byc.region, byc.opnd)? as usize;
                    let v   = self.read_data(ptr)?;
                    self.push(v);
                }
                Op::Store => {
                    let ptr = self.deref(byc.region, byc.opnd)? as usize;
                    let v   = self.pop();
                    self.write_data(ptr, v)?;
                }

                // ── Control flow ────────────────────────────────────────
                Op::Goto => {
                    self.ip = byc.opnd as usize;
                }
                Op::Bif => {
                    let cond = self.pop();
                    if cond != 0 {
                        self.ip = byc.opnd as usize;
                    }
                }

                // ── Call / return ───────────────────────────────────────
                Op::Call => {
                    let n       = byc.opnd as i64;
                    let fn_slot = self.sp - n;
                    let f       = self.stack[fn_slot as usize];

                    if f >= 0 {
                        self.call_stack.push((self.ip, self.bp, fn_slot));
                        self.bp = fn_slot;
                        self.ip = f as usize;
                    } else {
                        crate::runtime::dispatch(self, f)?;
                    }
                }
                Op::Return => {
                    let (saved_ip, saved_bp, fn_slot) =
                        self.call_stack.pop()
                            .ok_or_else(|| "RETURN with empty call stack".to_string())?;
                    self.ip = saved_ip;
                    self.bp = saved_bp;
                    self.sp = fn_slot - 1;
                }
            }
        }

        Ok(self.output.clone())
    }

    // -----------------------------------------------------------------------
    // Memory operations
    // -----------------------------------------------------------------------

    pub fn deref(&self, region: BycRegion, opnd: i64) -> Result<i64, String> {
        match region {
            BycRegion::None  => Ok(0),
            BycRegion::Imm   => Ok(opnd),
            BycRegion::Abs   => self.read_code(opnd as usize),
            BycRegion::Stack => {
                let idx = self.bp + opnd / 8;
                self.read_stack(idx)
            }
            BycRegion::Heap  => Err("heap not yet implemented".into()),
        }
    }

    pub fn assign(&mut self, region: BycRegion, opnd: i64, val: i64) -> Result<(), String> {
        match region {
            BycRegion::Stack => {
                let idx = self.bp + opnd / 8;
                self.write_stack(idx, val)
            }
            BycRegion::Abs => self.write_code(opnd as usize, val),
            _ => Err(format!("cannot assign to region {:?}", region)),
        }
    }

    // -----------------------------------------------------------------------
    // Stack helpers
    // -----------------------------------------------------------------------

    pub fn push(&mut self, v: i64) {
        self.sp += 1;
        self.stack[self.sp as usize] = v;
    }

    pub fn pop(&mut self) -> i64 {
        let v = self.stack[self.sp as usize];
        self.sp -= 1;
        v
    }

    fn pop2(&mut self) -> (i64, i64) {
        let b = self.pop();
        let a = self.pop();
        (b, a)
    }

    fn read_stack(&self, idx: i64) -> Result<i64, String> {
        if idx < 0 || idx as usize >= self.stack.len() {
            return Err(format!("stack index out of range: {}", idx));
        }
        Ok(self.stack[idx as usize])
    }

    fn write_stack(&mut self, idx: i64, val: i64) -> Result<(), String> {
        if idx < 0 || idx as usize >= self.stack.len() {
            return Err(format!("stack index out of range: {}", idx));
        }
        self.stack[idx as usize] = val;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Code / data region helpers
    // -----------------------------------------------------------------------

    fn read_code(&self, off: usize) -> Result<i64, String> {
        if off + 8 > self.code.len() {
            return Err(format!("code read out of range: off={}", off));
        }
        Ok(read_i64(&self.code, off))
    }

    fn write_code(&mut self, off: usize, val: i64) -> Result<(), String> {
        if off + 8 > self.code.len() {
            return Err(format!("code write out of range: off={}", off));
        }
        self.code[off..off + 8].copy_from_slice(&val.to_le_bytes());
        Ok(())
    }

    pub fn read_data(&self, off: usize) -> Result<i64, String> {
        if off + 8 > self.data.len() {
            return self.read_code(off);
        }
        Ok(read_i64(&self.data, off))
    }

    fn write_data(&mut self, off: usize, val: i64) -> Result<(), String> {
        if off + 8 > self.data.len() {
            return Err(format!("data write out of range: off={}", off));
        }
        self.data[off..off + 8].copy_from_slice(&val.to_le_bytes());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Fetch
    // -----------------------------------------------------------------------

    fn fetch(&mut self) -> Result<[u8; 8], String> {
        if self.ip + 8 > self.code.len() {
            return Err(format!("ip out of range: {}", self.ip));
        }
        let mut word = [0u8; 8];
        word.copy_from_slice(&self.code[self.ip..self.ip + 8]);
        self.ip += 8;
        Ok(word)
    }

    // -----------------------------------------------------------------------
    // String helpers (used by runtime and SPUSH)
    // -----------------------------------------------------------------------

    /// Read a NUL-terminated UTF-8 string from the data section at byte offset `off`.
    pub fn read_string(&self, off: usize) -> Result<String, String> {
        if off >= self.data.len() {
            return Err(format!("string offset out of range: {}", off));
        }
        let end = self.data[off..]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.data.len() - off);
        String::from_utf8(self.data[off..off + end].to_vec())
            .map_err(|e| format!("invalid utf8 in string: {}", e))
    }

    /// Resolve a stack value to a printable string.
    ///
    /// - Negative values are string-pool keys (set by SPUSH/SADD).
    /// - Non-negative values are data-section byte offsets (set by PUSH for
    ///   Strings-region addresses used directly with println).
    pub fn resolve_string(&self, val: i64) -> String {
        if val < 0 {
            self.spool.get(val)
                .map(|s| s.to_owned())
                .unwrap_or_else(|| format!("<unknown key {}>", val))
        } else {
            self.read_string(val as usize)
                .unwrap_or_else(|_| val.to_string())
        }
    }

    pub fn peek(&self) -> i64 { self.stack[self.sp as usize] }
    pub fn sp(&self)   -> i64 { self.sp }
}

fn read_i64(bytes: &[u8], off: usize) -> i64 {
    i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
}