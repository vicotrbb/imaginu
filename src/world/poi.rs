//! POI placement + settlement builders. A deterministic solver scores a
//! coarse world grid for suitability (slope, zone, altitude, prominence,
//! water) per POI kind, then greedily picks sites with separation. Sites
//! flatten the terrain through the world-space height function, so chunk
//! borders through a city stay seamless. Each POI exports as its own GLB,
//! referenced from the manifest with a world transform.

use glam::{Mat4, Quat, Vec3};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, cuboid, icosphere, lathe, to_flat_shaded};
use crate::palette::Palette;

use super::model::WorldModel;
use super::zones::ZoneKind;
use crate::generators::{Rand, range, rng};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PoiKind {
    City,
    Village,
    Castle,
    Watchtower,
    Dungeon,
}

impl PoiKind {
    pub fn name(self) -> &'static str {
        match self {
            PoiKind::City => "city",
            PoiKind::Village => "village",
            PoiKind::Castle => "castle",
            PoiKind::Watchtower => "watchtower",
            PoiKind::Dungeon => "dungeon",
        }
    }
    /// Footprint radius (flattened ground).
    pub fn radius(self) -> f32 {
        match self {
            PoiKind::City => 105.0,
            PoiKind::Village => 52.0,
            PoiKind::Castle => 55.0,
            PoiKind::Watchtower => 14.0,
            PoiKind::Dungeon => 16.0,
        }
    }
    /// Minimum distance to the next POI of the same kind.
    fn separation(self) -> f32 {
        match self {
            PoiKind::City => 1100.0,
            PoiKind::Village => 420.0,
            PoiKind::Castle => 900.0,
            PoiKind::Watchtower => 480.0,
            PoiKind::Dungeon => 380.0,
        }
    }
}

/// Recipe surface: `"pois":[{"kind":"city","count":2},
/// {"kind":"castle","at":[500,-800]}]`. Omit the field for area-scaled
/// defaults; pass `[]` for none.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PoiSpec {
    pub kind: PoiKind,
    #[serde(default)]
    pub count: Option<u32>,
    /// Pin at a world position (count ignored for this entry).
    #[serde(default)]
    pub at: Option<[f32; 2]>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PoiSite {
    pub kind: PoiKind,
    pub name: String,
    pub x: f32,
    pub z: f32,
    /// Flattened ground elevation at the site.
    pub ground: f32,
    pub radius: f32,
    pub seed: u64,
}

fn area_defaults(size_km2: f32) -> Vec<(PoiKind, u32)> {
    vec![
        (PoiKind::City, (size_km2 / 9.0).round() as u32),
        (PoiKind::Village, (size_km2 / 3.0).round() as u32),
        (PoiKind::Castle, (size_km2 / 16.0).round() as u32),
        (PoiKind::Watchtower, (size_km2 / 6.0).round() as u32),
        (PoiKind::Dungeon, (size_km2 / 8.0).round() as u32),
    ]
}

const NAME_A: [&str; 16] = [
    "Ald", "Thorn", "Ever", "Grim", "Wolf", "Ash", "Bright", "Stone", "Mist", "Raven", "Elder",
    "Iron", "Green", "Frost", "Gold", "Shadow",
];
const NAME_B: [&str; 12] = [
    "dale", "mere", "holt", "burg", "haven", "ford", "wick", "stead", "gate", "fell", "brook",
    "moor",
];

fn make_name(kind: PoiKind, r: &mut Rand) -> String {
    let base = format!(
        "{}{}",
        NAME_A[r.gen_range(0..NAME_A.len())],
        NAME_B[r.gen_range(0..NAME_B.len())]
    );
    match kind {
        PoiKind::Castle => format!("Castle {base}"),
        PoiKind::Watchtower => format!("{base} Watch"),
        PoiKind::Dungeon => format!("Barrow of {base}"),
        _ => base,
    }
}

