//! Playing together, as the game runs it. Hosting: guests are let in, given
//! the world, and their bodies kept in it as they move them; everyone is
//! told where everyone else is. As a guest: the host's world is the one
//! here, the guest moves their own body, and the others are drawn where the
//! host says, a moment behind so they move smoothly between what was heard.

use super::{ACCENT, App, DIM};
use crate::net::host::{Arrival, Host};
use crate::net::wire::{Body, Down, Traveler, TravelerNo, Up, Welcome};
use crate::net::Link;
use glam::Vec3;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How often bodies are sent.
const BODY_EVERY: Duration = Duration::from_millis(50);
/// How far behind what was heard the others are drawn (so there are two
/// to draw between).
const BEHIND: Duration = Duration::from_millis(110);
/// The farthest a guest may move between two of their messages (m): more
/// is put back.
const MAX_STEP: f32 = 25.0;

pub enum Together {
    Host(Hosting),
    Guest(Guesting),
}

pub struct Hosting {
    pub host: Host,
    /// The world's file (copied for each guest who joins).
    path: PathBuf,
    world: String,
    name: String,
    guests: BTreeMap<TravelerNo, Guest>,
    /// Who has had which number here (by token).
    numbers: HashMap<String, TravelerNo>,
    last_bodies: Instant,
    /// The roster changed: tell everyone.
    roster_dirty: bool,
}

struct Guest {
    name: String,
    link: Link<Up, Down>,
    /// The world has gone to them (they can be sent the rest).
    ready: Arc<AtomicBool>,
    told_ready: bool,
}

pub struct Guesting {
    link: Link<Down, Up>,
    pub you: TravelerNo,
    roster: Vec<Traveler>,
    /// The host's name.
    host: String,
    /// What was heard of each other body, and when.
    heard: HashMap<TravelerNo, VecDeque<(Instant, Body)>>,
    last_sent: Instant,
    /// The look last told the host.
    look: crate::world::species::LookMap,
}

impl Hosting {
    pub fn new(host: Host, path: PathBuf, world: String, name: String) -> Hosting {
        Hosting { host, path, world, name, guests: BTreeMap::new(), numbers: HashMap::new(), last_bodies: Instant::now(), roster_dirty: false }
    }

    /// How many are here besides the host.
    pub fn guests(&self) -> usize {
        self.guests.len()
    }

    pub fn names(&self) -> Vec<String> {
        self.guests.values().map(|g| g.name.clone()).collect()
    }
}

impl Guesting {
    pub fn new(link: Link<Down, Up>, w: &Welcome, roster: Vec<Traveler>) -> Guesting {
        Guesting { link, you: w.you, host: w.host.clone(), roster, heard: HashMap::new(), last_sent: Instant::now(), look: w.look.clone() }
    }

    /// The host's name, as they gave it.
    pub fn host_name(&self) -> String {
        self.roster.iter().find(|t| t.no == 0).map(|t| t.name.clone()).unwrap_or_else(|| self.host.clone())
    }
}

impl App {
    /// Each frame: whatever playing together needs.
    pub(super) fn together_tick(&mut self) {
        match self.together.take() {
            Some(Together::Host(mut h)) => {
                self.host_tick(&mut h);
                self.together = Some(Together::Host(h));
            }
            Some(Together::Guest(mut g)) => {
                let go_on = self.guest_tick(&mut g);
                if go_on {
                    self.together = Some(Together::Guest(g));
                }
            }
            None => {}
        }
    }

    /// Playing as a guest in someone's world.
    pub fn is_guest(&self) -> bool {
        matches!(self.together, Some(Together::Guest(_)))
    }

