//! One scorer for every body. Each moment someone is free, the world offers
//! them things to do (affordances): a verb, what it is done to or with, how
//! much it relieves each of their needs, and what it costs. They come from
//! things by their properties (food for hunger, toys and marks for fun, new
//! things for curiosity, harm to get away from), from people (company,
//! weighted by how they feel), from places (home, out of harm's way), from
//! trouble (push it back, or watch) and from goals (the next step toward
//! each). Each is scored by one formula, need × relief × how much their
//! nature cares, minus cost, and one is picked by a weighted draw from the
//! world's seeded numbers: variety, but a seed replays the same. Harm and
//! threats are very high scores, not fixed rungs.
//!
//! Species and mind only decide which verbs a body has (no words: no talk;
//! a simple mind doesn't make things). With an LLM, the planner is the main
//! brain: it hears the best few things to do as a menu of what is really
//! there and may still choose freely; between its decisions, and with no
//! LLM at all, this is the brain.

use super::actions::Action;
use super::npc::{Aim, CounterPlan, template_line};
use super::props::*;
use super::things::ThingId;
use super::{ActorId, Sim, Target};
use crate::world::species::Mind;
use glam::Vec3;

/// How close to the best a choice must score to be drawn at all, and how
/// sharply the draw favours the best.
const SPREAD: f32 = 0.2;
const TEMPER: f32 = 0.07;
/// What staying safe is worth against any need: harm and threats always win.
const SAFETY: f32 = 3.0;
/// The most choices the planner hears as a menu.
const MENU: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Need {
    Hunger,
    Fatigue,
    Social,
    Fun,
    Curiosity,
}

/// What in someone's nature makes them care about a verb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trait {
    Sociable,
    Playful,
    Curious,
    Crafty,
}

/// The kinds of things anyone can do: the rows of the table below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Eat,
    Seek,
    Play,
    Toss,
    Look,
    Explore,
    Rest,
    Gather,
    Work,
    Follow,
    Group,
    Dash,
    Wander,
}

/// One row of the hand-made table: which need a verb relieves and how much,
/// which trait makes someone care (as a multiplier: base + trait), and a
/// flat worth for verbs that relieve no need (also scaled by a trait).
struct Row {
    kind: Kind,
    need: Option<Need>,
    /// Only when the need is at least this pressing.
    gate: f32,
    relief: f32,
    care: Option<(f32, Trait)>,
    flat: f32,
    flat_trait: Option<(f32, Trait)>,
}

