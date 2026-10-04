//! Rules: data, not code. Each rule reads and changes properties, never
//! names: "anything that burns and is hot enough catches fire". The built-in
//! rules are the world's physics; a universe's bible may add more (a magic
//! world where `cursed` spreads), but cannot remove these.
//!
//! A rule is `{ "name", "near"?, "when", "do": [...] }`. Without `near` it
//! applies to each thing on its own (`self`). With `near: r` it applies to
//! each pair within r metres (`self` and `other`, `dist` between them).

use super::props::*;
use serde::{Deserialize, Serialize};

/// Names rules use for themselves; universe properties may not take them.
pub const RESERVED: &[&str] = &["self", "other", "dt", "dist", "hour", "night", "water", "held", "ground", "and", "or", "not", "min", "max", "clamp", "abs", "true", "false"];
pub const MAX_NEAR: f32 = 10.0;
pub const MAX_UNIVERSE_RULES: usize = 12;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleSpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub near: Option<f32>,
    pub when: String,
    #[serde(rename = "do")]
    pub effects: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Dt,
    Dist,
    Hour,
    Night,
    Water,
    Held,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    Min,
    Max,
    Clamp,
    Abs,
}

#[derive(Clone, Debug)]
pub enum RExpr {
    Num(f32),
    Prop { other: bool, id: usize },
    Var(Var),
    Neg(Box<RExpr>),
    Not(Box<RExpr>),
    Bin(Op, Box<RExpr>, Box<RExpr>),
    Call(Func, Vec<RExpr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assign {
    Set,
    Add,
    Sub,
    Mul,
}

#[derive(Clone, Debug)]
pub struct Effect {
    pub other: bool,
    pub prop: usize,
    pub op: Assign,
    pub value: RExpr,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub spec: RuleSpec,
    pub near: Option<f32>,
    /// Conjuncts of `when` that only read `self` (checked before looking for neighbours).
    pub when_self: Vec<RExpr>,
    /// The remaining conjuncts (read `other` or `dist`).
    pub when_pair: Vec<RExpr>,
    pub effects: Vec<Effect>,
    pub builtin: bool,
}

/// Everything an expression can read.
pub struct Env<'a> {
    pub me: &'a [f32],
    pub other: &'a [f32],
    pub dt: f32,
    pub dist: f32,
    pub hour: f32,
    pub night: f32,
    pub water: f32,
    pub held: f32,
    pub ground: f32,
}

impl RExpr {
    pub fn eval(&self, e: &Env) -> f32 {
        match self {
            RExpr::Num(v) => *v,
            RExpr::Prop { other, id } => {
                let src = if *other { e.other } else { e.me };
                src.get(*id).copied().unwrap_or(0.0)
            }
            RExpr::Var(v) => match v {
                Var::Dt => e.dt,
                Var::Dist => e.dist,
                Var::Hour => e.hour,
                Var::Night => e.night,
                Var::Water => e.water,
                Var::Held => e.held,
                Var::Ground => e.ground,
            },
            RExpr::Neg(a) => -a.eval(e),
            RExpr::Not(a) => (a.eval(e) == 0.0) as u32 as f32,
            RExpr::Bin(op, a, b) => {
                let x = a.eval(e);
                // Short-circuit logic.
                match op {
                    Op::And => return ((x != 0.0) && b.eval(e) != 0.0) as u32 as f32,
                    Op::Or => return ((x != 0.0) || b.eval(e) != 0.0) as u32 as f32,
                    _ => {}
                }
                let y = b.eval(e);
                let t = |c: bool| c as u32 as f32;
                match op {
                    Op::Add => x + y,
                    Op::Sub => x - y,
                    Op::Mul => x * y,
                    Op::Div => {
                        if y.abs() < 1e-9 {
                            0.0
                        } else {
                            x / y
                        }
                    }
                    Op::Lt => t(x < y),
                    Op::Le => t(x <= y),
                    Op::Gt => t(x > y),
                    Op::Ge => t(x >= y),
                    Op::Eq => t((x - y).abs() < 1e-6),
                    Op::Ne => t((x - y).abs() >= 1e-6),
                    Op::And | Op::Or => unreachable!(),
                }
            }
            RExpr::Call(f, args) => {
                let a = |i: usize| args.get(i).map(|x| x.eval(e)).unwrap_or(0.0);
                match f {
                    Func::Min => a(0).min(a(1)),
                    Func::Max => a(0).max(a(1)),
                    Func::Clamp => a(0).max(a(1)).min(a(2)),
                    Func::Abs => a(0).abs(),
                }
            }
        }
    }

