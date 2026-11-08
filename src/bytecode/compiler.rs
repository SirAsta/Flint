use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use crate::chunk::*;
use crate::error::ForgeError;
use crate::value::Value;

pub struct Compiler {
    chunk: Chunk,
    loops: Vec<LoopInfo>,
    counter: usize,
}

struct LoopInfo {
    continue_target: usize,
    breaks: Vec<usize>,
}

impl Compiler {
    pub fn compile_program(stmts: &[Stmt]) -> Result<Rc<Chunk>, ForgeError> {
        let mut c = Compiler {
            chunk: Chunk::new(),
            loops: Vec::new(),
            counter: 0,
        };
        for s in stmts {
            c.compile_stmt(s)?;
        }
        c.chunk.code.push(OP_NULL);
        c.chunk.code.push(OP_RETURN);
        Ok(Rc::new(c.chunk))
    }

    fn compile_function(
        &mut self,
        name: &str,
        params: &[String],
        body: &[Stmt],
    ) -> Result<Rc<Chunk>, ForgeError> {
        let mut f = Compiler {
            chunk: Chunk::new(),
            loops: Vec::new(),
            counter: 0,
        };
        for s in body {
            f.compile_stmt(s)?;
        }
        f.chunk.params = params.to_vec();
        f.chunk.code.push(OP_NULL);
        f.chunk.code.push(OP_RETURN);
        let _ = name;
        Ok(Rc::new(f.chunk))
    }

    fn emit(&mut self, b: u8) {
        self.chunk.code.push(b);
    }

    fn emit_u16(&mut self, n: u16) {
        self.chunk.code.push((n & 0xff) as u8);
        self.chunk.code.push((n >> 8) as u8);
    }

    fn const_idx(&mut self, v: Value) -> u16 {
        self.chunk.add_constant(v)
    }

    fn name_idx(&mut self, name: &str) -> u16 {
        self.chunk.add_constant(Value::Str(name.to_string()))
    }

    fn jump(&mut self, op: u8) -> usize {
        let pos = self.chunk.code.len();
        self.emit(op);
        self.emit(0);
        self.emit(0);
        pos
    }

    fn jump_back(&mut self, op: u8, target: usize) {
        let pos = self.chunk.code.len();
        self.emit(op);
        self.emit(0);
        self.emit(0);
        self.patch(pos, target);
    }

    fn patch(&mut self, pos: usize, target: usize) {
        let offset = target as i32 - (pos as i32 + 3);
        let bytes = (offset as i16).to_le_bytes();
        self.chunk.code[pos + 1] = bytes[0];
        self.chunk.code[pos + 2] = bytes[1];
    }

