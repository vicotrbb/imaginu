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
    /// Asset the spawn should instantiate (currently only the boss spawn,
    /// when the dungeon carries an inline `boss`). Absent for every other
    /// spawn kind, and `skip_serializing_if` keeps the key out of the JSON
    /// entirely so boss-less manifests stay byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
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
            file: None,
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
    let mut man = create(m);
    if let Some(bp) = &m.p.boss {
        let room = m
            .rooms
            .iter()
            .find(|r| r.kind == super::model::RoomKind::Boss)
            .ok_or("dungeon has a `boss` but no boss room (single-room layout?)")?;
        let spawn = m
            .spawns
            .iter()
            .find(|s| s.kind == super::model::SpawnKind::Boss)
            .ok_or("dungeon has a `boss` but no boss spawn point")?;
        let asset = super::place_boss(bp, room, spawn.pos);
        let glb = crate::gltf::to_glb(&asset);
        let path = dir.join("boss.glb");
        std::fs::write(&path, &glb).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        if let Some(entry) = man.spawn_points.iter_mut().find(|s| s.kind == "boss") {
            entry.file = Some("boss.glb".into());
        }
    }
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
    let mut tris_known = true;
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
        match parse_tris(&summary) {
            Some(n) => tris_total += n,
            None => tris_known = false,
        }
    }
    for s in &man.spawn_points {
        let Some(f) = &s.file else { continue };
        let path = dir.join(f);
        if !path.exists() {
            return Err(format!("spawn '{}' references missing file {f}", s.kind));
        }
        crate::validate::validate_glb(&path).map_err(|e| format!("{f}: {e}"))?;
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        crate::validate::validate_boss_bytes(&bytes).map_err(|e| format!("{f}: {e}"))?;
    }
    // Keep the tri count a diagnostic: report it only when every room's GLB
    // summary parsed cleanly, so an unexpected format never masquerades as 0.
    let tris = if tris_known {
        format!(", {tris_total} tris total")
    } else {
        String::new()
    };
    Ok(format!(
        "{} rooms, {} corridors, {} doors, {} spawns{tris}",
        man.rooms.len(),
        man.corridors.len(),
        man.doors.len(),
        man.spawn_points.len()
    ))
}

/// Pull the triangle count out of a `validate_glb` summary, whose format is
/// `"{tris} tris, {clips} clips, {imgs} images"`. Returns `None` if the summary
/// doesn't match that shape rather than silently coercing to 0.
fn parse_tris(summary: &str) -> Option<u64> {
    let (num, rest) = summary.split_once(' ')?;
    if !rest.starts_with("tris") {
        return None;
    }
    num.parse::<u64>().ok()
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

    #[test]
    fn dungeon_emits_and_references_inline_boss() {
        let p = params(
            r#"{"kind":"dungeon","type":"crypt","size":"small","boss":{"archetype":"hydra","element":"necrotic"}}"#,
        );
        assert!(p.boss.is_some());
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        let dir = std::env::temp_dir().join(format!("imaginu_boss_dtest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_dir(&m, &dir).unwrap();
        assert!(dir.join("boss.glb").exists());

        let text = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        let man: Manifest = serde_json::from_str(&text).unwrap();
        let boss_spawn = man
            .spawn_points
            .iter()
            .find(|s| s.kind == "boss")
            .expect("boss spawn present");
        assert_eq!(boss_spawn.file.as_deref(), Some("boss.glb"));

        let summary = validate_dir(&dir).unwrap();
        assert!(summary.contains("rooms"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
