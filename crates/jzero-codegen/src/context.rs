use std::collections::HashMap;
use crate::address::{Address, Region};
use crate::tac::Tac;

/// Per-node codegen state, stored parallel to the AST.
/// Keyed by `Tree::id`.
#[derive(Debug, Default, Clone)]
pub struct NodeInfo {
    /// Intermediate code instructions synthesized for this subtree.
    pub icode: Vec<Tac>,
    /// Address where this node's computed value can be found.
    pub addr: Option<Address>,
    /// Label for the first instruction of this node's code (entry point).
    pub first: Option<Address>,
    /// Label for whatever instruction follows this node's code (exit point).
    pub follow: Option<Address>,
    /// Label to jump to when a Boolean expression is true.
    pub on_true: Option<Address>,
    /// Label to jump to when a Boolean expression is false.
    pub on_false: Option<Address>,
}

/// A string literal entry in the string pool.
#[derive(Debug, Clone)]
pub struct StringEntry {
    /// Lab-region label printed before the string declaration (e.g. `L0:`).
    pub label: Address,
    /// Offset in the strings region — used in instructions (`strings:0`).
    pub string_offset: i64,
    pub value: String,
}

/// All codegen state that lives outside the AST and symbol table.
pub struct CodegenContext {
    /// Monotonically increasing counter for fresh label ids.
    label_counter: i64,
    /// Per-method: current byte offset for the next local allocation.
    /// Reset to 8 at the start of each method (loc:0 reserved for self).
    local_offset: i64,
    /// Maps each AST node id to its codegen attributes.
    pub node_info: HashMap<u32, NodeInfo>,
    /// Maps symbol table entry keys (scope::name) to their Address.
    pub var_addrs: HashMap<String, Address>,
    /// String literal pool.
    pub strings: Vec<StringEntry>,
    /// Next offset in the strings region.
    strings_offset: i64,
    /// Global variable declarations (name, address).
    pub globals: Vec<(String, Address)>,
    /// Next offset in the global region.
    global_offset: i64,
}

impl CodegenContext {
    pub fn new() -> Self {
        Self {
            label_counter:  0,
            local_offset:   8,   // loc:0 reserved for self
            node_info:      HashMap::new(),
            var_addrs:      HashMap::new(),
            strings:        Vec::new(),
            strings_offset: 0,
            globals:        Vec::new(),
            global_offset:  0,
        }
    }

    // ── Label generation ────────────────────────────────────────────────────

    /// Mint a fresh, unique label address.
    pub fn genlabel(&mut self) -> Address {
        let id = self.label_counter;
        self.label_counter += 1;
        Address::lab(id)
    }

    // ── Local variable allocation ────────────────────────────────────────────

    /// Allocate one 8-byte word in the current method's local region.
    /// Returns the address of the newly allocated slot.
    pub fn genlocal(&mut self) -> Address {
        let addr = Address::loc(self.local_offset);
        self.local_offset += 8;
        addr
    }

    /// Reset the local offset for a new method body.
    /// Call this at the start of each method.
    pub fn reset_locals(&mut self) {
        self.local_offset = 8;
    }

    // ── Global variable allocation ───────────────────────────────────────────

    /// Allocate one 8-byte slot in the global region for `name`.
    pub fn alloc_global(&mut self, name: &str) -> Address {
        let addr = Address::global(self.global_offset);
        self.globals.push((name.to_string(), addr.clone()));
        self.global_offset += 8;
        addr
    }

    // ── String pool ──────────────────────────────────────────────────────────

    /// Intern a string literal. Returns the strings-region address.
    ///
    /// Each string gets two addresses:
    /// - a `Lab` address used as the label printed before the string declaration
    /// - a `Strings` address used when referencing the string in instructions
    pub fn intern_string(&mut self, value: &str) -> Address {
        // Deduplicate: reuse existing entry if present.
        if let Some(e) = self.strings.iter().find(|e| e.value == value) {
            return Address::new(Region::Strings, e.string_offset);
        }
        let lab   = self.genlabel();
        let offset = self.strings_offset;
        self.strings_offset += 8;
        self.strings.push(StringEntry {
            label: lab,
            string_offset: offset,
            value: value.to_string(),
        });
        Address::new(Region::Strings, offset)
    }

    // ── Variable address lookup ──────────────────────────────────────────────

    /// Look up the address assigned to a variable.
    /// `scope` is the `SymTab` that owns the entry, `name` is the symbol name.
    pub fn lookup_addr(
        &self,
        scope: &std::rc::Rc<std::cell::RefCell<jzero_symtab::SymTab>>,
        name: &str,
    ) -> Option<&Address> {
        let key = format!("{:p}::{}", std::rc::Rc::as_ptr(scope), name);
        self.var_addrs.get(&key)
    }

    // ── Node info accessors ──────────────────────────────────────────────────

    pub fn node(&self, id: u32) -> Option<&NodeInfo> {
        self.node_info.get(&id)
    }

    pub fn node_mut(&mut self, id: u32) -> &mut NodeInfo {
        self.node_info.entry(id).or_default()
    }
}