/// Deterministic placement. `model` must have its zones/heights ready but
/// `pois` still empty (heights sampled here are the pre-flattening base).
pub fn place(model: &WorldModel, specs: Option<&[PoiSpec]>) -> Vec<PoiSite> {
    let size = model.size_x;
    let sea = model.p.sea_level;
    let km2 = (size / 1000.0) * (size / 1000.0);
    // resolve wanted counts + pins
    let mut wanted: Vec<(PoiKind, u32)> = Vec::new();
    let mut pins: Vec<(PoiKind, [f32; 2], Option<String>)> = Vec::new();
    match specs {
        None => wanted = area_defaults(km2),
        Some(list) => {
            for s in list {
                match s.at {
                    Some(at) => pins.push((s.kind, at, s.name.clone())),
                    None => {
                        let d = area_defaults(km2)
                            .iter()
                            .find(|(k, _)| *k == s.kind)
                            .map(|(_, c)| *c)
                            .unwrap_or(1);
                        wanted.push((s.kind, s.count.unwrap_or(d)));
                    }
                }
            }
        }
    }
    let mut r = rng(model.p.seed ^ 0x9017);
    let mut sites: Vec<PoiSite> = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn add(
        sites: &mut Vec<PoiSite>,
        model: &WorldModel,
        sea: f32,
        kind: PoiKind,
        x: f32,
        z: f32,
        name: Option<String>,
        r: &mut Rand,
    ) {
        // castles get a motte: plateau raised above the surrounding ground
        let motte = if kind == PoiKind::Castle { 2.5 } else { 0.0 };
        let ground = model.height(x, z).max(sea + 1.2) + motte;
        sites.push(PoiSite {
            kind,
            name: name.unwrap_or_else(|| make_name(kind, r)),
            x,
            z,
            ground,
            radius: kind.radius(),
            seed: model.p.seed ^ splitmix(sites.len() as u64 * 977 + 13),
        });
    }
    for (kind, at, name) in &pins {
        add(
            &mut sites,
            model,
            sea,
            *kind,
            at[0],
            at[1],
            name.clone(),
            &mut r,
        );
    }

    // candidate grid: coarse, deterministic order
    let step = (size / 96.0).clamp(24.0, 96.0);
    let n = (size / step) as i32;
    let margin = size * 0.5 - step * 1.5;
    struct Cand {
        x: f32,
        z: f32,
        h: f32,
        slope: f32,
        prom: f32,
        water: f32,
        zw: [f32; super::zones::NK],
    }
    let mut cands: Vec<Cand> = Vec::new();
    for jz in 0..n {
        for jx in 0..n {
            let x = (jx as f32 + 0.5) * step - size * 0.5;
            let z = (jz as f32 + 0.5) * step - size * 0.5;
            if x.abs() > margin || z.abs() > margin {
                continue;
            }
            let h = model.height(x, z);
            if h < sea + 1.0 {
                continue;
            }
            let e = 8.0;
            let slope = ((model.height(x + e, z) - model.height(x - e, z)).abs()
                + (model.height(x, z + e) - model.height(x, z - e)).abs())
                / (4.0 * e);
            // prominence + water proximity from a ring probe
            let mut ring = 0.0;
            let mut water = 0.0f32;
            for i in 0..8 {
                let a = i as f32 / 8.0 * core::f32::consts::TAU;
                let hh = model.height(x + a.cos() * 170.0, z + a.sin() * 170.0);
                ring += hh;
                if hh < sea {
                    water = 1.0;
                }
            }
            let prom = h - ring / 8.0;
            cands.push(Cand {
                x,
                z,
                h,
                slope,
                prom,
                water,
                zw: model.zones.weights(x, z),
            });
        }
    }

    let zi = |c: &Cand, k: ZoneKind| c.zw[k.index()];
    let score = |c: &Cand, kind: PoiKind| -> f32 {
        let flat = (1.0 - c.slope / 0.16).clamp(0.0, 1.0);
        let low = 1.0 / (1.0 + ((c.h - sea) / 45.0).max(0.0));
        match kind {
            PoiKind::City => {
                flat * low
                    * (zi(c, ZoneKind::Plains)
                        + zi(c, ZoneKind::Coast) * 0.9
                        + zi(c, ZoneKind::Forest) * 0.5
                        + zi(c, ZoneKind::Lake) * 0.6)
                    * (0.6 + 0.4 * c.water)
            }
            PoiKind::Village => {
                flat * low
                    * (1.0
                        - zi(c, ZoneKind::Mountains) * 0.85
                        - zi(c, ZoneKind::Swamp) * 0.4
                        - zi(c, ZoneKind::Badlands) * 0.5)
                        .max(0.0)
            }
            PoiKind::Castle => {
                (c.prom / 14.0).clamp(0.0, 1.2)
                    * (1.0 - c.slope / 0.55).clamp(0.0, 1.0)
                    * (1.0 - zi(c, ZoneKind::Swamp))
            }
            PoiKind::Watchtower => {
                (c.prom / 10.0).clamp(0.0, 1.4) * (1.0 - c.slope / 0.8).clamp(0.0, 1.0)
            }
            PoiKind::Dungeon => {
                ((c.slope - 0.35) / 0.5).clamp(0.0, 1.0)
                    * (0.25 + zi(c, ZoneKind::Mountains) + zi(c, ZoneKind::Badlands) * 0.9)
            }
        }
    };

    for (kind, count) in wanted {
        // rank candidates for this kind (stable: score desc, grid order)
        let mut ranked: Vec<(f32, usize)> = cands
            .iter()
            .enumerate()
            .map(|(i, c)| (score(c, kind), i))
            .filter(|(s, _)| *s > 0.05)
            .collect();
        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then(a.1.cmp(&b.1)));
        let mut placed = 0;
        for (_, i) in ranked {
            if placed >= count {
                break;
            }
            let c = &cands[i];
            // never straddle a river: disc query over the whole footprint
            if model.network.river_within(c.x, c.z, kind.radius() * 1.15) {
                continue;
            }
            let ok = sites.iter().all(|s| {
                let d = ((s.x - c.x).powi(2) + (s.z - c.z).powi(2)).sqrt();
                let min_same = if s.kind == kind {
                    kind.separation()
                } else {
                    0.0
                };
                let min_any = (s.radius + kind.radius()) * 1.5 + 40.0;
                d >= min_same.max(min_any)
            });
            if !ok {
                continue;
            }
            add(&mut sites, model, sea, kind, c.x, c.z, None, &mut r);
            placed += 1;
        }
    }
    sites
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

