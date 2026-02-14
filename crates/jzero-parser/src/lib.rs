use cfgrammar::NewlineCache;
use lrlex::{lrlex_mod, DefaultLexeme, DefaultLexerTypes, LRLexError, LRNonStreamingLexer};
use lrpar::{lrpar_mod, Lexeme, NonStreamingLexer};
use logos::Logos;

use jzero_lexer::token::Token;

// Import the generated token map (T_PUBLIC, T_CLASS, T_IDENTIFIER, etc.)
lrlex_mod!("token_map");
use token_map::*;

// Import the generated parser
lrpar_mod!("jzero.y");

/// Map a Logos `Token` variant to its lrpar token ID.
fn token_id(tok: &Token) -> u8 {
    match tok {
        // Keywords
        Token::Bool => T_BOOL,
        Token::Break => T_BREAK,
        Token::Class => T_CLASS,
        Token::Double => T_DOUBLE,
        Token::Else => T_ELSE,
        Token::For => T_FOR,
        Token::If => T_IF,
        Token::Int => T_INT,
        Token::Null => T_NULLVAL,
        Token::Public => T_PUBLIC,
        Token::Return => T_RETURN,
        Token::Static => T_STATIC,
        Token::StringKw => T_STRING,
        Token::Void => T_VOID,
        Token::While => T_WHILE,

        // Boolean literals
        Token::True | Token::False => T_BOOLLIT,

        // Literals
        Token::IntLit => T_INTLIT,
        Token::DoubleLit => T_DOUBLELIT,
        Token::StringLit => T_STRINGLIT,

        // Identifiers
        Token::Identifier => T_IDENTIFIER,

        // Delimiters
        Token::LParen => T_LPAREN,
        Token::RParen => T_RPAREN,
        Token::LBracket => T_LBRACKET,
        Token::RBracket => T_RBRACKET,
        Token::LBrace => T_LBRACE,
        Token::RBrace => T_RBRACE,
        Token::Semicolon => T_SEMICOLON,
        Token::Colon => T_COLON,
        Token::Comma => T_COMMA,
        Token::Dot => T_DOT,

        // Operators
        Token::Plus => T_PLUS,
        Token::Minus => T_MINUS,
        Token::Star => T_STAR,
        Token::Slash => T_SLASH,
        Token::Percent => T_PERCENT,
        Token::Assign => T_ASSIGN,
        Token::Bang => T_NOT,
        Token::Less => T_LESSTHAN,
        Token::Greater => T_GREATERTHAN,
        Token::LessEqual => T_LESSTHANOREQUAL,
        Token::GreaterEqual => T_GREATERTHANOREQUAL,
        Token::EqualEqual => T_ISEQUALTO,
        Token::NotEqual => T_NOTEQUALTO,
        Token::LogicalAnd => T_LOGICALAND,
        Token::LogicalOr => T_LOGICALOR,
        Token::PlusAssign => T_INCREMENT,
        Token::MinusAssign => T_DECREMENT,

        // Non-token variants that should never reach the parser
        Token::Newline | Token::LineComment | Token::BlockComment => {
            unreachable!("whitespace/comment tokens should be filtered before parsing")
        }
    }
}

/// Result of parsing: either success or a list of error messages.
pub struct ParseResult {
    pub success: bool,
    pub errors: Vec<String>,
}

