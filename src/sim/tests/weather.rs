//! Weather in the living world (docs/weather-plan.md): what falls acts on
//! things through the rules, bodies feel it and act on it, it turning is
//! news, and it can be made on purpose.

use super::*;
use crate::world::weather::{self, Kind, HOUR};

/// Make it `kind` from well before now, so the land is soaked through.
fn set_weather(s: &mut Session, kind: Kind) {
    s.sim.make_weather(kind, 12.0, "the test");
    if let Some(sp) = &mut s.sim.weather_st.spell {
        sp.from = s.sim.t - 4.0 * HOUR;
    }
    s.sim.step_weather();
}

fn rain() -> Kind {
    weather::sample("rain").unwrap()
}

/// The same grass fire, dry and in the rain: in the rain it goes out.
#[test]
fn rain_puts_out_a_grass_fire() {
    let burn = |wet: bool| {
        let (w, a, dir) = grass_patch(if wet { "rainfire" } else { "dryfire" }, 45, 12.0);
        let mut s = session(&w, 13, None);
        s.sim.player.pos = a - dir * 20.0;
        clear_scatter(&mut s, a + dir * 6.0, 40.0);
        if wet {
            set_weather(&mut s, rain());
            assert!(s.sim.wx.soak > 0.3 && s.sim.wx.falls.is_some());
        } else {
            set_weather(&mut s, weather::sample("clear").unwrap());
        }
        let p = s.sim.snap.instances.iter().min_by(|x, y| (x.pos - a).length().total_cmp(&(y.pos - a).length())).unwrap().id;
        let id = s.sim.promote_instance(p).unwrap();
        s.sim.things.get_mut(id).unwrap().props[P_FIRE] = 0.6;
        s.run(60.0, 0.1);
        let first = s.sim.things.get(id).map(|t| (t.props[P_FIRE], t.props[P_WET])).unwrap_or_default();
        (events(&s.sim, "ignited").len(), first)
    };
    let (dry, _) = burn(false);
    let (wet, (fire, soaked)) = burn(true);
    assert!(dry >= 5, "dry grass burns ({dry})");
    assert!(wet <= 1, "wet grass doesn't ({wet} caught)");
    assert!(fire <= 0.0 && soaked > 0.5, "the first tuft was soaked and put out: fire {fire}, wet {soaked}");
}

/// What falls also works through the world's own rules (`falling`), and
/// nothing falls on what is under a roof.
#[test]
fn the_worlds_own_rules_read_the_rain() {
    let rules = vec![super::super::rules::RuleSpec { name: "rusts in the rain".into(), near: None, when: "falling > 0.2 && self.conducts > 0".into(), effects: vec!["self.char += 0.1 * falling * dt".into()] }];
    let vocab = crate::sim::props::Vocab::builtin();
    let compiled = crate::sim::env::probe_rules(&rules, &vocab);
    assert!(compiled.is_ok(), "{compiled:?}");
    let bad = vec![super::super::rules::RuleSpec { name: "x".into(), near: None, when: "drizzle > 0".into(), effects: vec!["self.char += 1".into()] }];
    assert!(crate::sim::env::probe_rules(&bad, &vocab).unwrap_err().contains("falling, wind"));
}

/// Most people mind the rain and get out of it; a frog folk loves it and
/// goes out in it; a fine day troubles nobody.
#[test]
fn people_get_out_of_the_rain_and_frogs_go_out_in_it() {
    let w = world("shelter", 52);
    let row = r#"{"name":"frogling","body":"figure","mind":"sapient","speech":"words","weather":{"rain":0.9,"clear":-0.3}}"#;
    w.db.with(|c| db::put_species(c, "frogling", row)).unwrap();
    let home = dry_spot(&w, 10.0, 0.7);
    let ada = add_char(&w, "Ada", "kind", &[], home);
    let frog = add_being(&w, "Plip", "frogling", &[], home + Vec3::new(3.0, 0.0, 0.0));
    let mut s = session(&w, 21, None);
    calm(&mut s);
    let shelter = |s: &mut Session, cid: i64| s.sim.affordances(cid).into_iter().find(|a| matches!(a.verb, super::super::scorer::Verb::Shelter { .. })).map(|a| (a.score, a.label));
    set_weather(&mut s, weather::sample("fair").unwrap());
    assert!(shelter(&mut s, ada).is_none(), "a fine day");
    set_weather(&mut s, weather::sample("storm").unwrap());
    assert!(s.sim.weather_feeling(ada) < -0.5 && s.sim.weather_feeling(frog) > 0.4, "{} {}", s.sim.weather_feeling(ada), s.sim.weather_feeling(frog));
    let (score, label) = shelter(&mut s, ada).expect("Ada wants out of the storm");
    assert!(score > 1.2 && label.contains("rain"), "{score} {label}");
    let best = s.sim.affordances(ada).into_iter().max_by(|a, b| a.score.total_cmp(&b.score)).unwrap();
    assert!(matches!(best.verb, super::super::scorer::Verb::Shelter { .. }), "getting out of it comes first: {}", best.label);
    assert!(shelter(&mut s, frog).is_none(), "the frogling stays out in it");
    // The planner hears the weather and how they feel about it.
    let line = s.sim.weather_line(ada);
    assert!(line.contains("rain falling") && line.contains("dislike") || line.contains("hate"), "{line}");
    assert!(s.sim.weather_line(frog).contains("love"));
}