    fn reads_pair(&self) -> bool {
        match self {
            RExpr::Prop { other, .. } => *other,
            RExpr::Var(Var::Dist) => true,
            RExpr::Num(_) | RExpr::Var(_) => false,
            RExpr::Neg(a) | RExpr::Not(a) => a.reads_pair(),
            RExpr::Bin(_, a, b) => a.reads_pair() || b.reads_pair(),
            RExpr::Call(_, args) => args.iter().any(|a| a.reads_pair()),
        }
    }

    /// Split a condition into its top-level `&&` parts.
    fn conjuncts(self, out: &mut Vec<RExpr>) {
        match self {
            RExpr::Bin(Op::And, a, b) => {
                a.conjuncts(out);
                b.conjuncts(out);
            }
            other => out.push(other),
        }
    }
}

// ---------------------------------------------------------------- parsing

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f32),
    Ident(String),
    Sym(&'static str),
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    const SYMS: &[&str] = &["&&", "||", "<=", ">=", "==", "!=", "+=", "-=", "*=", "<", ">", "!", "+", "-", "*", "/", "(", ")", ",", ".", "="];
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let st = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.' || b[i] == 'e' || ((b[i] == '-' || b[i] == '+') && i > st && b[i - 1] == 'e')) {
                i += 1;
            }
            let t: String = b[st..i].iter().collect();
            out.push(Tok::Num(t.parse::<f32>().map_err(|_| format!("bad number '{t}'"))?));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(b[st..i].iter().collect()));
            continue;
        }
        let rest: String = b[i..(i + 2).min(b.len())].iter().collect();
        match SYMS.iter().find(|s| rest.starts_with(**s)) {
            Some(s) => {
                out.push(Tok::Sym(s));
                i += s.len();
            }
            None => return Err(format!("unexpected '{c}'")),
        }
    }
    Ok(out)
}

