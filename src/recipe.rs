//! Recipe schema — the JSON surface an AI agent writes. Small, forgiving
//! (everything except `kind` has a default), deterministic via `seed`.

use serde::{Deserialize, Serialize};

use crate::gltf::Asset;
use crate::palette;

fn d_seed() -> u64 {
    1
}
fn d_palette() -> String {
    "verdant".into()
}
fn d_true() -> bool {
    true
}
/// exposed for the custom-DSL serde defaults
pub fn d_true_pub() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TreeStyle {
    #[default]
    Oak,
    Pine,
    Palm,
    Dead,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    #[default]
    Barrel,
    Crate,
    Lantern,
    Campfire,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterClass {
    #[default]
    Villager,
    Warrior,
    Mage,
    Rogue,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerrainShape {
    #[default]
    Hills,
    Mountains,
    Island,
    Archipelago,
    Canyon,
    Mesa,
    Crater,
    Valley,
    Dunes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TerrainParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_terrain_size")]
    pub size: f32,
    #[serde(default = "d_terrain_res")]
    pub resolution: u32,
    #[serde(default = "d_one")]
    pub mountainousness: f32,
    /// 0 disables water; fraction of the height range that floods.
    #[serde(default = "d_water")]
    pub water_level: f32,
    #[serde(default = "d_true")]
    pub scatter: bool,
    /// Macro-shape mask applied to the heightfield.
    #[serde(default)]
    pub shape: TerrainShape,
    /// World-space chunk offset: adjacent chunks with matching offsets tile
    /// seamlessly (noise is sampled in world coordinates).
    #[serde(default)]
    pub offset_x: f32,
    #[serde(default)]
    pub offset_z: f32,
    /// Quantize heights into steps (0 = off). ~6-12 gives stepped mesas.
    #[serde(default)]
    pub terrace: f32,
    /// Diorama side walls + bottom. Turn OFF for tiled world chunks.
    #[serde(default = "d_true")]
    pub skirt: bool,
    /// Hydraulic erosion strength 0..1 (deterministic droplet simulation).
    /// Chunk-local — do not combine with seamless tiling.
    #[serde(default)]
    pub erosion: f32,
    /// Number of rivers traced downhill from high springs (carved channel
    /// + water ribbon). Chunk-local like erosion.
    #[serde(default)]
    pub rivers: u32,
    /// Dirt paths/roads: splines flattened into the terrain.
    #[serde(default)]
    pub paths: Vec<PathSpec>,
    /// Optional baked texture over the terrain (e.g. rock strata on cliffs).
    #[serde(default)]
    pub texture: Option<crate::texture::TextureSpec>,
    /// Scatter density multiplier (1.0 = default coverage).
    #[serde(default = "d_one")]
    pub scatter_density: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PathSpec {
    /// Waypoints in local chunk XZ coordinates.
    pub points: Vec<[f32; 2]>,
    #[serde(default = "d_path_w")]
    pub width: f32,
}
fn d_path_w() -> f32 {
    2.0
}

fn d_terrain_size() -> f32 {
    48.0
}
fn d_terrain_res() -> u32 {
    110
}
fn d_one() -> f32 {
    1.0
}
fn d_water() -> f32 {
    0.28
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TreeParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub style: TreeStyle,
    #[serde(default = "d_tree_h")]
    pub height: f32,
}
fn d_tree_h() -> f32 {
    6.0
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RockParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_one")]
    pub size: f32,
    #[serde(default = "d_jag")]
    pub jaggedness: f32,
}
fn d_jag() -> f32 {
    0.6
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CrystalParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_one")]
    pub size: f32,
    #[serde(default = "d_crystal_count")]
    pub count: u32,
}
fn d_crystal_count() -> u32 {
    7
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildingParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_bwidth")]
    pub width: f32,
    #[serde(default = "d_floors")]
    pub floors: u32,
}
fn d_bwidth() -> f32 {
    6.0
}
fn d_floors() -> u32 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PropParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub prop: PropKind,
    #[serde(default = "d_one")]
    pub size: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CharacterParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub class: CharacterClass,
    #[serde(default = "d_char_h")]
    pub height: f32,
    #[serde(default = "d_one")]
    pub bulk: f32,
    #[serde(default = "d_true")]
    pub animate: bool,
    /// short | ponytail | bun | bald | long | topknot (default: seeded pick)
    #[serde(default)]
    pub hair: Option<String>,
    /// none | mustache | short | long — ribbon-card facial hair.
    #[serde(default)]
    pub beard: Option<String>,
    /// Override hair/beard color (#rrggbb), e.g. "#e8e6e0" for elders.
    #[serde(default)]
    pub hair_color: Option<String>,
    /// 0..=3 light→dark (default: seeded pick)
    #[serde(default)]
    pub skin_tone: Option<u32>,
    /// Export facial morph targets (smile, blink, angry, surprised).
    #[serde(default = "d_true")]
    pub expressions: bool,
    /// Painted garment stack: robe (layered under-robe + open coat + sash +
    /// mantle) | tunic (belted knee tunic) | plain (bare v2 body).
    #[serde(default)]
    pub outfit: Option<String>,
    /// 0..1 — how much painted trim/motif detail garments get.
    #[serde(default = "d_ornament")]
    pub ornamentation: f32,
    /// Trim motif for garment borders: meander|zigzag|dots|diamonds|scroll|runes.
    #[serde(default)]
    pub trim_motif: Option<String>,
    /// 0..1 — painted age detail on the face (forehead lines, crow's feet,
    /// nasolabial folds).
    #[serde(default)]
    pub age: f32,
    /// Extra props: necklace | belt_knot | staff.
    #[serde(default)]
    pub accessories: Vec<String>,
    /// Tessellation multiplier 0.5..2.0 — 2.0 doubles segment counts and
    /// subdivision for hero-quality close-ups.
    #[serde(default = "d_one")]
    pub detail: f32,
}
fn d_ornament() -> f32 {
    0.6
}
fn d_char_h() -> f32 {
    1.7
}

fn d_zero() -> f32 {
    0.0
}
/// Sentinel meaning "let the body plan / class decide" for optional f32 knobs.
fn d_neg1() -> f32 {
    -1.0
}
/// Sentinel meaning "let the body plan / class decide" for the eye count.
fn d_neg1_i32() -> i32 {
    -1
}

/// Monster body plan — the skeleton template that drives limb count/placement,
/// gait, and collider shape.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BodyPlan {
    BipedBrute,
    #[default]
    QuadrupedBeast,
    #[serde(alias = "wyrm")]
    Serpent,
    Arachnid,
    WingedFlyer,
    #[serde(alias = "blob")]
    Ooze,
    Insectoid,
    Aberration,
}

/// Preset bundle over the monster knobs, like character `class`. `None` leaves
/// every knob at its plan default.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MonsterClass {
    #[default]
    None,
    Predator,
    Brute,
    Elemental,
    Undead,
    Aberration,
    Swarm,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MonsterParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    /// One of the 8 body plans (accepts `species` as an alias).
    #[serde(default, alias = "species")]
    pub body: BodyPlan,
    /// Preset bundle applied before the plan builds (explicit knobs still win).
    #[serde(default)]
    pub class: MonsterClass,
    /// Overall scale — drives geometry, collider size, and mass (accepts `bulk`).
    #[serde(default = "d_one", alias = "bulk")]
    pub size: f32,
    /// 0..1 — horn prominence.
    #[serde(default = "d_zero")]
    pub horns: f32,
    /// 0..1 — dorsal spike ridge.
    #[serde(default = "d_zero")]
    pub spikes: f32,
    /// 0..1 — armor plating.
    #[serde(default = "d_zero")]
    pub plates: f32,
    /// 0..1 tail length; `-1` = plan default, `0` disables.
    #[serde(default = "d_neg1")]
    pub tail: f32,
    /// 0..1 wing size; `-1` = plan default, `0` disables.
    #[serde(default = "d_neg1")]
    pub wings: f32,
    /// Eye count; `-1` = plan/class default.
    #[serde(default = "d_neg1_i32")]
    pub eyes: i32,
    /// 0..1 jaw/teeth prominence; `-1` = plan default.
    #[serde(default = "d_neg1")]
    pub maw: f32,
    /// 0..1 — proportion slider (heavier, more threatening build).
    #[serde(default = "d_zero")]
    pub menace: f32,
    /// 0..1 — wear/erosion detail (ancient, undead reads).
    #[serde(default = "d_zero")]
    pub age: f32,
    /// 0..1 fraction of the body lit with the palette accent as emissive
    /// markings; `-1` = class default.
    #[serde(default = "d_neg1")]
    pub emissive: f32,
    /// Tessellation multiplier 0.5..2.0.
    #[serde(default = "d_one")]
    pub detail: f32,
    #[serde(default = "d_true")]
    pub animate: bool,
}

