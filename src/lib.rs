//! imaginu — AI-drivable procedural 3D asset compiler.
//!
//! JSON recipes -> deterministic, game-ready GLB (meshes, PBR vertex colors,
//! skeletal animation, physics metadata) for Babylon.js, plus a built-in
//! software renderer for visual verification.

pub mod generators;
pub mod gltf;
pub mod mesh;
pub mod noise;
pub mod palette;
pub mod recipe;
pub mod render;
pub mod skinning;
pub mod texture;
pub mod uv;
