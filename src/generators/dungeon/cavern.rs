//! Organic SDF caves for the Cavern theme. Instead of boxy orthogonal rooms,
//! the cavern is meshed as NEGATIVE SPACE: each room becomes a blobby ellipsoid
//! cavity, each corridor a round-cone tunnel, all fused with a smooth-min union
//! into one connected void. The rock solid is that void's complement, meshed at
//! the void boundary with normals pointing INTO the air so you see cave walls
//! from inside. A flat floor is cut at y = 0 so props/spawns still sit on ground.
//!
//! Deterministic: fixed grid derived only from the layout bounds; the only
//! "randomness" is positional value-noise for color variation (pure function of
//! world position). No RNG, no seed dependence beyond the layout itself.

use glam::Vec3;

use crate::mesh::Mesh;
use crate::palette::{Palette, lerp};
use crate::sdf::{mesh_field, sd_ellipsoid, sd_round_cone, smin};

use super::model::{Corridor, Room};

/// Blend radius for the smooth-min union — generous so rooms/tunnels merge into
/// rounded, cave-like transitions rather than intersecting hard.
const BLEND_K: f32 = 3.0;
/// Tunnel radius (air) along a corridor path.
const TUNNEL_R: f32 = 2.1;
/// Height of a corridor tunnel's centerline above the floor.
const TUNNEL_Y: f32 = 1.7;
/// Cap on grid corner samples so a hostile / huge dungeon can't explode the
/// field evaluation. The cell is coarsened until the grid fits under this.
const SAMPLE_CAP: f64 = 4.0e6;

/// One organic cavity: an ellipsoid for a room footprint.
struct Blob {
    c: Vec3,
    r: Vec3,
}

/// A tunnel segment between two path points (already elevated to `TUNNEL_Y`).
struct Tube {
    a: Vec3,
    b: Vec3,
}

/// Deterministic positional value-noise in [0,1) for subtle color variation.
fn noise(p: Vec3) -> f32 {
    let s = (p.x * 12.9898 + p.y * 78.233 + p.z * 37.719).sin() * 43758.545;
    s - s.floor()
}

/// Build the whole cavern (or a subset of rooms/corridors) as one organic
/// rock-wall mesh. `include_ceiling` false caps the grid low so a top-down
/// overview sees the floor and lower walls through an open top.
pub fn cavern_mesh(
    rooms: &[Room],
    corridors: &[Corridor],
    pal: &Palette,
    detail: f32,
    include_ceiling: bool,
) -> Mesh {
    if rooms.is_empty() {
        return Mesh::new();
    }
    let ceil = rooms[0].max.y.max(3.0);

    // ---- gather cavity primitives ----
    let blobs: Vec<Blob> = rooms
        .iter()
        .map(|rm| {
            let c = rm.center();
            let hx = (rm.max.x - rm.min.x) * 0.5;
            let hz = (rm.max.z - rm.min.z) * 0.5;
            Blob {
                // centered on the floor; the y>0 floor cut keeps the lower half
                // out, leaving a domed chamber that reaches ~ceil at the top.
                c: Vec3::new(c.x, 0.0, c.z),
                r: Vec3::new(hx + 1.2, ceil * 0.95, hz + 1.2),
            }
        })
        .collect();

    let mut tubes: Vec<Tube> = Vec::new();
    for cr in corridors {
        for seg in cr.path.windows(2) {
            let a = Vec3::new(seg[0].x, TUNNEL_Y, seg[0].z);
            let b = Vec3::new(seg[1].x, TUNNEL_Y, seg[1].z);
            tubes.push(Tube { a, b });
        }
    }

    // Air SDF (negative inside the void): smooth union of every cavity.
    let air = |p: Vec3| -> f32 {
        let mut d = f32::INFINITY;
        for b in &blobs {
            d = smin(d, sd_ellipsoid(p, b.c, b.r), BLEND_K);
        }
        for t in &tubes {
            d = smin(d, sd_round_cone(p, t.a, t.b, TUNNEL_R, TUNNEL_R), BLEND_K);
        }
        d
    };

    // Rock field: intersect the void with the half-space y > 0 (flat floor),
    // then negate so rock is negative and the meshed normal points into the
    // air. `field < 0` == solid rock; the surface is the cave wall.
    let field = move |p: Vec3| -> f32 {
        let cavity = air(p).max(-p.y); // void above the floor only
        -cavity
    };

    // ---- grid bounds from the layout, padded for blob overflow ----
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for rm in rooms {
        lo = lo.min(rm.min);
        hi = hi.max(rm.max);
    }
    for cr in corridors {
        for pt in &cr.path {
            lo = lo.min(*pt);
            hi = hi.max(*pt);
        }
    }
    let pad = BLEND_K + 1.5;
    lo.x -= pad;
    lo.z -= pad;
    hi.x += pad;
    hi.z += pad;
    lo.y = -1.0; // straddle the floor so the y=0 surface is captured
    hi.y = if include_ceiling { ceil + 2.0 } else { 2.6 };

    // ---- cell size: fine by default, coarsened if the grid would blow up ----
    let mut cell = (0.7 / detail.clamp(0.5, 2.0)).clamp(0.42, 1.2);
    let est = |c: f32| -> f64 {
        let d = (hi - lo) / c;
        (d.x.ceil() as f64 + 2.0) * (d.y.ceil() as f64 + 2.0) * (d.z.ceil() as f64 + 2.0)
    };
    while est(cell) > SAMPLE_CAP && cell < 6.0 {
        cell *= 1.2;
    }

    // ---- color: purple fungal rock, darker in low crevices, subtle noise ----
    let rock_lo = pal.rock[0];
    let rock_hi = pal.rock[1];
    let tint = pal.terrain[2];
    let color = move |p: Vec3| -> Vec3 {
        let n = noise(p * 0.37);
        let base = lerp(rock_lo, rock_hi, n);
        // darker toward the floor, lighter toward the ceiling
        let up = (p.y / ceil).clamp(0.0, 1.0);
        let shaded = lerp(base * 0.62, base, 0.4 + up * 0.6);
        lerp(shaded, tint, 0.12)
    };

    mesh_field(lo, hi, cell, &field, &color)
}

