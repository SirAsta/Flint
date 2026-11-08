use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::value::Value;

#[derive(Debug)]
pub struct Env {
    pub values: HashMap<String, Value>,
    pub parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn new() -> Rc<RefCell<Env>> {
        Rc::new(RefCell::new(Env { values: HashMap::new(), parent: None }))
    }

    pub fn child(parent: Rc<RefCell<Env>>) -> Rc<RefCell<Env>> {
        Rc::new(RefCell::new(Env { values: HashMap::new(), parent: Some(parent) }))
    }

    pub fn define(env: &Rc<RefCell<Env>>, name: &str, value: Value) {
        env.borrow_mut().values.insert(name.to_string(), value);
    }

    pub fn assign(env: &Rc<RefCell<Env>>, name: &str, value: Value) -> bool {
        let mut current = env.borrow_mut();
        if current.values.contains_key(name) {
            current.values.insert(name.to_string(), value);
            return true;
        }
        if let Some(parent) = current.parent.clone() {
            drop(current);
            return Self::assign(&parent, name, value);
        }
        false
    }

    pub fn get(env: &Rc<RefCell<Env>>, name: &str) -> Option<Value> {
        let current = env.borrow();
        if let Some(v) = current.values.get(name) {
            return Some(v.clone());
        }
        if let Some(parent) = &current.parent {
            return Self::get(parent, name);
        }
        None
    }
}
