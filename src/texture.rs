//! Deterministic procedural texture baking: noise-driven patterns rendered
//! to PNG (baseColor sRGB, tangent-space normal map, ORM) and embedded in
//! the GLB. Same spec → identical bytes. All patterns tile seamlessly.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::noise::Noise2;
use crate::palette::{hex, lerp, ramp, to_srgb8};

fn d_one() -> f32 { 1.0 }
fn d_res() -> u32 { 1024 }

/// Agent-facing texture request (per part material).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TextureSpec {
    /// wood | rock | fabric | metal | plaster | noise
    pub pattern: String,
    /// World units per texture tile.
    #[serde(default = "d_one")]
    pub scale: f32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "d_one")]
    pub normal_strength: f32,
    /// Bake resolution, clamped to 64..=4096.
    #[serde(default = "d_res")]
    pub resolution: u32,
    /// Optional `#rrggbb` ramp overriding the pattern's built-in colors
    /// (dark → light, 2-4 stops).
    #[serde(default)]
    pub colors: Vec<String>,
}

/// 8-bit RGB image; `data` is row-major RGB triples.
#[derive(Clone, Debug)]
pub struct Rgb8Image {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Rgb8Image {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, data: vec![0; (w * h * 3) as usize] }
    }

    fn put(&mut self, x: u32, y: u32, px: [u8; 3]) {
        let i = ((y * self.w + x) * 3) as usize;
        self.data[i..i + 3].copy_from_slice(&px);
    }

    fn texel(&self, x: i64, y: i64) -> Vec3 {
        let x = x.rem_euclid(self.w as i64) as u32;
        let y = y.rem_euclid(self.h as i64) as u32;
        let i = ((y * self.w + x) * 3) as usize;
        Vec3::new(
            self.data[i] as f32 / 255.0,
            self.data[i + 1] as f32 / 255.0,
            self.data[i + 2] as f32 / 255.0,
        )
    }

    /// Bilinear sample with wrap; returns raw 0..1 values (no color decode).
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let x = u * self.w as f32 - 0.5;
        let y = v * self.h as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(x0, y0) * (1.0 - fx) + self.texel(x0 + 1, y0) * fx;
        let b = self.texel(x0, y0 + 1) * (1.0 - fx) + self.texel(x0 + 1, y0 + 1) * fx;
        a * (1.0 - fy) + b * fy
    }

    pub fn to_png_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().expect("png header");
            w.write_image_data(&self.data).expect("png data");
        }
        out
    }
}

/// A fully baked material texture set.
#[derive(Clone, Debug)]
pub struct BakedTexture {
    /// Dedup key: identical specs share one image set in the GLB.
    pub key: String,
    pub base_color: Rgb8Image,
    pub normal: Rgb8Image,
    /// R = occlusion, G = roughness, B = metallic.
    pub orm: Rgb8Image,
}

/// One sample of a pattern at (u, v) in [0,1): surface height, albedo,
/// roughness and metallic.
struct Sample {
    height: f32,
    albedo: Vec3,
    rough: f32,
    metal: f32,
}

struct Pat {
    n: Noise2,
    n2: Noise2,
    ramp_cols: Vec<Vec3>,
}

impl Pat {
    /// Seamlessly tiling fBm (period 1 in uv) with uniform statistics —
    /// the noise lattice itself wraps, so there is no blend seam and no
    /// amplitude dip mid-tile. `fu`/`fv` are integer per-axis frequencies;
    /// callers must pass u, v raw in [0,1), never pre-scaled.
    fn tfbm(&self, u: f32, v: f32, fu: f32, fv: f32, octaves: u32) -> f32 {
        self.n.fbm_tiled(u, v, fu as u32, fv as u32, octaves, 0.5)
    }

    /// Second independent tileable channel.
    fn tfbm2(&self, u: f32, v: f32, fu: f32, fv: f32, octaves: u32) -> f32 {
        self.n2.fbm_tiled(u, v, fu as u32, fv as u32, octaves, 0.5)
    }