#[cfg(test)]
mod tests {
    use super::super::model::DungeonModel;
    use super::super::{merged_asset, overview_asset};
    use crate::palette::by_name;
    use crate::recipe::DungeonParams;

    fn params(json: &str) -> DungeonParams {
        serde_json::from_str(json).unwrap()
    }

    fn tri_count(asset: &crate::gltf::Asset) -> usize {
        asset.parts.iter().map(|p| p.mesh.triangle_count()).sum()
    }

    #[test]
    fn cavern_builds_organic_mesh_distinct_from_crypt() {
        let cav_p = params(r#"{"kind":"dungeon","type":"cavern","size":"small","seed":3}"#);
        let cav = DungeonModel::new(&cav_p, &by_name("fungal")).unwrap();
        let cav_asset = merged_asset(&cav);
        for p in &cav_asset.parts {
            p.mesh.validate().unwrap();
        }
        let cav_tris = tri_count(&cav_asset);
        // an SDF cave is a big continuous surface — thousands of tris.
        assert!(cav_tris > 2000, "cavern too sparse: {cav_tris}");

        // same seed, crypt theme => the boxy geometry has a very different
        // (much smaller) triangle budget, proving the cavern path diverged.
        let crypt_p = params(r#"{"kind":"dungeon","type":"crypt","size":"small","seed":3}"#);
        let crypt = DungeonModel::new(&crypt_p, &by_name("necrotic")).unwrap();
        let crypt_tris = tri_count(&merged_asset(&crypt));
        assert_ne!(cav_tris, crypt_tris);

        // deterministic: rebuilding the same cavern yields an identical mesh.
        let again = tri_count(&merged_asset(
            &DungeonModel::new(&cav_p, &by_name("fungal")).unwrap(),
        ));
        assert_eq!(cav_tris, again);

        // ceiling-less overview is a strict subset (open top) -> fewer tris.
        let ov = overview_asset(&cav);
        for p in &ov.parts {
            p.mesh.validate().unwrap();
        }
        assert!(
            tri_count(&ov) < cav_tris,
            "overview should drop the ceiling dome"
        );
    }
}
