use std::cell::RefCell;
use std::rc::Rc;

use crate::entry::SymTabEntry;

#[derive(Debug)]
pub struct SymTab {
    pub scope: String,
    pub parent: Option<Rc<RefCell<SymTab>>>,
    entries: Vec<(String, SymTabEntry)>,
}

impl SymTab {
    pub fn new(scope: &str, parent: Option<Rc<RefCell<SymTab>>>) -> Self {
        SymTab { scope: scope.to_string(), parent, entries: Vec::new() }
    }

    pub fn into_rc(self) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(self))
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn insert(&mut self, entry: SymTabEntry) -> Result<(), SymTabEntry> {
        if let Some((_, existing)) = self.entries.iter().find(|(k, _)| k == &entry.sym) {
            return Err(existing.clone());
        }
        self.entries.push((entry.sym.clone(), entry));
        Ok(())
    }

    pub fn lookup_local(&self, name: &str) -> Option<&SymTabEntry> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn lookup_local_mut(&mut self, name: &str) -> Option<&mut SymTabEntry> {
        self.entries.iter_mut().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn lookup(&self, name: &str) -> Option<SymTabEntry> {
        if let Some((_, e)) = self.entries.iter().find(|(k, _)| k == name) {
            return Some(e.clone());
        }
        self.parent.as_ref()?.borrow().lookup(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, SymTabEntry)> {
        self.entries.iter()
    }

    /// Mutable iterator — used by `mkcls` to stamp `ClassType` onto entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut SymTabEntry)> {
        self.entries.iter_mut().map(|(k, v)| (k as &String, v))
    }

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