pub fn poi_file(site: &PoiSite, index: usize) -> String {
    format!("poi_{}_{index}.glb", site.kind.name())
}

/// Spawn points in world coordinates (gate/approach positions).
pub fn spawn_points(site: &PoiSite) -> Vec<[f32; 3]> {
    let r = site.radius;
    match site.kind {
        PoiKind::City | PoiKind::Village | PoiKind::Castle => {
            vec![
                [site.x, site.ground, site.z + r * 1.15],
                [site.x, site.ground, site.z],
            ]
        }
        _ => vec![[site.x, site.ground, site.z]],
    }
}

// ---------------------------------------------------------------------
// Asset builders. All geometry is local: ground plane at y = 0, POI is
// placed at manifest position [x, ground, z].
// ---------------------------------------------------------------------

pub fn build_asset(site: &PoiSite, pal: &Palette) -> Asset {
    match site.kind {
        PoiKind::City => city(site, pal),
        PoiKind::Village => village(site, pal),
        PoiKind::Castle => castle(site, pal),
        PoiKind::Watchtower => watchtower(site, pal),
        PoiKind::Dungeon => dungeon(site, pal),
    }
}

fn stone_of(pal: &Palette) -> (Vec3, Vec3) {
    (pal.rock[0], pal.rock[1])
}

