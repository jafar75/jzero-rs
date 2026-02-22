use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use jzero_symtab::SymTab;

/// Global counter for unique node IDs (used in DOT output).
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Reset the ID counter (useful for deterministic test output).
pub fn reset_ids() {
    NEXT_ID.store(1, Ordering::SeqCst);
}

fn next_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::SeqCst)
}

// ─── Leaf token info ─────────────────────────────────────

/// Token information stored in leaf nodes.
#[derive(Debug, Clone)]
pub struct LeafToken {
    /// Token category name, e.g. "IDENTIFIER", "INTLIT", "PLUS"
    pub category: String,
    /// The actual source text, e.g. "main", "42", "+"
    pub text: String,
    /// Source line number (1-based)
    pub lineno: usize,
}

// ─── Tree node ───────────────────────────────────────────

/// A syntax tree node.
///
/// - **Leaf nodes**: `tok` is `Some(...)`, `kids` is empty.
///   Created from terminal symbols (tokens).
///
/// - **Internal nodes**: `tok` is `None`, `kids` has 2+ children.
///   Created when a production rule branches (2+ RHS symbols).
///
/// Single-child productions do NOT create a node — the child
/// is passed through directly. This makes it a *syntax tree*
/// (not a full parse tree).
///
/// # Semantic attributes (Chapter 6+)
///
/// - `is_const`: synthesized — true if this subtree is a compile-time constant.
///   Computed bottom-up by `jzero-semantic`.
/// - `stab`: inherited — the nearest enclosing scope's symbol table.
///   Propagated top-down by `jzero-semantic`; `None` until semantic analysis runs.
#[derive(Debug, Clone)]
pub struct Tree {
    /// Unique node ID for DOT output.
    pub id: u32,
    /// Production rule name (internal) or token category (leaf).
    pub sym: String,
    /// Which alternative of the rule (0-based). -1 for leaves.
    pub rule: i32,
    /// Number of children.
    pub nkids: usize,
    /// Token info, only for leaf nodes.
    pub tok: Option<LeafToken>,
    /// Child nodes.
    pub kids: Vec<Tree>,

    // ─── Semantic attributes ─────────────────────────────
    /// Synthesized attribute: true if this node is a compile-time constant.
    /// `None` until computed by semantic analysis.
    pub is_const: Option<bool>,
    /// Inherited attribute: the nearest enclosing scope's symbol table.
    /// `None` until populated by semantic analysis.
    pub stab: Option<Rc<RefCell<SymTab>>>,
}

impl Tree {
    // ─── Constructors ────────────────────────────────────

    /// Create a leaf node from a terminal symbol.
    pub fn leaf(category: &str, text: &str, lineno: usize) -> Self {
        Tree {
            id: next_id(),
            sym: category.to_string(),
            rule: -1,
            nkids: 0,
            tok: Some(LeafToken {
                category: category.to_string(),
                text: text.to_string(),
                lineno,
            }),
            kids: Vec::new(),
            is_const: None,
            stab: None,
        }
    }

    /// Create an internal node from a production rule.
    ///
    /// `sym` is the production rule name (e.g. "ClassDecl", "MethodCall").
    /// `rule` is the alternative number (0-based).
    /// `kids` are the child nodes.
    pub fn new(sym: &str, rule: i32, kids: Vec<Tree>) -> Self {
        let nkids = kids.len();
        Tree {
            id: next_id(),
            sym: sym.to_string(),
            rule,
            nkids,
            tok: None,
            kids,
            is_const: None,
            stab: None,
        }
    }

    /// Returns true if this is a leaf node.
    pub fn is_leaf(&self) -> bool {
        self.tok.is_some()
    }

    // ─── Semantic attribute helpers ───────────────────────

    /// Attach a symbol table to this node (sets the `stab` attribute).
    pub fn set_stab(&mut self, st: Rc<RefCell<SymTab>>) {
        self.stab = Some(st);
    }

    /// Set the `is_const` synthesized attribute.
    pub fn set_const(&mut self, val: bool) {
        self.is_const = Some(val);
    }

    // ─── DOT output ──────────────────────────────────────

    /// Generate a complete DOT (Graphviz) representation of this tree.
    pub fn to_dot(&self) -> String {
        let mut buf = String::new();
        buf.push_str("digraph {\n");
        self.dot_nodes(&mut buf);
        self.dot_edges(&mut buf);
        buf.push_str("}\n");
        buf
    }

    /// Escape a string for use inside DOT double-quoted labels.
    fn dot_escape(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    }

