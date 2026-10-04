//! Steps 1 and 2 of the pipeline: parse with oxc, then check every node
//! against the allowlist while lowering to the typed IR.
//!
//! Anything not explicitly handled here is rejected, so code the checker does
//! not understand can never reach a translator.

use super::api::{self, Resolved, Ty};
use super::ir::*;
use super::{Diag, Stage};
use oxc_allocator::Allocator;
use oxc_ast::ast::{self as js, Argument, BindingPattern, Expression, Statement};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator};

pub const MAX_SOURCE_BYTES: usize = 16 * 1024;
pub const MAX_LOOP_ITERS: u32 = 32;
const MAX_NESTED_ITERS: u32 = 256;
const MAX_DEPTH: u32 = 48;
const MAX_STMTS: u32 = 600;

pub fn line_of(src: &str, offset: u32) -> u32 {
    let end = (offset as usize).min(src.len());
    src.as_bytes()[..end].iter().filter(|b| **b == b'\n').count() as u32 + 1
}

/// Parse and allowlist-check a type module.
pub fn check(src: &str) -> Result<Module, Vec<Diag>> {
    if src.len() > MAX_SOURCE_BYTES {
        return Err(vec![Diag::new(Stage::Parse, 0, format!("source is {} bytes; the limit is {MAX_SOURCE_BYTES}", src.len()))]);
    }
    if let Some(line) = nesting_too_deep(src) {
        return Err(vec![Diag::new(Stage::Parse, line, "brackets nested too deeply".into())]);
    }
    let alloc = Allocator::default();
    let ret = Parser::new(&alloc, src, SourceType::mjs()).parse();
    if ret.diagnostics.has_errors() || ret.fatal_error {
        let mut out = Vec::new();
        for d in ret.diagnostics.errors() {
            let line = d.labels.first().map(|l| line_of(src, l.offset())).unwrap_or(0);
            out.push(Diag::new(Stage::Parse, line, format!("syntax error: {}", d.message)));
        }
        if out.is_empty() {
            out.push(Diag::new(Stage::Parse, 0, "syntax error".into()));
        }
        return Err(out);
    }
    let mut cx = Cx { src, diags: Vec::new(), prop_names: Vec::new() };
    let module = cx.program(&ret.program);
    match module {
        Some(m) if cx.diags.is_empty() => Ok(m),
        _ => {
            if cx.diags.is_empty() {
                cx.diags.push(Diag::new(Stage::Allowlist, 0, "module rejected".into()));
            }
            cx.diags.sort_by_key(|d| d.line);
            cx.diags.dedup_by(|a, b| a.line == b.line && a.msg == b.msg);
            Err(cx.diags)
        }
    }
}

