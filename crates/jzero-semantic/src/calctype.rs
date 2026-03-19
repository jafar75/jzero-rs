//! Phase 4 — Declaration type assignment.
//!
//! - `calc_type(tree)`        — post-order: synthesizes a `TypeInfo` from a
//!                              Type/MethodHeader subtree
//! - `assign_type(tree, t)`   — top-down: inherits a `TypeInfo` downward
//!                              through VarDeclarator / MethodDeclarator nodes
//! - `mksig(tree)`            — collects parameter types from FormalParm kids

use jzero_ast::tree::Tree;
use jzero_symtab::{Parameter, TypeInfo};

use crate::error::SemanticError;

// ─── mksig ───────────────────────────────────────────────────────────────────

/// Collect the parameter list from a slice of `FormalParm` tree nodes.
///
/// Each `FormalParm` node has:
///   kids[0] = Type  (already typed by calc_type's post-order pass)
///   kids[1] = VarDeclarator  (IDENTIFIER leaf)
///
/// Returns a `Vec<Parameter>` in declaration order.
pub fn mksig(parms: &[Tree]) -> Vec<Parameter> {
    parms
        .iter()
        .filter(|p| p.sym == "FormalParm")
        .map(|p| {
            // Name: innermost IDENTIFIER leaf of kids[1] (VarDeclarator)
            let name = extract_identifier_name(&p.kids[1]).unwrap_or_default();
            // Type: from kids[0] (the Type node), which calc_type has already
            // stamped in the post-order pass. kids[1].typ is not yet set here
            // because assign_type runs later in builder.rs.
            let base_typ = p.kids[0].typ.clone().unwrap_or_else(TypeInfo::unknown);
            // If the VarDeclarator is rule 1 (array form), wrap in Array(...)
            let typ = if p.kids[1].rule == 1 {
                TypeInfo::array(base_typ)
            } else {
                base_typ
            };
            Parameter::new(&name, typ)
        })
        .collect()
}

/// Walk a VarDeclarator subtree to find the IDENTIFIER leaf text.
fn extract_identifier_name(tree: &Tree) -> Option<String> {
    if let Some(tok) = &tree.tok {
        if tok.category == "IDENTIFIER" {
            return Some(tok.text.clone());
        }
    }
    for kid in &tree.kids {
        if let Some(name) = extract_identifier_name(kid) {
            return Some(name);
        }
    }
    None
}

// ─── calc_type ───────────────────────────────────────────────────────────────

/// Synthesize a `TypeInfo` from a `Type` or `MethodHeader` subtree (post-order).
pub fn calc_type(tree: &mut Tree, errors: &mut Vec<SemanticError>) -> Option<TypeInfo> {
    // Post-order: recurse into children first
    for kid in &mut tree.kids {
        calc_type(kid, errors);
    }

    let typ = match tree.sym.as_str() {
        // ── Declaration nodes ────────────────────────────────────────────
        "FieldDecl" | "LocalVarDecl" | "FormalParm" => {
            tree.kids.first().and_then(|k| k.typ.clone())
        }

        // ── MethodHeader: build MethodType from return type + params ─────
        //
        // kids[0] = MethodReturnVal  (already typed by post-order above)
        // kids[1] = MethodDeclarator
        //   kids[1].kids[0] = IDENTIFIER (method name)
        //   kids[1].kids[1..] = FormalParm nodes
        "MethodHeader" => {
            let return_type = tree.kids.first().and_then(|k| k.typ.clone())?;
            let parms: Vec<Parameter> = if tree.kids.len() > 1 {
                let decl = &tree.kids[1];
                mksig(&decl.kids[1..])
            } else {
                vec![]
            };
            Some(TypeInfo::method(return_type, parms))
        }

        // ── Array type wrapper ───────────────────────────────────────────
        "ArrayType" => {
            let elem = tree.kids.first().and_then(|k| k.typ.clone())?;
            Some(TypeInfo::array(elem))
        }

        // ── Token leaf ───────────────────────────────────────────────────
        _ if tree.tok.is_some() => {
            let tok = tree.tok.as_ref().unwrap();
            match tok.category.as_str() {
                "INT"        => Some(TypeInfo::int()),
                "DOUBLE"     => Some(TypeInfo::double()),
                "BOOL"       => Some(TypeInfo::boolean()),
                "STRING"     => Some(TypeInfo::string()),
                "VOID"       => Some(TypeInfo::void()),
                "IDENTIFIER" => Some(TypeInfo::class(&tok.text)),
                _            => tree.typ.clone(),
            }
        }

        // ── Other internal nodes: pass through first child's type ────────
        _ => tree.kids.first().and_then(|k| k.typ.clone()),
    };

    if let Some(ref t) = typ {
        tree.set_typ(t.clone());
    }

    typ
}

