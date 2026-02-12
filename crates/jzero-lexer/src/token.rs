use logos::Logos;

#[derive(Default, Debug, Clone)]
pub struct LexerExtras {
    pub line: usize,
}

fn newline_callback(lex: &mut logos::Lexer<Token>) {
    lex.extras.line += 1;
}

fn block_comment_callback(lex: &mut logos::Lexer<Token>) {
    lex.extras.line += lex.slice().chars().filter(|&c| c == '\n').count();
}

#[derive(Logos, Debug, PartialEq, Eq, Hash, Clone)]
#[logos(extras = LexerExtras)]
#[logos(skip r"[ \t\r\f]+")]
#[logos(error = String)]
pub enum Token {
    // ── Comments & newlines (tracked for line counting, not emitted) ──
    #[regex(r"\n", newline_callback)]
    Newline,

    #[regex(r"//[^\n]*\n?", newline_callback, allow_greedy = true)]
    LineComment,

    #[regex(r"/\*([^*]|\*+[^*/])*\*+/", block_comment_callback)]
    BlockComment,

    // ── Keywords ──────────────────────────────────────────────
    #[token("bool")]
    Bool,
    #[token("break")]
    Break,
    #[token("class")]
    Class,
    #[token("double")]
    Double,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("if")]
    If,
    #[token("int")]
    Int,
    #[token("null")]
    Null,
    #[token("public")]
    Public,
    #[token("return")]
    Return,
    #[token("static")]
    Static,
    #[token("string")]
    StringKw,
    #[token("void")]
    Void,
    #[token("while")]
    While,

    // ── Boolean literals ──────────────────────────────────────
    #[token("true")]
    True,
    #[token("false")]
    False,

    // ── Delimiters ────────────────────────────────────────────
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,

    // ── Operators (multi-char) ────────────────────────────────
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("&&")]
    LogicalAnd,
    #[token("||")]
    LogicalOr,
    #[token("+=")]
    PlusAssign,
    #[token("-=")]
    MinusAssign,

    // ── Operators (single-char) ───────────────────────────────
    #[token("=")]
    Assign,
    #[token("!")]
    Bang,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,

    // ── Literals ──────────────────────────────────────────────
    #[regex(r"[0-9]*\.[0-9]*([eE][+-]?[0-9]+)?", priority = 3)]
    #[regex(r"[0-9]+[eE][+-]?[0-9]+", priority = 3)]
    DoubleLit,

    #[regex(r"[0-9]+", priority = 2)]
    IntLit,

    #[regex(r#""[^"]*""#)]
    StringLit,

    // ── Identifier ────────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier,
}

impl Token {
    /// Returns true for tokens that are only used for line tracking
    /// and should not be emitted to the parser.
    pub fn is_hidden(&self) -> bool {
        matches!(self, Token::Newline | Token::LineComment | Token::BlockComment)
    }
}