fn nesting_too_deep(src: &str) -> Option<u32> {
    let mut depth = 0i32;
    let mut line = 1u32;
    for c in src.chars() {
        match c {
            '\n' => line += 1,
            '(' | '[' | '{' => {
                depth += 1;
                if depth > 64 {
                    return Some(line);
                }
            }
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

struct Cx<'s> {
    src: &'s str,
    diags: Vec<Diag>,
    /// Property names referenced by behaviour code, shared by all functions.
    prop_names: Vec<String>,
}

impl<'s> Cx<'s> {
    fn err(&mut self, span: Span, msg: impl Into<String>) {
        let line = line_of(self.src, span.start);
        self.diags.push(Diag::new(Stage::Allowlist, line, msg.into()));
    }

    fn program<'a>(&mut self, p: &js::Program<'a>) -> Option<Module> {
        let mut meta = None;
        let mut sdf_fn: Option<&js::Function<'a>> = None;
        let mut color_fn: Option<&js::Function<'a>> = None;
        let mut behavior_fns: Vec<(Behavior, &js::Function<'a>)> = Vec::new();
        let mut consts: Vec<(&'a str, &js::Expression<'a>, Span)> = Vec::new();

        for d in p.directives.iter() {
            self.err(d.span, "directives are not allowed");
        }
        for st in p.body.iter() {
            match st {
                Statement::ExportDeclaration(ed) => match &ed.declaration {
                    js::Declaration::FunctionDeclaration(f) => {
                        let name = f.id.as_ref().map(|i| i.name.as_str()).unwrap_or("");
                        match name {
                            "sdf" if sdf_fn.is_none() => sdf_fn = Some(f),
                            "color" if color_fn.is_none() => color_fn = Some(f),
                            "sdf" | "color" => self.err(f.span, format!("{name}() is defined twice")),
                            n => match Behavior::from_name(n) {
                                Some(b) if behavior_fns.iter().any(|(x, _)| *x == b) => self.err(f.span, format!("{name}() is defined twice")),
                                Some(b) => behavior_fns.push((b, f)),
                                None => self.err(f.span, format!("only sdf(), color(), tick(), use() and touch() may be exported, not '{name}'")),
                            },
                        }
                    }
                    js::Declaration::VariableDeclaration(vd) => {
                        for decl in vd.declarations.iter() {
                            let name = binding_name(&decl.id);
                            if name == Some("meta") {
                                if vd.kind != js::VariableDeclarationKind::Const {
                                    self.err(vd.span, "meta must be declared with const");
                                }
                                match &decl.init {
                                    Some(Expression::ObjectExpression(o)) => meta = self.meta(o),
                                    _ => self.err(decl.span, "meta must be an object literal"),
                                }
                            } else {
                                self.err(decl.span, "the only exported const allowed is meta");
                            }
                        }
                    }
                    other => self.err(other.span(), "only meta, sdf(), color(), tick(), use() and touch() may be exported"),
                },
                Statement::VariableDeclaration(vd) => {
                    if vd.kind != js::VariableDeclarationKind::Const {
                        self.err(vd.span, "top-level declarations must be const");
                        continue;
                    }
                    for decl in vd.declarations.iter() {
                        match (binding_name(&decl.id), &decl.init) {
                            (Some(n), Some(init)) => consts.push((n, init, decl.span)),
                            _ => self.err(decl.span, "top-level const needs a simple name and a value"),
                        }
                    }
                }
                Statement::FunctionDeclaration(f) => {
                    let name = f.id.as_ref().map(|i| i.name.as_str()).unwrap_or("?");
                    self.err(f.span, format!("helper function '{name}' is not allowed; only the exported sdf(), color(), tick(), use() and touch()"));
                }
                Statement::ImportDeclaration(s) => self.err(s.span, "import is not allowed"),
                Statement::EmptyStatement(_) => {}
                other => self.top_level_reject(other),
            }
        }

        let meta = match meta {
            Some(m) => Some(m),
            None => {
                if !self.diags.iter().any(|d| d.msg.contains("meta")) {
                    self.diags.push(Diag::new(Stage::Allowlist, 1, "missing `export const meta = { name, bounds, tags }`".into()));
                }
                None
            }
        };
        if sdf_fn.is_none() {
            self.diags.push(Diag::new(Stage::Allowlist, 1, "missing `export function sdf(x, y, z, k)`".into()));
        }
        if color_fn.is_none() {
            self.diags.push(Diag::new(Stage::Allowlist, 1, "missing `export function color(x, y, z, k)`".into()));
        }
        let lists = meta.as_ref().map(|m| [m.says.len(), m.sounds.len(), m.spawns.len()]);
        let sdf = sdf_fn.and_then(|f| self.function(f, "sdf", Role::Shape(Ty::F), &consts, lists));
        let color = color_fn.and_then(|f| self.function(f, "color", Role::Shape(Ty::V), &consts, lists));
        let mut behaviors = Vec::new();
        for (b, f) in behavior_fns {
            if let Some(func) = self.function(f, b.name(), Role::Behavior(b), &consts, lists) {
                behaviors.push((b, func));
            }
        }
        Some(Module { meta: meta?, sdf: sdf?, color: color?, behaviors, prop_names: std::mem::take(&mut self.prop_names) })
    }

    fn top_level_reject(&mut self, st: &Statement) {
        let msg = stmt_reject_reason(st).unwrap_or("this statement is not allowed at the top level");
        self.err(st.span(), msg);
    }

    fn meta(&mut self, o: &js::ObjectExpression) -> Option<Meta> {
        let mut name = None;
        let mut bounds = None;
        let mut tags = Vec::new();
        let mut props: Vec<(String, f32)> = Vec::new();
        let mut lists: [Vec<String>; 3] = Default::default();
        let mut body = None;
        let mut fits = None;
        for prop in o.properties.iter() {
            let js::ObjectPropertyKind::ObjectProperty(p) = prop else {
                self.err(o.span, "spread is not allowed in meta");
                continue;
            };
            if p.computed || p.method || p.kind != js::PropertyKind::Init {
                self.err(p.span, "meta entries must be plain `key: value` pairs");
                continue;
            }
            let key = match &p.key {
                js::PropertyKey::StaticIdentifier(id) => id.name.as_str().to_string(),
                js::PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                _ => {
                    self.err(p.span, "meta keys must be plain names");
                    continue;
                }
            };
            match key.as_str() {
                "name" => match &p.value {
                    Expression::StringLiteral(s) => name = Some(s.value.as_str().chars().take(48).collect::<String>()),
                    _ => self.err(p.span, "meta.name must be a string"),
                },
                "bounds" => {
                    let nums = self.literal_numbers(&p.value);
                    match nums {
                        Some(v) if v.len() == 3 && v.iter().all(|b| b.is_finite() && *b > 0.0 && *b <= 40.0) => {
                            bounds = Some([v[0], v[1], v[2]]);
                        }
                        _ => self.err(p.span, "meta.bounds must be [hx, hy, hz]: three half-extents in metres, each in (0, 40]"),
                    }
                }
                "tags" => match &p.value {
                    Expression::ArrayExpression(a) => {
                        for el in a.elements.iter() {
                            match el {
                                js::ArrayExpressionElement::StringLiteral(s) => {
                                    tags.push(s.value.as_str().chars().take(24).collect::<String>().to_lowercase())
                                }
                                _ => self.err(a.span, "meta.tags must be an array of strings"),
                            }
                        }
                    }
                    _ => self.err(p.span, "meta.tags must be an array of strings"),
                },
                "body" => match &p.value {
                    Expression::ObjectExpression(bo) => body = self.meta_body(bo),
                    _ => self.err(p.span, "meta.body must be an object"),
                },
                "fits" => match &p.value {
                    Expression::StringLiteral(s) => fits = Some(s.value.as_str().trim().to_lowercase().chars().take(48).collect::<String>()),
                    _ => self.err(p.span, "meta.fits must be the name of the body this layer is worn on"),
                },
                "props" => match &p.value {
                    Expression::ObjectExpression(po) => props = self.meta_props(po),
                    _ => self.err(p.span, "meta.props must be an object of numbers, e.g. { mass: 2, burns: 0.5 }"),
                },
                "says" | "sounds" | "spawns" => {
                    let slot = match key.as_str() {
                        "says" => 0,
                        "sounds" => 1,
                        _ => 2,
                    };
                    match &p.value {
                        Expression::ArrayExpression(a) => {
                            for el in a.elements.iter() {
                                match el {
                                    js::ArrayExpressionElement::StringLiteral(s) if lists[slot].len() < 8 => {
                                        lists[slot].push(s.value.as_str().chars().take(120).collect::<String>())
                                    }
                                    js::ArrayExpressionElement::StringLiteral(_) => {
                                        self.err(a.span, format!("meta.{key} may hold at most 8 entries"));
                                        break;
                                    }
                                    _ => self.err(a.span, format!("meta.{key} must be an array of strings")),
                                }
                            }
                        }
                        _ => self.err(p.span, format!("meta.{key} must be an array of strings")),
                    }
                }
                _ => {
                    // Unknown keys are tolerated if they are inert literals.
                    if !is_inert_literal(&p.value) {
                        self.err(p.span, format!("meta.{key} must be a literal string, number or array"));
                    }
                }
            }
        }
        let name = match name {
            Some(n) if !n.trim().is_empty() => n,
            _ => {
                self.err(o.span, "meta.name is required");
                return None;
            }
        };
        let Some(bounds) = bounds else {
            self.err(o.span, "meta.bounds is required");
            return None;
        };
        let [says, sounds, spawns] = lists;
        if let Some(b) = &body {
            let r = bounds[0].max(bounds[2]);
            if b.height > bounds[1] * 2.0 + 0.05 || b.radius > r * 1.5 + 0.05 {
                self.err(o.span, "meta.body.height and radius must fit inside meta.bounds");
            }
        }
        Some(Meta { name, bounds, tags, props, says, sounds, spawns, body, fits })
    }

    fn meta_body(&mut self, o: &js::ObjectExpression) -> Option<Body> {
        let mut b = Body::default();
        let mut eye = None;
        let mut grip = None;
        let mut errs = 0;
        for prop in o.properties.iter() {
            let js::ObjectPropertyKind::ObjectProperty(p) = prop else {
                self.err(o.span, "spread is not allowed in meta.body");
                continue;
            };
            let key = match &p.key {
                js::PropertyKey::StaticIdentifier(id) if !p.computed => id.name.as_str().to_string(),
                js::PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                _ => {
                    self.err(p.span, "meta.body keys must be plain names");
                    continue;
                }
            };
            let mut bad = |s: &mut Self, msg: String| {
                s.err(p.span, msg);
                errs += 1;
            };
            match key.as_str() {
                "height" | "eye" | "radius" | "reach" => match literal_num(&p.value) {
                    Some(v) if v.is_finite() && v > 0.0 && v <= 80.0 => match key.as_str() {
                        "height" => b.height = v,
                        "eye" => eye = Some(v),
                        "radius" => b.radius = v,
                        _ => b.reach = v,
                    },
                    _ => bad(self, format!("meta.body.{key} must be a number of metres in (0, 80]")),
                },
                "grip" | "seat" => match literal_nums(&p.value) {
                    Some(v) if v.len() == 3 && v.iter().all(|x| x.is_finite() && x.abs() <= 80.0) => {
                        if key == "grip" {
                            grip = Some([v[0], v[1], v[2]]);
                        } else {
                            b.seat = Some([v[0], v[1], v[2]]);
                        }
                    }
                    _ => bad(self, format!("meta.body.{key} must be [x, y, z] in metres (x right, y up, z forward)")),
                },
                "roles" => match literal_strings(&p.value) {
                    Some(v) => {
                        for r in v {
                            let both = matches!(r.as_str(), "reach" | "raise");
                            let names = if both { vec![format!("{r}_l"), format!("{r}_r")] } else { vec![role_alias(&r)] };
                            for r in names {
                                if !ROLES.contains(&r.as_str()) {
                                    bad(self, format!("unknown role '{r}' in meta.body.roles (use {}, or reach / raise for both sides)", ROLES.join(", ")));
                                } else if !b.roles.contains(&r) {
                                    b.roles.push(r);
                                }
                            }
                        }
                    }
                    None => bad(self, "meta.body.roles must be an array of role names".into()),
                },
                "gait" => match &p.value {
                    Expression::StringLiteral(s) if ["biped", "quad", "slither", "hover"].contains(&s.value.as_str()) => b.gait = s.value.to_string(),
                    _ => bad(self, "meta.body.gait must be \"biped\", \"quad\", \"slither\" or \"hover\"".into()),
                },
                "from" => match &p.value {
                    Expression::StringLiteral(s) if !s.value.trim().is_empty() && s.value.len() <= 64 => b.from = Some(s.value.trim().to_string()),
                    _ => bad(self, "meta.body.from must be the name of the body it was written from".into()),
                },
                "flies" | "arms" => match &p.value {
                    Expression::BooleanLiteral(v) => {
                        if key == "flies" {
                            b.flies = v.value
                        } else {
                            b.arms = v.value
                        }
                    }
                    _ => bad(self, format!("meta.body.{key} must be true or false")),
                },
                "look" => match &p.value {
                    Expression::ObjectExpression(lo) => {
                        for lp in lo.properties.iter() {
                            let js::ObjectPropertyKind::ObjectProperty(lp) = lp else { continue };
                            let name = match &lp.key {
                                js::PropertyKey::StaticIdentifier(id) if !lp.computed => id.name.as_str().to_string(),
                                js::PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                                _ => {
                                    bad(self, "meta.body.look keys must be plain names".into());
                                    continue;
                                }
                            };
                            match literal_nums(&lp.value) {
                                Some(v) if v.len() == 2 && v[0].is_finite() && v[1].is_finite() && v[0] <= v[1] && valid_prop_name(&name) => {
                                    if b.look.len() >= MAX_SLIDERS {
                                        bad(self, format!("meta.body.look may hold at most {MAX_SLIDERS} sliders (they are k.a … k.e)"));
                                        break;
                                    }
                                    b.look.push((name, v[0], v[1]));
                                }
                                _ => bad(self, format!("meta.body.look.{name} must be [lo, hi]")),
                            }
                        }
                    }
                    _ => bad(self, "meta.body.look must be an object of [lo, hi] ranges, e.g. { height: [1.5, 2.0] }".into()),
                },
                _ => bad(self, format!("unknown key meta.body.{key} (height, eye, radius, reach, grip, seat, roles, gait, flies, arms, look, from)")),
            }
        }
        b.eye = eye.unwrap_or(b.height * 0.92).min(b.height * 1.2);
        b.grip = grip.unwrap_or([b.radius * 0.6, b.height * 0.55, b.radius]);
        if b.roles.is_empty() {
            self.err(o.span, format!("meta.body.roles must list the roles the shape answers to ({})", ROLES.join(", ")));
            return None;
        }
        (errs == 0).then_some(b)
    }

    fn meta_props(&mut self, o: &js::ObjectExpression) -> Vec<(String, f32)> {
        let mut out: Vec<(String, f32)> = Vec::new();
        for prop in o.properties.iter() {
            let js::ObjectPropertyKind::ObjectProperty(p) = prop else {
                self.err(o.span, "spread is not allowed in meta.props");
                continue;
            };
            let key = match &p.key {
                js::PropertyKey::StaticIdentifier(id) if !p.computed => id.name.as_str().to_string(),
                js::PropertyKey::StringLiteral(s) => s.value.as_str().to_string(),
                _ => {
                    self.err(p.span, "meta.props keys must be plain names");
                    continue;
                }
            };
            if !valid_prop_name(&key) || ctx_field(&key).is_some() {
                self.err(p.span, format!("'{key}' is not a valid property name (lower-case letters, digits and _, not one of {})", CTX_FIELDS.join(", ")));
                continue;
            }
            let v = match &p.value {
                Expression::NumericLiteral(n) => Some(n.value as f32),
                Expression::BooleanLiteral(b) => Some(if b.value { 1.0 } else { 0.0 }),
                Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => match &u.argument {
                    Expression::NumericLiteral(n) => Some(-n.value as f32),
                    _ => None,
                },
                _ => None,
            };
            match v {
                Some(v) if v.is_finite() && v.abs() <= 1e5 => {
                    if out.iter().any(|(k, _)| *k == key) {
                        self.err(p.span, format!("meta.props.{key} is given twice"));
                    } else if out.len() >= MAX_PROPS {
                        self.err(p.span, format!("meta.props may hold at most {MAX_PROPS} properties"));
                    } else {
                        out.push((key, v));
                    }
                }
                _ => self.err(p.span, format!("meta.props.{key} must be a literal number or boolean")),
            }
        }
        out
    }

    fn literal_numbers(&mut self, e: &Expression) -> Option<Vec<f32>> {
        let Expression::ArrayExpression(a) = e else { return None };
        let mut v = Vec::new();
        for el in a.elements.iter() {
            match el {
                js::ArrayExpressionElement::NumericLiteral(n) => v.push(n.value as f32),
                _ => return None,
            }
        }
        Some(v)
    }

    fn function<'a>(&mut self, f: &js::Function<'a>, name: &'static str, role: Role, consts: &[(&'a str, &Expression<'a>, Span)], lists: Option<[usize; 3]>) -> Option<Func> {
        if f.r#async || f.generator {
            self.err(f.span, format!("{name}() must be a plain function (no async or generators)"));
        }
        let mut params: Vec<String> = Vec::new();
        for p in f.params.items.iter() {
            if p.initializer.is_some() {
                self.err(p.span, "default parameter values are not allowed");
            }
            match binding_name(&p.pattern) {
                Some(n) => params.push(n.to_string()),
                None => self.err(p.span, "parameters must be plain names"),
            }
        }
        if f.params.rest.is_some() {
            self.err(f.params.span, "rest parameters are not allowed");
        }
        match role {
            Role::Shape(_) if !(3..=4).contains(&params.len()) => {
                self.err(f.params.span, format!("{name}() must take (x, y, z, k)"));
                return None;
            }
            Role::Behavior(b) if !(2..=4).contains(&params.len()) || (b == Behavior::Tick && params.len() > 3) => {
                let sig = if b == Behavior::Tick { "(s, w, k)" } else { "(s, w, k, o)" };
                self.err(f.params.span, format!("{name}() must take {sig}"));
                return None;
            }
            _ => {}
        }
        let body = f.body.as_ref()?;
        let ret = match role {
            Role::Shape(t) => t,
            Role::Behavior(_) => Ty::F,
        };
        let mut lw = Lower {
            cx: self,
            fname: name,
            role,
            lists,
            params,
            locals: Vec::new(),
            scopes: vec![Vec::new()],
            readonly: Vec::new(),
            iter_product: 1,
            depth: 0,
            stmts: 0,
            ret,
        };
        let mut out = Vec::new();
        for (cname, init, span) in consts {
            lw.depth = 0;
            if let Some(e) = lw.const_init(init) {
                if let Some(id) = lw.declare(cname, e.ty, false, *span) {
                    out.push(Stmt::Let { id, init: e });
                }
            }
        }
        lw.block(&body.statements, &mut out);
        if matches!(role, Role::Shape(_)) && !always_returns(&out) {
            let what = if ret == Ty::F { "a number" } else { "a colour, e.g. rgb(r, g, b)" };
            lw.cx.err(Span::new(body.span.end.saturating_sub(1), body.span.end), format!("{name}() can reach its end without returning; every path must return {what}"));
        }
        Some(Func { locals: lw.locals, body: out })
    }
}

