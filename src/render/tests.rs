use super::gpu::Gpu;
use super::*;
use crate::lang::{CompiledType, compile};
use crate::model::{BUILTIN_SOURCES, gpu_parity, probe_globals};
use crate::terrain::default_biomes;

fn all_types() -> Vec<(u32, CompiledType)> {
    let mut v: Vec<(u32, CompiledType)> = BUILTIN_SOURCES.iter().enumerate().map(|(i, s)| (i as u32 + 1, compile(s).unwrap())).collect();
    for (i, f) in ["good/lighthouse.js", "good/kitchen_sink.js"].iter().enumerate() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(f);
        v.push((100 + i as u32, compile(&std::fs::read_to_string(p).unwrap()).unwrap()));
    }
    v
}

#[test]
fn assembled_shader_validates() {
    let types = all_types();
    let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let src = shader::assemble(&refs);
    if let Err(e) = shader::validate(&src) {
        std::fs::write(std::env::temp_dir().join("pocket-test.wgsl"), &src).ok();
        panic!("{e}");
    }
    // Terrain-only shader also validates.
    shader::validate(&shader::assemble(&[])).unwrap();
}

/// CPU/GPU parity for every built-in and fixture type, and for terrain.
#[test]
fn gpu_parity_types_and_terrain() {
    let Ok(gpu) = Gpu::new() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let types = all_types();
    let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let pipe = gpu.build_pipeline(&shader::assemble(&refs), 1).unwrap();
    let terrain = crate::terrain::Terrain::new(1234, default_biomes());
    for (id, t) in &types {
        gpu_parity(&gpu, &pipe, &terrain, *id, t).unwrap_or_else(|d| panic!("{}", crate::lang::format_diags(&d)));
    }
    // Terrain: 2000 points spread over a few km.
    let mut pts = Vec::new();
    for i in 0..2000 {
        let a = i as f32 * 2.399;
        let r = (i as f32).sqrt() * 200.0;
        pts.push([a.cos() * r, 0.0, a.sin() * r, 0.0]);
    }
    let out = gpu.probe(&pipe, &probe_globals(&terrain), 1, 0, &GpuInst::default(), &pts).unwrap();
    let mut worst = 0.0f32;
    for (p, g) in pts.iter().zip(&out) {
        let c = terrain.height(p[0], p[2]);
        worst = worst.max((g[0] - c).abs());
    }
    assert!(worst <= 1e-3, "terrain parity: worst abs error {worst}");
    eprintln!("terrain parity worst error {worst:e}");
}

/// Cuts take the same pieces out on the GPU as on the CPU.
#[test]
fn cuts_agree_on_gpu_and_cpu() {
    let Ok(gpu) = Gpu::new() else { return };
    let types = all_types();
    let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let pipe = gpu.build_pipeline(&shader::assemble(&refs), 2).unwrap();
    let terrain = crate::terrain::Terrain::new(7, default_biomes());
    let (tid, ct) = (100, &types.iter().find(|(i, _)| *i == 100).unwrap().1);
    let mut inst = GpuInst { k0: [0.37, 1.0, 0.5, 0.5], k1: [0.5; 4], ..Default::default() };
    inst.cuts = [[0.0, 3.0, 1.0, 0.8], [0.5, 6.0, 0.0, -0.6], [0.0; 4], [-1.0, 1.0, -1.0, 0.3]];
    let pts = crate::lang::probe::sample_points(ct.meta.bounds);
    let input: Vec<[f32; 4]> = pts.iter().map(|p| [p[0], p[1], p[2], 0.0]).collect();
    let out = gpu.probe(&pipe, &probe_globals(&terrain), 0, tid, &inst, &input).unwrap();
    let k = inst.k();
    let mut worst = 0.0f32;
    let mut changed = 0;
    for (p, g) in pts.iter().zip(&out) {
        let plain = ct.sdf(*p, &k);
        let c = apply_cuts(plain, &inst.cuts, *p);
        if c > plain + 1e-4 {
            changed += 1;
        }
        worst = worst.max((g[0] - c).abs() / c.abs().max(1.0));
    }
    assert!(changed > 3, "the cuts remove something ({changed} points)");
    assert!(worst <= crate::model::PARITY_TOL, "GPU/CPU cut mismatch {worst:e}");
}