    fn colors(&self, defaults: &[Vec3], t: f32) -> Vec3 {
        if self.ramp_cols.len() >= 2 {
            ramp(&self.ramp_cols, t)
        } else {
            ramp(defaults, t)
        }
    }
}

fn srgbv(r: u8, g: u8, b: u8) -> Vec3 {
    crate::palette::srgb(r, g, b)
}

fn wood(p: &Pat, u: f32, v: f32) -> Sample {
    // two planks per tile, grain running along v
    let planks = 2.0;
    let pu = u * planks;
    let plank = pu.floor().min(planks - 1.0);
    let x = pu - plank; // 0..1 across the plank
    let grain = p.tfbm(u, v, 5.0, 2.0, 4);
    // rings use u directly with an integer angular frequency so the height
    // field is fully periodic — no per-plank phase offset, no seam cliffs
    let rings = ((u * 8.0 + grain * 1.8) * core::f32::consts::PI).sin() * 0.5 + 0.5;
    let streak = p.tfbm2(u, v, 10.0, 14.0, 3) * 0.5 + 0.5;
    let t = (rings * 0.55 + streak * 0.45).clamp(0.0, 1.0);
    let defaults = [srgbv(74, 48, 30), srgbv(122, 82, 50), srgbv(158, 116, 74)];
    let mut albedo = p.colors(&defaults, t);
    // plank seams darken albedo only; the height map stays groove-free
    // (height ramps at seams render as bright normal-mapped walls)
    let d = x.min(1.0 - x) * planks; // 0 at border
    let edge = (d * 18.0).clamp(0.0, 1.0);
    let edge = edge * edge * (3.0 - 2.0 * edge); // smoothstep
    albedo *= 0.45 + 0.55 * edge;
    let height = 0.4 + rings * 0.2 + streak * 0.1;
    Sample { height, albedo, rough: 0.86 - rings * 0.08, metal: 0.0 }
}

fn rock(p: &Pat, u: f32, v: f32) -> Sample {
    // sedimentary strata bands warped by noise
    let warp = p.tfbm(u, v, 3.0, 3.0, 4);
    let band = ((v * 6.0 + warp * 1.4) * core::f32::consts::TAU).sin() * 0.5 + 0.5;
    let detail = p.tfbm2(u, v, 9.0, 9.0, 5) * 0.5 + 0.5;
    let t = (band * 0.6 + detail * 0.4).clamp(0.0, 1.0);
    let defaults = [srgbv(84, 76, 70), srgbv(128, 116, 104), srgbv(168, 152, 132)];
    let albedo = p.colors(&defaults, t);
    Sample {
        height: (band * 0.5 + detail * 0.5).clamp(0.0, 1.0),
        albedo,
        rough: 0.95,
        metal: 0.0,
    }
}

fn fabric(p: &Pat, u: f32, v: f32) -> Sample {
    // interleaved warp/weft: checkerboard of horizontal/vertical threads
    let threads = 12.0;
    let tau = core::f32::consts::TAU;
    let wobble = p.tfbm(u, v, 6.0, 6.0, 3) * 0.02;
    let (cu, cv) = (u + wobble, v - wobble);
    let checker = ((cu * threads).floor() + (cv * threads).floor()) as i64 % 2 == 0;
    // thread profile within its cell (rounded bump along the crossing dir)
    let fx = (cu * threads).fract();
    let fy = (cv * threads).fract();
    let bump = |t: f32| ((t * tau / 2.0).sin()).max(0.0);
    let (profile, along) = if checker { (bump(fx), fy) } else { (bump(fy), fx) };
    let strand = 0.85 + 0.15 * ((along * threads * 2.0 * tau).sin() * 0.5 + 0.5);
    let weave = profile * strand;
    let patchy = p.tfbm2(u, v, 3.0, 3.0, 4) * 0.5 + 0.5;
    let defaults = [srgbv(96, 78, 96), srgbv(140, 116, 138)];
    let dir_tint = if checker { 1.0 } else { 0.88 };
    let albedo = p.colors(&defaults, (weave * 0.6 + patchy * 0.4).clamp(0.0, 1.0)) * dir_tint;
    Sample { height: weave * 0.7 + patchy * 0.15, albedo, rough: 0.97, metal: 0.0 }
}

