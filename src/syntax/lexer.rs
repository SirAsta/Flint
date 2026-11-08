use crate::token::{Token, TokenKind};

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if let Some(c) = c {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
            }
        }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
                self.advance();
            } else if c == '#' {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn number(&mut self) -> TokenKind {
        let mut buf = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                buf.push(c);
                self.advance();
            } else {
                break;
            }
        }
        TokenKind::Number(buf.parse().unwrap_or(0.0))
    }

    fn string(&mut self) -> Result<TokenKind, String> {
        self.advance();
        let mut buf = String::new();
        loop {
            match self.advance() {
                Some('"') => break,
                Some('\\') => match self.advance() {
                    Some('n') => buf.push('\n'),
                    Some('t') => buf.push('\t'),
                    Some('\\') => buf.push('\\'),
                    Some('"') => buf.push('"'),
                    Some(c) => buf.push(c),
                    None => return Err(format!("Unterminated string at line {}", self.line)),
                },
                Some(c) => buf.push(c),
                None => return Err(format!("Unterminated string at line {}", self.line)),
            }
        }
        Ok(TokenKind::Str(buf))
    }

    fn identifier(&mut self) -> TokenKind {
        let mut buf = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                buf.push(c);
                self.advance();
            } else {
                break;
            }
        }
        match buf.as_str() {
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "elif" => TokenKind::Elif,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "import" => TokenKind::Import,
            "class" => TokenKind::Class,
            "this" => TokenKind::This,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Ident(buf),
        }
    }

    pub fn scan(mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ws();
            let Some(c) = self.peek() else {
                tokens.push(Token { kind: TokenKind::Eof, line: self.line });
                break;
            };
            let line = self.line;
            let kind = match c {
                '0'..='9' => self.number(),
                '"' => self.string()?,
                '+' => { self.advance(); TokenKind::Plus }
                '-' => { self.advance(); TokenKind::Minus }
                '*' => { self.advance(); TokenKind::Star }
                '/' => { self.advance(); TokenKind::Slash }
                '%' => { self.advance(); TokenKind::Percent }
                '=' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::EqEq } else { TokenKind::Assign }
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::BangEq } else { TokenKind::Not }
                }
                '<' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::LessEq } else { TokenKind::Less }
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); TokenKind::GreaterEq } else { TokenKind::Greater }
                }
                '(' => { self.advance(); TokenKind::LParen }
                ')' => { self.advance(); TokenKind::RParen }
                '{' => { self.advance(); TokenKind::LBrace }
                '}' => { self.advance(); TokenKind::RBrace }
                '[' => { self.advance(); TokenKind::LBracket }
                ']' => { self.advance(); TokenKind::RBracket }
                ',' => { self.advance(); TokenKind::Comma }
                ';' => { self.advance(); TokenKind::Semicolon }
                ':' => { self.advance(); TokenKind::Colon }
                '.' => { self.advance(); TokenKind::Dot }
                _ if c.is_alphanumeric() || c == '_' => self.identifier(),
                _ => return Err(format!("Unexpected character '{}' at line {}", c, line)),
            };
            tokens.push(Token { kind, line });
        }
        Ok(tokens)
    }
}
