//! Zone system — the map layout brain. A seeded jittered-grid Voronoi
//! assigns every region of the world a zone kind; samples get a smooth
//! (effectively C1: Gaussian-kernel) weight vector over kinds, so height
//! character, ground color and scatter blend across borders without seams.
//! Pure functions of world coordinates + seed.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::noise::Noise2;
use crate::palette::srgb;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    Mountains,
    Forest,
    Plains,
    Desert,
    Swamp,
    Lake,
    Coast,
    Badlands,
}

pub const KINDS: [ZoneKind; 8] = [
    ZoneKind::Mountains,
    ZoneKind::Forest,
    ZoneKind::Plains,
    ZoneKind::Desert,
    ZoneKind::Swamp,
    ZoneKind::Lake,
    ZoneKind::Coast,
    ZoneKind::Badlands,
];

pub const NK: usize = 8;

impl ZoneKind {
    pub fn index(self) -> usize {
        KINDS.iter().position(|k| *k == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        match self {
            ZoneKind::Mountains => "mountains",
            ZoneKind::Forest => "forest",
            ZoneKind::Plains => "plains",
            ZoneKind::Desert => "desert",
            ZoneKind::Swamp => "swamp",
            ZoneKind::Lake => "lake",
            ZoneKind::Coast => "coast",
            ZoneKind::Badlands => "badlands",
        }
    }
}

/// Recipe surface: `"zones":[{"kind":"forest","weight":2},
/// {"kind":"lake","at":[300,-500],"radius":400}]`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ZoneSpec {
    pub kind: ZoneKind,
    #[serde(default = "d_one")]
    pub weight: f32,
    /// Pin this zone at a world position (instead of random placement).
    #[serde(default)]
    pub at: Option<[f32; 2]>,
    #[serde(default = "d_pin_radius")]
    pub radius: f32,
}
fn d_one() -> f32 { 1.0 }
fn d_pin_radius() -> f32 { 600.0 }

/// Ground color ramp per zone (low→high, 4 stops, linear color space).
pub fn ground_ramp(kind: ZoneKind) -> [Vec3; 4] {
    match kind {
        ZoneKind::Mountains => [
            srgb(104, 122, 78),
            srgb(112, 104, 88),
            srgb(96, 90, 82),
            srgb(120, 116, 110),
        ],
        ZoneKind::Forest => [
            srgb(86, 118, 60),
            srgb(66, 104, 52),
            srgb(56, 92, 48),
            srgb(72, 96, 58),
        ],
        ZoneKind::Plains => [
            srgb(148, 158, 86),
            srgb(122, 150, 74),
            srgb(104, 136, 66),
            srgb(96, 120, 62),
        ],
        ZoneKind::Desert => [
            srgb(224, 198, 142),
            srgb(212, 180, 122),
            srgb(196, 158, 104),
            srgb(174, 136, 92),
        ],
        ZoneKind::Swamp => [
            srgb(78, 88, 56),
            srgb(88, 96, 58),
            srgb(96, 102, 62),
            srgb(104, 110, 70),
        ],
        ZoneKind::Lake => [
            srgb(150, 160, 104),
            srgb(118, 144, 78),
            srgb(100, 130, 68),
            srgb(92, 118, 64),
        ],
        ZoneKind::Coast => [
            srgb(216, 200, 152),
            srgb(190, 186, 120),
            srgb(140, 156, 86),
            srgb(112, 136, 72),
        ],
        ZoneKind::Badlands => [
            srgb(178, 122, 82),
            srgb(196, 142, 96),
            srgb(168, 108, 74),
            srgb(148, 92, 64),
        ],
    }
}

/// Minimap display color per zone.
pub fn map_color(kind: ZoneKind) -> Vec3 {
    ground_ramp(kind)[1]
}

