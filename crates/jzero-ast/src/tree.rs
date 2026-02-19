use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

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
        }
    }

    /// Returns true if this is a leaf node.
    pub fn is_leaf(&self) -> bool {
        self.tok.is_some()
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

    /// Emit node declarations.
    fn dot_nodes(&self, buf: &mut String) {
        if let Some(ref tok) = self.tok {
            // Leaf node: two labels like the book
            // First: compact label with text and id
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box label=\"{}:{} id {}\"];\n",
                self.id, tok.text, tok.category, self.id
            )));
            // Second: dotted-style detailed label
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box style=dotted label=\" {} \\n text = {} \\l lineno = {} \\l\"];\n",
                self.id, tok.category, tok.text, tok.lineno
            )));
        } else {
            // Internal node
            buf.push_str(&fmt::format(format_args!(
                "N{} [shape=box label=\"{}#{}\"];\n",
                self.id, self.sym, self.rule
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
            buf.push_str(&fmt::format(format_args!(
                "{}{}#{} ({} kids)\n",
                pad, self.sym, self.rule, self.nkids
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
    }

    #[test]
    fn test_internal_node() {
        reset_ids();
        let left = Tree::leaf("IDENTIFIER", "x", 1);
        let op = Tree::leaf("ASSIGN", "=", 1);
        let right = Tree::leaf("INTLIT", "42", 1);
        let assign = Tree::new("Assignment", 0, vec![left, op, right]);
        assert!(!assign.is_leaf());
        assert_eq!(assign.nkids, 3);
        assert_eq!(assign.kids.len(), 3);
    }

    #[test]
    fn test_single_child_passthrough() {
        // Demonstrates that single-child rules should NOT create nodes.
        // The parser action code will just use `=> <>` to pass through.
        reset_ids();
        let leaf = Tree::leaf("INTLIT", "42", 1);
        // Instead of: Tree::new("Expr", 0, vec![leaf])
        // We just pass `leaf` through directly.
        assert!(leaf.is_leaf());
        assert_eq!(leaf.sym, "INTLIT");
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
        let val = Tree::leaf("INTLIT", "42", 1);
        let op = Tree::leaf("ASSIGN", "=", 1);
        let assign = Tree::new("Assignment", 0, vec![name, op, val]);

        let text = assign.to_text(0);
        assert!(text.contains("Assignment#0"));
        assert!(text.contains("[IDENTIFIER] x"));
        assert!(text.contains("[INTLIT] 42"));
    }
}