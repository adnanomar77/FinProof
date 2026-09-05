#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Currency,
    Account,
    Transaction,
    Rule,

    Pay,
    Transfer,
    Debit,
    Credit,
    Receive,
    Hold,
    Release,
    Capture,
    Settle,
    Fee,
    Refund,
    Reverse,
    Authorize,

    From,
    To,

    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,

    If,
    Module,
    Import,
    Export,

    // General tokens
    Identifier,
    Number,

    // Symbols
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    Colon,
    Comma,
    Dot,

    Plus,
    Minus,
    Star,
    Slash,

    Equal,
    EqualEqual,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Special
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(kind: TokenKind, lexeme: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            kind,
            lexeme: lexeme.into(),
            line,
            column,
        }
    }
}