fn metal(p: &Pat, u: f32, v: f32) -> Sample {
    // painted metal: paint coat, thin scratches, worn patches showing metal
    let dents = p.tfbm(u, v, 3.0, 3.0, 3);
    // sparse thin scratches: tight ridge threshold gated by a coarse mask
    let scratch_field = p.tfbm2(u, v, 18.0, 3.0, 2);
    let mask = (p.tfbm(u, v, 2.0, 2.0, 2) * 2.5 - 0.8).clamp(0.0, 1.0);
    let scratch = ((1.0 - scratch_field.abs() * 30.0).clamp(0.0, 1.0)) * mask;
    // wear concentrated in a few patches
    let wear = ((p.tfbm(u, v, 4.0, 4.0, 4) - 0.42) * 4.0).clamp(0.0, 1.0);
    let defaults = [srgbv(52, 84, 96), srgbv(74, 112, 124)];
    let paint = p.colors(&defaults, (dents * 1.4 + 0.5).clamp(0.0, 1.0));
    let bare = srgbv(150, 148, 145);
    let worn = (wear + scratch * 0.7).clamp(0.0, 1.0);
    let albedo = lerp(paint, bare, worn);
    Sample {
        height: 0.6 + dents * 0.25 - scratch * 0.3 - wear * 0.1,
        albedo,
        rough: 0.55 - worn * 0.25,
        metal: 0.15 + worn * 0.75,
    }
}

fn plaster(p: &Pat, u: f32, v: f32) -> Sample {
    let base = p.tfbm(u, v, 5.0, 5.0, 5) * 0.5 + 0.5;
    let speck = (p.tfbm2(u, v, 40.0, 40.0, 2)).max(0.0);
    let defaults = [srgbv(196, 186, 168), srgbv(226, 218, 202)];
    let albedo = p.colors(&defaults, base) * (1.0 - speck * 0.25);
    Sample { height: 0.5 + base * 0.3 - speck * 0.2, albedo, rough: 0.9, metal: 0.0 }
}

fn plain_noise(p: &Pat, u: f32, v: f32) -> Sample {
    let t = p.tfbm(u, v, 4.0, 4.0, 5) * 0.5 + 0.5;
    let defaults = [srgbv(90, 90, 90), srgbv(180, 180, 180)];
    Sample { height: t, albedo: p.colors(&defaults, t), rough: 0.9, metal: 0.0 }
}

pub const PATTERNS: [&str; 6] = ["wood", "rock", "fabric", "metal", "plaster", "noise"];

