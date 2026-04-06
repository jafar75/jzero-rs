//! `jzero-codegen` — Intermediate code generation for the Jzero compiler.
//!
//! # Pipeline
//!
//! After [`jzero_semantic::analyze`] returns a [`SemanticResult`], call
//! [`generate`] to run the codegen passes in order:
//!
//! 1. **Layout**     — assign memory addresses to every variable/param.
//! 2. **genfirst**   — synthesize `first` entry-point labels (post-order).
//! 3. **genfollow**  — inherit `follow` exit-point labels (pre-order).
//! 4. **gentargets** — inherit `on_true`/`on_false` for Boolean exprs (pre-order).
//! 5. **gencode**    — emit `Vec<Tac>` for each node (post-order).

pub mod address;
pub mod context;
pub mod emit;
pub mod gencode;
pub mod labels;
pub mod layout;
pub mod tac;
mod tests;

use jzero_ast::tree::Tree;
use jzero_semantic::SemanticResult;

pub use address::{Address, Region};
pub use context::CodegenContext;
pub use tac::{Op, Tac};

/// Run all codegen passes on an already-analysed syntax tree.
///
/// Returns the populated [`CodegenContext`]. Call [`emit::emit`] on the
/// result to produce human-readable assembler output.
///
/// # Example
///
/// ```no_run
/// # use jzero_ast::tree::Tree;
/// # use jzero_semantic::SemanticResult;
/// let mut tree: Tree = todo!("parse your source file");
/// let sem: SemanticResult = jzero_semantic::analyze(&mut tree);
/// let ctx = jzero_codegen::generate(&tree, &sem);
/// let asm = jzero_codegen::emit::emit(&tree, &ctx);
/// println!("{}", asm);
/// ```
pub fn generate(tree: &Tree, sem: &SemanticResult) -> CodegenContext {
    let mut ctx = CodegenContext::new();

    // Pass 1 — assign addresses to all variables and parameters.
    layout::assign_addresses(&sem.global, &mut ctx);

    // Pass 2 — synthesize `first` labels (post-order).
    labels::genfirst(tree, &mut ctx);

    // Pass 3 — inherit `follow` labels (pre-order).
    labels::genfollow(tree, &mut ctx);

    // Pass 4 — inherit `on_true`/`on_false` (pre-order).
    labels::gentargets(tree, &mut ctx);

    // Pass 5 — emit intermediate code (post-order).
    gencode::gencode(tree, &mut ctx);

    ctx
}