//! `mkcls` pass — builds full `ClassType` for every `ClassDecl` node.
//!
//! Runs after `build_symtabs` (so all symbol tables are populated) and
//! before `check_type` (so `InstanceCreation` nodes can look up class types).
//!
//! For each `ClassDecl`, looks up the class entry in the global scope,
//! walks its symbol table, partitions entries into fields vs methods,
//! and stamps a complete `ClassType` onto the entry's `typ` field.

use jzero_ast::tree::Tree;
use jzero_symtab::{ClassType, Parameter, TypeInfo};

/// Entry point — walk the whole tree looking for `ClassDecl` nodes.
pub fn mkcls(tree: &mut Tree) {
    if tree.sym == "ClassDecl" {
        build_class_type(tree);
    } else {
        for kid in &mut tree.kids {
            mkcls(kid);
        }
    }
}

/// Build and stamp the `ClassType` for a single `ClassDecl` node.
///
/// Tree shape:
///   ClassDecl#0 → IDENTIFIER  [MethodDecl | FieldDecl | ConstructorDecl]*
///
/// kids[0] = IDENTIFIER (class name leaf)
fn build_class_type(tree: &mut Tree) {
    // Get the class name
    let class_name = match tree.kids.first().and_then(|n| n.tok.as_ref()) {
        Some(tok) => tok.text.clone(),
        None => return,
    };

    // Get the class stab entry via the stab attached to this node
    let stab = match &tree.stab {
        Some(s) => s.clone(),
        None => return,
    };

    // Look up the class entry — it's in the parent (global) scope
    let parent_stab = match stab.borrow().parent.clone() {
        Some(p) => p,
        None => {
            // stab IS the global scope if this ClassDecl has the class scope attached;
            // try looking up in stab itself as fallback
            stab.clone()
        }
    };

    let class_entry = match parent_stab.borrow().lookup(&class_name) {
        Some(e) => e,
        None => return,
    };

    // Get the class's own symbol table
    let class_st = match class_entry.st {
        Some(ref st) => st.clone(),
        None => return,
    };

    // Partition entries into methods and fields
    let mut methods: Vec<Parameter> = Vec::new();
    let mut fields: Vec<Parameter> = Vec::new();

    for (name, entry) in class_st.borrow().iter() {
        if let Some(ref typ) = entry.typ {
            if typ.str().starts_with("method") {
                methods.push(Parameter::new(name, typ.clone()));
            } else {
                fields.push(Parameter::new(name, typ.clone()));
            }
        }
    }

    // Build the ClassType
    let class_type = TypeInfo::Class(ClassType {
        name: class_name.clone(),
        st: Some(class_st),
        methods,
        fields,
        constrs: Vec::new(),
    });

    // Stamp it onto the class entry in the parent scope
    if let Some((_, entry)) = parent_stab
        .borrow_mut()
        .iter_mut()
        .find(|(k, _)| *k == &class_name)
    {
        entry.set_typ(class_type);
    }
}