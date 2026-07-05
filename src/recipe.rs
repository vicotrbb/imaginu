//! Recipe schema — the JSON surface an AI agent writes. Small, forgiving
//! (everything except `kind` has a default), deterministic via `seed`.

use serde::{Deserialize, Serialize};

use crate::gltf::Asset;
use crate::palette;

fn d_seed() -> u64 { 1 }
fn d_palette() -> String { "verdant".into() }
fn d_true() -> bool { true }
/// exposed for the custom-DSL serde defaults
pub fn d_true_pub() -> bool { true }

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
    /// Fully generic declarative geometry DSL — build anything.
    Custom {
        #[serde(flatten)]
        params: crate::generators::custom::CustomParams,
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
            Recipe::Custom { .. } => "verdant",
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
            assert!(a.parts.iter().map(|p| p.mesh.triangle_count()).sum::<usize>() > 0);
        }
    }

    #[test]
    fn bad_palette_rejected() {
        let r = Recipe::parse(r#"{"kind": "tree", "palette": "nope"}"#).unwrap();
        assert!(r.build().is_err());
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
    fn character_ships_clip_library() {
        let a = Recipe::parse(r#"{"kind":"character","seed":2}"#).unwrap().build().unwrap();
        let names: Vec<&str> = a.animations.iter().map(|c| c.name.as_str()).collect();
        for expected in ["idle", "walk", "run", "attack", "sit", "wave", "death", "dance"] {
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
    fn deterministic_build() {
        let j = r#"{"kind": "tree", "seed": 9, "style": "oak"}"#;
        let a = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        let b = crate::gltf::to_glb(&Recipe::parse(j).unwrap().build().unwrap());
        assert_eq!(a, b);
    }
}
