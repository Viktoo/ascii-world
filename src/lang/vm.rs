//! CPU evaluator: the checked IR compiled to a small register bytecode.
//!
//! Used for probing, collision, placement, describe and the CPU fallback
//! renderer. Every program is verified after compilation (register indices and
//! jump targets in range), so execution cannot index out of bounds, and every
//! instruction burns fuel, so execution always terminates.

use super::api::{self, Api, Ty};
use super::ir::*;

pub const MAX_REGS: usize = 2048;
/// Hard per-call instruction limit (second line of defence after the checker).
pub const HARD_FUEL: u32 = 200_000;

#[derive(Clone, Debug)]
enum Op {
    Const { dst: u16, v: f32 },
    Mov { dst: u16, src: u16 },
    Bin { op: BinOp, dst: u16, a: u16, b: u16 },
    Neg { dst: u16, a: u16 },
    Not { dst: u16, a: u16 },
    Truthy { dst: u16, a: u16 },
    Cmp { op: CmpOp, dst: u16, a: u16, b: u16 },
    Logic { and: bool, dst: u16, a: u16, b: u16 },
    Select { dst: u16, c: u16, a: u16, b: u16 },
    Call { api: Api, dst: u16, args: u16 },
    /// dst = start + f32(ctr) * step
    LoopVar { dst: u16, ctr: u16, start: f32, step: f32 },
    /// if ctr >= count { jump } (ctr is a register holding an integer-valued float)
    LoopTest { ctr: u16, count: u32, to: u32 },
    Inc { ctr: u16 },
    Jmp { to: u32 },
    Jz { c: u16, to: u32 },
    Ret { src: u16, width: u8 },
    /// Behaviour: queue an effect with the value of register `arg`.
    Effect { effect: Effect, arg: u16 },
    /// Behaviour: finish normally.
    End,
    Trap,
}

#[derive(Clone, Debug)]
pub struct Program {
    ops: Vec<Op>,
    nregs: usize,
    /// Behaviour programs: number of property registers per side (self, other).
    nprops: u16,
}

/// Inputs and outputs of one behaviour call.
pub struct BehaviorIo<'a> {
    pub k: &'a [f32; 16],
    pub state: &'a mut [f32; 8],
    pub ctx: &'a [f32; CTX_LEN],
    /// The thing's own properties, in `Module::prop_names` order.
    pub props: &'a mut [f32],
    /// The other thing's properties (zeros when there is none).
    pub other: &'a mut [f32],
    pub effects: &'a mut Vec<(Effect, f32)>,
}

/// At most this many effects are kept per call.
pub const MAX_EFFECTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmError {
    OutOfFuel,
    NoReturn,
}

pub const REG_X: u16 = 0;
pub const REG_K: u16 = 3;
pub const K_LEN: usize = 16;
pub const CTX_LEN: usize = CTX_FIELDS.len();
const SHAPE_FIRST_LOCAL: u16 = REG_K + K_LEN as u16;
pub const REG_STATE: u16 = REG_K + K_LEN as u16;
pub const REG_CTX: u16 = REG_STATE + 8;
pub const REG_PROPS: u16 = REG_CTX + CTX_LEN as u16;

struct Compiler<'f> {
    f: &'f Func,
    nprops: u16,
    ops: Vec<Op>,
    local_reg: Vec<u16>,
    next: u16,
    hwm: u16,
    overflow: bool,
}

impl<'f> Compiler<'f> {
    fn alloc(&mut self, n: u16) -> u16 {
        let r = self.next;
        if (self.next as usize) + (n as usize) >= MAX_REGS {
            self.overflow = true;
            return 0;
        }
        self.next += n;
        self.hwm = self.hwm.max(self.next);
        r
    }

