//! imaginu — AI-drivable procedural 3D asset compiler.
//!
//! JSON recipes → deterministic, game-ready GLB (meshes, PBR vertex colors,
//! skeletal animation, physics metadata) for Babylon.js, plus a built-in
//! software renderer for visual verification.
//!
//! # Quick start
//!
//! ```
//! // Compile a recipe straight to GLB bytes.
//! let glb = imaginu::compile_to_glb(r#"{"kind":"tree","style":"oak"}"#).unwrap();
//! assert_eq!(&glb[0..4], b"glTF");
//! ```
//!
//! Compilation is **deterministic**: the same recipe (and seed) always yields
//! byte-identical output, on every platform. Malformed or hostile recipe JSON
//! returns an [`Error`] rather than panicking — see the [`error`] module.

pub mod anim;
pub mod csg;
pub mod error;
pub mod generators;
pub mod gltf;
pub mod mesh;
pub mod noise;
pub mod palette;
pub mod recipe;
pub mod render;
pub mod sdf;
pub mod skinning;
pub mod subdiv;
pub mod texture;
pub mod uv;
pub mod validate;
pub mod world;

pub use error::{Error, Result};
pub use gltf::{Asset, to_glb};
pub use recipe::Recipe;

/// Parse a recipe and compile it into an in-memory [`Asset`].
///
/// This is the primary library entry point. It is total over arbitrary input:
/// malformed JSON yields [`Error::Parse`] and an invalid-but-parseable recipe
/// yields [`Error::Build`]; neither panics.
///
/// ```
/// let asset = imaginu::compile(r#"{"kind":"crystal"}"#).unwrap();
/// assert!(asset.parts.iter().map(|p| p.mesh.triangle_count()).sum::<usize>() > 0);
/// assert!(imaginu::compile("not json").is_err());
/// ```
pub fn compile(recipe_json: &str) -> Result<Asset> {
    let recipe = Recipe::parse(recipe_json).map_err(Error::Parse)?;
    recipe.build().map_err(Error::Build)
}

/// Compile a recipe directly to serialized GLB (`.glb`) bytes, ready to write
/// to disk or hand to a glTF loader such as Babylon.js.
///
/// ```
/// let glb = imaginu::compile_to_glb(r#"{"kind":"rock"}"#).unwrap();
/// assert_eq!(&glb[0..4], b"glTF");
/// ```
pub fn compile_to_glb(recipe_json: &str) -> Result<Vec<u8>> {
    Ok(to_glb(&compile(recipe_json)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_json_is_a_parse_error_not_a_panic() {
        assert!(matches!(compile("}{ not json"), Err(Error::Parse(_))));
    }

    #[test]
    fn invalid_recipe_is_a_build_error_not_a_panic() {
        assert!(matches!(
            compile(r#"{"kind":"tree","palette":"does-not-exist"}"#),
            Err(Error::Build(_))
        ));
    }

    #[test]
    fn hostile_numeric_input_is_clamped_not_a_panic() {
        // Agent-controlled JSON with absurd magnitudes must compile (bounded by
        // internal clamps) rather than OOM or panic. Guards the library
        // boundary against untrusted recipes.
        for json in [
            r#"{"kind":"tree","params":{"branches":100000000}}"#,
            r#"{"kind":"terrain","params":{"size":-50}}"#,
            r##"{"kind":"custom","parts":[{"nodes":[{"shape":"sphere","radius":1,"subdivide":50,"color":"#ffffff"}]}]}"##,
        ] {
            let glb = compile_to_glb(json).expect("hostile-but-parseable recipe should compile");
            assert_eq!(&glb[0..4], b"glTF");
        }
    }

    #[test]
    fn error_displays_and_is_std_error() {
        let e = compile("nope").unwrap_err();
        let _: &dyn std::error::Error = &e;
        assert!(!e.to_string().is_empty());
    }
}