struct Parser<'v> {
    toks: Vec<Tok>,
    i: usize,
    vocab: &'v Vocab,
    pair: bool,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn sym(&mut self, s: &str) -> bool {
        if self.peek() == Some(&Tok::Sym(match s {
            "&&" => "&&",
            "||" => "||",
            "<=" => "<=",
            ">=" => ">=",
            "==" => "==",
            "!=" => "!=",
            "<" => "<",
            ">" => ">",
            "!" => "!",
            "+" => "+",
            "-" => "-",
            "*" => "*",
            "/" => "/",
            "(" => "(",
            ")" => ")",
            "," => ",",
            "." => ".",
            _ => return false,
        })) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn word(&mut self, w: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Ident(x)) if x == w) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.sym(s) { Ok(()) } else { Err(format!("expected '{s}'")) }
    }

    fn or(&mut self) -> Result<RExpr, String> {
        let mut a = self.and()?;
        while self.sym("||") || self.word("or") {
            let b = self.and()?;
            a = RExpr::Bin(Op::Or, Box::new(a), Box::new(b));
        }
        Ok(a)
    }
    fn and(&mut self) -> Result<RExpr, String> {
        let mut a = self.cmp()?;
        while self.sym("&&") || self.word("and") {
            let b = self.cmp()?;
            a = RExpr::Bin(Op::And, Box::new(a), Box::new(b));
        }
        Ok(a)
    }
    fn cmp(&mut self) -> Result<RExpr, String> {
        let a = self.add()?;
        for (s, op) in [("<=", Op::Le), (">=", Op::Ge), ("==", Op::Eq), ("!=", Op::Ne), ("<", Op::Lt), (">", Op::Gt)] {
            if self.sym(s) {
                let b = self.add()?;
                return Ok(RExpr::Bin(op, Box::new(a), Box::new(b)));
            }
        }
        Ok(a)
    }
    fn add(&mut self) -> Result<RExpr, String> {
        let mut a = self.mul()?;
        loop {
            let op = if self.sym("+") {
                Op::Add
            } else if self.sym("-") {
                Op::Sub
            } else {
                break;
            };
            let b = self.mul()?;
            a = RExpr::Bin(op, Box::new(a), Box::new(b));
        }
        Ok(a)
    }
    fn mul(&mut self) -> Result<RExpr, String> {
        let mut a = self.unary()?;
        loop {
            let op = if self.sym("*") {
                Op::Mul
            } else if self.sym("/") {
                Op::Div
            } else {
                break;
            };
            let b = self.unary()?;
            a = RExpr::Bin(op, Box::new(a), Box::new(b));
        }
        Ok(a)
    }
    fn unary(&mut self) -> Result<RExpr, String> {
        if self.sym("-") {
            return Ok(RExpr::Neg(Box::new(self.unary()?)));
        }
        if self.sym("!") || self.word("not") {
            return Ok(RExpr::Not(Box::new(self.unary()?)));
        }
        self.primary()
    }
    fn primary(&mut self) -> Result<RExpr, String> {
        if self.sym("(") {
            let e = self.or()?;
            self.expect(")")?;
            return Ok(e);
        }
        match self.toks.get(self.i).cloned() {
            Some(Tok::Num(v)) => {
                self.i += 1;
                Ok(RExpr::Num(v))
            }
            Some(Tok::Ident(w)) => {
                self.i += 1;
                match w.as_str() {
                    "self" | "other" => {
                        if w == "other" && !self.pair {
                            return Err("'other' needs \"near\": the rule must be about pairs of things".into());
                        }
                        self.expect(".")?;
                        let Some(Tok::Ident(p)) = self.toks.get(self.i).cloned() else { return Err(format!("{w}. needs a property name")) };
                        self.i += 1;
                        let id = self.vocab.id(&p).ok_or_else(|| format!("unknown property '{p}'"))?;
                        Ok(RExpr::Prop { other: w == "other", id })
                    }
                    "true" => Ok(RExpr::Num(1.0)),
                    "false" => Ok(RExpr::Num(0.0)),
                    "dt" => Ok(RExpr::Var(Var::Dt)),
                    "dist" if self.pair => Ok(RExpr::Var(Var::Dist)),
                    "dist" => Err("'dist' needs \"near\"".into()),
                    "hour" => Ok(RExpr::Var(Var::Hour)),
                    "night" => Ok(RExpr::Var(Var::Night)),
                    "water" => Ok(RExpr::Var(Var::Water)),
                    "held" => Ok(RExpr::Var(Var::Held)),
                    "ground" => Ok(RExpr::Var(Var::Ground)),
                    "min" | "max" | "clamp" | "abs" => {
                        let f = match w.as_str() {
                            "min" => Func::Min,
                            "max" => Func::Max,
                            "clamp" => Func::Clamp,
                            _ => Func::Abs,
                        };
                        let n = match f {
                            Func::Abs => 1,
                            Func::Clamp => 3,
                            _ => 2,
                        };
                        self.expect("(")?;
                        let mut args = vec![self.or()?];
                        while self.sym(",") {
                            args.push(self.or()?);
                        }
                        self.expect(")")?;
                        if args.len() != n {
                            return Err(format!("{w}() takes {n} arguments"));
                        }
                        Ok(RExpr::Call(f, args))
                    }
                    _ => Err(format!("unknown name '{w}' (use self.<property>, other.<property>, dt, dist, hour, night, water, held, ground)")),
                }
            }
            Some(t) => Err(format!("unexpected {t:?}")),
            None => Err("unexpected end".into()),
        }
    }
}

pub fn parse_expr(s: &str, vocab: &Vocab, pair: bool) -> Result<RExpr, String> {
    let toks = lex(s)?;
    let mut p = Parser { toks, i: 0, vocab, pair };
    let e = p.or()?;
    if p.i != p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.i]));
    }
    Ok(e)
}

