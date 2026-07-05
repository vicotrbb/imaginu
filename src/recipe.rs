//! Recipe schema — the JSON surface an AI agent writes. Small, forgiving
//! (everything except `kind` has a default), deterministic via `seed`.

use serde::{Deserialize, Serialize};

use crate::gltf::Asset;
use crate::palette;

fn d_seed() -> u64 { 1 }
fn d_palette() -> String { "verdant".into() }
fn d_true() -> bool { true }

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
}
fn d_terrain_size() -> f32 { 48.0 }
fn d_terrain_res() -> u32 { 110 }
fn d_one() -> f32 { 1.0 }
fn d_water() -> f32 { 0.28 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TreeParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub style: TreeStyle,
    #[serde(default = "d_tree_h")]
    pub height: f32,
}
fn d_tree_h() -> f32 { 6.0 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RockParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_one")]
    pub size: f32,
    #[serde(default = "d_jag")]
    pub jaggedness: f32,
}
fn d_jag() -> f32 { 0.6 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CrystalParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_one")]
    pub size: f32,
    #[serde(default = "d_crystal_count")]
    pub count: u32,
}
fn d_crystal_count() -> u32 { 7 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildingParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default = "d_bwidth")]
    pub width: f32,
    #[serde(default = "d_floors")]
    pub floors: u32,
}
fn d_bwidth() -> f32 { 6.0 }
fn d_floors() -> u32 { 1 }

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
}
fn d_char_h() -> f32 { 1.7 }

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
            | Recipe::Character { palette, .. } => palette,
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
        let pal = palette::by_name(self.palette_name());
        let asset = match self {
            Recipe::Terrain { params, .. } => crate::generators::terrain::generate(params, &pal),
            Recipe::Tree { params, .. } => crate::generators::tree::generate(params, &pal),
            Recipe::Rock { params, .. } => crate::generators::rock::generate(params, &pal),
            Recipe::Crystal { params, .. } => crate::generators::crystal::generate(params, &pal),
            Recipe::Building { params, .. } => crate::generators::building::generate(params, &pal),
            Recipe::Prop { params, .. } => crate::generators::prop::generate(params, &pal),
            Recipe::Character { params, .. } => crate::generators::character::generate(params, &pal),
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
            assert!(a.parts.iter().map(|p| p.mesh.triangle_count()).sum::<usize>() > 0);
        }
    }

    #[test]
    fn bad_palette_rejected() {
        let r = Recipe::parse(r#"{"kind": "tree", "palette": "nope"}"#).unwrap();
        assert!(r.build().is_err());
    }

    #[test]
    fn deterministic_build() {
        let j = r#"{"kind": "tree", "seed": 9, "style": "oak"}"#;
        let a = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        let b = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        assert_eq!(a, b);
    }
}
