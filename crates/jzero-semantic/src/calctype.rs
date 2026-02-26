//! Phase 4 — Declaration type assignment.
//!
//! Two cooperative functions that mirror the book's `calctype()` and
//! `assigntype()` methods:
//!
//! - `calc_type(tree)`   — post-order: synthesizes a `TypeInfo` from a
//!                         `Type` subtree (reserved words + array brackets)
//! - `assign_type(tree, t)` — top-down: inherits a `TypeInfo` downward
//!                         through `VarDeclarator` nodes to the `IDENTIFIER`
//!                         leaf, stamping `typ` on each node along the way.
//!
//! Called from `builder.rs` at `FieldDecl`, `LocalVarDecl`, and `FormalParm`
//! nodes — not as a separate full-tree pass.

use jzero_ast::tree::Tree;
use jzero_symtab::TypeInfo;

use crate::error::SemanticError;

// ─── calc_type ───────────────────────────────────────────────────────────────

/// Synthesize a `TypeInfo` from a `Type` subtree (post-order).
///
/// Handles:
/// - Primitive keywords: `int`, `double`, `bool`, `String`, `void`
/// - User-defined type: `IDENTIFIER` → `ClassType`
/// - Array type: `ArrayType` node wrapping any of the above
///
/// Returns `None` and pushes a `SemanticError` if the type cannot be determined.
pub fn calc_type(tree: &mut Tree, errors: &mut Vec<SemanticError>) -> Option<TypeInfo> {
    // Post-order: recurse into children first
    for kid in &mut tree.kids {
        calc_type(kid, errors);
    }

    let typ = match tree.sym.as_str() {
        // ── Declaration nodes: inherit type from kids[0] (the Type child) ──
        "FieldDecl" | "LocalVarDecl" | "FormalParm" => {
            tree.kids.first().and_then(|k| k.typ.clone())
        }

        // ── Array type wrapper ────────────────────────────────────────────
        // ArrayType node has one child: the element type node
        "ArrayType" => {
            let elem = tree.kids.first().and_then(|k| k.typ.clone())?;
            Some(TypeInfo::array(elem))
        }

        // ── Token leaf: the actual type keyword or identifier ─────────────
        _ if tree.tok.is_some() => {
            let tok = tree.tok.as_ref().unwrap();
            match tok.category.as_str() {
                "INT"        => Some(TypeInfo::int()),
                "DOUBLE"     => Some(TypeInfo::double()),
                "BOOL"       => Some(TypeInfo::boolean()),
                "STRING"     => Some(TypeInfo::string()),
                "VOID"       => Some(TypeInfo::void()),
                "IDENTIFIER" => Some(TypeInfo::class(&tok.text)),
                // Literals and operators already typed by Phase 3 — pass through
                _ => tree.typ.clone(),
            }
        }

        // ── Other internal nodes: pass through first child's type ─────────
        _ => tree.kids.first().and_then(|k| k.typ.clone()),
    };

    if let Some(ref t) = typ {
        tree.set_typ(t.clone());
    }

    typ
}

// ─── assign_type ─────────────────────────────────────────────────────────────