/// What a function is for: a shape function returning a value of this type,
/// or a behaviour entry point (no return value; may change state and props).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Shape(Ty),
    Behavior(Behavior),
}

/// What a parameter name stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParamRole {
    /// x, y, z of a shape function.
    Coord(u8),
    K,
    State,
    World,
    Other,
}

/// An assignable member in behaviour code.
#[derive(Clone, Copy, Debug)]
enum Target {
    State(u8),
    Prop(bool, u16),
}

fn literal_num(e: &Expression) -> Option<f32> {
    match e {
        Expression::NumericLiteral(n) => Some(n.value as f32),
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => match &u.argument {
            Expression::NumericLiteral(n) => Some(-n.value as f32),
            _ => None,
        },
        _ => None,
    }
}

fn literal_nums(e: &Expression) -> Option<Vec<f32>> {
    let Expression::ArrayExpression(a) = e else { return None };
    a.elements.iter().map(|el| el.as_expression().and_then(literal_num)).collect()
}

fn literal_strings(e: &Expression) -> Option<Vec<String>> {
    let Expression::ArrayExpression(a) = e else { return None };
    a.elements
        .iter()
        .map(|el| match el {
            js::ArrayExpressionElement::StringLiteral(s) => Some(s.value.as_str().trim().to_lowercase()),
            _ => None,
        })
        .collect()
}