fn parse_effect(s: &str, vocab: &Vocab, pair: bool) -> Result<Effect, String> {
    let toks = lex(s)?;
    let (who, prop) = match (toks.first(), toks.get(1), toks.get(2)) {
        (Some(Tok::Ident(w)), Some(Tok::Sym(".")), Some(Tok::Ident(p))) if w == "self" || w == "other" => (w.clone(), p.clone()),
        _ => return Err("an effect looks like `self.temp += 10 * dt` or `other.wet = 1`".into()),
    };
    if who == "other" && !pair {
        return Err("'other' needs \"near\"".into());
    }
    let id = vocab.id(&prop).ok_or_else(|| format!("unknown property '{prop}'"))?;
    let op = match toks.get(3) {
        Some(Tok::Sym("=")) => Assign::Set,
        Some(Tok::Sym("+=")) => Assign::Add,
        Some(Tok::Sym("-=")) => Assign::Sub,
        Some(Tok::Sym("*=")) => Assign::Mul,
        _ => return Err("expected =, +=, -= or *= after the property".into()),
    };
    let mut p = Parser { toks: toks[4..].to_vec(), i: 0, vocab, pair };
    let value = p.or()?;
    if p.i != p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.i]));
    }
    Ok(Effect { other: who == "other", prop: id, op, value })
}

pub fn compile(spec: &RuleSpec, vocab: &Vocab, builtin: bool) -> Result<Rule, String> {
    let name = spec.name.trim();
    if name.is_empty() || name.len() > 60 {
        return Err("a rule needs a short name".into());
    }
    let near = match spec.near {
        Some(r) if !(r.is_finite() && r > 0.0 && r <= MAX_NEAR) => return Err(format!("{name}: near must be between 0 and {MAX_NEAR} metres")),
        n => n,
    };
    let pair = near.is_some();
    let when = parse_expr(&spec.when, vocab, pair).map_err(|e| format!("{name}: when: {e}"))?;
    if spec.effects.is_empty() || spec.effects.len() > 6 {
        return Err(format!("{name}: a rule needs 1 to 6 effects in \"do\""));
    }
    let mut effects = Vec::new();
    for e in &spec.effects {
        let fx = parse_effect(e, vocab, pair).map_err(|x| format!("{name}: do \"{e}\": {x}"))?;
        if !builtin && vocab.is_act(fx.prop) {
            return Err(format!("{name}: do \"{e}\": '{}' is what an action is doing right now; rules may read it but not set it", vocab.names[fx.prop]));
        }
        effects.push(fx);
    }
    let mut parts = Vec::new();
    when.conjuncts(&mut parts);
    let (when_pair, when_self): (Vec<RExpr>, Vec<RExpr>) = parts.into_iter().partition(|c| c.reads_pair());
    Ok(Rule { spec: spec.clone(), near, when_self, when_pair, effects, builtin })
}

fn spec(name: &str, near: Option<f32>, when: &str, effects: &[&str]) -> RuleSpec {
    RuleSpec { name: name.into(), near, when: when.into(), effects: effects.iter().map(|s| s.to_string()).collect() }
}

/// The world's physics. Tuned so fire crawls from tuft to tuft in seconds,
/// a stream stops it, and a warm lamp does not set the grass alight.
///
/// Heat and flame are apart. Heat warms what is around, less the further
/// off, and never to burning point: a kiln at 900° warms the yard and leaves
/// the grass by its wall alone. Only something on fire sets fire to what
/// burns near it.
pub fn builtin_specs() -> Vec<RuleSpec> {
    vec![
        spec("keeps its own heat", None, "self.heat > self.temp", &["self.temp = self.heat"]),
        spec("fire is hot", None, "self.fire > 0", &["self.temp = max(self.temp, 350 + 450 * self.fire)"]),
        spec("heat spreads", Some(6.0), "self.temp > 45 && other.temp < 10 + min(self.temp - 15, 135) / (1 + dist * dist)", &["other.temp += (15 + min(self.temp - 15, 135) / (1 + dist * dist) - other.temp) * 0.5 * dt"]),
        spec("flames spread", Some(6.0), "self.fire > 0 && other.burns > 0 && other.fire <= 0 && self.temp > other.temp + 30", &["other.temp += (self.temp - other.temp) * 0.3 * dt / (1 + dist * dist)"]),
        spec("cools down", None, "self.temp > self.heat && abs(self.temp - 15) > 0.5", &["self.temp += (15 - self.temp) * 0.05 * dt"]),
        spec("warms up", None, "self.temp < 14.5", &["self.temp += (15 - self.temp) * 0.05 * dt"]),
        spec("catches fire", None, "self.burns > 0 && self.fire <= 0 && self.fuel > 0 && self.wet < 0.4 && self.temp > 240 - 60 * self.burns", &["self.fire = 0.2"]),
        spec("burns", None, "self.fire > 0", &["self.fire += 0.3 * dt", "self.fuel -= 0.035 * self.fire * dt", "self.char += 0.04 * dt"]),
        spec("burns out", None, "self.fire > 0 && self.fuel <= 0", &["self.fire = 0", "self.char = 1", "self.burns = 0", "self.alive = 0"]),
        spec("beaten out", Some(0.5), "self.fire > 0 && other.force > 0", &["self.fire -= 0.8 * other.force * dt", "self.temp = min(self.temp, 150)"]),
        spec("doused", None, "self.fire > 0 && self.wet > 0.5", &["self.fire = 0", "self.temp = min(self.temp, 60)"]),
        spec("soaked in water", None, "water > 0", &["self.wet = 1"]),
        spec("dries", None, "self.wet > 0 && water <= 0", &["self.wet -= (0.004 + max(0, self.temp - 30) * 0.0004) * dt"]),
        spec("wets what it touches", Some(1.2), "other.wet > 0.8 && self.wet < other.wet - 0.1 && water <= 0", &["self.wet += 0.25 * dt"]),
        spec("grows", None, "self.alive > 0 && self.growth < 1 && self.fire <= 0 && held <= 0", &["self.growth += (0.002 + 0.006 * self.wet) * dt"]),
        spec("killed by heat", None, "self.alive > 0 && self.temp > 120", &["self.alive = 0"]),
        spec("broken open, it burns", None, "self.health <= 0 && self.burns > 0 && self.fuel > 0 && self.fire <= 0 && self.temp > 90", &["self.fire = 1"]),
        spec("conducts heat", Some(0.8), "self.conducts > 0 && other.conducts > 0 && self.temp > other.temp + 5", &["other.temp += (self.temp - other.temp) * self.conducts * other.conducts * dt"]),
    ]
}

