pub mod token;

use logos::Logos;
use token::{LexerExtras, Token};

/// A token paired with its source text and line number.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub text: String,
    pub line: usize,
}

/// Lex the input source, returning all meaningful tokens with line numbers.
///
/// Hidden tokens (newlines, comments) are consumed for line tracking
/// but not included in the output.
pub fn lex(source: &str) -> Result<Vec<SpannedToken>, Vec<LexError>> {
    let mut lexer = Token::lexer_with_extras(source, LexerExtras { line: 1 });
    let mut tokens = Vec::new();
    let mut errors = Vec::new();

    while let Some(result) = lexer.next() {
        let line = lexer.extras.line;
        let text = lexer.slice().to_string();

        match result {
            Ok(tok) if tok.is_hidden() => continue,
            Ok(tok) => {
                tokens.push(SpannedToken {
                    token: tok,
                    text,
                    line,
                });
            }
            Err(_) => {
                errors.push(LexError {
                    line,
                    text,
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(tokens)
    } else {
        Err(errors)
    }
}

/// A lexical error with location info.
#[derive(Debug, Clone)]
pub struct LexError {
    pub line: usize,
    pub text: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: unrecognized character: {:?}", self.line, self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::Token;

    #[test]
    fn test_hello_jzero() {
        let source = r#"public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}"#;

        let tokens = lex(source).expect("lexing should succeed");

        for token in &tokens {
            println!("{:?}", token);
        }

        // println!("tokens: {:?}", tokens);

        // Verify first few tokens
        assert_eq!(tokens[0].token, Token::Public);
        assert_eq!(tokens[0].line, 1);

        assert_eq!(tokens[1].token, Token::Class);
        assert_eq!(tokens[1].line, 1);

        assert_eq!(tokens[2].token, Token::Identifier);
        assert_eq!(tokens[2].text, "hello");
        assert_eq!(tokens[2].line, 1);

        // "hello, jzero!" string literal
        let string_tok = tokens.iter().find(|t| t.token == Token::StringLit).unwrap();
        assert_eq!(string_tok.text, r#""hello, jzero!""#);
        assert_eq!(string_tok.line, 3);

        // Last token should be closing brace on line 5
        let last = tokens.last().unwrap();
        assert_eq!(last.token, Token::RBrace);
        assert_eq!(last.line, 5);
    }

    #[test]
    fn test_block_comment_line_tracking() {
        let source = "int /* comment\nspanning\nlines */ x";

        let tokens = lex(source).expect("lexing should succeed");

        assert_eq!(tokens[0].token, Token::Int);
        assert_eq!(tokens[0].line, 1);

        // 'x' should be on line 3 after the multi-line comment
        assert_eq!(tokens[1].token, Token::Identifier);
        assert_eq!(tokens[1].text, "x");
        assert_eq!(tokens[1].line, 3);
    }

    #[test]
    fn test_unrecognized_character() {
        let source = "int @ x";

        let errors = lex(source).unwrap_err();

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].text, "@");
        assert_eq!(errors[0].line, 1);
    }
}