impl Default for MonsterParams {
    /// Serde defaults for every field (body = QuadrupedBeast, etc.).
    fn default() -> Self {
        serde_json::from_str("{}").expect("monster defaults deserialize")
    }
}

/// Boss archetype — the encounter template driving multi-part, multi-phase
/// geometry (built in Task 4+).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BossArchetype {
    #[default]
    Hydra,
    Colossus,
    Lich,
    SwarmQueen,
    DragonLord,
}

/// Boss elemental theme — each arm maps to one of the nine real palettes via
/// `element_palette`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BossElement {
    #[default]
    Infernal,
    Necrotic,
    Fungal,
    Arctic,
    Volcanic,
    Verdant,
    Autumn,
    Desert,
    Mystic,
}

/// Palette an element maps to (used only when the recipe left the default
/// palette). Every arm MUST be a name in `palette::PALETTES`
/// (verdant, autumn, arctic, volcanic, desert, mystic, necrotic, infernal, fungal).
pub(crate) fn element_palette(e: BossElement) -> &'static str {
    match e {
        BossElement::Infernal => "infernal",
        BossElement::Necrotic => "necrotic",
        BossElement::Fungal => "fungal",
        BossElement::Arctic => "arctic",
        BossElement::Volcanic => "volcanic",
        BossElement::Verdant => "verdant",
        BossElement::Autumn => "autumn",
        BossElement::Desert => "desert",
        BossElement::Mystic => "mystic",
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BossParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub archetype: BossArchetype,
    #[serde(default)]
    pub element: BossElement,
    /// Overall scale (default LARGE, per-archetype). Alias `bulk`.
    #[serde(default = "d_boss_size", alias = "bulk")]
    pub size: f32,
    /// Number of baked phase metadata blocks (clamp 1..=4).
    #[serde(default = "d_two_u32")]
    pub phases: u32,
    /// Reserved: optional single-phase geometry selector. Not yet
    /// implemented — per-phase geometry regeneration is deferred; the
    /// baked `phase_transition` clip + `phases` metadata cover phase
    /// changes today.
    #[serde(default)]
    pub phase: Option<u32>,
    #[serde(default = "d_true")]
    pub weak_points: bool,
    /// 0..1 armor escalation; `-1` = archetype default.
    #[serde(default = "d_neg1")]
    pub armor: f32,
    #[serde(default = "d_neg1")]
    pub plates: f32,
    #[serde(default = "d_neg1")]
    pub crown: f32,
    #[serde(default = "d_neg1")]
    pub regalia: f32,
    #[serde(default = "d_neg1")]
    pub horns: f32,
    #[serde(default = "d_neg1")]
    pub spikes: f32,
    #[serde(default = "d_neg1_i32")]
    pub eyes: i32,
    #[serde(default = "d_neg1")]
    pub maw: f32,
    #[serde(default = "d_neg1")]
    pub wings: f32,
    #[serde(default = "d_neg1")]
    pub tail: f32,
    #[serde(default = "d_neg1")]
    pub menace: f32,
    #[serde(default = "d_neg1")]
    pub emissive: f32,
    /// Hero tessellation default.
    #[serde(default = "d_hero_detail")]
    pub detail: f32,
    #[serde(default = "d_true")]
    pub animate: bool,
}