/// Crenellated wall segment from `a` to `b` (XZ, y up), with parapet merlons.
fn wall_segment(m: &mut Mesh, a: Vec3, b: Vec3, h: f32, th: f32, stone: Vec3) {
    let dir = (b - a).normalize_or(Vec3::X);
    let len = (b - a).length();
    let mid = (a + b) / 2.0;
    let yaw = (-dir.z).atan2(dir.x);
    let mut w = cuboid(Vec3::ZERO, Vec3::new(len / 2.0, h / 2.0, th / 2.0), stone);
    // merlons
    let n_mer = (len / (th * 2.2)).max(2.0) as i32;
    for i in 0..n_mer {
        let t = (i as f32 + 0.5) / n_mer as f32;
        if i % 2 == 0 {
            w.merge(&cuboid(
                Vec3::new((t - 0.5) * len, h / 2.0 + th * 0.35, 0.0),
                Vec3::new(len / n_mer as f32 * 0.32, th * 0.35, th * 0.55),
                stone * 0.92,
            ));
        }
    }
    w.transform(Mat4::from_rotation_translation(
        Quat::from_rotation_y(yaw),
        mid + Vec3::new(0.0, h / 2.0, 0.0),
    ));
    m.merge(&w);
}

fn round_tower(m: &mut Mesh, at: Vec3, radius: f32, h: f32, stone: Vec3, roof: Option<Vec3>) {
    let mut t = lathe(
        &[
            (radius * 1.15, 0.0),
            (radius * 1.05, h * 0.12),
            (radius, h * 0.75),
            (radius * 1.12, h * 0.82),
            (radius * 1.12, h),
            (radius * 0.9, h),
            (radius * 0.9, h * 0.9),
        ],
        10,
        |_, v| stone * (0.85 + v * 0.3),
    );
    t = to_flat_shaded(&t);
    if let Some(rc) = roof {
        let mut cone = lathe(&[(radius * 1.2, h), (0.0, h + radius * 2.0)], 10, |_, _| rc);
        cone = to_flat_shaded(&cone);
        t.merge(&cone);
    }
    t.translate(at);
    m.merge(&t);
}

/// One cottage (reuses the building generator), rotated/placed.
fn cottage(m: &mut Mesh, seed: u64, width: f32, floors: u32, at: Vec3, yaw: f32, pal: &Palette) {
    let a = crate::generators::building::generate(
        &crate::recipe::BuildingParams {
            seed,
            width,
            floors,
        },
        pal,
    );
    for part in &a.parts {
        let mut mesh = part.mesh.clone();
        mesh.transform(Mat4::from_rotation_translation(
            Quat::from_rotation_y(yaw),
            at,
        ));
        m.merge(&mesh);
    }
}