/// The weather turning is told once, as news, and those near who mind it
/// think again.
#[test]
fn the_weather_turning_is_news() {
    let w = world("news", 53);
    let home = dry_spot(&w, 10.0, 0.2);
    add_char(&w, "Ben", "grumpy", &[], home);
    let mut s = session(&w, 22, None);
    s.sim.player.pos = home;
    let all = record(&mut s);
    s.run(2.0, 0.1);
    assert!(of(&all, "weather").is_empty(), "nothing is news at first");
    s.sim.make_weather(rain(), 4.0, "the traveler");
    s.run(HOUR as f32 * 0.6, 0.25);
    let news = of(&all, "weather");
    assert_eq!(news.len(), 1, "{:?}", news.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(news[0].text.contains("Rain begins"), "{}", news[0].text);
    assert_eq!(of(&all, "weather_made").len(), 1);
}

/// Made weather (a deed's answer) holds for its hours, comes through a
/// restart, then the land's own weather returns.
#[test]
fn made_weather_holds_through_a_restart_then_leaves() {
    let fx: crate::sim::interp::InterpEffect = serde_json::from_value(serde_json::json!({
        "narration": "Golden flakes drift down.",
        "weather": { "name": "golden snow", "clouds": 0.6, "falls": { "what": "golden flakes", "amount": 0.6, "color": [255, 210, 90], "look": "flake", "props": { "wet": 0.2 } }, "hours": 5 }
    }))
    .unwrap();
    let wfx = fx.weather.clone().unwrap();
    assert_eq!(wfx.hours, 5.0);
    assert_eq!(wfx.kind.falls.as_ref().unwrap().look, weather::FallLook::Flake);
    let w = world("made", 54);
    let mut s = session(&w, 23, None);
    s.sim.make_weather(wfx.kind, wfx.hours, "the traveler");
    s.run(HOUR as f32, 0.25);
    assert_eq!(s.sim.wx.name, "golden snow");
    assert!(s.sim.wx.words().contains("golden flakes falling") && s.sim.wx.words().contains("made by the traveler"), "{}", s.sim.wx.words());
    s.save();
    let mut s2 = session(&w, 23, None);
    s2.sim.t = s.sim.t;
    s2.sim.step_weather();
    assert_eq!(s2.sim.wx.name, "golden snow", "it holds through a restart");
    s2.sim.t += 6.0 * HOUR;
    s2.sim.step_weather();
    assert_ne!(s2.sim.wx.name, "golden snow", "and then it goes");
}

/// A being that only comes with the rain is away on a dry day and about in
/// the rain.
#[test]
fn rain_snails_come_with_the_rain() {
    let w = world("snails", 55);
    let row = r#"{"name":"rain snail","body":"quadruped","mind":"instinct","speech":"none","comes_with":["rain"]}"#;
    w.db.with(|c| db::put_species(c, "rain snail", row)).unwrap();
    let home = dry_spot(&w, 30.0, 1.1);
    let snail = add_being(&w, "Slow", "rain snail", &[], home);
    let mut s = session(&w, 24, None);
    s.sim.player.pos = home + Vec3::new(60.0, 0.0, 0.0);
    s.sim.player.yaw = 0.0;
    set_weather(&mut s, weather::sample("fair").unwrap());
    s.run(5.0, 0.1);
    assert!(s.sim.cast.get(snail).is_some_and(|n| n.away), "away on a fine day");
    set_weather(&mut s, rain());
    s.run(5.0, 0.1);
    assert!(s.sim.cast.get(snail).is_some_and(|n| !n.away), "out in the rain");
}
