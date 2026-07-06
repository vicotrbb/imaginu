//! `manifest.json`: the streaming index a game loads first — grid layout,
//! per-chunk files + conservative bounds, POIs, road/river polylines and
//! zone summary. Deterministic: identical for lazy and full builds.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::model::WorldModel;

pub const FORMAT: &str = "imaginu-world/1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub name: String,
    pub seed: u64,
    pub palette: String,
    /// Total world extent in meters [x, z].
    pub size: [f32; 2],
    pub chunk_size: f32,
    pub chunk_resolution: u32,
    /// Grid dimensions in chunks [nx, nz].
    pub grid: [u32; 2],
    pub sea_level: f32,
    pub chunks: Vec<ChunkEntry>,
    pub pois: Vec<Poi>,
    pub roads: Vec<Polyline>,
    pub rivers: Vec<Polyline>,
    pub zones: Vec<ZoneSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkEntry {
    pub x: u32,
    pub z: u32,
    pub file: String,
    /// World-space translation to place the (chunk-local) GLB.
    pub position: [f32; 3],
    /// Conservative world-space AABB (probed heights, padded).
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Poi {
    pub name: String,
    pub kind: String,
    pub position: [f32; 3],
    pub radius: f32,
    /// Separate GLB placed at `position` (chunks stay lean).
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub spawn_points: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Polyline {
    pub kind: String,
    pub width: f32,
    pub points: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneSummary {
    pub kind: String,
    /// Zone cell center in world coordinates.
    pub center: [f32; 2],
}

pub fn chunk_file(cx: u32, cz: u32) -> String {
    format!("chunk_{cx}_{cz}.glb")
}

/// Build the manifest for the whole grid. Bounds come from a coarse probe
/// of the world height function (17×17 per chunk) padded generously — exact
/// enough for streaming/culling, cheap enough for lazy single-chunk builds.
pub fn create(m: &WorldModel) -> Manifest {
    let cs = m.p.chunk_size;
    let mut chunks = Vec::with_capacity((m.nx * m.nz) as usize);
    for cz in 0..m.nz {
        for cx in 0..m.nx {
            let (ox, oz) = m.chunk_origin(cx, cz);
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for iz in 0..=16 {
                for ix in 0..=16 {
                    let h = m.height(
                        ox + (ix as f32 / 16.0 - 0.5) * cs,
                        oz + (iz as f32 / 16.0 - 0.5) * cs,
                    );
                    lo = lo.min(h);
                    hi = hi.max(h);
                }
            }
            lo = lo.min(m.p.sea_level) - 8.0;
            hi = hi + 16.0; // headroom for detail octaves + scattered trees
            chunks.push(ChunkEntry {
                x: cx,
                z: cz,
                file: chunk_file(cx, cz),
                position: [ox, 0.0, oz],
                min: [ox - cs / 2.0, lo, oz - cs / 2.0],
                max: [ox + cs / 2.0, hi, oz + cs / 2.0],
            });
        }
    }
    Manifest {
        format: FORMAT.into(),
        name: m.p.name.clone(),
        seed: m.p.seed,
        palette: m.p.palette.clone(),
        size: [m.size_x, m.size_z],
        chunk_size: cs,
        chunk_resolution: m.p.chunk_resolution,
        grid: [m.nx, m.nz],
        sea_level: m.p.sea_level,
        chunks,
        pois: Vec::new(),
        roads: Vec::new(),
        rivers: Vec::new(),
        zones: m
            .zones
            .cells_in(
                [-m.size_x / 2.0, -m.size_z / 2.0],
                [m.size_x / 2.0, m.size_z / 2.0],
            )
            .into_iter()
            .map(|(kind, center)| ZoneSummary { kind: kind.name().into(), center })
            .collect(),
    }
}

/// Validate a world output directory: manifest structure + every chunk GLB
/// present and structurally valid (missing chunks are an error — build the
/// full map before shipping it).
pub fn validate_dir(dir: &Path) -> Result<String, String> {
    let man_path = dir.join("manifest.json");
    let text = std::fs::read_to_string(&man_path)
        .map_err(|e| format!("cannot read {}: {e}", man_path.display()))?;
    let man: Manifest =
        serde_json::from_str(&text).map_err(|e| format!("bad manifest.json: {e}"))?;
    if man.format != FORMAT {
        return Err(format!("unknown manifest format '{}'", man.format));
    }
    let expect = (man.grid[0] as usize) * (man.grid[1] as usize);
    if man.chunks.len() != expect {
        return Err(format!(
            "manifest lists {} chunks, grid {}×{} needs {expect}",
            man.chunks.len(),
            man.grid[0],
            man.grid[1]
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for c in &man.chunks {
        if c.x >= man.grid[0] || c.z >= man.grid[1] {
            return Err(format!("chunk ({},{}) outside grid", c.x, c.z));
        }
        if !seen.insert((c.x, c.z)) {
            return Err(format!("duplicate chunk entry ({},{})", c.x, c.z));
        }
        for i in 0..3 {
            if c.min[i] > c.max[i] {
                return Err(format!("chunk ({},{}) has inverted bounds", c.x, c.z));
            }
        }
    }
    for p in &man.pois {
        if let Some(f) = &p.file {
            if !dir.join(f).exists() {
                return Err(format!("POI '{}' references missing file {f}", p.name));
            }
            crate::validate::validate_glb(&dir.join(f))
                .map_err(|e| format!("POI GLB {f}: {e}"))?;
        }
    }
    let mut missing = 0usize;
    let mut tris_total = 0u64;
    for c in &man.chunks {
        let path = dir.join(&c.file);
        if !path.exists() {
            missing += 1;
            continue;
        }
        let summary =
            crate::validate::validate_glb(&path).map_err(|e| format!("{}: {e}", c.file))?;
        tris_total += summary
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
    }
    if missing > 0 {
        return Err(format!(
            "{missing}/{} chunk GLB(s) missing (lazy build? run the full build)",
            man.chunks.len()
        ));
    }
    Ok(format!(
        "{} chunks ({}×{}), {} POIs, {} roads, {} rivers, {tris_total} tris total",
        man.chunks.len(),
        man.grid[0],
        man.grid[1],
        man.pois.len(),
        man.roads.len(),
        man.rivers.len()
    ))
}
