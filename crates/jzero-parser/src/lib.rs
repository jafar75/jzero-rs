pub mod action;
pub mod lexer;
pub mod loc;

// LALRPOP generates the parser module from jzero.lalrpop at build time
lalrpop_util::lalrpop_mod!(
    #[allow(
        clippy::all,
        unused_parens,
        unused_imports,
        dead_code,
        unused_variables
    )]
    jzero
);

use jzero_ast::tree::Tree;
use lexer::{Lexer, LexicalError, Tok};
use lalrpop_util::ParseError;

/// Result of parsing: success flag plus any error messages.
#[derive(Debug)]
pub struct ParseResult {
    pub success: bool,
    pub errors: Vec<String>,
}

/// Parse the given source code and return whether it is syntactically valid.
///
/// This corresponds to Chapter 4 of the book: accept/reject with error recovery.
pub fn parse(input: &str) -> ParseResult {
    let lexer = Lexer::new(input);
    match jzero::ClassDeclParser::new().parse(input, lexer) {
        Ok(_) => ParseResult {
            success: true,
            errors: vec![],
        },
        Err(e) => {
            let msg = format_error(input, e);
            ParseResult {
                success: false,
                errors: vec![msg],
            }
        }
    }
}

/// Parse the given source code and return the syntax tree.
///
/// This corresponds to Chapter 5 of the book: building syntax trees.
pub fn parse_tree(input: &str) -> Result<Tree, String> {
    let lexer = Lexer::new(input);
    jzero::ClassDeclParser::new()
        .parse(input, lexer)
        .map_err(|e| format_error(input, e))
}

/// Format a LALRPOP ParseError into a human-readable string.
fn format_error(
    input: &str,
    err: ParseError<usize, Tok<'_>, LexicalError>,
) -> String {
    match err {
        ParseError::InvalidToken { location } => {
            let (line, col) = offset_to_line_col(input, location);
            format!("Invalid token at line {} column {}", line, col)
        }
        ParseError::UnrecognizedEof { location, expected } => {
            let (line, col) = offset_to_line_col(input, location);
            format!(
                "Unexpected end of file at line {} column {}. Expected one of: {}",
                line, col, expected.join(", ")
            )
        }
        ParseError::UnrecognizedToken { token: (start, tok, _end), expected } => {
            let (line, col) = offset_to_line_col(input, start);
            format!(
                "Unexpected token '{}' at line {} column {}. Expected one of: {}",
                tok, line, col, expected.join(", ")
            )
        }
        ParseError::ExtraToken { token: (start, tok, _end) } => {
            let (line, col) = offset_to_line_col(input, start);
            format!("Extra token '{}' at line {} column {}", tok, line, col)
        }
        ParseError::User { error } => {
            format!("{}", error)
        }
    }
}

