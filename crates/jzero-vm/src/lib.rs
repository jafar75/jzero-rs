//! Jzero bytecode interpreter (Chapter 12).
//!
//! Public entry point: `run(bytes, args)` takes a `.j0` binary image and
//! the command-line arguments to pass to main().

pub mod machine;
pub mod runtime;

pub use machine::J0Machine;

/// Execute a `.j0` binary image, passing `args` as argv to main().
/// Returns the collected stdout output.
pub fn run(bytes: &[u8], args: &[String]) -> Result<String, String> {
    let mut m = J0Machine::load(bytes, args.len() as i64)?;
    m.interp()
}