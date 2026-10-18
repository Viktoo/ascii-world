//! Playing together, as the game runs it. Hosting: guests are let in, given
//! the world, and their bodies kept in it as they move them; everyone is
//! told where everyone else is. As a guest: the host's world is the one
//! here, the guest moves their own body, and the others are drawn where the
//! host says, a moment behind so they move smoothly between what was heard.

use super::{ACCENT, App, DIM};
use crate::net::host::{Arrival, Host};
use crate::net::rows::Watch;
use crate::net::wire::{self, Body, Down, Traveler, TravelerNo, Up, Welcome};
use crate::net::Link;
use crate::sim::mirror::{Seen, from_host};
use crate::sim::{ActorId, Note};
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
    last_bodies: Instant,
    /// The roster changed: tell everyone.
    roster_dirty: bool,
    /// Keeps every guest's copy of the world's own tables the same.
    watch: crossbeam_channel::Sender<Watch>,
}

struct Guest {
    name: String,
    token: String,
    link: Link<Up, Down>,
    /// The world has gone to them (they can be sent the rest).
    ready: Arc<AtomicBool>,
    told_ready: bool,
    /// What they were last told of the world near them, and of themselves.
    seen: Seen,
    own: Option<(f32, bool, f32)>,
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
    /// Our own name in the host's world.
    me: String,
}

impl Hosting {
    pub fn new(host: Host, path: PathBuf, world: String, name: String) -> Hosting {
        let (watch, rx) = crossbeam_channel::unbounded();
        let p = path.clone();
        let _ = std::thread::Builder::new().name("host-rows".into()).spawn(move || crate::net::rows::watch(p, rx));
        Hosting { host, path, world, name, guests: BTreeMap::new(), last_bodies: Instant::now(), roster_dirty: false, watch }
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
        Guesting { link, you: w.you, host: w.host.clone(), roster, heard: HashMap::new(), last_sent: Instant::now(), look: w.look.clone(), me: w.name.clone() }
    }