    fn expr(&mut self, e: &Expr) -> u16 {
        match &e.kind {
            ExprKind::Num(v) => {
                let d = self.alloc(1);
                self.ops.push(Op::Const { dst: d, v: *v });
                d
            }
            ExprKind::Local(id) => self.local_reg.get(*id as usize).copied().unwrap_or(0),
            ExprKind::Param(i) => REG_X + *i as u16,
            ExprKind::KField(i) => REG_K + (*i as u16).min(K_LEN as u16 - 1),
            ExprKind::State(i) => REG_STATE + (*i as u16).min(7),
            ExprKind::Ctx(i) => REG_CTX + (*i as u16).min(CTX_LEN as u16 - 1),
            ExprKind::Prop { other, idx } => {
                if *idx >= self.nprops {
                    self.overflow = true;
                    return 0;
                }
                REG_PROPS + if *other { self.nprops } else { 0 } + *idx
            }
            ExprKind::Comp(v, c) => self.expr(v) + *c as u16,
            ExprKind::Neg(a) => {
                let w = e.ty.width();
                let ra = self.expr(a);
                let d = self.alloc(w);
                for i in 0..w {
                    self.ops.push(Op::Neg { dst: d + i, a: ra + i });
                }
                d
            }
            ExprKind::Not(a) => {
                let ra = self.expr(a);
                let d = self.alloc(1);
                self.ops.push(Op::Not { dst: d, a: ra });
                d
            }
            ExprKind::Truthy(a) => {
                let ra = self.expr(a);
                let d = self.alloc(1);
                self.ops.push(Op::Truthy { dst: d, a: ra });
                d
            }
            ExprKind::Bin(op, a, b) => {
                let w = e.ty.width();
                let ra = self.expr(a);
                let rb = self.expr(b);
                let d = self.alloc(w);
                let sa = a.ty == Ty::V;
                let sb = b.ty == Ty::V;
                for i in 0..w {
                    self.ops.push(Op::Bin {
                        op: *op,
                        dst: d + i,
                        a: ra + if sa { i } else { 0 },
                        b: rb + if sb { i } else { 0 },
                    });
                }
                d
            }
            ExprKind::Cmp(op, a, b) => {
                let ra = self.expr(a);
                let rb = self.expr(b);
                let d = self.alloc(1);
                self.ops.push(Op::Cmp { op: *op, dst: d, a: ra, b: rb });
                d
            }
            ExprKind::Logic(and, a, b) => {
                let ra = self.expr(a);
                let rb = self.expr(b);
                let d = self.alloc(1);
                self.ops.push(Op::Logic { and: *and, dst: d, a: ra, b: rb });
                d
            }
            ExprKind::Cond(c, a, b) => {
                let w = e.ty.width();
                let rc = self.expr(c);
                let ra = self.expr(a);
                let rb = self.expr(b);
                let d = self.alloc(w);
                for i in 0..w {
                    self.ops.push(Op::Select { dst: d + i, c: rc, a: ra + i, b: rb + i });
                }
                d
            }
            ExprKind::Call(api, args) => {
                let regs: Vec<(u16, u16)> = args.iter().map(|a| (self.expr(a), a.ty.width())).collect();
                let total: u16 = regs.iter().map(|r| r.1).sum();
                let base = self.alloc(total.max(1));
                let mut o = 0;
                for (r, w) in regs {
                    for i in 0..w {
                        self.ops.push(Op::Mov { dst: base + o, src: r + i });
                        o += 1;
                    }
                }
                let d = self.alloc(e.ty.width());
                self.ops.push(Op::Call { api: *api, dst: d, args: base });
                d
            }
        }
    }

    fn assign(&mut self, id: u32, e: &Expr) {
        let dst = self.local_reg.get(id as usize).copied().unwrap_or(0);
        self.store(dst, e);
    }

    fn store(&mut self, dst: u16, e: &Expr) {
        let mark = self.next;
        let r = self.expr(e);
        for i in 0..e.ty.width() {
            self.ops.push(Op::Mov { dst: dst + i, src: r + i });
        }
        self.next = mark;
    }

    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            match s {
                Stmt::Let { id, init } => self.assign(*id, init),
                Stmt::Assign { id, value } => self.assign(*id, value),
                Stmt::SetState { idx, value } => self.store(REG_STATE + (*idx as u16).min(7), value),
                Stmt::SetProp { other, idx, value } => {
                    if *idx >= self.nprops {
                        self.overflow = true;
                        continue;
                    }
                    self.store(REG_PROPS + if *other { self.nprops } else { 0 } + *idx, value)
                }
                Stmt::Effect { effect, arg } => {
                    let mark = self.next;
                    let r = match arg {
                        Some(a) => self.expr(a),
                        None => {
                            let d = self.alloc(1);
                            self.ops.push(Op::Const { dst: d, v: 0.0 });
                            d
                        }
                    };
                    self.ops.push(Op::Effect { effect: *effect, arg: r });
                    self.next = mark;
                }
                Stmt::End => self.ops.push(Op::End),
                Stmt::Return(e) => {
                    let mark = self.next;
                    let r = self.expr(e);
                    self.ops.push(Op::Ret { src: r, width: e.ty.width() as u8 });
                    self.next = mark;
                }
                Stmt::If { cond, then, els } => {
                    let mark = self.next;
                    let c = self.expr(cond);
                    self.next = mark;
                    let jz = self.ops.len();
                    self.ops.push(Op::Jz { c, to: 0 });
                    self.block(then);
                    let jmp = self.ops.len();
                    self.ops.push(Op::Jmp { to: 0 });
                    let else_at = self.ops.len() as u32;
                    self.block(els);
                    let end = self.ops.len() as u32;
                    self.ops[jz] = Op::Jz { c, to: else_at };
                    self.ops[jmp] = Op::Jmp { to: end };
                }
                Stmt::For { id, start, step, count, body } => {
                    let ctr = self.alloc(1);
                    self.ops.push(Op::Const { dst: ctr, v: 0.0 });
                    let top = self.ops.len();
                    self.ops.push(Op::LoopTest { ctr, count: *count, to: 0 });
                    let var = self.local_reg.get(*id as usize).copied().unwrap_or(0);
                    self.ops.push(Op::LoopVar { dst: var, ctr, start: *start, step: *step });
                    self.block(body);
                    self.ops.push(Op::Inc { ctr });
                    self.ops.push(Op::Jmp { to: top as u32 });
                    let end = self.ops.len() as u32;
                    self.ops[top] = Op::LoopTest { ctr, count: *count, to: end };
                }
            }
        }
    }
}

