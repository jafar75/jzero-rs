//! Phase 5 — Expression type checking.
//!
//! Three cooperative functions mirroring the book's methods:
//!
//! - `check_type(tree, in_codeblock)` — main dispatcher; matches on `sym`
//!   to compute and verify the type of each expression node.
//! - `check_kids(tree, in_codeblock)` — selective child traversal; controls
//!   which children are visited and returns whether the parent is done.
//! - `check_types(tree, op1, op2)` — compatibility checker; enforces type
//!   rules for a given operator and records the result.
//!
//! Results (both OK and FAIL) are collected into `Vec<TypeCheckResult>`
//! rather than printed directly, so tests can assert on them precisely.

use std::cell::RefCell;
use std::rc::Rc;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, TypeInfo};

// ─── TypeCheckResult ─────────────────────────────────────────────────────────

/// The outcome of a single binary type check — mirrors the book's diagnostic
/// output: `"line 4: typecheck = on a int and a int -> OK"`
#[derive(Debug, Clone)]
pub struct TypeCheckResult {
    pub lineno: usize,
    pub operator: String,
    /// Type of operand 2 (printed first in the book's output)
    pub op2: String,
    /// Type of operand 1 (printed second in the book's output)
    pub op1: String,
    pub ok: bool,
}

impl TypeCheckResult {
    pub fn new(lineno: usize, operator: &str, op1: &TypeInfo, op2: &TypeInfo, ok: bool) -> Self {
        TypeCheckResult {
            lineno,
            operator: operator.to_string(),
            op1: op1.str(),
            op2: op2.str(),
            ok,
        }
    }
}

impl std::fmt::Display for TypeCheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}: typecheck {} on a {} and a {} -> {}",
            self.lineno,
            self.operator,
            self.op2,
            self.op1,
            if self.ok { "OK" } else { "FAIL" }
        )
    }
}

// ─── check_type ──────────────────────────────────────────────────────────────

/// Main type-checking dispatcher (post-order via check_kids).
///
/// - Calls `check_kids()` first to handle children selectively.
/// - If `check_kids` returns `true`, the current node is fully handled.
/// - If `!in_codeblock`, skips expression checking (declarations only).
/// - Otherwise matches on `sym` to compute and verify the node's type.
pub fn check_type(
    tree: &mut Tree,
    in_codeblock: bool,
    results: &mut Vec<TypeCheckResult>,
) {
    if check_kids(tree, in_codeblock, results) {
        return;
    }
    if !in_codeblock {
        return;
    }

    match tree.sym.as_str() {
        "Assignment" => {
            // kids[0]=lhs, kids[1]=op, kids[2]=rhs
            if let (Some(lhs), Some(rhs)) = (
                tree.kids.get(0).and_then(|k| k.typ.clone()),
                tree.kids.get(2).and_then(|k| k.typ.clone()),
            ) {
                let result = check_types(tree, &lhs, &rhs);
                let typ = if result.ok { Some(lhs.clone()) } else { None };
                results.push(result);
                if let Some(t) = typ { tree.set_typ(t); }
            }
        }

        "AddExpr" | "MulExpr" => {
            // kids[0]=lhs, kids[1]=op, kids[2]=rhs
            if let (Some(lhs), Some(rhs)) = (
                tree.kids.get(0).and_then(|k| k.typ.clone()),
                tree.kids.get(2).and_then(|k| k.typ.clone()),
            ) {
                let result = check_types(tree, &lhs, &rhs);
                let typ = if result.ok { Some(lhs.clone()) } else { None };
                results.push(result);
                if let Some(t) = typ { tree.set_typ(t); }
            }
        }

        "RelExpr" | "EqExpr" => {
            // Result is always bool if operands are compatible
            if let (Some(lhs), Some(rhs)) = (
                tree.kids.get(0).and_then(|k| k.typ.clone()),
                tree.kids.get(2).and_then(|k| k.typ.clone()),
            ) {
                let result = check_types(tree, &lhs, &rhs);
                let ok = result.ok;
                results.push(result);
                if ok { tree.set_typ(TypeInfo::boolean()); }
            }
        }

        "CondAndExpr" | "CondOrExpr" => {
            if let (Some(lhs), Some(rhs)) = (
                tree.kids.get(0).and_then(|k| k.typ.clone()),
                tree.kids.get(2).and_then(|k| k.typ.clone()),
            ) {
                let result = check_types(tree, &lhs, &rhs);
                let ok = result.ok;
                results.push(result);
                if ok { tree.set_typ(TypeInfo::boolean()); }
            }
        }

        "UnaryMinus" => {
            if let Some(operand) = tree.kids.first().and_then(|k| k.typ.clone()) {
                if operand.is_numeric() {
                    tree.set_typ(operand);
                }
            }
        }

        "UnaryNot" => {
            if let Some(operand) = tree.kids.first().and_then(|k| k.typ.clone()) {
                if operand.is_boolean() {
                    tree.set_typ(TypeInfo::boolean());
                }
            }
        }

        "FieldAccess" => {
            // kids[0] = object expr (already typed by check_kids)
            // kids[1] = field name IDENTIFIER
            if let Some(obj_typ) = tree.kids.first().and_then(|k| k.typ.clone()) {
                if let TypeInfo::Class(ref ct) = obj_typ {
                    if let Some(ref st) = ct.st {
                        let field_name = tree.kids.get(1)
                            .and_then(|k| k.tok.as_ref())
                            .map(|t| t.text.clone());
                        if let Some(name) = field_name {
                            let typ = st.borrow().lookup(&name).and_then(|e| e.typ.clone());
                            if let Some(t) = typ {
                                tree.set_typ(t);
                            }
                        }
                    }
                }
            }
        }

        "Block" | "BlockStmts" | "EmptyStmt" | "BreakStmt" | "ReturnStmt" => {
            tree.set_typ(TypeInfo::void());
        }

        "MethodCall" => {
            // Type checking of method calls deferred to Chapter 8
        }

        // Token leaf — type set by assign_leaf_types (literals) or
        // looked up in stab (IDENTIFIER)
        _ if tree.tok.is_some() => {
            let tok = tree.tok.as_ref().unwrap().clone();
            if tok.category == "IDENTIFIER" {
                // Look up in symbol table chain
                if let Some(typ) = lookup_in_stab(tree) {
                    tree.set_typ(typ);
                }
            }
            // Other token types already handled by assign_leaf_types
        }

        // Nodes that simply propagate their first child's type
        "StmtExprList" => {
            if let Some(t) = tree.kids.first().and_then(|k| k.typ.clone()) {
                tree.set_typ(t);
            }
        }

        _ => {
            // Silently skip unknown nodes — avoids noisy errors for
            // structural nodes (MethodHeader, MethodDeclarator, etc.)
            // that don't carry a value type.
        }
    }
}

