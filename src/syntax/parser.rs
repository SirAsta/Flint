use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use crate::error::ForgeError;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek().kind == *kind
    }

    fn consume(&mut self, kind: TokenKind, message: &str) -> Result<Token, ForgeError> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            Err(ForgeError::Parse { message: message.to_string(), line: self.peek().line })
        }
    }

    fn err(&self, message: &str) -> ForgeError {
        ForgeError::Parse { message: message.to_string(), line: self.peek().line }
    }

    pub fn parse_program(&mut self) -> Result<Vec<Stmt>, ForgeError> {
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::Eof) {
            stmts.push(self.statement()?);
        }
        Ok(stmts)
    }

    fn block(&mut self) -> Result<Vec<Stmt>, ForgeError> {
        self.consume(TokenKind::LBrace, "Expected '{' to start block")?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            stmts.push(self.statement()?);
        }
        self.consume(TokenKind::RBrace, "Expected '}' to end block")?;
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt, ForgeError> {
        if self.check(&TokenKind::Let) {
            self.advance();
            return self.var_decl();
        }
        if self.check(&TokenKind::If) {
            return self.if_stmt();
        }
        if self.check(&TokenKind::While) {
            return self.while_stmt();
        }
        if self.check(&TokenKind::For) {
            return self.for_stmt();
        }
        if self.check(&TokenKind::Fn) {
            return self.fn_stmt();
        }
        if self.check(&TokenKind::Return) {
            self.advance();
            let expr = self.expression()?;
            self.optional_semicolon();
            return Ok(Stmt::Return(expr));
        }
        if self.check(&TokenKind::Break) {
            self.advance();
            self.optional_semicolon();
            return Ok(Stmt::Break);
        }
        if self.check(&TokenKind::Continue) {
            self.advance();
            self.optional_semicolon();
            return Ok(Stmt::Continue);
        }
        if self.check(&TokenKind::Import) {
            self.advance();
            let path = match self.advance().kind {
                TokenKind::Str(s) => s,
                _ => return Err(self.err("Expected file path string after 'import'")),
            };
            self.optional_semicolon();
            return Ok(Stmt::Import(path));
        }
        if self.check(&TokenKind::Class) {
            return self.class_stmt();
        }
        if self.check(&TokenKind::LBrace) {
            let body = self.block()?;
            return Ok(Stmt::Block(body));
        }
        let expr = self.expression()?;
        if self.check(&TokenKind::Assign) {
            self.advance();
            let value = self.expression()?;
            self.optional_semicolon();
            match expr {
                Expr::Variable(name) => return Ok(Stmt::Assign { name, value }),
                Expr::Get { object, name } => {
                    return Ok(Stmt::Set { object, name, value: Box::new(value) });
                }
                Expr::Index { object, index } => {
                    return Ok(Stmt::SetIndex { object, index, value: Box::new(value) });
                }
                _ => return Err(self.err("Invalid assignment target")),
            }
        }
        self.optional_semicolon();
        Ok(Stmt::Expression(expr))
    }

    fn optional_semicolon(&mut self) {
        if self.check(&TokenKind::Semicolon) {
            self.advance();
        }
    }

    fn var_decl(&mut self) -> Result<Stmt, ForgeError> {
        let name = match self.advance().kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.err("Expected variable name after 'let'")),
        };
        let initializer = if self.check(&TokenKind::Assign) {
            self.advance();
            Some(self.expression()?)
        } else {
            None
        };
        self.optional_semicolon();
        Ok(Stmt::Var { name, initializer })
    }

    fn if_stmt(&mut self) -> Result<Stmt, ForgeError> {
        self.advance();
        let mut branches = Vec::new();
        let mut condition = self.expression()?;
        let mut body = self.block()?;
        branches.push((condition.clone(), std::mem::take(&mut body)));
        while self.check(&TokenKind::Elif) {
            self.advance();
            condition = self.expression()?;
            body = self.block()?;
            branches.push((condition, std::mem::take(&mut body)));
        }
        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
            Some(self.block()?)
        } else {
            None
        };
        Ok(Stmt::If { branches, else_branch })
    }

    fn while_stmt(&mut self) -> Result<Stmt, ForgeError> {
        self.advance();
        let condition = self.expression()?;
        let body = self.block()?;
        Ok(Stmt::While { condition, body })
    }

    fn for_stmt(&mut self) -> Result<Stmt, ForgeError> {
        self.advance();
        let variable = match self.advance().kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.err("Expected loop variable after 'for'")),
        };
        self.consume(TokenKind::In, "Expected 'in' in for loop")?;
        let iterable = self.expression()?;
        let body = self.block()?;
        Ok(Stmt::For { variable, iterable, body })
    }

    fn fn_stmt(&mut self) -> Result<Stmt, ForgeError> {
        self.advance();
        let name = match self.advance().kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.err("Expected function name")),
        };
        let params = self.params()?;
        let body = self.block()?;
        Ok(Stmt::Function { name, params, body })
    }

    fn class_stmt(&mut self) -> Result<Stmt, ForgeError> {
        self.advance();
        let name = match self.advance().kind {
            TokenKind::Ident(n) => n,
            _ => return Err(self.err("Expected class name")),
        };
        self.consume(TokenKind::LBrace, "Expected '{' to start class")?;
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            if self.check(&TokenKind::Fn) {
                self.advance();
                let mname = match self.advance().kind {
                    TokenKind::Ident(n) => n,
                    _ => return Err(self.err("Expected method name")),
                };
                let params = self.params()?;
                let body = self.block()?;
                methods.push((mname, params, body));
            } else {
                return Err(self.err("Expected a method definition"));
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}' to end class")?;
        Ok(Stmt::Class { name, methods })
    }

    fn params(&mut self) -> Result<Vec<String>, ForgeError> {
        self.consume(TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                match self.advance().kind {
                    TokenKind::Ident(n) => params.push(n),
                    _ => return Err(self.err("Expected parameter name")),
                }
                if self.check(&TokenKind::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')' after parameters")?;
        Ok(params)
    }

    fn expression(&mut self) -> Result<Expr, ForgeError> {
        self.logical_or()
    }

    fn logical_or(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.logical_and()?;
        while self.check(&TokenKind::Or) {
            self.advance();
            let right = self.logical_and()?;
            left = Expr::Logical { left: Box::new(left), op: LogicalOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn logical_and(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.equality()?;
        while self.check(&TokenKind::And) {
            self.advance();
            let right = self.equality()?;
            left = Expr::Logical { left: Box::new(left), op: LogicalOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    fn equality(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.comparison()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::EqEq => BinaryOp::EqEq,
                TokenKind::BangEq => BinaryOp::Neq,
                _ => break,
            };
            self.advance();
            let right = self.comparison()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn comparison(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Less => BinaryOp::Lt,
                TokenKind::LessEq => BinaryOp::Le,
                TokenKind::Greater => BinaryOp::Gt,
                TokenKind::GreaterEq => BinaryOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.term()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.factor()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Plus => BinaryOp::Add,
                TokenKind::Minus => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.factor()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Expr, ForgeError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Star => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.unary()?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, ForgeError> {
        if self.check(&TokenKind::Minus) {
            self.advance();
            let expr = self.unary()?;
            return Ok(Expr::Unary { op: UnaryOp::Neg, expr: Box::new(expr) });
        }
        if self.check(&TokenKind::Not) {
            self.advance();
            let expr = self.unary()?;
            return Ok(Expr::Unary { op: UnaryOp::Not, expr: Box::new(expr) });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, ForgeError> {
        let mut expr = self.primary()?;
        loop {
            if self.check(&TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.expression()?);
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after arguments")?;
                expr = Expr::Call { callee: Box::new(expr), args };
            } else if self.check(&TokenKind::Dot) {
                self.advance();
                let name = match self.advance().kind {
                    TokenKind::Ident(n) => n,
                    _ => return Err(self.err("Expected property name after '.'")),
                };
                expr = Expr::Get { object: Box::new(expr), name };
            } else if self.check(&TokenKind::LBracket) {
                self.advance();
                let index = self.expression()?;
                self.consume(TokenKind::RBracket, "Expected ']' after index")?;
                expr = Expr::Index { object: Box::new(expr), index: Box::new(index) };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn primary(&mut self) -> Result<Expr, ForgeError> {
        match self.advance().kind {
            TokenKind::Number(n) => Ok(Expr::Number(n)),
            TokenKind::Str(s) => Ok(Expr::Str(s)),
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Null => Ok(Expr::Null),
            TokenKind::Ident(name) => Ok(Expr::Variable(name)),
            TokenKind::This => Ok(Expr::Variable("this".to_string())),
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(TokenKind::RParen, "Expected ')' after expression")?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                let mut items = Vec::new();
                if !self.check(&TokenKind::RBracket) {
                    loop {
                        items.push(self.expression()?);
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RBracket, "Expected ']' after list")?;
                Ok(Expr::List(items))
            }
            TokenKind::LBrace => {
                let mut pairs = Vec::new();
                if !self.check(&TokenKind::RBrace) {
                    loop {
                        let k = self.expression()?;
                        self.consume(TokenKind::Colon, "Expected ':' between key and value")?;
                        let v = self.expression()?;
                        pairs.push((k, v));
                        if self.check(&TokenKind::Comma) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RBrace, "Expected '}' after map")?;
                Ok(Expr::Map(pairs))
            }
            _ => Err(self.err("Expected expression")),
        }
    }
}
