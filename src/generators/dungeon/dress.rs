//! Dressing props + emissive torch lighting cues. Placement is a deterministic
//! function of (room, seed); density scales the prop count. Props sit on the
//! floor, inset from the walls so they never poke through.

use glam::Vec3;

use crate::mesh::{Mesh, cuboid, icosphere, to_flat_shaded};
use crate::palette::{Palette, lerp, srgb};
use crate::recipe::DungeonTheme;

use super::super::{range, rng};
use super::model::{Room, RoomKind};

/// The dungeon-specific prop set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DProp {
    Pillar,
    TorchBracket,
    Door,
    Portcullis,
    Sarcophagus,
    Chest,
    Rubble,
}

/// A prop instance already positioned in world space. `emissive`, when set, is
/// the glow color the caller should render as an emissive material (torches).
pub struct PlacedProp {
    pub mesh: Mesh,
    pub emissive: Option<Vec3>,
}

const TORCH_GLOW: Vec3 = Vec3::new(1.0, 0.55, 0.18);

/// Build a prop mesh at the origin, base sitting on the floor (y = 0).
pub fn dungeon_prop(kind: DProp, pal: &Palette) -> Mesh {
    match kind {
        DProp::Pillar => pillar(pal),
        DProp::TorchBracket => torch_bracket(pal),
        DProp::Door => door(pal),
        DProp::Portcullis => portcullis(),
        DProp::Sarcophagus => sarcophagus(pal),
        DProp::Chest => chest(pal),
        DProp::Rubble => rubble(pal, 3),
    }
}

fn pillar(pal: &Palette) -> Mesh {
    let stone = pal.rock[0];
    let mut m = cuboid(Vec3::new(0.0, 1.7, 0.0), Vec3::new(0.35, 1.7, 0.35), stone);
    // base + capital
    m.merge(&cuboid(
        Vec3::new(0.0, 0.15, 0.0),
        Vec3::new(0.5, 0.15, 0.5),
        stone * 0.9,
    ));
    m.merge(&cuboid(
        Vec3::new(0.0, 3.35, 0.0),
        Vec3::new(0.5, 0.15, 0.5),
        stone * 0.9,
    ));
    m
}

fn torch_bracket(pal: &Palette) -> Mesh {
    let iron = srgb(48, 50, 56);
    // wall bracket + shaft
    let mut m = cuboid(Vec3::new(0.0, 2.2, 0.0), Vec3::new(0.06, 0.28, 0.06), iron);
    m.merge(&cuboid(
        Vec3::new(0.0, 2.5, 0.12),
        Vec3::new(0.05, 0.05, 0.14),
        iron,
    ));
    // flame blob (emissive)
    let glow = lerp(pal.accent, srgb(255, 200, 110), 0.5);
    let mut flame = icosphere(0.16, 1, glow);
    for v in flame.positions.iter_mut() {
        v.y *= 1.5;
    }
    flame.recompute_smooth_normals();
    let mut flame = to_flat_shaded(&flame);
    flame.translate(Vec3::new(0.0, 2.78, 0.12));
    m.merge(&flame);
    m
}

fn door(pal: &Palette) -> Mesh {
    let wood = lerp(pal.trunk, srgb(120, 84, 52), 0.4);
    let iron = srgb(48, 50, 56);
    let mut m = cuboid(Vec3::new(0.0, 1.2, 0.0), Vec3::new(0.8, 1.2, 0.08), wood);
    for y in [0.5f32, 1.9] {
        m.merge(&cuboid(
            Vec3::new(0.0, y, 0.1),
            Vec3::new(0.8, 0.06, 0.02),
            iron,
        ));
    }
    m
}

fn portcullis() -> Mesh {
    let iron = srgb(56, 58, 64);
    let mut m = Mesh::new();
    for i in 0..5 {
        let x = (i as f32 - 2.0) * 0.36;
        m.merge(&cuboid(
            Vec3::new(x, 1.4, 0.0),
            Vec3::new(0.05, 1.4, 0.05),
            iron,
        ));
    }
    for y in [0.2f32, 1.4, 2.6] {
        m.merge(&cuboid(
            Vec3::new(0.0, y, 0.0),
            Vec3::new(0.95, 0.05, 0.05),
            iron,
        ));
    }
    m
}

fn sarcophagus(pal: &Palette) -> Mesh {
    let stone = lerp(pal.rock[0], pal.terrain[2], 0.3);
    let mut m = cuboid(Vec3::new(0.0, 0.45, 0.0), Vec3::new(0.6, 0.45, 1.1), stone);
    m.merge(&cuboid(
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.65, 0.12, 1.15),
        stone * 1.05,
    ));
    m
}

fn chest(pal: &Palette) -> Mesh {
    let wood = lerp(pal.trunk, srgb(150, 110, 60), 0.4);
    let iron = srgb(60, 52, 40);
    let mut m = cuboid(Vec3::new(0.0, 0.28, 0.0), Vec3::new(0.5, 0.28, 0.35), wood);
    m.merge(&cuboid(
        Vec3::new(0.0, 0.58, 0.0),
        Vec3::new(0.52, 0.06, 0.37),
        iron,
    ));
    m
}

fn rubble(pal: &Palette, n: u32) -> Mesh {
    let mut m = Mesh::new();
    let mut r = rng(0x00DD_BA11_u64.wrapping_add(n as u64));
    for i in 0..n {
        let s = range(&mut r, 0.12, 0.28);
        let mut rock = icosphere(s, 1, pal.rock[(i % 2) as usize]);
        for v in rock.positions.iter_mut() {
            v.y *= 0.6;
        }
        rock.recompute_smooth_normals();
        let mut rock = to_flat_shaded(&rock);
        let a = i as f32 * 2.399;
        rock.translate(Vec3::new(a.cos() * 0.3, s * 0.5, a.sin() * 0.3));
        m.merge(&rock);
    }
    m
}

