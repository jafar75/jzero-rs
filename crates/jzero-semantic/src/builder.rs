//! Symbol table builder — walks the syntax tree and constructs
//! one `SymTab` per scope, attaching each to the relevant `Tree` nodes.

use std::cell::RefCell;
use std::rc::Rc;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, SymTabEntry, TypeInfo, entry::SymbolKind};

use crate::calctype::{calc_type, assign_type};
use crate::error::SemanticError;

// ─── Public entry point ───────────────────────────────────────────────────────

pub fn build_symtabs(
    tree: &mut Tree,
    current_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    tree.set_stab(Rc::clone(&current_scope));

    match tree.sym.as_str() {
        "ClassDecl"    => walk_class(tree, current_scope, errors),
        "MethodDecl"   => walk_method(tree, current_scope, errors),
        "FieldDecl"    => walk_field_decl(tree, current_scope, errors),
        "LocalVarDecl" => walk_local_var_decl(tree, current_scope, errors),
        "FormalParm"   => walk_formal_parm(tree, current_scope, errors),
        "Block"        => walk_block(tree, current_scope, errors),
        _              => walk_children(tree, current_scope, errors),
    }
}

// ─── Generic child walker ─────────────────────────────────────────────────────

fn walk_children(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    for kid in &mut tree.kids {
        build_symtabs(kid, Rc::clone(&scope), errors);
    }
}

// ─── ClassDecl ────────────────────────────────────────────────────────────────

fn walk_class(
    tree: &mut Tree,
    global: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    let class_name = match tree.kids.first() {
        Some(n) => n.tok.as_ref().map(|t| t.text.clone()).unwrap_or_default(),
        None => return,
    };
    let lineno = tree.kids.first()
        .and_then(|n| n.tok.as_ref())
        .map(|t| t.lineno)
        .unwrap_or(0);

    let class_scope = SymTab::new("class", Some(Rc::clone(&global))).into_rc();

    let class_entry = SymTabEntry::with_scope(
        &class_name,
        SymbolKind::Class,
        Rc::clone(&global),
        false,
        Rc::clone(&class_scope),
    );
    if let Err(_) = global.borrow_mut().insert(class_entry) {
        errors.push(SemanticError::RedeclaredVariable { name: class_name.clone(), lineno });
    }

    tree.set_stab(Rc::clone(&class_scope));

    // First pass: register fields + method signatures
    for kid in &tree.kids[1..] {
        match kid.sym.as_str() {
            "FieldDecl"  => register_field(kid, Rc::clone(&class_scope), errors),
            "MethodDecl" => register_method_signature(kid, Rc::clone(&class_scope), errors),
            _ => {}
        }
    }

    // Second pass: walk method bodies
    for kid in &mut tree.kids[1..] {
        if kid.sym == "MethodDecl" {
            walk_method(kid, Rc::clone(&class_scope), errors);
        } else {
            build_symtabs(kid, Rc::clone(&class_scope), errors);
        }
    }
}

// ─── FieldDecl registration (first pass) ─────────────────────────────────────

fn register_field(
    tree: &Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    if tree.kids.len() < 2 { return; }

    // Collect all VarDeclarator kids (kids[1..]) — there may be multiple
    // e.g. `int x, y;`
    let type_node = &tree.kids[0];

    // Compute base type from the type node (read-only snapshot)
    let base_typ = type_node_to_typeinfo(type_node);

    for decl in &tree.kids[1..] {
        if decl.sym != "VarDeclarator" { continue; }
        let (name, lineno) = declarator_name_and_line(decl);
        let typ = if decl.rule == 1 {
            base_typ.as_ref().map(|t| TypeInfo::array(t.clone()))
        } else {
            base_typ.clone()
        };
        let mut entry = SymTabEntry::new(&name, SymbolKind::Field, Rc::clone(&class_scope), false);
        if let Some(t) = typ { entry.set_typ(t); }
        if let Err(_) = class_scope.borrow_mut().insert(entry) {
            errors.push(SemanticError::RedeclaredVariable { name, lineno });
        }
    }
}

/// Derive a `TypeInfo` from a type keyword leaf node without mutating it.
fn type_node_to_typeinfo(node: &Tree) -> Option<TypeInfo> {
    if let Some(tok) = &node.tok {
        return match tok.category.as_str() {
            "INT"        => Some(TypeInfo::int()),
            "DOUBLE"     => Some(TypeInfo::double()),
            "BOOL"       => Some(TypeInfo::boolean()),
            "STRING"     => Some(TypeInfo::string()),
            "VOID"       => Some(TypeInfo::void()),
            "IDENTIFIER" => Some(TypeInfo::class(&tok.text)),
            _ => None,
        };
    }
    // ArrayType node
    if node.sym == "ArrayType" {
        let elem = node.kids.first().and_then(type_node_to_typeinfo)?;
        return Some(TypeInfo::array(elem));
    }
    node.kids.first().and_then(type_node_to_typeinfo)
}

// ─── MethodDecl ───────────────────────────────────────────────────────────────

/// Register method signature in class scope (first pass).
fn register_method_signature(
    tree: &Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    let name = method_name(tree);
    let lineno = method_lineno(tree);

    let method_scope = SymTab::new("method", Some(Rc::clone(&class_scope))).into_rc();

    // Build the MethodType from the MethodHeader (read-only)
    let method_typ = build_method_type(tree);

    let mut entry = SymTabEntry::with_scope(
        &name,
        SymbolKind::Method,
        Rc::clone(&class_scope),
        false,
        Rc::clone(&method_scope),
    );
    if let Some(t) = method_typ { entry.set_typ(t); }

    if let Err(_) = class_scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }
}

