//! Assemble the full WGSL: base shader + every active type + dispatch switch.

use crate::lang::{CompiledType, api::API_WGSL, wgsl::ID};
use std::fmt::Write;

pub const COMMON: &str = include_str!("../shaders/common.wgsl");
pub const SCENE: &str = include_str!("../shaders/scene.wgsl");

/// `types` is (shader type id, compiled type). Ids must be unique.
pub fn assemble(types: &[(u32, &CompiledType)]) -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str(&crate::noise::octave_wgsl());
    s.push_str(COMMON);
    s.push_str(API_WGSL);
    for (id, t) in types {
        let _ = writeln!(s, "// type {id}: {}", t.meta.name.replace(['\n', '\r'], " "));
        s.push_str(&t.wgsl.replace(ID, &id.to_string()));
    }
    s.push_str("fn type_sdf(tid: u32, p: vec3f, k: Params) -> f32 {\n  switch tid {\n");
    for (id, _) in types {
        let _ = writeln!(s, "    case {id}u: {{ return sdf_T{id}(p, k); }}");
    }
    s.push_str("    default: { return 1e9; }\n  }\n}\n");
    s.push_str("fn type_color(tid: u32, p: vec3f, k: Params) -> vec3f {\n  switch tid {\n");
    for (id, _) in types {
        let _ = writeln!(s, "    case {id}u: {{ return color_T{id}(p, k); }}");
    }
    s.push_str("    default: { return vec3f(1.0, 0.0, 1.0); }\n  }\n}\n");
    // Types that mark their lit parts with glow(); the rest glow all over.
    s.push_str("fn type_marks_glow(tid: u32) -> bool {\n  switch tid {\n");
    let marked: Vec<String> = types.iter().filter(|(_, t)| t.marks_glow()).map(|(id, _)| format!("{id}u")).collect();
    if !marked.is_empty() {
        let _ = writeln!(s, "    case {}: {{ return true; }}", marked.join(", "));
    }
    s.push_str("    default: { return false; }\n  }\n}\n");
    s.push_str(SCENE);
    s
}

/// Validate WGSL with naga; returns a readable error.
pub fn validate(src: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| e.emit_to_string(src))?;
    let mut v = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default());
    v.validate(&module).map_err(|e| format!("{e:?}"))?;
    Ok(())
}

/// Stable key for a set of types (pipeline cache key).
pub fn key(types: &[(u32, &CompiledType)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (id, t) in types {
        id.hash(&mut h);
        t.source.hash(&mut h);
    }
    h.finish()
}
