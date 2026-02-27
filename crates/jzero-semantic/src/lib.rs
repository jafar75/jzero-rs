pub mod builder;
pub mod calctype;
pub mod checktype;
pub mod error;
pub mod typeinit;
mod tests;

pub use builder::build_symtabs;
pub use calctype::{calc_type, assign_type};
pub use checktype::{check_type, TypeCheckResult};
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
    /// Semantic errors found (undeclared / redeclared variables, type errors).
    pub errors: Vec<SemanticError>,
    /// Type check results (both OK and FAIL) from expression checking.
    pub type_checks: Vec<TypeCheckResult>,
}

/// Run full semantic analysis on a parsed syntax tree.
///
/// Passes performed in order:
/// 1. Build global scope + predefined symbols (System.out.println)
/// 2. Assign types to literal/operator leaf nodes (Phase 3)
/// 3. Build symbol tables, attach stab, assign declaration types (Phase 4)
/// 4. Check expression types throughout method bodies (Phase 5)
pub fn analyze(tree: &mut Tree) -> SemanticResult {
    let global = SymTab::new("global", None).into_rc();
    build_predefined(&global);

    // Phase 3: stamp literal and operator leaf types
    assign_leaf_types(tree);

    // Phase 4: build symbol tables + declaration types
    let mut errors = Vec::new();
    build_symtabs(tree, Rc::clone(&global), &mut errors);

    // Phase 5: check expression types in method bodies
    let mut type_checks = Vec::new();
    check_type(tree, false, &mut type_checks);

    SemanticResult { global, errors, type_checks }
}