/// Old figure channel names still work as role names.
pub fn role_alias(r: &str) -> String {
    match r {
        "l_raise" => "raise_l",
        "r_raise" => "raise_r",
        "l_fwd" | "l_reach" => "reach_l",
        "r_fwd" | "r_reach" => "reach_r",
        "nod" => "head",
        r => r,
    }
    .to_string()
}

fn is_inert_literal(e: &Expression) -> bool {
    match e {
        Expression::StringLiteral(_) | Expression::NumericLiteral(_) | Expression::BooleanLiteral(_) => true,
        Expression::ArrayExpression(a) => a.elements.iter().all(|el| {
            matches!(
                el,
                js::ArrayExpressionElement::StringLiteral(_) | js::ArrayExpressionElement::NumericLiteral(_) | js::ArrayExpressionElement::BooleanLiteral(_)
            )
        }),
        _ => false,
    }
}

fn binding_name<'a>(b: &BindingPattern<'a>) -> Option<&'a str> {
    match b {
        BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

fn stmt_reject_reason(st: &Statement) -> Option<&'static str> {
    Some(match st {
        Statement::WhileStatement(_) => "while loops are not allowed; use for with literal bounds (at most 32 iterations)",
        Statement::DoWhileStatement(_) => "do/while loops are not allowed; use for with literal bounds (at most 32 iterations)",
        Statement::ForInStatement(_) | Statement::ForOfStatement(_) => "for-in/for-of loops are not allowed",
        Statement::SwitchStatement(_) => "switch is not allowed; use if/else",
        Statement::TryStatement(_) => "try/catch is not allowed",
        Statement::ThrowStatement(_) => "throw is not allowed",
        Statement::LabeledStatement(_) => "labels are not allowed",
        Statement::BreakStatement(_) => "break is not allowed",
        Statement::ContinueStatement(_) => "continue is not allowed",
        Statement::DebuggerStatement(_) => "debugger is not allowed",
        Statement::WithStatement(_) => "with is not allowed",
        Statement::ClassDeclaration(_) => "classes are not allowed",
        Statement::FunctionDeclaration(_) => "nested functions are not allowed",
        Statement::ImportDeclaration(_) => "import is not allowed",
        Statement::ExportAllDeclaration(_)
        | Statement::ExportDefaultDeclaration(_)
        | Statement::ExportNamedDeclaration(_)
        | Statement::ExportFromDeclaration(_) => "only `export const meta` and `export function` sdf, color, tick, use or touch are allowed",
        _ => return None,
    })
}

/// True if every path through `stmts` ends in a return.
pub fn always_returns(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Return(_) | Stmt::End => true,
        Stmt::If { then, els, .. } => always_returns(then) && always_returns(els),
        _ => false,
    })
}

struct Lower<'c, 's> {
    cx: &'c mut Cx<'s>,
    fname: &'static str,
    role: Role,
    /// Lengths of meta.says / meta.sounds / meta.spawns, when meta parsed.
    lists: Option<[usize; 3]>,
    params: Vec<String>,
    locals: Vec<Local>,
    scopes: Vec<Vec<(String, u32)>>,
    readonly: Vec<u32>,
    iter_product: u32,
    depth: u32,
    stmts: u32,
    ret: Ty,
}

fn ex(kind: ExprKind, ty: Ty) -> Expr {
    Expr { kind, ty }
}

impl<'c, 's> Lower<'c, 's> {
    fn err(&mut self, span: Span, msg: impl Into<String>) {
        self.cx.err(span, msg);
    }

    /// The role of a parameter name (if it is one and not shadowed).
    fn param_role(&self, name: &str) -> Option<ParamRole> {
        if self.lookup(name).is_some() {
            return None;
        }
        let i = self.params.iter().position(|p| p == name)?;
        Some(match self.role {
            Role::Shape(_) => {
                if i == 3 {
                    ParamRole::K
                } else {
                    ParamRole::Coord(i as u8)
                }
            }
            Role::Behavior(_) => match i {
                0 => ParamRole::State,
                1 => ParamRole::World,
                2 => ParamRole::K,
                _ => ParamRole::Other,
            },
        })
    }

    fn is_behavior(&self) -> bool {
        matches!(self.role, Role::Behavior(_))
    }

    /// Index of a property name in the module's list (added on first use).
    fn prop_index(&mut self, name: &str, span: Span) -> Option<u16> {
        if !valid_prop_name(name) {
            self.err(span, format!("'{name}' is not a property name (lower-case letters, digits and _)"));
            return None;
        }
        if let Some(i) = self.cx.prop_names.iter().position(|p| p == name) {
            return Some(i as u16);
        }
        if self.cx.prop_names.len() >= MAX_PROPS {
            self.err(span, format!("behaviour code may use at most {MAX_PROPS} different properties"));
            return None;
        }
        self.cx.prop_names.push(name.to_string());
        Some((self.cx.prop_names.len() - 1) as u16)
    }

    /// A member that can be assigned in behaviour code: s.sN, w.<prop>, o.<prop>.
    fn member_target(&mut self, m: &js::StaticMemberExpression) -> Option<Target> {
        let field = m.property.name.as_str();
        let Expression::Identifier(obj) = &m.object else {
            self.err(m.span, "only s.<slot>, w.<property> and o.<property> can be assigned");
            return None;
        };
        let on = obj.name.as_str();
        match self.param_role(on) {
            Some(ParamRole::State) => match state_field(field) {
                Some(i) => Some(Target::State(i)),
                None => {
                    self.err(m.span, format!("{on}.{field} does not exist; state slots are {}", STATE_FIELDS.join(", ")));
                    None
                }
            },
            Some(ParamRole::World) => {
                if ctx_field(field).is_some() {
                    self.err(m.span, format!("{on}.{field} is read-only (it describes the situation); only properties can be set"));
                    return None;
                }
                self.prop_index(field, m.span).map(|i| Target::Prop(false, i))
            }
            Some(ParamRole::Other) => {
                if ctx_field(field).is_some() {
                    self.err(m.span, format!("{on}.{field} is not a property"));
                    return None;
                }
                self.prop_index(field, m.span).map(|i| Target::Prop(true, i))
            }
            Some(ParamRole::K) => {
                self.err(m.span, format!("{on} (instance params) is read-only; keep changing values in s.s0 … s.s7"));
                None
            }
            _ => {
                self.err(m.span, "only s.<slot>, w.<property> and o.<property> can be assigned");
                None
            }
        }
    }

    fn target_read(&self, t: Target) -> Expr {
        match t {
            Target::State(i) => ex(ExprKind::State(i), Ty::F),
            Target::Prop(o, i) => ex(ExprKind::Prop { other: o, idx: i }, Ty::F),
        }
    }

    fn target_write(t: Target, value: Expr) -> Stmt {
        match t {
            Target::State(idx) => Stmt::SetState { idx, value },
            Target::Prop(other, idx) => Stmt::SetProp { other, idx, value },
        }
    }

    fn lookup(&self, name: &str) -> Option<u32> {
        for scope in self.scopes.iter().rev() {
            if let Some((_, id)) = scope.iter().rev().find(|(n, _)| n == name) {
                return Some(*id);
            }
        }
        None
    }

    fn declare(&mut self, name: &str, ty: Ty, mutable: bool, span: Span) -> Option<u32> {
        if self.params.iter().any(|p| p == name) {
            self.err(span, format!("'{name}' shadows a parameter"));
            return None;
        }
        // glow() came after many saved types had named a local `glow`; calls
        // resolve by name, so the two never mix.
        if (api::is_api_name(name) && name != "glow") || matches!(name, "PI" | "Math" | "sdf" | "color" | "meta") {
            self.err(span, format!("'{name}' is a reserved name"));
            return None;
        }
        if self.scopes.last().is_some_and(|s| s.iter().any(|(n, _)| n == name)) {
            self.err(span, format!("'{name}' is declared twice"));
            return None;
        }
        let id = self.locals.len() as u32;
        self.locals.push(Local { name: sanitize(name), ty, mutable });
        self.scopes.last_mut()?.push((name.to_string(), id));
        Some(id)
    }