/// Convert a byte offset into (1-based line, 1-based column).
fn offset_to_line_col(input: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in input.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Chapter 4 tests (accept/reject) ─────────────────

    #[test]
    fn test_empty_class() {
        let src = "public class T { }";
        let result = parse(src);
        assert!(result.success, "empty class failed: {:?}", result.errors);
    }

    #[test]
    fn test_method_decl() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "method decl failed: {:?}", result.errors);
    }

    #[test]
    fn test_simple_method_call() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        foo("hello");
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "simple call failed: {:?}", result.errors);
    }

    #[test]
    fn test_one_dot_method_call() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        System.println("hello");
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "one dot call failed: {:?}", result.errors);
    }

    #[test]
    fn test_two_dot_method_call() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        System.out.println("hello");
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "two dot call failed: {:?}", result.errors);
    }

    #[test]
    fn test_variable_decl() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "var decl failed: {:?}", result.errors);
    }

    #[test]
    fn test_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        x = 42;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "assignment failed: {:?}", result.errors);
    }

    #[test]
    fn test_if_else() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        if (x == 1) {
            y = 2;
        } else {
            y = 3;
        }
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "if-else failed: {:?}", result.errors);
    }

    #[test]
    fn test_for_loop() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        for (int i; i < 10; i += 1) {
            x = x + i;
        }
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "for loop failed: {:?}", result.errors);
    }

    #[test]
    fn test_arithmetic_expr() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        x = a + b * c - d / e;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "arithmetic failed: {:?}", result.errors);
    }

    #[test]
    fn test_field_access() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        x = obj.field;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "field access failed: {:?}", result.errors);
    }

    #[test]
    fn test_assignment_in_expr() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        foo(x = 5);
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "assignment in expr failed: {:?}", result.errors);
    }

    #[test]
    fn test_nested_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        x = y = 5;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "nested assignment failed: {:?}", result.errors);
    }

    #[test]
    fn test_field_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        obj.field = 42;
    }
}
"#;
        let result = parse(src);
        assert!(result.success, "field assignment failed: {:?}", result.errors);
    }

    // ─── Chapter 5 tests (tree structure) ────────────────

    /// Helper: given a typical single-method class, return the Block node
    /// of the first (and only) method.
    fn get_method_block(tree: &Tree) -> &Tree {
        let method = &tree.kids[1]; // MethodDecl
        assert_eq!(method.sym, "MethodDecl");
        let block = &method.kids[1]; // Block
        assert_eq!(block.sym, "Block");
        block
    }

    #[test]
    fn test_tree_empty_class() {
        let src = "public class T { }";
        let tree = parse_tree(src).expect("parse failed");
        assert_eq!(tree.sym, "ClassDecl");
        assert_eq!(tree.nkids, 1); // just the class name, no body decls
        assert_eq!(tree.kids[0].tok.as_ref().unwrap().text, "T");
    }

    #[test]
    fn test_tree_hello_world() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        assert_eq!(tree.sym, "ClassDecl");
        assert_eq!(tree.nkids, 2); // name + MethodDecl
        assert_eq!(tree.kids[0].tok.as_ref().unwrap().text, "hello");
        assert_eq!(tree.kids[1].sym, "MethodDecl");

        eprintln!("\n=== Tree (text) ===\n{}", tree);
        eprintln!("=== DOT ===\n{}", tree.to_dot());
    }

    #[test]
    fn test_tree_variable_decl() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let var_decl = &block.kids[0];
        assert_eq!(var_decl.sym, "LocalVarDecl");
        assert_eq!(var_decl.kids[0].tok.as_ref().unwrap().text, "int");
        assert_eq!(var_decl.kids[1].sym, "VarDeclarator");
    }

    #[test]
    fn test_tree_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        x = 42;
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let assign = &block.kids[0];
        assert_eq!(assign.sym, "Assignment");
        assert_eq!(assign.nkids, 3);
        assert_eq!(assign.kids[0].tok.as_ref().unwrap().text, "x");
        assert_eq!(assign.kids[1].tok.as_ref().unwrap().text, "=");
        assert_eq!(assign.kids[2].tok.as_ref().unwrap().text, "42");
    }

    #[test]
    fn test_tree_arithmetic() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        x = a + b * c;
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let assign = &block.kids[0];
        assert_eq!(assign.sym, "Assignment");
        // rhs is AddExpr(a, +, MulExpr(b, *, c))
        let add = &assign.kids[2];
        assert_eq!(add.sym, "AddExpr");
        assert_eq!(add.kids[0].tok.as_ref().unwrap().text, "a");
        assert_eq!(add.kids[1].tok.as_ref().unwrap().text, "+");
        let mul = &add.kids[2];
        assert_eq!(mul.sym, "MulExpr");
        assert_eq!(mul.kids[0].tok.as_ref().unwrap().text, "b");
        assert_eq!(mul.kids[2].tok.as_ref().unwrap().text, "c");
    }

    #[test]
    fn test_tree_two_dot_method_call() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        System.out.println("hello");
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let call = &block.kids[0];
        assert_eq!(call.sym, "MethodCall");
        // First child is FieldAccess(FieldAccess(System, out), println)
        let outer_access = &call.kids[0];
        assert_eq!(outer_access.sym, "FieldAccess");
        let inner_access = &outer_access.kids[0];
        assert_eq!(inner_access.sym, "FieldAccess");
        assert_eq!(inner_access.kids[0].tok.as_ref().unwrap().text, "System");
        assert_eq!(inner_access.kids[1].tok.as_ref().unwrap().text, "out");
        assert_eq!(outer_access.kids[1].tok.as_ref().unwrap().text, "println");
        // Second child is the string argument
        assert_eq!(call.kids[1].tok.as_ref().unwrap().text, "\"hello\"");
    }

    #[test]
    fn test_tree_if_else() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        if (x == 1) {
            y = 2;
        } else {
            y = 3;
        }
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let if_stmt = &block.kids[0];
        assert_eq!(if_stmt.sym, "IfThenElseStmt");
        assert_eq!(if_stmt.nkids, 3); // condition, then-block, else-block
        assert_eq!(if_stmt.kids[0].sym, "EqExpr");
        assert_eq!(if_stmt.kids[1].sym, "Block");
        assert_eq!(if_stmt.kids[2].sym, "Block");
    }

    #[test]
    fn test_tree_for_loop() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        for (int i; i < 10; i += 1) {
            x = x + i;
        }
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let for_stmt = &block.kids[0];
        assert_eq!(for_stmt.sym, "ForStmt");
        assert_eq!(for_stmt.nkids, 4); // init, cond, update, body
        assert_eq!(for_stmt.kids[0].sym, "LocalVarDecl");
        assert_eq!(for_stmt.kids[1].sym, "RelExpr");
        assert_eq!(for_stmt.kids[2].sym, "Assignment");
        assert_eq!(for_stmt.kids[3].sym, "Block");
    }

    #[test]
    fn test_tree_field_assignment() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        obj.field = 42;
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let block = get_method_block(&tree);
        let assign = &block.kids[0];
        assert_eq!(assign.sym, "Assignment");
        assert_eq!(assign.kids[0].sym, "FieldAccess");
        assert_eq!(assign.kids[0].kids[0].tok.as_ref().unwrap().text, "obj");
        assert_eq!(assign.kids[0].kids[1].tok.as_ref().unwrap().text, "field");
    }

    #[test]
    fn test_tree_dot_output_file() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
"#;
        let tree = parse_tree(src).expect("parse failed");
        let dot = tree.to_dot();

        // Write to file for visual inspection with: dot -Tpng hello.dot -o hello.png
        let out_dir = std::env::var("CARGO_TARGET_TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
        let dot_path = format!("{}/hello.java.dot", out_dir);
        std::fs::write(&dot_path, &dot).expect("failed to write dot file");
        eprintln!("\nDOT file written to: {}", dot_path);
        eprintln!("Render with: dot -Tpng {} -o hello.png", dot_path);

        // Basic sanity checks on DOT content
        assert!(dot.contains("digraph {"));
        assert!(dot.contains("ClassDecl#0"));
        assert!(dot.contains("MethodDecl#0"));
        assert!(dot.contains("MethodCall#0"));
        assert!(dot.contains("FieldAccess#0"));
        assert!(dot.contains("hello, jzero!"));
    }
}