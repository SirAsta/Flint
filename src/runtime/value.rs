use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::Stmt;
use crate::chunk::Chunk;
use crate::env::Env;

pub type NativeFn = fn(&[Value]) -> Result<Value, String>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    Str(String),
    Bool(bool),
    Null,
    List(Rc<RefCell<Vec<Value>>>),
    Function(FunctionObj),
    Native(NativeFn),
    Map(Rc<RefCell<HashMap<String, Value>>>),
    Object(Rc<RefCell<Obj>>),
    Class(Rc<ClassDef>),
    BoundMethod(Rc<BoundMethod>),
    Method(Rc<Method>),
    Closure(Rc<Closure>),
}

#[derive(Clone)]
pub struct Obj {
    pub klass: Rc<ClassDef>,
    pub fields: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct ClassDef {
    pub name: String,
    pub methods: HashMap<String, (Vec<String>, Rc<Vec<Stmt>>)>,
    pub closure: Rc<RefCell<Env>>,
}

#[derive(Clone)]
pub struct BoundMethod {
    pub name: String,
    pub params: Vec<String>,
    pub body: Rc<Vec<Stmt>>,
    pub receiver: Rc<RefCell<Obj>>,
}

#[derive(Clone)]
pub struct Method {
    pub name: String,
    pub receiver: Value,
}

#[derive(Clone)]
pub struct Closure {
    pub name: String,
    pub params: Vec<String>,
    pub chunk: Rc<Chunk>,
    pub closure: Rc<RefCell<Env>>,
}

#[derive(Clone)]
pub struct FunctionObj {
    pub name: String,
    pub params: Vec<String>,
    pub body: Rc<Vec<Stmt>>,
    pub closure: Rc<RefCell<Env>>,
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.render())
    }
}

impl Value {
    pub fn render(&self) -> String {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::List(items) => {
                let items: Vec<String> = items.borrow().iter().map(|v| v.render()).collect();
                format!("[{}]", items.join(", "))
            }
            Value::Function(f) => format!("<fn {}>", f.name),
            Value::Native(_) => "<native fn>".to_string(),
            Value::Map(m) => {
                let m = m.borrow();
                let parts: Vec<String> = m.iter()
                    .map(|(k, v)| format!("{}: {}", k, v.render()))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::Object(o) => format!("<instance {}>", o.borrow().klass.name),
            Value::Class(c) => format!("<class {}>", c.name),
            Value::BoundMethod(m) => format!("<method {}>", m.name),
            Value::Method(m) => format!("<method {}>", m.name),
            Value::Closure(f) => format!("<fn {}>", f.name),
        }
    }

    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            _ => true,
        }
    }
}
