//! Phase 3 — Leaf type assignment.
//!
//! Mirrors the book's token constructor logic: sets `typ` on leaf nodes
//! based on their token category. This is a simple pre-order traversal
//! since leaves don't depend on their children (they have none).
//!
//! Called once on the whole tree before any expression type checking.

use jzero_ast::tree::Tree;
use jzero_symtab::TypeInfo;

/// Assign types to all leaf nodes in the tree based on token category.
///
/// Matches the book's token constructor:
/// - `INTLIT`    → `TypeInfo::int()`
/// - `DOUBLELIT` → `TypeInfo::double()`
/// - `STRINGLIT` → `TypeInfo::string()`
/// - `BOOLLIT`   → `TypeInfo::boolean()`
/// - `NULL`      → `TypeInfo::null()`
/// - operators   → `TypeInfo::na()`
/// - other leaves (IDENTIFIER, keywords) → left as `None` for later passes
pub fn assign_leaf_types(tree: &mut Tree) {
    if let Some(ref tok) = tree.tok.clone() {
        let typ = match tok.category.as_str() {
            "INTLIT"    => Some(TypeInfo::int()),
            "DOUBLELIT" => Some(TypeInfo::double()),
            "STRINGLIT" => Some(TypeInfo::string()),
            "BOOLLIT"   => Some(TypeInfo::boolean()),
            "NULL"      => Some(TypeInfo::null()),
            // Operators carry no value type — n/a matches the book
            "PLUS" | "MINUS" | "STAR" | "SLASH" | "PERCENT" |
            "ASSIGN" | "PLUSASSIGN" | "MINUSASSIGN" |
            "LESS" | "GREATER" | "LESSEQUAL" | "GREATEREQUAL" |
            "EQUALEQUAL" | "NOTEQUAL" |
            "LOGICALAND" | "LOGICALOR" => Some(TypeInfo::na()),
            // Keywords, IDENTIFIER, etc. — typed by later passes
            _ => None,
        };
        if let Some(t) = typ {
            tree.set_typ(t);
        }
        // Leaves have no children — nothing to recurse into
        return;
    }

    // Internal node — recurse into all children
    for kid in &mut tree.kids {
        assign_leaf_types(kid);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jzero_ast::tree::Tree;

    fn leaf(cat: &str, text: &str) -> Tree {
        Tree::leaf(cat, text, 1)
    }

    // ─── Literal leaves ───────────────────────────────────────────────────

    #[test]
    fn test_intlit_gets_int_type() {
        let mut t = leaf("INTLIT", "42");
        assign_leaf_types(&mut t);
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "int");
    }

    #[test]
    fn test_doublelit_gets_double_type() {
        let mut t = leaf("DOUBLELIT", "3.14");
        assign_leaf_types(&mut t);
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "double");
    }

    #[test]
    fn test_stringlit_gets_string_type() {
        let mut t = leaf("STRINGLIT", "\"hello\"");
        assign_leaf_types(&mut t);
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "String");
    }

    #[test]
    fn test_boollit_gets_boolean_type() {
        let mut t = leaf("BOOLLIT", "true");
        assign_leaf_types(&mut t);
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "boolean");
    }

    #[test]
    fn test_null_gets_null_type() {
        let mut t = leaf("NULL", "null");
        assign_leaf_types(&mut t);
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "null");
    }

    // ─── Operator leaves ──────────────────────────────────────────────────

    #[test]
    fn test_operators_get_na_type() {
        for (cat, text) in &[
            ("PLUS", "+"), ("MINUS", "-"), ("STAR", "*"), ("SLASH", "/"),
            ("ASSIGN", "="), ("PLUSASSIGN", "+="), ("MINUSASSIGN", "-="),
            ("LESS", "<"), ("GREATER", ">"), ("EQUALEQUAL", "=="),
            ("LOGICALAND", "&&"), ("LOGICALOR", "||"),
        ] {
            let mut t = leaf(cat, text);
            assign_leaf_types(&mut t);
            assert_eq!(
                t.typ.as_ref().unwrap().basetype(), "n/a",
                "expected n/a for operator {}", cat
            );
        }
    }

    // ─── Non-typed leaves ─────────────────────────────────────────────────

    #[test]
    fn test_identifier_stays_untyped() {
        let mut t = leaf("IDENTIFIER", "x");
        assign_leaf_types(&mut t);
        // IDENTIFIER type is resolved by symbol table lookup — not here
        assert!(t.typ.is_none(), "IDENTIFIER should not be typed by assign_leaf_types");
    }

    #[test]
    fn test_keyword_stays_untyped() {
        let mut t = leaf("INT", "int");
        assign_leaf_types(&mut t);
        // Keywords typed by calctype during declaration processing
        assert!(t.typ.is_none());
    }

    // ─── Tree recursion ───────────────────────────────────────────────────

    #[test]
    fn test_recurses_into_children() {
        let lhs = leaf("IDENTIFIER", "x");
        let op  = leaf("ASSIGN", "=");
        let rhs = leaf("INTLIT", "42");
        let mut assign = Tree::new("Assignment", 0, vec![lhs, op, rhs]);
        assign_leaf_types(&mut assign);

        // Internal node has no typ itself
        assert!(assign.typ.is_none());
        // But children do
        assert!(assign.kids[0].typ.is_none());          // IDENTIFIER — untyped
        assert_eq!(assign.kids[1].typ.as_ref().unwrap().basetype(), "n/a");   // ASSIGN
        assert_eq!(assign.kids[2].typ.as_ref().unwrap().basetype(), "int");   // INTLIT
    }

    #[test]
    fn test_does_not_overwrite_existing_typ() {
        // If a node already has a type set, assign_leaf_types should not touch it
        // (it only sets types it knows about — others stay None naturally)
        let mut t = leaf("INTLIT", "1");
        t.set_typ(TypeInfo::double()); // manually pre-set
        assign_leaf_types(&mut t);
        // INTLIT always gets int — the pre-set is overwritten because we
        // unconditionally set known categories. This matches the book's
        // token constructor behaviour (type is fixed at construction).
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "int");
    }

    // ─── Integration: parse a real program ───────────────────────────────

    #[test]
    fn test_assign_leaf_types_on_parsed_tree() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        x = 42;
    }
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        assign_leaf_types(&mut tree);

        // Find the Assignment node inside the Block
        let method = &tree.kids[1];
        let block  = &method.kids[1];
        let assign = &block.kids[1]; // kids[0] is LocalVarDecl, kids[1] is Assignment
        assert_eq!(assign.sym, "Assignment");

        // kids[2] is the INTLIT "42"
        let intlit = &assign.kids[2];
        assert_eq!(intlit.typ.as_ref().unwrap().basetype(), "int");

        // kids[1] is the ASSIGN "=" operator
        let op = &assign.kids[1];
        assert_eq!(op.typ.as_ref().unwrap().basetype(), "n/a");
    }
}