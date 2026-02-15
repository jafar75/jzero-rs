pub mod lexer;

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
    match jzero::ClassDeclParser::new().parse(lexer) {
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
        if !result.success {
            println!("one_dot errors:");
            for e in &result.errors {
                println!("  {}", e);
            }
        }
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
        if !result.success {
            println!("two_dot errors:");
            for e in &result.errors {
                println!("  {}", e);
            }
        }
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
        // Assignment as expression: foo(x = 5)
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
        // Chained assignment: x = y = 5
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
        // Assignment to dotted field: obj.field = 42
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
}