fn d_boss_size() -> f32 {
    3.0
}
fn d_two_u32() -> u32 {
    2
}
fn d_hero_detail() -> f32 {
    1.3
}

impl Default for BossParams {
    fn default() -> Self {
        serde_json::from_str("{}").expect("boss defaults deserialize")
    }
}

/// Dungeon theme — palette + wall material + prop set + shape bias
/// (orthogonal rooms vs. organic caves).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DungeonTheme {
    #[default]
    Crypt,
    Cavern,
    Sewer,
    Mine,
    Temple,
    Fortress,
}

/// Target dungeon extent (drives room count / footprint).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DungeonSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DungeonParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    /// One of the 6 themes (JSON key is `type`).
    #[serde(default, rename = "type")]
    pub theme: DungeonTheme,
    #[serde(default)]
    pub size: DungeonSize,
    /// Explicit room cap (overrides the size-derived count when set).
    #[serde(default)]
    pub rooms: Option<u32>,
    /// 0..1 — extra corridor edges beyond the spanning tree (loopiness).
    #[serde(default = "d_loops")]
    pub loops: f32,
    /// 0..1 — how much dressing (props) rooms receive.
    #[serde(default = "d_density")]
    pub density: f32,
    /// Tessellation multiplier 0.5..2.0.
    #[serde(default = "d_one")]
    pub detail: f32,
    /// Optional inline boss: when set, the boss room's `Boss` spawn also
    /// emits+places a scaled `boss.glb` and references it in the manifest.
    #[serde(default)]
    pub boss: Option<BossParams>,
}
fn d_loops() -> f32 {
    0.3
}
fn d_density() -> f32 {
    0.5
}

