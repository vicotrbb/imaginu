//! World compiler: one recipe -> a directory of seamless, streamable chunk
//! GLBs + `manifest.json`. Every height/color sample is a pure function of
//! WORLD coordinates + seed (the "seam law"), so adjacent chunks share
//! bit-identical edges and any chunk built alone equals the same chunk
//! built in a full run.

pub mod chunk;
pub mod erosion;
pub mod manifest;
pub mod minimap;
pub mod model;
pub mod network;
pub mod overview;
pub mod poi;
pub mod zones;

use serde::{Deserialize, Serialize};

fn d_seed() -> u64 { 1 }
fn d_palette() -> String { "verdant".into() }
fn d_name() -> String { "world".into() }
fn d_size() -> f32 { 2048.0 }
fn d_chunk_size() -> f32 { 256.0 }
fn d_chunk_res() -> u32 { 128 }
fn d_one() -> f32 { 1.0 }
fn d_true() -> bool { true }

/// The `{"kind":"world", ...}` recipe surface.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorldParams {
    #[serde(default = "d_name")]
    pub name: String,
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_palette")]
    pub palette: String,
    /// World edge length in meters (square map). Snapped to a whole number
    /// of chunks.
    #[serde(default = "d_size")]
    pub size: f32,
    /// Chunk edge length in meters.
    #[serde(default = "d_chunk_size")]
    pub chunk_size: f32,
    /// Grid cells per chunk edge (vertices = res+1).
    #[serde(default = "d_chunk_res")]
    pub chunk_resolution: u32,
    #[serde(default = "d_one")]
    pub mountainousness: f32,
    /// Absolute water elevation in meters (world-space, so the sea is one
    /// continuous plane across every chunk).
    #[serde(default)]
    pub sea_level: f32,
    #[serde(default = "d_true")]
    pub scatter: bool,
    #[serde(default = "d_one")]
    pub scatter_density: f32,
    /// Zone mix: `[{"kind":"forest","weight":2}, {"kind":"lake",
    /// "at":[300,-500],"radius":400}]`. Empty → a sensible default mix.
    #[serde(default)]
    pub zones: Vec<zones::ZoneSpec>,
    /// Approximate zone region diameter in meters.
    #[serde(default = "d_zone_size")]
    pub zone_size: f32,
    /// POIs: omit for area-scaled defaults, `[]` for none, or
    /// `[{"kind":"city","count":2},{"kind":"castle","at":[500,-800]}]`.
    #[serde(default)]
    pub pois: Option<Vec<poi::PoiSpec>>,
    /// World-scale rivers traced from mountain springs to lakes/sea.
    /// Omit for an area-scaled default count.
    #[serde(default)]
    pub rivers: Option<u32>,
    /// Road network connecting cities/villages/castles (A*, slope-penalized,
    /// bridges where roads cross rivers).
    #[serde(default = "d_true")]
    pub roads: bool,
    /// World-scale erosion strength 0..1: a global coarse heightmap is
    /// eroded once and upsampled, so gullies span chunks seamlessly.
    #[serde(default = "d_erosion")]
    pub erosion: f32,
    /// Pick per-chunk mesh resolution by terrain roughness/POI presence
    /// (flat plains coarse, mountains & settlements fine; edges stitched).
    #[serde(default = "d_true")]
    pub adaptive_resolution: bool,
    /// Embed N decimated LOD levels per chunk (MSFT_lod).
    #[serde(default)]
    pub lods: u32,
}
fn d_erosion() -> f32 { 0.5 }
fn d_zone_size() -> f32 { 900.0 }

