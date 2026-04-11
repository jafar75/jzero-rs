//! Phase 5 — Expression type checking (Chapter 7 + Chapter 8).

use std::cell::RefCell;
use std::rc::Rc;

use jzero_ast::tree::Tree;
use jzero_symtab::{SymTab, TypeInfo};

// ─── TypeCheckResult ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypeCheckResult {
    pub lineno: usize,
    pub operator: String,
    pub op2: String,
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
                if operand.is_numeric() { tree.set_typ(operand); }
            }
        }

        "UnaryNot" => {
            if let Some(operand) = tree.kids.first().and_then(|k| k.typ.clone()) {
                if operand.is_boolean() { tree.set_typ(TypeInfo::boolean()); }
            }
        }

        // ── ArrayCreation: new int[n] → Array(int) ────────────────────────
        "ArrayCreation" => {
            let elem_typ = tree.kids.first().and_then(|k| {
                k.typ.clone().or_else(|| {
                    k.tok.as_ref().and_then(|t| match t.category.as_str() {
                        "INT"        => Some(TypeInfo::int()),
                        "DOUBLE"     => Some(TypeInfo::double()),
                        "BOOL"       => Some(TypeInfo::boolean()),
                        "STRING"     => Some(TypeInfo::string()),
                        "IDENTIFIER" => Some(TypeInfo::class(&t.text)),
                        _ => None,
                    })
                })
            });
            if let Some(et) = elem_typ {
                tree.set_typ(TypeInfo::array(et));
            }
        }

        // ── ArrayAccess: arr[i] → element type ───────────────────────────
        "ArrayAccess" => {
            let base_typ = tree.kids.get(0).and_then(|k| k.typ.clone());
            let idx_typ  = tree.kids.get(1).and_then(|k| k.typ.clone());
            match (base_typ, idx_typ) {
                (Some(TypeInfo::Array(elem)), Some(idx)) => {
                    if idx.basetype() == "int" {
                        tree.set_typ(*elem);
                    } else {
                        let lineno = find_token(tree)
                            .and_then(|t| t.tok.as_ref())
                            .map(|t| t.lineno)
                            .unwrap_or(0);
                        results.push(TypeCheckResult {
                            lineno,
                            operator: "subscript".to_string(),
                            op1: idx.str(),
                            op2: "int".to_string(),
                            ok: false,
                        });
                    }
                }
                (Some(base), _) if base.basetype() != "array" => {
                    let lineno = find_token(tree)
                        .and_then(|t| t.tok.as_ref())
                        .map(|t| t.lineno)
                        .unwrap_or(0);
                    results.push(TypeCheckResult {
                        lineno,
                        operator: "subscript".to_string(),
                        op1: base.str(),
                        op2: "array".to_string(),
                        ok: false,
                    });
                }
                _ => {}
            }
        }

        // ── MethodCall ────────────────────────────────────────────────────
        "MethodCall" => {
            match tree.rule {
                0 => {
                    let name = tree.kids.first()
                        .and_then(|k| k.tok.as_ref())
                        .map(|t| t.text.clone());
                    if let Some(name) = name {
                        let entry = lookup_in_stab_by_name(tree, &name);
                        match entry {
                            Some(TypeInfo::Method(mt)) => {
                                let args: Vec<TypeInfo> = tree.kids[1..]
                                    .iter()
                                    .filter_map(|k| k.typ.clone())
                                    .collect();
                                let return_typ = *mt.return_type.clone();
                                cksig(tree, &mt.parameters, &args, results);
                                tree.set_typ(return_typ);
                            }
                            Some(other) => {
                                let lineno = find_token(tree)
                                    .and_then(|t| t.tok.as_ref())
                                    .map(|t| t.lineno)
                                    .unwrap_or(0);
                                results.push(TypeCheckResult {
                                    lineno,
                                    operator: "param".to_string(),
                                    op1: other.str(),
                                    op2: "method".to_string(),
                                    ok: false,
                                });
                            }
                            None => {}
                        }
                    }
                }
                2 => {
                    if let Some(method_typ) = dequalify(tree) {
                        if let TypeInfo::Method(mt) = method_typ {
                            let args: Vec<TypeInfo> = tree.kids[2..]
                                .iter()
                                .filter_map(|k| k.typ.clone())
                                .collect();
                            let return_typ = *mt.return_type.clone();
                            cksig(tree, &mt.parameters, &args, results);
                            tree.set_typ(return_typ);
                        }
                    }
                }
                _ => {}
            }
        }

        // ── ReturnStmt ────────────────────────────────────────────────────
        "ReturnStmt" => {
            let return_typ = lookup_in_stab_by_name(tree, "return");
            match (return_typ, tree.rule) {
                (Some(rt), 0) => {
                    if let Some(expr_typ) = tree.kids.first().and_then(|k| k.typ.clone()) {
                        let lineno = find_token(tree)
                            .and_then(|t| t.tok.as_ref())
                            .map(|t| t.lineno)
                            .unwrap_or(0);
                        let ok = rt.same_base(&expr_typ);
                        results.push(TypeCheckResult {
                            lineno,
                            operator: "return".to_string(),
                            op1: rt.str(),
                            op2: expr_typ.str(),
                            ok,
                        });
                        if ok { tree.set_typ(rt); }
                    }
                }
                (Some(rt), 1) => {
                    if rt.basetype() == "void" {
                        tree.set_typ(TypeInfo::void());
                    }
                }
                _ => {}
            }
        }

        // ── InstanceCreation: new Foo(args) ───────────────────────────────
        "InstanceCreation" => {
            let name = tree.kids.first()
                .and_then(|k| k.tok.as_ref())
                .map(|t| t.text.clone());
            if let Some(name) = name {
                if let Some(typ) = lookup_in_stab_by_name(tree, &name) {
                    tree.set_typ(typ);
                }
            }
        }

        "FieldAccess" => {
            if let Some(obj_typ) = tree.kids.first().and_then(|k| k.typ.clone()) {
                if let TypeInfo::Class(ref ct) = obj_typ {
                    if let Some(ref st) = ct.st {
                        let field_name = tree.kids.get(1)
                            .and_then(|k| k.tok.as_ref())
                            .map(|t| t.text.clone());
                        if let Some(name) = field_name {
                            let typ = st.borrow().lookup(&name).and_then(|e| e.typ.clone());
                            if let Some(t) = typ { tree.set_typ(t); }
                        }
                    }
                }
            }
        }

        "Block" | "BlockStmts" | "EmptyStmt" | "BreakStmt" => {
            tree.set_typ(TypeInfo::void());
        }

        _ if tree.tok.is_some() => {
            let tok = tree.tok.as_ref().unwrap().clone();
            if tok.category == "IDENTIFIER" {
                if let Some(typ) = lookup_in_stab(tree) {
                    tree.set_typ(typ);
                }
            }
        }

        "StmtExprList" => {
            if let Some(t) = tree.kids.first().and_then(|k| k.typ.clone()) {
                tree.set_typ(t);
            }
        }

        _ => {}
    }
}

