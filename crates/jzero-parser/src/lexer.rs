//! Bridge between jzero-lexer (Logos) and LALRPOP's expected iterator format.
//!
//! LALRPOP expects an iterator of `Result<(usize, Token, usize), Error>`.
//! We define a wrapper `Tok` enum that carries `&str` slices for
//! value-bearing tokens and map from jzero_lexer::Token.

use jzero_lexer::token::Token;
use logos::SpannedIter;
use std::fmt;

/// Parser-facing token enum. Wraps jzero_lexer::Token but carries
/// borrowed string slices for tokens that need their text content.
#[derive(Clone, Debug, PartialEq)]
pub enum Tok<'input> {
    // Keywords
    Bool,
    Break,
    Class,
    Double,
    Else,
    For,
    If,
    Int,
    Null,
    Public,
    Return,
    Static,
    StringKw,
    Void,
    While,

    // Boolean literals
    BoolLit(bool),

    // Literals with text
    IntLit(&'input str),
    DoubleLit(&'input str),
    StringLit(&'input str),

    // Identifier with text
    Identifier(&'input str),

    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Semicolon,
    Comma,
    Dot,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Bang,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    EqualEqual,
    NotEqual,
    LogicalAnd,
    LogicalOr,
    PlusAssign,
    MinusAssign,
}

impl<'input> fmt::Display for Tok<'input> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Bool => write!(f, "bool"),
            Tok::Break => write!(f, "break"),
            Tok::Class => write!(f, "class"),
            Tok::Double => write!(f, "double"),
            Tok::Else => write!(f, "else"),
            Tok::For => write!(f, "for"),
            Tok::If => write!(f, "if"),
            Tok::Int => write!(f, "int"),
            Tok::Null => write!(f, "null"),
            Tok::Public => write!(f, "public"),
            Tok::Return => write!(f, "return"),
            Tok::Static => write!(f, "static"),
            Tok::StringKw => write!(f, "string"),
            Tok::Void => write!(f, "void"),
            Tok::While => write!(f, "while"),
            Tok::BoolLit(b) => write!(f, "{}", b),
            Tok::IntLit(s) => write!(f, "{}", s),
            Tok::DoubleLit(s) => write!(f, "{}", s),
            Tok::StringLit(s) => write!(f, "{}", s),
            Tok::Identifier(s) => write!(f, "{}", s),
            Tok::LParen => write!(f, "("),
            Tok::RParen => write!(f, ")"),
            Tok::LBracket => write!(f, "["),
            Tok::RBracket => write!(f, "]"),
            Tok::LBrace => write!(f, "{{"),
            Tok::RBrace => write!(f, "}}"),
            Tok::Semicolon => write!(f, ";"),
            Tok::Comma => write!(f, ","),
            Tok::Dot => write!(f, "."),
            Tok::Plus => write!(f, "+"),
            Tok::Minus => write!(f, "-"),
            Tok::Star => write!(f, "*"),
            Tok::Slash => write!(f, "/"),
            Tok::Percent => write!(f, "%"),
            Tok::Assign => write!(f, "="),
            Tok::Bang => write!(f, "!"),
            Tok::Less => write!(f, "<"),
            Tok::Greater => write!(f, ">"),
            Tok::LessEqual => write!(f, "<="),
            Tok::GreaterEqual => write!(f, ">="),
            Tok::EqualEqual => write!(f, "=="),
            Tok::NotEqual => write!(f, "!="),
            Tok::LogicalAnd => write!(f, "&&"),
            Tok::LogicalOr => write!(f, "||"),
            Tok::PlusAssign => write!(f, "+="),
            Tok::MinusAssign => write!(f, "-="),
        }
    }
}

/// Lexical error type for LALRPOP.
#[derive(Clone, Debug, PartialEq)]
pub struct LexicalError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for LexicalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lexical error at byte {}: {}", self.pos, self.msg)
    }
}

/// LALRPOP-compatible lexer that wraps jzero_lexer's Logos lexer.
/// Produces `Result<(usize, Tok, usize), LexicalError>` triples.
pub struct Lexer<'input> {
    input: &'input str,
    inner: SpannedIter<'input, Token>,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        use logos::Logos;
        Lexer {
            input,
            inner: Token::lexer(input).spanned(),
        }
    }

    /// Convert a jzero_lexer::Token (bare) into a Tok (with borrowed text).
    fn map_token(&self, tok: Token, start: usize, end: usize) -> Tok<'input> {
        let slice = &self.input[start..end];
        match tok {
            // Keywords
            Token::Bool => Tok::Bool,
            Token::Break => Tok::Break,
            Token::Class => Tok::Class,
            Token::Double => Tok::Double,
            Token::Else => Tok::Else,
            Token::For => Tok::For,
            Token::If => Tok::If,
            Token::Int => Tok::Int,
            Token::Null => Tok::Null,
            Token::Public => Tok::Public,
            Token::Return => Tok::Return,
            Token::Static => Tok::Static,
            Token::StringKw => Tok::StringKw,
            Token::Void => Tok::Void,
            Token::While => Tok::While,

            // Boolean literals
            Token::True => Tok::BoolLit(true),
            Token::False => Tok::BoolLit(false),

            // Literals with text
            Token::IntLit => Tok::IntLit(slice),
            Token::DoubleLit => Tok::DoubleLit(slice),
            Token::StringLit => Tok::StringLit(slice),

            // Identifier
            Token::Identifier => Tok::Identifier(slice),

            // Delimiters
            Token::LParen => Tok::LParen,
            Token::RParen => Tok::RParen,
            Token::LBracket => Tok::LBracket,
            Token::RBracket => Tok::RBracket,
            Token::LBrace => Tok::LBrace,
            Token::RBrace => Tok::RBrace,
            Token::Semicolon => Tok::Semicolon,
            Token::Comma => Tok::Comma,
            Token::Dot => Tok::Dot,

            // Operators
            Token::Plus => Tok::Plus,
            Token::Minus => Tok::Minus,
            Token::Star => Tok::Star,
            Token::Slash => Tok::Slash,
            Token::Percent => Tok::Percent,
            Token::Assign => Tok::Assign,
            Token::Bang => Tok::Bang,
            Token::Less => Tok::Less,
            Token::Greater => Tok::Greater,
            Token::LessEqual => Tok::LessEqual,
            Token::GreaterEqual => Tok::GreaterEqual,
            Token::EqualEqual => Tok::EqualEqual,
            Token::NotEqual => Tok::NotEqual,
            Token::LogicalAnd => Tok::LogicalAnd,
            Token::LogicalOr => Tok::LogicalOr,
            Token::PlusAssign => Tok::PlusAssign,
            Token::MinusAssign => Tok::MinusAssign,

            // Hidden tokens — should be filtered, but just in case
            Token::Colon => Tok::Semicolon, // unused in grammar; map to something safe
            Token::Newline | Token::LineComment | Token::BlockComment => {
                unreachable!("hidden tokens should be filtered")
            }
        }
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = Result<(usize, Tok<'input>, usize), LexicalError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                None => return None,
                Some((result, span)) => {
                    match result {
                        Ok(tok) => {
                            if tok.is_hidden() {
                                continue;
                            }
                            let mapped = self.map_token(tok, span.start, span.end);
                            eprintln!("  TOKEN: {:?} @ {}..{}", mapped, span.start, span.end);
                            return Some(Ok((span.start, mapped, span.end)));
                        }
                        Err(err_msg) => {
                            return Some(Err(LexicalError {
                                pos: span.start,
                                msg: err_msg,
                            }));
                        }
                    }
                }
            }
        }
    }
}