pub fn compile(f: &Func) -> Result<Program, String> {
    build(f, SHAPE_FIRST_LOCAL, 0, Op::Trap)
}

/// Compile a behaviour function that uses `nprops` property names.
pub fn compile_behavior(f: &Func, nprops: usize) -> Result<Program, String> {
    let n = nprops.min(MAX_PROPS) as u16;
    build(f, REG_PROPS + 2 * n, n, Op::End)
}

fn build(f: &Func, first_local: u16, nprops: u16, tail: Op) -> Result<Program, String> {
    let mut c = Compiler { f, nprops, ops: Vec::new(), local_reg: Vec::new(), next: first_local, hwm: first_local, overflow: false };
    for l in &c.f.locals.clone() {
        let r = c.alloc(l.ty.width());
        c.local_reg.push(r);
    }
    let body = c.f.body.clone();
    c.block(&body);
    c.ops.push(tail);
    if c.overflow {
        return Err("function is too complex (register limit)".into());
    }
    let p = Program { ops: c.ops, nregs: c.hwm as usize + 1, nprops };
    p.verify()?;
    Ok(p)
}

impl Program {
    /// Check every register index and jump target, so `run` cannot go out of bounds.
    fn verify(&self) -> Result<(), String> {
        let n = self.nregs;
        let len = self.ops.len() as u32;
        let r = |x: u16, w: u16| (x as usize) + (w as usize) <= n;
        let ok = self.ops.iter().all(|op| match *op {
            Op::Const { dst, .. } => r(dst, 1),
            Op::Mov { dst, src } => r(dst, 1) && r(src, 1),
            Op::Bin { dst, a, b, .. } | Op::Cmp { dst, a, b, .. } | Op::Logic { dst, a, b, .. } => r(dst, 1) && r(a, 1) && r(b, 1),
            Op::Neg { dst, a } | Op::Not { dst, a } | Op::Truthy { dst, a } => r(dst, 1) && r(a, 1),
            Op::Select { dst, c, a, b } => r(dst, 1) && r(c, 1) && r(a, 1) && r(b, 1),
            Op::Call { api, dst, args } => r(dst, api::ret_ty(api).width()) && r(args, api::arg_width(api) as u16),
            Op::LoopVar { dst, ctr, .. } => r(dst, 1) && r(ctr, 1),
            Op::LoopTest { ctr, to, .. } => r(ctr, 1) && to <= len,
            Op::Inc { ctr } => r(ctr, 1),
            Op::Jmp { to } => to <= len,
            Op::Jz { c, to } => r(c, 1) && to <= len,
            Op::Ret { src, width } => r(src, width as u16),
            Op::Effect { arg, .. } => r(arg, 1),
            Op::End | Op::Trap => true,
        });
        if ok { Ok(()) } else { Err("internal: bytecode failed verification".into()) }
    }

    /// Run with inputs. Returns (result, fuel used).
    pub fn run(&self, regs: &mut Vec<f32>, p: [f32; 3], k: &[f32; 16], fuel: u32) -> Result<([f32; 3], u32), VmError> {
        if regs.len() < self.nregs {
            regs.resize(self.nregs, 0.0);
        }
        let r = &mut regs[..self.nregs];
        r[0] = p[0];
        r[1] = p[1];
        r[2] = p[2];
        r[REG_K as usize..REG_K as usize + K_LEN].copy_from_slice(k);
        self.exec(r, fuel, None)
    }

