//! `pocket listen`: hear the game's sound offline, as WAV files: built-in
//! demo scenes, one species' voice, or a place in a world (standing or
//! walking). For checking by ear, and for a model to check by spectrogram.

use super::call::{Call, Kind, Material, Part, guess, species_call};
use super::foley::Foley;
use super::mix::{Ambience, At, Cmd, Mixer, Play, Song};
use super::synth::Body;
use super::write_wav;
use anyhow::{Context, Result};
use glam::Vec3;
use std::path::{Path, PathBuf};

const SR: f32 = 48000.0;

pub const USAGE: &str = "  pocket listen --play                   a dog walks round you, through the speakers (checks the device)
  pocket listen --demo [--out DIR]       built-in scenes as WAVs (approach, materials, voices, night, contacts, fire)
  pocket listen FILE --species NAME [--out F.wav]          each of a species' sounds, 6 m ahead
  pocket listen FILE [--at x,z,yawDeg] [--hour H] [--secs S] [--walk] [--calls] [--verbose] [--out F.wav]
                                         stand (or walk) somewhere in a world and listen";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn out_dir(args: &[String]) -> PathBuf {
    flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| crate::log::dir().join("listen"))
}

pub fn cli(args: &[String]) -> Result<()> {
    if has(args, "--play") {
        return play_live();
    }
    if has(args, "--demo") {
        let dir = out_dir(args);
        std::fs::create_dir_all(&dir)?;
        for (name, st) in [("approach", approach()), ("materials", materials()), ("voices", voices()), ("night", night()), ("contacts", contacts()), ("fire", fire())] {
            let p = dir.join(format!("{name}.wav"));
            write_wav(&p, SR as u32, &st)?;
            println!("{}", p.display());
        }
        return Ok(());
    }
    let file = args.first().filter(|a| !a.starts_with("--")).context(format!("usage:\n{USAGE}"))?;
    let db = crate::db::Db::open(Path::new(file))?;
    let mut s = crate::sim::headless::Session::with_db(db, Some(1), None)?;
    if let Some(name) = flag(args, "--species") {
        let sp = s.sim.snap.species.list.iter().find(|x| x.name.eq_ignore_ascii_case(name.trim())).cloned().with_context(|| format!("no species {name} in this world"))?;
        let mut m = Mixer::new(SR);
        m.apply(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
        let mut t = 0.3;
        for i in 0..sp.sounds.len() {
            if let Some(call) = species_call(&sp, i) {
                println!("{}: {}", sp.sounds[i], serde_json::to_string(&call)?);
                let span = call.span();
                m.apply(Cmd::Play(Box::new(Play { at: At::Point(Vec3::new(0.0, 1.0, 6.0)), call, body: Body { mass: sp.mass, pitch: 1.0 }, seed: i as u64, gain: 1.0, delay: t })));
                t += span + 0.8;
            }
        }
        let st = m.render(t + 0.5);
        let p = flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| out_dir(&[]).join(format!("{}.wav", sp.name.replace(' ', "_"))));
        std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")))?;
        write_wav(&p, SR as u32, &st)?;
        println!("{}", p.display());
        return Ok(());
    }
    // A place: stand or walk, the world going on around.
    if let Some(at) = flag(args, "--at") {
        let v: Vec<f32> = at.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if v.len() >= 2 {
            let p = Vec3::new(v[0], s.sim.snap.terrain.height(v[0], v[1]), v[1]);
            s.sim.player.pos = p;
            s.sim.player.yaw = v.get(2).copied().unwrap_or(0.0).to_radians();
        }
    }
    if let Some(h) = flag(args, "--hour").and_then(|h| h.parse::<f64>().ok()) {
        let day = crate::render::sky::DAY_SECONDS;
        s.sim.t = (s.sim.t / day).floor() * day + h.rem_euclid(24.0) / 24.0 * day;
    }
    let secs: f32 = flag(args, "--secs").and_then(|x| x.parse().ok()).unwrap_or(15.0);
    let walk = has(args, "--walk");
    let calls = has(args, "--calls");
    let verbose = has(args, "--verbose");
    let mut m = Mixer::new(SR);
    let mut foley = Foley::default();
    let mut cmds = Vec::new();
    let dt = 1.0 / 60.0;
    let mut out = Vec::with_capacity((secs * SR) as usize * 2);
    let mut made = 0usize;
    let mut next_call = 1.0;
    let mut t = 0.0;
    while t < secs {
        if walk {
            let y = s.sim.player.yaw;
            s.sim.walk(crate::sim::ActorId::Player, Vec3::new(y.sin(), 0.0, y.cos()) * 3.0 * dt);
        }
        if calls && t >= next_call {
            next_call += 2.5;
            let me = s.sim.player.pos;
            let near: Vec<i64> = s.sim.cast.npcs.iter().filter(|n| n.here() && (n.a.pos - me).length() < 60.0).map(|n| n.def.id).collect();
            if !near.is_empty() {
                let c = near[(t * 7.0) as usize % near.len()];
                s.sim.make_noise(c, true);
            }
        }
        s.sim.step(dt);
        foley.frame(&mut s.sim, dt, &mut cmds);
        if verbose {
            for c in cmds.iter() {
                match c {
                    Cmd::Play(p) => {
                        let d = match p.at {
                            At::Point(v) | At::Follow(_, v) => (v - s.sim.player.pos).length(),
                            At::Listener => 0.0,
                        };
                        println!("{t:6.2} play {:?} {d:5.1} m mass {:.0}: {}", p.call.0.first().map(|x| x.kind), p.body.mass, serde_json::to_string(&p.call).unwrap_or_default())
                    }
                    Cmd::Ambience(a) => println!("{t:6.2} ambience wind {:.2} water {:?} dread {:.2} singers {} songs {} hush {}", a.wind, a.water.map(|w| w.1), a.dread, a.singers, a.songs.len(), a.hush.len()),
                    Cmd::Texture { key, drive, at, .. } if *drive > 0.0 && (t * 10.0) as i32 % 10 == 0 => println!("{t:6.2} texture {key:x} {at:?} drive {drive:.2}"),
                    _ => {}
                }
            }
        }
        for c in cmds.drain(..) {
            m.apply(c);
        }
        t += dt;
        let want = (t * SR) as usize * 2;
        if want > made {
            let mut buf = vec![0.0; want - made];
            m.process(&mut buf);
            out.extend_from_slice(&buf);
            made = want;
        }
    }
    let p = flag(args, "--out").map(PathBuf::from).unwrap_or_else(|| out_dir(&[]).join("place.wav"));
    std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")))?;
    write_wav(&p, SR as u32, &out)?;
    println!("{}", p.display());
    Ok(())
}

