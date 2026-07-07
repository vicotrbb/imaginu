//! Boss gameplay metadata: weak points, destructible parts, per-phase ability
//! timings, arena spawn. Serialized into `nodes[0].extras.imaginu_boss`
//! (format `imaginu-boss/1`), a sibling of the untouched `imaginu_physics`.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ColliderJson {
    Sphere { radius: f32 },
    Box { half_extents: [f32; 3] },
    Capsule { radius: f32, height: f32 },
}

#[derive(Clone, Debug, Serialize)]
pub struct AbilityMeta {
    pub name: String,
    pub telegraph_s: f32,
    pub active_s: f32,
    pub recover_s: f32,
    pub clip: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseMeta {
    pub id: u32,
    pub name: String,
    pub hp_fraction: f32,
    pub enrage: bool,
    pub active_weak_points: Vec<String>,
    pub abilities: Vec<AbilityMeta>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WeakPointMeta {
    pub name: String,
    pub joint: String,
    pub collider: ColliderJson,
    pub offset: [f32; 3],
    pub destructible: bool,
    pub phase: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartMeta {
    pub name: String,
    pub joint: String,
    pub destructible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArenaMeta {
    pub recommended_radius: f32,
    pub spawn_offset: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct BossMeta {
    pub format: &'static str,
    pub archetype: String,
    pub element: String,
    pub phases: Vec<PhaseMeta>,
    pub weak_points: Vec<WeakPointMeta>,
    pub parts: Vec<PartMeta>,
    pub arena: ArenaMeta,
}

impl BossMeta {
    pub fn new(archetype: String, element: String) -> Self {
        Self {
            format: "imaginu-boss/1",
            archetype,
            element,
            phases: Vec::new(),
            weak_points: Vec::new(),
            parts: Vec::new(),
            arena: ArenaMeta {
                recommended_radius: 8.0,
                spawn_offset: [0.0, 0.0, 0.0],
            },
        }
    }
}

/// Deterministic JSON for the extras block.
pub fn boss_meta_json(m: &BossMeta) -> Value {
    serde_json::to_value(m).expect("boss meta serializes")
}