/// Inherit a `TypeInfo` downward through declarator nodes (top-down).
///
/// Stamps `typ` on the current node, then:
/// - `VarDeclarator` (array form, rule 1): recurses with `Array(t)` wrapping
/// - `VarDeclarator` (plain form, rule 0): recurses with `t` as-is
/// - `IDENTIFIER` leaf: stamps `typ` and stops — base case
///
/// Returns the final `TypeInfo` stamped on the innermost `IDENTIFIER` leaf,
/// so the caller can store it in the symbol table entry.
pub fn assign_type(
    tree: &mut Tree,
    t: TypeInfo,
    errors: &mut Vec<SemanticError>,
) -> Option<TypeInfo> {
    tree.set_typ(t.clone());

    match tree.sym.as_str() {
        // ── Array VarDeclarator (rule 1): int x[] ─────────────────────────
        // kids[0] is the inner VarDeclarator or IDENTIFIER
        // The array bracket is implicit in rule 1 — wrap type in Array(...)
        "VarDeclarator" if tree.rule == 1 => {
            if let Some(kid) = tree.kids.first_mut() {
                return assign_type(kid, TypeInfo::array(t), errors);
            }
            None
        }

        // ── Plain VarDeclarator (rule 0): just the identifier ─────────────
        "VarDeclarator" => {
            if let Some(kid) = tree.kids.first_mut() {
                return assign_type(kid, t, errors);
            }
            None
        }

        // ── IDENTIFIER leaf: base case — type is now fully resolved ───────
        _ if tree.tok.is_some() => {
            let tok = tree.tok.as_ref().unwrap();
            if tok.category == "IDENTIFIER" {
                Some(t)
            } else {
                let lineno = tok.lineno;
                errors.push(SemanticError::TypeAssignmentError {
                    msg: format!("unexpected token '{}' in declarator", tok.text),
                    lineno,
                });
                None
            }
        }

        _ => {
            errors.push(SemanticError::TypeAssignmentError {
                msg: format!("cannot assign type to node '{}'", tree.sym),
                lineno: 0,
            });
            None
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jzero_ast::tree::Tree;

    fn no_errors() -> Vec<SemanticError> { Vec::new() }

    // ─── calc_type tests ──────────────────────────────────────────────────

    #[test]
    fn test_calc_type_int_keyword() {
        let mut t = Tree::leaf("INT", "int", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "int");
        assert_eq!(t.typ.as_ref().unwrap().basetype(), "int");
    }

    #[test]
    fn test_calc_type_double_keyword() {
        let mut t = Tree::leaf("DOUBLE", "double", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "double");
    }

    #[test]
    fn test_calc_type_string_keyword() {
        let mut t = Tree::leaf("STRING", "String", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "String");
    }

    #[test]
    fn test_calc_type_void_keyword() {
        let mut t = Tree::leaf("VOID", "void", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "void");
    }

    #[test]
    fn test_calc_type_identifier_becomes_classtype() {
        let mut t = Tree::leaf("IDENTIFIER", "MyClass", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "MyClass");
    }

    #[test]
    fn test_calc_type_array_type_node() {
        // ArrayType { INT }  →  Array(int)
        let int_leaf = Tree::leaf("INT", "int", 1);
        let mut array_node = Tree::new("ArrayType", 0, vec![int_leaf]);
        let typ = calc_type(&mut array_node, &mut no_errors());
        assert_eq!(typ.as_ref().unwrap().basetype(), "array");
        assert_eq!(typ.unwrap().to_string(), "int[]");
    }

    // ─── assign_type tests ────────────────────────────────────────────────

    #[test]
    fn test_assign_type_plain_var_declarator() {
        // VarDeclarator#0 { IDENTIFIER("x") }
        let ident = Tree::leaf("IDENTIFIER", "x", 1);
        let mut decl = Tree::new("VarDeclarator", 0, vec![ident]);
        let result = assign_type(&mut decl, TypeInfo::int(), &mut no_errors());

        assert_eq!(result.unwrap().basetype(), "int");
        assert_eq!(decl.typ.as_ref().unwrap().basetype(), "int");
        assert_eq!(decl.kids[0].typ.as_ref().unwrap().basetype(), "int");
    }

    #[test]
    fn test_assign_type_array_var_declarator() {
        // VarDeclarator#1 { VarDeclarator#0 { IDENTIFIER("argv") } }
        // Represents `argv[]` — rule 1 wraps type in Array(...)
        let ident  = Tree::leaf("IDENTIFIER", "argv", 1);
        let inner  = Tree::new("VarDeclarator", 0, vec![ident]);
        let mut outer = Tree::new("VarDeclarator", 1, vec![inner]);

        let result = assign_type(&mut outer, TypeInfo::string(), &mut no_errors());

        // outer gets String, inner gets String[], identifier gets String[]
        assert_eq!(outer.typ.as_ref().unwrap().basetype(), "String");
        assert_eq!(outer.kids[0].typ.as_ref().unwrap().basetype(), "array");
        assert_eq!(result.unwrap().to_string(), "String[]");
    }

    // ─── Integration: parse real declarations ─────────────────────────────

    #[test]
    fn test_local_var_decl_int() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
    }
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        crate::typeinit::assign_leaf_types(&mut tree);

        // Manually drive calc_type + assign_type on the LocalVarDecl node
        let method = &mut tree.kids[1];
        let block  = &mut method.kids[1];
        let var_decl = &mut block.kids[0];
        assert_eq!(var_decl.sym, "LocalVarDecl");

        let mut errors = no_errors();
        let typ = calc_type(&mut var_decl.kids[0], &mut errors); // Type child
        assert_eq!(typ.as_ref().unwrap().basetype(), "int");

        let final_typ = assign_type(&mut var_decl.kids[1], typ.unwrap(), &mut errors);
        assert!(errors.is_empty());
        assert_eq!(final_typ.unwrap().basetype(), "int");
    }

    #[test]
    fn test_formal_parm_string_array() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
    }
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        crate::typeinit::assign_leaf_types(&mut tree);

        // MethodDecl → MethodHeader → MethodDeclarator → FormalParm
        let parm = find_node(&mut tree, "FormalParm").expect("FormalParm not found");
        let mut errors = no_errors();

        // kids[0] = Type ("String" arrives as IDENTIFIER → ClassType)
        let typ = calc_type(&mut parm.kids[0], &mut errors);
        assert_eq!(typ.as_ref().unwrap().basetype(), "String");

        let final_typ = assign_type(&mut parm.kids[1], typ.unwrap(), &mut errors);
        assert!(errors.is_empty());
        // String is a class type, so argv[] → ClassType("String")[]
        // Display renders as "String[]" because ClassType::fmt uses the name directly
        assert_eq!(final_typ.unwrap().to_string(), "String[]");
    }

    /// Simple DFS to find the first node with the given sym.
    fn find_node<'a>(tree: &'a mut Tree, sym: &str) -> Option<&'a mut Tree> {
        if tree.sym == sym { return Some(tree); }
        for kid in &mut tree.kids {
            if let Some(found) = find_node(kid, sym) {
                return Some(found);
            }
        }
        None
    }
}