/// Parse the given source code and return whether it is syntactically valid.
///
/// This corresponds to Chapter 4 of the book: accept/reject with error recovery.
pub fn parse(input: &str) -> ParseResult {
    // Step 1: Lex with Logos, converting to lrpar's DefaultLexeme format.
    // LRNonStreamingLexer::new expects Vec<Result<DefaultLexeme, LRLexError>>.
    let mut lexemes: Vec<Result<DefaultLexeme<u8>, LRLexError>> = Vec::new();
    let mut newlines = NewlineCache::new();
    newlines.feed(input);

    let mut lex = Token::lexer(input);
    while let Some(result) = lex.next() {
        match result {
            Ok(tok) => {
                // Skip whitespace/comments — they're not parser tokens
                match tok {
                    Token::Newline | Token::LineComment | Token::BlockComment => continue,
                    _ => {
                        let span = lex.span();
                        let tid = token_id(&tok);
                        let start = span.start;
                        let len = span.end - span.start;
                        lexemes.push(Ok(DefaultLexeme::new(tid, start, len)));
                    }
                }
            }
            Err(_) => {
                // Lexer error — skip for now, the parser's error recovery will handle it.
                // We could also push an LRLexError here for better diagnostics.
            }
        }
    }

    // Step 2: Create the non-streaming lexer that lrpar expects
    let lexer = LRNonStreamingLexer::new(input, lexemes, newlines);

    // Step 3: Parse!
    // With YaccKind::Original(NoAction), parse() returns Vec<LexParseError>,
    // not a tuple.
    let errs = jzero_y::parse(&lexer);

    // Step 4: Collect errors into human-readable strings
    let error_msgs: Vec<String> = errs
        .iter()
        .map(|e| e.pp(&lexer, &jzero_y::token_epp))
        .collect();

    ParseResult {
        success: error_msgs.is_empty(),
        errors: error_msgs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_method_call() {
        // Simple: no dots at all
        let src = r#"
public class T {
    public static void main(String argv[]) {
        foo("hello");
    }
}
"#;
        let result = parse(src);
        if !result.success {
            println!("simple_method_call errors:");
            for e in &result.errors {
                println!("  {}", e);
            }
        }
        assert!(result.success, "simple call failed: {:?}", result.errors);
    }

    #[test]
    fn test_one_dot_method_call() {
        // One dot: System.println("hello")
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
        // Two dots: System.out.println("hello")
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
    fn test_empty_class() {
        let src = "public class Empty { }";
        let result = parse(src);
        assert!(
            result.success,
            "Expected success but got errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_field_declarations() {
        let src = r#"
public class Foo {
    int x;
    double y;
    string name;
}
"#;
        let result = parse(src);
        assert!(
            result.success,
            "Expected success but got errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_if_else() {
        let src = r#"
public class Test {
    public static void main(String argv[]) {
        if (x > 0) {
            y = 1;
        } else {
            y = 0;
        }
    }
}
"#;
        let result = parse(src);
        assert!(
            result.success,
            "Expected success but got errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_for_loop_no_init_decl() {
        // Jzero grammar doesn't support initializers in declarations.
        // For-loop with assignment in ForInit instead.
        let src = r#"
public class Test {
    public static void main(String args[]) {
        int i;
        for (i = 0; i < 10; i = i + 1) {
            i = i;
        }
    }
}
"#;
        let result = parse(src);
        if !result.success {
            println!("for_loop errors:");
            for e in &result.errors {
                println!("  {}", e);
            }
        }
        assert!(
            result.success,
            "Expected success but got errors: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_syntax_error_reports_error() {
        let src = "public class { }"; // missing identifier
        let result = parse(src);
        assert!(!result.success, "Expected errors but got success");
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_multiple_errors() {
        let src = r#"
public class Bad {
    public static void main(String args[]) {
        int = 5;
        if x > 0 {
            y = ;
        }
    }
}
"#;
        let result = parse(src);
        assert!(!result.success, "Expected errors but got success");
        // With error recovery, lrpar should report multiple errors
        println!("Errors found:");
        for e in &result.errors {
            println!("  {}", e);
        }
    }

    #[test]
    fn test_expressions() {
        let src = r#"
public class Expr {
    public static void main(String args[]) {
        int result;
        result = 2 + 3 * 4;
        result = (a + b) / c;
        result = x <= y && y >= z;
        result = a == b || c != d;
    }
}
"#;
        let result = parse(src);
        assert!(
            result.success,
            "Expected success but got errors: {:?}",
            result.errors
        );
    }
}