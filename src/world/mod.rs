//! World compiler: one recipe -> a directory of seamless, streamable chunk
//! GLBs + `manifest.json`. Every height/color sample is a pure function of
//! WORLD coordinates + seed (the "seam law"), so adjacent chunks share
//! bit-identical edges and any chunk built alone equals the same chunk
//! built in a full run.

pub mod chunk;
pub mod manifest;
pub mod model;

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
}

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
        WorldParams::parse(
            r#"{"kind":"world","seed":11,"size":192,"chunk_size":64,
                "chunk_resolution":32,"sea_level":-2.0,"scatter":false}"#,
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
    }
}