/// Bake a texture spec to images. Deterministic.
pub fn bake(spec: &TextureSpec) -> Result<BakedTexture, String> {
    let res = spec.resolution.clamp(64, 4096);
    let sampler: fn(&Pat, f32, f32) -> Sample = match spec.pattern.as_str() {
        "wood" => wood,
        "rock" => rock,
        "fabric" => fabric,
        "metal" => metal,
        "plaster" => plaster,
        "noise" => plain_noise,
        other => {
            return Err(format!(
                "unknown texture pattern '{other}' (available: {})",
                PATTERNS.join(", ")
            ));
        }
    };
    let mut ramp_cols = Vec::new();
    for c in &spec.colors {
        ramp_cols.push(hex(c)?);
    }
    let pat = Pat {
        n: Noise2::new(spec.seed.wrapping_add(0xA11CE)),
        n2: Noise2::new(spec.seed.wrapping_add(0xB0B0_5EED)),
        ramp_cols,
    };

    // heights kept for the normal-map derivative pass
    let mut heights = vec![0.0f32; (res * res) as usize];
    let mut base = Rgb8Image::new(res, res);
    let mut orm = Rgb8Image::new(res, res);
    for y in 0..res {
        for x in 0..res {
            let u = x as f32 / res as f32;
            let v = y as f32 / res as f32;
            let s = sampler(&pat, u, v);
            let h = s.height.clamp(0.0, 1.0);
            heights[(y * res + x) as usize] = h;
            // subtle cavity shading baked into albedo
            let ao = 0.85 + 0.15 * h;
            base.put(x, y, to_srgb8(s.albedo * ao));
            orm.put(
                x,
                y,
                [
                    (ao * 255.0) as u8,
                    (s.rough.clamp(0.03, 1.0) * 255.0) as u8,
                    (s.metal.clamp(0.0, 1.0) * 255.0) as u8,
                ],
            );
        }
    }

    // Sobel-derived tangent-space normal map (wrapping)
    let mut normal = Rgb8Image::new(res, res);
    let strength = spec.normal_strength.clamp(0.0, 8.0) * res as f32 / 256.0;
    let hgt = |x: i64, y: i64| -> f32 {
        let x = x.rem_euclid(res as i64) as u32;
        let y = y.rem_euclid(res as i64) as u32;
        heights[(y * res + x) as usize]
    };
    for y in 0..res as i64 {
        for x in 0..res as i64 {
            let dx = (hgt(x + 1, y - 1) + 2.0 * hgt(x + 1, y) + hgt(x + 1, y + 1))
                - (hgt(x - 1, y - 1) + 2.0 * hgt(x - 1, y) + hgt(x - 1, y + 1));
            let dy = (hgt(x - 1, y + 1) + 2.0 * hgt(x, y + 1) + hgt(x + 1, y + 1))
                - (hgt(x - 1, y - 1) + 2.0 * hgt(x, y - 1) + hgt(x + 1, y - 1));
            // clamp slopes so hard height steps don't blow out into white rims
            let (dx, dy) = ((dx * strength).clamp(-0.85, 0.85), (dy * strength).clamp(-0.85, 0.85));
            let n = Vec3::new(-dx, -dy, 1.0).normalize();
            normal.put(
                x as u32,
                y as u32,
                [
                    ((n.x * 0.5 + 0.5) * 255.0) as u8,
                    ((n.y * 0.5 + 0.5) * 255.0) as u8,
                    ((n.z * 0.5 + 0.5) * 255.0) as u8,
                ],
            );
        }
    }

    Ok(BakedTexture {
        key: format!(
            "{}:{}:{}:{}:{}:{:?}",
            spec.pattern, spec.scale, spec.seed, spec.normal_strength, res, spec.colors
        ),
        base_color: base,
        normal,
        orm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(pattern: &str) -> TextureSpec {
        TextureSpec {
            pattern: pattern.into(),
            scale: 1.0,
            seed: 3,
            normal_strength: 1.0,
            resolution: 64,
            colors: Vec::new(),
        }
    }

    #[test]
    fn bake_deterministic() {
        let a = bake(&spec("wood")).unwrap();
        let b = bake(&spec("wood")).unwrap();
        assert_eq!(a.base_color.data, b.base_color.data);
        assert_eq!(a.normal.data, b.normal.data);
        assert_eq!(a.orm.data, b.orm.data);
    }

    #[test]
    fn all_patterns_bake_and_tile() {
        for p in PATTERNS {
            let t = bake(&spec(p)).unwrap();
            assert_eq!(t.base_color.data.len(), 64 * 64 * 3);
            // seamless: wrapped bilinear sample at the seam ≈ sample just inside
            let a = t.base_color.sample(0.0, 0.5);
            let b = t.base_color.sample(1.0, 0.5);
            assert!((a - b).length() < 1e-4, "{p} seam {a:?} vs {b:?}");
        }
    }

    #[test]
    fn png_encodes() {
        let t = bake(&spec("rock")).unwrap();
        let png = t.base_color.to_png_bytes();
        assert_eq!(&png[1..4], b"PNG");
    }

    #[test]
    fn unknown_pattern_rejected() {
        assert!(bake(&spec("tartan")).is_err());
    }
}
