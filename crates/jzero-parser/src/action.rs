//! Type aliases for grammar action closures.
//!
//! LALRPOP doesn't support `Box<dyn FnOnce(Tree) -> Tree + 'input>` syntax
//! in rule return types, so we define a wrapper type here.

use jzero_ast::tree::Tree;

/// A deferred tree-building action. Used by left-factored rules
/// (IdentifierStartedStmt, DotTail, CallTail, etc.) that need to
/// receive the leading identifier/expression and produce the final node.
pub struct TreeAction<'a>(Box<dyn FnOnce(Tree) -> Tree + 'a>);

impl<'a> TreeAction<'a> {
    pub fn new<F: FnOnce(Tree) -> Tree + 'a>(f: F) -> Self {
        TreeAction(Box::new(f))
    }

    pub fn apply(self, tree: Tree) -> Tree {
        (self.0)(tree)
    }
}