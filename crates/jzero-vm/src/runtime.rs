//! Jzero runtime built-in functions (Chapter 12).
//!
//! When `CALL` encounters a negative function address, it dispatches here.
//! The convention:
//!   -1  →  PrintStream__println(arg)
//!
//! The stack at dispatch time looks like:
//!   ... | fn_addr (-1) | arg0 | … | argN | saved_ip | saved_bp |
//!                                          ↑ bp points here after CALL setup
//!
//! For runtime calls we do NOT set up a real frame (CALL already skipped the
//! normal ip/bp save for negative addresses), so arguments sit just below TOS.

use crate::machine::J0Machine;

/// Dispatch a runtime call by function index.
pub fn dispatch(m: &mut J0Machine, f: i64) -> Result<(), String> {
    match f {
        -1 => do_println(m),
        _  => Err(format!("unknown runtime function: {}", f)),
    }
}

/// `System.out.println(arg)` — prints a string from the data section.
///
/// Stack layout when called (CALL has NOT saved ip/bp for negative calls):
///   sp → arg (strings-region byte offset or integer)
///   sp-1 → fn_addr (-1)
///
/// We pop the argument and print it.
fn do_println(m: &mut J0Machine) -> Result<(), String> {
    let arg = m.pop();
    let _fn = m.pop();

    // arg is the byte offset into the data section.
    let line = match m.read_string(arg as usize) {
        Ok(s)  => s,
        Err(_) => arg.to_string(),
    };

    // Append to output buffer (newline included, as println does).
    m.output.push_str(&line);
    m.output.push('\n');

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::J0Machine;

    /// Build a minimal machine with a known data section.
    fn make_machine_with_data(data: &[u8]) -> J0Machine {
        // Construct a minimal valid .j0 image with the given data section
        // and a single HALT instruction so interp() can run.
        let mut image: Vec<u8> = Vec::new();
        image.extend_from_slice(b"Jzero!!\0");   // magic
        image.extend_from_slice(b"1.0\0\0\0\0\0"); // version

        // Pad data to 8 bytes
        let mut padded = data.to_vec();
        let rem = padded.len() % 8;
        if rem != 0 { padded.resize(padded.len() + (8 - rem), 0); }
        let data_words = padded.len() / 8;

        // first-instr word offset = 3 (header) + data_words
        let first_off = (3 + data_words) as i64;
        image.extend_from_slice(&first_off.to_le_bytes());
        image.extend_from_slice(&padded);

        // HALT instruction
        let mut halt = [0u8; 8];
        halt[0] = 1; // Op::Halt
        image.extend_from_slice(&halt);

        J0Machine::load(&image, 0).unwrap()
    }

    #[test]
    fn println_appends_to_output() {
        let mut m = make_machine_with_data(b"hello\0\0\0");
        // Manually push fn_addr then arg.
        m.push(-1);  // fn sentinel
        m.push(0);   // byte offset 0 in data → "hello"
        do_println(&mut m).unwrap();
        assert_eq!(m.output, "hello\n");
    }

    #[test]
    fn println_integer_fallback() {
        let mut m = make_machine_with_data(b"");
        m.push(-1);
        m.push(99999); // not a valid string offset → printed as integer
        do_println(&mut m).unwrap();
        assert_eq!(m.output, "99999\n");
    }
}