    fn const_init(&mut self, e: &Expression) -> Option<Expr> {
        // Top-level consts cannot see the function parameters.
        let saved = std::mem::take(&mut self.params);
        let r = self.expr(e);
        self.params = saved;
        r
    }

    fn block(&mut self, stmts: &[Statement], out: &mut Vec<Stmt>) {
        self.scopes.push(Vec::new());
        for st in stmts {
            if always_returns(out) {
                // Unreachable code: dropped, so both targets agree on control flow.
                break;
            }
            self.stmt(st, out);
        }
        self.scopes.pop();
    }

    fn body(&mut self, st: &Statement) -> Vec<Stmt> {
        let mut out = Vec::new();
        match st {
            Statement::BlockStatement(b) => self.block(&b.body, &mut out),
            other => {
                self.scopes.push(Vec::new());
                self.stmt(other, &mut out);
                self.scopes.pop();
            }
        }
        out
    }

    fn stmt(&mut self, st: &Statement, out: &mut Vec<Stmt>) {
        self.stmts += 1;
        if self.stmts > MAX_STMTS {
            if self.stmts == MAX_STMTS + 1 {
                self.err(st.span(), format!("too many statements (limit {MAX_STMTS})"));
            }
            return;
        }
        match st {
            Statement::VariableDeclaration(vd) => self.var_decl(vd, out),
            Statement::ExpressionStatement(es) => self.expr_stmt(&es.expression, out),
            Statement::IfStatement(is) => {
                let cond = self.cond(&is.test);
                let then = self.body(&is.consequent);
                let els = is.alternate.as_ref().map(|a| self.body(a)).unwrap_or_default();
                if let Some(cond) = cond {
                    out.push(Stmt::If { cond, then, els });
                }
            }
            Statement::BlockStatement(b) => self.block(&b.body, out),
            Statement::ForStatement(fs) => self.for_stmt(fs, out),
            Statement::ReturnStatement(rs) if self.is_behavior() => match &rs.argument {
                None => out.push(Stmt::End),
                Some(_) => self.err(rs.span, format!("{}() does not return a value; use a plain `return;`", self.fname)),
            },
            Statement::ReturnStatement(rs) => match &rs.argument {
                None => self.err(rs.span, format!("{}() must return a value", self.fname)),
                Some(a) => {
                    if let Some(e) = self.expr(a) {
                        if e.ty != self.ret {
                            let want = if self.ret == Ty::F { "a number (distance)" } else { "a colour from rgb(), hsv() or mix()" };
                            self.err(rs.span, format!("{}() must return {want}, not a {}", self.fname, e.ty.name()));
                        } else {
                            out.push(Stmt::Return(e));
                        }
                    }
                }
            },
            Statement::EmptyStatement(_) => {}
            other => {
                let msg = stmt_reject_reason(other).unwrap_or("this statement is not allowed");
                self.err(other.span(), msg);
            }
        }
    }

    fn var_decl(&mut self, vd: &js::VariableDeclaration, out: &mut Vec<Stmt>) {
        let mutable = match vd.kind {
            js::VariableDeclarationKind::Const => false,
            js::VariableDeclarationKind::Let => true,
            _ => {
                self.err(vd.span, "use const or let");
                return;
            }
        };
        for decl in vd.declarations.iter() {
            let Some(name) = binding_name(&decl.id) else {
                self.err(decl.span, "destructuring is not allowed");
                continue;
            };
            let Some(init) = &decl.init else {
                self.err(decl.span, format!("'{name}' needs an initial value"));
                continue;
            };
            if let Some(e) = self.expr(init) {
                if let Some(id) = self.declare(name, e.ty, mutable, decl.span) {
                    out.push(Stmt::Let { id, init: e });
                }
            }
        }
    }

    fn assign_target(&mut self, name: &str, span: Span) -> Option<(u32, Ty)> {
        let Some(id) = self.lookup(name) else {
            if self.params.iter().any(|p| p == name) {
                self.err(span, format!("parameter '{name}' cannot be reassigned; copy it into a let"));
            } else {
                self.err(span, format!("unknown variable '{name}'"));
            }
            return None;
        };
        let local = &self.locals[id as usize];
        if self.readonly.contains(&id) {
            self.err(span, format!("loop variable '{name}' cannot be assigned"));
            return None;
        }
        if !local.mutable {
            self.err(span, format!("'{name}' is const; declare it with let to reassign"));
            return None;
        }
        Some((id, local.ty))
    }

    fn assign_op(&mut self, op: AssignmentOperator, span: Span) -> Option<Option<BinOp>> {
        Some(match op {
            AssignmentOperator::Assign => None,
            AssignmentOperator::Addition => Some(BinOp::Add),
            AssignmentOperator::Subtraction => Some(BinOp::Sub),
            AssignmentOperator::Multiplication => Some(BinOp::Mul),
            AssignmentOperator::Division => Some(BinOp::Div),
            AssignmentOperator::Remainder => Some(BinOp::Rem),
            _ => {
                self.err(span, "only =, +=, -=, *=, /= and %= are allowed");
                return None;
            }
        })
    }

