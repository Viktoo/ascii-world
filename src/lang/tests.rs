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
            let k = [a, b, a, b, a, b, a, b];
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