fn city(site: &PoiSite, pal: &Palette) -> Asset {
    let mut r = rng(site.seed);
    let (stone, stone_d) = stone_of(pal);
    let rad = site.radius * 0.88;
    let mut m = Mesh::new();
    // ring wall: 12-gon with towers on every other vertex, gate on +Z
    let nseg = 12;
    let vert = |i: i32| {
        let a =
            (i as f32 + 0.5) / nseg as f32 * core::f32::consts::TAU + core::f32::consts::FRAC_PI_2;
        Vec3::new(a.cos() * rad, 0.0, a.sin() * rad)
    };
    let wall_h = 7.0;
    for i in 0..nseg {
        let (a, b) = (vert(i), vert(i + 1));
        if i == 0 {
            // gate segment: two flanking towers + lintel over the opening
            let gap = (b - a) * 0.30;
            wall_segment(&mut m, a, a + gap, wall_h, 2.0, stone);
            wall_segment(&mut m, b - gap, b, wall_h, 2.0, stone);
            round_tower(
                &mut m,
                a + gap,
                2.6,
                wall_h * 1.45,
                stone_d,
                Some(pal.accent * 0.7),
            );
            round_tower(
                &mut m,
                b - gap,
                2.6,
                wall_h * 1.45,
                stone_d,
                Some(pal.accent * 0.7),
            );
            let mid = (a + b) / 2.0 + Vec3::new(0.0, wall_h * 0.85, 0.0);
            m.merge(&cuboid(
                mid,
                Vec3::new((b - a).length() * 0.2, wall_h * 0.15, 1.4),
                stone,
            ));
        } else {
            wall_segment(&mut m, a, b, wall_h, 2.0, stone);
        }
        if i % 2 == 1 {
            round_tower(&mut m, vert(i), 2.2, wall_h * 1.3, stone_d, None);
        }
    }
    // buildings: three dense rings around a plaza, aligned to face the
    // center, with the south gate road kept clear
    for (ring_r, count, wmin, wmax) in [
        (rad * 0.78, 16, 6.0, 8.5),
        (rad * 0.56, 12, 6.5, 9.5),
        (rad * 0.35, 7, 7.0, 11.0),
    ] {
        for i in 0..count {
            let a = i as f32 / count as f32 * core::f32::consts::TAU + ring_r;
            let jitter = range(&mut r, -0.05, 0.05);
            let p = Vec3::new(
                (a + jitter).cos() * ring_r,
                0.0,
                (a + jitter).sin() * ring_r,
            );
            // keep the gate road (south) clear
            if p.z > rad * 0.28 && p.x.abs() < rad * 0.22 {
                continue;
            }
            let floors = 1 + (r.gen_range(0..10) as u32) / 4;
            cottage(
                &mut m,
                site.seed ^ (i as u64 * 131 + ring_r as u64),
                range(&mut r, wmin, wmax),
                floors.min(3),
                p,
                a + core::f32::consts::FRAC_PI_2,
                pal,
            );
        }
    }
    // plaza: cobble disc + well
    let mut plaza = lathe(
        &[(rad * 0.22, 0.0), (rad * 0.22, 0.25), (0.0, 0.25)],
        20,
        |_, _| stone * 1.1,
    );
    plaza = to_flat_shaded(&plaza);
    m.merge(&plaza);
    let mut well = lathe(
        &[(1.4, 0.25), (1.4, 1.2), (1.0, 1.2), (1.0, 0.25)],
        8,
        |_, _| stone_d,
    );
    well = to_flat_shaded(&well);
    m.merge(&well);
    finish(
        site,
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.9,
                ..Default::default()
            },
        }],
    )
}

fn village(site: &PoiSite, pal: &Palette) -> Asset {
    let mut r = rng(site.seed);
    let (stone, stone_d) = stone_of(pal);
    let rad = site.radius * 0.55;
    let mut m = Mesh::new();
    let count = 6 + (r.gen_range(0..3) as i32);
    for i in 0..count {
        let a = i as f32 / count as f32 * core::f32::consts::TAU + range(&mut r, -0.1, 0.1);
        let d = rad * range(&mut r, 0.62, 0.95);
        let p = Vec3::new(a.cos() * d, 0.0, a.sin() * d);
        cottage(
            &mut m,
            site.seed ^ (i as u64 * 953),
            range(&mut r, 3.8, 5.6),
            1,
            p,
            a + core::f32::consts::FRAC_PI_2,
            pal,
        );
    }
    // village well + a few fence posts
    let mut well = lathe(
        &[(1.2, 0.0), (1.2, 1.0), (0.85, 1.0), (0.85, 0.0)],
        8,
        |_, _| stone_d,
    );
    well = to_flat_shaded(&well);
    m.merge(&well);
    for i in 0..10 {
        let a = i as f32 / 10.0 * core::f32::consts::TAU;
        m.merge(&cuboid(
            Vec3::new(a.cos() * rad * 1.05, 0.5, a.sin() * rad * 1.05),
            Vec3::new(0.09, 0.5, 0.09),
            pal.trunk,
        ));
    }
    let _ = stone;
    finish(
        site,
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.92,
                ..Default::default()
            },
        }],
    )
}

