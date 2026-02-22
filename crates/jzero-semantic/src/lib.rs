pub mod builder;
pub mod error;
mod tests;

pub use builder::build_symtabs;
pub use error::SemanticError;

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
/// This is the main entry point for Chapter 6:
/// 1. Creates the global scope and populates predefined symbols (System.out.println)
/// 2. Walks the tree to build symbol tables for each scope
/// 3. Checks for undeclared and redeclared variables
/// 4. Attaches `stab` references to tree nodes as an inherited attribute
pub fn analyze(tree: &mut Tree) -> SemanticResult {
    // Build global scope + predefined symbols
    let global = SymTab::new("global", None).into_rc();
    build_predefined(&global);

    let mut errors = Vec::new();

    // Walk the tree — builds symbol tables and attaches stab to nodes
    builder::build_symtabs(tree, Rc::clone(&global), &mut errors);

    SemanticResult { global, errors }
}