// ─── check_kids ──────────────────────────────────────────────────────────────

/// Selectively traverse children, controlling which are visited and how.
///
/// Returns `true`  → parent (`check_type`) should stop after this call.
/// Returns `false` → parent should continue with its own switch.
fn check_kids(
    tree: &mut Tree,
    in_codeblock: bool,
    results: &mut Vec<TypeCheckResult>,
) -> bool {
    match tree.sym.as_str() {
        // Enter method body with in_codeblock=true; skip header
        "MethodDecl" => {
            if let Some(block) = tree.kids.get_mut(1) {
                check_type(block, true, results);
            }
            true
        }

        // VarDeclarator initialiser is not a codeblock expression
        "LocalVarDecl" => {
            if let Some(declarator) = tree.kids.get_mut(1) {
                check_type(declarator, false, results);
            }
            true
        }

        // Only check the object expression — field name is resolved, not checked
        "FieldAccess" => {
            if let Some(obj) = tree.kids.get_mut(0) {
                check_type(obj, in_codeblock, results);
            }
            // Return false so check_type continues to resolve the field type
            false
        }

        // Walk lhs only, then let check_type resolve the field
        "QualifiedName" => {
            if let Some(lhs) = tree.kids.get_mut(0) {
                check_type(lhs, in_codeblock, results);
            }
            false
        }

        // Default: walk all children uniformly
        _ => {
            // Collect kids indices to avoid borrow issues
            let n = tree.kids.len();
            for i in 0..n {
                let kid = &mut tree.kids[i];
                check_type(kid, in_codeblock, results);
            }
            false
        }
    }
}

// ─── check_types ─────────────────────────────────────────────────────────────

/// Check that `op1` and `op2` are compatible under the node's operator.
///
/// For Chapter 7, checks `=`, `+`, `-`, `*`, `/`, `%` on matching basetypes.
/// Relational and logical operators check for compatible types too.
/// Returns a `TypeCheckResult` recording the outcome.
fn check_types(tree: &Tree, op1: &TypeInfo, op2: &TypeInfo) -> TypeCheckResult {
    let operator = get_op(tree).unwrap_or("?").to_string();
    let lineno = find_token(tree).and_then(|t| t.tok.as_ref()).map(|t| t.lineno).unwrap_or(0);

    let ok = match operator.as_str() {
        // Assignment and arithmetic: operands must have the same basetype
        "=" | "+=" | "-=" =>
            op1.same_base(op2),
        "+" | "-" | "*" | "/" | "%" =>
            op1.same_base(op2) && op1.is_numeric(),
        // Relational: both operands must be the same type
        "<" | ">" | "<=" | ">=" =>
            op1.same_base(op2) && op1.is_numeric(),
        // Equality: same basetype
        "==" | "!=" =>
            op1.same_base(op2),
        // Logical: both must be boolean
        "&&" | "||" =>
            op1.is_boolean() && op2.is_boolean(),
        _ => false,
    };

    TypeCheckResult::new(lineno, &operator, op1, op2, ok)
}

// ─── Helper: get_op ──────────────────────────────────────────────────────────

/// Find the operator token text within a binary expression node.
/// For binary nodes the operator is always kids[1].
fn get_op(tree: &Tree) -> Option<&str> {
    tree.kids.get(1)?.tok.as_ref().map(|t| t.text.as_str())
}

// ─── Helper: find_token ──────────────────────────────────────────────────────