/// The one table of hand design: which verb relieves which need. Everything
/// else (what with, where, how far, how good the food or the company) comes
/// from the world's data.
const TABLE: &[Row] = &[
    Row { kind: Kind::Eat, need: Some(Need::Hunger), gate: 0.45, relief: 1.1, care: None, flat: 0.0, flat_trait: None },
    Row { kind: Kind::Seek, need: Some(Need::Social), gate: 0.0, relief: 0.6, care: Some((0.5, Trait::Sociable)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Play, need: Some(Need::Fun), gate: 0.0, relief: 1.2, care: Some((0.4, Trait::Playful)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Toss, need: Some(Need::Fun), gate: 0.0, relief: 0.75, care: Some((0.0, Trait::Playful)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Look, need: Some(Need::Curiosity), gate: 0.0, relief: 1.3, care: Some((0.5, Trait::Curious)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Explore, need: Some(Need::Curiosity), gate: 0.6, relief: 0.35, care: Some((0.5, Trait::Curious)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Rest, need: Some(Need::Fatigue), gate: 0.6, relief: 0.8, care: None, flat: 0.0, flat_trait: None },
    Row { kind: Kind::Gather, need: None, gate: 0.0, relief: 0.0, care: None, flat: 0.18, flat_trait: Some((0.35, Trait::Crafty)) },
    Row { kind: Kind::Work, need: None, gate: 0.0, relief: 0.0, care: None, flat: 0.1, flat_trait: Some((0.45, Trait::Crafty)) },
    Row { kind: Kind::Follow, need: Some(Need::Social), gate: 0.0, relief: 0.6, care: None, flat: 0.35, flat_trait: None },
    Row { kind: Kind::Group, need: Some(Need::Social), gate: 0.0, relief: 0.6, care: None, flat: 0.3, flat_trait: None },
    Row { kind: Kind::Dash, need: Some(Need::Fun), gate: 0.5, relief: 0.7, care: Some((0.0, Trait::Playful)), flat: 0.0, flat_trait: None },
    Row { kind: Kind::Wander, need: None, gate: 0.0, relief: 0.0, care: None, flat: 0.15, flat_trait: None },
];

/// What someone could do now, with what it needs to be carried out.
#[derive(Clone, Debug)]
pub enum Verb {
    /// Get away from something that would hunt them.
    Flee { from: ActorId, at: Vec3 },
    /// What they wear harms them.
    TakeOff(ThingId),
    /// What they hold harms them.
    LetGo(ThingId),
    /// Push back trouble near them (as imagined beforehand).
    Counter(CounterPlan),
    /// Keep clear of something harmful.
    Avoid { at: Vec3, size: f32, what: String },
    /// Watch trouble from a safe distance.
    Watch { at: Vec3, extent: f32, noun: String },
    /// Go after prey, or take it once caught up.
    Hunt(ActorId),
    Catch(ActorId),
    /// Bring back what someone threw.
    Fetch { thing: ThingId, by: ActorId },
    /// Bring what they hold to their person, or put it down.
    Bring(Option<ActorId>),
    /// Eat this (None: what they hold; no target at all: nothing in sight).
    Eat(Option<Target>, bool),
    /// Seek someone's company.
    Seek { other: ActorId, aff: f32 },
    /// Play with a toy: a game with someone, or throwing it at a mark.
    Play(ThingId),
    /// Toss a loose thing for fun.
    Toss(Target),
    /// Look at something new.
    Look { at: Vec3, what: String },
    /// Go and see a place they haven't been lately.
    Explore(Vec3),
    Rest,
    /// Bring a loose thing home.
    Gather(Target),
    /// Their craft (`now`: the maker clock is due).
    Work { now: bool },
    /// Nothing to do, and bored of it.
    Bored,
    /// Keep with their person or parent.
    Follow(ActorId),
    /// Keep with their herd or pack.
    Group(Vec3),
    /// Dash about for fun.
    Dash(Vec3),
    /// The next step toward a goal.
    Goal { id: u64, steps: Vec<Action>, what: String },
    /// Get out of the weather (`from`) to somewhere (`place`: home, indoors, under a tree).
    Shelter { to: Target, place: String, from: String },
    /// Under a roof: stay there until it passes.
    StayIn(String),
    /// Go out into weather they love.
    GoOut(Vec3, String),
    Wander,
}

#[derive(Clone, Debug)]
pub struct Affordance {
    pub verb: Verb,
    pub score: f32,
    /// In words, for the planner's menu ("eat the apple (6 m)").
    pub label: String,
}

fn row(k: Kind) -> &'static Row {
    TABLE.iter().find(|r| r.kind == k).expect("every kind has a row")
}

impl Sim {
    /// How much someone's nature makes them care, 0..1 (people by their
    /// traits, animals by their temper).
    fn care(&self, cid: i64, t: Trait) -> f32 {
        let Some(n) = self.cast.get(cid) else { return 0.5 };
        if n.species.mind == Mind::Sapient {
            match t {
                Trait::Sociable => n.traits.sociable,
                Trait::Playful => n.traits.playful,
                Trait::Curious => n.traits.curious,
                Trait::Crafty => n.traits.crafty,
            }
        } else {
            match t {
                Trait::Playful => n.temper.playful,
                _ => 0.5,
            }
        }
    }

    fn need_of(&self, cid: i64, need: Need) -> f32 {
        let Some(n) = self.cast.get(cid) else { return 0.0 };
        let x = &n.needs;
        match need {
            Need::Hunger => x.hunger,
            Need::Fatigue => x.fatigue,
            Need::Social => x.social,
            Need::Fun => x.fun,
            Need::Curiosity => x.curiosity,
        }
    }

    /// The one formula: need × relief × care, plus a flat worth, times how
    /// good this particular one is (`quality`: the food, the company), less
    /// what it costs. None when its need isn't pressing enough.
    fn worth(&self, cid: i64, kind: Kind, quality: f32, cost: f32) -> Option<f32> {
        let r = row(kind);
        let mut s = r.flat + r.flat_trait.map(|(k, t)| k * self.care(cid, t)).unwrap_or(0.0);
        if let Some(need) = r.need {
            let v = self.need_of(cid, need);
            if v < r.gate {
                return None;
            }
            let care = r.care.map(|(base, t)| base + self.care(cid, t)).unwrap_or(1.0);
            s += v * r.relief * quality * care;
        }
        Some(s - cost)
    }

    /// Everything someone could do now, scored.
    pub fn affordances(&mut self, cid: i64) -> Vec<Affordance> {
        let mut out: Vec<Affordance> = Vec::new();
        let me = ActorId::Npc(cid);
        let Some(n) = self.cast.get(cid) else { return out };
        let (pos, home, held, needs) = (n.a.pos, n.def.home, n.a.held, n.needs);
        let mind = self.mind_of(me);
        let sapient = mind == Mind::Sapient;
        let playful = self.care(cid, Trait::Playful);
        let vocab = self.vocab.clone();
        let harm_of = |s: &Sim, id: ThingId| s.things.get(id).map(|t| vocab.harm(&t.props)).unwrap_or(0.0);
        let mut add = |verb: Verb, score: f32, label: String| out.push(Affordance { verb, score, label });

        // Safety: harm on them, threats, trouble.
        if let Some((from, at)) = self.threat(me) {
            add(Verb::Flee { from, at }, SAFETY + 0.5, format!("get away from {}", self.actor_name(from)));
        }
        if sapient {
            if let Some(id) = self.worn_by(me).into_iter().find(|id| harm_of(self, *id) > 0.05) {
                add(Verb::TakeOff(id), SAFETY + 0.2 + harm_of(self, id), format!("take off the {} (it harms you)", self.thing_name(id)));
            }
        }
        if let Some(h) = held.filter(|h| harm_of(self, *h) > 0.05) {
            add(Verb::LetGo(h), SAFETY + 0.1 + harm_of(self, h), format!("let go of the {} (it harms you)", self.thing_name(h)));
        }
        if let Some(p) = self.counter_plan(cid) {
            let label = format!("fight the {}{}", p.noun, p.tool.map(|t| format!(" with the {}", self.thing_name(t))).unwrap_or_else(|| " with bare hands".into()));
            add(Verb::Counter(p), SAFETY, label);
        }
        if let Some((at, size, what)) = self.hazards_near(pos, 9.0).into_iter().next() {
            add(Verb::Avoid { at, size, what: what.clone() }, SAFETY - 0.3, format!("get away from the {what}"));
        }
        // Weather: out of what they mind, or out into what they love.
        let feel = self.weather_feeling(cid);
        if feel.abs() > 0.25 {
            let outside = self.open_sky(pos);
            let what = self.wx.falls.as_ref().filter(|f| f.amount > 0.05).map(|f| f.what.clone()).unwrap_or_else(|| self.wx.name.clone());
            if outside && feel < 0.0 {
                if let Some((to, place)) = self.shelter_for(cid) {
                    add(Verb::Shelter { to, place: place.clone(), from: what.clone() }, 0.5 + 1.8 * -feel, format!("get out of the {what} ({place})"));
                }
            } else if !outside && feel < 0.0 {
                add(Verb::StayIn(what.clone()), 0.3 + 1.2 * -feel, format!("stay in until the {what} passes"));
            } else if !outside && feel > 0.0 && !self.night() {
                if let Some(door) = self.room_at(pos).and_then(|r| self.rooms[r].doors.first().map(|d| d.0)) {
                    let a = (cid as f32 * 2.4).sin();
                    add(Verb::GoOut(door + Vec3::new(a * 3.0, 0.0, a.cos() * 3.0), what.clone()), 0.3 + 0.8 * feel, format!("go out into the {what}"));
                }
            }
        }
        let tr = self.cast.get(cid).map(|n| n.traits).unwrap_or(super::npc::Traits::from_persona(&Default::default(), 0));
        if sapient && tr.curious + tr.brave > 0.9 && needs.curiosity > 0.2 {
            if let Some((c, extent, noun)) = self.incidents_near(pos, 45.0).first().map(|i| (i.centre(), i.extent(), i.words.noun.clone())) {
                add(Verb::Watch { at: c, extent, noun: noun.clone() }, 2.1, format!("watch the {noun} from a safe distance"));
            }
        }

        // Animals' own: the hunt, fetching, what is in their mouth.
        if !sapient {
            let chasing = self.cast.get(cid).is_some_and(|n| matches!(n.aim, Aim::Chase(_)));
            if let Some((prey, pp)) = self.prey_near(me, 60.0) {
                let d = (pp - pos).length();
                let hungry = needs.hunger > 0.5 && needs.fatigue < 0.8;
                if (chasing || hungry) && d < 1.8 + self.contact_gap(me, prey) {
                    add(Verb::Catch(prey), 2.5, format!("catch {}", self.actor_name(prey)));
                } else if (chasing && needs.fatigue < 0.9 && d < 45.0) || (hungry && d < 40.0) {
                    add(Verb::Hunt(prey), 1.6 + needs.hunger * 0.5, format!("hunt {}", self.actor_name(prey)));
                }
            }
            if held.is_none() && playful > 0.5 {
                if let Some((id, by)) = self.fetchable(cid, 30.0) {
                    add(Verb::Fetch { thing: id, by }, 1.5, format!("fetch the {} for {}", self.thing_name(id), self.actor_name(by)));
                }
            }
            if held.is_some() {
                let to = self.owner_of(cid).filter(|o| self.actor(*o).is_some_and(|a| (a.pos - pos).length() < 40.0 && a.held.is_none()));
                add(Verb::Bring(to), 1.45, "bring what you carry to your person, or put it down".into());
            }
        }

        // Food.
        let food = self.food_for(cid, pos, if sapient { 30.0 } else { 25.0 });
        let food_held = held.is_some_and(|h| self.things.get(h).is_some_and(|x| x.props[P_EDIBLE] > 0.0));
        if food_held {
            if let Some(s) = self.worth(cid, Kind::Eat, 1.0, 0.0) {
                add(Verb::Eat(None, true), s, format!("eat the {}", held.map(|h| self.thing_name(h)).unwrap_or_default()));
            }
        } else if let Some((tg, at)) = food.clone() {
            if let Target::Thing(f) = tg {
                self.see_thing(cid, f);
            }
            if let Some(s) = self.worth(cid, Kind::Eat, 1.0, 0.0) {
                add(Verb::Eat(Some(tg.clone()), true), s, format!("eat something ({:.0} m)", (at - pos).length()));
            }
        } else if sapient {
            if let Some(s) = self.worth(cid, Kind::Eat, 0.27, 0.0) {
                add(Verb::Eat(None, false), s, "look for something to eat".into());
            }
        }
        // Rest: by day for people (at night they sleep), any time for animals.
        if !sapient || !self.night() {
            let gate = if sapient { 0.7 } else { 0.6 };
            if needs.fatigue > gate {
                if let Some(s) = self.worth(cid, Kind::Rest, 1.0, 0.0) {
                    add(Verb::Rest, s, "rest a while".into());
                }
            }
        }

        if sapient {
            // Company, weighted by how they feel about them.
            if self.speaks(me) {
                if let Some((other, aff)) = self.best_company(cid, 35.0) {
                    if let Some(s) = self.worth(cid, Kind::Seek, (0.6 + aff.max(0.0)) / 0.6, 0.0) {
                        add(Verb::Seek { other, aff }, s, format!("spend time with {}", self.actor_name(other)));
                    }
                }
            }
            // Toys and what is aimed at.
            let ball = self.nearest_matching(pos, 30.0, is_toy);
            let ball_held = held.filter(|h| self.things.get(*h).is_some_and(|x| is_toy(&x.props)));
            let toy = ball_held.or_else(|| ball.as_ref().and_then(|(tg, _)| self.liven(tg)));
            if let Some(id) = toy {
                self.see_thing(cid, id);
                if let Some(s) = self.worth(cid, Kind::Play, 1.0, 0.0) {
                    add(Verb::Play(id), s, format!("play with the {}", self.thing_name(id)));
                }
            } else {
                // None in sight: go where they believe one lies (it may be gone).
                let toys: Vec<String> = self.things.live().filter(|t| is_toy(&t.props)).map(|t| self.thing_name(t.id)).collect();
                if let Some((_, at, sure)) = self.believed_where(cid, |n| toys.iter().any(|x| x == n), pos).filter(|(_, at, _)| (*at - pos).length() < 100.0) {
                    if let Some(s) = self.worth(cid, Kind::Play, 0.8 * sure, (at - pos).length() / 100.0) {
                        add(Verb::Goal { id: 0, steps: vec![Action::Goto { target: Target::Point(at.to_array()), run: false }], what: "go where the ball was".into() }, s, format!("go where you last saw something to play with ({:.0} m)", (at - pos).length()));
                    }
                }
            }
            // Something new to look at.
            if let Some((_, p, what)) = self.novelty_near(cid, pos, 40.0) {
                if let Some(s) = self.worth(cid, Kind::Look, 1.0, 0.0) {
                    add(Verb::Look { at: p, what: what.clone() }, s, format!("look at the new {what} ({:.0} m)", (p - pos).length()));
                }
            }
            // A place a little way off, to see what is there.
            if let Some(spot) = self.somewhere_new(cid, pos, home) {
                if let Some(s) = self.worth(cid, Kind::Explore, 1.0, 0.0) {
                    add(Verb::Explore(spot), s, format!("go and see what is {} of here", super::compass(spot - pos)));
                }
            }
            // Loose small things: bring some home, or toss one.
            let loose = if held.is_none() { self.loose_thing(pos, home, 14.0) } else { None };
            if let Some((tg, _)) = loose {
                let pile = self.things.near(home, 4.0).len() as f32;
                if (pos - home).length() > 6.0 {
                    if let Some(s) = self.worth(cid, Kind::Gather, 1.0, pile * 0.04) {
                        add(Verb::Gather(tg.clone()), s, "gather loose things to bring home".into());
                    }
                }
                if ball.is_none() {
                    if let Some(s) = self.worth(cid, Kind::Toss, 1.0, 0.0) {
                        add(Verb::Toss(tg), s, "toss a stone for fun".into());
                    }
                }
            }
            // Their craft: rare, and only when nothing is being built (each
            // piece of work may add a type to the world).
            let t = self.t;
            let can_work = self.has_llm && self.interp.building.is_empty() && t - self.cast.last_work > self.cfg.work_gap_secs as f64 && self.cast.get(cid).is_some_and(|n| t - n.last_work > self.cfg.work_secs as f64 && t >= n.next_llm);
            let maker = self.maker_turn(cid);
            if maker {
                add(Verb::Work { now: true }, 2.0, "make something with your craft".into());
            } else if can_work {
                if let Some(s) = self.worth(cid, Kind::Work, 1.0, 0.0) {
                    add(Verb::Work { now: false }, s, "make or mend something with your craft".into());
                }
            }
        } else {
            // The young keep to a parent, pets to their person.
            let owner = self.parent_of(cid).or_else(|| self.owner_of(cid));
            if let Some((o, op)) = owner.and_then(|o| self.actor(o).filter(|a| !a.asleep).map(|a| (o, a.pos))) {
                let d = (op - pos).length();
                if d < 80.0 && d > 4.0 {
                    if let Some(s) = self.worth(cid, Kind::Follow, 1.0, 0.0) {
                        add(Verb::Follow(o), s, format!("keep with {}", self.actor_name(o)));
                    }
                }
            }
            // Herds and packs keep together.
            if self.species_of(me).is_some_and(|s| s.group()) {
                let kin = self.kind_near(me, pos, 80.0);
                if !kin.is_empty() {
                    let c = kin.iter().fold(Vec3::ZERO, |a, (_, p)| a + *p) / kin.len() as f32;
                    let d = (c - pos).length();
                    if d > 10.0 {
                        if let Some(s) = self.worth(cid, Kind::Group, 1.0, -(d / 60.0).min(0.4)) {
                            add(Verb::Group(c), s, "rejoin the others".into());
                        }
                    }
                }
            }
            if playful > 0.5 {
                if let Some(s) = self.worth(cid, Kind::Dash, 1.0, 0.0) {
                    let centre = owner.and_then(|o| self.actor(o).map(|a| a.pos)).unwrap_or(home);
                    add(Verb::Dash(centre), s, "dash about".into());
                }
            }
        }

        // Goals: the next step toward each.
        for (id, steps, what, priority, dist) in self.goal_steps(cid) {
            add(Verb::Goal { id, steps, what: what.clone() }, 0.6 + priority * 0.6 - dist / 60.0, what);
        }

        // Nothing better: wander (and, bored enough, get restless).
        let r = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0);
        let wander = self.worth(cid, Kind::Wander, 1.0, -0.1 * r).unwrap_or(0.15);
        let best_else = out.iter().map(|a| a.score).fold(f32::MIN, f32::max);
        if sapient && needs.fun > 0.85 && best_else <= wander {
            out.push(Affordance { verb: Verb::Bored, score: 0.5, label: "find something to do (you are bored)".into() });
        }
        out.push(Affordance { verb: Verb::Wander, score: wander, label: "wander about".into() });
        out
    }

    /// For each open goal the world can act on, the next step and how far
    /// it is: (goal, steps, words, priority, distance).
    fn goal_steps(&mut self, cid: i64) -> Vec<(u64, Vec<Action>, String, f32, f32)> {
        use super::goals::{Want, fits};
        let me = ActorId::Npc(cid);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return Vec::new() };
        let held = self.actor(me).and_then(|a| a.held);
        let goals: Vec<(u64, Want, String, f32)> = self.goals_of(cid).into_iter().map(|g| (g.id, g.want.clone(), g.text.clone(), g.priority)).collect();
        let mut out = Vec::new();
        for (id, want, text, priority) in goals {
            let held_fits = |s: &Sim, what: &str| held.is_some_and(|h| fits(what, &s.thing_name(h)));
            let loose_fitting = |s: &mut Sim, what: &str| {
                let ids = s.things.near(pos, 30.0);
                ids.into_iter().filter_map(|i| s.things.get(i)).filter(|t| !t.held() && t.worn.is_none() && t.liftable(1)).find(|t| fits(what, &s.thing_name(t.id))).map(|t| (t.id, (t.pos - pos).length()))
            };
            match want {
                Want::Has { who, what } if who != me => {
                    let there = self.actor(who).map(|a| (a.pos - pos).length()).unwrap_or(1e9);
                    if held_fits(self, &what) {
                        out.push((id, vec![Action::Goto { target: Target::Actor(who), run: false }, Action::Give { to: Target::Actor(who) }], format!("{text}: hand it over"), priority, there));
                    } else if let Some((t, d)) = loose_fitting(self, &what) {
                        let mut steps = Vec::new();
                        if held.is_some() {
                            steps.push(Action::Drop);
                        }
                        steps.extend([Action::Hold { target: Target::Thing(t) }, Action::Goto { target: Target::Actor(who), run: false }, Action::Give { to: Target::Actor(who) }]);
                        out.push((id, steps, format!("{text}: fetch one and hand it over"), priority, d + there));
                    }
                }
                Want::Hold { what } => {
                    if let Some((t, d)) = loose_fitting(self, &what) {
                        let mut steps = Vec::new();
                        if held.is_some() {
                            steps.push(Action::Drop);
                        }
                        steps.push(Action::Hold { target: Target::Thing(t) });
                        out.push((id, steps, format!("{text}: pick it up"), priority, d));
                    } else if let Some((_, at, sure)) = self.believed_where(cid, |n| fits(&what, n), pos) {
                        // Not in sight: where they believe one lies.
                        out.push((id, vec![Action::Goto { target: Target::Point(at.to_array()), run: false }], format!("{text}: go where you saw one"), priority * sure, (at - pos).length()));
                    }
                }
                Want::Be { place } => {
                    if let Some(t) = self.find_named(&place, pos, me) {
                        let d = self.resolve(&t, me).map(|r| (r.pos - pos).length()).unwrap_or(1e9);
                        if d > 5.0 && d < 300.0 {
                            out.push((id, vec![Action::Goto { target: t, run: false }], format!("{text}: go there"), priority, d * 0.3));
                        }
                    }
                }
                Want::Affection { with, .. } => {
                    if let Some(d) = self.actor(with).filter(|a| !a.asleep).map(|a| (a.pos - pos).length()).filter(|d| *d < 80.0) {
                        out.push((id, vec![Action::Goto { target: Target::Actor(with), run: false }, Action::Gesture { kind: "wave".into(), to: Some(Target::Actor(with)) }], format!("{text}: spend time with them"), priority * 0.6, d));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// A dry spot 30–60 m off, away from where they are, still within reach
    /// of home: somewhere to go and look about.
    fn somewhere_new(&mut self, cid: i64, pos: Vec3, home: Vec3) -> Option<Vec3> {
        for _ in 0..4 {
            let (a, r) = {
                let n = self.cast.get_mut(cid)?;
                (n.rand() * std::f32::consts::TAU, 30.0 + n.rand() * 30.0)
            };
            let p = pos + Vec3::new(a.cos(), 0.0, a.sin()) * r;
            if (p - home).length() < 120.0 && self.snap.terrain.height(p.x, p.z) > crate::terrain::WATER_LEVEL + 0.3 {
                return Some(Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z));
            }
        }
        None
    }

    /// Draw one, weighted toward the best (seeded per character).
    fn draw_choice(&mut self, cid: i64, mut v: Vec<Affordance>) -> Option<Affordance> {
        v.sort_by(|a, b| b.score.total_cmp(&a.score));
        let best = v.first()?.score;
        let pool: Vec<Affordance> = v.into_iter().take_while(|a| a.score > best - SPREAD).collect();
        let weights: Vec<f32> = pool.iter().map(|a| ((a.score - best) / TEMPER).exp()).collect();
        let sum: f32 = weights.iter().sum();
        let mut r = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * sum;
        for (a, w) in pool.iter().zip(&weights) {
            if r <= *w {
                return Some(a.clone());
            }
            r -= w;
        }
        pool.into_iter().next()
    }

    /// The best few things to do now, in words, for the planner's menu.
    pub fn menu_line(&mut self, cid: i64) -> Option<String> {
        let mut v = self.affordances(cid);
        v.sort_by(|a, b| b.score.total_cmp(&a.score));
        let mut seen = std::collections::HashSet::new();
        let items: Vec<String> = v.into_iter().filter(|a| seen.insert(a.label.clone())).take(MENU).map(|a| a.label).collect();
        (!items.is_empty()).then(|| items.join("; "))
    }

    /// A free moment: choose what to do and set out.
    pub(super) fn decide(&mut self, cid: i64) {
        let me = ActorId::Npc(cid);
        // Something it goes after (a night horror, a firefly drawn to you).
        if self.pursue_want(cid) {
            return;
        }
        // Carrying someone: it goes where it is steered, unless it takes
        // fright (then a mount that isn't fully tame throws its rider).
        if let Some(rider) = self.rider_of(me) {
            if let Some((from, at)) = self.threat(me) {
                if self.cast.get(cid).is_some_and(|n| n.temper.tame < 0.85) {
                    self.throw_rider(me);
                }
                self.flee(cid, from, at);
                return;
            }
            let rn = self.actor_name(rider);
            self.set_doing(cid, &format!("carrying {rn}"));
            self.think_again(cid, 1.0);
            return;
        }
        // What they believed lay about here and doesn't any more is forgotten.
        if let Some(pos) = self.actor(me).map(|a| a.pos) {
            self.look_about(cid, pos);
        }
        let v = self.affordances(cid);
        let safe = v.iter().all(|a| a.score < SAFETY - 0.5);
        // With an LLM, the planner is the main brain when its turn comes:
        // it hears the menu; meanwhile the scorer carries on.
        if safe && self.has_llm && !self.social.busy(me) {
            let what = match self.mind_of(me) {
                Mind::Sapient => "A free moment. What do you do next? Choose from what you could do, or anything else that fits who you are.".to_string(),
                _ => {
                    let sp = self.cast.get(cid).map(|n| n.species.name.clone()).unwrap_or_default();
                    format!("You are a {sp}: a simple mind, no words, only sounds and gestures. Choose what you do next from your needs and what is around you, as a {sp} would.")
                }
            };
            self.ask_planner_weighted(cid, "idle", &what, false, 0.3);
        }
        let Some(a) = self.draw_choice(cid, v) else { return };
        self.carry_out(cid, a.verb);
    }

    fn think_again(&mut self, cid: i64, secs: f64) {
        let t = self.t;
        if let Some(n) = self.cast.get_mut(cid) {
            n.think_at = t + secs;
        }
    }

    /// Set out to do what was chosen.
    fn carry_out(&mut self, cid: i64, verb: Verb) {
        let me = ActorId::Npc(cid);
        let t = self.t;
        let Some(n) = self.cast.get(cid) else { return };
        let (pos, home, held) = (n.a.pos, n.def.home, n.a.held);
        let sapient = self.mind_of(me) == Mind::Sapient;
        let hour = self.hour();
        let k = self.cast.get_mut(cid).map(|n| crate::noise::pcg(n.rng)).unwrap_or(0);
        let bump = |s: &mut Sim, f: &dyn Fn(&mut crate::world::characters::Needs)| {
            if let Some(n) = s.cast.get_mut(cid) {
                f(&mut n.needs);
            }
        };
        match verb {
            Verb::Flee { from, at } => self.flee(cid, from, at),
            Verb::TakeOff(id) => {
                let what = self.thing_name(id);
                let line = template_line("alarm", &what, hour, k % 100);
                self.say(me, &line, None);
                let mut steps = Vec::new();
                if held.is_some() {
                    steps.push(Action::Drop);
                }
                steps.push(Action::TakeOff { target: Some(Target::Thing(id)), from: None });
                steps.push(Action::Drop);
                self.plan(me, steps, &format!("get the {what} off"), false);
                self.set_aim(cid, Aim::Avoid, &format!("tearing off the {what}"));
                self.think_again(cid, 2.0);
            }
            Verb::LetGo(h) => {
                let what = self.thing_name(h);
                self.plan(me, vec![Action::Drop], &format!("let go of the {what}"), false);
                self.set_aim(cid, Aim::Avoid, &format!("dropping the {what}"));
                self.think_again(cid, 2.0);
            }
            Verb::Counter(p) => {
                self.do_counter(cid, p);
                self.think_again(cid, 1.2);
            }
            Verb::Avoid { at, size, what } => {
                let away = (pos - at).normalize_or_zero();
                let dest = pos + if away == Vec3::ZERO { Vec3::X } else { away } * (12.0 + size * 2.0);
                if sapient {
                    let line = template_line("alarm", &what, hour, k % 100);
                    self.say(me, &line, None);
                }
                self.plan(me, vec![Action::Goto { target: Target::Point(dest.to_array()), run: true }], &format!("get away from the {what}"), false);
                self.set_aim(cid, Aim::Avoid, &format!("getting away from the {what}"));
                self.think_again(cid, 3.0);
            }
            Verb::Watch { at, extent, noun } => {
                let d = (at - pos).length();
                let spot = at + (pos - at).normalize_or_zero() * (12.0 + extent * 0.5).min(d);
                self.plan(me, vec![Action::Goto { target: Target::Point(spot.to_array()), run: false }, Action::Wait { secs: 8.0 }], &format!("watch the {noun}"), false);
                self.set_doing(cid, &format!("watching the {noun}"));
                self.think_again(cid, 6.0);
            }
            Verb::Hunt(prey) => self.chase(cid, prey),
            Verb::Catch(prey) => {
                self.caught(cid, prey);
                self.set_doing(cid, "panting");
                self.think_again(cid, 6.0);
            }
            Verb::Fetch { thing, by } => {
                self.plan(me, vec![Action::Hold { target: Target::Thing(thing) }, Action::Goto { target: Target::Actor(by), run: true }, Action::Give { to: Target::Actor(by) }], "fetch", false);
                self.set_doing(cid, "fetching");
                bump(self, &|x| x.fun = (x.fun - 0.15).max(0.0));
                self.event("fetch", Some(me), Some(format!("thing:{thing}")), format!("{} runs after the {}", self.actor_name(me), self.thing_name(thing)), Some(pos), serde_json::json!({ "for": by }));
                self.think_again(cid, 3.0);
            }
            Verb::Bring(to) => {
                match to {
                    Some(o) => self.plan(me, vec![Action::Goto { target: Target::Actor(o), run: false }, Action::Give { to: Target::Actor(o) }], "bring it back", false),
                    None => self.plan(me, vec![Action::Drop], "", false),
                }
                self.think_again(cid, 3.0);
            }
            Verb::Eat(target, found) => {
                match (target, found) {
                    (None, true) => {
                        if let Some(h) = held {
                            self.plan(me, vec![Action::Eat { target: Some(Target::Thing(h)) }], "eat", false);
                        }
                    }
                    (Some(tg), _) => {
                        let mut steps = Vec::new();
                        if held.is_some() && sapient {
                            steps.push(Action::Drop);
                        }
                        steps.push(Action::Eat { target: Some(tg) });
                        self.plan(me, steps, if sapient { "find something to eat" } else { "eat" }, false);
                    }
                    (None, false) => {
                        self.ask(cid, "hungry", "You are hungry and see nothing to eat nearby.");
                        self.wander(cid, home, 25.0);
                    }
                }
                self.set_doing(cid, if sapient { "looking for food" } else { "grazing" });
                self.think_again(cid, 4.0);
            }
            Verb::Seek { other, aff } => {
                let oname = self.actor_name(other);
                let mut steps = vec![Action::Goto { target: Target::Actor(other), run: false }];
                if other == ActorId::Player {
                    let first = oname.split_whitespace().next().unwrap_or("").to_string();
                    steps.push(Action::Say { text: template_line("greet", if first == "the" { "traveler" } else { &first }, hour, k), to: Some(Target::Actor(other)) });
                }
                let hug_ok = aff > 0.6 && other != ActorId::Player && self.social.rel(me, other).is_some_and(|r| r.family || r.partner || r.affection > 0.7);
                let kind = if hug_ok && k % 3 == 0 { "hug" } else { "wave" };
                steps.push(Action::Gesture { kind: kind.into(), to: Some(Target::Actor(other)) });
                self.plan(me, steps, &format!("spend time with {oname}"), false);
                if let ActorId::Npc(o) = other {
                    self.social.want_chat(cid, o, t);
                }
                bump(self, &|x| x.social = (x.social - 0.35).max(0.0));
                self.set_doing(cid, &format!("seeking out {oname}"));
                self.think_again(cid, 8.0);
            }
            Verb::Play(ball_id) => {
                // Someone to play with, unless they were asked a moment ago (no nagging).
                let asked = |s: &Sim, p: ActorId| s.social.asked_at.get(&(me.code(), p.code())).is_some_and(|at| t - at < 60.0);
                if let Some(p) = self.best_company(cid, 25.0).map(|x| x.0).filter(|p| !asked(self, *p)) {
                    if self.propose(me, p, "catch", Some(ball_id)).is_ok() {
                        self.set_doing(cid, "suggesting a game of catch");
                        self.think_again(cid, 6.0);
                        return;
                    }
                }
                // Alone: throw it at something made to be aimed at, or up in the air.
                let mut steps = Vec::new();
                if held != Some(ball_id) {
                    if held.is_some() {
                        steps.push(Action::Drop);
                    }
                    steps.push(Action::Hold { target: Target::Thing(ball_id) });
                }
                match self.throwing_mark(pos, 30.0) {
                    Some((mt, mp)) => {
                        let stand = mp + (pos - mp).normalize_or_zero() * 5.0;
                        steps.push(Action::Goto { target: Target::Point(stand.to_array()), run: false });
                        steps.push(Action::Throw { at: Some(mt), dir: None, force: None });
                        self.set_doing(cid, "throwing at a target");
                    }
                    None => {
                        steps.push(Action::Throw { at: None, dir: Some([0.1, 1.0, 0.1]), force: Some(6.0) });
                        self.set_doing(cid, "tossing a ball");
                    }
                }
                self.plan(me, steps, "play", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fun = (n.needs.fun - 0.2).max(0.0);
                    n.a.catching = t + 4.0;
                }
                self.think_again(cid, 5.0);
            }
            Verb::Toss(tg) => {
                let throw = match self.throwing_mark(pos, 25.0) {
                    Some((mt, _)) => Action::Throw { at: Some(mt), dir: None, force: None },
                    None => {
                        let a = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * std::f32::consts::TAU;
                        Action::Throw { at: None, dir: Some([a.cos(), 0.6, a.sin()]), force: Some(7.0 + 5.0 * (a.sin() * 0.5 + 0.5)) }
                    }
                };
                self.plan(me, vec![Action::Hold { target: tg }, throw], "throw something for fun", false);
                bump(self, &|x| x.fun = (x.fun - 0.12).max(0.0));
                self.set_doing(cid, "throwing stones");
                self.think_again(cid, 5.0);
            }
            Verb::Look { at, what } => {
                let stand = at + (pos - at).normalize_or_zero() * 3.0;
                self.plan(me, vec![Action::Goto { target: Target::Point(stand.to_array()), run: false }, Action::Wait { secs: 5.0 }], &format!("look at the {what}"), false);
                bump(self, &|x| x.curiosity = (x.curiosity - 0.4).max(0.0));
                self.social.seen_novelty(cid, &what);
                self.set_doing(cid, &format!("looking at the {what}"));
                self.think_again(cid, 6.0);
            }
            Verb::Shelter { to, place, from } => {
                let hurry = self.weather_feeling(cid) < -0.6;
                self.plan(me, vec![Action::Goto { target: to, run: hurry }, Action::Wait { secs: 20.0 }], &format!("get out of the {from}"), false);
                self.set_doing(cid, &format!("sheltering from the {from} ({place})"));
                self.think_again(cid, 25.0);
            }
            Verb::StayIn(from) => {
                self.plan(me, vec![Action::Wait { secs: 20.0 }], &format!("wait out the {from}"), false);
                self.set_doing(cid, &format!("waiting out the {from}"));
                self.think_again(cid, 20.0);
            }
            Verb::GoOut(at, what) => {
                self.plan(me, vec![Action::Goto { target: Target::Point(at.to_array()), run: false }, Action::Wait { secs: 12.0 }], &format!("go out into the {what}"), false);
                self.set_doing(cid, &format!("out in the {what}"));
                self.think_again(cid, 15.0);
            }
            Verb::Explore(spot) => {
                self.plan(me, vec![Action::Goto { target: Target::Point(spot.to_array()), run: false }, Action::Wait { secs: 4.0 }], "see what is there", false);
                bump(self, &|x| x.curiosity = (x.curiosity - 0.3).max(0.0));
                self.set_doing(cid, "exploring");
                self.think_again(cid, 8.0);
            }
            Verb::Rest => {
                self.plan(me, vec![Action::Gesture { kind: "sit".into(), to: None }, Action::Wait { secs: 15.0 }], "rest", false);
                bump(self, &|x| x.fatigue = (x.fatigue - 0.25).max(0.0));
                self.set_doing(cid, if sapient { "resting" } else { "lying down" });
                self.think_again(cid, 16.0);
            }
            Verb::Gather(tg) => {
                let n = self.things.near(home, 4.0).len() as f32;
                let a = n * 1.1 + cid as f32;
                let spot = home + Vec3::new(a.cos(), 0.0, a.sin()) * (1.4 + (n * 0.15).min(1.5));
                let spot = Vec3::new(spot.x, self.snap.terrain.height(spot.x, spot.z), spot.z);
                self.plan(me, vec![Action::Hold { target: tg }, Action::Goto { target: Target::Point(spot.to_array()), run: false }, Action::Place { at: spot.to_array() }], "bring it home", false);
                self.set_doing(cid, "gathering things to bring home");
                self.think_again(cid, 8.0);
            }
            Verb::Work { now } => {
                self.cast.last_work = t;
                if let Some(n) = self.cast.get_mut(cid) {
                    n.last_work = t;
                }
                let crowded = if self.loose_near(pos, 15.0) > self.cfg.max_loose {
                    " Many things already lie about here: make your new thing from some of them, using them up, rather than adding more."
                } else {
                    ""
                };
                let what = format!("You have a little time for your craft or daily work. Look at what is around you: your tools, things lying about, things others made. Make something new from one or two of them, or fix or improve something, in a way that fits who you are: hold what you work with, then a \"do\" step in words that names what you use. What you make stays here for anyone to use.{crowded} If nothing fits, carry on as you were.");
                self.ask_planner(cid, "work", &what, now);
                self.set_doing(cid, "thinking about work");
                self.think_again(cid, 8.0);
            }
            Verb::Bored => {
                let since = self.cast.get(cid).map(|n| n.bored_since).unwrap_or(f64::MAX);
                if since == f64::MAX {
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.bored_since = t;
                    }
                } else if t - since > 45.0 {
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.bored_since = f64::MAX;
                    }
                    self.ask(cid, "bored", "You are bored: nothing to play with and nobody around to play with. You could make something, fix or improve something nearby, find someone, or go somewhere.");
                }
                self.wander(cid, home, 30.0);
                self.set_doing(cid, "at a loose end");
                self.think_again(cid, 6.0);
            }
            Verb::Follow(o) => {
                let last = self.cast.get(cid).map(|n| n.last_greet).unwrap_or(f64::MIN);
                let mut steps = vec![Action::Goto { target: Target::Actor(o), run: true }];
                // A greeting after time apart (with a trick it was taught).
                if t - last > 120.0 {
                    let trick = self.cast.get(cid).and_then(|n| n.tricks.last().cloned());
                    steps.push(Action::Gesture { kind: trick.unwrap_or_else(|| "wag".into()), to: None });
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.last_greet = t;
                    }
                    self.make_noise(cid, true);
                }
                steps.push(Action::Follow { target: Target::Actor(o), secs: Some(10.0) });
                let oname = self.actor_name(o);
                self.plan(me, steps, &format!("stay with {oname}"), false);
                bump(self, &|x| x.social = (x.social - 0.2).max(0.0));
                self.set_aim(cid, Aim::Follow(o), &format!("following {oname}"));
                self.think_again(cid, 4.0);
            }
            Verb::Group(c) => {
                let a = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * std::f32::consts::TAU;
                let p = c + Vec3::new(a.cos(), 0.0, a.sin()) * 3.0;
                self.plan(me, vec![Action::Goto { target: Target::Point([p.x, 0.0, p.z]), run: false }], "keep with the others", false);
                bump(self, &|x| x.social = (x.social - 0.25).max(0.0));
                self.set_doing(cid, "rejoining the others");
                self.think_again(cid, 5.0);
            }
            Verb::Dash(centre) => {
                let a = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * std::f32::consts::TAU;
                let p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * 6.0;
                self.plan(me, vec![Action::Goto { target: Target::Point([p.x, 0.0, p.z]), run: true }, Action::Gesture { kind: "wag".into(), to: None }], "play", false);
                bump(self, &|x| x.fun = (x.fun - 0.2).max(0.0));
                self.set_doing(cid, "dashing about");
                self.think_again(cid, 4.0);
            }
            Verb::Goal { id, steps, what } => {
                if let Some(g) = self.goals.list.iter_mut().find(|g| g.id == id && id != 0) {
                    g.progress = g.progress.max(0.1);
                }
                self.plan(me, steps, &what, false);
                self.set_doing(cid, &what);
                self.think_again(cid, 4.0);
            }
            Verb::Wander => {
                // Animals keep near their person or herd; people near home.
                let centre = if sapient {
                    home
                } else {
                    let owner = self.parent_of(cid).or_else(|| self.owner_of(cid)).and_then(|o| self.actor(o).filter(|a| !a.asleep).map(|a| a.pos));
                    let kin = self.kind_near(me, pos, 80.0);
                    owner.or_else(|| (!kin.is_empty()).then(|| kin.iter().fold(Vec3::ZERO, |a, (_, p)| a + *p) / kin.len() as f32)).unwrap_or(home)
                };
                self.wander(cid, centre, if sapient { 17.0 } else { 12.0 });
                self.set_doing(cid, if sapient { "pottering about" } else { "sniffing about" });
                let r = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.5);
                self.think_again(cid, if sapient { 3.0 } else { 4.0 } + r as f64 * 6.0);
            }
        }
    }
}