// ─── check_kids ──────────────────────────────────────────────────────────────

fn check_kids(
    tree: &mut Tree,
    in_codeblock: bool,
    results: &mut Vec<TypeCheckResult>,
) -> bool {
    match tree.sym.as_str() {
        "MethodDecl" => {
            if let Some(block) = tree.kids.get_mut(1) {
                check_type(block, true, results);
            }
            true
        }

        "LocalVarDecl" => {
            if let Some(declarator) = tree.kids.get_mut(1) {
                check_type(declarator, false, results);
            }
            true
        }

        "FieldAccess" => {
            if let Some(obj) = tree.kids.get_mut(0) {
                check_type(obj, in_codeblock, results);
            }
            false
        }

        "ReturnStmt" => {
            let n = tree.kids.len();
            for i in 0..n {
                check_type(&mut tree.kids[i], in_codeblock, results);
            }
            false
        }

        "MethodCall" => {
            let n = tree.kids.len();
            for i in 0..n {
                check_type(&mut tree.kids[i], in_codeblock, results);
            }
            false
        }

        "ArrayCreation" => {
            let n = tree.kids.len();
            for i in 0..n {
                check_type(&mut tree.kids[i], in_codeblock, results);
            }
            false
        }

        "ArrayAccess" => {
            let n = tree.kids.len();
            for i in 0..n {
                check_type(&mut tree.kids[i], in_codeblock, results);
            }
            false
        }

        "QualifiedName" => {
            if let Some(lhs) = tree.kids.get_mut(0) {
                check_type(lhs, in_codeblock, results);
            }
            false
        }

        _ => {
            let n = tree.kids.len();
            for i in 0..n {
                check_type(&mut tree.kids[i], in_codeblock, results);
            }
            false
        }
    }
}

