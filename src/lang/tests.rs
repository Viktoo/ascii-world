use super::*;
use crate::lang::probe::{default_k, probe};

fn fixture(path: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(path);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Malicious code must die at the allowlist step: never translated, never run.
fn assert_rejected_at_allowlist(name: &str, expect: &str) {
    let src = fixture(&format!("malicious/{name}.js"));
    let diags = check::check(&src).expect_err(name);
    assert!(
        diags.iter().all(|d| d.stage == Stage::Allowlist),
        "{name}: expected only allowlist errors, got:\n{}",
        format_diags(&diags)
    );
    assert!(
        diags.iter().any(|d| d.msg.contains(expect) && d.line > 0),
        "{name}: expected a line-numbered error mentioning {expect:?}, got:\n{}",
        format_diags(&diags)
    );
    assert!(compile(&src).is_err());
}

#[test]
fn malicious_fixtures_rejected_at_allowlist() {
    assert_rejected_at_allowlist("while_true", "while loops are not allowed");
    assert_rejected_at_allowlist("fetch", "'fetch' is not an allowed function");
    assert_rejected_at_allowlist("process_exit", "unknown identifier 'process'");
    assert_rejected_at_allowlist("constructor_escape", "'this' is not allowed");
    assert_rejected_at_allowlist("dynamic_import", "import() is not allowed");
    assert_rejected_at_allowlist("unbounded_for", "for loops need literal bounds");
    assert_rejected_at_allowlist("variable_bound_for", "for loops need literal bounds");
    assert_rejected_at_allowlist("huge_for", "more than 32 times");
    assert_rejected_at_allowlist("recursion", "recursion");
    assert_rejected_at_allowlist("helper_recursion", "helper function 'f' is not allowed");
    assert_rejected_at_allowlist("unknown_identifier", "unknown identifier 'globalThis'");
    assert_rejected_at_allowlist("closure", "closures are not allowed");
    assert_rejected_at_allowlist("new_object", "'new' is not allowed");
    assert_rejected_at_allowlist("class", "classes are not allowed");
    assert_rejected_at_allowlist("string_outside_meta", "strings are only allowed in meta");
    assert_rejected_at_allowlist("static_import", "import is not allowed");
    assert_rejected_at_allowlist("async", "async");
    assert_rejected_at_allowlist("computed_member", "computed member access");
}

#[test]
fn every_malicious_fixture_is_covered() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/malicious");
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let src = std::fs::read_to_string(&p).unwrap();
        let d = check::check(&src).expect_err(&p.display().to_string());
        assert!(d.iter().all(|d| d.stage == Stage::Allowlist), "{}: {}", p.display(), format_diags(&d));
    }
}

#[test]
fn syntax_error_reports_line() {
    let d = compile(&fixture("broken/syntax_error.js")).expect_err("syntax");
    assert_eq!(d[0].stage, Stage::Parse);
    assert_eq!(d[0].line, 3, "{}", format_diags(&d));
}

#[test]
fn missing_return_path_rejected() {
    let d = compile(&fixture("broken/color_missing_return.js")).expect_err("no return");
    assert!(d.iter().any(|d| d.stage == Stage::Allowlist && d.msg.contains("color() can reach its end")), "{}", format_diags(&d));
}

#[test]
fn probe_catches_broken_shapes() {
    for (name, expect) in [("nan_output", "NaN"), ("outside_bounds", "outside meta.bounds"), ("empty_shape", "shape is empty")] {
        let t = compile(&fixture(&format!("broken/{name}.js"))).unwrap_or_else(|d| panic!("{name}: {}", format_diags(&d)));
        let d = probe(&t).expect_err(name);
        assert!(d.iter().all(|d| d.stage == Stage::Probe));
        assert!(d.iter().any(|d| d.msg.contains(expect)), "{name}: {}", format_diags(&d));
    }
}

