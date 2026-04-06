//! Phase 2 — Variable layout pass.
//!
//! Walks the symbol table tree and assigns an [`Address`] to every symbol
//! that occupies memory, storing the result in [`CodegenContext::var_addrs`].
//!
//! Key map: `(scope_ptr, name)` where `scope_ptr` is the raw pointer of the
//! [`SymTab`] that owns the entry.  Using the pointer keeps the key unique
//! even when two scopes share the same `scope` string (e.g. two methods both
//! report `"method"`).

use std::cell::RefCell;
use std::rc::Rc;

use jzero_symtab::{SymTab, entry::SymbolKind};

use crate::address::{Address, Region};
use crate::context::CodegenContext;

/// Public entry point.  Call once after `analyze()` returns.
pub fn assign_addresses(
    global: &Rc<RefCell<SymTab>>,
    ctx: &mut CodegenContext,
) {
    walk_scope(global, ctx);
}

// ─── Scope walker ─────────────────────────────────────────────────────────────

fn walk_scope(scope: &Rc<RefCell<SymTab>>, ctx: &mut CodegenContext) {
    let scope_ref = scope.borrow();
    let scope_name = scope_ref.scope.clone();

    match scope_name.as_str() {
        "global" => walk_global_scope(&scope_ref, scope, ctx),
        "class"  => walk_class_scope(&scope_ref, scope, ctx),
        "method" => walk_method_scope(&scope_ref, scope, ctx),
        _        => {}
    }
}

// ─── Global scope ─────────────────────────────────────────────────────────────

fn walk_global_scope(
    scope_ref: &std::cell::Ref<SymTab>,
    scope: &Rc<RefCell<SymTab>>,
    ctx: &mut CodegenContext,
) {
    for (name, entry) in scope_ref.iter() {
        if entry.kind == SymbolKind::Class {
            // Each class gets one global slot (for the class object itself).
            let addr = ctx.alloc_global(name);
            let key = var_key(scope, name);
            ctx.var_addrs.insert(key, addr);

            // Recurse into the class scope.
            if let Some(ref child) = entry.st {
                walk_scope(child, ctx);
            }
        }
        // Predefined entries (System etc.) — also get a global slot.
        // They are inserted by build_predefined and have no child scope here.
    }
}

// ─── Class scope ──────────────────────────────────────────────────────────────

fn walk_class_scope(
    scope_ref: &std::cell::Ref<SymTab>,
    scope: &Rc<RefCell<SymTab>>,
    ctx: &mut CodegenContext,
) {
    let mut field_offset: i64 = 0;

    for (name, entry) in scope_ref.iter() {
        match entry.kind {
            SymbolKind::Field => {
                let addr = Address::new(Region::Class, field_offset);
                field_offset += 8;
                ctx.var_addrs.insert(var_key(scope, name), addr);
            }
            SymbolKind::Method => {
                // Methods don't get a data address; they get a code label.
                // We'll assign that during gencode. Just recurse into the
                // method scope so its locals get addresses.
                if let Some(ref child) = entry.st {
                    walk_scope(child, ctx);
                }
            }
            _ => {}
        }
    }
}

// ─── Method scope ─────────────────────────────────────────────────────────────

fn walk_method_scope(
    scope_ref: &std::cell::Ref<SymTab>,
    scope: &Rc<RefCell<SymTab>>,
    ctx: &mut CodegenContext,
) {
    // Reset local offset for this method: loc:0 reserved for self/this.
    ctx.reset_locals();

    for (name, entry) in scope_ref.iter() {
        // Skip the "return" dummy — it is not a real variable.
        if name == "return" {
            continue;
        }

        match entry.kind {
            SymbolKind::Param | SymbolKind::Local => {
                let addr = ctx.genlocal();
                ctx.var_addrs.insert(var_key(scope, name), addr);
            }
            _ => {}
        }
    }
}

// ─── Key helper ───────────────────────────────────────────────────────────────

/// Produce a unique key for a symbol in `scope` with `name`.
///
/// Uses the raw pointer of the `SymTab` allocation as the scope
/// discriminator, which is stable for the lifetime of the `Rc`.
pub fn var_key(scope: &Rc<RefCell<SymTab>>, name: &str) -> String {
    format!("{:p}::{}", Rc::as_ptr(scope), name)
}