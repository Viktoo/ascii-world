//! Typed IR produced by the allowlist checker. Both translators consume this;
//! nothing downstream ever sees the JS AST.

use super::api::{Api, Ty};

/// Instance parameter fields readable as `k.<name>`.
pub const K_FIELDS: [&str; 8] = ["seed", "scale", "a", "b", "c", "d", "e", "f"];

pub fn k_field(name: &str) -> Option<u8> {
    K_FIELDS.iter().position(|f| *f == name).map(|i| i as u8)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Num(f32),
    Local(u32),
    /// 0 = x, 1 = y, 2 = z
    Param(u8),
    KField(u8),
    /// Component 0..2 of a vec3 expression.
    Comp(Box<Expr>, u8),
    /// Arithmetic negation (number or vec3).
    Neg(Box<Expr>),
    Not(Box<Expr>),
    /// Number → boolean (`x != 0`), JS truthiness for conditions.
    Truthy(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    /// true = &&, false = ||
    Logic(bool, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Api, Vec<Expr>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let { id: u32, init: Expr },
    Assign { id: u32, value: Expr },
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    /// `for (let i = start; i < end; i += step)`, unrolled count known statically.
    For { id: u32, start: f32, step: f32, count: u32, body: Vec<Stmt> },
    Return(Expr),
}

#[derive(Clone, Debug)]
pub struct Local {
    pub name: String,
    pub ty: Ty,
    pub mutable: bool,
}

#[derive(Clone, Debug)]
pub struct Func {
    pub locals: Vec<Local>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Meta {
    pub name: String,
    pub bounds: [f32; 3],
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub meta: Meta,
    pub sdf: Func,
    pub color: Func,
}