/// Pre-order search for the first token leaf in a subtree.
/// Used to extract a line number for error/diagnostic messages.
pub fn find_token(tree: &Tree) -> Option<&Tree> {
    if tree.tok.is_some() {
        return Some(tree);
    }
    for kid in &tree.kids {
        if let Some(t) = find_token(kid) {
            return Some(t);
        }
    }
    None
}

// ─── Helper: lookup_in_stab ──────────────────────────────────────────────────

/// Walk the stab parent chain to find a symbol's declared type.
fn lookup_in_stab(tree: &Tree) -> Option<TypeInfo> {
    let stab: Rc<RefCell<SymTab>> = tree.stab.clone()?;
    let name = tree.tok.as_ref().map(|t| t.text.clone())?;
    stab.borrow().lookup(&name).and_then(|e| e.typ.clone())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jzero_parser::parse_tree;
    use crate::analyze;

    fn run(src: &str) -> (crate::SemanticResult, Vec<TypeCheckResult>) {
        let mut tree = parse_tree(src).expect("parse failed");
        let result = analyze(&mut tree);
        let mut type_results = Vec::new();
        check_type(&mut tree, false, &mut type_results);
        (result, type_results)
    }

    // ─── Book's example: hello.java with type error ───────────────────────

    #[test]
    fn test_book_hello_typecheck() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        int x;
        x = 0;
        x = x + "hello";
        System.out.println("hello, jzero!");
    }
}
"#;
        let (result, type_results) = run(src);
        assert!(result.errors.is_empty(), "unexpected semantic errors: {:?}", result.errors);

        println!("\n=== Type Check Results ===");
        for r in &type_results {
            println!("{}", r);
        }

        // x = 0  → int = int → OK
        let ok = type_results.iter().find(|r| r.operator == "=" && r.ok);
        assert!(ok.is_some(), "expected OK assignment not found");
        assert_eq!(ok.unwrap().op1, "int");
        assert_eq!(ok.unwrap().op2, "int");

        // x + "hello" → int + String → FAIL
        let fail = type_results.iter().find(|r| r.operator == "+" && !r.ok);
        assert!(fail.is_some(), "expected FAIL addition not found");
        assert_eq!(fail.unwrap().op1, "int");
        assert_eq!(fail.unwrap().op2, "String");
    }

    #[test]
    fn test_book_output_format() {
        // Verify the exact string format matches the book's output
        let r_ok = TypeCheckResult::new(4, "=", &TypeInfo::int(), &TypeInfo::int(), true);
        assert_eq!(r_ok.to_string(), "line 4: typecheck = on a int and a int -> OK");

        let r_fail = TypeCheckResult::new(5, "+", &TypeInfo::int(), &TypeInfo::string(), false);
        assert_eq!(r_fail.to_string(), "line 5: typecheck + on a String and a int -> FAIL");
    }

    // ─── Individual expression checks ────────────────────────────────────

    #[test]
    fn test_valid_int_arithmetic() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        int y;
        x = y + 1;
    }
}
"#;
        let (result, type_results) = run(src);
        assert!(result.errors.is_empty());

        let add = type_results.iter().find(|r| r.operator == "+");
        assert!(add.is_some(), "expected AddExpr result");
        assert!(add.unwrap().ok, "int + int should be OK");
    }

    #[test]
    fn test_type_mismatch_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        x = "hello";
    }
}
"#;
        let (_result, type_results) = run(src);
        let assign = type_results.iter().find(|r| r.operator == "=");
        assert!(assign.is_some());
        assert!(!assign.unwrap().ok, "int = String should FAIL");
    }

    #[test]
    fn test_find_token_returns_first_leaf() {
        let lhs = Tree::leaf("IDENTIFIER", "x", 5);
        let op  = Tree::leaf("ASSIGN", "=", 5);
        let rhs = Tree::leaf("INTLIT", "0", 5);
        let assign = Tree::new("Assignment", 0, vec![lhs, op, rhs]);
        let tok = find_token(&assign);
        assert!(tok.is_some());
        assert_eq!(tok.unwrap().tok.as_ref().unwrap().text, "x");
    }

    #[test]
    fn test_get_op_finds_operator() {
        let lhs = Tree::leaf("IDENTIFIER", "x", 1);
        let op  = Tree::leaf("PLUS", "+", 1);
        let rhs = Tree::leaf("INTLIT", "1", 1);
        let add = Tree::new("AddExpr", 0, vec![lhs, op, rhs]);
        assert_eq!(get_op(&add), Some("+"));
    }

    #[test]
    fn test_typecheck_result_display_ok() {
        let r = TypeCheckResult::new(4, "=", &TypeInfo::int(), &TypeInfo::int(), true);
        assert_eq!(r.to_string(), "line 4: typecheck = on a int and a int -> OK");
    }

    #[test]
    fn test_typecheck_result_display_fail() {
        let r = TypeCheckResult::new(5, "+", &TypeInfo::int(), &TypeInfo::string(), false);
        assert_eq!(r.to_string(), "line 5: typecheck + on a String and a int -> FAIL");
    }
}