/// A tilted, turned, scaled shape is the same shape on the GPU and the CPU,
/// and so is ground taken away by a hollow.
#[test]
fn tilt_and_hollows_agree_on_gpu_and_cpu() {
    let Ok(gpu) = Gpu::new() else { return };
    let types = all_types();
    let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let pipe = gpu.build_pipeline(&shader::assemble(&refs), 3).unwrap();
    let terrain = crate::terrain::Terrain::new(11, default_biomes());
    let (tid, ct) = (100, &types.iter().find(|(i, _)| *i == 100).unwrap().1);
    let mut inst = GpuInst { pos_scale: [4.0, 2.0, -3.0, 1.3], rot: [0.7f32.cos(), 0.7f32.sin(), 10.0, 1.0], k0: [0.37, 1.3, 0.5, 0.5], k1: [0.5; 4], info: [tid, 1.5f32.to_bits(), 0, 0], ..Default::default() };
    inst.set_tilt(glam::Quat::from_rotation_x(0.9) * glam::Quat::from_rotation_z(-0.5));
    let pts: Vec<[f32; 4]> = crate::lang::probe::sample_points(ct.meta.bounds).iter().map(|p| {
        let w = inst.from_local(glam::Vec3::from_array(*p));
        [w.x, w.y, w.z, 0.0]
    }).collect();
    let out = gpu.probe(&pipe, &probe_globals(&terrain), 3, tid, &inst, &pts).unwrap();
    let mut worst = 0.0f32;
    for (p, g) in pts.iter().zip(&out) {
        let c = inst.sdf(ct, glam::Vec3::new(p[0], p[1], p[2]));
        worst = worst.max((g[0] - c).abs() / c.abs().max(1.0));
    }
    assert!(worst <= crate::model::PARITY_TOL, "GPU/CPU tilted sdf mismatch {worst:e}");
    let c = inst.center();
    assert!((glam::Vec3::new(out[0][1], out[0][2], out[0][3]) - c).length() < 1e-3, "the middle agrees");
    // A box hollow and a round pit near the origin.
    {
        let mut cv = terrain.carve.write();
        let h = terrain.natural_height(3.0, 2.0);
        cv.fixed.push(crate::terrain::Hollow { c: [3.0, 2.0], rot: [0.8f32.cos(), 0.8f32.sin()], half: [2.0, 1.2], floor: h - 2.0, round: false });
        cv.dug.push(crate::terrain::Hollow { c: [-4.0, 1.0], rot: [1.0, 0.0], half: [0.6, 0.6], floor: terrain.natural_height(-4.0, 1.0) - 0.7, round: true });
        cv.reindex();
    }
    let mut pts = Vec::new();
    for i in 0..400 {
        pts.push([-7.0 + (i % 20) as f32 * 0.7, 0.0, -5.0 + (i / 20) as f32 * 0.6, 0.0]);
    }
    let out = gpu.probe(&pipe, &probe_globals(&terrain), 1, 0, &GpuInst::default(), &pts).unwrap();
    let mut worst = 0.0f32;
    let mut dug = 0;
    for (p, g) in pts.iter().zip(&out) {
        let c = terrain.height(p[0], p[2]);
        if c < terrain.natural_height(p[0], p[2]) - 0.1 {
            dug += 1;
        }
        worst = worst.max((g[0] - c).abs());
    }
    assert!(dug > 10, "the hollows take ground away ({dug} points)");
    assert!(worst <= 1e-3, "terrain with hollows: worst abs error {worst}");
}