// ─── cksig ───────────────────────────────────────────────────────────────────

fn cksig(
    tree: &Tree,
    params: &[jzero_symtab::Parameter],
    args: &[TypeInfo],
    results: &mut Vec<TypeCheckResult>,
) {
    let lineno = find_token(tree)
        .and_then(|t| t.tok.as_ref())
        .map(|t| t.lineno)
        .unwrap_or(0);

    if args.len() != params.len() {
        results.push(TypeCheckResult {
            lineno,
            operator: "param".to_string(),
            op1: format!("{} params", params.len()),
            op2: format!("{} args", args.len()),
            ok: false,
        });
        return;
    }

    for (param, arg) in params.iter().zip(args.iter()) {
        let ok = param.param_type.same_base(arg);
        results.push(TypeCheckResult {
            lineno,
            operator: "param".to_string(),
            op1: param.param_type.str(),
            op2: arg.str(),
            ok,
        });
    }
}

// ─── dequalify ───────────────────────────────────────────────────────────────

fn dequalify(tree: &Tree) -> Option<TypeInfo> {
    let base_typ = tree.kids.first().and_then(|k| k.typ.clone())?;
    match base_typ {
        TypeInfo::Class(ref ct) => {
            let st = ct.st.as_ref()?;
            let method_name = tree.kids.get(1)
                .and_then(|k| k.tok.as_ref())
                .map(|t| t.text.clone())?;
            st.borrow().lookup(&method_name).and_then(|e| e.typ.clone())
        }
        _ => None,
    }
}

// ─── check_types ─────────────────────────────────────────────────────────────

