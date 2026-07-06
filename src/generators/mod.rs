//! Procedural asset generators. Each takes recipe params + seed and returns
//! a fully-formed [`crate::gltf::Asset`].

pub mod building;
pub mod character;
pub mod crystal;
pub mod custom;
pub mod monster;
pub mod prop;
pub mod rock;
pub mod terrain;
pub mod tree;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

pub type Rand = ChaCha8Rng;

pub fn rng(seed: u64) -> Rand {
    ChaCha8Rng::seed_from_u64(seed)
}

/// Uniform in [lo, hi).
pub fn range(r: &mut Rand, lo: f32, hi: f32) -> f32 {
    if hi <= lo { lo } else { r.gen_range(lo..hi) }
}
