use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::chunk::*;
use crate::env::Env;
use crate::error::ForgeError;
use crate::eval::{install_natives, Evaluator};
use crate::value::{BoundMethod, Closure, Method, Value};

pub struct Vm {
    pub globals: Rc<RefCell<Env>>,
}

impl Vm {
    pub fn new() -> Self {
        let globals = Env::new();
        install_natives(&globals);
        Self { globals }
    }

    pub fn interpret(
        &mut self,
        chunk: &Rc<Chunk>,
        env: &Rc<RefCell<Env>>,
    ) -> Result<Value, ForgeError> {
        let mut stack: Vec<Value> = Vec::new();
        self.run(chunk, env, &mut stack)
    }

    fn run(
        &mut self,
        chunk: &Rc<Chunk>,
        env: &Rc<RefCell<Env>>,
        stack: &mut Vec<Value>,
    ) -> Result<Value, ForgeError> {
        let code = &chunk.code;
        let constants = &chunk.constants;
        let mut ip: usize = 0;

        fn read_u16(code: &[u8], ip: &mut usize) -> u16 {
            let lo = code[*ip] as u16;
            let hi = code[*ip + 1] as u16;
            *ip += 2;
            lo | (hi << 8)
        }

        loop {
            if ip >= code.len() {
                return Err(self.runtime("bytecode ran off the end"));
            }
            let op = code[ip];
            ip += 1;
            match op {
                OP_CONST => {
                    let i = read_u16(code, &mut ip) as usize;
                    stack.push(constants[i].clone());
                }
                OP_NULL => stack.push(Value::Null),
                OP_POP => {
                    stack.pop();
                }
                OP_ADD | OP_SUB | OP_MUL | OP_DIV | OP_MOD | OP_EQ | OP_NE | OP_LT | OP_LE
                | OP_GT | OP_GE => {
                    let right = stack.pop().unwrap();
                    let left = stack.pop().unwrap();
                    let v = self.binary(op, left, right)?;
                    stack.push(v);
                }
                OP_NEG => {
                    let v = stack.pop().unwrap();
                    let n = self.as_number(v)?;
                    stack.push(Value::Number(-n));
                }
                OP_NOT => {
                    let v = stack.pop().unwrap();
                    stack.push(Value::Bool(!v.truthy()));
                }
                OP_JUMP => {
                    let off = read_u16(code, &mut ip) as i16 as i32;
                    ip = (ip as i32 + off) as usize;
                }
                OP_JUMP_IF_FALSE => {
                    let off = read_u16(code, &mut ip) as i16 as i32;
                    let v = stack.pop().unwrap();
                    if !v.truthy() {
                        ip = (ip as i32 + off) as usize;
                    }
                }
                OP_JUMP_IF_TRUE => {
                    let off = read_u16(code, &mut ip) as i16 as i32;
                    let v = stack.pop().unwrap();
                    if v.truthy() {
                        ip = (ip as i32 + off) as usize;
                    }
                }
                OP_GET_VAR => {
                    let i = read_u16(code, &mut ip) as usize;
                    let name = constants[i].render();
                    let v = Env::get(env, &name)
                        .ok_or_else(|| self.runtime(&format!("Undefined variable '{}'", name)))?;
                    stack.push(v);
                }
                OP_SET_VAR => {
                    let i = read_u16(code, &mut ip) as usize;
                    let name = constants[i].render();
                    let v = stack.pop().unwrap();
                    if !Env::assign(env, &name, v.clone()) {
                        Env::define(env, &name, v);
                    }
                }
                OP_DEFINE_VAR => {
                    let i = read_u16(code, &mut ip) as usize;
                    let name = constants[i].render();
                    let v = stack.pop().unwrap();
                    Env::define(env, &name, v);
                }
                OP_CALL => {
                    let argc = code[ip] as usize;
                    ip += 1;
                    let mut args = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.insert(0, stack.pop().unwrap());
                    }
                    let callee = stack.pop().unwrap();
                    let v = self.call_value(callee, args)?;
                    stack.push(v);
                }
                OP_RETURN => {
                    return Ok(stack.pop().unwrap_or(Value::Null));
                }
                OP_LIST => {
                    let count = read_u16(code, &mut ip) as usize;
                    let mut items = Vec::with_capacity(count);
                    for _ in 0..count {
                        items.insert(0, stack.pop().unwrap());
                    }
                    stack.push(Value::List(Rc::new(RefCell::new(items))));
                }
                OP_MAP => {
                    let count = read_u16(code, &mut ip) as usize;
                    let mut map = HashMap::new();
                    for _ in 0..count {
                        let value = stack.pop().unwrap();
                        let key = stack.pop().unwrap();
                        map.insert(key.render(), value);
                    }
                    stack.push(Value::Map(Rc::new(RefCell::new(map))));
                }
                OP_INDEX => {
                    let idx = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    let v = self.index_val(obj, idx)?;
                    stack.push(v);
                }
                OP_SET_INDEX => {
                    let value = stack.pop().unwrap();
                    let idx = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    self.set_index(obj, idx, value)?;
                }
                OP_GET_PROP => {
                    let i = read_u16(code, &mut ip) as usize;
                    let name = constants[i].render();
                    let obj = stack.pop().unwrap();
                    let v = self.get_field(obj, &name)?;
                    stack.push(v);
                }
                OP_SET_PROP => {
                    let i = read_u16(code, &mut ip) as usize;
                    let name = constants[i].render();
                    let value = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    self.set_prop(obj, &name, value)?;
                }
                OP_LEN => {
                    let v = stack.pop().unwrap();
                    let n = match &v {
                        Value::Str(s) => s.chars().count() as f64,
                        Value::List(l) => l.borrow().len() as f64,
                        Value::Map(m) => m.borrow().len() as f64,
                        _ => return Err(self.runtime("len expects a string, list, or map")),
                    };
                    stack.push(Value::Number(n));
                }
                OP_ITERABLE => {
                    let v = stack.pop().unwrap();
                    let list = match v {
                        Value::List(l) => l.borrow().clone(),
                        Value::Str(s) => {
                            s.chars().map(|c| Value::Str(c.to_string())).collect()
                        }
                        Value::Map(m) => {
                            m.borrow().keys().cloned().map(Value::Str).collect()
                        }
                        _ => return Err(self.runtime("Cannot iterate over this value")),
                    };
                    stack.push(Value::List(Rc::new(RefCell::new(list))));
                }
                OP_CLOSURE => {
                    let name_i = read_u16(code, &mut ip) as usize;
                    let subidx = read_u16(code, &mut ip) as usize;
                    let sub = chunk.sub_chunks[subidx].clone();
                    let name = constants[name_i].render();
                    let params = sub.params.clone();
                    stack.push(Value::Closure(Rc::new(Closure {
                        name,
                        params,
                        chunk: sub,
                        closure: env.clone(),
                    })));
                }
                _ => return Err(self.runtime("unknown opcode")),
            }
        }
    }

    fn binary(&self, op: u8, left: Value, right: Value) -> Result<Value, ForgeError> {
        match op {
            OP_ADD => match (left, right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                (Value::List(a), Value::List(b)) => {
                    a.borrow_mut().extend(b.borrow().iter().cloned());
                    Ok(Value::List(a))
                }
                _ => Err(self.runtime("Operands must be numbers or strings for +")),
            },
            OP_SUB => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Number(a - b))
            }
            OP_MUL => match (left, right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
                (Value::Str(s), Value::Number(n)) => Ok(Value::Str(s.repeat(n as usize))),
                (Value::Number(n), Value::Str(s)) => Ok(Value::Str(s.repeat(n as usize))),
                _ => Err(self.runtime("Operands must be numbers or string*int")),
            },
            OP_DIV => {
                let (a, b) = self.nums(left, right)?;
                if b == 0.0 {
                    return Err(self.runtime("Division by zero"));
                }
                Ok(Value::Number(a / b))
            }
            OP_MOD => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Number(a % b))
            }
            OP_EQ => Ok(Value::Bool(values_equal(&left, &right))),
            OP_NE => Ok(Value::Bool(!values_equal(&left, &right))),
            OP_LT => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Bool(a < b))
            }
            OP_LE => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Bool(a <= b))
            }
            OP_GT => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Bool(a > b))
            }
            OP_GE => {
                let (a, b) = self.nums(left, right)?;
                Ok(Value::Bool(a >= b))
            }
            _ => Err(self.runtime("bad arithmetic opcode")),
        }
    }

    fn nums(&self, left: Value, right: Value) -> Result<(f64, f64), ForgeError> {
        match (left, right) {
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

    fn set_index(&self, obj: Value, idx: Value, value: Value) -> Result<(), ForgeError> {
        match obj {
            Value::Map(m) => {
                m.borrow_mut().insert(idx.render(), value);
                Ok(())
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
                l[i as usize] = value;
                Ok(())
            }
            _ => Err(self.runtime("Cannot assign to index of this value")),
        }
    }

    fn get_field(&self, obj: Value, name: &str) -> Result<Value, ForgeError> {
        match obj {
            Value::Str(_) | Value::List(_) => Ok(Value::Method(Rc::new(Method {
                name: name.to_string(),
                receiver: obj,
            }))),
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

    fn set_prop(&self, obj: Value, name: &str, value: Value) -> Result<(), ForgeError> {
        match obj {
            Value::Object(o) => {
                o.borrow_mut().fields.insert(name.to_string(), value);
                Ok(())
            }
            _ => Err(self.runtime("Can only set properties on objects")),
        }
    }

    fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, ForgeError> {
        match callee {
            Value::Native(f) => f(&args).map_err(|m| self.runtime(&m)),
            Value::Closure(f) => {
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
                self.interpret(&f.chunk, &env)
            }
            Value::BoundMethod(m) => {
                let mut e = Evaluator::new();
                e.call_external(Value::BoundMethod(m), args)
            }
            Value::Method(m) => {
                call_method_dispatch(&m.receiver, &m.name, &args).map_err(|e| self.runtime(&e))
            }
            Value::Class(klass) => {
                let mut e = Evaluator::new();
                e.call_external(Value::Class(klass), args)
            }
            _ => Err(self.runtime("Can only call functions")),
        }
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

fn call_method_dispatch(receiver: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
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