    fn host_tick(&mut self, h: &mut Hosting) {
        for a in h.host.arrivals() {
            self.let_in(h, a);
        }
        // What each guest says.
        let mut gone = Vec::new();
        for (&no, g) in &h.guests {
            for m in g.link.drain() {
                match m {
                    Up::Me(b) => self.guest_moved(no, b, g),
                    Up::Look(look) => {
                        if let Some(t) = self.sim.travelers.get_mut(&no) {
                            t.own.look = look;
                            h.roster_dirty = true;
                        }
                    }
                    Up::Bye => g.link.close(),
                    Up::Hello(_) => {}
                }
            }
            if g.link.is_closed() {
                gone.push(no);
            }
            if !g.told_ready && g.ready.load(Ordering::Relaxed) {
                h.roster_dirty = true;
            }
        }
        for no in gone {
            if let Some(g) = h.guests.remove(&no) {
                self.sim.remove_traveler(no);
                let why = g.link.why().filter(|w| w != "closed").map(|w| format!(" ({w})")).unwrap_or_default();
                self.say(None, &format!("{} left.{why}", g.name), DIM);
                h.roster_dirty = true;
            }
        }
        if h.roster_dirty {
            h.roster_dirty = false;
            let roster = self.roster(h);
            for g in h.guests.values_mut() {
                if g.ready.load(Ordering::Relaxed) {
                    if !g.told_ready {
                        g.told_ready = true;
                        self.say(None, &format!("{} is here.", g.name), ACCENT);
                    }
                    g.link.send(Down::Roster(roster.clone()));
                }
            }
        }
        if h.last_bodies.elapsed() >= BODY_EVERY && !h.guests.is_empty() {
            h.last_bodies = Instant::now();
            let mut all: Vec<(TravelerNo, Body)> = vec![(0, Body::of(&self.sim.player, self.pitch))];
            all.extend(self.sim.travelers.values().map(|t| (t.no, Body::of(&t.a, t.pitch))));
            for (&no, g) in &h.guests {
                if g.told_ready {
                    let others = all.iter().filter(|(n, _)| *n != no).copied().collect();
                    g.link.send(Down::Bodies { t: self.sim.t, bodies: others });
                }
            }
        }
    }

    /// Everyone here, for the guests: the host as traveler 0.
    fn roster(&self, h: &Hosting) -> Vec<Traveler> {
        let mut r = vec![Traveler { no: 0, name: h.name.clone(), look: self.sim.own.look.clone() }];
        for (no, g) in &h.guests {
            let look = self.sim.travelers.get(no).map(|t| t.own.look.clone()).unwrap_or_default();
            r.push(Traveler { no: *no, name: g.name.clone(), look });
        }
        r
    }

    /// A guest let in: their number, their body in the world, and the
    /// world sent to them (saved first, so it is as it is now).
    fn let_in(&mut self, h: &mut Hosting, a: Arrival) {
        let Arrival { hello, link, from } = a;
        let next = (1..=u8::MAX).find(|n| !h.numbers.values().any(|x| x == n)).unwrap_or(u8::MAX);
        let no = *h.numbers.entry(hello.token.clone()).or_insert(next);
        // Back from somewhere else (a dropped connection): the old one goes.
        if let Some(old) = h.guests.remove(&no) {
            old.link.close();
            self.sim.remove_traveler(no);
        }
        crate::log::info(format!("{} joins from {from} as traveler {no}", hello.name));
        self.say(None, &format!("{} is joining…", hello.name), DIM);
        self.save_player();
        self.save_characters();
        let (at, yaw) = self.beside_me();
        let look = Default::default();
        self.sim.add_traveler(no, &hello.name, &look, at, yaw);
        let body = self.sim.travelers.get(&no).map(|t| Body::of(&t.a, 0.0)).unwrap_or_default();
        let welcome = Welcome { you: no, world: h.world.clone(), host: h.name.clone(), bytes: 0, packed: 0, body, look, wounds: 0.0, t: self.sim.t };
        let ready = Arc::new(AtomicBool::new(false));
        let (tx, path, r) = (link.sender(), h.path.clone(), ready.clone());
        let name = hello.name.clone();
        let _ = std::thread::Builder::new().name("host-send-world".into()).spawn(move || match crate::net::host::guest_copy(&path) {
            Ok((packed, bytes)) => {
                let _ = tx.send(Down::Welcome(Welcome { bytes, packed: packed.len() as u64, ..welcome }));
                for c in packed.chunks(crate::net::host::CHUNK) {
                    if tx.send(Down::WorldChunk(c.to_vec())).is_err() {
                        return;
                    }
                }
                let _ = tx.send(Down::WorldDone);
                r.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                crate::log::error(format!("sending the world to {name}: {e:#}"));
                let _ = tx.send(Down::Refused(format!("The host couldn't send the world: {e}")));
            }
        });
        h.guests.insert(no, Guest { name: hello.name, link, ready, told_ready: false });
    }