    /// Emit node declarations.
    fn dot_nodes(&self, buf: &mut String) {
        if let Some(ref tok) = self.tok {
            let escaped = Self::dot_escape(&tok.text);
            // Leaf node: two labels like the book
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box label=\"{}:{} id {}\"];\n",
                self.id, escaped, tok.category, self.id
            )));
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box style=dotted label=\" {} \\n text = {} \\l lineno = {} \\l\"];\n",
                self.id, tok.category, escaped, tok.lineno
            )));
        } else {
            // Internal node — include is_const in label if computed
            let const_label = match self.is_const {
                Some(true)  => " ✓const",
                Some(false) => "",
                None        => "",
            };
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box label=\"{}#{}{}\"];\n",
                self.id, self.sym, self.rule, const_label
            )));
        }

        for kid in &self.kids {
            kid.dot_nodes(buf);
        }
    }

    /// Emit edges from parent to children.
    fn dot_edges(&self, buf: &mut String) {
        for kid in &self.kids {
            buf.push_str(&fmt::format(format_args!(
                "N{} -> N{};\n",
                self.id, kid.id
            )));
        }
        for kid in &self.kids {
            kid.dot_edges(buf);
        }
    }

    // ─── Text output (for testing) ───────────────────────

    /// Print the tree in a simple indented text format.
    pub fn to_text(&self, indent: usize) -> String {
        let mut buf = String::new();
        let pad = "  ".repeat(indent);
        if let Some(ref tok) = self.tok {
            buf.push_str(&fmt::format(format_args!(
                "{}[{}] {} (line {})\n",
                pad, tok.category, tok.text, tok.lineno
            )));
        } else {
            let const_label = match self.is_const {
                Some(true)  => " [const]",
                Some(false) => "",
                None        => "",
            };
            buf.push_str(&fmt::format(format_args!(
                "{}{}#{} ({} kids){}\n",
                pad, self.sym, self.rule, self.nkids, const_label
            )));
        }
        for kid in &self.kids {
            buf.push_str(&kid.to_text(indent + 1));
        }
        buf
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_text(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_node() {
        reset_ids();
        let leaf = Tree::leaf("IDENTIFIER", "hello", 1);
        assert!(leaf.is_leaf());
        assert_eq!(leaf.nkids, 0);
        assert_eq!(leaf.tok.as_ref().unwrap().text, "hello");
        assert_eq!(leaf.tok.as_ref().unwrap().lineno, 1);
        assert!(leaf.is_const.is_none());
        assert!(leaf.stab.is_none());
    }

    #[test]
    fn test_internal_node() {
        reset_ids();
        let left  = Tree::leaf("IDENTIFIER", "x", 1);
        let op    = Tree::leaf("ASSIGN", "=", 1);
        let right = Tree::leaf("INTLIT", "42", 1);
        let assign = Tree::new("Assignment", 0, vec![left, op, right]);
        assert!(!assign.is_leaf());
        assert_eq!(assign.nkids, 3);
        assert!(assign.is_const.is_none());
        assert!(assign.stab.is_none());
    }

    #[test]
    fn test_single_child_passthrough() {
        reset_ids();
        let leaf = Tree::leaf("INTLIT", "42", 1);
        assert!(leaf.is_leaf());
        assert_eq!(leaf.sym, "INTLIT");
    }

    #[test]
    fn test_set_const() {
        reset_ids();
        let mut lit = Tree::leaf("INTLIT", "42", 1);
        lit.set_const(true);
        assert_eq!(lit.is_const, Some(true));
    }

    #[test]
    fn test_set_stab() {
        reset_ids();
        let st = SymTab::new("global", None).into_rc();
        let mut node = Tree::new("ClassDecl", 0, vec![]);
        node.set_stab(Rc::clone(&st));
        assert!(node.stab.is_some());
        assert_eq!(node.stab.as_ref().unwrap().borrow().scope, "global");
    }

    #[test]
    fn test_dot_output() {
        reset_ids();
        let name = Tree::leaf("IDENTIFIER", "hello", 1);
        let body = Tree::new("ClassBody", 1, vec![]);
        let class = Tree::new("ClassDecl", 0, vec![name, body]);

        let dot = class.to_dot();
        assert!(dot.contains("digraph {"));
        assert!(dot.contains("N3 [shape=box label=\"ClassDecl#0\"]"));
        assert!(dot.contains("N3 -> N1"));
        assert!(dot.contains("N3 -> N2"));
        assert!(dot.contains("IDENTIFIER"));
    }

    #[test]
    fn test_text_output() {
        reset_ids();
        let name = Tree::leaf("IDENTIFIER", "x", 1);
        let val  = Tree::leaf("INTLIT", "42", 1);
        let op   = Tree::leaf("ASSIGN", "=", 1);
        let assign = Tree::new("Assignment", 0, vec![name, op, val]);

        let text = assign.to_text(0);
        assert!(text.contains("Assignment#0"));
        assert!(text.contains("[IDENTIFIER] x"));
        assert!(text.contains("[INTLIT] 42"));
    }

    #[test]
    fn test_const_label_in_text_output() {
        reset_ids();
        let mut node = Tree::new("AddExpr", 0, vec![]);
        node.set_const(true);
        let text = node.to_text(0);
        assert!(text.contains("[const]"));
    }
}