#[test]
fn spec_lighthouse_passes() {
    let t = compile(&fixture("good/lighthouse.js")).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    assert_eq!(t.meta.name, "lighthouse");
    assert_eq!(t.meta.tags, vec!["building", "landmark"]);
    let r = probe(&t).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    assert!((r.bottom - 0.0).abs() < 0.05, "bottom {}", r.bottom);
    // Inside the tower and in the lamp.
    let k = default_k(0.0);
    assert!(t.sdf([0.0, 3.0, 0.0], &k) < 0.0);
    assert!(t.sdf([0.0, 12.5, 0.0], &k) < 0.0);
    assert!(t.sdf([5.0, 3.0, 0.0], &k) > 0.0);
    let c = t.color([0.0, 12.0, 0.0], &k);
    assert!((c[0] - 1.0).abs() < 1e-6 && (c[1] - 230.0 / 255.0).abs() < 1e-6);
    let stripe_a = t.color([1.0, 0.5, 0.0], &k);
    let stripe_b = t.color([1.0, 2.5, 0.0], &k);
    assert_ne!(stripe_a, stripe_b);
    assert!(t.wgsl.contains("fn sdf_T__TID__"));
}

#[test]
fn kitchen_sink_compiles_and_probes() {
    let t = compile(&fixture("good/kitchen_sink.js")).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    probe(&t).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
}

#[test]
fn type_errors_are_reported() {
    let src = r#"
export const meta = { name: "x", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  const c = rgb(1, 2, 3);
  return c;
}
export function color(x, y, z, k) { return 1.0; }
"#;
    let d = compile(src).expect_err("types");
    assert!(d.iter().any(|d| d.line == 5 && d.msg.contains("must return a number")), "{}", format_diags(&d));
    assert!(d.iter().any(|d| d.line == 7 && d.msg.contains("must return a colour")), "{}", format_diags(&d));
}

#[test]
fn const_and_loop_rules() {
    let src = r#"
export const meta = { name: "x", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  const a = 1;
  a = 2;
  for (let i = 0; i < 4; i++) { i = 3; }
  x = 4;
  return sphere(x, y, z, 0.5);
}
export function color(x, y, z, k) { return rgb(1, 2, 3); }
"#;
    let d = compile(src).expect_err("rules");
    assert!(d.iter().any(|d| d.line == 5 && d.msg.contains("is const")));
    assert!(d.iter().any(|d| d.line == 6 && d.msg.contains("loop variable")));
    assert!(d.iter().any(|d| d.line == 7 && d.msg.contains("parameter 'x'")));
}

#[test]
fn loops_match_js_semantics() {
    let src = r#"
export const meta = { name: "x", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  let s = 0;
  for (let i = 0; i <= 10; i += 2) { s += i; }
  for (let j = 3; j > 0; j--) { s += j * 100; }
  return s;
}
export function color(x, y, z, k) { return rgb(1, 2, 3); }
"#;
    let t = compile(src).unwrap();
    // 0+2+4+6+8+10 = 30, plus (3+2+1)*100
    assert_eq!(t.sdf([0.0; 3], &default_k(0.0)), 630.0);
}

#[test]
fn vm_never_panics_on_weird_inputs() {
    let t = compile(&fixture("good/kitchen_sink.js")).unwrap();
    let weird = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, f32::MIN, 0.0, -0.0, 1e-38, 1e30];
    for &a in &weird {
        for &b in &weird {
            let k = [a, b, a, b, a, b, a, b, b, a, b, a, b, a, b, a];
            let _ = t.sdf_checked([a, b, a], &k, 100_000);
            let _ = t.color_checked([b, a, b], &k, 100_000);
            let _ = t.sdf_checked([a, b, a], &k, 3);
        }
    }
}