/// Build a `MethodType` from a `MethodDecl` tree (read-only, no mutation).
fn build_method_type(method_decl: &Tree) -> Option<TypeInfo> {
    // MethodDecl → MethodHeader Block
    // MethodHeader → MethodReturnVal MethodDeclarator
    // MethodDeclarator → IDENTIFIER FormalParm*
    let header = method_decl.kids.first()?;
    if header.sym != "MethodHeader" { return None; }

    let return_node = header.kids.first()?;
    let return_typ = type_node_to_typeinfo(return_node)?;

    let decl = header.kids.get(1)?;
    // FormalParm nodes are kids[1..] of MethodDeclarator
    let parms = mksig_from_tree(&decl.kids[1..]);

    Some(TypeInfo::method(return_typ, parms))
}

/// Like `mksig` but works on a read-only slice (no `typ` stamped yet).
fn mksig_from_tree(parms: &[Tree]) -> Vec<jzero_symtab::Parameter> {
    parms
        .iter()
        .filter(|p| p.sym == "FormalParm")
        .map(|p| {
            let name = extract_identifier_name(&p.kids[1]).unwrap_or_default();
            let base_typ = type_node_to_typeinfo(&p.kids[0])
                .unwrap_or_else(TypeInfo::unknown);
            let typ = if p.kids[1].rule == 1 {
                TypeInfo::array(base_typ)
            } else {
                base_typ
            };
            jzero_symtab::Parameter::new(&name, typ)
        })
        .collect()
}

/// Walk a MethodDecl fully (second pass: params + body).
fn walk_method(
    tree: &mut Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    let name = method_name(tree);

    let method_scope = class_scope
        .borrow()
        .lookup_local(&name)
        .and_then(|e| e.st.clone())
        .unwrap_or_else(|| SymTab::new("method", Some(Rc::clone(&class_scope))).into_rc());

    tree.set_stab(Rc::clone(&method_scope));

    // Insert "return" dummy symbol with the method's return type
    if let Some(return_typ) = get_return_type(tree) {
        let mut ret_entry = SymTabEntry::new(
            "return",
            SymbolKind::Local,
            Rc::clone(&method_scope),
            false,
        );
        ret_entry.set_typ(return_typ);
        // Ignore error — may already be present if walk_method is called twice
        let _ = method_scope.borrow_mut().insert(ret_entry);
    }

    walk_children(tree, Rc::clone(&method_scope), errors);
}

/// Extract the return type from a MethodDecl node (read-only).
fn get_return_type(method_decl: &Tree) -> Option<TypeInfo> {
    let header = method_decl.kids.first()?;
    if header.sym != "MethodHeader" { return None; }
    let return_node = header.kids.first()?;
    type_node_to_typeinfo(return_node)
}

// ─── FormalParm ───────────────────────────────────────────────────────────────

fn walk_formal_parm(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    if tree.kids.len() < 2 { return; }

    let typ = calc_type(&mut tree.kids[0], errors);
    let final_typ = typ.and_then(|t| assign_type(&mut tree.kids[1], t, errors));

    let ident_node = &tree.kids[1];
    let (name, lineno) = ident_name_and_line(ident_node);

    let mut entry = SymTabEntry::new(&name, SymbolKind::Param, Rc::clone(&scope), false);
    if let Some(t) = final_typ { entry.set_typ(t); }
    if let Err(_) = scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }

    walk_children(tree, scope, errors);
}

// ─── FieldDecl (second pass) ──────────────────────────────────────────────────

fn walk_field_decl(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    // Types already registered in first pass; walk children for initialiser exprs
    walk_children(tree, scope, errors);
}

// ─── LocalVarDecl ─────────────────────────────────────────────────────────────

fn walk_local_var_decl(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    if tree.kids.len() < 2 { return; }

    let typ = calc_type(&mut tree.kids[0], errors);
    let final_typ = typ.and_then(|t| assign_type(&mut tree.kids[1], t, errors));

    let var_decl = &tree.kids[1];
    let (name, lineno) = declarator_name_and_line(var_decl);

    let mut entry = SymTabEntry::new(&name, SymbolKind::Local, Rc::clone(&scope), false);
    if let Some(t) = final_typ { entry.set_typ(t); }
    if let Err(_) = scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }

    walk_children(tree, scope, errors);
}

// ─── Block ────────────────────────────────────────────────────────────────────

fn walk_block(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    walk_children(tree, scope, errors);
}

// ─── Identifier helpers ───────────────────────────────────────────────────────

fn ident_name_and_line(node: &Tree) -> (String, usize) {
    if let Some(ref tok) = node.tok {
        (tok.text.clone(), tok.lineno)
    } else {
        declarator_name_and_line(node)
    }
}

fn declarator_name_and_line(node: &Tree) -> (String, usize) {
    if let Some(ref tok) = node.tok {
        return (tok.text.clone(), tok.lineno);
    }
    if let Some(first) = node.kids.first() {
        return declarator_name_and_line(first);
    }
    (String::new(), 0)
}

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

fn method_name(method_decl: &Tree) -> String {
    find_method_declarator(method_decl)
        .and_then(|md| md.kids.first())
        .and_then(|n| n.tok.as_ref())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

fn method_lineno(method_decl: &Tree) -> usize {
    find_method_declarator(method_decl)
        .and_then(|md| md.kids.first())
        .and_then(|n| n.tok.as_ref())
        .map(|t| t.lineno)
        .unwrap_or(0)
}

fn find_method_declarator(node: &Tree) -> Option<&Tree> {
    if node.sym == "MethodDeclarator" { return Some(node); }
    node.kids.iter().find_map(find_method_declarator)
}