pub fn builtin_rules(vocab: &Vocab) -> Vec<Rule> {
    builtin_specs().iter().map(|s| compile(s, vocab, true).expect("built-in rule")).collect()
}

/// One entity's view for the rule pass.
pub struct EntView<'a> {
    pub props: &'a [f32],
    pub water: f32,
    pub held: f32,
    pub ground: f32,
}

/// Apply one effect to a working copy.
pub fn apply(target: &mut [f32], prop: usize, op: Assign, v: f32) {
    let Some(x) = target.get_mut(prop) else { return };
    if !v.is_finite() {
        return;
    }
    match op {
        Assign::Set => *x = v,
        Assign::Add => *x += v,
        Assign::Sub => *x -= v,
        Assign::Mul => *x *= v,
    }
}

/// Evaluate all single-entity rules on `me`, writing into `out` (a copy of
/// `me.props`). Returns the names of rules that fired.
pub fn run_single<'r>(rules: &'r [Rule], me: &EntView, out: &mut [f32], dt: f32, hour: f32, night: f32, fired: &mut Vec<&'r str>) {
    for r in rules.iter().filter(|r| r.near.is_none()) {
        let env = Env { me: me.props, other: &[], dt, dist: 0.0, hour, night, water: me.water, held: me.held, ground: me.ground };
        if r.when_self.iter().all(|c| c.eval(&env) != 0.0) {
            for e in &r.effects {
                apply(out, e.prop, e.op, e.value.eval(&env));
            }
            fired.push(&r.spec.name);
        }
    }
}

/// Whether a pair rule's self-only conditions hold (so neighbours are worth checking).
pub fn pair_self_ok(r: &Rule, me: &EntView, dt: f32, hour: f32, night: f32) -> bool {
    let env = Env { me: me.props, other: &[], dt, dist: 0.0, hour, night, water: me.water, held: me.held, ground: me.ground };
    r.when_self.iter().all(|c| c.eval(&env) != 0.0)
}

