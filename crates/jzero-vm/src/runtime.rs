//! Jzero runtime built-in functions (Chapters 12 + 15).
//!
//! When `CALL` encounters a negative function address, it dispatches here.
//! The convention:
//!   -1  →  PrintStream__println(arg)

use crate::machine::J0Machine;

/// Dispatch a runtime call by function index.
pub fn dispatch(m: &mut J0Machine, f: i64) -> Result<(), String> {
    match f {
        -1 => do_println(m),
        _  => Err(format!("unknown runtime function: {}", f)),
    }
}

/// `System.out.println(arg)` — prints a string.
///
/// Stack layout when called:
///   sp   → arg  (string-pool key if from SPUSH/SADD, or data-section offset)
///   sp-1 → fn_addr sentinel (-1)
///
/// `resolve_string` handles both cases transparently.
fn do_println(m: &mut J0Machine) -> Result<(), String> {
    let arg = m.pop();
    let _fn = m.pop();  // fn_addr sentinel

    let line = m.resolve_string(arg);
    m.output.push_str(&line);
    m.output.push('\n');

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::J0Machine;

    fn make_machine_with_data(data: &[u8]) -> J0Machine {
        let mut image: Vec<u8> = Vec::new();
        image.extend_from_slice(b"Jzero!!\0");
        image.extend_from_slice(b"1.0\0\0\0\0\0");

        let mut padded = data.to_vec();
        let rem = padded.len() % 8;
        if rem != 0 { padded.resize(padded.len() + (8 - rem), 0); }
        let data_words = padded.len() / 8;

        let first_off = (3 + data_words) as i64;
        image.extend_from_slice(&first_off.to_le_bytes());
        image.extend_from_slice(&padded);

        let mut halt = [0u8; 8];
        halt[0] = 1;
        image.extend_from_slice(&halt);

        J0Machine::load(&image, 0).unwrap()
    }

    #[test]
    fn println_from_data_section() {
        let mut m = make_machine_with_data(b"hello\0\0\0");
        m.push(-1);  // fn sentinel
        m.push(0);   // data-section offset 0 → "hello"
        do_println(&mut m).unwrap();
        assert_eq!(m.output, "hello\n");
    }

    #[test]
    fn println_from_string_pool() {
        let mut m = make_machine_with_data(b"");
        let key = m.spool.put("world".to_string());
        m.push(-1);   // fn sentinel
        m.push(key);  // pool key → "world"
        do_println(&mut m).unwrap();
        assert_eq!(m.output, "world\n");
    }

    #[test]
    fn string_pool_concatenation() {
        let mut m = make_machine_with_data(b"");
        let k1 = m.spool.put("hello, ".to_string());
        let k2 = m.spool.put("jzero!".to_string());
        let s1 = m.spool.get(k1).unwrap().to_owned();
        let s2 = m.spool.get(k2).unwrap().to_owned();
        let k3 = m.spool.put(s1 + &s2);
        assert_eq!(m.spool.get(k3), Some("hello, jzero!"));
    }

    #[test]
    fn resolve_string_handles_both() {
        let mut m = make_machine_with_data(b"hi\0\0\0\0\0\0");
        // Data-section offset
        assert_eq!(m.resolve_string(0), "hi");
        // Pool key
        let key = m.spool.put("there".to_string());
        assert_eq!(m.resolve_string(key), "there");
    }
}