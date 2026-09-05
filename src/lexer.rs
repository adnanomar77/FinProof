use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.skip_whitespace_and_comments();

            if self.is_at_end() {
                break;
            }

            tokens.push(self.next_token()?);
        }

        tokens.push(Token::new(TokenKind::Eof, "", self.line, self.column));

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let start_line = self.line;
        let start_column = self.column;

        let ch = self.advance();

        let token = match ch {
            '{' => Token::new(TokenKind::LeftBrace, "{", start_line, start_column),

            '}' => Token::new(TokenKind::RightBrace, "}", start_line, start_column),

            '(' => Token::new(TokenKind::LeftParen, "(", start_line, start_column),

            ')' => Token::new(TokenKind::RightParen, ")", start_line, start_column),

            ':' => Token::new(TokenKind::Colon, ":", start_line, start_column),

            ',' => Token::new(TokenKind::Comma, ",", start_line, start_column),

            '.' => Token::new(TokenKind::Dot, ".", start_line, start_column),

            '+' => Token::new(TokenKind::Plus, "+", start_line, start_column),

            '-' => Token::new(TokenKind::Minus, "-", start_line, start_column),

            '*' => Token::new(TokenKind::Star, "*", start_line, start_column),

            '/' => Token::new(TokenKind::Slash, "/", start_line, start_column),

            '=' => {
                if self.match_char('=') {
                    Token::new(TokenKind::EqualEqual, "==", start_line, start_column)
                } else {
                    Token::new(TokenKind::Equal, "=", start_line, start_column)
                }
            }

            '!' => {
                if self.match_char('=') {
                    Token::new(TokenKind::NotEqual, "!=", start_line, start_column)
                } else {
                    return Err(self.error(
                        start_line,
                        start_column,
                        "Unexpected '!'. Expected '!='.",
                    ));
                }
            }

            '>' => {
                if self.match_char('=') {
                    Token::new(TokenKind::GreaterEqual, ">=", start_line, start_column)
                } else {
                    Token::new(TokenKind::Greater, ">", start_line, start_column)
                }
            }

            '<' => {
                if self.match_char('=') {
                    Token::new(TokenKind::LessEqual, "<=", start_line, start_column)
                } else {
                    Token::new(TokenKind::Less, "<", start_line, start_column)
                }
            }

            c if c.is_ascii_digit() => {
                return self.number(c, start_line, start_column);
            }

            c if is_identifier_start(c) => {
                return Ok(self.identifier(c, start_line, start_column));
            }

            other => {
                return Err(self.error(
                    start_line,
                    start_column,
                    &format!("Unexpected character '{}'.", other),
                ));
            }
        };

        Ok(token)
    }

    fn number(&mut self, first: char, line: usize, column: usize) -> Result<Token, String> {
        let mut value = String::new();
        value.push(first);

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                value.push(self.advance());
            } else {
                break;
            }
        }

        if self.peek() == Some('.') {
            value.push(self.advance());

            if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(self.error(
                    line,
                    column,
                    "Invalid number: decimal point must be followed by digits.",
                ));
            }

            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    value.push(self.advance());
                } else {
                    break;
                }
            }
        }

        Ok(Token::new(TokenKind::Number, value, line, column))
    }

    fn identifier(&mut self, first: char, line: usize, column: usize) -> Token {
        let mut value = String::new();
        value.push(first);

        while let Some(c) = self.peek() {
            if is_identifier_continue(c) {
                value.push(self.advance());
            } else {
                break;
            }
        }

        let kind = keyword_kind(&value).unwrap_or(TokenKind::Identifier);

        Token::new(kind, value, line, column)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while let Some(c) = self.peek() {
                if c.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }

            if self.peek() == Some('/') && self.peek_next() == Some('/') {
                while let Some(c) = self.peek() {
                    self.advance();

                    if c == '\n' {
                        break;
                    }
                }

                continue;
            }

            break;
        }
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.position];
        self.position += 1;

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        c
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.position).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.source.get(self.position + 1).copied()
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.source.len()
    }

    fn error(&self, line: usize, column: usize, message: &str) -> String {
        format!(
            "FP3001 LEXER_ERROR: Lexer error at line {}, column {}: {}",
            line, column, message
        )
    }
}

fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_identifier_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn keyword_kind(value: &str) -> Option<TokenKind> {
    let kind = match value {
        "currency" => TokenKind::Currency,
        "account" => TokenKind::Account,
        "transaction" => TokenKind::Transaction,
        "rule" => TokenKind::Rule,

        "pay" => TokenKind::Pay,
        "transfer" => TokenKind::Transfer,
        "debit" => TokenKind::Debit,
        "credit" => TokenKind::Credit,
        "receive" => TokenKind::Receive,
        "hold" => TokenKind::Hold,
        "release" => TokenKind::Release,
        "capture" => TokenKind::Capture,
        "settle" => TokenKind::Settle,
        "fee" => TokenKind::Fee,
        "refund" => TokenKind::Refund,
        "reverse" => TokenKind::Reverse,
        "authorize" => TokenKind::Authorize,

        "from" => TokenKind::From,
        "to" => TokenKind::To,

        // Financial account types
        "Asset" | "asset" => TokenKind::Asset,
        "Liability" | "liability" => TokenKind::Liability,
        "Equity" | "equity" => TokenKind::Equity,
        "Revenue" | "revenue" => TokenKind::Revenue,
        "Expense" | "expense" => TokenKind::Expense,

        "if" => TokenKind::If,
        "module" => TokenKind::Module,
        "import" => TokenKind::Import,
        "export" => TokenKind::Export,

        _ => return None,
    };

    Some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_transfer() {
        let source = "transfer 500 USD from Cash to Bank";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Transfer);
        assert_eq!(tokens[1].kind, TokenKind::Number);
        assert_eq!(tokens[1].lexeme, "500");
        assert_eq!(tokens[2].kind, TokenKind::Identifier);
        assert_eq!(tokens[2].lexeme, "USD");
        assert_eq!(tokens[3].kind, TokenKind::From);
        assert_eq!(tokens[4].kind, TokenKind::Identifier);
        assert_eq!(tokens[4].lexeme, "Cash");
        assert_eq!(tokens[5].kind, TokenKind::To);
        assert_eq!(tokens[6].lexeme, "Bank");
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn tokenizes_decimal() {
        let mut lexer = Lexer::new("1000.50 USD");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[0].lexeme, "1000.50");
    }

    #[test]
    fn tokenizes_comparisons() {
        let mut lexer = Lexer::new(">= <= == != > <");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::GreaterEqual);
        assert_eq!(tokens[1].kind, TokenKind::LessEqual);
        assert_eq!(tokens[2].kind, TokenKind::EqualEqual);
        assert_eq!(tokens[3].kind, TokenKind::NotEqual);
        assert_eq!(tokens[4].kind, TokenKind::Greater);
        assert_eq!(tokens[5].kind, TokenKind::Less);
    }

    #[test]
    fn tokenizes_debit_and_credit() {
        let mut lexer = Lexer::new("debit credit");
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Debit);
        assert_eq!(tokens[1].kind, TokenKind::Credit);
    }

    #[test]
    fn ignores_comments() {
        let source = "pay 100 USD // payment\nfrom Cash";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Pay);
        assert_eq!(tokens[1].lexeme, "100");
        assert_eq!(tokens[2].lexeme, "USD");
        assert_eq!(tokens[3].kind, TokenKind::From);
    }

    #[test]
    fn rejects_unknown_character() {
        let mut lexer = Lexer::new("pay @");

        let result = lexer.tokenize();

        assert!(result.is_err());
    }

    #[test]
    fn tracks_line_and_column() {
        let source = "pay 100 USD\nfrom Cash";

        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();

        assert_eq!(tokens[3].line, 2);
        assert_eq!(tokens[3].column, 1);
    }
}
