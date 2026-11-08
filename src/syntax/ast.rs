#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    Str(String),
    Bool(bool),
    Null,
    Variable(String),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
    Logical { left: Box<Expr>, op: LogicalOp, right: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    List(Vec<Expr>),
    Index { object: Box<Expr>, index: Box<Expr> },
    Map(Vec<(Expr, Expr)>),
    Get { object: Box<Expr>, name: String },
}

#[derive(Clone, Copy, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    EqEq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy, Debug)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Expression(Expr),
    Var { name: String, initializer: Option<Expr> },
    Assign { name: String, value: Expr },
    Set { object: Box<Expr>, name: String, value: Box<Expr> },
    SetIndex { object: Box<Expr>, index: Box<Expr>, value: Box<Expr> },
    If { branches: Vec<(Expr, Vec<Stmt>)>, else_branch: Option<Vec<Stmt>> },
    While { condition: Expr, body: Vec<Stmt> },
    For { variable: String, iterable: Expr, body: Vec<Stmt> },
    Import(String),
    Function { name: String, params: Vec<String>, body: Vec<Stmt> },
    Class { name: String, methods: Vec<(String, Vec<String>, Vec<Stmt>)> },
    Return(Expr),
    Block(Vec<Stmt>),
    Break,
    Continue,
}