    /// Do something in the host's world.
    pub fn act(&self, json: String) {
        self.link.send(Up::Act(json));
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
                    Up::Act(json) => self.guest_acts(no, &json, g),
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
                let _ = h.watch.send(Watch::Remove(no));
                self.save_guest(no, &g);
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
            for (&no, g) in h.guests.iter_mut() {
                if !g.told_ready {
                    continue;
                }
                let others = all.iter().filter(|(n, _)| *n != no).copied().collect();
                g.link.send(Down::Bodies { t: self.sim.t, bodies: others });
                // The world near them, as it changed.
                let Some(at) = self.sim.travelers.get(&no).map(|t| t.a.pos) else { continue };
                let w = self.sim.world_near(&mut g.seen, at, no);
                if w != (wire::WorldNear { t: w.t, ..Default::default() }) {
                    g.link.send(Down::World(Box::new(w)));
                }
                let own = self.sim.own_of(no);
                if own.is_some() && own != g.own {
                    g.own = own;
                    let (wounds, fallen, glow) = own.unwrap_or_default();
                    g.link.send(Down::Own { wounds, fallen, glow });
                }
            }
        }
    }

    /// What the host's log is told goes to the guests too: about the host
    /// by their name; what is said to the host alone ("you…") stays theirs.
    pub(super) fn share_notes(&self, notes: &[Note]) {
        let Some(Together::Host(h)) = &self.together else { return };
        if h.guests.is_empty() {
            return;
        }
        for n in notes {
            let Some(w) = wire_note(n, &h.name) else { continue };
            for g in h.guests.values().filter(|g| g.told_ready) {
                g.link.send(Down::Note(w.clone()));
            }
        }
    }

    /// Sounds go to the guests who can hear them.
    pub(super) fn share_sounds(&self) {
        let Some(Together::Host(h)) = &self.together else { return };
        if h.guests.is_empty() || self.sim.sounds.is_empty() {
            return;
        }
        for (no, g) in h.guests.iter().filter(|(_, g)| g.told_ready) {
            let Some(at) = self.sim.travelers.get(no).map(|t| t.a.pos) else { continue };
            let near: Vec<wire::Sound> = self.sim.sounds.iter().filter(|c| (c.at - at).length() < 80.0).map(wire_sound).collect();
            if !near.is_empty() {
                g.link.send(Down::Heard(near));
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
        // The host playing in their own world from another window.
        let token = if hello.token == self.settings.player().1 { format!("{}+", hello.token) } else { hello.token.clone() };
        let known = self.db.travelers();
        let row = known.iter().find(|r| r.token == token).cloned();
        let Some(no) = row.as_ref().map(|r| r.no).or_else(|| (1..=u8::MAX).find(|n| !known.iter().any(|r| r.no == *n))) else {
            link.send(Down::Refused("This world has had too many travelers.".into()));
            link.close();
            return;
        };
        // Back from somewhere else (a dropped connection): the old one goes.
        if let Some(old) = h.guests.remove(&no) {
            old.link.close();
            self.save_guest(no, &old);
            self.sim.remove_traveler(no);
        }
        crate::log::info(format!("{} joins from {from} as traveler {no}", hello.name));
        self.say(None, &format!("{} is joining…", hello.name), DIM);
        // Back: where they left, as they were. New: before the host.
        let (at, yaw) = match &row {
            Some(r) => (Vec3::from_array(r.pos), r.yaw),
            None => self.beside_me(),
        };
        let look: crate::world::species::LookMap = row.as_ref().and_then(|r| serde_json::from_str(&r.look_json).ok()).unwrap_or_default();
        let wounds = row.as_ref().map(|r| r.wounds).unwrap_or(0.0);
        self.sim.add_traveler(no, &hello.name, &look, at, yaw);
        if let Some(t) = self.sim.travelers.get_mut(&no) {
            t.wounds = wounds;
        }
        let g = Guest { name: hello.name.clone(), token, link, ready: Arc::new(AtomicBool::new(false)), told_ready: false, seen: Seen::default(), own: None };
        self.save_guest(no, &g);
        // The copy they get is the world as it is now.
        self.save_player();
        self.save_characters();
        let body = self.sim.travelers.get(&no).map(|t| Body::of(&t.a, 0.0)).unwrap_or_default();
        let welcome = Welcome { you: no, world: h.world.clone(), name: hello.name.clone(), host: h.name.clone(), bytes: 0, packed: 0, body, look, wounds, t: self.sim.t, seed: self.sim.seed };
        let (tx, path, r, watch) = (g.link.sender(), h.path.clone(), g.ready.clone(), h.watch.clone());
        let name = hello.name.clone();
        let _ = std::thread::Builder::new().name("host-send-world".into()).spawn(move || match crate::net::host::guest_copy(&path) {
            Ok((packed, bytes, had)) => {
                let _ = tx.send(Down::Welcome(Welcome { bytes, packed: packed.len() as u64, ..welcome }));
                for c in packed.chunks(crate::net::host::CHUNK) {
                    if tx.send(Down::WorldChunk(c.to_vec())).is_err() {
                        return;
                    }
                }
                let _ = tx.send(Down::WorldDone);
                // From here the world's tables are kept the same for them.
                let _ = watch.send(Watch::Add(no, tx, had));
                r.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                crate::log::error(format!("sending the world to {name}: {e:#}"));
                let _ = tx.send(Down::Refused(format!("The host couldn't send the world: {e}")));
            }
        });
        h.guests.insert(no, g);
    }

    /// Keep a guest as they are now (where, how they look, how hurt), for
    /// when they come back.
    fn save_guest(&self, no: TravelerNo, g: &Guest) {
        let Some(t) = self.sim.travelers.get(&no) else { return };
        let row = crate::db::TravelerRow { no, token: g.token.clone(), name: g.name.clone(), pos: t.a.pos.to_array(), yaw: t.a.yaw, look_json: serde_json::to_string(&t.own.look).unwrap_or_else(|_| "{}".into()), wounds: if t.fallen.is_some() { 0.0 } else { t.wounds }, last_seen: crate::db::now() };
        if let Err(e) = self.db.save_traveler(&row) {
            crate::log::error(format!("saving traveler {no}: {e:#}"));
        }
    }

    /// Every guest kept as they are now (with the regular saves).
    pub(super) fn save_guests(&self) {
        if let Some(Together::Host(h)) = &self.together {
            for (no, g) in &h.guests {
                self.save_guest(*no, g);
            }
        }
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

    /// A guest does something: done as them, and they are told how it went.
    fn guest_acts(&mut self, no: TravelerNo, json: &str, g: &Guest) {
        let a = serde_json::from_str::<serde_json::Value>(json).map(|mut v| {
            as_host(&mut v, no);
            v
        });
        let a: Option<crate::sim::Action> = a.ok().and_then(|v| serde_json::from_value(v).ok());
        let Some(a) = a else {
            g.link.send(Down::Acted { verb: String::new(), ok: false, msg: "the host's game didn't understand that".into(), thing: false });
            return;
        };
        let verb = a.verb().to_string();
        let r = self.sim.act(ActorId::Traveler(no), a);
        let (ok, msg, thing) = match r {
            Ok(o) => (o.ok, o.msg, o.thing.is_some()),
            Err(e) => (false, e.to_string(), false),
        };
        g.link.send(Down::Acted { verb, ok, msg, thing });
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
                Down::World(w) => self.sim.take_world(*w),
                Down::Rows { tables, rebuild } => match crate::net::rows::apply(&self.db, &tables) {
                    Ok(ground) => {
                        if rebuild {
                            self.brain.send(crate::brain::Cmd::Reload);
                        } else if ground {
                            crate::sim::persist::apply_made_ground(&mut self.sim);
                        }
                    }
                    Err(e) => crate::log::error(format!("taking in the host's world: {e:#}")),
                },
                Down::Own { wounds, fallen, glow } => self.sim.take_own(wounds, fallen, glow),
                Down::Acted { verb, ok, msg, thing } => {
                    let told = matches!(verb.as_str(), "use" | "do" | "eat" | "answer" | "give" | "gesture" | "propose" | "pet" | "ride" | "dismount") || (matches!(verb.as_str(), "hold" | "drop") && !thing);
                    if !ok {
                        self.say(None, &format!("Can't: {}.", super::you_as(&msg, &g.me)), DIM);
                    } else if told {
                        self.say(None, &super::you_as(&msg, &g.me), super::TEXT);
                    }
                }
                Down::Note(n) => {
                    let n = guest_note(n, g.you, &self.sim);
                    self.tell(n);
                }
                Down::Heard(sounds) => {
                    for s in sounds {
                        if let Some(c) = cue_of(s, g.you) {
                            self.sim.sounds.push(c);
                        }
                    }
                }
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
        self.sim.mirror.on = true;
        self.sim.mirror.you = w.you;
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

/// An action from a guest, naming actors as the host knows them: the
/// guest's "player" is traveler `no`, their traveler 0 the host's player.
fn as_host(v: &mut serde_json::Value, no: TravelerNo) {
    match v {
        serde_json::Value::String(s) if s == "player" => *s = ActorId::Traveler(no).key(),
        serde_json::Value::String(s) if s == "player:0" => *s = "player".into(),
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| as_host(x, no)),
        serde_json::Value::Object(o) => o.values_mut().for_each(|x| as_host(x, no)),
        _ => {}
    }
}

/// An action as a guest sends it.
pub(super) fn action_json(a: &crate::sim::Action) -> String {
    serde_json::to_string(a).unwrap_or_default()
}

/// A note of the host's as a guest hears it: none when it was said to the
/// host alone ("You fall."), and the host by their name.
fn wire_note(n: &Note, host: &str) -> Option<wire::Note> {
    let personal = |t: &str| {
        let l = format!(" {} ", t.to_lowercase());
        l.starts_with(" you ") || l.contains(" you ") || l.contains(" your ") || l.contains(" you.") || l.contains(" you,")
    };
    let named = |t: &str| t.replace("the traveler's", &format!("{host}'s")).replace("The traveler", host).replace("the traveler", host);
    Some(match n {
        Note::Line { id, who, text } => wire::Note::Line { id: id.map(|a| a.code()), who: who.clone(), text: text.clone() },
        Note::Info(t) | Note::Effect(t) if personal(t) => return None,
        Note::Notable(t) | Note::Made(t) | Note::Ambient(t) if personal(t) => return None,
        Note::Info(t) => wire::Note::Info(named(t)),
        Note::Effect(t) => wire::Note::Effect(named(t)),
        Note::Ambient(t) => wire::Note::Ambient(named(t)),
        Note::Notable(t) => wire::Note::Notable(named(t)),
        Note::Made(t) => wire::Note::Made(named(t)),
        Note::Far { text, at, made } => wire::Note::Far { text: named(text), at: *at, made: *made },
        Note::Incident { id, text, at, near } => wire::Note::Incident { id: *id, text: named(text), at: *at, near: *near },
    })
}

/// A note from the host, for this guest's log.
fn guest_note(n: wire::Note, you: TravelerNo, sim: &crate::sim::Sim) -> Note {
    let _ = sim;
    match n {
        wire::Note::Line { id, who, text } => Note::Line { id: id.map(|c| from_host(c, you)), who, text },
        wire::Note::Info(t) => Note::Info(t),
        wire::Note::Ambient(t) => Note::Ambient(t),
        wire::Note::Notable(t) => Note::Notable(t),
        wire::Note::Made(t) => Note::Made(t),
        wire::Note::Far { text, at, made } => Note::Far { text, at, made },
        wire::Note::Incident { id, text, at, near } => Note::Incident { id, text, at, near },
        wire::Note::Effect(t) => Note::Effect(t),
    }
}

fn wire_sound(c: &crate::audio::Cue) -> wire::Sound {
    use crate::audio::Heard;
    let mat = |m: &crate::audio::call::Material| [m.hard, m.dry, m.ring, m.leafy];
    let what = match &c.what {
        Heard::Call { call, mass, pitch, gain } => wire::Heard::Call { call: serde_json::to_string(call).unwrap_or_default(), mass: *mass, pitch: *pitch, gain: *gain },
        Heard::Hit { mat: m, mass, speed, by } => wire::Heard::Hit { mat: mat(m), mass: *mass, speed: *speed, by: by.as_ref().map(|(m, x)| (mat(m), *x)) },
    };
    wire::Sound { at: c.at.to_array(), from: c.from.map(|a| a.code()), what }
}

fn cue_of(s: wire::Sound, you: TravelerNo) -> Option<crate::audio::Cue> {
    use crate::audio::Heard;
    let mat = |m: [f32; 4]| crate::audio::call::Material { hard: m[0], dry: m[1], ring: m[2], leafy: m[3] };
    let what = match s.what {
        wire::Heard::Call { call, mass, pitch, gain } => {
            let v: serde_json::Value = serde_json::from_str(&call).ok()?;
            Heard::Call { call: crate::audio::call::Call::from_value(&v), mass, pitch, gain }
        }
        wire::Heard::Hit { mat: m, mass, speed, by } => Heard::Hit { mat: mat(m), mass, speed, by: by.map(|(m, x)| (mat(m), x)) },
    };
    Some(crate::audio::Cue { at: Vec3::from_array(s.at), from: s.from.map(|c| from_host(c, you)), what })
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
        App::new(super::super::Setup { db, brain, events: rx, render: crate::render::cpu::spawn(), snap, live, truecolor: true, size: (100, 34), llm: None, genesis: false, together, seed: None })
    }

    /// Tick until `done`, or fail after a while.
    fn until(games: &mut [&mut App], what: &str, mut done: impl FnMut(&mut [&mut App]) -> bool) {
        let t0 = Instant::now();
        while !done(games) {
            assert!(t0.elapsed() < Duration::from_secs(15), "waited too long for {what}");
            for g in games.iter_mut() {
                g.tick(1.0 / 60.0);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A world with someone in it, hosted, and a guest joined to it.
    fn pair(tag: &str, port: u16) -> (App, App, PathBuf) {
        let path = std::env::temp_dir().join(format!("pocket-together-{tag}-{}.pocket", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = Db::create(&path, 4242, "a quiet test valley", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
        db.kv_set("genesis", "done").unwrap();
        let spawn = {
            let live = Arc::new(Mutex::new(Live::default()));
            WorldModel::load(db.clone(), None, live).unwrap().snapshot().unwrap().spawn
        };
        let persona = crate::world::Persona { name: "Mara".into(), ..Default::default() };
        db.tx(|tx| {
            let v = crate::db::add_version(tx, None, "region", "test region", None)?;
            crate::db::add_character(tx, crate::world::region_of(spawn.x, spawn.z), &serde_json::to_string(&persona)?, spawn.x + 3.0, spawn.z + 3.0, v)?;
            Ok(())
        })
        .unwrap();
        let host = Host::start(port, "lime-otter-42".into()).unwrap();
        let addr = format!("127.0.0.1:{}", host.port);
        let mut h = game(db, Some(Together::Host(Hosting::new(host, path.clone(), "Test Vale".into(), "Hal".into()))));

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
        (h, g, path)
    }

    #[test]
    fn two_players_see_each_other_walk() {
        let port = 43000 + (std::process::id() % 1000) as u16 * 7 % 9000;
        let (mut h, mut g, path) = pair("walk", port);
        let start = h.sim.player.pos;

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
        // Kept for next time: where she was, by her token.
        let kept = h.db.travelers();
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].no, kept[0].token.as_str(), kept[0].name.as_str()), (1, "tok-bea", "Bea"));
        assert!((Vec3::from_array(kept[0].pos) - g.sim.player.pos).length() < 30.0);
        h.shutdown();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_guest_sees_the_living_world() {
        let port = 45000 + (std::process::id() % 1000) as u16 * 7 % 9000;
        let (mut h, mut g, path) = pair("living", port);
        until(&mut [&mut h, &mut g], "the guest to be in", |gs| gs[1].sim.travelers.contains_key(&0));
        // The same people, looking the same.
        let mara = h.sim.cast.npcs[0].def.id;
        assert_eq!(g.sim.cast.get(mara).map(|n| n.sliders), h.sim.cast.get(mara).map(|n| n.sliders), "Mara looks the same to both");
        // Mara walks off on the host: the guest sees her go.
        let to = h.sim.cast.get(mara).unwrap().a.pos + Vec3::new(5.0, 0.0, 0.0);
        until(&mut [&mut h, &mut g], "Mara to move for the guest", |gs| {
            if let Some(n) = gs[0].sim.cast.get_mut(mara) {
                n.a.pos = to;
                n.think_at = f64::MAX;
            }
            gs[1].sim.cast.get(mara).is_some_and(|n| (n.a.pos - to).length() < 0.5)
        });
        // A stone appears by the host: the guest sees it, and its flash.
        let stone = h.sim.type_by_name("stone").unwrap().id;
        let at = h.sim.player.pos + Vec3::new(1.0, 0.5, 1.0);
        let id = h.sim.spawn_thing(stone, at, 0.0, 1.0, Default::default(), true).unwrap();
        h.sim.flashes.push((at, h.sim.t));
        until(&mut [&mut h, &mut g], "the stone to show", |gs| gs[1].sim.things.get(id).is_some());
        assert!(!g.sim.flashes.is_empty(), "and its flash");
        // The host picks it up: in the host's hand for the guest too.
        h.sim.hand_to(ActorId::Player, id);
        until(&mut [&mut h, &mut g], "the stone in Hal's hand", |gs| gs[1].sim.travelers.get(&0).is_some_and(|t| t.a.held == Some(id)));
        assert_eq!(g.sim.player.held, None, "not in the guest's");
        // The host's world gets a new building: the guest's is made anew with it.
        let (lh, _) = (h.sim.type_by_name("lighthouse").or_else(|| h.sim.type_by_name("stone")).unwrap().id, ());
        let placed = h.db.tx(|tx| {
            let v = crate::db::add_version(tx, None, "make", "a cairn", None)?;
            crate::db::add_instance(tx, lh as i64, v, at.x + 6.0, at.y, at.z, 0.0, 1.0, &[0.5; 8])
        }).unwrap();
        until(&mut [&mut h, &mut g], "the guest's world to have the cairn", |gs| {
            while let Ok(ev) = gs[1].events.try_recv() {
                gs[1].on_event(ev);
            }
            gs[1].snap.instances.iter().any(|p| p.id == placed)
        });
        // The host says something to the world: the guest hears it, the host by name.
        h.sim.notes.push(Note::Ambient("Mara waves at the traveler.".into()));
        h.sim.notes.push(Note::Notable("You feel watched.".into()));
        until(&mut [&mut h, &mut g], "the guest to hear it", |gs| gs[1].log.iter().any(|l| l.text.contains("Mara waves at Hal")));
        assert!(!g.log.iter().any(|l| l.text.contains("You feel watched")), "what is said to the host alone stays theirs");
        // The guest picks up a stone by them: done in the host's world.
        let gp = h.sim.travelers[&1].a.pos;
        let mine = h.sim.spawn_thing(stone, gp + Vec3::new(0.6, 0.3, 0.0), 0.0, 1.0, Default::default(), true).unwrap();
        until(&mut [&mut h, &mut g], "the guest to see the stone by them", |gs| gs[1].sim.things.get(mine).is_some());
        g.player_act(crate::sim::Action::Hold { target: crate::sim::Target::Thing(mine) });
        until(&mut [&mut h, &mut g], "the stone in the guest's hand", |gs| gs[1].sim.player.held == Some(mine));
        assert_eq!(h.sim.travelers[&1].a.held, Some(mine), "the host's world says so");
        assert_eq!(h.sim.things.get(mine).and_then(|t| t.holder), Some(ActorId::Traveler(1)));
        // Hurt on the host, the guest knows it.
        h.sim.wound(ActorId::Traveler(1), 0.3, ActorId::Npc(mara), "pushed");
        until(&mut [&mut h, &mut g], "the guest to feel it", |gs| (gs[1].sim.health() - 0.7).abs() < 0.01);
        g.shutdown();
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
