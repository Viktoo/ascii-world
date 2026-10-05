//! How open a spot is to the sky and the weather, 0..1: high on a bare
//! hilltop or ridge, low in a valley, among trees or indoors. One number any
//! system can read (the wind's sound now; later the weather, drying, fire,
//! where plants and people choose to be). Nothing here knows what wind is.

use super::Sim;
use crate::terrain::WATER_LEVEL;
use glam::Vec3;

/// How far around the ground is compared, to tell a hilltop from a hollow.
const RINGS: [f32; 2] = [25.0, 60.0];
/// Rising this far above the land around counts as fully up.
const PROMINENT: f32 = 12.0;
/// And this far above the water.
const HIGH: f32 = 60.0;
/// How far around standing growth (trees, rocks) gives shelter.
const SHELTER: f32 = 22.0;

impl Sim {
    /// How exposed `p` is: 0 sheltered or indoors, 1 a bare summit.
    pub fn exposure(&mut self, p: Vec3) -> f32 {
        if self.room_at(p).is_some() {
            return 0.0;
        }
        let snap = self.snap.clone();
        let t = &snap.terrain;
        let ground = t.height(p.x, p.z);
        // Above the land around: the mean height on two rings.
        let mut sum = 0.0;
        let mut n = 0.0;
        for r in RINGS {
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                sum += t.height(p.x + a.sin() * r, p.z + a.cos() * r);
                n += 1.0;
            }
        }
        let rise = ((ground - sum / n) / PROMINENT).clamp(0.0, 1.0);
        let high = ((ground - WATER_LEVEL) / HIGH).clamp(0.0, 1.0);
        let lift = 0.6 * rise + 0.4 * high;
        // Standing things round about break it up.
        let solids = self.cache.items_near(&snap, p, SHELTER).iter().filter(|i| i.solid).count();
        let open = 1.0 / (1.0 + solids as f32 * 0.12);
        ((0.25 + 0.75 * lift) * open).clamp(0.0, 1.0)
    }
}