// ─── assign_type ─────────────────────────────────────────────────────────────

/// Inherit a `TypeInfo` downward through declarator nodes (top-down).
pub fn assign_type(
    tree: &mut Tree,
    t: TypeInfo,
    errors: &mut Vec<SemanticError>,
) -> Option<TypeInfo> {
    tree.set_typ(t.clone());

    match tree.sym.as_str() {
        // ── Array VarDeclarator (rule 1): int x[] ────────────────────────
        "VarDeclarator" if tree.rule == 1 => {
            if let Some(kid) = tree.kids.first_mut() {
                return assign_type(kid, TypeInfo::array(t), errors);
            }
            None
        }

        // ── Plain VarDeclarator (rule 0) ─────────────────────────────────
        "VarDeclarator" => {
            if let Some(kid) = tree.kids.first_mut() {
                return assign_type(kid, t, errors);
            }
            None
        }

        // ── MethodDeclarator: build method type, stamp on name leaf ──────
        //
        // kids[0] = IDENTIFIER (method name)
        // kids[1..] = FormalParm nodes (already typed by calc_type)
        //
        // `t` here is the return type inherited from MethodHeader.
        "MethodDeclarator" => {
            let parms = mksig(&tree.kids[1..]);
            let method_typ = TypeInfo::method(t, parms);
            tree.set_typ(method_typ.clone());
            if let Some(name_leaf) = tree.kids.first_mut() {
                name_leaf.set_typ(method_typ.clone());
            }
            Some(method_typ)
        }

        // ── IDENTIFIER leaf: base case ────────────────────────────────────
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

    #[test]
    fn test_calc_type_int_keyword() {
        let mut t = Tree::leaf("INT", "int", 1);
        let typ = calc_type(&mut t, &mut no_errors());
        assert_eq!(typ.unwrap().basetype(), "int");
    }

    #[test]
    fn test_calc_type_double_keyword() {
        let mut t = Tree::leaf("DOUBLE", "double", 1);
        assert_eq!(calc_type(&mut t, &mut no_errors()).unwrap().basetype(), "double");
    }

    #[test]
    fn test_calc_type_string_keyword() {
        let mut t = Tree::leaf("STRING", "String", 1);
        assert_eq!(calc_type(&mut t, &mut no_errors()).unwrap().basetype(), "String");
    }

    #[test]
    fn test_calc_type_void_keyword() {
        let mut t = Tree::leaf("VOID", "void", 1);
        assert_eq!(calc_type(&mut t, &mut no_errors()).unwrap().basetype(), "void");
    }

    #[test]
    fn test_calc_type_identifier_becomes_classtype() {
        let mut t = Tree::leaf("IDENTIFIER", "MyClass", 1);
        assert_eq!(calc_type(&mut t, &mut no_errors()).unwrap().basetype(), "MyClass");
    }

    #[test]
    fn test_calc_type_array_type_node() {
        let int_leaf = Tree::leaf("INT", "int", 1);
        let mut array_node = Tree::new("ArrayType", 0, vec![int_leaf]);
        let typ = calc_type(&mut array_node, &mut no_errors());
        assert_eq!(typ.as_ref().unwrap().basetype(), "array");
        assert_eq!(typ.unwrap().to_string(), "int[]");
    }

    #[test]
    fn test_assign_type_plain_var_declarator() {
        let ident = Tree::leaf("IDENTIFIER", "x", 1);
        let mut decl = Tree::new("VarDeclarator", 0, vec![ident]);
        let result = assign_type(&mut decl, TypeInfo::int(), &mut no_errors());
        assert_eq!(result.unwrap().basetype(), "int");
        assert_eq!(decl.kids[0].typ.as_ref().unwrap().basetype(), "int");
    }

    #[test]
    fn test_assign_type_array_var_declarator() {
        let ident  = Tree::leaf("IDENTIFIER", "argv", 1);
        let inner  = Tree::new("VarDeclarator", 0, vec![ident]);
        let mut outer = Tree::new("VarDeclarator", 1, vec![inner]);
        let result = assign_type(&mut outer, TypeInfo::string(), &mut no_errors());
        assert_eq!(outer.kids[0].typ.as_ref().unwrap().basetype(), "array");
        assert_eq!(result.unwrap().to_string(), "String[]");
    }

    #[test]
    fn test_method_header_builds_method_type() {
        let src = r#"
public class T {
    public static int foo(int x, int y, string z) {
        return 0;
    }
    public static void main(string argv[]) {}
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        crate::typeinit::assign_leaf_types(&mut tree);

        let method_decl = &mut tree.kids[1];
        let header = &mut method_decl.kids[0];
        assert_eq!(header.sym, "MethodHeader");

        let mut errors = no_errors();
        let typ = calc_type(header, &mut errors);
        assert!(errors.is_empty());
        let typ = typ.expect("MethodHeader should produce a type");
        assert_eq!(typ.basetype(), "method");
        if let TypeInfo::Method(mt) = &typ {
            assert_eq!(mt.return_type.basetype(), "int");
            assert_eq!(mt.parameters.len(), 3);
            assert_eq!(mt.parameters[0].param_type.basetype(), "int");
            assert_eq!(mt.parameters[1].param_type.basetype(), "int");
            assert_eq!(mt.parameters[2].param_type.basetype(), "String");
        } else {
            panic!("expected Method type");
        }
    }

    #[test]
    fn test_local_var_decl_int() {
        let src = r#"
public class T {
    public static void main(string argv[]) {
        int x;
    }
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        crate::typeinit::assign_leaf_types(&mut tree);
        let method = &mut tree.kids[1];
        let block  = &mut method.kids[1];
        let var_decl = &mut block.kids[0];
        assert_eq!(var_decl.sym, "LocalVarDecl");
        let mut errors = no_errors();
        let typ = calc_type(&mut var_decl.kids[0], &mut errors);
        assert_eq!(typ.as_ref().unwrap().basetype(), "int");
        let final_typ = assign_type(&mut var_decl.kids[1], typ.unwrap(), &mut errors);
        assert!(errors.is_empty());
        assert_eq!(final_typ.unwrap().basetype(), "int");
    }

    #[test]
    fn test_formal_parm_string_array() {
        let src = r#"
public class T {
    public static void main(string argv[]) {
    }
}
"#;
        let mut tree = jzero_parser::parse_tree(src).expect("parse failed");
        crate::typeinit::assign_leaf_types(&mut tree);
        let parm = find_node(&mut tree, "FormalParm").expect("FormalParm not found");
        let mut errors = no_errors();
        let typ = calc_type(&mut parm.kids[0], &mut errors);
        assert_eq!(typ.as_ref().unwrap().basetype(), "String");
        let final_typ = assign_type(&mut parm.kids[1], typ.unwrap(), &mut errors);
        assert!(errors.is_empty());
        assert_eq!(final_typ.unwrap().to_string(), "String[]");
    }

    fn find_node<'a>(tree: &'a mut Tree, sym: &str) -> Option<&'a mut Tree> {
        if tree.sym == sym { return Some(tree); }
        for kid in &mut tree.kids {
            if let Some(found) = find_node(kid, sym) { return Some(found); }
        }
        None
    }
}