impl Default for DungeonParams {
    fn default() -> Self {
        serde_json::from_str("{}").expect("dungeon defaults deserialize")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recipe {
    Terrain {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: TerrainParams,
    },
    Tree {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: TreeParams,
    },
    Rock {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: RockParams,
    },
    Crystal {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: CrystalParams,
    },
    Building {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: BuildingParams,
    },
    Prop {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: PropParams,
    },
    Character {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: CharacterParams,
    },
    Monster {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: MonsterParams,
    },
    Dungeon {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: DungeonParams,
    },
    Boss {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: BossParams,
    },
    /// Fully generic declarative geometry DSL — build anything.
    Custom {
        #[serde(flatten)]
        params: crate::generators::custom::CustomParams,
    },
}

/// A monster class's preferred palette, used only when the recipe left the
/// default palette. Keeps the class read cohesive (glowing infernal elementals,
/// necrotic undead, fungal aberrations) without a palette field on the params.
fn preferred_palette(class: MonsterClass) -> Option<&'static str> {
    match class {
        MonsterClass::Elemental => Some("infernal"),
        MonsterClass::Undead => Some("necrotic"),
        MonsterClass::Aberration => Some("fungal"),
        _ => None,
    }
}

/// A dungeon theme's default palette, used only when the recipe left the
/// default palette (an explicit palette always wins).
fn theme_palette(theme: DungeonTheme) -> &'static str {
    match theme {
        DungeonTheme::Crypt => "necrotic",
        DungeonTheme::Cavern => "fungal",
        DungeonTheme::Sewer => "fungal",
        DungeonTheme::Mine => "volcanic",
        DungeonTheme::Temple => "mystic",
        DungeonTheme::Fortress => "volcanic",
    }
}

