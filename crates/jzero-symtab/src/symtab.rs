use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::entry::SymTabEntry;

/// A single scope's symbol table.
///
/// Scopes form a tree: each table holds an optional reference to its
/// enclosing (parent) table. Lookup walks outward from innermost to outermost.
#[derive(Debug)]
pub struct SymTab {
    /// Human-readable scope name, e.g. "global", "hello", "main".
    pub scope: String,
    /// Enclosing scope, `None` only for the global scope.
    pub parent: Option<Rc<RefCell<SymTab>>>,
    /// Symbols declared in this scope, in insertion order.
    /// We use a `Vec` of `(name, entry)` pairs rather than a plain `HashMap`
    /// so that printing preserves declaration order (matches book output).
    entries: Vec<(String, SymTabEntry)>,
}

impl SymTab {
    /// Create a new symbol table for the given scope.
    pub fn new(scope: &str, parent: Option<Rc<RefCell<SymTab>>>) -> Self {
        SymTab {
            scope: scope.to_string(),
            parent,
            entries: Vec::new(),
        }
    }

    /// Wrap `self` in `Rc<RefCell<...>>` for sharing.
    pub fn into_rc(self) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(self))
    }

    /// Number of symbols in this scope.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a symbol into this scope.
    /// Returns `Err(existing)` if the name is already declared here
    /// (used for redeclaration checking).
    pub fn insert(&mut self, entry: SymTabEntry) -> Result<(), SymTabEntry> {
        if let Some((_, existing)) = self.entries.iter().find(|(k, _)| k == &entry.sym) {
            return Err(existing.clone());
        }
        self.entries.push((entry.sym.clone(), entry));
        Ok(())
    }

    /// Look up a name in this scope only (no parent walk).
    pub fn lookup_local(&self, name: &str) -> Option<&SymTabEntry> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// Look up a name starting from this scope, walking outward to parents.
    pub fn lookup(&self, name: &str) -> Option<SymTabEntry> {
        if let Some((_, e)) = self.entries.iter().find(|(k, _)| k == name) {
            return Some(e.clone());
        }
        self.parent.as_ref()?.borrow().lookup(name)
    }

    /// Iterate over entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &(String, SymTabEntry)> {
        self.entries.iter()
    }

    // ─── Printing ────────────────────────────────────────────────────────────

    /// Print the symbol table in the book's format, e.g.:
    ///
    /// ```text
    /// global - 2 symbols
    ///  hello
    ///   class - 2 symbols
    ///    main
    ///     method - 0 symbols
    /// ```
    pub fn print(&self, indent: usize) {
        let pad = " ".repeat(indent);
        println!("{}{} - {} symbols", pad, self.scope, self.len());
        for (name, entry) in &self.entries {
            let child_pad = " ".repeat(indent + 1);
            println!("{}{}", child_pad, name);
            if let Some(ref child_st) = entry.st {
                child_st.borrow().print(indent + 2);
            }
        }
    }
}