/// Random expression soup: the checker + VM must never panic, whatever the input.
#[test]
fn fuzz_checker_and_vm() {
    let atoms = ["x", "y", "z", "k.a", "k.seed", "1.5", "-2", "PI", "q", "true", "(x>y)"];
    let calls = ["sphere", "min", "mix", "rgb", "rotY", "hash", "floor", "smoothUnion", "pow", "sqrt", "clamp", "nope", "length3"];
    let ops = ["+", "-", "*", "/", "%", "**", "<", "&&", "||", "==", "?"];
    let mut s = 12345u32;
    let mut rnd = |n: usize| {
        s = crate::noise::pcg(s);
        (s as usize) % n
    };
    for _ in 0..1500 {
        fn genx(rnd: &mut dyn FnMut(usize) -> usize, d: u32, atoms: &[&str], calls: &[&str], ops: &[&str]) -> String {
            if d == 0 || rnd(3) == 0 {
                return atoms[rnd(atoms.len())].to_string();
            }
            match rnd(3) {
                0 => {
                    let n = 1 + rnd(4);
                    let args: Vec<String> = (0..n).map(|_| genx(rnd, d - 1, atoms, calls, ops)).collect();
                    format!("{}({})", calls[rnd(calls.len())], args.join(", "))
                }
                1 => {
                    let op = ops[rnd(ops.len())];
                    if op == "?" {
                        format!("({} ? {} : {})", genx(rnd, d - 1, atoms, calls, ops), genx(rnd, d - 1, atoms, calls, ops), genx(rnd, d - 1, atoms, calls, ops))
                    } else {
                        format!("({} {op} {})", genx(rnd, d - 1, atoms, calls, ops), genx(rnd, d - 1, atoms, calls, ops))
                    }
                }
                _ => format!("-{}", genx(rnd, d - 1, atoms, calls, ops)),
            }
        }
        let e1 = genx(&mut rnd, 4, &atoms, &calls, &ops);
        let e2 = genx(&mut rnd, 3, &atoms, &calls, &ops);
        let src = format!(
            "export const meta = {{ name: \"f\", bounds: [1, 1, 1], tags: [] }};\n\
             export function sdf(x, y, z, k) {{ const q = rotX(x, y, z, 0.5); let v = {e1}; if ({e2}) {{ v = v * 2; }} return v; }}\n\
             export function color(x, y, z, k) {{ return rgb(10, 20, 30); }}\n"
        );
        if let Ok(t) = compile(&src) {
            assert!(t.wgsl.contains("sdf_T"));
            for p in [[0.0, 0.0, 0.0], [1.0, -2.0, 3.0], [f32::NAN, 1.0, 1.0]] {
                let _ = t.sdf_checked(p, &default_k(0.5), 10_000);
            }
        }
    }
}

const LANTERN: &str = r#"
export const meta = {
  name: "lantern",
  bounds: [0.2, 0.3, 0.2],
  tags: ["item", "light"],
  props: { mass: 1.2, light: 0, heat: 0, fragile: 0.5, lit: true },
  says: ["The flame steadies."],
  sounds: ["clink"],
  spawns: ["ash"],
};
export function sdf(x, y, z, k) {
  return roundBox(x, y, z, 0.15, 0.25 - k.s1 * 0.01, 0.15, 0.03);
}
export function color(x, y, z, k) {
  return mix(rgb(60, 50, 40), rgb(255, 210, 120), k.s0);
}
export function tick(s, w, k) {
  s.s1 += w.dt;
  if (w.fire > 0 && w.wet < 0.5) {
    s.s0 = 1;
  }
  w.light = s.s0;
  w.heat = s.s0 * 300;
  if (w.water > 0) {
    s.s0 = 0;
    sound(0);
    return;
  }
  if (s.s1 > 100) { remove(); }
}
export function use(s, w, k, o) {
  s.s0 = 1 - s.s0;
  if (w.on > 0) {
    o.temp += 200 * s.s0;
  }
  say(0);
}
export function touch(s, w, k) {
  if (w.impact > 4) { w.health -= 0.5; spawn(0); }
}
"#;