fn check_types(tree: &Tree, op1: &TypeInfo, op2: &TypeInfo) -> TypeCheckResult {
    let operator = get_op(tree).unwrap_or("?").to_string();
    let lineno = find_token(tree).and_then(|t| t.tok.as_ref()).map(|t| t.lineno).unwrap_or(0);

    let ok = match operator.as_str() {
        "=" | "+=" | "-=" => {
            if op1.basetype() == "array" && op2.basetype() == "array" {
                if let (TypeInfo::Array(e1), TypeInfo::Array(e2)) = (op1, op2) {
                    e1.same_base(e2)
                } else { false }
            } else {
                op1.same_base(op2)
            }
        }
        "+" | "-" | "*" | "/" | "%" => {
            if op1.same_base(op2) {
                // String supports + (concatenation) but not -, *, /, %
                if op1.basetype() == "String" {
                    operator == "+"
                } else {
                    op1.is_numeric()
                }
            } else {
                false
            }
        }
        "<" | ">" | "<=" | ">=" =>
            op1.same_base(op2) && op1.is_numeric(),
        "==" | "!=" =>
            op1.same_base(op2),
        "&&" | "||" =>
            op1.is_boolean() && op2.is_boolean(),
        "param" | "return" =>
            op1.same_base(op2),
        _ => false,
    };

    TypeCheckResult::new(lineno, &operator, op1, op2, ok)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_op(tree: &Tree) -> Option<&str> {
    tree.kids.get(1)?.tok.as_ref().map(|t| t.text.as_str())
}

pub fn find_token(tree: &Tree) -> Option<&Tree> {
    if tree.tok.is_some() { return Some(tree); }
    for kid in &tree.kids {
        if let Some(t) = find_token(kid) { return Some(t); }
    }
    None
}

fn lookup_in_stab(tree: &Tree) -> Option<TypeInfo> {
    let stab: Rc<RefCell<SymTab>> = tree.stab.clone()?;
    let name = tree.tok.as_ref().map(|t| t.text.clone())?;
    stab.borrow().lookup(&name).and_then(|e| e.typ.clone())
}

fn lookup_in_stab_by_name(tree: &Tree, name: &str) -> Option<TypeInfo> {
    let stab: Rc<RefCell<SymTab>> = tree.stab.clone()?;
    stab.borrow().lookup(name).and_then(|e| e.typ.clone())
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
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
        let ok = type_results.iter().find(|r| r.operator == "=" && r.ok);
        assert!(ok.is_some());
        assert_eq!(ok.unwrap().op1, "int");
        let fail = type_results.iter().find(|r| r.operator == "+" && !r.ok);
        assert!(fail.is_some());
    }

    #[test]
    fn test_book_output_format() {
        let r_ok = TypeCheckResult::new(4, "=", &TypeInfo::int(), &TypeInfo::int(), true);
        assert_eq!(r_ok.to_string(), "line 4: typecheck = on a int and a int -> OK");
        let r_fail = TypeCheckResult::new(5, "+", &TypeInfo::int(), &TypeInfo::string(), false);
        assert_eq!(r_fail.to_string(), "line 5: typecheck + on a String and a int -> FAIL");
    }

    #[test]
    fn test_funtest_method_call_and_return() {
        let src = r#"
public class funtest {
    public static int foo(int x, int y, String z) {
        return 0;
    }
    public static void main(String argv[]) {
        int x;
        x = foo(0, 1, "howdy");
        x = x + 1;
        System.out.println("hello, jzero!");
    }
}
"#;
        let (result, type_results) = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        println!("\n=== funtest type checks ===");
        for r in &type_results { println!("{}", r); }

        let ret = type_results.iter().find(|r| r.operator == "return");
        assert!(ret.is_some(), "expected return typecheck");
        assert!(ret.unwrap().ok, "return int should be OK");

        let params: Vec<_> = type_results.iter().filter(|r| r.operator == "param").collect();
        assert_eq!(params.len(), 3, "expected 3 param checks for foo");
        assert!(params.iter().all(|r| r.ok), "all params should be OK");
    }

    #[test]
    fn test_array_creation_and_access() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x[];
        x = new int[3];
        int y;
        y = x[0];
    }
}
"#;
        let (result, type_results) = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        println!("\n=== array type checks ===");
        for r in &type_results { println!("{}", r); }

        let arr_assign = type_results.iter().find(|r| r.operator == "=" && r.op1 == "array");
        assert!(arr_assign.is_some(), "expected array assignment typecheck");
        assert!(arr_assign.unwrap().ok);

        let elem_assign = type_results.iter()
            .filter(|r| r.operator == "=" && r.op1 == "int")
            .last();
        assert!(elem_assign.is_some(), "expected element assignment typecheck");
        assert!(elem_assign.unwrap().ok);
    }

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
        assert!(add.is_some());
        assert!(add.unwrap().ok);
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
        assert!(!assign.unwrap().ok);
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

    #[test]
    fn test_string_concatenation_typechecks_ok() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        String s;
        s = "hello, " + "jzero!";
    }
}
"#;
        let (_result, type_results) = run(src);
        let add = type_results.iter().find(|r| r.operator == "+");
        assert!(add.is_some(), "expected + typecheck");
        assert!(add.unwrap().ok, "String + String should be OK");
        assert_eq!(add.unwrap().op1, "String");
    }

    #[test]
    fn test_string_subtraction_typechecks_fail() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        String s;
        s = "hello" - "world";
    }
}
"#;
        let (_result, type_results) = run(src);
        let sub = type_results.iter().find(|r| r.operator == "-");
        assert!(sub.is_some(), "expected - typecheck");
        assert!(!sub.unwrap().ok, "String - String should FAIL");
    }
}