/// Drive a mixer through a scene: `each(t, mixer)` every 10 ms.
fn scene(secs: f32, mut each: impl FnMut(f32, &mut Mixer)) -> Vec<f32> {
    let mut m = Mixer::new(SR);
    let step = 0.01;
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < secs {
        each(t, &mut m);
        let mut buf = vec![0.0; (step * SR) as usize * 2];
        m.process(&mut buf);
        out.extend_from_slice(&buf);
        t += step;
    }
    out
}

/// The acceptance scene: a dog barks 30 m behind and to the left; the
/// listener turns round (4–5 s), then walks up to it (6–16 s).
fn approach() -> Vec<f32> {
    let src = Vec3::new(-8.0, 0.6, -30.0);
    let bark = guess("a happy bark", 25.0, false).unwrap_or_default();
    let mut next = 0.5;
    let mut k = 0;
    scene(18.0, |t, m| {
        let yaw = if t < 4.0 { 0.0 } else { std::f32::consts::PI * ((t - 4.0) / 1.0).min(1.0) };
        let face = (src - Vec3::ZERO).normalize();
        let walked = if t < 6.0 { 0.0 } else { ((t - 6.0) * 2.6).min(26.0) };
        let pos = Vec3::new(face.x, 0.0, face.z).normalize() * walked + Vec3::Y * 1.6;
        let yaw = if t < 5.0 { yaw } else { face.x.atan2(face.z) };
        m.apply(Cmd::Listener { pos, yaw });
        if t >= next {
            next += 1.6;
            k += 1;
            m.apply(Cmd::Play(Box::new(Play { at: At::Point(src), call: bark.clone(), body: Body { mass: 25.0, pitch: 1.0 }, seed: k, gain: 1.0, delay: 0.0 })));
        }
    })
}

