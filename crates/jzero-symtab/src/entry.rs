use std::cell::RefCell;
use std::rc::Rc;

use crate::symtab::SymTab;

/// The kind of a symbol — determines what fields are relevant.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Class,
    Method,
    Field,
    Param,
    Local,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Class  => write!(f, "class"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Field  => write!(f, "field"),
            SymbolKind::Param  => write!(f, "param"),
            SymbolKind::Local  => write!(f, "local"),
        }
    }
}

/// One entry in a symbol table.
#[derive(Debug, Clone)]
pub struct SymTabEntry {
    /// The declared name.
    pub sym: String,
    /// The scope this entry was declared in.
    pub parent_st: Rc<RefCell<SymTab>>,
    /// Child scope — present only for classes and methods.
    pub st: Option<Rc<RefCell<SymTab>>>,
    /// Whether this symbol is a compile-time constant.
    pub is_const: bool,
    /// What kind of symbol this is.
    pub kind: SymbolKind,
}

impl SymTabEntry {
    /// Create a regular entry (local variable, field, parameter).
    pub fn new(
        sym: &str,
        kind: SymbolKind,
        parent: Rc<RefCell<SymTab>>,
        is_const: bool,
    ) -> Self {
        SymTabEntry {
            sym: sym.to_string(),
            parent_st: parent,
            st: None,
            is_const,
            kind,
        }
    }

    /// Create an entry that introduces a child scope (class or method).
    pub fn with_scope(
        sym: &str,
        kind: SymbolKind,
        parent: Rc<RefCell<SymTab>>,
        is_const: bool,
        child: Rc<RefCell<SymTab>>,
    ) -> Self {
        SymTabEntry {
            sym: sym.to_string(),
            parent_st: parent,
            st: Some(child),
            is_const,
            kind,
        }
    }
}