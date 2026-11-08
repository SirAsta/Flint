use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, LogicalOp, Stmt, UnaryOp};
use crate::env::Env;
use crate::error::ForgeError;
use crate::ffi::{
    ffi_alloc_native, ffi_call_native, ffi_free_native, ffi_load_native, ffi_read_f64_native,
    ffi_read_i64_native, ffi_read_str_native, ffi_read_u8_native, ffi_write_f64_native,
    ffi_write_i64_native, ffi_write_str_native, ffi_write_u8_native,
};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::value::{BoundMethod, ClassDef, FunctionObj, Method, Obj, Value};

#[derive(Debug)]
enum Flow {
    Normal,
    Return(Value),
    Break,
    Continue,
}

pub struct Evaluator {
    pub globals: Rc<RefCell<Env>>,
}

impl Evaluator {
    pub fn new() -> Self {
        let globals = Env::new();
        install_natives(&globals);
        Self { globals }
    }

    pub fn run(&mut self, stmts: &[Stmt]) -> Result<Value, ForgeError> {
        let globals = self.globals.clone();
        for s in stmts {
            match self.execute(s, &globals)? {
                Flow::Return(v) => return Ok(v),
                _ => {}
            }
        }
        Ok(Value::Null)
    }

    pub fn call_external(
        &mut self,
        callee: Value,
        args: Vec<Value>,
    ) -> Result<Value, ForgeError> {
        self.call(callee, args)
    }

