//! The object-type language: a strict JS subset, checked and translated to
//! WGSL (rendering) and bytecode (CPU). LLM code is never executed as JS.

pub mod api;
pub mod check;
pub mod ir;
pub mod probe;
pub mod vm;
pub mod wgsl;

use ir::{Behavior, Meta};
use std::cell::RefCell;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Stage {
    Parse,
    Allowlist,
    Translate,
    Probe,
    Place,
    Gpu,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Parse => "parse",
            Stage::Allowlist => "allowlist",
            Stage::Translate => "translate",
            Stage::Probe => "probe",
            Stage::Place => "placement",
            Stage::Gpu => "gpu",
        }
    }
    /// Errors at these stages are the model's to fix; later ones are our bugs.
    pub fn repairable(self) -> bool {
        !matches!(self, Stage::Translate | Stage::Gpu)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Diag {
    pub stage: Stage,
    pub line: u32,
    pub msg: String,
}

impl Diag {
    pub fn new(stage: Stage, line: u32, msg: String) -> Self {
        Diag { stage, line, msg }
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "[{}] line {}: {}", self.stage.label(), self.line, self.msg)
        } else {
            write!(f, "[{}] {}", self.stage.label(), self.msg)
        }
    }
}

pub fn format_diags(d: &[Diag]) -> String {
    d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n")
}

/// A type that passed parse, allowlist and translation.
#[derive(Clone, Debug)]
pub struct CompiledType {
    pub meta: Meta,
    pub source: String,
    /// WGSL with `__TID__` placeholders.
    pub wgsl: String,
    pub sdf: vm::Program,
    pub color: vm::Program,
    pub tick: Option<vm::Program>,
    pub use_fn: Option<vm::Program>,
    pub touch: Option<vm::Program>,
    /// Property names behaviour code reads or writes, in register order.
    pub prop_names: Vec<String>,
}

/// Fuel budget used when evaluating on the CPU outside of probing.
pub const RUN_FUEL: u32 = 20_000;
/// Fuel budget for one behaviour call.
pub const BEHAVIOR_FUEL: u32 = 6_000;

thread_local! {
    static REGS: RefCell<Vec<f32>> = RefCell::new(Vec::with_capacity(256));
}

impl CompiledType {
    pub fn sdf_checked(&self, p: [f32; 3], k: &[f32; 16], fuel: u32) -> Result<(f32, u32), vm::VmError> {
        REGS.with(|r| self.sdf.run(&mut r.borrow_mut(), p, k, fuel).map(|(o, u)| (o[0], u)))
    }
    pub fn color_checked(&self, p: [f32; 3], k: &[f32; 16], fuel: u32) -> Result<([f32; 3], u32), vm::VmError> {
        REGS.with(|r| self.color.run(&mut r.borrow_mut(), p, k, fuel))
    }
    /// The colour and how much it glows (0..1), from glow() in color().
    pub fn color_glow(&self, p: [f32; 3], k: &[f32; 16]) -> ([f32; 3], f32) {
        api::split_glow(self.color(p, k))
    }
    /// Whether color() marks which parts give off light.
    pub fn marks_glow(&self) -> bool {
        self.color.calls(api::Api::Glow)
    }
    pub fn behavior(&self, b: Behavior) -> Option<&vm::Program> {
        match b {
            Behavior::Tick => self.tick.as_ref(),
            Behavior::Use => self.use_fn.as_ref(),
            Behavior::Touch => self.touch.as_ref(),
        }
    }
    pub fn has_behavior(&self) -> bool {
        self.tick.is_some() || self.use_fn.is_some() || self.touch.is_some()
    }
    /// Run a behaviour entry point with the given fuel.
    pub fn run_behavior(&self, b: Behavior, io: vm::BehaviorIo, fuel: u32) -> Option<Result<u32, vm::VmError>> {
        let prog = self.behavior(b)?;
        Some(REGS.with(|r| prog.run_behavior(&mut r.borrow_mut(), io, fuel)))
    }
    /// Distance in local space; a failing evaluation counts as "far away".
    pub fn sdf(&self, p: [f32; 3], k: &[f32; 16]) -> f32 {
        match self.sdf_checked(p, k, RUN_FUEL) {
            Ok((d, _)) if d.is_finite() => d,
            _ => 1e9,
        }
    }
    pub fn color(&self, p: [f32; 3], k: &[f32; 16]) -> [f32; 3] {
        match self.color_checked(p, k, RUN_FUEL) {
            Ok((c, _)) if c.iter().all(|v| v.is_finite()) => c,
            _ => [1.0, 0.0, 1.0],
        }
    }
}

/// Steps 1–3: parse, allowlist, translate to both targets.
pub fn compile(source: &str) -> Result<CompiledType, Vec<Diag>> {
    let module = check::check(source)?;
    let wgsl = wgsl::emit(&module);
    let tr = |e: String| vec![Diag::new(Stage::Translate, 0, e)];
    let sdf = vm::compile(&module.sdf).map_err(tr)?;
    let color = vm::compile(&module.color).map_err(tr)?;
    let n = module.prop_names.len();
    let mut progs: [Option<vm::Program>; 3] = [None, None, None];
    for (b, f) in &module.behaviors {
        let i = match b {
            Behavior::Tick => 0,
            Behavior::Use => 1,
            Behavior::Touch => 2,
        };
        progs[i] = Some(vm::compile_behavior(f, n).map_err(tr)?);
    }
    let [tick, use_fn, touch] = progs;
    Ok(CompiledType { meta: module.meta, source: source.to_string(), wgsl, sdf, color, tick, use_fn, touch, prop_names: module.prop_names })
}

#[cfg(test)]
mod tests;