impl WorldParams {
    /// Parse a `{"kind":"world"}` recipe.
    pub fn parse(json: &str) -> Result<Self, String> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("invalid world recipe: {e}"))?;
        match v.get("kind").and_then(|k| k.as_str()) {
            Some("world") => {}
            Some(other) => {
                return Err(format!(
                    "recipe kind is '{other}', not 'world' — use `imaginu generate` for single assets"
                ));
            }
            None => return Err("world recipe needs \"kind\":\"world\"".into()),
        }
        serde_json::from_value(v).map_err(|e| format!("invalid world recipe: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf::to_glb;

    fn tiny() -> WorldParams {
        // pinned lake at the origin: its feathered override crosses every
        // inner seam, so the seam test also covers zone blending
        WorldParams::parse(
            r#"{"kind":"world","seed":11,"size":192,"chunk_size":64,
                "chunk_resolution":32,"sea_level":-2.0,"scatter":false,
                "adaptive_resolution":false,
                "zones":[{"kind":"forest","weight":2},{"kind":"mountains","weight":1},
                         {"kind":"lake","at":[0,0],"radius":60}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn world_recipe_parses_with_defaults() {
        let p = WorldParams::parse(r#"{"kind":"world"}"#).unwrap();
        assert_eq!(p.chunk_size, 256.0);
        assert_eq!(p.chunk_resolution, 128);
        assert!(WorldParams::parse(r#"{"kind":"terrain"}"#).is_err());
    }

    /// The seam law, sampled over a full 3×3 world: every pair of adjacent
    /// chunks must share bit-identical edge vertices (positions AND colors).
    #[test]
    fn world_chunks_tile_seamlessly_3x3() {
        let m = model::WorldModel::new(&tiny()).unwrap();
        assert_eq!((m.nx, m.nz), (3, 3));
        // the smooth vertex grid is the seam contract: shared edge vertices
        // must match bit-exactly in position AND color (flat shading later
        // averages per-face colors, which is face-to-face variation, not a
        // seam)
        let meshes: Vec<Vec<crate::mesh::Mesh>> = (0..3)
            .map(|cz| (0..3).map(|cx| chunk::vertex_grid(&m, cx, cz).1).collect())
            .collect();
        let cs = m.p.chunk_size;
        let edge = |mesh: &crate::mesh::Mesh, axis: usize, side: f32| -> Vec<[i64; 5]> {
            let mut v: Vec<[i64; 5]> = mesh
                .positions
                .iter()
                .zip(&mesh.colors)
                .filter(|(p, _)| p[axis] == side)
                .map(|(p, c)| {
                    let along = if axis == 0 { p.z } else { p.x };
                    [
                        along.to_bits() as i64,
                        p.y.to_bits() as i64,
                        c.x.to_bits() as i64,
                        c.y.to_bits() as i64,
                        c.z.to_bits() as i64,
                    ]
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        for cz in 0..3usize {
            for cx in 0..3usize {
                if cx + 1 < 3 {
                    let a = edge(&meshes[cz][cx], 0, cs / 2.0);
                    let b = edge(&meshes[cz][cx + 1], 0, -cs / 2.0);
                    assert!(!a.is_empty());
                    assert_eq!(a, b, "x-seam between ({cx},{cz}) and ({},{cz})", cx + 1);
                }
                if cz + 1 < 3 {
                    let a = edge(&meshes[cz][cx], 2, cs / 2.0);
                    let b = edge(&meshes[cz + 1][cx], 2, -cs / 2.0);
                    assert_eq!(a, b, "z-seam between ({cx},{cz}) and ({cx},{})", cz + 1);
                }
            }
        }
    }

    /// Chunk built alone == the same chunk built in any other order, and
    /// repeated builds are byte-identical (scatter included).
    #[test]
    fn world_chunk_build_order_independent() {
        let mut p = tiny();
        p.scatter = true;
        let m1 = model::WorldModel::new(&p).unwrap();
        let full: Vec<Vec<u8>> = (0..9)
            .map(|i| to_glb(&chunk::build(&m1, i % 3, i / 3)))
            .collect();
        let m2 = model::WorldModel::new(&p).unwrap();
        let alone = to_glb(&chunk::build(&m2, 1, 1));
        assert_eq!(full[4], alone, "chunk (1,1) must not depend on build order");
        let again = to_glb(&chunk::build(&m2, 1, 1));
        assert_eq!(alone, again, "chunk build must be deterministic");
    }

    #[test]
    fn world_manifest_covers_grid() {
        let p = tiny();
        let m = model::WorldModel::new(&p).unwrap();
        let man = manifest::create(&m);
        assert_eq!(man.grid, [3, 3]);
        assert_eq!(man.chunks.len(), 9);
        // deterministic order: z-major, x-minor
        assert_eq!(man.chunks[0].file, "chunk_0_0.glb");
        assert_eq!(man.chunks[3].file, "chunk_0_1.glb");
        // bounds are conservative: mesh must fit inside
        let a = chunk::build(&m, 0, 0);
        let (lo, hi) = a.parts[0].mesh.bounds();
        let e = &man.chunks[0];
        let (ox, oz) = m.chunk_origin(0, 0);
        assert!(e.min[1] <= lo.y + 1e-3 && e.max[1] >= hi.y - 1e-3);
        assert!(e.min[0] <= ox + lo.x + 1e-3 && e.max[0] >= ox + hi.x - 1e-3);
        assert!(e.min[2] <= oz + lo.z + 1e-3 && e.max[2] >= oz + hi.z - 1e-3);
        // manifest serializes deterministically
        let j1 = serde_json::to_string_pretty(&man).unwrap();
        let j2 = serde_json::to_string_pretty(&manifest::create(&m)).unwrap();
        assert_eq!(j1, j2);
        // zone summary present
        assert!(!man.zones.is_empty());
    }

    #[test]
    fn zones_shape_the_terrain() {
        let mk = |zone: &str| {
            let p = WorldParams::parse(&format!(
                r#"{{"kind":"world","seed":3,"size":1024,"chunk_size":256,
                    "sea_level":0.0,
                    "zones":[{{"kind":"plains","weight":1}},
                             {{"kind":"{zone}","at":[0,0],"radius":700}}]}}"#
            ))
            .unwrap();
            model::WorldModel::new(&p).unwrap()
        };
        // pinned lake sinks the center below sea level; the sea plane fills it
        let lake = mk("lake");
        assert!(lake.height(0.0, 0.0) < 0.0, "lake pin must scoop below sea");
        // pinned mountains tower over the plains baseline
        let mtn = mk("mountains");
        let plains_ref = mk("plains");
        let mut hi_m = f32::NEG_INFINITY;
        let mut hi_p = f32::NEG_INFINITY;
        for i in 0..64 {
            let a = i as f32 / 64.0 * core::f32::consts::TAU;
            let (x, z) = (a.cos() * 300.0, a.sin() * 300.0);
            hi_m = hi_m.max(mtn.height(x, z));
            hi_p = hi_p.max(plains_ref.height(x, z));
        }
        assert!(hi_m > hi_p + 15.0, "mountains {hi_m} vs plains {hi_p}");
        // weights are normalized everywhere
        for i in 0..40 {
            let (x, z) = (i as f32 * 47.3 - 900.0, i as f32 * 31.7 - 600.0);
            let w = lake.zones.weights(x, z);
            let sum: f32 = w.iter().sum();
            assert!((sum - 1.0).abs() < 1e-3, "weights sum {sum} at ({x},{z})");
        }
    }

    #[test]
    fn pois_place_and_flatten() {
        let p = WorldParams::parse(
            r#"{"kind":"world","seed":9,"size":1536,"chunk_size":256,
                "chunk_resolution":32,"sea_level":0,"scatter":false,
                "zones":[{"kind":"plains","weight":2},{"kind":"forest","weight":1},
                         {"kind":"mountains","weight":1}],
                "pois":[{"kind":"city","count":1},{"kind":"village","count":2},
                        {"kind":"watchtower","count":1},
                        {"kind":"castle","at":[200,180],"name":"Pinhold"}]}"#,
        )
        .unwrap();
        let m = model::WorldModel::new(&p).unwrap();
        // pinned castle honored (position + name)
        let castle = m
            .pois
            .iter()
            .find(|s| s.kind == poi::PoiKind::Castle)
            .expect("pinned castle placed");
        assert_eq!((castle.x, castle.z), (200.0, 180.0));
        assert_eq!(castle.name, "Pinhold");
        let cities = m.pois.iter().filter(|s| s.kind == poi::PoiKind::City).count();
        assert_eq!(cities, 1);
        // every settlement sits above water on flat ground
        for s in &m.pois {
            assert!(s.ground > m.p.sea_level, "{} underwater", s.name);
            if s.kind == poi::PoiKind::Dungeon {
                continue;
            }
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for i in 0..24 {
                let a = i as f32 / 24.0 * core::f32::consts::TAU;
                let d = if i % 2 == 0 { 0.45 } else { 0.9 } * s.radius;
                let h = m.height(s.x + a.cos() * d, s.z + a.sin() * d);
                lo = lo.min(h);
                hi = hi.max(h);
            }
            assert!(
                hi - lo < 1.5,
                "{} ground not flat: span {}",
                s.name,
                hi - lo
            );
        }
        // determinism: same recipe → same sites
        let m2 = model::WorldModel::new(&p).unwrap();
        assert_eq!(m.pois.len(), m2.pois.len());
        for (a, b) in m.pois.iter().zip(&m2.pois) {
            assert_eq!((a.x, a.z, a.seed, a.name.clone()), (b.x, b.z, b.seed, b.name.clone()));
        }
        // manifest carries them with files + spawn points (+ bridges)
        let man = manifest::create(&m);
        let non_bridge = man.pois.iter().filter(|p| p.kind != "bridge").count();
        assert_eq!(non_bridge, m.pois.len());
        assert!(man.pois.iter().all(|p| p.file.is_some() && !p.spawn_points.is_empty()));
    }

    #[test]
    fn rivers_and_roads_carve_the_world() {
        // mountains north feed a river; two pinned villages get a road
        let p = WorldParams::parse(
            r#"{"kind":"world","seed":21,"size":1536,"chunk_size":256,
                "chunk_resolution":32,"sea_level":0,"scatter":false,
                "zones":[{"kind":"plains","weight":3},
                         {"kind":"mountains","at":[0,-500],"radius":550},
                         {"kind":"lake","at":[100,550],"radius":320}],
                "pois":[{"kind":"village","at":[-450,300]},
                        {"kind":"village","at":[450,300]}],
                "rivers":2}"#,
        )
        .unwrap();
        let m = model::WorldModel::new(&p).unwrap();
        assert!(!m.network.rivers.is_empty(), "expected at least one river");
        assert_eq!(m.network.roads.len(), 1, "two villages → one road");
        for r in &m.network.rivers {
            // beds descend monotonically
            for w in r.points.windows(2) {
                assert!(w[1].y <= w[0].y + 1e-4, "river bed must not climb");
            }
            // channel is carved: center lower than 25 m to the side
            let mid = r.points[r.points.len() / 2];
            let h_center = m.height(mid.x, mid.z);
            let h_side = m.height(mid.x + 25.0, mid.z).max(m.height(mid.x - 25.0, mid.z));
            assert!(
                h_center < h_side - 0.5,
                "river channel not carved: {h_center} vs {h_side}"
            );
        }
        // road deck is walkable: terrain under the road midpoint ≈ deck
        let road = &m.network.roads[0];
        let mid = road.points[road.points.len() / 2];
        let h = m.height(mid.x, mid.z);
        assert!((h - mid.y).abs() < 1.0, "road not flattened: {h} vs deck {}", mid.y);
        // determinism
        let m2 = model::WorldModel::new(&p).unwrap();
        assert_eq!(m.network.rivers.len(), m2.network.rivers.len());
        assert_eq!(m.network.roads[0].points.len(), m2.network.roads[0].points.len());
        // manifest carries polylines
        let man = manifest::create(&m);
        assert!(!man.rivers.is_empty());
        assert_eq!(man.roads.len(), 1);
    }

    #[test]
    fn global_erosion_field_is_seamless_and_deterministic() {
        let base = r#"{"kind":"world","seed":13,"size":768,"chunk_size":256,
            "chunk_resolution":32,"sea_level":0,"scatter":false,
            "adaptive_resolution":false,"pois":[],"rivers":0,
            "zones":[{"kind":"mountains","weight":1},{"kind":"plains","weight":1}],
            "erosion":ER}"#;
        let with = model::WorldModel::new(
            &WorldParams::parse(&base.replace("ER", "0.8")).unwrap(),
        )
        .unwrap();
        let without = model::WorldModel::new(
            &WorldParams::parse(&base.replace("ER", "0.0")).unwrap(),
        )
        .unwrap();
        // erosion actually moves terrain
        let mut moved = 0;
        for i in 0..100 {
            let (x, z) = (i as f32 * 7.1 - 350.0, (i as f32 * 3.7) % 700.0 - 350.0);
            if (with.height(x, z) - without.height(x, z)).abs() > 0.05 {
                moved += 1;
            }
        }
        assert!(moved > 30, "erosion changed only {moved}/100 samples");
        // deterministic across model rebuilds
        let with2 = model::WorldModel::new(
            &WorldParams::parse(&base.replace("ER", "0.8")).unwrap(),
        )
        .unwrap();
        for i in 0..50 {
            let (x, z) = (i as f32 * 11.3 - 300.0, i as f32 * 5.9 - 200.0);
            assert_eq!(with.height(x, z).to_bits(), with2.height(x, z).to_bits());
        }
        // seams: chunk edges still bit-identical with erosion on
        let a = chunk::vertex_grid(&with, 0, 1).1;
        let b = chunk::vertex_grid(&with, 1, 1).1;
        let cs = with.p.chunk_size;
        let ea: Vec<u32> = a
            .positions
            .iter()
            .filter(|p| p.x == cs / 2.0)
            .map(|p| p.y.to_bits())
            .collect();
        let eb: Vec<u32> = b
            .positions
            .iter()
            .filter(|p| p.x == -cs / 2.0)
            .map(|p| p.y.to_bits())
            .collect();
        assert_eq!(ea, eb);
    }

    #[test]
    fn adaptive_resolution_stitches_without_cracks() {
        // mountains pinned in one corner chunk, dead-flat plains elsewhere →
        // neighboring chunks pick different resolutions
        let p = WorldParams::parse(
            r#"{"kind":"world","seed":5,"size":768,"chunk_size":256,
                "chunk_resolution":64,"sea_level":-30,"scatter":false,
                "pois":[],"rivers":0,"erosion":0,
                "zones":[{"kind":"plains","weight":1},
                         {"kind":"mountains","at":[-256,-256],"radius":150}]}"#,
        )
        .unwrap();
        let m = model::WorldModel::new(&p).unwrap();
        let r00 = m.chunk_res(1, 0);
        let r10 = m.chunk_res(2, 0);
        assert!(r00 > r10, "expected mountain-side chunk finer: {r00} vs {r10}");
        let fine = chunk::vertex_grid(&m, 1, 0).1;
        let coarse = chunk::vertex_grid(&m, 2, 0).1;
        let cs = m.p.chunk_size;
        // coarse edge vertices must appear bit-identically in the fine edge
        let fine_edge: std::collections::BTreeMap<i64, (u32, [u32; 3])> = fine
            .positions
            .iter()
            .zip(&fine.colors)
            .filter(|(p, _)| p.x == cs / 2.0)
            .map(|(p, c)| {
                (
                    p.z.to_bits() as i64,
                    (p.y.to_bits(), [c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]),
                )
            })
            .collect();
        let mut coarse_hits = 0;
        for (p, _c) in coarse.positions.iter().zip(&coarse.colors) {
            if p.x != -cs / 2.0 {
                continue;
            }
            let key = p.z.to_bits() as i64;
            let (fy, _fc) = fine_edge.get(&key).expect("coarse edge vertex missing from fine edge");
            assert_eq!(*fy, p.y.to_bits(), "height crack at z={}", p.z);
            coarse_hits += 1;
        }
        assert!(coarse_hits > r10 as usize / 2);
        // fine midpoints must lie exactly on the coarse segments
        let ratio = (r00 / r10) as usize;
        let mut checked = 0;
        let fine_sorted: Vec<(f32, f32)> = {
            let mut v: Vec<(f32, f32)> = fine
                .positions
                .iter()
                .filter(|p| p.x == cs / 2.0)
                .map(|p| (p.z, p.y))
                .collect();
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            v.dedup_by(|a, b| a.0 == b.0);
            v
        };
        for s in (0..fine_sorted.len().saturating_sub(ratio)).step_by(ratio) {
            let w = &fine_sorted[s..=s + ratio];
            let (z0, y0) = w[0];
            let (z1, y1) = w[ratio];
            for k in 1..ratio {
                let (zk, yk) = w[k];
                let t = (zk - z0) / (z1 - z0);
                let expect = y0 + (y1 - y0) * t;
                assert!(
                    (yk - expect).abs() < 1e-3,
                    "crack at z={zk}: {yk} vs {expect}"
                );
                checked += 1;
            }
        }
        assert!(checked > 8, "checked only {checked} midpoints");
        // budget: finest chunk stays comfortably under 2M tris
        let a = chunk::build(&m, 1, 0);
        assert!(a.parts[0].mesh.triangle_count() < 2_000_000);
    }

    #[test]
    fn bridge_asset_builds() {
        let b = network::Bridge {
            pos: glam::Vec2::new(0.0, 0.0),
            yaw: 0.7,
            len: 18.0,
            deck: 5.0,
        };
        let pal = crate::palette::by_name("verdant");
        let a = poi::bridge_asset(&b, &pal);
        a.validate().unwrap();
        assert_eq!(to_glb(&a), to_glb(&poi::bridge_asset(&b, &pal)));
        let (lo, hi) = a.parts[0].mesh.bounds();
        assert!(hi.x - lo.x > 12.0, "bridge should span its length");
    }

    #[test]
    fn poi_assets_build_and_validate() {
        let pal = crate::palette::by_name("verdant");
        for kind in [
            poi::PoiKind::City,
            poi::PoiKind::Village,
            poi::PoiKind::Castle,
            poi::PoiKind::Watchtower,
            poi::PoiKind::Dungeon,
        ] {
            let site = poi::PoiSite {
                kind,
                name: format!("Test {}", kind.name()),
                x: 0.0,
                z: 0.0,
                ground: 10.0,
                radius: kind.radius(),
                seed: 77,
            };
            let a = poi::build_asset(&site, &pal);
            a.validate().unwrap_or_else(|e| panic!("{}: {e}", kind.name()));
            assert!(
                a.parts.iter().map(|p| p.mesh.triangle_count()).sum::<usize>() > 50,
                "{} too small",
                kind.name()
            );
            let g1 = to_glb(&a);
            let g2 = to_glb(&poi::build_asset(&site, &pal));
            assert_eq!(g1, g2, "{} not deterministic", kind.name());
            crate::validate::validate_glb_bytes(&g1)
                .unwrap_or_else(|e| panic!("{} glb: {e}", kind.name()));
        }
    }

    #[test]
    fn minimap_renders_deterministically() {
        let m = model::WorldModel::new(&tiny()).unwrap();
        let (w, h, a) = minimap::render(&m, 96);
        let (_, _, b) = minimap::render(&m, 96);
        assert_eq!((w, h), (96, 96));
        assert_eq!(a.len(), 96 * 96 * 3);
        assert_eq!(a, b);
        // water must appear somewhere (pinned lake) and land elsewhere
        let water = m.pal.water;
        let wr = (water.x.powf(1.0 / 2.2) * 255.0) as i32;
        let has_dark_blue = a.chunks(3).any(|c| (c[2] as i32) > c[0] as i32 + 10);
        assert!(has_dark_blue, "expected water pixels (ref r {wr})");
    }
}