impl Recipe {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid recipe: {e}"))
    }

    pub fn palette_name(&self) -> &str {
        match self {
            Recipe::Terrain { palette, .. }
            | Recipe::Tree { palette, .. }
            | Recipe::Rock { palette, .. }
            | Recipe::Crystal { palette, .. }
            | Recipe::Building { palette, .. }
            | Recipe::Prop { palette, .. }
            | Recipe::Character { palette, .. }
            | Recipe::Monster { palette, .. }
            | Recipe::Dungeon { palette, .. }
            | Recipe::Boss { palette, .. } => palette,
            Recipe::Custom { .. } => "verdant",
        }
    }

    /// The palette actually used to build this recipe. Usually the recipe's
    /// own `palette`, but a monster class or dungeon theme substitutes a
    /// thematic default when the user left the palette unset — an explicit
    /// palette always wins. Exposed so the CLI's dungeon path resolves the
    /// palette exactly as `build` does.
    pub fn resolved_palette(&self) -> &str {
        // A monster class carries a preferred palette (Elemental->infernal,
        // Undead->necrotic, Aberration->fungal); a dungeon theme carries one
        // too. Substitute it ONLY when the user left the default palette.
        match self {
            Recipe::Monster { palette, params } if *palette == d_palette() => {
                preferred_palette(params.class).unwrap_or(palette.as_str())
            }
            Recipe::Dungeon { palette, params } if *palette == d_palette() => {
                theme_palette(params.theme)
            }
            Recipe::Boss { palette, params } if *palette == d_palette() => {
                element_palette(params.element)
            }
            _ => self.palette_name(),
        }
    }

    /// Compile the recipe into an asset.
    pub fn build(&self) -> Result<Asset, String> {
        if !palette::PALETTES.contains(&self.palette_name()) {
            return Err(format!(
                "unknown palette '{}' (available: {})",
                self.palette_name(),
                palette::PALETTES.join(", ")
            ));
        }
        let pal_name: &str = self.resolved_palette();
        let pal = palette::by_name(pal_name);
        let asset = match self {
            Recipe::Terrain { params, .. } => crate::generators::terrain::generate(params, &pal),
            Recipe::Tree { params, .. } => crate::generators::tree::generate(params, &pal),
            Recipe::Rock { params, .. } => crate::generators::rock::generate(params, &pal),
            Recipe::Crystal { params, .. } => crate::generators::crystal::generate(params, &pal),
            Recipe::Building { params, .. } => crate::generators::building::generate(params, &pal),
            Recipe::Prop { params, .. } => crate::generators::prop::generate(params, &pal),
            Recipe::Character { params, .. } => {
                crate::generators::character::generate(params, &pal)
            }
            Recipe::Monster { params, .. } => crate::generators::monster::generate(params, &pal),
            Recipe::Dungeon { params, .. } => crate::generators::dungeon::generate(params, &pal)?,
            Recipe::Boss { params, .. } => crate::generators::boss::generate(params, &pal),
            Recipe::Custom { params } => crate::generators::custom::generate(params)?,
        };
        asset.validate()?;
        Ok(asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_recipes_parse_and_build() {
        for kind in ["tree", "rock", "crystal", "building", "prop", "character"] {
            let r = Recipe::parse(&format!("{{\"kind\": \"{kind}\"}}")).unwrap();
            let a = r.build().unwrap();
            assert!(
                a.parts
                    .iter()
                    .map(|p| p.mesh.triangle_count())
                    .sum::<usize>()
                    > 0
            );
        }
    }

    #[test]
    fn bad_palette_rejected() {
        let r = Recipe::parse(r#"{"kind": "tree", "palette": "nope"}"#).unwrap();
        assert!(r.build().is_err());
    }

    #[test]
    fn boss_minimal_parses_and_builds() {
        let r =
            Recipe::parse(r#"{"kind":"boss","archetype":"hydra","element":"infernal"}"#).unwrap();
        assert_eq!(r.resolved_palette(), "infernal");
        let asset = r.build().unwrap();
        assert!(!asset.parts.is_empty());
    }

    #[test]
    fn boss_element_drives_palette_only_when_default() {
        // explicit palette wins
        let r =
            Recipe::parse(r#"{"kind":"boss","element":"infernal","palette":"arctic"}"#).unwrap();
        assert_eq!(r.resolved_palette(), "arctic");
        // element substitutes when palette left default
        let r = Recipe::parse(r#"{"kind":"boss","element":"necrotic"}"#).unwrap();
        assert_eq!(r.resolved_palette(), "necrotic");
    }

    #[test]
    fn custom_dsl_builds_anything() {
        let j = r##"{"kind":"custom","name":"totem",
          "physics":{"collider":"auto","mass":0},
          "bones":[{"name":"root"},{"name":"top","parent":"root","translation":[0,2,0]}],
          "animations":[{"name":"spin","duration":2,
            "channels":[{"bone":"top","path":"rotation","axis":[0,1,0],"keys":[0,360]}]}],
          "parts":[{"material":{"roughness":0.7},
            "nodes":[
              {"shape":"lathe","profile":[[0.5,0],[0.4,2]],"color":"#886644"},
              {"shape":"sphere","radius":0.4,"color":[0.8,0.2,0.2],"bone":"top",
               "transform":{"translate":[0,2.3,0]},"displace":{"amplitude":0.05}},
              {"shape":"box","size":[0.2,0.2,0.2],"color":"#ffffff",
               "repeat":{"count":6,"radius":1.0,"orient":true}}]}]}"##;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        assert_eq!(a.animations.len(), 1);
        assert!(a.skeleton.is_some());
        assert!(a.parts[0].mesh.triangle_count() > 50);
        // regression: node without explicit transform must keep scale 1
        let (lo, hi) = a.parts[0].mesh.bounds();
        assert!((hi.y - lo.y) > 1.5, "lathe collapsed: {lo:?} {hi:?}");
    }

    #[test]
    fn dsl_geometry_v2() {
        // csg subtract carves, bevel/subdivide/curve all build valid meshes
        let j = r##"{"kind":"custom","name":"geo","parts":[{"nodes":[
          {"shape":"box","size":[2,2,2],"color":"#888888","bevel":0.2,
           "csg":[{"op":"subtract","shape":"cylinder","radius":0.6,"height":3,
                   "color":"#888888","transform":{"translate":[0,-1.5,0]}}]},
          {"shape":"sphere","radius":0.5,"subdiv":1,"color":"#ffffff",
           "subdivide":1,"smooth":true,"flat":false},
          {"shape":"curve","points":[[2,0,0],[2.5,1,0],[2,2,0.5]],
           "radius":[0.2,0.1],"samples":12,"color":"#aa6644"}
        ]}]}"##;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        a.validate().unwrap();
        let m = &a.parts[0].mesh;
        assert!(m.triangle_count() > 100);
        // the cylinder bored a hole through the box floor: vertices exist
        // on the cylinder wall inside the box
        let wall = m
            .positions
            .iter()
            .any(|p| (p.x * p.x + p.z * p.z).sqrt() < 0.65 && p.y.abs() < 0.9);
        assert!(wall, "expected carved cylinder wall");
        // bad csg op rejected
        let bad = j.replace("\"op\":\"subtract\"", "\"op\":\"xor\"");
        assert!(Recipe::parse(&bad).unwrap().build().is_err());
    }

    #[test]
    fn dsl_easing_and_euler_keys() {
        let j = r##"{"kind":"custom","name":"nod",
          "bones":[{"name":"root"},{"name":"top","parent":"root","translation":[0,1,0]}],
          "animations":[{"name":"nod","duration":1,
            "channels":[{"bone":"top","path":"rotation","ease":"cubic_in_out",
                         "keys_euler":[[0,0,0],[30,45,0],[0,0,0]]}]}],
          "parts":[{"nodes":[{"shape":"box","size":[0.5,0.5,0.5],"color":"#ffffff","bone":"top"}]}]}"##;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        let ch = &a.animations[0].channels[0];
        // easing bakes to dense keys
        assert!(ch.times.len() > 3);
        match &ch.data {
            crate::gltf::ChannelData::Rotation(qs) => {
                assert_eq!(qs.len(), ch.times.len());
                // multi-axis euler key produces a non-single-axis quaternion mid-clip
                let mid = qs[qs.len() / 2];
                assert!(mid.x.abs() > 0.01 && mid.y.abs() > 0.01);
            }
            _ => panic!("expected rotation channel"),
        }
        // bad ease rejected
        let bad = j.replace("cubic_in_out", "bounce");
        assert!(Recipe::parse(&bad).unwrap().build().is_err());
    }

    #[test]
    fn character_v2_features() {
        let a = Recipe::parse(r#"{"kind":"character","seed":4,"hair":"ponytail"}"#)
            .unwrap()
            .build()
            .unwrap();
        let names: Vec<&str> = a.parts[0]
            .mesh
            .morphs
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        for e in ["smile", "blink", "angry", "surprised"] {
            assert!(names.contains(&e), "missing morph {e}");
        }
        // expressions off removes morphs
        let b = Recipe::parse(r#"{"kind":"character","seed":4,"expressions":false}"#)
            .unwrap()
            .build()
            .unwrap();
        assert!(b.parts[0].mesh.morphs.is_empty());
        // hair styles change geometry
        let bald = Recipe::parse(r#"{"kind":"character","seed":4,"hair":"bald"}"#)
            .unwrap()
            .build()
            .unwrap();
        assert!(
            a.parts[0].mesh.vertex_count() > bald.parts[0].mesh.vertex_count(),
            "ponytail should add geometry over bald"
        );
    }

    #[test]
    fn hair_and_beard_cards() {
        let base = Recipe::parse(r#"{"kind":"character","seed":4,"hair":"bald"}"#)
            .unwrap()
            .build()
            .unwrap();
        let long = Recipe::parse(
            r##"{"kind":"character","seed":4,"hair":"long","beard":"long","hair_color":"#eae7e0"}"##,
        )
        .unwrap()
        .build()
        .unwrap();
        assert!(
            long.parts[0].mesh.vertex_count() > base.parts[0].mesh.vertex_count() + 200,
            "ribbon cards should add real geometry"
        );
        // determinism with cards
        let again = Recipe::parse(
            r##"{"kind":"character","seed":4,"hair":"long","beard":"long","hair_color":"#eae7e0"}"##,
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(crate::gltf::to_glb(&long), crate::gltf::to_glb(&again));
    }

    #[test]
    fn accessories_and_ao() {
        let j = r#"{"kind":"character","seed":3,"accessories":["necklace","staff","belt_knot"]}"#;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        a.validate().unwrap();
        let bare = Recipe::parse(r#"{"kind":"character","seed":3}"#)
            .unwrap()
            .build()
            .unwrap();
        assert!(
            a.parts[0].mesh.vertex_count() > bare.parts[0].mesh.vertex_count() + 100,
            "accessories should add geometry"
        );
        // staff reaches above the head
        let (_, hi) = a.parts[0].mesh.bounds();
        let (_, bare_hi) = bare.parts[0].mesh.bounds();
        assert!(
            hi.y > bare_hi.y + 0.05,
            "staff orb should top the silhouette"
        );
        // AO darkened somewhere without blowing out colors
        assert!(
            a.parts[0]
                .mesh
                .colors
                .iter()
                .all(|c| c.max_element() <= 4.0)
        );
        let b = Recipe::parse(j).unwrap().build().unwrap();
        assert_eq!(crate::gltf::to_glb(&a), crate::gltf::to_glb(&b));
    }

    #[test]
    fn character_outfits() {
        let j = r#"{"kind":"character","seed":9,"outfit":"robe","ornamentation":0.7}"#;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        a.validate().unwrap();
        // body + under-robe + coat + 2 sleeves + sash + tail + mantle
        assert!(
            a.parts.len() >= 7,
            "robe outfit should add garment parts: {}",
            a.parts.len()
        );
        // garments carry painted textures and skin weights
        let dressed_parts = a
            .parts
            .iter()
            .filter(|p| p.material.texture.is_some())
            .count();
        assert!(dressed_parts >= 5);
        assert!(a.parts[1].mesh.is_skinned());
        // deterministic incl. baked garment paint
        let b = Recipe::parse(j).unwrap().build().unwrap();
        assert_eq!(crate::gltf::to_glb(&a), crate::gltf::to_glb(&b));
        // plain = body + painted-face head only
        let p = Recipe::parse(r#"{"kind":"character","seed":9}"#)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(p.parts.len(), 2);
    }

    #[test]
    fn character_ships_clip_library() {
        let a = Recipe::parse(r#"{"kind":"character","seed":2}"#)
            .unwrap()
            .build()
            .unwrap();
        let names: Vec<&str> = a.animations.iter().map(|c| c.name.as_str()).collect();
        for expected in [
            "idle", "walk", "run", "attack", "sit", "wave", "death", "dance",
        ] {
            assert!(names.contains(&expected), "missing clip {expected}");
        }
        // posing at mid-clip moves vertices
        let posed = crate::anim::pose_asset(&a, "walk", 0.25).unwrap();
        let moved = posed.parts[0]
            .mesh
            .positions
            .iter()
            .zip(&a.parts[0].mesh.positions)
            .any(|(p, q)| p.distance(*q) > 0.01);
        assert!(moved, "walk pose should deform the mesh");
    }

    #[test]
    fn terrain_tiles_seamlessly() {
        let mk = |ox: f32| {
            let j = format!(
                concat!(
                    "{{\"kind\":\"terrain\",\"seed\":5,\"size\":16,\"resolution\":32,",
                    "\"scatter\":false,\"skirt\":false,\"water_level\":0,\"offset_x\":{}}}"
                ),
                ox
            );
            Recipe::parse(&j).unwrap().build().unwrap()
        };
        let a = mk(0.0);
        let b = mk(16.0);
        let edge = |asset: &crate::gltf::Asset, x: f32| -> Vec<(i32, i32)> {
            let mut v: Vec<(i32, i32)> = asset.parts[0]
                .mesh
                .positions
                .iter()
                .filter(|p| (p.x - x).abs() < 1e-4)
                .map(|p| ((p.z * 1000.0) as i32, (p.y * 1000.0) as i32))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        assert_eq!(edge(&a, 8.0), edge(&b, -8.0));
    }

    #[test]
    fn terrain_v3_features() {
        let j = r#"{"kind":"terrain","seed":7,"size":24,"resolution":48,"erosion":0.6,
            "rivers":1,"water_level":0.1,
            "paths":[{"points":[[-10,-10],[0,0],[10,10]],"width":2.0}]}"#;
        let a = Recipe::parse(j).unwrap().build().unwrap();
        a.validate().unwrap();
        // deterministic (erosion + rivers + paths + instanced scatter)
        let b = Recipe::parse(j).unwrap().build().unwrap();
        assert_eq!(
            crate::gltf::to_glb(&a),
            crate::gltf::to_glb(&b),
            "terrain v3 must stay byte-deterministic"
        );
        // scatter exports as GPU instances
        assert!(!a.instanced.is_empty());
        let glb = crate::gltf::to_glb(&a);
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let exts = doc["extensionsUsed"].as_array().unwrap();
        assert!(exts.iter().any(|e| e == "EXT_mesh_gpu_instancing"));
        let inst_node = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["extensions"]["EXT_mesh_gpu_instancing"].is_object())
            .expect("instanced node");
        let attrs = &inst_node["extensions"]["EXT_mesh_gpu_instancing"]["attributes"];
        for k in ["TRANSLATION", "ROTATION", "SCALE"] {
            let acc = attrs[k].as_u64().unwrap() as usize;
            assert!(doc["accessors"][acc]["count"].as_u64().unwrap() > 0);
        }
        // erosion actually changes the heightfield
        let flat = Recipe::parse(&j.replace("\"erosion\":0.6", "\"erosion\":0.0"))
            .unwrap()
            .build()
            .unwrap();
        assert!(
            !(a.parts[0].mesh.positions.len() == flat.parts[0].mesh.positions.len()
                && a.parts[0].mesh.positions == flat.parts[0].mesh.positions),
            "erosion should alter geometry"
        );
    }

    #[test]
    fn deterministic_build() {
        let j = r#"{"kind": "tree", "seed": 9, "style": "oak"}"#;
        let a = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        let b = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn monster_parses_and_builds() {
        let r =
            Recipe::parse(r#"{"kind":"monster","species":"wyrm","palette":"infernal"}"#).unwrap();
        let asset = r.build().expect("monster builds");
        assert!(!asset.parts.is_empty());
        assert!(asset.physics.is_some());
        // aliases resolve: body/serpent and bulk
        Recipe::parse(r#"{"kind":"monster","body":"serpent","bulk":1.5}"#)
            .unwrap()
            .build()
            .unwrap();
        // blob alias for ooze
        Recipe::parse(r#"{"kind":"monster","body":"blob"}"#)
            .unwrap()
            .build()
            .unwrap();
    }

    #[test]
    fn monster_is_deterministic() {
        let json = r#"{"kind":"monster","body":"wyrm","seed":7,"class":"elemental"}"#;
        let a = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
        let b = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn boss_is_deterministic() {
        let json = r#"{"kind":"boss","archetype":"hydra","element":"infernal","seed":3}"#;
        let a = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
        let b = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn dungeon_parses_and_builds_single_glb() {
        let r = Recipe::parse(r#"{"kind":"dungeon","type":"crypt","rooms":1}"#).unwrap();
        let asset = r.build().expect("1-room dungeon builds a single asset");
        assert!(!asset.parts.is_empty());
        assert!(asset.physics.is_some());
        // theme default palette applies (crypt -> necrotic) without erroring
        Recipe::parse(r#"{"kind":"dungeon","type":"cavern"}"#)
            .unwrap()
            .build()
            .unwrap();
        // explicit palette still honored
        Recipe::parse(r#"{"kind":"dungeon","type":"mine","palette":"arctic"}"#)
            .unwrap()
            .build()
            .unwrap();
    }

    #[test]
    fn dungeon_is_deterministic() {
        for json in [
            r#"{"kind":"dungeon","type":"cavern","seed":9}"#,
            r#"{"kind":"dungeon","type":"crypt","size":"medium","seed":9}"#,
        ] {
            let a = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
            let b = crate::gltf::to_glb(&Recipe::parse(json).unwrap().build().unwrap());
            assert_eq!(a, b, "dungeon output must be byte-identical: {json}");
        }
    }

    #[test]
    fn dungeon_survives_hostile_input() {
        for json in [
            r#"{"kind":"dungeon","loops":1e9,"density":-5.0,"rooms":100000000}"#,
            r#"{"kind":"dungeon","type":"cavern","detail":1e30,"rooms":100000000}"#,
            r#"{"kind":"dungeon","type":"fortress","loops":-9.0,"density":50.0}"#,
        ] {
            Recipe::parse(json).unwrap().build().unwrap();
        }
    }

    #[test]
    fn every_dungeon_theme_and_size_builds() {
        for theme in ["crypt", "cavern", "sewer", "mine", "temple", "fortress"] {
            for size in ["small", "medium", "large"] {
                let j = format!(r#"{{"kind":"dungeon","type":"{theme}","size":"{size}"}}"#);
                Recipe::parse(&j).unwrap().build().unwrap();
            }
        }
    }

    #[test]
    fn monster_survives_hostile_input() {
        // absurd numeric values must clamp, not panic or explode the grid
        for json in [
            r#"{"kind":"monster","size":1e30,"detail":1e30,"horns":-5.0,"eyes":999999}"#,
            r#"{"kind":"monster","body":"arachnid","size":-1.0,"maw":1e30,"spikes":-9.0}"#,
            r#"{"kind":"monster","body":"aberration","class":"aberration","emissive":50.0}"#,
        ] {
            Recipe::parse(json).unwrap().build().unwrap();
        }
    }

    #[test]
    fn every_body_plan_and_class_builds() {
        for body in [
            "biped_brute",
            "quadruped_beast",
            "serpent",
            "arachnid",
            "winged_flyer",
            "ooze",
            "insectoid",
            "aberration",
        ] {
            let j = format!(r#"{{"kind":"monster","body":"{body}"}}"#);
            Recipe::parse(&j).unwrap().build().unwrap();
        }
        for class in [
            "none",
            "predator",
            "brute",
            "elemental",
            "undead",
            "aberration",
            "swarm",
        ] {
            let j = format!(r#"{{"kind":"monster","class":"{class}"}}"#);
            Recipe::parse(&j).unwrap().build().unwrap();
        }
    }
}
