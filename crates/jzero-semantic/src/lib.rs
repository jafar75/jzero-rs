pub mod builder;
pub mod calctype;
pub mod checktype;
pub mod error;
pub mod mkcls;
pub mod typeinit;
mod tests;

pub use builder::build_symtabs;
pub use calctype::{calc_type, assign_type};
pub use checktype::{check_type, TypeCheckResult};
pub use error::SemanticError;
pub use mkcls::mkcls;
pub use typeinit::assign_leaf_types;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, build_predefined};
use std::rc::Rc;
use std::cell::RefCell;

/// The result of semantic analysis.
pub struct SemanticResult {
    pub global: Rc<RefCell<SymTab>>,
    pub errors: Vec<SemanticError>,
    pub type_checks: Vec<TypeCheckResult>,
}

/// Run full semantic analysis on a parsed syntax tree.
///
/// Passes in order:
/// 1. Build global scope + predefined symbols
/// 2. Assign types to literal/operator leaves          (Phase 3)
/// 3. Build symbol tables + declaration types          (Phase 4)
/// 4. Build full ClassType for every ClassDecl         (mkcls)
/// 5. Check expression types in method bodies          (Phase 5)
pub fn analyze(tree: &mut Tree) -> SemanticResult {
    let global = SymTab::new("global", None).into_rc();
    build_predefined(&global);

    assign_leaf_types(tree);

    let mut errors = Vec::new();
    build_symtabs(tree, Rc::clone(&global), &mut errors);

    // Build ClassType entries so InstanceCreation can look them up
    mkcls(tree);

    let mut type_checks = Vec::new();
    check_type(tree, false, &mut type_checks);

    SemanticResult { global, errors, type_checks }
}