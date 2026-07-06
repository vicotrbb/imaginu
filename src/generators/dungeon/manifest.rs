//! `manifest.json` for a dungeon: the index a game loads first — per-room GLB
//! files with bounds, the corridor polylines, doors and spawn points. Mirrors
//! `world/manifest.rs`. Deterministic and round-trips through serde.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::recipe::DungeonTheme;

use super::model::DungeonModel;

pub const FORMAT: &str = "imaginu-dungeon/1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub name: String,
    pub seed: u64,
    pub palette: String,
    pub theme: String,
    /// Overall AABB as [min, max].
    pub bounds: [[f32; 3]; 2],
    pub rooms: Vec<RoomEntry>,
    pub corridors: Vec<Polyline>,
    pub doors: Vec<DoorEntry>,
    pub spawn_points: Vec<SpawnEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomEntry {
    pub id: usize,
    pub kind: String,
    pub file: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Polyline {
    pub kind: String,
    pub width: f32,
    pub points: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoorEntry {
    pub room: usize,
    pub pos: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpawnEntry {
    pub kind: String,
    pub pos: [f32; 3],
}

pub fn theme_name(t: DungeonTheme) -> &'static str {
    match t {
        DungeonTheme::Crypt => "crypt",
        DungeonTheme::Cavern => "cavern",
        DungeonTheme::Sewer => "sewer",
        DungeonTheme::Mine => "mine",
        DungeonTheme::Temple => "temple",
        DungeonTheme::Fortress => "fortress",
    }
}

pub fn room_file(id: usize) -> String {
    format!("room_{id}.glb")
}

pub fn create(m: &DungeonModel) -> Manifest {
    let rooms = m
        .rooms
        .iter()
        .map(|r| RoomEntry {
            id: r.id,
            kind: r.kind.name().into(),
            file: room_file(r.id),
            min: [r.min.x, r.min.y, r.min.z],
            max: [r.max.x, r.max.y, r.max.z],
        })
        .collect();
    let corridors = m
        .corridors
        .iter()
        .map(|c| Polyline {
            kind: "corridor".into(),
            width: 3.0,
            points: c.path.iter().map(|p| [p.x, p.y, p.z]).collect(),
        })
        .collect();
    let doors = m
        .doors
        .iter()
        .map(|d| DoorEntry {
            room: d.room,
            pos: [d.pos.x, d.pos.y, d.pos.z],
        })
        .collect();
    let spawn_points = m
        .spawns
        .iter()
        .map(|s| SpawnEntry {
            kind: s.kind.name().into(),
            pos: [s.pos.x, s.pos.y, s.pos.z],
        })
        .collect();
    Manifest {
        format: FORMAT.into(),
        name: format!("{}_dungeon", theme_name(m.p.theme)),
        seed: m.p.seed,
        palette: m.pal.name.into(),
        theme: theme_name(m.p.theme).into(),
        bounds: [
            [m.bounds.0.x, m.bounds.0.y, m.bounds.0.z],
            [m.bounds.1.x, m.bounds.1.y, m.bounds.1.z],
        ],
        rooms,
        corridors,
        doors,
        spawn_points,
    }
}

/// Write per-room GLBs + `manifest.json` to `dir`.
pub fn write_dir(m: &DungeonModel, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for room in &m.rooms {
        let asset = super::room_asset(m, room);
        let glb = crate::gltf::to_glb(&asset);
        let path = dir.join(room_file(room.id));
        std::fs::write(&path, &glb).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    let man = create(m);
    let json = serde_json::to_string_pretty(&man).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("manifest.json"), json)
        .map_err(|e| format!("cannot write manifest.json: {e}"))?;
    Ok(())
}

/// Validate a dungeon output directory: manifest structure + every room GLB
/// present and structurally valid.
pub fn validate_dir(dir: &Path) -> Result<String, String> {
    let man_path = dir.join("manifest.json");
    let text = std::fs::read_to_string(&man_path)
        .map_err(|e| format!("cannot read {}: {e}", man_path.display()))?;
    let man: Manifest =
        serde_json::from_str(&text).map_err(|e| format!("bad manifest.json: {e}"))?;
    if man.format != FORMAT {
        return Err(format!("unknown manifest format '{}'", man.format));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut tris_total = 0u64;
    for r in &man.rooms {
        if !seen.insert(r.id) {
            return Err(format!("duplicate room id {}", r.id));
        }
        for i in 0..3 {
            if r.min[i] > r.max[i] {
                return Err(format!("room {} has inverted bounds", r.id));
            }
        }
        let path = dir.join(&r.file);
        if !path.exists() {
            return Err(format!("room {} references missing file {}", r.id, r.file));
        }
        let summary =
            crate::validate::validate_glb(&path).map_err(|e| format!("{}: {e}", r.file))?;
        tris_total += summary
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
    }
    Ok(format!(
        "{} rooms, {} corridors, {} doors, {} spawns, {tris_total} tris total",
        man.rooms.len(),
        man.corridors.len(),
        man.doors.len(),
        man.spawn_points.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::by_name;
    use crate::recipe::DungeonParams;

    fn params(json: &str) -> DungeonParams {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn manifest_round_trips_through_serde() {
        let p = params(r#"{"kind":"dungeon","type":"crypt","size":"medium"}"#);
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        let man = create(&m);
        assert_eq!(man.format, "imaginu-dungeon/1");
        assert_eq!(man.rooms.len(), m.rooms.len());
        let s = serde_json::to_string(&man).unwrap();
        let back: Manifest = serde_json::from_str(&s).unwrap();
        assert_eq!(back.rooms.len(), man.rooms.len());
        assert_eq!(back.theme, "crypt");
        // deterministic serialization
        assert_eq!(s, serde_json::to_string(&create(&m)).unwrap());
    }

    #[test]
    fn write_and_validate_round_trip() {
        let p = params(r#"{"kind":"dungeon","type":"crypt","size":"small"}"#);
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        let dir = std::env::temp_dir().join(format!("imaginu_dtest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_dir(&m, &dir).unwrap();
        let summary = validate_dir(&dir).unwrap();
        assert!(summary.contains("rooms"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
