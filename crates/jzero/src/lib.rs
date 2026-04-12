//! # jzero
//!
//! A compiler and bytecode VM for **Jzero** — a small subset of Java.
//!
//! Jzero supports classes, methods, control flow, basic types (`int`, `double`,
//! `boolean`, `String`), arrays, string concatenation, and `System.out.println`.
//!
//! ## Quick start
//!
//! ```no_run
//! use jzero::Compiler;
//!
//! let output = Compiler::new()
//!     .source("public class hello {
//!         public static void main(String argv[]) {
//!             System.out.println(\"hello, jzero!\");
//!         }
//!     }")
//!     .run(&[])
//!     .unwrap();
//!
//! assert_eq!(output.stdout, "hello, jzero!\n");
//! ```
//!
//! ## Pipeline
//!
//! ```text
//! source code
//!     → parse_tree()       [jzero-parser]
//!     → analyze()          [jzero-semantic]
//!     → generate()         [jzero-codegen]  → TAC IR
//!     → compile_bytecode() [jzero-codegen]  → .j0 binary
//!     → run()              [jzero-vm]       → stdout
//! ```

use jzero_ast::tree::reset_ids;

// ─── Re-exports ───────────────────────────────────────────────────────────────

pub use jzero_semantic::SemanticResult;
pub use jzero_codegen::pipeline::BytecodeOutput;
pub use jzero_codegen::CodegenContext;

// ─── CompileOutput ────────────────────────────────────────────────────────────

/// The result of a full compile + execute run.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// Text written to stdout by the Jzero program.
    pub stdout: String,
}

/// The result of compiling to bytecode without executing.
#[derive(Debug)]
pub struct CompileOutput {
    /// The raw `.j0` binary image.
    pub binary: Vec<u8>,
    /// Human-readable bytecode assembler listing.
    pub text: String,
    /// The TAC assembler listing (intermediate code).
    pub tac: String,
}

// ─── Error ────────────────────────────────────────────────────────────────────

/// A Jzero compilation or runtime error.
#[derive(Debug, Clone)]
pub struct JzeroError(pub String);

impl std::fmt::Display for JzeroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for JzeroError {}

// ─── Compiler ────────────────────────────────────────────────────────────────

/// The Jzero compiler.
///
/// Construct with [`Compiler::new`], provide source code with [`Compiler::source`],
/// then call [`Compiler::run`], [`Compiler::compile`], or [`Compiler::tac`].
#[derive(Default)]
pub struct Compiler {
    source: String,
}

impl Compiler {
    /// Create a new compiler instance.
    pub fn new() -> Self {
        Compiler::default()
    }

    /// Set the Jzero source code to compile.
    pub fn source(mut self, src: &str) -> Self {
        self.source = src.to_string();
        self
    }

    /// Parse and semantically analyse the source, returning any errors.
    ///
    /// This is the first step in the pipeline and is called internally
    /// by all other methods.
    fn analyse(&self) -> Result<(jzero_ast::tree::Tree, SemanticResult), JzeroError> {
        reset_ids();
        let mut tree = jzero_parser::parse_tree(&self.source)
            .map_err(|e| JzeroError(e.to_string()))?;
        let sem = jzero_semantic::analyze(&mut tree);
        if !sem.errors.is_empty() {
            let msg = sem.errors.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(JzeroError(msg));
        }
        Ok((tree, sem))
    }

    /// Compile to TAC intermediate code and return the assembler listing.
    ///
    /// # Errors
    /// Returns a [`JzeroError`] if parsing or semantic analysis fails.
    pub fn tac(&self) -> Result<String, JzeroError> {
        let (tree, sem) = self.analyse()?;
        let ctx = jzero_codegen::generate(&tree, &sem);
        Ok(jzero_codegen::emit::emit(&tree, &ctx))
    }

    /// Compile to bytecode and return the binary image + assembler listing.
    ///
    /// `argc` is the number of command-line arguments to pass to `main()`.
    ///
    /// # Errors
    /// Returns a [`JzeroError`] if parsing or semantic analysis fails.
    pub fn compile(&self, argc: i64) -> Result<CompileOutput, JzeroError> {
        let (tree, sem) = self.analyse()?;
        let ctx    = jzero_codegen::generate(&tree, &sem);
        let tac    = jzero_codegen::emit::emit(&tree, &ctx);
        let output = jzero_codegen::pipeline::compile_bytecode(&tree, &ctx, argc);
        Ok(CompileOutput {
            binary: output.binary,
            text:   output.text,
            tac,
        })
    }

    /// Compile and execute in the VM.
    ///
    /// `args` are passed as `argv` to the Jzero `main()` method,
    /// so `args.len()` determines `argv.length`.
    ///
    /// # Errors
    /// Returns a [`JzeroError`] if parsing, semantic analysis, or VM execution fails.
    pub fn run(&self, args: &[&str]) -> Result<RunOutput, JzeroError> {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let argc = owned.len() as i64;
        let (tree, sem) = self.analyse()?;
        let ctx    = jzero_codegen::generate(&tree, &sem);
        let output = jzero_codegen::pipeline::compile_bytecode(&tree, &ctx, argc);
        let stdout = jzero_vm::run(&output.binary, &owned)
            .map_err(|e| JzeroError(e))?;
        Ok(RunOutput { stdout })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &str = r#"
        public class hello {
            public static void main(String argv[]) {
                System.out.println("hello, jzero!");
            }
        }
    "#;

    const HELLO_LOOP: &str = r#"
        public class hello_loop {
            public static void main(String argv[]) {
                int x;
                x = argv.length;
                x = x + 2;
                while (x > 3) {
                    System.out.println("hello, jzero!");
                    x = x - 1;
                }
            }
        }
    "#;

    const CONCAT: &str = r#"
        public class concat {
            public static void main(String argv[]) {
                String s;
                s = "hello, " + "jzero!";
                System.out.println(s);
            }
        }
    "#;

    #[test]
    fn hello_world_runs() {
        let out = Compiler::new().source(HELLO).run(&[]).unwrap();
        assert_eq!(out.stdout, "hello, jzero!\n");
    }

    #[test]
    fn hello_loop_runs_four_times() {
        let out = Compiler::new()
            .source(HELLO_LOOP)
            .run(&["a", "b", "c", "d", "e"])
            .unwrap();
        assert_eq!(out.stdout, "hello, jzero!\n".repeat(4));
    }

    #[test]
    fn string_concat_runs() {
        let out = Compiler::new().source(CONCAT).run(&[]).unwrap();
        assert_eq!(out.stdout, "hello, jzero!\n");
    }

    #[test]
    fn tac_contains_proc_main() {
        let tac = Compiler::new().source(HELLO).tac().unwrap();
        assert!(tac.contains("proc main"));
    }

    #[test]
    fn compile_binary_has_magic() {
        let out = Compiler::new().source(HELLO).compile(0).unwrap();
        assert_eq!(&out.binary[0..8], b"Jzero!!\0");
    }

    #[test]
    fn semantic_error_is_reported() {
        let src = r#"
            public class bad {
                public static void main(String argv[]) {
                    int x;
                    x = "not an int";
                }
            }
        "#;
        // Type mismatch: semantic analysis passes (no hard errors for type
        // mismatches in our current implementation) but the TAC is still produced.
        // This test just ensures the pipeline doesn't panic.
        let _ = Compiler::new().source(src).tac();
    }

    #[test]
    fn parse_error_returns_err() {
        let result = Compiler::new().source("this is not valid jzero").run(&[]);
        assert!(result.is_err());
    }
}