fn run(t: &CompiledType, b: crate::lang::ir::Behavior, state: &mut [f32; 8], ctx: &[(&str, f32)], props: &mut Vec<f32>, other: &mut Vec<f32>) -> Vec<(crate::lang::ir::Effect, f32)> {
    use crate::lang::ir::CTX_FIELDS;
    let mut c = [0.0f32; crate::lang::vm::CTX_LEN];
    for (n, v) in ctx {
        c[CTX_FIELDS.iter().position(|f| f == n).unwrap()] = *v;
    }
    let mut effects = Vec::new();
    let io = crate::lang::vm::BehaviorIo { k: &default_k(0.0), state, ctx: &c, props, other, effects: &mut effects };
    t.run_behavior(b, io, BEHAVIOR_FUEL).unwrap().unwrap();
    effects
}

#[test]
fn behaviour_code_reads_and_writes_state_props_and_effects() {
    use crate::lang::ir::{Behavior, Effect};
    let t = compile(LANTERN).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    assert_eq!(t.meta.props.iter().find(|p| p.0 == "lit").map(|p| p.1), Some(1.0));
    assert_eq!(t.meta.says, vec!["The flame steadies."]);
    assert_eq!(t.meta.spawns, vec!["ash"]);
    assert!(t.tick.is_some() && t.use_fn.is_some() && t.touch.is_some());
    probe(&t).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    let idx = |n: &str| t.prop_names.iter().position(|p| p == n).unwrap();
    let mut state = [0.0; 8];
    let mut props = vec![0.0; t.prop_names.len()];
    let mut other = vec![0.0; t.prop_names.len()];
    // Lit by fire: light and heat follow.
    props[idx("fire")] = 1.0;
    let fx = run(&t, Behavior::Tick, &mut state, &[("dt", 0.5)], &mut props, &mut other);
    assert!(fx.is_empty());
    assert_eq!(state[0], 1.0);
    assert_eq!(state[1], 0.5);
    assert_eq!(props[idx("light")], 1.0);
    assert_eq!(props[idx("heat")], 300.0);
    // Dropped in water: goes out, makes its sound, and `return;` stops the rest.
    let fx = run(&t, Behavior::Tick, &mut state, &[("dt", 0.5), ("water", 1.0)], &mut props, &mut other);
    assert_eq!(state[0], 0.0);
    assert_eq!(fx, vec![(Effect::Sound, 0.0)]);
    // Used on something: heats the other thing.
    other[idx("temp")] = 15.0;
    let fx = run(&t, Behavior::Use, &mut state, &[("on", 1.0)], &mut props, &mut other);
    assert_eq!(state[0], 1.0);
    assert_eq!(other[idx("temp")], 215.0);
    assert_eq!(fx, vec![(Effect::Say, 0.0)]);
    // Hit hard: loses health and spawns ash.
    props[idx("health")] = 1.0;
    let fx = run(&t, Behavior::Touch, &mut state, &[("impact", 6.0)], &mut props, &mut other);
    assert_eq!(props[idx("health")], 0.5);
    assert_eq!(fx, vec![(Effect::Spawn, 0.0)]);
    // State is visible to the shape as k.s0 … k.s7.
    let mut k = default_k(0.0);
    k[8] = 1.0;
    let c = t.color([0.0, 0.0, 0.0], &k);
    assert!((c[0] - 1.0).abs() < 1e-6);
    assert!(t.wgsl.contains("k.s0") && t.wgsl.contains("k.s1"));
    assert!(!t.wgsl.contains("tick"), "behaviour stays on the CPU");
}