    /// Run a behaviour function. State and properties are updated in place;
    /// effects are appended. Returns the fuel used.
    pub fn run_behavior(&self, regs: &mut Vec<f32>, io: BehaviorIo, fuel: u32) -> Result<u32, VmError> {
        let need = self.nregs.max(REG_PROPS as usize + 2 * self.nprops as usize);
        if regs.len() < need {
            regs.resize(need, 0.0);
        }
        let r = &mut regs[..need];
        r[..3].fill(0.0);
        r[REG_K as usize..REG_K as usize + K_LEN].copy_from_slice(io.k);
        r[REG_STATE as usize..REG_STATE as usize + 8].copy_from_slice(io.state);
        r[REG_CTX as usize..REG_CTX as usize + CTX_LEN].copy_from_slice(io.ctx);
        let n = self.nprops as usize;
        let pb = REG_PROPS as usize;
        for i in 0..n {
            r[pb + i] = io.props.get(i).copied().unwrap_or(0.0);
            r[pb + n + i] = io.other.get(i).copied().unwrap_or(0.0);
        }
        let (_, used) = self.exec(r, fuel, Some(io.effects))?;
        io.state.copy_from_slice(&r[REG_STATE as usize..REG_STATE as usize + 8]);
        for i in 0..n {
            if let Some(v) = io.props.get_mut(i) {
                *v = r[pb + i];
            }
            if let Some(v) = io.other.get_mut(i) {
                *v = r[pb + n + i];
            }
        }
        Ok(used)
    }

    fn exec(&self, regs: &mut [f32], fuel: u32, mut effects: Option<&mut Vec<(Effect, f32)>>) -> Result<([f32; 3], u32), VmError> {
        let mut pc = 0usize;
        let mut used = 0u32;
        let fuel = fuel.min(HARD_FUEL);
        let mut out = [0.0f32; 3];
        let b2f = |b: bool| if b { 1.0 } else { 0.0 };
        loop {
            let Some(op) = self.ops.get(pc) else { return Err(VmError::NoReturn) };
            used += 1;
            if used > fuel {
                return Err(VmError::OutOfFuel);
            }
            pc += 1;
            match *op {
                Op::Const { dst, v } => regs[dst as usize] = v,
                Op::Mov { dst, src } => regs[dst as usize] = regs[src as usize],
                Op::Bin { op, dst, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[dst as usize] = match op {
                        BinOp::Add => x + y,
                        BinOp::Sub => x - y,
                        BinOp::Mul => x * y,
                        BinOp::Div => x / y,
                        BinOp::Rem => x % y,
                        BinOp::Pow => x.abs().max(1e-20).powf(y),
                    };
                }
                Op::Neg { dst, a } => regs[dst as usize] = -regs[a as usize],
                Op::Not { dst, a } => regs[dst as usize] = b2f(regs[a as usize] == 0.0),
                Op::Truthy { dst, a } => regs[dst as usize] = b2f(regs[a as usize] != 0.0),
                Op::Cmp { op, dst, a, b } => {
                    let (x, y) = (regs[a as usize], regs[b as usize]);
                    regs[dst as usize] = b2f(match op {
                        CmpOp::Lt => x < y,
                        CmpOp::Le => x <= y,
                        CmpOp::Gt => x > y,
                        CmpOp::Ge => x >= y,
                        CmpOp::Eq => x == y,
                        CmpOp::Ne => x != y,
                    });
                }
                Op::Logic { and, dst, a, b } => {
                    let (x, y) = (regs[a as usize] != 0.0, regs[b as usize] != 0.0);
                    regs[dst as usize] = b2f(if and { x && y } else { x || y });
                }
                Op::Select { dst, c, a, b } => {
                    regs[dst as usize] = if regs[c as usize] != 0.0 { regs[a as usize] } else { regs[b as usize] };
                }
                Op::Call { api, dst, args } => {
                    used += 3;
                    let w = api::arg_width(api);
                    let mut o = [0.0f32; 3];
                    let a = args as usize;
                    api::eval(api, &regs[a..a + w], &mut o);
                    let rw = api::ret_ty(api).width() as usize;
                    regs[dst as usize..dst as usize + rw].copy_from_slice(&o[..rw]);
                }
                Op::LoopVar { dst, ctr, start, step } => regs[dst as usize] = start + regs[ctr as usize] * step,
                Op::LoopTest { ctr, count, to } => {
                    if regs[ctr as usize] >= count as f32 {
                        pc = to as usize;
                    }
                }
                Op::Inc { ctr } => regs[ctr as usize] += 1.0,
                Op::Jmp { to } => pc = to as usize,
                Op::Jz { c, to } => {
                    if regs[c as usize] == 0.0 {
                        pc = to as usize;
                    }
                }
                Op::Ret { src, width } => {
                    let s = src as usize;
                    out[..width as usize].copy_from_slice(&regs[s..s + width as usize]);
                    return Ok((out, used));
                }
                Op::Effect { effect, arg } => {
                    if let Some(e) = effects.as_deref_mut() {
                        if e.len() < MAX_EFFECTS {
                            e.push((effect, regs[arg as usize]));
                        }
                    }
                }
                Op::End => return Ok((out, used)),
                Op::Trap => return Err(VmError::NoReturn),
            }
        }
    }

}