    /// Where a newcomer arrives: a couple of steps before the host, facing
    /// them (out of anything solid).
    fn beside_me(&mut self) -> (Vec3, f32) {
        use crate::world::collide::{Obstacles, PLAYER_RADIUS, free_spot};
        let me = self.sim.player.pos;
        let yaw = self.sim.player.yaw;
        let want = me + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 2.5;
        let want = Vec3::new(want.x, self.snap.terrain.height(want.x, want.z).max(me.y - 1.0), want.z);
        let solids = self.sim.solids_near(want, 6.0);
        let obs = Obstacles { solids: &solids, bodies: &[] };
        let at = free_spot(&self.snap.terrain, &obs, want, PLAYER_RADIUS, 20.0, true);
        let face = me - at;
        (at, face.x.atan2(face.z))
    }

    /// A guest moved themselves: kept, unless it is too far to be a step.
    fn guest_moved(&mut self, no: TravelerNo, b: Body, g: &Guest) {
        let Some(t) = self.sim.travelers.get_mut(&no) else { return };
        let to = Vec3::from_array(b.pos);
        if !to.is_finite() || (to - t.a.pos).length() > MAX_STEP {
            g.link.send(Down::Place(Body::of(&t.a, t.pitch)));
            return;
        }
        b.onto(&mut t.a);
        t.pitch = b.pitch;
    }

    /// One frame as a guest: what the host says, the others drawn between
    /// what was heard, and our own body sent. False when the game is over.
    fn guest_tick(&mut self, g: &mut Guesting) -> bool {
        let now = Instant::now();
        for m in g.link.drain() {
            match m {
                Down::Roster(r) => {
                    self.take_roster(g, r);
                }
                Down::Bodies { t, bodies } => {
                    // The host's clock: followed gently, jumped to when far off.
                    let d = t - self.sim.t;
                    if d.abs() > 2.0 { self.sim.t = t } else { self.sim.t += d * 0.1 }
                    for (no, b) in bodies {
                        let q = g.heard.entry(no).or_default();
                        q.push_back((now, b));
                        while q.len() > 8 {
                            q.pop_front();
                        }
                    }
                }
                Down::Place(b) => {
                    b.onto(&mut self.sim.player);
                    self.pitch = b.pitch;
                }
                Down::Say(s) => self.say(None, &s, DIM),
                Down::Bye(why) => {
                    self.leave_world(why);
                    return false;
                }
                Down::Refused(why) => {
                    self.leave_world(why);
                    return false;
                }
                Down::Welcome(_) | Down::WorldChunk(_) | Down::WorldDone => {}
            }
        }
        if g.link.is_closed() {
            let why = g.link.why().filter(|w| w != "closed").map(|w| format!(" ({w})")).unwrap_or_default();
            self.leave_world(format!("The connection to {} was lost{why}.", g.host_name()));
            return false;
        }
        let at = now.checked_sub(BEHIND).unwrap_or(now);
        for (no, q) in &g.heard {
            let Some(t) = self.sim.travelers.get_mut(no) else { continue };
            let b = between(q, at);
            b.onto(&mut t.a);
            t.pitch = b.pitch;
        }
        if g.last_sent.elapsed() >= BODY_EVERY {
            g.last_sent = now;
            g.link.send(Up::Me(Body::of(&self.sim.player, self.pitch)));
            // The look they chose, when it changes, for the others to see.
            if g.look != self.sim.own.look {
                g.look = self.sim.own.look.clone();
                g.link.send(Up::Look(g.look.clone()));
            }
        }
        true
    }

    /// Who is here, as the host says: the others come and go.
    fn take_roster(&mut self, g: &mut Guesting, r: Vec<Traveler>) {
        for t in r.iter().filter(|t| t.no != g.you) {
            match self.sim.travelers.get_mut(&t.no) {
                Some(have) => {
                    have.name = t.name.clone();
                    have.own.look = t.look.clone();
                }
                None => {
                    let at = g.heard.get(&t.no).and_then(|q| q.back()).map(|(_, b)| Vec3::from_array(b.pos)).unwrap_or(self.sim.player.pos);
                    self.sim.add_traveler(t.no, &t.name, &t.look, at, 0.0);
                    if !g.roster.iter().any(|o| o.no == t.no) {
                        self.say(None, &format!("{} is here.", t.name), ACCENT);
                    }
                }
            }
        }
        let gone: Vec<TravelerNo> = self.sim.travelers.keys().copied().filter(|n| !r.iter().any(|t| t.no == *n)).collect();
        for no in gone {
            if let Some(t) = self.sim.remove_traveler(no) {
                self.say(None, &format!("{} left.", t.name), DIM);
            }
            g.heard.remove(&no);
        }
        g.roster = r;
    }