fn castle(site: &PoiSite, pal: &Palette) -> Asset {
    let (stone, stone_d) = stone_of(pal);
    let rad = site.radius * 0.72;
    let mut m = Mesh::new();
    // curtain walls: square with corner towers
    let wall_h = 9.0;
    let c = |sx: f32, sz: f32| Vec3::new(sx * rad, 0.0, sz * rad);
    for (a, b) in [
        (c(-1.0, -1.0), c(1.0, -1.0)),
        (c(1.0, -1.0), c(1.0, 1.0)),
        (c(-1.0, 1.0), c(-1.0, -1.0)),
    ] {
        wall_segment(&mut m, a, b, wall_h, 2.4, stone);
    }
    // gate wall (+Z): two segments + arch carved by CSG
    let (ga, gb) = (c(-1.0, 1.0), c(1.0, 1.0));
    let gap = (gb - ga) * 0.36;
    wall_segment(&mut m, ga, ga + gap, wall_h, 2.4, stone);
    wall_segment(&mut m, gb - gap, gb, wall_h, 2.4, stone);
    let mut gate = cuboid(
        Vec3::new(0.0, wall_h * 0.55, rad),
        Vec3::new((gb - ga).length() * 0.15, wall_h * 0.55, 1.9),
        stone,
    );
    // carve the archway (capped cylinder: profile touches the axis, so the
    // CSG cutter is a closed solid)
    let mut bore = lathe(
        &[(0.0, -3.0), (2.6, -3.0), (2.6, 3.0), (0.0, 3.0)],
        12,
        |_, _| stone,
    );
    bore.transform(Mat4::from_rotation_translation(
        Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
        Vec3::new(0.0, 3.2, rad),
    ));
    let mut opening = cuboid(Vec3::new(0.0, 1.6, rad), Vec3::new(2.6, 1.6, 3.0), stone);
    opening.merge(&bore);
    gate = crate::csg::subtract(&gate, &opening);
    m.merge(&gate);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        round_tower(
            &mut m,
            c(sx, sz),
            3.4,
            wall_h * 1.5,
            stone_d,
            Some(pal.accent * 0.65),
        );
    }
    // keep: massive block with arrow slits + banner
    let keep_h = wall_h * 2.1;
    let mut keep = cuboid(
        Vec3::new(0.0, keep_h / 2.0, -rad * 0.25),
        Vec3::new(rad * 0.38, keep_h / 2.0, rad * 0.32),
        stone * 1.05,
    );
    for i in -1..=1 {
        let slit = cuboid(
            Vec3::new(
                i as f32 * rad * 0.16,
                keep_h * 0.68,
                -rad * 0.25 + rad * 0.32,
            ),
            Vec3::new(0.22, 1.1, 0.6),
            stone,
        );
        keep = crate::csg::subtract(&keep, &slit);
    }
    // merlons on the keep top
    let kw = rad * 0.38;
    for i in -2..=2 {
        keep.merge(&cuboid(
            Vec3::new(i as f32 * kw * 0.45, keep_h + 0.5, -rad * 0.25 + rad * 0.30),
            Vec3::new(kw * 0.14, 0.5, 0.35),
            stone * 0.9,
        ));
        keep.merge(&cuboid(
            Vec3::new(i as f32 * kw * 0.45, keep_h + 0.5, -rad * 0.25 - rad * 0.30),
            Vec3::new(kw * 0.14, 0.5, 0.35),
            stone * 0.9,
        ));
    }
    m.merge(&keep);
    // banner pole + pennant
    let pole = Vec3::new(0.0, keep_h + 4.0, -rad * 0.25);
    m.merge(&cuboid(
        pole - Vec3::Y * 2.0,
        Vec3::new(0.12, 2.0, 0.12),
        pal.trunk * 0.7,
    ));
    let mut pennant = Mesh::new();
    pennant.add_flat_tri(
        pole + Vec3::new(0.12, 1.8, 0.0),
        pole + Vec3::new(0.12, 0.6, 0.0),
        pole + Vec3::new(2.4, 1.2, 0.0),
        pal.accent,
    );
    m.merge(&pennant);
    finish(
        site,
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.88,
                double_sided: true,
                ..Default::default()
            },
        }],
    )
}