/// Evaluate a pair rule for (me, other); effects go to `out_me` / `out_other`
/// as deltas (Set effects are applied directly). Returns true if it fired.
#[allow(clippy::too_many_arguments)]
pub fn run_pair(r: &Rule, me: &EntView, other: &EntView, dist: f32, dt: f32, hour: f32, night: f32, out_me: &mut [f32], out_other: &mut [f32]) -> bool {
    let env = Env { me: me.props, other: other.props, dt, dist, hour, night, water: me.water, held: me.held, ground: me.ground };
    if !r.when_pair.iter().all(|c| c.eval(&env) != 0.0) {
        return false;
    }
    for e in &r.effects {
        let v = e.value.eval(&env);
        apply(if e.other { &mut *out_other } else { &mut *out_me }, e.prop, e.op, v);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vocab {
        Vocab::builtin()
    }

    #[test]
    fn parses_and_evaluates() {
        let vo = v();
        let e = parse_expr("self.temp > 200 && !(self.wet >= 0.5) or other.fire * 2 == 1", &vo, true).unwrap();
        let mut me = vo.defaults.clone();
        let mut ot = vo.defaults.clone();
        me[P_TEMP] = 250.0;
        let env = |me: &[f32], ot: &[f32]| e.eval(&Env { me, other: ot, dt: 0.1, dist: 1.0, hour: 12.0, night: 0.0, water: 0.0, held: 0.0, ground: 1.0 });
        assert_eq!(env(&me, &ot), 1.0);
        me[P_WET] = 0.7;
        assert_eq!(env(&me, &ot), 0.0);
        ot[P_FIRE] = 0.5;
        assert_eq!(env(&me, &ot), 1.0);
        let e2 = parse_expr("clamp(-self.temp / 0, 1, 3) + min(2, abs(-5))", &vo, false).unwrap();
        assert_eq!(e2.eval(&Env { me: &me, other: &[], dt: 0.0, dist: 0.0, hour: 0.0, night: 0.0, water: 0.0, held: 0.0, ground: 0.0 }), 3.0);
    }

    #[test]
    fn rejects_bad_rules_with_reasons() {
        let vo = v();
        let bad = [
            (spec("x", None, "other.fire > 0", &["self.fire = 1"]), "needs \"near\""),
            (spec("x", None, "self.magic > 0", &["self.fire = 1"]), "unknown property 'magic'"),
            (spec("x", Some(50.0), "self.fire > 0", &["self.fire = 1"]), "near must be"),
            (spec("x", None, "self.fire > 0", &["fire = 1"]), "an effect looks like"),
            (spec("x", None, "self.fire >", &["self.fire = 1"]), "unexpected end"),
            (spec("x", None, "self.fire > 0", &[]), "1 to 6 effects"),
            (spec("x", None, "launch(1)", &["self.fire = 1"]), "unknown name 'launch'"),
            (spec("x", None, "self.fire > 0", &["self.force = 1"]), "may read it but not set it"),
        ];
        for (s, expect) in bad {
            let e = compile(&s, &vo, false).expect_err(&format!("{s:?}"));
            assert!(e.contains(expect), "{s:?}: {e}");
        }
    }

    #[test]
    fn builtin_rules_compile_and_split_conditions() {
        let vo = v();
        let rules = builtin_rules(&vo);
        let heat = rules.iter().find(|r| r.spec.name == "heat spreads").unwrap();
        assert_eq!(heat.near, Some(6.0));
        assert!(heat.when_self.len() == 1 && heat.when_pair.len() == 1, "cheap self check first, then neighbours");
        let wets = rules.iter().find(|r| r.spec.name == "wets what it touches").unwrap();
        assert_eq!((wets.when_self.len(), wets.when_pair.len()), (1, 2), "self-only parts are checked first");
    }

    #[test]
    fn universe_properties_work_in_rules() {
        let mut vo = v();
        vo.add("cursed", 0.0, "how cursed it is").unwrap();
        assert!(vo.add("dist", 0.0, "").is_err());
        assert!(vo.add("Bad Name", 0.0, "").is_err());
        let r = compile(&spec("curse spreads", Some(2.0), "self.cursed > 0.5 && other.cursed < self.cursed", &["other.cursed += 0.1 * dt"]), &vo, false).unwrap();
        let c = vo.id("cursed").unwrap();
        let mut a = vo.defaults.clone();
        a[c] = 1.0;
        let b = vo.defaults.clone();
        let mut oa = a.clone();
        let mut ob = b.clone();
        let va = EntView { props: &a, water: 0.0, held: 0.0, ground: 1.0 };
        let vb = EntView { props: &b, water: 0.0, held: 0.0, ground: 1.0 };
        assert!(pair_self_ok(&r, &va, 1.0, 12.0, 0.0));
        assert!(run_pair(&r, &va, &vb, 1.0, 1.0, 12.0, 0.0, &mut oa, &mut ob));
        assert!((ob[c] - 0.1).abs() < 1e-6);
    }
}