    pub fn eval_expr(&mut self, expr: &Expr, env: &Rc<RefCell<Env>>) -> Result<Value, ForgeError> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Variable(name) => Env::get(env, name)
                .ok_or_else(|| self.runtime(&format!("Undefined variable '{}'", name))),
            Expr::Unary { op, expr } => {
                let v = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => Ok(Value::Number(-self.as_number(v)?)),
                    UnaryOp::Not => Ok(Value::Bool(!v.truthy())),
                }
            }
            Expr::Binary { left, op, right } => {
                let l = self.eval_expr(left, env)?;
                let r = self.eval_expr(right, env)?;
                self.binary(op, l, r)
            }
            Expr::Logical { left, op, right } => {
                let l = self.eval_expr(left, env)?;
                match op {
                    LogicalOp::And => {
                        if !l.truthy() { Ok(l) } else { self.eval_expr(right, env) }
                    }
                    LogicalOp::Or => {
                        if l.truthy() { Ok(l) } else { self.eval_expr(right, env) }
                    }
                }
            }
            Expr::List(items) => {
                let mut out = Vec::new();
                for it in items {
                    out.push(self.eval_expr(it, env)?);
                }
                Ok(Value::List(Rc::new(RefCell::new(out))))
            }
            Expr::Index { object, index } => {
                let obj = self.eval_expr(object, env)?;
                let idx = self.eval_expr(index, env)?;
                self.index_val(obj, idx)
            }
            Expr::Map(pairs) => {
                let mut map = HashMap::new();
                for (k, v) in pairs {
                    let key = self.eval_expr(k, env)?;
                    let val = self.eval_expr(v, env)?;
                    map.insert(key.render(), val);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }
            Expr::Get { object, name } => {
                let obj = self.eval_expr(object, env)?;
                self.get_field(obj, name)
            }
            Expr::Call { callee, args } => {
                let callee = self.eval_expr(callee, env)?;
                let mut vals = Vec::new();
                for a in args {
                    vals.push(self.eval_expr(a, env)?);
                }
                self.call(callee, vals)
            }
        }
    }

    fn index_val(&self, obj: Value, idx: Value) -> Result<Value, ForgeError> {
        match obj {
            Value::Map(m) => {
                let m = m.borrow();
                m.get(&idx.render()).cloned().ok_or_else(|| self.runtime("Key not found"))
            }
            Value::List(items) => {
                let mut i = self.as_int(idx)?;
                let items = items.borrow();
                if i < 0 {
                    i += items.len() as i64;
                }
                if i < 0 || i >= items.len() as i64 {
                    return Err(self.runtime("Index out of bounds"));
                }
                Ok(items[i as usize].clone())
            }
            Value::Str(s) => {
                let mut i = self.as_int(idx)?;
                let chars: Vec<char> = s.chars().collect();
                if i < 0 {
                    i += chars.len() as i64;
                }
                if i < 0 || i >= chars.len() as i64 {
                    return Err(self.runtime("Index out of bounds"));
                }
                Ok(Value::Str(chars[i as usize].to_string()))
            }
            _ => Err(self.runtime("Cannot index non-indexable value")),
        }
    }

    fn binary(&self, op: &BinaryOp, l: Value, r: Value) -> Result<Value, ForgeError> {
        use BinaryOp::*;
        match op {
            Add => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                (Value::List(a), Value::List(b)) => {
                    a.borrow_mut().extend(b.borrow().iter().cloned());
                    Ok(Value::List(a))
                }
                _ => Err(self.runtime("Operands must be numbers or strings for +")),
            },
            Sub => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Number(a - b))
            }
            Mul => match (l, r) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
                (Value::Str(s), Value::Number(n)) => Ok(Value::Str(s.repeat(n as usize))),
                (Value::Number(n), Value::Str(s)) => Ok(Value::Str(s.repeat(n as usize))),
                _ => Err(self.runtime("Operands must be numbers or string*int for *")),
            },
            Div => {
                let (a, b) = self.nums(l, r)?;
                if b == 0.0 {
                    return Err(self.runtime("Division by zero"));
                }
                Ok(Value::Number(a / b))
            }
            Mod => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Number(a % b))
            }
            EqEq => Ok(Value::Bool(values_equal(&l, &r))),
            Neq => Ok(Value::Bool(!values_equal(&l, &r))),
            Lt => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Bool(a < b))
            }
            Le => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Bool(a <= b))
            }
            Gt => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Bool(a > b))
            }
            Ge => {
                let (a, b) = self.nums(l, r)?;
                Ok(Value::Bool(a >= b))
            }
        }
    }

    fn nums(&self, l: Value, r: Value) -> Result<(f64, f64), ForgeError> {
        match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok((a, b)),
            _ => Err(self.runtime("Operands must be numbers")),
        }
    }

    fn as_number(&self, v: Value) -> Result<f64, ForgeError> {
        match v {
            Value::Number(n) => Ok(n),
            _ => Err(self.runtime("Expected a number")),
        }
    }

    fn as_int(&self, v: Value) -> Result<i64, ForgeError> {
        match v {
            Value::Number(n) => Ok(n as i64),
            _ => Err(self.runtime("Expected a number for index")),
        }
    }

    fn get_field(&self, obj: Value, name: &str) -> Result<Value, ForgeError> {
        match obj {
            Value::Str(_) | Value::List(_) => {
                Ok(Value::Method(Rc::new(Method {
                    name: name.to_string(),
                    receiver: obj,
                })))
            }
            Value::Object(o) => {
                if let Some(v) = o.borrow().fields.get(name).cloned() {
                    return Ok(v);
                }
                if let Some((params, body)) = o.borrow().klass.methods.get(name).cloned() {
                    return Ok(Value::BoundMethod(Rc::new(BoundMethod {
                        name: name.to_string(),
                        params,
                        body,
                        receiver: o.clone(),
                    })));
                }
                Err(self.runtime(&format!("Undefined property '{}'", name)))
            }
            _ => Err(self.runtime("Can only access properties on objects")),
        }
    }

    fn call(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, ForgeError> {
        match callee {
            Value::Native(f) => f(&args).map_err(|m| self.runtime(&m)),
            Value::Function(f) => {
                if args.len() != f.params.len() {
                    return Err(self.runtime(&format!(
                        "Expected {} arguments, got {}",
                        f.params.len(),
                        args.len()
                    )));
                }
                let env = Env::child(f.closure.clone());
                for (p, a) in f.params.iter().zip(args) {
                    Env::define(&env, p, a);
                }
                for s in f.body.iter() {
                    match self.execute(s, &env)? {
                        Flow::Return(v) => return Ok(v),
                        _ => {}
                    }
                }
                Ok(Value::Null)
            }
            Value::Class(klass) => {
                let obj = Rc::new(RefCell::new(Obj {
                    klass: klass.clone(),
                    fields: HashMap::new(),
                }));
                if let Some((params, body)) = klass.methods.get("init") {
                    if args.len() != params.len() {
                        return Err(self.runtime(&format!(
                            "Expected {} arguments, got {}",
                            params.len(),
                            args.len()
                        )));
                    }
                    let env = Env::child(klass.closure.clone());
                    Env::define(&env, "this", Value::Object(obj.clone()));
                    for (p, a) in params.iter().zip(args) {
                        Env::define(&env, p, a);
                    }
                    for s in body.iter() {
                        let _ = self.execute(s, &env)?;
                    }
                }
                Ok(Value::Object(obj))
            }
            Value::BoundMethod(m) => {
                if args.len() != m.params.len() {
                    return Err(self.runtime(&format!(
                        "Expected {} arguments, got {}",
                        m.params.len(),
                        args.len()
                    )));
                }
                let env = Env::child(m.receiver.borrow().klass.closure.clone());
                Env::define(&env, "this", Value::Object(m.receiver.clone()));
                for (p, a) in m.params.iter().zip(args) {
                    Env::define(&env, p, a);
                }
                for s in m.body.iter() {
                    match self.execute(s, &env)? {
                        Flow::Return(v) => return Ok(v),
                        _ => {}
                    }
                }
                Ok(Value::Null)
            }
            Value::Method(m) => {
                call_method(&m.receiver, &m.name, &args).map_err(|e| self.runtime(&e))
            }
            _ => Err(self.runtime("Can only call functions")),
        }
    }

    fn execute(&mut self, stmt: &Stmt, env: &Rc<RefCell<Env>>) -> Result<Flow, ForgeError> {
        match stmt {
            Stmt::Expression(e) => {
                self.eval_expr(e, env)?;
                Ok(Flow::Normal)
            }
            Stmt::Var { name, initializer } => {
                let value = match initializer {
                    Some(e) => self.eval_expr(e, env)?,
                    None => Value::Null,
                };
                Env::define(env, name, value);
                Ok(Flow::Normal)
            }
            Stmt::Assign { name, value } => {
                let v = self.eval_expr(value, env)?;
                if !Env::assign(env, name, v.clone()) {
                    Env::define(env, name, v);
                }
                Ok(Flow::Normal)
            }
            Stmt::If { branches, else_branch } => {
                for (cond, body) in branches {
                    if self.eval_expr(cond, env)?.truthy() {
                        return self.execute_block(body, env);
                    }
                }
                if let Some(body) = else_branch {
                    return self.execute_block(body, env);
                }
                Ok(Flow::Normal)
            }
            Stmt::While { condition, body } => {
                while self.eval_expr(condition, env)?.truthy() {
                    match self.execute_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For { variable, iterable, body } => {
                let iterable = self.eval_expr(iterable, env)?;
                let items = match iterable {
                    Value::List(l) => l.borrow().clone(),
                    Value::Str(s) => s.chars().map(|c| Value::Str(c.to_string())).collect(),
                    Value::Map(m) => {
                        m.borrow().keys().cloned().map(Value::Str).collect()
                    }
                    _ => return Err(self.runtime("Can only iterate over lists, strings, or maps")),
                };
                for item in items {
                    Env::define(env, variable, item);
                    match self.execute_block(body, env)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Function { name, params, body } => {
                let f = Value::Function(FunctionObj {
                    name: name.clone(),
                    params: params.clone(),
                    body: Rc::new(body.clone()),
                    closure: env.clone(),
                });
                Env::define(env, name, f);
                Ok(Flow::Normal)
            }
            Stmt::Set { object, name, value } => {
                let v = self.eval_expr(value, env)?;
                let obj = self.eval_expr(object, env)?;
                match obj {
                    Value::Object(o) => {
                        o.borrow_mut().fields.insert(name.clone(), v);
                    }
                    _ => return Err(self.runtime("Can only set properties on objects")),
                }
                Ok(Flow::Normal)
            }
            Stmt::Class { name, methods } => {
                let mut map = HashMap::new();
                for (mn, params, body) in methods {
                    map.insert(mn.clone(), (params.clone(), Rc::new(body.clone())));
                }
                let klass = Value::Class(Rc::new(ClassDef {
                    name: name.clone(),
                    methods: map,
                    closure: env.clone(),
                }));
                Env::define(env, name, klass);
                Ok(Flow::Normal)
            }
            Stmt::SetIndex { object, index, value } => {
                let v = self.eval_expr(value, env)?;
                let obj = self.eval_expr(object, env)?;
                let idx = self.eval_expr(index, env)?;
                match obj {
                    Value::Map(m) => {
                        m.borrow_mut().insert(idx.render(), v);
                    }
                    Value::List(l) => {
                        let mut i = self.as_int(idx)?;
                        let mut l = l.borrow_mut();
                        if i < 0 {
                            i += l.len() as i64;
                        }
                        if i < 0 || i >= l.len() as i64 {
                            return Err(self.runtime("Index out of bounds"));
                        }
                        l[i as usize] = v;
                    }
                    _ => return Err(self.runtime("Cannot assign to index of this value")),
                }
                Ok(Flow::Normal)
            }
            Stmt::Import(path) => {
                let source = std::fs::read_to_string(path).map_err(|e| {
                    self.runtime(&format!("cannot import {}: {}", path, e))
                })?;
                let tokens = Lexer::new(&source).scan().map_err(ForgeError::Scan)?;
                let mut p = Parser::new(tokens);
                let stmts = p.parse_program()?;
                for s in &stmts {
                    match self.execute(s, env)? {
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Return(e) => {
                let v = self.eval_expr(e, env)?;
                Ok(Flow::Return(v))
            }
            Stmt::Block(body) => self.execute_block(body, env),
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
        }
    }

    fn execute_block(&mut self, body: &[Stmt], env: &Rc<RefCell<Env>>) -> Result<Flow, ForgeError> {
        for s in body {
            match self.execute(s, env)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn runtime(&self, message: &str) -> ForgeError {
        ForgeError::Runtime {
            message: message.to_string(),
            line: 0,
        }
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Null, Value::Null) => true,
        (Value::List(x), Value::List(y)) => {
            let x = x.borrow();
            let y = y.borrow();
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        _ => false,
    }
}

pub(crate) fn install_natives(globals: &Rc<RefCell<Env>>) {
    Env::define(globals, "print", Value::Native(print_native));
    Env::define(globals, "len", Value::Native(len_native));
    Env::define(globals, "str", Value::Native(str_native));
    Env::define(globals, "int", Value::Native(int_native));
    Env::define(globals, "float", Value::Native(float_native));
    Env::define(globals, "range", Value::Native(range_native));
    Env::define(globals, "append", Value::Native(append_native));
    Env::define(globals, "pop", Value::Native(pop_native));
    Env::define(globals, "input", Value::Native(input_native));
    Env::define(globals, "keys", Value::Native(keys_native));
    Env::define(globals, "has", Value::Native(has_native));
    Env::define(globals, "ffi_load", Value::Native(ffi_load_native));
    Env::define(globals, "ffi_call", Value::Native(ffi_call_native));
    Env::define(globals, "ffi_alloc", Value::Native(ffi_alloc_native));
    Env::define(globals, "ffi_free", Value::Native(ffi_free_native));
    Env::define(globals, "ffi_write_i64", Value::Native(ffi_write_i64_native));
    Env::define(globals, "ffi_write_f64", Value::Native(ffi_write_f64_native));
    Env::define(globals, "ffi_write_u8", Value::Native(ffi_write_u8_native));
    Env::define(globals, "ffi_write_str", Value::Native(ffi_write_str_native));
    Env::define(globals, "ffi_read_i64", Value::Native(ffi_read_i64_native));
    Env::define(globals, "ffi_read_f64", Value::Native(ffi_read_f64_native));
    Env::define(globals, "ffi_read_u8", Value::Native(ffi_read_u8_native));
    Env::define(globals, "ffi_read_str", Value::Native(ffi_read_str_native));
}

fn print_native(args: &[Value]) -> Result<Value, String> {
    let line: Vec<String> = args.iter().map(|a| a.render()).collect();
    println!("{}", line.join(" "));
    Ok(Value::Null)
}

fn len_native(args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("len expects 1 argument")?;
    match v {
        Value::Str(s) => Ok(Value::Number(s.chars().count() as f64)),
        Value::List(l) => Ok(Value::Number(l.borrow().len() as f64)),
        _ => Err("len expects a string or list".to_string()),
    }
}

fn str_native(args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("str expects 1 argument")?;
    Ok(Value::Str(v.render()))
}

fn int_native(args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("int expects 1 argument")?;
    match v {
        Value::Number(n) => Ok(Value::Number(n.trunc())),
        Value::Str(s) => s.trim()
            .parse::<i64>()
            .map(|i| Value::Number(i as f64))
            .map_err(|_| format!("Cannot convert '{}' to int", s)),
        _ => Err("int expects a number or string".to_string()),
    }
}

fn float_native(args: &[Value]) -> Result<Value, String> {
    let v = args.first().ok_or("float expects 1 argument")?;
    match v {
        Value::Number(n) => Ok(Value::Number(*n)),
        Value::Str(s) => s.trim()
            .parse::<f64>()
            .map(Value::Number)
            .map_err(|_| format!("Cannot convert '{}' to float", s)),
        _ => Err("float expects a number or string".to_string()),
    }
}

fn as_num(v: &Value) -> Result<f64, String> {
    match v {
        Value::Number(n) => Ok(*n),
        _ => Err("expected a number".to_string()),
    }
}

fn range_native(args: &[Value]) -> Result<Value, String> {
    let (start, stop, step) = match args.len() {
        1 => (0.0, as_num(&args[0])?, 1.0),
        2 => (as_num(&args[0])?, as_num(&args[1])?, 1.0),
        3 => (as_num(&args[0])?, as_num(&args[1])?, as_num(&args[2])?),
        _ => return Err("range expects 1-3 arguments".to_string()),
    };
    if step == 0.0 {
        return Err("range step cannot be zero".to_string());
    }
    let mut out = Vec::new();
    let mut i = start;
    if step > 0.0 {
        while i < stop {
            out.push(Value::Number(i));
            i += step;
        }
    } else {
        while i > stop {
            out.push(Value::Number(i));
            i += step;
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn append_native(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("append expects 2 arguments".to_string());
    }
    match &args[0] {
        Value::List(l) => {
            l.borrow_mut().push(args[1].clone());
            Ok(Value::Null)
        }
        _ => Err("append expects a list".to_string()),
    }
}

fn pop_native(args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err("pop expects 1 argument".to_string());
    }
    match &args[0] {
        Value::List(l) => {
            let mut l = l.borrow_mut();
            l.pop().ok_or_else(|| "pop on empty list".to_string())
        }
        _ => Err("pop expects a list".to_string()),
    }
}

fn input_native(_args: &[Value]) -> Result<Value, String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(Value::Str(line.trim_end().to_string()))
}

fn keys_native(args: &[Value]) -> Result<Value, String> {
    match args.first() {
        Some(Value::Map(m)) => {
            let keys: Vec<Value> = m.borrow().keys().cloned().map(Value::Str).collect();
            Ok(Value::List(Rc::new(RefCell::new(keys))))
        }
        _ => Err("keys expects a map".to_string()),
    }
}

fn has_native(args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("has expects 2 arguments".to_string());
    }
    match &args[0] {
        Value::Map(m) => Ok(Value::Bool(m.borrow().contains_key(&args[1].render()))),
        _ => Err("has expects a map".to_string()),
    }
}

fn call_method(receiver: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
    match receiver {
        Value::Str(s) => match name {
            "upper" => Ok(Value::Str(s.to_uppercase())),
            "lower" => Ok(Value::Str(s.to_lowercase())),
            "strip" => Ok(Value::Str(s.trim().to_string())),
            "split" => {
                let sep = match args.first() {
                    Some(Value::Str(sep)) => sep.clone(),
                    _ => " ".to_string(),
                };
                let parts: Vec<Value> = s
                    .split(&sep)
                    .map(|p| Value::Str(p.to_string()))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(parts))))
            }
            "replace" => {
                if args.len() != 2 {
                    return Err("replace expects 2 arguments".to_string());
                }
                Ok(Value::Str(s.replace(&args[0].render(), &args[1].render())))
            }
            "len" => Ok(Value::Number(s.chars().count() as f64)),
            _ => Err(format!("unknown string method '{}'", name)),
        },
        Value::List(l) => match name {
            "append" => {
                if args.len() != 1 {
                    return Err("append expects 1 argument".to_string());
                }
                l.borrow_mut().push(args[0].clone());
                Ok(Value::Null)
            }
            "pop" => l.borrow_mut().pop().ok_or_else(|| "pop on empty list".to_string()),
            "join" => {
                let sep = match args.first() {
                    Some(Value::Str(sep)) => sep.clone(),
                    _ => ", ".to_string(),
                };
                let parts: Vec<String> = l.borrow().iter().map(|v| v.render()).collect();
                Ok(Value::Str(parts.join(&sep)))
            }
            "len" => Ok(Value::Number(l.borrow().len() as f64)),
            _ => Err(format!("unknown list method '{}'", name)),
        },
        _ => Err("method call on unsupported value".to_string()),
    }
}
