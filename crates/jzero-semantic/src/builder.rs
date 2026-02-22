//! Symbol table builder — walks the syntax tree and constructs
//! one `SymTab` per scope, attaching each to the relevant `Tree` nodes.
//!
//! Pass structure (two-pass per class, matching the book):
//!
//! 1. **`walk`** — the main recursive traversal. Dispatches to specialised
//!    handlers based on `tree.sym`. Propagates `stab` top-down (inherited).
//!
//! 2. Class body uses a **two-pass** approach:
//!    - First pass: register all class-level members (fields + method signatures)
//!    - Second pass: walk method bodies with full class scope visible
//!    This allows forward references within a class.

use std::cell::RefCell;
use std::rc::Rc;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, SymTabEntry, entry::SymbolKind};

use crate::error::SemanticError;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Walk `tree`, building symbol tables under `current_scope`.
/// Attaches the scope to each node as an inherited `stab` attribute.
/// Semantic errors are appended to `errors`.
pub fn build_symtabs(
    tree: &mut Tree,
    current_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    // Attach the current scope to this node (inherited attribute)
    tree.set_stab(Rc::clone(&current_scope));

    match tree.sym.as_str() {
        "ClassDecl" => walk_class(tree, current_scope, errors),
        "MethodDecl" => walk_method(tree, current_scope, errors),
        "FieldDecl" => walk_field_decl(tree, current_scope, errors),
        "LocalVarDecl" => walk_local_var_decl(tree, current_scope, errors),
        "FormalParm" => walk_formal_parm(tree, current_scope, errors),
        "Block" => walk_block(tree, current_scope, errors),
        _ => walk_children(tree, current_scope, errors),
    }
}

// ─── Generic child walker ─────────────────────────────────────────────────────

/// Walk all children in the current scope (no new scope introduced).
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

/// Handle a ClassDecl node.
///
/// Tree shape (from grammar):
///   ClassDecl#0 → IDENTIFIER  [MethodDecl | FieldDecl]*
///
/// kids[0]     = IDENTIFIER (class name)
/// kids[1..]   = class body members
fn walk_class(
    tree: &mut Tree,
    global: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    // Extract class name from kids[0]
    let class_name = match tree.kids.first() {
        Some(n) => n.tok.as_ref().map(|t| t.text.clone()).unwrap_or_default(),
        None => return,
    };
    let lineno = tree.kids.first()
        .and_then(|n| n.tok.as_ref())
        .map(|t| t.lineno)
        .unwrap_or(0);

    // Create the class scope (parent = global)
    let class_scope = SymTab::new("class", Some(Rc::clone(&global))).into_rc();

    // Register the class in the global scope
    let class_entry = SymTabEntry::with_scope(
        &class_name,
        SymbolKind::Class,
        Rc::clone(&global),
        false,
        Rc::clone(&class_scope),
    );
    if let Err(_existing) = global.borrow_mut().insert(class_entry) {
        errors.push(SemanticError::RedeclaredVariable { name: class_name.clone(), lineno });
    }

    // Attach class scope to the ClassDecl node itself
    tree.set_stab(Rc::clone(&class_scope));

    // ── First pass: register fields + method signatures ──────────────────────
    // This allows methods to reference fields/methods declared later in the class.
    for kid in &tree.kids[1..] {
        match kid.sym.as_str() {
            "FieldDecl" => register_field(kid, Rc::clone(&class_scope), errors),
            "MethodDecl" => register_method_signature(kid, Rc::clone(&class_scope), errors),
            _ => {}
        }
    }

    // ── Second pass: walk method bodies ──────────────────────────────────────
    for kid in &mut tree.kids[1..] {
        if kid.sym == "MethodDecl" {
            walk_method(kid, Rc::clone(&class_scope), errors);
        } else {
            build_symtabs(kid, Rc::clone(&class_scope), errors);
        }
    }
}

// ─── FieldDecl registration (first pass, read-only) ──────────────────────────

fn register_field(
    tree: &Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    // FieldDecl shape: Type IDENTIFIER [= Expr]
    // The identifier is in kids[1] (kids[0] is the type)
    if tree.kids.len() < 2 {
        return;
    }
    let ident = &tree.kids[1];
    let name = match ident.tok.as_ref() {
        Some(t) => t.text.clone(),
        None => return,
    };
    let lineno = ident.tok.as_ref().map(|t| t.lineno).unwrap_or(0);

    let entry = SymTabEntry::new(&name, SymbolKind::Field, Rc::clone(&class_scope), false);
    if let Err(_) = class_scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }
}

// ─── MethodDecl ───────────────────────────────────────────────────────────────

