pub mod builder;
pub mod error;
pub mod typeinit;
pub mod calctype;
mod tests;

pub use builder::build_symtabs;
pub use error::SemanticError;
pub use typeinit::assign_leaf_types;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, build_predefined};
use std::rc::Rc;
use std::cell::RefCell;

/// The result of semantic analysis.
pub struct SemanticResult {
    /// The global symbol table (root of the scope tree).
    pub global: Rc<RefCell<SymTab>>,
    /// Any semantic errors found (undeclared / redeclared variables).
    pub errors: Vec<SemanticError>,
}

/// Run semantic analysis on a parsed syntax tree.
///
/// Passes performed in order:
/// 1. Build global scope + predefined symbols (System.out.println)
/// 2. Assign types to literal/operator leaf nodes
/// 3. Walk the tree to build symbol tables and attach stab to nodes
pub fn analyze(tree: &mut Tree) -> SemanticResult {
    let global = SymTab::new("global", None).into_rc();
    build_predefined(&global);

    // Phase 3: stamp types onto all literal and operator leaves
    assign_leaf_types(tree);

    let mut errors = Vec::new();

    // Phase 4+: build symbol tables, attach stab, assign declaration types
    builder::build_symtabs(tree, Rc::clone(&global), &mut errors);

    SemanticResult { global, errors }
}