/// Brushing through four materials (same code, four sliders each).
fn materials() -> Vec<f32> {
    let mats = [
        Material { hard: 0.10, dry: 0.15, ring: 0.0, leafy: 1.0 },
        Material { hard: 0.45, dry: 0.75, ring: 0.0, leafy: 1.0 },
        Material { hard: 0.75, dry: 0.95, ring: 0.0, leafy: 1.0 },
        Material { hard: 0.30, dry: 0.05, ring: 0.0, leafy: 1.0 },
    ];
    scene(22.0, |t, m| {
        m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw: 0.0 });
        let i = (t / 5.5) as usize;
        let local = t - i as f32 * 5.5;
        // Walking: a push every 0.55 s.
        let push = (1.0 - ((local % 0.55) / 0.35)).max(0.0);
        let drive = if local < 4.6 && i < 4 { 0.25 + 0.9 * push } else { 0.0 };
        m.apply(Cmd::Texture { key: 7 + i as u64, at: At::Listener, mat: mats[i.min(3)], size: 0.5, drive, roar: 0.0, gain: super::foley::UNDERFOOT });
    })
}

/// The built-in creatures' sounds (guessed from their words), each 6 m
/// ahead, with the bigger ones further left.
fn voices() -> Vec<f32> {
    let book = crate::world::species::SpeciesBook::builtin();
    let mut plays = Vec::new();
    let mut t = 0.3;
    for sp in book.list.iter() {
        for i in 0..sp.sounds.len() {
            if let Some(call) = species_call(sp, i) {
                let span = call.span();
                plays.push((t, sp.mass, call));
                t += span + 0.7;
            }
        }
    }
    // And a few the words alone can't make: insects, a bird's phrase, a trumpet.
    let extra: Vec<(f32, Call)> = vec![
        (0.05, Call(vec![Part { kind: Kind::Whistle, hz: [4400.0, 4350.0, 4300.0], len: 0.016, times: 4, gap: 0.015, ..Part::default() }])),
        (0.03, Call(vec![Part { kind: Kind::Whistle, hz: [4000.0, 3950.0, 3900.0], len: 0.3, swell: 0.15, ..Part::default() }, Part { kind: Kind::Whistle, hz: [3600.0, 3520.0, 3450.0], len: 0.4, swell: 0.1, ..Part::default() }])),
        (5000.0, guess("a loud trumpet", 5000.0, false).unwrap_or_default()),
        (5000.0, guess("a deep rumble", 5000.0, false).unwrap_or_default()),
    ];
    for (mass, call) in extra {
        let span = call.span();
        plays.push((t, mass, call));
        t += span + 0.7;
    }
    let total = t + 1.0;
    let mut m = Mixer::new(SR);
    m.apply(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
    for (k, (t, mass, call)) in plays.into_iter().enumerate() {
        let x = -(mass.max(1.0).log10() - 1.5) * 2.0;
        m.apply(Cmd::Play(Box::new(Play { at: At::Point(Vec3::new(x, 1.0, 6.0)), call, body: Body { mass, pitch: 1.0 }, seed: k as u64, gain: 1.0, delay: t })));
    }
    m.render(total)
}

/// Night: a cricket field; something breathes behind and to the left and
/// comes closer; the field falls quiet around it; the dread rises.
fn night() -> Vec<f32> {
    let crickets = vec![
        Song { part: Part { kind: Kind::Whistle, hz: [4500.0, 4450.0, 4390.0], len: 0.016, times: 4, gap: 0.015, breath: 0.02, swell: 0.05, loud: 0.25, ..Part::default() }, body: Body::default(), gain: 0.22, every: 0.7 },
        Song { part: Part { kind: Kind::Whistle, hz: [3600.0; 3], len: 0.011, times: 40, gap: 0.011, breath: 0.03, swell: 0.0, loud: 0.15, ..Part::default() }, body: Body::default(), gain: 0.16, every: 1.4 },
    ];
    let breath = guess("a long, slow, wet breath", 60.0, false).unwrap_or_default();
    let groan = guess("a low groan", 60.0, false).unwrap_or_default();
    let mut next = 1.0;
    let mut k = 0;
    scene(20.0, |t, m| {
        m.apply(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
        let d = 30.0 - 26.0 * (t / 17.0).min(1.0);
        let a = -2.7 + 0.9 * (t / 17.0).min(1.0);
        let at = Vec3::new(a.sin() * d, 1.6, a.cos() * d);
        if (t * 100.0) as i32 % 50 == 0 {
            m.apply(Cmd::Ambience(Box::new(Ambience { wind: 0.3, water: None, dread: (1.0 - d / 30.0).powf(1.5), songs: crickets.clone(), singers: 18, hush: vec![(at, 16.0)] })));
        }
        if t >= next {
            k += 1;
            let call = if k % 3 == 0 { groan.clone() } else { breath.clone() };
            next += call.span() + 1.2;
            m.apply(Cmd::Play(Box::new(Play { at: At::Point(at), call, body: Body { mass: 90.0, pitch: 0.9 }, seed: k, gain: 1.4, delay: 0.0 })));
        }
    })
}

/// Through the real device: a dog barking as it walks round the listener
/// (front, right, behind, left), for six seconds.
fn play_live() -> Result<()> {
    let out = super::start(0.8).context("no sound device (or POCKET_NO_SOUND is set)")?;
    out.send(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
    let bark = guess("a happy bark", 25.0, false).unwrap_or_default();
    let t0 = std::time::Instant::now();
    let mut k = 0u64;
    while t0.elapsed().as_secs_f32() < 6.5 {
        let a = t0.elapsed().as_secs_f32() / 6.0 * std::f32::consts::TAU;
        let at = Vec3::new(a.sin() * 5.0, 0.6, a.cos() * 5.0);
        k += 1;
        out.send(Cmd::Play(Box::new(Play { at: At::Point(at), call: bark.clone(), body: Body { mass: 25.0, pitch: 1.0 }, seed: k, gain: 1.0, delay: 0.0 })));
        println!("{:>5}", ["ahead", "right", "behind", "left"][((a / std::f32::consts::FRAC_PI_2 + 0.5) as usize) % 4]);
        std::thread::sleep(std::time::Duration::from_millis(750));
    }
    std::thread::sleep(std::time::Duration::from_millis(600));
    Ok(())
}

/// Things met and struck, by the same rules for all (each 2 m ahead, a
/// second apart): a body walks into stone, into wood, into a dry dead bush;
/// a stone lands on the ground; a clay pot hits it; a bell is struck.
fn contacts() -> Vec<f32> {
    use super::{Cue, Heard};
    use crate::sim::props::*;
    let props = |f: &dyn Fn(&mut Vec<f32>)| {
        let mut v = vec![0.0; 32];
        f(&mut v);
        v
    };
    let stone = Material::of(&props(&|_| {}), &[], None);
    let wood = Material::of(&props(&|p| p[P_BURNS] = 0.6), &[], None);
    let dead_bush = Material::of(&props(&|p| p[P_BURNS] = 0.8), &["bush".into()], None);
    let pot = Material::of(&props(&|p| p[P_FRAGILE] = 1.0), &[], None);
    let bell = Material::from_meta(&[("hard".into(), 0.95), ("ring".into(), 1.0), ("dry".into(), 0.5)]).unwrap_or_default();
    let flesh = Some((Material::FLESH, 70.0));
    let hits = [
        (stone, 4000.0, 2.5, flesh),
        (wood, 600.0, 2.5, flesh),
        (dead_bush, 30.0, 2.5, flesh),
        (stone, 0.7, 7.0, None),
        (pot, 1.2, 5.0, None),
        (bell, 40.0, 3.0, None),
    ];
    let mut m = Mixer::new(SR);
    m.apply(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
    for (k, (mat, mass, speed, by)) in hits.into_iter().enumerate() {
        let cue = Cue { at: Vec3::new(0.0, 0.8, 2.0), from: None, what: Heard::Hit { mat, mass, speed, by } };
        if let Some(mut p) = super::foley::play_of(cue, k as u64) {
            p.delay = 0.4 + k as f32 * 1.3;
            m.apply(Cmd::Play(Box::new(p)));
        }
    }
    m.render(8.5)
}

/// A fire 6 m ahead and to the left, growing, then dying down.
fn fire() -> Vec<f32> {
    scene(12.0, |t, m| {
        m.apply(Cmd::Listener { pos: Vec3::Y * 1.6, yaw: 0.0 });
        let drive = if t < 8.0 { (t / 3.0).min(1.0) * 1.1 } else { (1.0 - (t - 8.0) / 4.0).max(0.0) * 1.1 };
        m.apply(Cmd::Texture { key: 99, at: At::Point(Vec3::new(-3.0, 0.8, 5.0)), mat: Material::FIRE, size: 0.3, drive, roar: 0.8, gain: 1.0 });
    })
}