    fn compile_expr(&mut self, e: &Expr) -> Result<(), ForgeError> {
        match e {
            Expr::Number(n) => {
                let i = self.const_idx(Value::Number(*n));
                self.emit(OP_CONST);
                self.emit_u16(i);
            }
            Expr::Str(s) => {
                let i = self.const_idx(Value::Str(s.clone()));
                self.emit(OP_CONST);
                self.emit_u16(i);
            }
            Expr::Bool(b) => {
                let i = self.const_idx(Value::Bool(*b));
                self.emit(OP_CONST);
                self.emit_u16(i);
            }
            Expr::Null => self.emit(OP_NULL),
            Expr::Variable(name) => {
                let i = self.name_idx(name);
                self.emit(OP_GET_VAR);
                self.emit_u16(i);
            }
            Expr::Unary { op, expr } => {
                self.compile_expr(expr)?;
                match op {
                    UnaryOp::Neg => self.emit(OP_NEG),
                    UnaryOp::Not => self.emit(OP_NOT),
                }
            }
            Expr::Binary { left, op, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                let op = match op {
                    BinaryOp::Add => OP_ADD,
                    BinaryOp::Sub => OP_SUB,
                    BinaryOp::Mul => OP_MUL,
                    BinaryOp::Div => OP_DIV,
                    BinaryOp::Mod => OP_MOD,
                    BinaryOp::EqEq => OP_EQ,
                    BinaryOp::Neq => OP_NE,
                    BinaryOp::Lt => OP_LT,
                    BinaryOp::Le => OP_LE,
                    BinaryOp::Gt => OP_GT,
                    BinaryOp::Ge => OP_GE,
                };
                self.emit(op);
            }
            Expr::Logical { left, op, right } => {
                self.compile_expr(left)?;
                match op {
                    LogicalOp::And => {
                        let j = self.jump(OP_JUMP_IF_FALSE);
                        self.compile_expr(right)?;
                        let end = self.jump(OP_JUMP);
                        self.patch(j, self.chunk.code.len());
                        self.emit(OP_CONST);
                        let fi = self.const_idx(Value::Bool(false));
                        self.emit_u16(fi);
                        self.patch(end, self.chunk.code.len());
                    }
                    LogicalOp::Or => {
                        let j = self.jump(OP_JUMP_IF_FALSE);
                        self.emit(OP_CONST);
                        let ti = self.const_idx(Value::Bool(true));
                        self.emit_u16(ti);
                        let end = self.jump(OP_JUMP);
                        self.patch(j, self.chunk.code.len());
                        self.compile_expr(right)?;
                        self.patch(end, self.chunk.code.len());
                    }
                }
            }
            Expr::Call { callee, args } => {
                self.compile_expr(callee)?;
                for a in args {
                    self.compile_expr(a)?;
                }
                self.emit(OP_CALL);
                self.emit(args.len() as u8);
            }
            Expr::List(items) => {
                for it in items {
                    self.compile_expr(it)?;
                }
                self.emit(OP_LIST);
                self.emit_u16(items.len() as u16);
            }
            Expr::Map(pairs) => {
                for (k, v) in pairs {
                    self.compile_expr(k)?;
                    self.compile_expr(v)?;
                }
                self.emit(OP_MAP);
                self.emit_u16(pairs.len() as u16);
            }
            Expr::Index { object, index } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.emit(OP_INDEX);
            }
            Expr::Get { object, name } => {
                self.compile_expr(object)?;
                let i = self.name_idx(name);
                self.emit(OP_GET_PROP);
                self.emit_u16(i);
            }
        }
        Ok(())
    }

    fn compile_stmt(&mut self, s: &Stmt) -> Result<(), ForgeError> {
        match s {
            Stmt::Expression(e) => {
                self.compile_expr(e)?;
                self.emit(OP_POP);
            }
            Stmt::Var { name, initializer } => {
                match initializer {
                    Some(e) => self.compile_expr(e)?,
                    None => self.emit(OP_NULL),
                }
                let i = self.name_idx(name);
                self.emit(OP_DEFINE_VAR);
                self.emit_u16(i);
            }
            Stmt::Assign { name, value } => {
                self.compile_expr(value)?;
                let i = self.name_idx(name);
                self.emit(OP_SET_VAR);
                self.emit_u16(i);
            }
            Stmt::Set { object, name, value } => {
                self.compile_expr(object)?;
                self.compile_expr(value)?;
                let i = self.name_idx(name);
                self.emit(OP_SET_PROP);
                self.emit_u16(i);
            }
            Stmt::SetIndex { object, index, value } => {
                self.compile_expr(object)?;
                self.compile_expr(index)?;
                self.compile_expr(value)?;
                self.emit(OP_SET_INDEX);
            }
            Stmt::If { branches, else_branch } => {
                let mut next_patches = Vec::new();
                let mut end_patches = Vec::new();
                for (cond, body) in branches {
                    self.compile_expr(cond)?;
                    next_patches.push(self.jump(OP_JUMP_IF_FALSE));
                    for b in body {
                        self.compile_stmt(b)?;
                    }
                    end_patches.push(self.jump(OP_JUMP));
                    let n = next_patches.pop().unwrap();
                    self.patch(n, self.chunk.code.len());
                }
                if let Some(eb) = else_branch {
                    for b in eb {
                        self.compile_stmt(b)?;
                    }
                }
                for p in end_patches {
                    self.patch(p, self.chunk.code.len());
                }
            }
            Stmt::While { condition, body } => {
                let start = self.chunk.code.len();
                self.loops.push(LoopInfo {
                    continue_target: start,
                    breaks: Vec::new(),
                });
                self.compile_expr(condition)?;
                let end = self.jump(OP_JUMP_IF_FALSE);
                for b in body {
                    self.compile_stmt(b)?;
                }
                self.jump_back(OP_JUMP, start);
                self.patch(end, self.chunk.code.len());
                let info = self.loops.pop().unwrap();
                for br in info.breaks {
                    self.patch(br, self.chunk.code.len());
                }
            }
            Stmt::For { variable, iterable, body } => {
                self.counter += 1;
                let it = format!("$iter{}", self.counter);
                let ix = format!("$idx{}", self.counter);
                self.compile_expr(iterable)?;
                self.emit(OP_ITERABLE);
                let i_it = self.name_idx(&it);
                self.emit(OP_DEFINE_VAR);
                self.emit_u16(i_it);
                let z = self.const_idx(Value::Number(0.0));
                self.emit(OP_CONST);
                self.emit_u16(z);
                let i_ix = self.name_idx(&ix);
                self.emit(OP_DEFINE_VAR);
                self.emit_u16(i_ix);
                let start = self.chunk.code.len();
                self.loops.push(LoopInfo {
                    continue_target: start,
                    breaks: Vec::new(),
                });
                self.emit(OP_GET_VAR);
                self.emit_u16(i_ix);
                self.emit(OP_GET_VAR);
                self.emit_u16(i_it);
                self.emit(OP_LEN);
                self.emit(OP_LT);
                let end = self.jump(OP_JUMP_IF_FALSE);
                self.emit(OP_GET_VAR);
                self.emit_u16(i_it);
                self.emit(OP_GET_VAR);
                self.emit_u16(i_ix);
                self.emit(OP_INDEX);
                let i_v = self.name_idx(variable);
                self.emit(OP_DEFINE_VAR);
                self.emit_u16(i_v);
                for b in body {
                    self.compile_stmt(b)?;
                }
                self.emit(OP_GET_VAR);
                self.emit_u16(i_ix);
                let one = self.const_idx(Value::Number(1.0));
                self.emit(OP_CONST);
                self.emit_u16(one);
                self.emit(OP_ADD);
                self.emit(OP_SET_VAR);
                self.emit_u16(i_ix);
                self.jump_back(OP_JUMP, start);
                self.patch(end, self.chunk.code.len());
                let info = self.loops.pop().unwrap();
                for br in info.breaks {
                    self.patch(br, self.chunk.code.len());
                }
            }
            Stmt::Function { name, params, body } => {
                let sub = self.compile_function(name, params, body)?;
                let subidx = self.chunk.sub_chunks.len() as u16;
                self.chunk.sub_chunks.push(sub);
                let n = self.name_idx(name);
                self.emit(OP_CLOSURE);
                self.emit_u16(n);
                self.emit_u16(subidx);
                self.emit(OP_DEFINE_VAR);
                self.emit_u16(n);
            }
            Stmt::Return(e) => {
                self.compile_expr(e)?;
                self.emit(OP_RETURN);
            }
            Stmt::Block(body) => {
                for b in body {
                    self.compile_stmt(b)?;
                }
            }
            Stmt::Break => {
                let j = self.jump(OP_JUMP);
                if let Some(info) = self.loops.last_mut() {
                    info.breaks.push(j);
                }
            }
            Stmt::Continue => {
                if let Some(info) = self.loops.last() {
                    self.jump_back(OP_JUMP, info.continue_target);
                }
            }
            Stmt::Class { name, .. } => {
                return Err(ForgeError::Runtime {
                    message: format!("vm-unsupported:class {}", name),
                    line: 0,
                });
            }
            Stmt::Import(path) => {
                return Err(ForgeError::Runtime {
                    message: format!("vm-unsupported:import {}", path),
                    line: 0,
                });
            }
        }
        Ok(())
    }
}