#[test]
fn behaviour_rules_are_enforced() {
    let base = |body: &str| {
        format!(
            "export const meta = {{ name: \"x\", bounds: [1, 1, 1], tags: [], says: [\"hi\"] }};\n\
             export function sdf(x, y, z, k) {{ return sphere(x, y, z, 0.5); }}\n\
             export function color(x, y, z, k) {{ return rgb(1, 2, 3); }}\n{body}"
        )
    };
    let cases = [
        ("export function tick(s, w, k) { return 1; }", "does not return a value"),
        ("export function tick(s, w, k) { w.dt = 1; }", "read-only"),
        ("export function tick(s, w, k) { k.a = 1; }", "read-only"),
        ("export function tick(s, w, k) { s.s9 = 1; }", "state slots are"),
        ("export function tick(s, w, k) { say(3); }", "needs meta.says to have an entry 3"),
        ("export function tick(s, w, k) { remove(1); }", "takes no arguments"),
        ("export function tick(s, w, k) { const a = s; }", "can only be used as s.<slot>"),
        ("export function tick(s, w, k, o) { }", "must take (s, w, k)"),
        ("export function think(s, w, k) { }", "may be exported"),
        ("export function tick(s, w, k) { w.Fire = 1; }", "not a property name"),
        ("export function tick(s, w, k) { fetch(1); }", "'fetch' is not an allowed function"),
    ];
    for (body, expect) in cases {
        let d = compile(&base(body)).expect_err(body);
        assert!(d.iter().any(|d| d.msg.contains(expect)), "{body}: expected {expect:?}, got {}", format_diags(&d));
    }
    // Shape functions still cannot assign members or call effects.
    let src = "export const meta = { name: \"x\", bounds: [1, 1, 1], tags: [] };\n\
               export function sdf(x, y, z, k) { k.a = 2; return 1; }\n\
               export function color(x, y, z, k) { remove(); return rgb(1, 2, 3); }";
    let d = compile(src).expect_err("shape effects");
    assert!(d.iter().any(|d| d.msg.contains("only local variables can be assigned")), "{}", format_diags(&d));
    assert!(d.iter().any(|d| d.msg.contains("'remove' is not an allowed function")), "{}", format_diags(&d));
    // Bad meta.props.
    for (props, expect) in [("{ mass: \"heavy\" }", "literal number"), ("{ Mass: 1 }", "not a valid property name"), ("{ dt: 1 }", "not a valid property name")] {
        let src = format!("export const meta = {{ name: \"x\", bounds: [1, 1, 1], tags: [], props: {props} }};\n\
               export function sdf(x, y, z, k) {{ return sphere(x, y, z, 0.5); }}\n\
               export function color(x, y, z, k) {{ return rgb(1, 2, 3); }}");
        let d = compile(&src).expect_err(props);
        assert!(d.iter().any(|d| d.msg.contains(expect)), "{props}: {}", format_diags(&d));
    }
}

#[test]
fn expensive_or_nan_behaviour_fails_the_probe() {
    let src = r#"
export const meta = { name: "x", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) { return sphere(x, y, z, 0.5); }
export function color(x, y, z, k) { return rgb(1, 2, 3); }
export function tick(s, w, k) {
  for (let i = 0; i < 32; i++) { for (let j = 0; j < 8; j++) { s.s0 = s.s0 + sin(i * j + s.s0) * cos(j) + noise3(i, j, s.s1) + hash(i, j); } }
}
"#;
    let t = compile(src).unwrap_or_else(|d| panic!("{}", format_diags(&d)));
    let d = probe(&t).expect_err("too expensive");
    assert!(d[0].msg.contains("too expensive"), "{}", format_diags(&d));
    let src = r#"
export const meta = { name: "x", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) { return sphere(x, y, z, 0.5); }
export function color(x, y, z, k) { return rgb(1, 2, 3); }
export function tick(s, w, k) { s.s2 = 1 / (w.dt - w.dt); }
"#;
    let t = compile(src).unwrap();
    let d = probe(&t).expect_err("nan");
    assert!(d[0].msg.contains("s.s2"), "{}", format_diags(&d));
}