    fn expr_stmt(&mut self, e: &Expression, out: &mut Vec<Stmt>) {
        match e {
            Expression::AssignmentExpression(a) => {
                if let js::AssignmentTarget::StaticMemberExpression(m) = &a.left {
                    if !self.is_behavior() {
                        self.err(a.span, "only local variables can be assigned");
                        return;
                    }
                    let Some(t) = self.member_target(m) else { return };
                    let Some(rhs) = self.expr(&a.right) else { return };
                    let Some(op) = self.assign_op(a.operator, a.span) else { return };
                    let value = match op {
                        None => rhs,
                        Some(op) => match self.arith(op, self.target_read(t), rhs, a.span) {
                            Some(v) => v,
                            None => return,
                        },
                    };
                    if value.ty != Ty::F {
                        self.err(a.span, format!("state slots and properties hold numbers, not a {}", value.ty.name()));
                        return;
                    }
                    out.push(Self::target_write(t, value));
                    return;
                }
                let js::AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    let what = if self.is_behavior() { "only local variables, s.<slot>, w.<property> and o.<property> can be assigned" } else { "only local variables can be assigned" };
                    self.err(a.span, what);
                    return;
                };
                let Some((vid, ty)) = self.assign_target(id.name.as_str(), a.span) else { return };
                let Some(rhs) = self.expr(&a.right) else { return };
                let Some(op) = self.assign_op(a.operator, a.span) else { return };
                let value = match op {
                    None => rhs,
                    Some(op) => {
                        let cur = ex(ExprKind::Local(vid), ty);
                        match self.arith(op, cur, rhs, a.span) {
                            Some(v) => v,
                            None => return,
                        }
                    }
                };
                if value.ty != ty {
                    self.err(a.span, format!("cannot assign a {} to '{}', which holds a {}", value.ty.name(), id.name.as_str(), ty.name()));
                    return;
                }
                out.push(Stmt::Assign { id: vid, value });
            }
            Expression::UpdateExpression(u) => {
                let op = if u.operator == UpdateOperator::Increment { BinOp::Add } else { BinOp::Sub };
                if let js::SimpleAssignmentTarget::StaticMemberExpression(m) = &u.argument {
                    if !self.is_behavior() {
                        self.err(u.span, "only local variables can be incremented");
                        return;
                    }
                    let Some(t) = self.member_target(m) else { return };
                    let value = ex(ExprKind::Bin(op, Box::new(self.target_read(t)), Box::new(ex(ExprKind::Num(1.0), Ty::F))), Ty::F);
                    out.push(Self::target_write(t, value));
                    return;
                }
                let js::SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                    self.err(u.span, "only local variables can be incremented");
                    return;
                };
                let Some((vid, ty)) = self.assign_target(id.name.as_str(), u.span) else { return };
                if ty != Ty::F {
                    self.err(u.span, "++/-- need a number");
                    return;
                }
                let value = ex(ExprKind::Bin(op, Box::new(ex(ExprKind::Local(vid), Ty::F)), Box::new(ex(ExprKind::Num(1.0), Ty::F))), Ty::F);
                out.push(Stmt::Assign { id: vid, value });
            }
            Expression::CallExpression(c) if self.is_behavior() && self.effect_call(c, out) => {}
            other => {
                // Still walk it so that disallowed constructs inside are reported precisely.
                if self.expr(other).is_some() {
                    let what = if self.is_behavior() { "this expression does nothing; only assignments and effects (say, sound, spawn, transform, remove) may be statements" } else { "this expression does nothing; only assignments may be statements" };
                    self.err(other.span(), what);
                }
            }
        }
    }

    /// An effect call statement in behaviour code. Returns false if `c` is not
    /// an effect call (it is then checked as an ordinary expression).
    fn effect_call(&mut self, c: &js::CallExpression, out: &mut Vec<Stmt>) -> bool {
        let Expression::Identifier(id) = &c.callee else { return false };
        let name = id.name.as_str();
        if self.lookup(name).is_some() {
            return false;
        }
        let Some(effect) = Effect::from_name(name) else { return false };
        let mut args = Vec::new();
        for a in c.arguments.iter() {
            match a.as_expression().and_then(|e| self.expr(e)) {
                Some(e) => args.push(e),
                None => return true,
            }
        }
        if !effect.takes_index() {
            if !args.is_empty() {
                self.err(c.span, format!("{name}() takes no arguments"));
            } else {
                out.push(Stmt::Effect { effect, arg: None });
            }
            return true;
        }
        if args.len() > 1 || args.first().is_some_and(|a| a.ty != Ty::F) {
            self.err(c.span, format!("{name}(i) takes one number: the index into meta.{}", effect_list(effect)));
            return true;
        }
        let arg = args.pop().unwrap_or(ex(ExprKind::Num(0.0), Ty::F));
        if let (Some(lists), ExprKind::Num(v)) = (self.lists, &arg.kind) {
            let n = lists[effect_list_index(effect)];
            if *v < 0.0 || (*v as usize) >= n || v.fract() != 0.0 {
                self.err(c.span, format!("{name}({v}) needs meta.{} to have an entry {v} (it has {n})", effect_list(effect)));
                return true;
            }
        }
        out.push(Stmt::Effect { effect, arg: Some(arg) });
        true
    }

    fn lit_num(&self, e: &Expression) -> Option<f64> {
        match e {
            Expression::NumericLiteral(n) => Some(n.value),
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => match &u.argument {
                Expression::NumericLiteral(n) => Some(-n.value),
                _ => None,
            },
            Expression::ParenthesizedExpression(p) => self.lit_num(&p.expression),
            _ => None,
        }
    }

    fn for_stmt(&mut self, fs: &js::ForStatement, out: &mut Vec<Stmt>) {
        const HELP: &str = "for loops need literal bounds, e.g. for (let i = 0; i < 8; i++), with at most 32 iterations";
        let bad = |s: &mut Self| s.err(fs.span, HELP);
        // init: let i = <literal>
        let Some(js::ForStatementInit::VariableDeclaration(vd)) = &fs.init else { return bad(self) };
        if vd.declarations.len() != 1 || vd.kind != js::VariableDeclarationKind::Let {
            return bad(self);
        }
        let decl = &vd.declarations[0];
        let Some(name) = binding_name(&decl.id) else { return bad(self) };
        let Some(start) = decl.init.as_ref().and_then(|e| self.lit_num(e)) else { return bad(self) };
        // test: i < N, i <= N, i > N, i >= N
        let Some(Expression::BinaryExpression(test)) = &fs.test else { return bad(self) };
        let Expression::Identifier(tid) = &test.left else { return bad(self) };
        if tid.name.as_str() != name {
            return bad(self);
        }
        let Some(end) = self.lit_num(&test.right) else { return bad(self) };
        // update: i++, ++i, i--, i += c, i -= c
        let step: f64 = match &fs.update {
            Some(Expression::UpdateExpression(u)) => {
                let js::SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else { return bad(self) };
                if id.name.as_str() != name {
                    return bad(self);
                }
                if u.operator == UpdateOperator::Increment { 1.0 } else { -1.0 }
            }
            Some(Expression::AssignmentExpression(a)) => {
                let js::AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else { return bad(self) };
                if id.name.as_str() != name {
                    return bad(self);
                }
                let Some(c) = self.lit_num(&a.right) else { return bad(self) };
                match a.operator {
                    AssignmentOperator::Addition => c,
                    AssignmentOperator::Subtraction => -c,
                    _ => return bad(self),
                }
            }
            _ => return bad(self),
        };
        if step == 0.0 || !step.is_finite() || !start.is_finite() || !end.is_finite() {
            return bad(self);
        }
        let inclusive = matches!(test.operator, BinaryOperator::LessEqualThan | BinaryOperator::GreaterEqualThan);
        let up = matches!(test.operator, BinaryOperator::LessThan | BinaryOperator::LessEqualThan);
        let down = matches!(test.operator, BinaryOperator::GreaterThan | BinaryOperator::GreaterEqualThan);
        if !(up || down) || (up && step < 0.0) || (down && step > 0.0) {
            return bad(self);
        }
        // Count iterations exactly as JS would, with a hard cap.
        let mut count = 0u32;
        let mut v = start;
        loop {
            let go = match test.operator {
                BinaryOperator::LessThan => v < end,
                BinaryOperator::LessEqualThan => v <= end,
                BinaryOperator::GreaterThan => v > end,
                _ => v >= end,
            };
            if !go {
                break;
            }
            count += 1;
            if count > MAX_LOOP_ITERS {
                self.err(fs.span, format!("this loop runs more than {MAX_LOOP_ITERS} times; {HELP}"));
                return;
            }
            v += step;
        }
        let _ = inclusive;
        let product = self.iter_product.saturating_mul(count.max(1));
        if product > MAX_NESTED_ITERS {
            self.err(fs.span, format!("nested loops run more than {MAX_NESTED_ITERS} times in total"));
            return;
        }
        self.scopes.push(Vec::new());
        let Some(id) = self.declare(name, Ty::F, false, decl.span) else {
            self.scopes.pop();
            return;
        };
        self.readonly.push(id);
        let saved = self.iter_product;
        self.iter_product = product;
        let body = self.body(&fs.body);
        self.iter_product = saved;
        self.readonly.pop();
        self.scopes.pop();
        out.push(Stmt::For { id, start: start as f32, step: step as f32, count, body });
    }

    fn cond(&mut self, e: &Expression) -> Option<Expr> {
        let c = self.expr(e)?;
        self.to_bool(c, e.span())
    }

    fn to_bool(&mut self, e: Expr, span: Span) -> Option<Expr> {
        match e.ty {
            Ty::B => Some(e),
            Ty::F => Some(ex(ExprKind::Truthy(Box::new(e)), Ty::B)),
            Ty::V => {
                self.err(span, "a colour/point cannot be used as a condition");
                None
            }
        }
    }

    fn arith(&mut self, op: BinOp, a: Expr, b: Expr, span: Span) -> Option<Expr> {
        let ty = match (a.ty, b.ty) {
            (Ty::F, Ty::F) => Ty::F,
            (Ty::V, Ty::V) | (Ty::V, Ty::F) | (Ty::F, Ty::V) if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div) => Ty::V,
            _ => {
                self.err(span, format!("cannot apply this operator to a {} and a {}", a.ty.name(), b.ty.name()));
                return None;
            }
        };
        Some(ex(ExprKind::Bin(op, Box::new(a), Box::new(b)), ty))
    }

    fn expr(&mut self, e: &Expression) -> Option<Expr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            self.err(e.span(), "expression nested too deeply");
            return None;
        }
        let r = self.expr_inner(e);
        self.depth -= 1;
        r
    }

    fn expr_inner(&mut self, e: &Expression) -> Option<Expr> {
        match e {
            Expression::NumericLiteral(n) => {
                let v = n.value as f32;
                if !v.is_finite() {
                    self.err(n.span, "number literal out of range");
                    return None;
                }
                Some(ex(ExprKind::Num(v), Ty::F))
            }
            Expression::BooleanLiteral(b) => Some(ex(ExprKind::Num(if b.value { 1.0 } else { 0.0 }), Ty::B)),
            Expression::Identifier(id) => self.ident(id.name.as_str(), id.span),
            Expression::ParenthesizedExpression(p) => self.expr(&p.expression),
            Expression::StaticMemberExpression(m) => self.member(m),
            Expression::ComputedMemberExpression(m) => {
                self.reject_object(&m.object);
                self.err(m.span, "computed member access (a[b]) is not allowed");
                None
            }
            Expression::PrivateFieldExpression(m) => {
                self.err(m.span, "private fields are not allowed");
                None
            }
            Expression::CallExpression(c) => self.call(c),
            Expression::UnaryExpression(u) => {
                let a = self.expr(&u.argument)?;
                match u.operator {
                    UnaryOperator::UnaryNegation => {
                        if a.ty == Ty::B {
                            self.err(u.span, "cannot negate a boolean");
                            return None;
                        }
                        if let ExprKind::Num(v) = a.kind {
                            return Some(ex(ExprKind::Num(-v), Ty::F));
                        }
                        let ty = a.ty;
                        Some(ex(ExprKind::Neg(Box::new(a)), ty))
                    }
                    UnaryOperator::UnaryPlus => {
                        if a.ty != Ty::F {
                            self.err(u.span, "unary + needs a number");
                            return None;
                        }
                        Some(a)
                    }
                    UnaryOperator::LogicalNot => {
                        let b = self.to_bool(a, u.span)?;
                        Some(ex(ExprKind::Not(Box::new(b)), Ty::B))
                    }
                    _ => {
                        self.err(u.span, format!("operator '{}' is not allowed", u.operator.as_str()));
                        None
                    }
                }
            }
            Expression::BinaryExpression(b) => {
                let l = self.expr(&b.left);
                let r = self.expr(&b.right);
                let (l, r) = (l?, r?);
                use BinaryOperator as O;
                let arith = |op| Some(op);
                let op = match b.operator {
                    O::Addition => arith(BinOp::Add),
                    O::Subtraction => arith(BinOp::Sub),
                    O::Multiplication => arith(BinOp::Mul),
                    O::Division => arith(BinOp::Div),
                    O::Remainder => arith(BinOp::Rem),
                    O::Exponential => arith(BinOp::Pow),
                    _ => None,
                };
                if let Some(op) = op {
                    return self.arith(op, l, r, b.span);
                }
                let cmp = match b.operator {
                    O::LessThan => CmpOp::Lt,
                    O::LessEqualThan => CmpOp::Le,
                    O::GreaterThan => CmpOp::Gt,
                    O::GreaterEqualThan => CmpOp::Ge,
                    O::Equality | O::StrictEquality => CmpOp::Eq,
                    O::Inequality | O::StrictInequality => CmpOp::Ne,
                    other => {
                        self.err(b.span, format!("operator '{}' is not allowed", other.as_str()));
                        return None;
                    }
                };
                if l.ty != Ty::F || r.ty != Ty::F {
                    self.err(b.span, "comparisons need numbers on both sides");
                    return None;
                }
                Some(ex(ExprKind::Cmp(cmp, Box::new(l), Box::new(r)), Ty::B))
            }
            Expression::LogicalExpression(l) => {
                let and = match l.operator {
                    LogicalOperator::And => true,
                    LogicalOperator::Or => false,
                    LogicalOperator::Coalesce => {
                        self.err(l.span, "?? is not allowed");
                        return None;
                    }
                };
                let a = self.expr(&l.left);
                let b = self.expr(&l.right);
                let a = self.to_bool(a?, l.left.span())?;
                let b = self.to_bool(b?, l.right.span())?;
                Some(ex(ExprKind::Logic(and, Box::new(a), Box::new(b)), Ty::B))
            }
            Expression::ConditionalExpression(c) => {
                let t = self.cond(&c.test);
                let a = self.expr(&c.consequent);
                let b = self.expr(&c.alternate);
                let (t, a, b) = (t?, a?, b?);
                if a.ty != b.ty || a.ty == Ty::B {
                    self.err(c.span, "both branches of ?: must be numbers, or both colours");
                    return None;
                }
                let ty = a.ty;
                Some(ex(ExprKind::Cond(Box::new(t), Box::new(a), Box::new(b)), ty))
            }
            Expression::AssignmentExpression(a) => {
                self.err(a.span, "assignments are only allowed as statements");
                None
            }
            Expression::UpdateExpression(u) => {
                self.err(u.span, "++/-- are only allowed as statements or in a for loop");
                None
            }
            Expression::StringLiteral(s) => {
                self.err(s.span, "strings are only allowed in meta");
                None
            }
            Expression::TemplateLiteral(s) => {
                self.err(s.span, "strings are only allowed in meta");
                None
            }
            Expression::TaggedTemplateExpression(s) => {
                self.err(s.span, "template tags are not allowed");
                None
            }
            Expression::ArrayExpression(a) => {
                self.err(a.span, "arrays are only allowed in meta");
                None
            }
            Expression::ObjectExpression(o) => {
                self.err(o.span, "objects are only allowed in meta");
                None
            }
            Expression::ArrowFunctionExpression(f) => {
                self.err(f.span, "closures are not allowed");
                None
            }
            Expression::FunctionExpression(f) => {
                self.err(f.span, "closures are not allowed");
                None
            }
            Expression::ClassExpression(c) => {
                self.err(c.span, "classes are not allowed");
                None
            }
            Expression::NewExpression(n) => {
                self.err(n.span, "'new' is not allowed");
                None
            }
            Expression::ThisExpression(t) => {
                self.err(t.span, "'this' is not allowed");
                None
            }
            Expression::Super(s) => {
                self.err(s.span, "'super' is not allowed");
                None
            }
            Expression::AwaitExpression(a) => {
                self.err(a.span, "async/await is not allowed");
                None
            }
            Expression::YieldExpression(y) => {
                self.err(y.span, "yield is not allowed");
                None
            }
            Expression::ImportExpression(i) => {
                self.err(i.span, "import() is not allowed");
                None
            }
            Expression::ImportMeta(i) => {
                self.err(i.span, "import.meta is not allowed");
                None
            }
            Expression::SequenceExpression(s) => {
                self.err(s.span, "comma expressions are not allowed");
                None
            }
            Expression::NullLiteral(n) => {
                self.err(n.span, "null is not allowed");
                None
            }
            Expression::RegExpLiteral(r) => {
                self.err(r.span, "regular expressions are not allowed");
                None
            }
            Expression::BigIntLiteral(b) => {
                self.err(b.span, "BigInt is not allowed");
                None
            }
            Expression::ChainExpression(c) => {
                self.err(c.span, "optional chaining is not allowed");
                None
            }
            other => {
                self.err(other.span(), "this expression is not allowed");
                None
            }
        }
    }

    fn ident(&mut self, name: &str, span: Span) -> Option<Expr> {
        if let Some(id) = self.lookup(name) {
            return Some(ex(ExprKind::Local(id), self.locals[id as usize].ty));
        }
        match self.param_role(name) {
            Some(ParamRole::Coord(i)) => return Some(ex(ExprKind::Param(i), Ty::F)),
            Some(ParamRole::K) => {
                self.err(span, format!("'{name}' (instance params) can only be read as {name}.<field>: {}", K_FIELDS.join(", ")));
                return None;
            }
            Some(ParamRole::State) => {
                self.err(span, format!("'{name}' (state) can only be used as {name}.<slot>: {}", STATE_FIELDS.join(", ")));
                return None;
            }
            Some(ParamRole::World) => {
                self.err(span, format!("'{name}' can only be used as {name}.<field>: {} or a property name", CTX_FIELDS.join(", ")));
                return None;
            }
            Some(ParamRole::Other) => {
                self.err(span, format!("'{name}' (the other thing) can only be used as {name}.<property>"));
                return None;
            }
            None => {}
        }
        match name {
            "PI" => Some(ex(ExprKind::Num(std::f32::consts::PI), Ty::F)),
            "undefined" | "NaN" | "Infinity" => {
                self.err(span, format!("'{name}' is not allowed"));
                None
            }
            n if api::is_api_name(n) => {
                self.err(span, format!("'{n}' is a function; call it"));
                None
            }
            n => {
                self.err(span, format!("unknown identifier '{n}' (only locals, parameters and allowlisted API names may be used)"));
                None
            }
        }
    }

    /// Report the first disallowed thing at the root of a member chain.
    fn reject_object(&mut self, e: &Expression) {
        match e {
            Expression::StaticMemberExpression(m) => self.reject_object(&m.object),
            Expression::ComputedMemberExpression(m) => self.reject_object(&m.object),
            Expression::CallExpression(c) => self.reject_object(&c.callee),
            Expression::Identifier(id) => {
                let n = id.name.as_str();
                if self.lookup(n).is_none() && !self.params.iter().any(|p| p == n) && n != "Math" {
                    self.err(id.span, format!("unknown identifier '{n}' (only locals, parameters and allowlisted API names may be used)"));
                }
            }
            Expression::ThisExpression(t) => self.err(t.span, "'this' is not allowed"),
            _ => {}
        }
    }

    fn member(&mut self, m: &js::StaticMemberExpression) -> Option<Expr> {
        let field = m.property.name.as_str();
        if m.optional {
            self.err(m.span, "optional chaining is not allowed");
            return None;
        }
        if let Expression::Identifier(obj) = &m.object {
            let on = obj.name.as_str();
            match self.param_role(on) {
                Some(ParamRole::K) => {
                    return match k_field(field) {
                        Some(i) => Some(ex(ExprKind::KField(i), Ty::F)),
                        None => {
                            self.err(m.span, format!("{on}.{field} does not exist; fields are {}", K_FIELDS.join(", ")));
                            None
                        }
                    };
                }
                Some(ParamRole::State) => {
                    return match state_field(field) {
                        Some(i) => Some(ex(ExprKind::State(i), Ty::F)),
                        None => {
                            self.err(m.span, format!("{on}.{field} does not exist; state slots are {}", STATE_FIELDS.join(", ")));
                            None
                        }
                    };
                }
                Some(ParamRole::World) => {
                    if let Some(i) = ctx_field(field) {
                        return Some(ex(ExprKind::Ctx(i), Ty::F));
                    }
                    return self.prop_index(field, m.span).map(|i| ex(ExprKind::Prop { other: false, idx: i }, Ty::F));
                }
                Some(ParamRole::Other) => {
                    if ctx_field(field).is_some() {
                        self.err(m.span, format!("{on}.{field} is not a property of the other thing"));
                        return None;
                    }
                    return self.prop_index(field, m.span).map(|i| ex(ExprKind::Prop { other: true, idx: i }, Ty::F));
                }
                _ => {}
            }
            if on == "Math" && self.lookup(on).is_none() {
                if field == "PI" {
                    return Some(ex(ExprKind::Num(std::f32::consts::PI), Ty::F));
                }
                self.err(m.span, format!("Math.{field} can only be called"));
                return None;
            }
        }
        if let Expression::ThisExpression(t) = &m.object {
            self.err(t.span, "'this' is not allowed");
            return None;
        }
        let obj = match &m.object {
            Expression::Identifier(id) => self.ident(id.name.as_str(), id.span)?,
            Expression::StaticMemberExpression(_) | Expression::CallExpression(_) | Expression::ParenthesizedExpression(_) => {
                self.expr(&m.object)?
            }
            other => {
                self.err(other.span(), "member access is only allowed on k or on vec3 values");
                return None;
            }
        };
        if obj.ty != Ty::V {
            self.err(m.span, format!(".{field} is only allowed on k or on a vec3 (colour/point)"));
            return None;
        }
        let comp = match field {
            "x" | "r" => 0,
            "y" | "g" => 1,
            "z" | "b" => 2,
            _ => {
                self.err(m.span, format!("vec3 has no field '{field}'; use .x .y .z (or .r .g .b)"));
                return None;
            }
        };
        Some(ex(ExprKind::Comp(Box::new(obj), comp), Ty::F))
    }

    fn call(&mut self, c: &js::CallExpression) -> Option<Expr> {
        if c.optional {
            self.err(c.span, "optional calls are not allowed");
            return None;
        }
        let name: &str = match &c.callee {
            Expression::Identifier(id) => {
                let n = id.name.as_str();
                if n == "sdf" || n == "color" || n == self.fname {
                    self.err(c.span, format!("calling {n}() from inside the module is recursion, which is not allowed"));
                    return None;
                }
                if self.lookup(n).is_some() || self.params.iter().any(|p| p == n) {
                    self.err(c.span, format!("'{n}' is not a function"));
                    return None;
                }
                n
            }
            Expression::StaticMemberExpression(m) => match &m.object {
                Expression::Identifier(o) if o.name.as_str() == "Math" && self.lookup("Math").is_none() => m.property.name.as_str(),
                _ => {
                    self.reject_object(&m.object);
                    self.err(c.span, "method calls are not allowed; only allowlisted API functions may be called");
                    return None;
                }
            },
            Expression::ImportExpression(i) => {
                self.err(i.span, "import() is not allowed");
                return None;
            }
            other => {
                self.reject_object(other);
                self.err(c.span, "only allowlisted API functions may be called");
                return None;
            }
        };
        if !api::is_api_name(name) {
            self.err(c.span, format!("'{name}' is not an allowed function"));
            return None;
        }
        let mut args = Vec::new();
        let mut ok = true;
        for a in c.arguments.iter() {
            match a {
                Argument::SpreadElement(s) => {
                    self.err(s.span, "spread arguments are not allowed");
                    ok = false;
                }
                other => match other.as_expression().and_then(|e| self.expr(e)) {
                    Some(e) => args.push(e),
                    None => ok = false,
                },
            }
        }
        if !ok {
            return None;
        }
        let tys: Vec<Ty> = args.iter().map(|a| a.ty).collect();
        match api::resolve(name, &tys) {
            Err(msg) => {
                self.err(c.span, msg);
                None
            }
            Ok(Resolved::Call(api, ty)) => Some(ex(ExprKind::Call(api, args), ty)),
            Ok(Resolved::PadHash) => {
                while args.len() < 3 {
                    args.push(ex(ExprKind::Num(0.0), Ty::F));
                }
                Some(ex(ExprKind::Call(api::Api::Hash, args), Ty::F))
            }
            Ok(Resolved::PadGlow) => {
                args.push(ex(ExprKind::Num(1.0), Ty::F));
                Some(ex(ExprKind::Call(api::Api::Glow, args), Ty::V))
            }
            Ok(Resolved::Fold(api)) => {
                let mut it = args.into_iter();
                let mut acc = it.next()?;
                for next in it {
                    acc = ex(ExprKind::Call(api, vec![acc, next]), Ty::F);
                }
                Some(acc)
            }
        }
    }
}

fn effect_list_index(e: Effect) -> usize {
    match e {
        Effect::Say => 0,
        Effect::Sound => 1,
        _ => 2,
    }
}

fn effect_list(e: Effect) -> &'static str {
    ["says", "sounds", "spawns"][effect_list_index(e)]
}

/// Make a JS identifier safe as a WGSL identifier fragment.
fn sanitize(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).take(24).collect()
}
