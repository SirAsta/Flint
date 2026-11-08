pub const OP_CONST: u8 = 0;
pub const OP_POP: u8 = 1;
pub const OP_ADD: u8 = 2;
pub const OP_SUB: u8 = 3;
pub const OP_MUL: u8 = 4;
pub const OP_DIV: u8 = 5;
pub const OP_MOD: u8 = 6;
pub const OP_NEG: u8 = 7;
pub const OP_NOT: u8 = 8;
pub const OP_EQ: u8 = 9;
pub const OP_NE: u8 = 10;
pub const OP_LT: u8 = 11;
pub const OP_LE: u8 = 12;
pub const OP_GT: u8 = 13;
pub const OP_GE: u8 = 14;
pub const OP_JUMP: u8 = 15;
pub const OP_JUMP_IF_FALSE: u8 = 16;
pub const OP_GET_VAR: u8 = 17;
pub const OP_SET_VAR: u8 = 18;
pub const OP_DEFINE_VAR: u8 = 19;
pub const OP_CALL: u8 = 20;
pub const OP_RETURN: u8 = 21;
pub const OP_LIST: u8 = 22;
pub const OP_MAP: u8 = 23;
pub const OP_INDEX: u8 = 24;
pub const OP_SET_INDEX: u8 = 25;
pub const OP_GET_PROP: u8 = 26;
pub const OP_SET_PROP: u8 = 27;
pub const OP_CLOSURE: u8 = 28;
pub const OP_LEN: u8 = 29;
pub const OP_NULL: u8 = 30;
pub const OP_JUMP_IF_TRUE: u8 = 31;
pub const OP_ITERABLE: u8 = 32;

use std::rc::Rc;

use crate::value::Value;

#[derive(Clone)]
pub struct Chunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub sub_chunks: Vec<Rc<Chunk>>,
    pub params: Vec<String>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            sub_chunks: Vec::new(),
            params: Vec::new(),
        }
    }

    pub fn add_constant(&mut self, v: Value) -> u16 {
        self.constants.push(v);
        (self.constants.len() - 1) as u16
    }
}