#[test]
fn renders_a_frame_quickly() {
    let Ok(gpu) = Gpu::new() else { return };
    let types = all_types();
    let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let pipe = gpu.build_pipeline(&shader::assemble(&refs), 1).unwrap();
    let terrain = crate::terrain::Terrain::new(7, default_biomes());
    let pal = crate::terrain::Palette::default();
    let h = terrain.height(0.0, 0.0);
    let cam = Camera { pos: glam::Vec3::new(0.0, h.max(0.0) + 1.7, 0.0), yaw: 0.3, pitch: -0.05, fov_y: 1.0 };
    // A lighthouse ahead of the camera.
    let lid = 100;
    let lz = 30.0;
    let inst = GpuInst {
        pos_scale: [0.0, terrain.height(0.0, lz), lz, 1.0],
        rot: [1.0, 0.0, 14.7, 1.0],
        k0: [0.0, 1.0, 0.5, 0.5],
        k1: [0.5; 4],
        info: [lid, 0, 0, 0],
        ..Default::default()
    };
    let (w, hgt) = (250u32, 140u32);
    let sp = SceneParams { terrain: &terrain, palette: &pal, camera: cam, width: w, height: hgt, pixel_aspect: 1.0, light: sky::lighting(500.0, &pal), time: 1.0, frame: 0, shadows: true, lights: &[], weather: Default::default() };
    let scene = std::sync::Arc::new(crate::world::SceneTypes { types: Default::default(), pipeline: Some(pipe) });
    let mut handle = gpu::spawn(gpu.clone());
    let mut times = Vec::new();
    for i in 0..20 {
        let req = FrameRequest { id: i, width: w, height: hgt, globals: build_globals(&sp, 1, None), instances: vec![inst], grid: None, scene: scene.clone(), terrain: std::sync::Arc::new(terrain.clone()), look: Default::default() };
        handle.tx.send(RenderMsg::Frame(Box::new(req))).unwrap();
        let f = handle.rx.recv_timeout(std::time::Duration::from_secs(20)).unwrap();
        assert_eq!(f.pixels.len(), (w * hgt) as usize);
        times.push(f.gpu_ms.unwrap_or(0.0));
        if i == 19 {
            let distinct: std::collections::HashSet<u32> = f.pixels.iter().copied().collect();
            assert!(distinct.len() > 50, "frame looks blank");
            // the lighthouse's red/white stripes should be on screen
            let reddish = f.pixels.iter().filter(|p| { let c = unpack(**p); c[0] > 120 && c[1] < 90 && c[2] < 90 }).count();
            assert!(reddish > 5, "no lighthouse red found");
        }
    }
    handle.shutdown();
    times.sort_by(|a, b| a.total_cmp(b));
    eprintln!("GPU ms at 250x140: median {:.2}", times[times.len() / 2]);
}

#[test]
fn terrain_debug() {
    let Ok(gpu) = Gpu::new() else { return };
    let pipe = gpu.build_pipeline(&shader::assemble(&[]), 1).unwrap();
    let terrain = crate::terrain::Terrain::new(1234, default_biomes());
    let mut pts = Vec::new();
    for i in 0..2000 {
        let a = i as f32 * 2.399;
        let r = (i as f32).sqrt() * 60.0;
        pts.push([a.cos() * r, 0.0, a.sin() * r, 0.0]);
    }
    let out = gpu.probe(&pipe, &probe_globals(&terrain), 2, 0, &GpuInst::default(), &pts).unwrap();
    let mut worst = [0.0f32; 4];
    for (p, g) in pts.iter().zip(&out) {
        let (x, z) = (p[0], p[2]);
        let n = crate::noise::fbm2(x * 0.0045 + terrain.off[4], z * 0.0045 + terrain.off[5], terrain.seed.wrapping_add(101), 5);
        let r = crate::noise::ridged2(x * 0.0032 + terrain.off[5], z * 0.0032 + terrain.off[4], terrain.seed.wrapping_add(202), 3);
        let w = terrain.weights(x, z);
        let c = [n, r, w[0], w[1]];
        for k in 0..4 { worst[k] = worst[k].max((g[k] - c[k]).abs()); }
    }
    eprintln!("worst fbm {:e} ridged {:e} w0 {:e} w1 {:e}", worst[0], worst[1], worst[2], worst[3]);
}

/// How pipeline build time scales with the number of types (prints only).
#[test]
#[ignore]
fn pipeline_build_scaling() {
    let Ok(gpu) = Gpu::new() else { return };
    let base = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/good/kitchen_sink.js")).unwrap();
    for n in [10usize, 40, 100] {
        let types: Vec<(u32, CompiledType)> = (0..n).map(|i| (i as u32 + 1, compile(&base.replace("0.3 + k.seed", &format!("{}.{} + k.seed", i, std::process::id()))).unwrap())).collect();
        let refs: Vec<(u32, &CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
        let t0 = std::time::Instant::now();
        gpu.build_pipeline(&shader::assemble(&refs), n as u64 + 1000).unwrap();
        eprintln!("{n} types: {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    }
}
