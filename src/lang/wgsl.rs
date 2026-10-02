//! IR → WGSL. Each type becomes `sdf_T{id}` and `color_T{id}`; the shader
//! assembler substitutes `{id}` and generates the dispatch switch.

use super::api::{self, Ty};
use super::ir::*;
use std::fmt::Write;

pub const ID: &str = "__TID__";

pub fn emit(m: &Module) -> String {
    let mut s = String::new();
    func(&mut s, &m.sdf, "sdf", "f32");
    func(&mut s, &m.color, "color", "vec3f");
    s
}

fn func(s: &mut String, f: &Func, name: &str, ret: &str) {
    let _ = writeln!(s, "fn {name}_T{ID}(p: vec3f, k: Params) -> {ret} {{");
    block(s, f, &f.body, 1);
    s.push_str("}\n");
}

fn ty(t: Ty) -> &'static str {
    match t {
        Ty::F => "f32",
        Ty::B => "bool",
        Ty::V => "vec3f",
    }
}

fn indent(s: &mut String, d: usize) {
    for _ in 0..d {
        s.push_str("  ");
    }
}

fn local(f: &Func, id: u32) -> String {
    let n = f.locals.get(id as usize).map(|l| l.name.as_str()).unwrap_or("");
    format!("l{id}_{n}")
}

fn block(s: &mut String, f: &Func, stmts: &[Stmt], d: usize) {
    for st in stmts {
        indent(s, d);
        match st {
            Stmt::Let { id, init } => {
                let l = &f.locals[*id as usize];
                let kw = if l.mutable { "var" } else { "let" };
                let _ = writeln!(s, "{kw} {}: {} = {};", local(f, *id), ty(l.ty), expr(f, init));
            }
            Stmt::Assign { id, value } => {
                let _ = writeln!(s, "{} = {};", local(f, *id), expr(f, value));
            }
            Stmt::Return(e) => {
                let _ = writeln!(s, "return {};", expr(f, e));
            }
            // Behaviour-only statements never reach shape functions (the checker
            // only allows them in tick/use/touch, which run on the CPU).
            Stmt::SetState { .. } | Stmt::SetProp { .. } | Stmt::Effect { .. } | Stmt::End => {
                s.push_str("// (behaviour statement)\n");
            }
            Stmt::If { cond, then, els } => {
                let _ = writeln!(s, "if ({}) {{", expr(f, cond));
                block(s, f, then, d + 1);
                if !els.is_empty() {
                    indent(s, d);
                    s.push_str("} else {\n");
                    block(s, f, els, d + 1);
                }
                indent(s, d);
                s.push_str("}\n");
            }
            Stmt::For { id, start, step, count, body } => {
                let c = format!("c{id}");
                let _ = writeln!(s, "for (var {c}: u32 = 0u; {c} < {count}u; {c} = {c} + 1u) {{");
                indent(s, d + 1);
                let _ = writeln!(s, "let {}: f32 = {} + f32({c}) * {};", local(f, *id), num(*start), num(*step));
                block(s, f, body, d + 1);
                indent(s, d);
                s.push_str("}\n");
            }
        }
    }
}

pub fn num(v: f32) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    let mut t = format!("{v:?}");
    if !t.contains('.') && !t.contains('e') && !t.contains("inf") && !t.contains("NaN") {
        t.push_str(".0");
    }
    if v < 0.0 { format!("({t})") } else { t }
}

fn expr(f: &Func, e: &Expr) -> String {
    match &e.kind {
        ExprKind::Num(v) => {
            if e.ty == Ty::B {
                if *v != 0.0 { "true".into() } else { "false".into() }
            } else {
                format!("f32({})", num(*v))
            }
        }
        ExprKind::Local(id) => local(f, *id),
        ExprKind::Param(i) => ["p.x", "p.y", "p.z"][*i as usize % 3].into(),
        ExprKind::KField(i) => format!("k.{}", K_FIELDS[*i as usize % K_FIELDS.len()]),
        ExprKind::State(_) | ExprKind::Ctx(_) | ExprKind::Prop { .. } => "f32(0.0)".into(),
        ExprKind::Comp(v, c) => format!("({}).{}", expr(f, v), ["x", "y", "z"][*c as usize % 3]),
        ExprKind::Neg(a) => format!("(-{})", expr(f, a)),
        ExprKind::Not(a) => format!("(!{})", expr(f, a)),
        ExprKind::Truthy(a) => format!("({} != 0.0)", expr(f, a)),
        ExprKind::Bin(op, a, b) => {
            let (x, y) = (expr(f, a), expr(f, b));
            match op {
                BinOp::Add => format!("({x} + {y})"),
                BinOp::Sub => format!("({x} - {y})"),
                BinOp::Mul => format!("({x} * {y})"),
                BinOp::Div => format!("({x} / {y})"),
                BinOp::Rem => format!("({x} % {y})"),
                BinOp::Pow => format!("api_pow({x}, {y})"),
            }
        }
        ExprKind::Cmp(op, a, b) => {
            let o = match op {
                CmpOp::Lt => "<",
                CmpOp::Le => "<=",
                CmpOp::Gt => ">",
                CmpOp::Ge => ">=",
                CmpOp::Eq => "==",
                CmpOp::Ne => "!=",
            };
            format!("({} {o} {})", expr(f, a), expr(f, b))
        }
        ExprKind::Logic(and, a, b) => format!("({} {} {})", expr(f, a), if *and { "&&" } else { "||" }, expr(f, b)),
        ExprKind::Cond(c, a, b) => format!("select({}, {}, {})", expr(f, b), expr(f, a), expr(f, c)),
        ExprKind::Call(a, args) => {
            let parts: Vec<String> = args.iter().map(|x| expr(f, x)).collect();
            format!("{}({})", api::wgsl_name(*a), parts.join(", "))
        }
    }
}