/// Register a method's name in the class scope (first pass).
fn register_method_signature(
    tree: &Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    let name = method_name(tree);
    let lineno = method_lineno(tree);

    // Create the method scope now so the entry can reference it.
    // It will be populated with params + locals during the second pass.
    let method_scope = SymTab::new("method", Some(Rc::clone(&class_scope))).into_rc();

    let entry = SymTabEntry::with_scope(
        &name,
        SymbolKind::Method,
        Rc::clone(&class_scope),
        false,
        Rc::clone(&method_scope),
    );
    if let Err(_) = class_scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }
}

/// Walk a MethodDecl node fully (second pass: params + body).
///
/// Tree shape:
///   MethodDecl#0 → MethodHeader  Block
///   MethodHeader → return-type  MethodDeclarator
///   MethodDeclarator → IDENTIFIER  FormalParm*
fn walk_method(
    tree: &mut Tree,
    class_scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    let name = method_name(tree);

    // Retrieve the method scope that was created in register_method_signature.
    // If it wasn't registered (e.g. isolated test), create a fresh one.
    let method_scope = class_scope
        .borrow()
        .lookup_local(&name)
        .and_then(|e| e.st.clone())
        .unwrap_or_else(|| {
            SymTab::new("method", Some(Rc::clone(&class_scope))).into_rc()
        });

    // Attach method scope to the MethodDecl node
    tree.set_stab(Rc::clone(&method_scope));

    // Walk all children within the method scope
    walk_children(tree, Rc::clone(&method_scope), errors);
}

// ─── FormalParm ───────────────────────────────────────────────────────────────

/// FormalParm shape: Type IDENTIFIER  or  Type IDENTIFIER[]
/// kids[0] = type leaf, kids[1] = IDENTIFIER (possibly wrapped in VarDeclarator)
fn walk_formal_parm(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    if tree.kids.len() < 2 {
        return;
    }
    let ident_node = &tree.kids[1];
    let (name, lineno) = ident_name_and_line(ident_node);

    let entry = SymTabEntry::new(&name, SymbolKind::Param, Rc::clone(&scope), false);
    if let Err(_) = scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }

    walk_children(tree, scope, errors);
}

// ─── FieldDecl (full walk, second pass) ──────────────────────────────────────

fn walk_field_decl(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    // Already registered in first pass; just walk children for initialiser exprs
    walk_children(tree, scope, errors);
}

// ─── LocalVarDecl ─────────────────────────────────────────────────────────────

/// LocalVarDecl shape: Type VarDeclarator
/// kids[0] = type leaf, kids[1] = VarDeclarator
/// VarDeclarator contains the IDENTIFIER (possibly nested for arrays)
fn walk_local_var_decl(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    if tree.kids.len() < 2 {
        return;
    }
    let var_decl = &tree.kids[1];
    let (name, lineno) = declarator_name_and_line(var_decl);

    let entry = SymTabEntry::new(&name, SymbolKind::Local, Rc::clone(&scope), false);
    if let Err(_) = scope.borrow_mut().insert(entry) {
        errors.push(SemanticError::RedeclaredVariable { name, lineno });
    }

    walk_children(tree, scope, errors);
}

// ─── Block ────────────────────────────────────────────────────────────────────

/// A Block does NOT introduce a new scope in Jzero (unlike full Java).
/// Local variables declared inside a block go into the enclosing method scope.
/// We simply walk the children in the same scope.
fn walk_block(
    tree: &mut Tree,
    scope: Rc<RefCell<SymTab>>,
    errors: &mut Vec<SemanticError>,
) {
    walk_children(tree, scope, errors);
}

// ─── Identifier helpers ───────────────────────────────────────────────────────

/// Extract (name, lineno) from a plain IDENTIFIER leaf node.
fn ident_name_and_line(node: &Tree) -> (String, usize) {
    if let Some(ref tok) = node.tok {
        (tok.text.clone(), tok.lineno)
    } else {
        // Might be a VarDeclarator wrapping an IDENTIFIER — drill down
        declarator_name_and_line(node)
    }
}

/// Extract the declared variable name from a VarDeclarator node.
///
/// VarDeclarator can be nested for arrays:
///   argv[]  →  VarDeclarator#1( VarDeclarator#0( IDENTIFIER("argv") ) )
/// We just keep unwrapping until we find a leaf.
fn declarator_name_and_line(node: &Tree) -> (String, usize) {
    if let Some(ref tok) = node.tok {
        return (tok.text.clone(), tok.lineno);
    }
    // Drill into first child
    if let Some(first) = node.kids.first() {
        return declarator_name_and_line(first);
    }
    (String::new(), 0)
}

/// Extract the method name from a MethodDecl node.
///
/// MethodDecl → MethodHeader Block
/// MethodHeader → (return-type) MethodDeclarator
/// MethodDeclarator → IDENTIFIER FormalParms
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

/// Recursively search for a MethodDeclarator node within a subtree.
fn find_method_declarator(node: &Tree) -> Option<&Tree> {
    if node.sym == "MethodDeclarator" {
        return Some(node);
    }
    node.kids.iter().find_map(find_method_declarator)
}