/// (scatter density multiplier, tree fraction 0..1) per zone.
pub fn scatter_profile(kind: ZoneKind) -> (f32, f64) {
    match kind {
        ZoneKind::Mountains => (0.45, 0.55),
        ZoneKind::Forest => (2.4, 0.92),
        ZoneKind::Plains => (0.4, 0.7),
        ZoneKind::Desert => (0.18, 0.12),
        ZoneKind::Swamp => (0.8, 0.75),
        ZoneKind::Lake => (0.1, 0.6),
        ZoneKind::Coast => (0.35, 0.5),
        ZoneKind::Badlands => (0.22, 0.15),
    }
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn cell_hash(seed: u64, ix: i64, iz: i64) -> u64 {
    splitmix(seed ^ (ix as u64).wrapping_mul(0x8DA6_B343) ^ (iz as u64).wrapping_mul(0xD8163841))
}

fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub struct ZoneField {
    seed: u64,
    /// Zone Voronoi cell size in meters.
    cell: f32,
    /// Cumulative weights for the seeded per-cell kind pick.
    cum: Vec<(f32, ZoneKind)>,
    total: f32,
    pinned: Vec<(ZoneKind, [f32; 2], f32)>,
    warp: Noise2,
}

impl ZoneField {
    pub fn new(seed: u64, specs: &[ZoneSpec], cell: f32) -> Self {
        let mut specs = specs.to_vec();
        if specs.iter().all(|s| s.at.is_some()) {
            // random layout needs at least a background mix
            specs.extend([
                ZoneSpec { kind: ZoneKind::Forest, weight: 2.0, at: None, radius: 0.0 },
                ZoneSpec { kind: ZoneKind::Plains, weight: 2.0, at: None, radius: 0.0 },
                ZoneSpec { kind: ZoneKind::Mountains, weight: 1.2, at: None, radius: 0.0 },
                ZoneSpec { kind: ZoneKind::Lake, weight: 0.6, at: None, radius: 0.0 },
            ]);
        }
        let mut cum = Vec::new();
        let mut total = 0.0;
        let mut pinned = Vec::new();
        for s in &specs {
            match s.at {
                Some(at) => pinned.push((s.kind, at, s.radius.max(50.0))),
                None => {
                    total += s.weight.max(0.0);
                    cum.push((total, s.kind));
                }
            }
        }
        Self { seed, cell: cell.clamp(200.0, 4000.0), cum, total, pinned, warp: Noise2::new(seed ^ 0x20E5) }
    }

    fn cell_kind(&self, h: u64) -> ZoneKind {
        let t = unit(splitmix(h ^ 0xC0FFEE)) * self.total;
        for (c, k) in &self.cum {
            if t <= *c {
                return *k;
            }
        }
        self.cum.last().map(|(_, k)| *k).unwrap_or(ZoneKind::Plains)
    }

    /// Smooth normalized weight per zone kind at a world position.
    pub fn weights(&self, wx: f32, wz: f32) -> [f32; NK] {
        let cell = self.cell;
        // domain warp for organic borders
        let qx = self.warp.fbm(wx / (cell * 0.7) + 11.3, wz / (cell * 0.7) + 3.1, 3, 2.0, 0.5);
        let qz = self.warp.fbm(wx / (cell * 0.7) + 90.2, wz / (cell * 0.7) + 47.8, 3, 2.0, 0.5);
        let x = wx + qx * cell * 0.35;
        let z = wz + qz * cell * 0.35;
        let (cx, cz) = ((x / cell).floor() as i64, (z / cell).floor() as i64);
        let sigma2 = (cell * 0.26) * (cell * 0.26) * 2.0;
        let mut w = [0.0f32; NK];
        for dz in -2i64..=2 {
            for dx in -2i64..=2 {
                let (ix, iz) = (cx + dx, cz + dz);
                let h = cell_hash(self.seed, ix, iz);
                let px = (ix as f32 + 0.15 + 0.7 * unit(h)) * cell;
                let pz = (iz as f32 + 0.15 + 0.7 * unit(splitmix(h))) * cell;
                let d2 = (x - px) * (x - px) + (z - pz) * (z - pz);
                w[self.cell_kind(h).index()] += (-d2 / sigma2).exp();
            }
        }
        let sum: f32 = w.iter().sum();
        if sum > 1e-12 {
            for v in &mut w {
                *v /= sum;
            }
        } else {
            w[ZoneKind::Plains.index()] = 1.0;
        }
        // pinned zones: feathered override
        for (kind, at, radius) in &self.pinned {
            let d = ((wx - at[0]).powi(2) + (wz - at[1]).powi(2)).sqrt();
            let feather = (radius * 0.35).max(80.0);
            let t = (1.0 - (d - radius) / feather).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t); // smoothstep: C1
            if t > 0.0 {
                for v in &mut w {
                    *v *= 1.0 - t;
                }
                w[kind.index()] += t;
            }
        }
        w
    }

    /// Dominant zone at a position (for scatter picks, POI suitability, maps).
    pub fn dominant(&self, wx: f32, wz: f32) -> ZoneKind {
        let w = self.weights(wx, wz);
        let mut best = 0;
        for i in 1..NK {
            if w[i] > w[best] {
                best = i;
            }
        }
        KINDS[best]
    }

    /// Zone cells whose feature point falls inside the given world bounds
    /// (for the manifest summary).
    pub fn cells_in(&self, min: [f32; 2], max: [f32; 2]) -> Vec<(ZoneKind, [f32; 2])> {
        let cell = self.cell;
        let (x0, z0) = ((min[0] / cell).floor() as i64, (min[1] / cell).floor() as i64);
        let (x1, z1) = ((max[0] / cell).ceil() as i64, (max[1] / cell).ceil() as i64);
        let mut out = Vec::new();
        for (kind, at, _) in &self.pinned {
            if at[0] >= min[0] && at[0] <= max[0] && at[1] >= min[1] && at[1] <= max[1] {
                out.push((*kind, *at));
            }
        }
        for iz in z0..=z1 {
            for ix in x0..=x1 {
                let h = cell_hash(self.seed, ix, iz);
                let px = (ix as f32 + 0.15 + 0.7 * unit(h)) * cell;
                let pz = (iz as f32 + 0.15 + 0.7 * unit(splitmix(h))) * cell;
                if px >= min[0] && px <= max[0] && pz >= min[1] && pz <= max[1] {
                    out.push((self.cell_kind(h), [px, pz]));
                }
            }
        }
        out
    }
}