/// Deterministically dress a room. Count scales with `density`; torches are
/// emitted as emissive lighting cues.
pub fn dress_room(
    room: &Room,
    theme: DungeonTheme,
    density: f32,
    seed: u64,
    pal: &Palette,
) -> Vec<PlacedProp> {
    let density = density.clamp(0.0, 1.0);
    // local seed folds in the room footprint so each room is distinct yet
    // fully deterministic
    let s = seed
        ^ ((room.min.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((room.min.z as i64 as u64).wrapping_mul(0xD1B5_4A32_D192_ED03));
    let mut r = rng(s);

    let (mn, mx) = (room.min, room.max);
    let inset = 0.9f32;
    let (ix0, ix1) = (mn.x + inset, mx.x - inset);
    let (iz0, iz1) = (mn.z + inset, mx.z - inset);
    let w = (mx.x - mn.x).max(1.0);
    let d = (mx.z - mn.z).max(1.0);
    let perim = 2.0 * (w + d);
    let area = w * d;

    let mut out: Vec<PlacedProp> = Vec::new();
    let place = |mesh: Mesh, pos: Vec3, emissive: Option<Vec3>, out: &mut Vec<PlacedProp>| {
        let mut m = mesh;
        m.translate(pos);
        out.push(PlacedProp { mesh: m, emissive });
    };

    // ---- wall torches (emissive lighting cues) ----
    let n_torch = (density * perim / 7.0).round() as usize;
    for k in 0..n_torch {
        // walk the perimeter, alternating walls
        let t = (k as f32 + 0.5) / n_torch.max(1) as f32;
        let pos = perimeter_point(mn, mx, t, 0.45);
        place(
            dungeon_prop(DProp::TorchBracket, pal),
            Vec3::new(pos.x, 0.0, pos.z),
            Some(TORCH_GLOW),
            &mut out,
        );
    }

    // ---- pillars in larger rooms ----
    let n_pillar = if area > 60.0 {
        (density * area / 70.0).round() as usize
    } else {
        0
    };
    for _ in 0..n_pillar {
        let pos = Vec3::new(range(&mut r, ix0, ix1), 0.0, range(&mut r, iz0, iz1));
        place(dungeon_prop(DProp::Pillar, pal), pos, None, &mut out);
    }

    // ---- feature by room role ----
    let center = Vec3::new((mn.x + mx.x) * 0.5, 0.0, (mn.z + mx.z) * 0.5);
    match room.kind {
        RoomKind::Treasure => {
            place(dungeon_prop(DProp::Chest, pal), center, None, &mut out);
        }
        RoomKind::Boss => {
            let feat = if matches!(theme, DungeonTheme::Crypt) {
                DProp::Sarcophagus
            } else {
                DProp::Portcullis
            };
            place(dungeon_prop(feat, pal), center, None, &mut out);
        }
        RoomKind::Entrance => {
            // a barred gate marks the way in
            place(
                dungeon_prop(DProp::Door, pal),
                Vec3::new(center.x, 0.0, iz0),
                None,
                &mut out,
            );
        }
        _ => {}
    }

    // ---- scattered rubble ----
    let n_rubble = (density * area / 45.0).round() as usize;
    for _ in 0..n_rubble {
        let pos = Vec3::new(range(&mut r, ix0, ix1), 0.0, range(&mut r, iz0, iz1));
        place(dungeon_prop(DProp::Rubble, pal), pos, None, &mut out);
    }

    out
}

/// A point on the room's inner wall perimeter at parameter `t` in [0,1),
/// pulled `pull` meters in from the wall.
fn perimeter_point(mn: Vec3, mx: Vec3, t: f32, pull: f32) -> Vec3 {
    let w = mx.x - mn.x;
    let d = mx.z - mn.z;
    let per = 2.0 * (w + d);
    let mut s = (t.fract() * per).max(0.0);
    if s < w {
        return Vec3::new(mn.x + s, 0.0, mn.z + pull);
    }
    s -= w;
    if s < d {
        return Vec3::new(mx.x - pull, 0.0, mn.z + s);
    }
    s -= d;
    if s < w {
        return Vec3::new(mx.x - s, 0.0, mx.z - pull);
    }
    s -= w;
    Vec3::new(mn.x + pull, 0.0, mx.z - s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::by_name;

    fn fixed_room() -> Room {
        Room {
            id: 0,
            kind: RoomKind::Normal,
            min: Vec3::new(0.0, 0.0, 0.0),
            max: Vec3::new(12.0, 4.0, 12.0),
        }
    }

    #[test]
    fn dressing_is_deterministic_and_scales_with_density() {
        let room = fixed_room();
        let sparse = dress_room(&room, DungeonTheme::Crypt, 0.2, 1, &by_name("necrotic"));
        let dense = dress_room(&room, DungeonTheme::Crypt, 0.9, 1, &by_name("necrotic"));
        assert!(dense.len() >= sparse.len());
        let again = dress_room(&room, DungeonTheme::Crypt, 0.9, 1, &by_name("necrotic"));
        assert_eq!(dense.len(), again.len());
        // torches are emissive lighting cues
        assert!(dense.iter().any(|p| p.emissive.is_some()));
    }

    #[test]
    fn every_prop_builds() {
        let pal = by_name("necrotic");
        for k in [
            DProp::Pillar,
            DProp::TorchBracket,
            DProp::Door,
            DProp::Portcullis,
            DProp::Sarcophagus,
            DProp::Chest,
            DProp::Rubble,
        ] {
            let m = dungeon_prop(k, &pal);
            m.validate().unwrap();
            assert!(m.triangle_count() > 0);
        }
    }
}