    /// The game together is over: back to the worlds, saying why.
    fn leave_world(&mut self, why: String) {
        crate::log::info(format!("left the host's world: {why}"));
        self.left_because = Some(why);
        self.to_worlds = true;
        self.quit = true;
    }

    /// Leaving the game: guests are told (as a host) or the host is (as a guest).
    pub(super) fn together_end(&mut self) {
        match self.together.take() {
            Some(Together::Host(h)) => {
                for g in h.guests.values() {
                    g.link.send(Down::Bye(format!("{} closed the world.", h.name)));
                    g.link.close();
                }
                h.host.stop();
                // A moment for the goodbyes to go out.
                let until = Instant::now() + Duration::from_millis(300);
                while Instant::now() < until && h.guests.values().any(|g| !g.link.is_closed()) {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            Some(Together::Guest(g)) => {
                g.link.send(Up::Bye);
                g.link.close();
                std::thread::sleep(Duration::from_millis(50));
            }
            None => {}
        }
    }

    /// Just joined: standing where the host says, as the host keeps us,
    /// with the others here.
    pub fn arrive_as_guest(&mut self, w: &Welcome, roster: Vec<Traveler>) {
        w.body.onto(&mut self.sim.player);
        self.pitch = w.body.pitch;
        self.sim.own.look = w.look.clone();
        self.sim.t = w.t;
        self.cam_y = self.sim.player.pos.y + self.sim.player.dims.eye;
        if let Some(Together::Guest(mut g)) = self.together.take() {
            self.take_roster(&mut g, roster);
            let host = g.host_name();
            self.together = Some(Together::Guest(g));
            self.say(None, &format!("You're in {host}'s world. They keep it, and its words come from them."), ACCENT);
        }
    }

    /// Who else is here, for the top line.
    pub(super) fn together_line(&self) -> Option<String> {
        match &self.together {
            Some(Together::Host(h)) => {
                let who = match h.guests() {
                    0 => "nobody has joined yet".to_string(),
                    _ => format!("with {}", h.names().join(", ")),
                };
                Some(format!("hosting at {} · code {} · {who}", h.host.lan_address(), h.host.code))
            }
            Some(Together::Guest(g)) => Some(format!("in {}'s world", g.host_name())),
            None => None,
        }
    }

}

/// A body at `at`, between the two heard around it (or the last, held).
fn between(q: &VecDeque<(Instant, Body)>, at: Instant) -> Body {
    let mut prev: Option<&(Instant, Body)> = None;
    for s in q {
        if s.0 >= at {
            return match prev {
                Some(p) => {
                    let span = (s.0 - p.0).as_secs_f32().max(1e-3);
                    p.1.lerp(&s.1, ((at - p.0).as_secs_f32() / span).clamp(0.0, 1.0))
                }
                None => s.1,
            };
        }
        prev = Some(s);
    }
    q.back().map(|s| s.1).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::model::{Live, WorldModel};
    use parking_lot::Mutex;

    /// A game on `db` (no LLM, drawn on the CPU), hosting or a guest.
    fn game(db: Arc<Db>, together: Option<Together>) -> App {
        let live = Arc::new(Mutex::new(Live::default()));
        let mut model = WorldModel::load(db.clone(), None, live.clone()).unwrap();
        let snap = model.snapshot().unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        let brain = crate::brain::Brain::start(model, None, tx, false);
        App::new(super::super::Setup { db, brain, events: rx, render: crate::render::cpu::spawn(), snap, live, truecolor: true, size: (100, 34), llm: None, genesis: false, together })
    }

    /// Tick until `done`, or fail after a while.
    fn until(games: &mut [&mut App], what: &str, mut done: impl FnMut(&[&mut App]) -> bool) {
        let t0 = Instant::now();
        while !done(games) {
            assert!(t0.elapsed() < Duration::from_secs(15), "waited too long for {what}");
            for g in games.iter_mut() {
                g.tick(1.0 / 60.0);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn two_players_see_each_other_walk() {
        let path = std::env::temp_dir().join(format!("pocket-together-{}.pocket", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Db::create(&path, 4242, "a quiet test valley", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
        db.kv_set("genesis", "done").unwrap();
        let port = 43000 + (std::process::id() % 1000) as u16 * 7 % 9000;
        let host = Host::start(port, "lime-otter-42".into()).unwrap();
        let addr = format!("127.0.0.1:{}", host.port);
        let mut h = game(db, Some(Together::Host(Hosting::new(host, path.clone(), "Test Vale".into(), "Hal".into()))));
        let start = h.sim.player.pos;

        // The guest joins on another thread while the host's game runs.
        let joining = std::thread::spawn(move || crate::net::guest::join(&addr, "lime-otter-42", "Bea", "tok-bea", |_, _| {}));
        let t0 = Instant::now();
        while !joining.is_finished() {
            assert!(t0.elapsed() < Duration::from_secs(15), "the join took too long");
            h.tick(1.0 / 60.0);
            std::thread::sleep(Duration::from_millis(5));
        }
        let j = joining.join().unwrap().unwrap();
        assert_eq!(j.welcome.you, 1);
        let gdb = Db::from_bytes(&j.world).unwrap();
        crate::net::guest::as_guest(&gdb, &j.welcome).unwrap();
        let mut g = game(gdb, Some(Together::Guest(Guesting::new(j.link, &j.welcome, j.roster.clone()))));
        g.arrive_as_guest(&j.welcome, j.roster);
        assert!(g.is_guest());

        // Each comes to see the other.
        until(&mut [&mut h, &mut g], "each to see the other", |gs| gs[0].sim.travelers.contains_key(&1) && gs[1].sim.travelers.contains_key(&0));
        assert_eq!(g.sim.travelers[&0].name, "Hal");
        assert_eq!(h.sim.travelers[&1].name, "Bea");
        assert!(h.log.iter().any(|l| l.text.contains("Bea is here")));

        // The guest walks: the host sees them go.
        let from = h.sim.travelers[&1].a.pos;
        g.sim.player.pos = g.sim.player.pos + Vec3::new(3.0, 0.0, 0.0);
        until(&mut [&mut h, &mut g], "the host to see the guest walk", |gs| (gs[0].sim.travelers[&1].a.pos - from).length() > 2.5);
        // The host walks: the guest sees them go, a moment behind.
        h.sim.player.pos = start + Vec3::new(0.0, 0.0, 4.0);
        until(&mut [&mut h, &mut g], "the guest to see the host walk", |gs| (gs[1].sim.travelers[&0].a.pos - (start + Vec3::new(0.0, 0.0, 4.0))).length() < 0.5);

        // A step too far to be a step is put back.
        g.sim.player.pos += Vec3::new(500.0, 0.0, 0.0);
        until(&mut [&mut h, &mut g], "the guest to be put back", |gs| (gs[1].sim.player.pos - gs[0].sim.travelers[&1].a.pos).length() < 30.0);

        // The guest leaves: gone from the host's world.
        g.shutdown();
        until(&mut [&mut h], "the guest to be gone", |gs| !gs[0].sim.travelers.contains_key(&1));
        assert!(h.log.iter().any(|l| l.text.contains("Bea left")));
        h.shutdown();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn others_are_drawn_between_what_was_heard() {
        let t0 = Instant::now();
        let mut q = VecDeque::new();
        q.push_back((t0, Body { pos: [0.0, 0.0, 0.0], ..Default::default() }));
        q.push_back((t0 + Duration::from_millis(100), Body { pos: [10.0, 0.0, 0.0], ..Default::default() }));
        assert!((between(&q, t0 + Duration::from_millis(50)).pos[0] - 5.0).abs() < 0.01);
        assert_eq!(between(&q, t0 + Duration::from_secs(1)).pos[0], 10.0);
        assert_eq!(between(&q, t0.checked_sub(Duration::from_secs(1)).unwrap_or(t0)).pos[0], 0.0);
    }
}
