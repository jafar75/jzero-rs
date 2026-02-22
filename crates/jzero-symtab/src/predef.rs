use std::cell::RefCell;
use std::rc::Rc;

use crate::entry::{SymTabEntry, SymbolKind};
use crate::symtab::SymTab;

/// Build the predefined `System.out.println` scope hierarchy and insert it
/// into the given global scope.
///
/// After this call, the global scope contains a `System` class entry whose
/// child scope contains `out`, whose child scope contains `println`.
///
/// This matches the book's predefined symbol layout:
/// ```text
/// System
///   class - 1 symbols
///    out
///      class - 1 symbols
///       println
/// ```
pub fn build_predefined(global: &Rc<RefCell<SymTab>>) {
    // println scope (empty — no local vars)
    let println_st = SymTab::new("method", Some(Rc::clone(global))).into_rc();

    // out scope — contains println
    let out_st = SymTab::new("class", Some(Rc::clone(global))).into_rc();
    let println_entry = SymTabEntry::with_scope(
        "println",
        SymbolKind::Method,
        Rc::clone(&out_st),
        false,
        Rc::clone(&println_st),
    );
    out_st.borrow_mut().insert(println_entry).expect("predefined insert failed");

    // System scope — contains out
    let system_st = SymTab::new("class", Some(Rc::clone(global))).into_rc();
    let out_entry = SymTabEntry::with_scope(
        "out",
        SymbolKind::Class,
        Rc::clone(&system_st),
        false,
        Rc::clone(&out_st),
    );
    system_st.borrow_mut().insert(out_entry).expect("predefined insert failed");

    // Insert System into global
    let system_entry = SymTabEntry::with_scope(
        "System",
        SymbolKind::Class,
        Rc::clone(global),
        false,
        Rc::clone(&system_st),
    );
    global.borrow_mut().insert(system_entry).expect("predefined insert failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symtab::SymTab;

    #[test]
    fn test_predefined_structure() {
        let global = SymTab::new("global", None).into_rc();
        build_predefined(&global);

        let g = global.borrow();

        // Global has System
        let system_entry = g.lookup_local("System").expect("System not found");
        assert_eq!(system_entry.kind, SymbolKind::Class);

        // System scope has out
        let system_st = system_entry.st.as_ref().expect("System has no child scope");
        let out_entry = system_st.borrow().lookup_local("out")
            .cloned()
            .expect("out not found");
        assert_eq!(out_entry.kind, SymbolKind::Class);

        // out scope has println
        let out_st = out_entry.st.as_ref().expect("out has no child scope").clone();
        let println_entry = out_st.borrow().lookup_local("println")
            .cloned()
            .expect("println not found");
        assert_eq!(println_entry.kind, SymbolKind::Method);
    }

    #[test]
    fn test_predefined_print() {
        let global = SymTab::new("global", None).into_rc();
        build_predefined(&global);
        // Visual check — prints to stdout when run with `cargo test -- --nocapture`
        global.borrow().print(0);
    }
}