fn watchtower(site: &PoiSite, pal: &Palette) -> Asset {
    let (stone, stone_d) = stone_of(pal);
    let mut m = Mesh::new();
    round_tower(&mut m, Vec3::ZERO, 3.0, 13.0, stone, None);
    // wooden lookout platform
    m.merge(&cuboid(
        Vec3::new(0.0, 13.3, 0.0),
        Vec3::new(3.9, 0.25, 3.9),
        pal.trunk,
    ));
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        m.merge(&cuboid(
            Vec3::new(sx * 3.6, 14.4, sz * 3.6),
            Vec3::new(0.14, 0.9, 0.14),
            pal.trunk * 0.8,
        ));
    }
    let mut roof = lathe(&[(4.6, 15.3), (0.0, 18.2)], 8, |_, _| pal.accent * 0.6);
    roof = to_flat_shaded(&roof);
    m.merge(&roof);
    let _ = stone_d;
    // brazier glow
    let fire = icosphere(0.45, 1, Vec3::new(1.0, 0.55, 0.2));
    let mut fire_m = fire;
    fire_m.translate(Vec3::new(0.0, 14.1, 0.0));
    finish(
        site,
        vec![
            Part {
                mesh: m,
                material: Material {
                    roughness: 0.9,
                    ..Default::default()
                },
            },
            Part {
                mesh: fire_m,
                material: Material {
                    roughness: 0.5,
                    emissive: Vec3::new(1.0, 0.45, 0.12) * 1.6,
                    ..Default::default()
                },
            },
        ],
    )
}

fn dungeon(site: &PoiSite, pal: &Palette) -> Asset {
    let mut r = rng(site.seed);
    let (_, stone_d) = stone_of(pal);
    // cliff-mouth portal composed explicitly (no CSG): a boulder massif
    // arranged around a built stone doorframe with a dark vestibule
    let mut m = Mesh::new();
    for (dx, dz, s) in [
        (-8.0f32, -3.5f32, 4.2f32),
        (8.0, -3.5, 4.2),
        (0.0, -8.0, 5.5),
    ] {
        let mut rock = crate::generators::rock::rock_mesh(&mut r, pal, s, 0.7);
        rock.translate(Vec3::new(dx, 0.0, dz));
        m.merge(&rock);
    }
    // vestibule: stone side walls + ceiling, black back wall reads as depth
    let dark = Vec3::splat(0.012);
    m.merge(&cuboid(
        Vec3::new(0.0, 2.6, -0.4),
        Vec3::new(2.4, 2.6, 0.3),
        dark,
    ));
    for s in [-1.0f32, 1.0] {
        m.merge(&cuboid(
            Vec3::new(s * 2.4, 2.6, 1.6),
            Vec3::new(0.6, 2.6, 2.4),
            stone_d * 0.55,
        ));
    }
    m.merge(&cuboid(
        Vec3::new(0.0, 5.0, 1.6),
        Vec3::new(3.0, 0.6, 2.4),
        stone_d * 0.55,
    ));
    // doorframe: posts + lintel + capstone
    for s in [-1.0f32, 1.0] {
        m.merge(&cuboid(
            Vec3::new(s * 2.7, 2.5, 3.9),
            Vec3::new(0.65, 2.5, 0.75),
            stone_d,
        ));
    }
    m.merge(&cuboid(
        Vec3::new(0.0, 5.5, 3.9),
        Vec3::new(3.6, 0.6, 0.95),
        stone_d,
    ));
    m.merge(&cuboid(
        Vec3::new(0.0, 6.3, 3.9),
        Vec3::new(1.4, 0.45, 0.8),
        stone_d * 1.15,
    ));
    // worn steps down to the mouth
    for i in 0..3 {
        m.merge(&cuboid(
            Vec3::new(0.0, 0.3 - i as f32 * 0.12, 4.8 + i as f32 * 0.8),
            Vec3::new(2.3 - i as f32 * 0.15, 0.14, 0.5),
            stone_d * 0.9,
        ));
    }
    // emissive marker shards flanking the mouth
    let mut shards = Mesh::new();
    for s in [-1.0f32, 1.0] {
        let mut sh = crate::mesh::lathe(
            &[(0.0, 0.0), (0.45, 0.6), (0.28, 2.0), (0.0, 2.9)],
            5,
            |_, _| pal.accent,
        );
        sh = to_flat_shaded(&sh);
        sh.translate(Vec3::new(s * 4.0, 0.0, 5.2));
        shards.merge(&sh);
    }
    finish(
        site,
        vec![
            Part {
                mesh: m,
                material: Material {
                    roughness: 0.95,
                    double_sided: true,
                    ..Default::default()
                },
            },
            Part {
                mesh: shards,
                material: Material {
                    roughness: 0.3,
                    emissive: pal.accent * 1.8,
                    ..Default::default()
                },
            },
        ],
    )
}

/// Stone arch bridge spanning a river; yaw baked into the mesh so the game
/// only needs the manifest position. Deck at y=0.
pub fn bridge_asset(b: &super::network::Bridge, pal: &Palette) -> Asset {
    let (stone, stone_d) = stone_of(pal);
    let mut m = Mesh::new();
    let half = b.len * 0.5;
    let width = 4.2;
    let segs = 7;
    for i in 0..segs {
        let t0 = i as f32 / segs as f32;
        let t1 = (i + 1) as f32 / segs as f32;
        let x0 = (t0 - 0.5) * b.len;
        let x1 = (t1 - 0.5) * b.len;
        let arc = |t: f32| ((t - 0.5) * core::f32::consts::PI).cos() * b.len * 0.07;
        let (y0, y1) = (arc(t0), arc(t1));
        // deck slab
        let mut slab = Mesh::new();
        slab.add_flat_quad(
            Vec3::new(x0, y0, -width / 2.0),
            Vec3::new(x1, y1, -width / 2.0),
            Vec3::new(x1, y1, width / 2.0),
            Vec3::new(x0, y0, width / 2.0),
            stone * 1.05,
        );
        m.merge(&slab);
        // under-structure
        m.merge(&cuboid(
            Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0 - 0.5, 0.0),
            Vec3::new((x1 - x0) / 2.0, 0.5, width / 2.0 * 0.92),
            stone_d,
        ));
        // parapets
        for s in [-1.0f32, 1.0] {
            m.merge(&cuboid(
                Vec3::new(
                    (x0 + x1) / 2.0,
                    (y0 + y1) / 2.0 + 0.55,
                    s * (width / 2.0 - 0.25),
                ),
                Vec3::new((x1 - x0) / 2.0, 0.55, 0.25),
                stone * 0.9,
            ));
        }
    }
    // piers at 1/3 spans, sunk toward the riverbed
    for s in [-0.28f32, 0.28] {
        m.merge(&cuboid(
            Vec3::new(s * b.len, -2.6, 0.0),
            Vec3::new(1.0, 2.4, width / 2.0 * 0.8),
            stone_d,
        ));
    }
    // abutments
    for s in [-1.0f32, 1.0] {
        m.merge(&cuboid(
            Vec3::new(s * (half + 1.0), -0.9, 0.0),
            Vec3::new(1.4, 1.1, width / 2.0 + 0.6),
            stone_d * 1.05,
        ));
    }
    m.transform(Mat4::from_rotation_y(b.yaw));
    Asset::static_mesh(
        "bridge",
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.85,
                double_sided: true,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::TriMesh,
            mass: 0.0,
            friction: 0.9,
            restitution: 0.05,
        }),
    )
}

fn finish(site: &PoiSite, parts: Vec<Part>) -> Asset {
    let mut a = Asset::static_mesh(
        &format!("poi_{}", site.kind.name()),
        parts,
        Some(Physics {
            collider: Collider::TriMesh,
            mass: 0.0,
            friction: 0.8,
            restitution: 0.05,
        }),
    );
    a.name = site.name.clone();
    a
}
