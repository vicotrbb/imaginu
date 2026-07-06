//! Deterministic procedural texture baking: noise-driven patterns rendered
//! to PNG (baseColor sRGB, tangent-space normal map, ORM) and embedded in
//! the GLB. Same spec → identical bytes. All patterns tile seamlessly.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::noise::Noise2;
use crate::palette::{hex, lerp, ramp, to_srgb8};

fn d_one() -> f32 {
    1.0
}
fn d_res() -> u32 {
    1024
}

/// A 2D paint operation composited over the base pattern in UV space.
/// With loft UVs (u = around, v = hem→collar) these place borders and
/// ornament exactly where a garment needs them.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PaintLayer {
    /// Horizontal band at `v` (0 = hem) of `height`, optionally decorated
    /// with a repeating motif.
    Band {
        v: f32,
        height: f32,
        color: String,
        #[serde(default)]
        motif: Option<String>,
        #[serde(default)]
        motif_color: Option<String>,
        #[serde(default = "d_one")]
        motif_scale: f32,
    },
    /// Repeating stripes along `axis` ("u" default, or "v").
    Stripes {
        count: u32,
        #[serde(default = "d_stripe_w")]
        width: f32,
        color: String,
        #[serde(default)]
        axis: Option<String>,
    },
    /// Vertical color gradient from v=0 to v=1 (or along u with axis "u").
    Gradient {
        from: String,
        to: String,
        #[serde(default)]
        axis: Option<String>,
    },
    /// Repeating ornament stamps over a v range.
    MotifGrid {
        motif: String,
        color: String,
        #[serde(default = "d_one")]
        scale: f32,
        #[serde(default)]
        v_min: f32,
        #[serde(default = "d_one")]
        v_max: f32,
    },
    /// Painted cloth drape: vertical fold shading + normal-map relief,
    /// wider toward the hem.
    Folds {
        #[serde(default = "d_one")]
        strength: f32,
        #[serde(default = "d_fold_count")]
        count: u32,
    },
    /// Noise-driven grime concentrated toward the hem.
    Weathering {
        #[serde(default = "d_half")]
        strength: f32,
    },
    /// Vertical band at `u` (0..1 around the arc) — front-edge trim on
    /// open coats (u 0 and 1 are the opening edges).
    UBand { u: f32, width: f32, color: String },
}
fn d_stripe_w() -> f32 {
    0.5
}
fn d_fold_count() -> u32 {
    9
}
fn d_half() -> f32 {
    0.5
}
fn d_pattern() -> String {
    "none".into()
}

/// Agent-facing texture request (per part material).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TextureSpec {
    /// wood | rock | fabric | metal | plaster | noise | none (flat base)
    #[serde(default = "d_pattern")]
    pub pattern: String,
    /// Base color for pattern "none" (cloth ground color).
    #[serde(default)]
    pub base: Option<String>,
    /// Paint operations composited over the base, in order.
    #[serde(default)]
    pub paint: Vec<PaintLayer>,
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
        Self {
            w,
            h,
            data: vec![0; (w * h * 3) as usize],
        }
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

/// Encode a height-gradient normal in f64. In-process float-state drift
/// (rounding-mode/FPCR level, observed on macOS ARM: identical inputs,
/// 1-ULP-different f32 results after other work ran) can flip u8 encoding
/// on knife-edge pixels; f64 headroom makes that probability vanish.
fn sobel_normal(dx: f64, dy: f64) -> [u8; 3] {
    let len = (dx * dx + dy * dy + 1.0).sqrt();
    let enc = |c: f64| ((c / len) * 0.5 + 0.5) * 255.0;
    [enc(-dx) as u8, enc(-dy) as u8, enc(1.0) as u8]
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
    Sample {
        height,
        albedo,
        rough: 0.86 - rings * 0.08,
        metal: 0.0,
    }
}

fn rock(p: &Pat, u: f32, v: f32) -> Sample {
    // sedimentary strata bands warped by noise
    let warp = p.tfbm(u, v, 3.0, 3.0, 4);
    let band = ((v * 6.0 + warp * 1.4) * core::f32::consts::TAU).sin() * 0.5 + 0.5;
    let detail = p.tfbm2(u, v, 9.0, 9.0, 5) * 0.5 + 0.5;
    let t = (band * 0.6 + detail * 0.4).clamp(0.0, 1.0);
    let defaults = [
        srgbv(84, 76, 70),
        srgbv(128, 116, 104),
        srgbv(168, 152, 132),
    ];
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
    let (profile, along) = if checker {
        (bump(fx), fy)
    } else {
        (bump(fy), fx)
    };
    let strand = 0.85 + 0.15 * ((along * threads * 2.0 * tau).sin() * 0.5 + 0.5);
    let weave = profile * strand;
    let patchy = p.tfbm2(u, v, 3.0, 3.0, 4) * 0.5 + 0.5;
    let defaults = [srgbv(96, 78, 96), srgbv(140, 116, 138)];
    let dir_tint = if checker { 1.0 } else { 0.88 };
    let albedo = p.colors(&defaults, (weave * 0.6 + patchy * 0.4).clamp(0.0, 1.0)) * dir_tint;
    Sample {
        height: weave * 0.7 + patchy * 0.15,
        albedo,
        rough: 0.97,
        metal: 0.0,
    }
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
    Sample {
        height: 0.5 + base * 0.3 - speck * 0.2,
        albedo,
        rough: 0.9,
        metal: 0.0,
    }
}

fn plain_noise(p: &Pat, u: f32, v: f32) -> Sample {
    let t = p.tfbm(u, v, 4.0, 4.0, 5) * 0.5 + 0.5;
    let defaults = [srgbv(90, 90, 90), srgbv(180, 180, 180)];
    Sample {
        height: t,
        albedo: p.colors(&defaults, t),
        rough: 0.9,
        metal: 0.0,
    }
}

fn flat_none(_p: &Pat, _u: f32, _v: f32) -> Sample {
    Sample {
        height: 0.5,
        albedo: srgbv(200, 200, 200),
        rough: 0.88,
        metal: 0.0,
    }
}

pub const PATTERNS: [&str; 7] = [
    "none", "wood", "rock", "fabric", "metal", "plaster", "noise",
];

// ---------- paint layer compositor ----------

/// Parsed paint layer (colors resolved to linear RGB).
enum PL {
    Band {
        v: f32,
        h: f32,
        color: Vec3,
        motif: Option<Motif>,
        mcolor: Vec3,
        mscale: f32,
    },
    Stripes {
        count: f32,
        width: f32,
        color: Vec3,
        along_v: bool,
    },
    Gradient {
        from: Vec3,
        to: Vec3,
        along_u: bool,
    },
    MotifGrid {
        motif: Motif,
        color: Vec3,
        scale: f32,
        v0: f32,
        v1: f32,
    },
    Folds {
        strength: f32,
        count: f32,
    },
    Weathering {
        strength: f32,
    },
    UBand {
        u: f32,
        width: f32,
        color: Vec3,
    },
}

#[derive(Clone, Copy)]
enum Motif {
    Meander,
    Zigzag,
    Dots,
    Diamonds,
    Scroll,
    Runes,
}

fn parse_motif(s: &str) -> Result<Motif, String> {
    Ok(match s {
        "meander" => Motif::Meander,
        "zigzag" => Motif::Zigzag,
        "dots" => Motif::Dots,
        "diamonds" => Motif::Diamonds,
        "scroll" => Motif::Scroll,
        "runes" => Motif::Runes,
        other => {
            return Err(format!(
                "unknown motif '{other}' (meander|zigzag|dots|diamonds|scroll|runes)"
            ));
        }
    })
}

fn parse_layers(layers: &[PaintLayer]) -> Result<Vec<PL>, String> {
    layers
        .iter()
        .map(|l| {
            Ok(match l {
                PaintLayer::Band {
                    v,
                    height,
                    color,
                    motif,
                    motif_color,
                    motif_scale,
                } => {
                    let c = hex(color)?;
                    PL::Band {
                        v: *v,
                        h: *height,
                        color: c,
                        motif: motif.as_deref().map(parse_motif).transpose()?,
                        mcolor: motif_color
                            .as_deref()
                            .map(hex)
                            .transpose()?
                            .unwrap_or(c * 1.6),
                        mscale: motif_scale.max(0.05),
                    }
                }
                PaintLayer::Stripes {
                    count,
                    width,
                    color,
                    axis,
                } => PL::Stripes {
                    count: (*count).max(1) as f32,
                    width: width.clamp(0.02, 0.98),
                    color: hex(color)?,
                    along_v: axis.as_deref() == Some("v"),
                },
                PaintLayer::Gradient { from, to, axis } => PL::Gradient {
                    from: hex(from)?,
                    to: hex(to)?,
                    along_u: axis.as_deref() == Some("u"),
                },
                PaintLayer::MotifGrid {
                    motif,
                    color,
                    scale,
                    v_min,
                    v_max,
                } => PL::MotifGrid {
                    motif: parse_motif(motif)?,
                    color: hex(color)?,
                    scale: scale.max(0.05),
                    v0: *v_min,
                    v1: *v_max,
                },
                PaintLayer::Folds { strength, count } => PL::Folds {
                    strength: strength.clamp(0.0, 3.0),
                    count: (*count).max(2) as f32,
                },
                PaintLayer::Weathering { strength } => PL::Weathering {
                    strength: strength.clamp(0.0, 1.0),
                },
                PaintLayer::UBand { u, width, color } => PL::UBand {
                    u: *u,
                    width: width.max(0.005),
                    color: hex(color)?,
                },
            })
        })
        .collect()
}

fn wang_hash(mut x: u32) -> u32 {
    x = (x ^ 61) ^ (x >> 16);
    x = x.wrapping_mul(9);
    x ^= x >> 4;
    x = x.wrapping_mul(0x27d4_eb2f);
    x ^ (x >> 15)
}

/// Coverage (0..1) of a motif within one repeat cell (x, y in 0..1).
fn motif_coverage(m: Motif, x: f32, y: f32, cell_id: u32) -> f32 {
    let bar = |lo: f32, hi: f32, t: f32| (t >= lo && t <= hi) as u8 as f32;
    match m {
        Motif::Meander => {
            // simplified greek key: interlocking bars
            let t = 0.14;
            let mut c = 0.0f32;
            c = c.max(bar(0.08, 0.08 + t, y) * bar(0.08, 0.92, x)); // top bar
            c = c.max(bar(0.08, 0.62, y) * bar(0.78, 0.78 + t, x)); // right drop
            c = c.max(bar(0.48, 0.48 + t, y) * bar(0.30, 0.92, x)); // mid bar
            c = c.max(bar(0.48, 0.92, y) * bar(0.30, 0.30 + t, x)); // inner drop
            c = c.max(bar(0.78, 0.78 + t, y) * bar(0.30, 1.0, x)); // bottom bar
            c = c.max(bar(0.08, 0.92, y) * bar(0.0, 0.06, x)); // left rail
            c
        }
        Motif::Zigzag => {
            let tri = (2.0 * (x - (x + 0.5).floor())).abs(); // 0..1 triangle
            let d = (y - (0.2 + tri * 0.6)).abs();
            (1.0 - d / 0.12).clamp(0.0, 1.0)
        }
        Motif::Dots => {
            let d = ((x - 0.5).powi(2) + (y - 0.5).powi(2)).sqrt();
            (1.0 - (d - 0.16).max(0.0) / 0.08).clamp(0.0, 1.0)
        }
        Motif::Diamonds => {
            let d = (x - 0.5).abs() + (y - 0.5).abs();
            (1.0 - (d - 0.24).max(0.0) / 0.07).clamp(0.0, 1.0)
        }
        Motif::Scroll => {
            // circular ring with a tail — reads as scrollwork in a row
            let (dx, dy) = (x - 0.42, y - 0.5);
            let ring = ((dx * dx + dy * dy).sqrt() - 0.24).abs();
            let c = (1.0 - ring / 0.07).clamp(0.0, 1.0);
            let tail = bar(0.44, 0.56, y) * bar(0.66, 1.0, x);
            c.max(tail)
        }
        Motif::Runes => {
            // 2-3 pseudo-random strokes per cell — reads as script
            let h = wang_hash(cell_id.wrapping_mul(2654435761));
            let mut c = 0.0f32;
            // vertical stroke
            if h & 1 != 0 {
                c = c.max(bar(0.2, 0.8, y) * bar(0.44, 0.56, x));
            }
            // cross stroke at hashed height
            let cy = 0.25 + ((h >> 3) & 3) as f32 * 0.15;
            c = c.max(bar(cy, cy + 0.12, y) * bar(0.2, 0.8, x));
            // diagonal tick
            if h & 4 != 0 {
                let d = (y - (x * 0.6 + 0.15)).abs();
                c = c.max((1.0 - d / 0.09).clamp(0.0, 1.0) * bar(0.15, 0.85, x));
            }
            c
        }
    }
}

/// Apply parsed paint layers to a pattern sample at (u, v).
fn apply_layers(pat: &Pat, layers: &[PL], u: f32, v: f32, s: &mut Sample) {
    for l in layers {
        match l {
            PL::Band {
                v: v0,
                h,
                color,
                motif,
                mcolor,
                mscale,
            } => {
                if v >= *v0 && v <= *v0 + *h {
                    s.albedo = *color;
                    s.rough = 0.7; // trim reads silkier than ground cloth
                    s.height = 0.55;
                    if let Some(m) = motif {
                        let cols = (12.0 * mscale).max(1.0).round();
                        let x = (u * cols).fract();
                        let y = ((v - v0) / h).clamp(0.0, 1.0);
                        let cell = (u * cols) as u32;
                        let cov = motif_coverage(*m, x, y, cell);
                        if cov > 0.0 {
                            s.albedo = lerp(s.albedo, *mcolor, cov);
                            s.height += 0.12 * cov;
                            s.rough -= 0.1 * cov;
                        }
                    }
                }
            }
            PL::Stripes {
                count,
                width,
                color,
                along_v,
            } => {
                let t = if *along_v { v } else { u };
                if (t * count).fract() < *width {
                    s.albedo = *color;
                }
            }
            PL::Gradient { from, to, along_u } => {
                let t = if *along_u { u } else { v };
                s.albedo *= lerp(*from, *to, t);
            }
            PL::MotifGrid {
                motif,
                color,
                scale,
                v0,
                v1,
            } => {
                if v >= *v0 && v <= *v1 {
                    let cols = (8.0 * scale).max(1.0).round();
                    let rows = ((v1 - v0) * cols).max(1.0).round();
                    let x = (u * cols).fract();
                    let yy = ((v - v0) / (v1 - v0) * rows).fract();
                    let cell = (u * cols) as u32 ^ (((v - v0) / (v1 - v0) * rows) as u32) << 8;
                    let cov = motif_coverage(*motif, x, yy, cell);
                    if cov > 0.0 {
                        s.albedo = lerp(s.albedo, *color, cov);
                        s.height += 0.1 * cov;
                    }
                }
            }
            PL::Folds { strength, count } => {
                // drape: sine folds warped by noise, deeper toward the hem
                let warp = pat.tfbm(u, v, 3.0, 2.0, 3) * 0.8;
                let phase = (u * count + warp) * core::f32::consts::TAU;
                let depth = strength * (1.0 - v * 0.65);
                let shade = phase.sin() * 0.5 + 0.5;
                s.albedo *= 1.0 - 0.28 * depth * (1.0 - shade);
                s.height += (shade - 0.5) * 0.35 * depth;
            }
            PL::Weathering { strength } => {
                let g = pat.tfbm2(u, v, 5.0, 5.0, 4) * 0.5 + 0.5;
                let hem = (1.0 - v * 2.2).clamp(0.0, 1.0);
                let dirt = (g * 0.55 + hem * 0.6) * strength;
                s.albedo *= 1.0 - 0.35 * dirt.clamp(0.0, 1.0);
                s.rough = (s.rough + 0.15 * dirt).min(1.0);
            }
            PL::UBand {
                u: u0,
                width,
                color,
            } => {
                if (u - u0).abs() <= *width / 2.0 {
                    s.albedo = *color;
                    s.rough = 0.7;
                    s.height = 0.58;
                }
            }
        }
    }
}

/// Bake a texture spec to images. Deterministic.
pub fn bake(spec: &TextureSpec) -> Result<BakedTexture, String> {
    let res = spec.resolution.clamp(64, 4096);
    let sampler: fn(&Pat, f32, f32) -> Sample = match spec.pattern.as_str() {
        "none" => flat_none,
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
    let base_col = spec.base.as_deref().map(hex).transpose()?;
    let layers = parse_layers(&spec.paint)?;
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
            let mut s = sampler(&pat, u, v);
            if let Some(b) = base_col {
                if spec.pattern == "none" {
                    s.albedo = b;
                } else {
                    s.albedo = s.albedo * 0.35 + s.albedo * b * 1.3; // tint
                }
            }
            apply_layers(&pat, &layers, u, v, &mut s);
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
            // f64 throughout: in-process rounding-state drift gives 1-ULP f32
            // differences that flip u8 encoding on knife-edge pixels
            let g = |x: i64, y: i64| hgt(x, y) as f64;
            let dx = (g(x + 1, y - 1) + 2.0 * g(x + 1, y) + g(x + 1, y + 1))
                - (g(x - 1, y - 1) + 2.0 * g(x - 1, y) + g(x - 1, y + 1));
            let dy = (g(x - 1, y + 1) + 2.0 * g(x, y + 1) + g(x + 1, y + 1))
                - (g(x - 1, y - 1) + 2.0 * g(x, y - 1) + g(x + 1, y - 1));
            // clamp slopes so hard height steps don't blow out into white rims
            // black_box pins the codegen: the auto-vectorized form of this
            // loop produced process-history-dependent results on macOS ARM
            // (same binary, same inputs, two stable outcomes)
            let (dx, dy) = (std::hint::black_box(dx), std::hint::black_box(dy));
            let (dx, dy) = (
                (dx * strength as f64).clamp(-0.85, 0.85),
                (dy * strength as f64).clamp(-0.85, 0.85),
            );
            let n = sobel_normal(dx, dy);
            normal.put(x as u32, y as u32, n);
        }
    }

    Ok(BakedTexture {
        // full spec serialization: any difference (incl. paint) = new image set
        key: serde_json::to_string(spec).unwrap_or_default(),
        base_color: base,
        normal,
        orm,
    })
}

/// Bake a face texture for a spherically-unwrapped head: skin mottling,
/// cheek blush, eye-socket shading, and age wrinkles (forehead lines,
/// crow's feet, nasolabial folds). Convention: u = azimuth with the face
/// centered at u = 0.5, v = 0 at the crown → 1 at the chin.
pub fn bake_face(skin: Vec3, age: f32, seed: u64, res: u32) -> BakedTexture {
    let res = res.clamp(64, 1024);
    let pat = Pat {
        n: Noise2::new(seed.wrapping_add(0xFACE)),
        n2: Noise2::new(seed.wrapping_add(0xFACE2)),
        ramp_cols: Vec::new(),
    };
    let age = age.clamp(0.0, 1.0);
    // soft darkened stroke along a horizontal arc
    let line = |u: f32, v: f32, cu: f32, cv: f32, half_w: f32, sag: f32, thick: f32| -> f32 {
        if (u - cu).abs() > half_w {
            return 0.0;
        }
        let x = (u - cu) / half_w; // -1..1
        let target_v = cv + sag * x * x;
        (1.0 - ((v - target_v) / thick).abs()).clamp(0.0, 1.0)
    };
    let mut heights = vec![0.5f32; (res * res) as usize];
    let mut base = Rgb8Image::new(res, res);
    let mut orm = Rgb8Image::new(res, res);
    for y in 0..res {
        for x in 0..res {
            let u = x as f32 / res as f32;
            let v = y as f32 / res as f32;
            let mut c = skin;
            // gentle mottling
            let m = pat.tfbm(u, v, 6.0, 6.0, 4);
            c *= 1.0 + m * 0.05;
            // cheek warmth
            for cu in [0.40f32, 0.60] {
                let d = ((u - cu).powi(2) * 3.0 + (v - 0.58).powi(2)).sqrt();
                let blush = (1.0 - d / 0.14).clamp(0.0, 1.0);
                c = lerp(c, c * Vec3::new(1.10, 0.94, 0.90), blush * 0.5);
            }
            // eye-socket shading
            for cu in [0.42f32, 0.58] {
                let d = ((u - cu).powi(2) * 4.0 + (v - 0.47).powi(2)).sqrt();
                let s = (1.0 - d / 0.075).clamp(0.0, 1.0);
                c *= 1.0 - 0.10 * s;
            }
            let mut wrinkle = 0.0f32;
            if age > 0.05 {
                // forehead lines
                for (i, cv) in [0.30f32, 0.345, 0.39].iter().enumerate() {
                    if age > 0.2 + i as f32 * 0.25 {
                        wrinkle = wrinkle.max(line(u, v, 0.5, *cv, 0.13, 0.03, 0.012));
                    }
                }
                // crow's feet
                for (cu, dir) in [(0.345f32, -1.0f32), (0.655, 1.0)] {
                    for dv in [-0.012f32, 0.0, 0.012] {
                        let x = (u - cu) * dir;
                        if (0.0..0.045).contains(&x) {
                            let target = 0.47 + dv + x * (dv * 14.0);
                            wrinkle = wrinkle
                                .max(((1.0 - ((v - target) / 0.008).abs()).clamp(0.0, 1.0)) * 0.8);
                        }
                    }
                }
                // nasolabial folds
                for (cu, dir) in [(0.46f32, -1.0f32), (0.54, 1.0)] {
                    let t = ((v - 0.60) / 0.10).clamp(0.0, 1.0);
                    let target_u = cu + dir * (0.015 + t * 0.035);
                    if (0.60..0.70).contains(&v) {
                        wrinkle = wrinkle
                            .max(((1.0 - ((u - target_u) / 0.008).abs()).clamp(0.0, 1.0)) * 0.9);
                    }
                }
                wrinkle *= age;
                c *= 1.0 - 0.16 * wrinkle;
            }
            heights[(y * res + x) as usize] = 0.5 + m * 0.08 - wrinkle * 0.22;
            let ao = 1.0 - 0.05 * wrinkle;
            base.put(x, y, to_srgb8(c));
            orm.put(x, y, [(ao * 255.0) as u8, 178, 0]);
        }
    }
    // Sobel normal pass (same as bake)
    let mut normal = Rgb8Image::new(res, res);
    let strength = 1.0 * res as f32 / 256.0;
    let hgt = |x: i64, y: i64| -> f32 {
        let x = x.rem_euclid(res as i64) as u32;
        let y = y.rem_euclid(res as i64) as u32;
        heights[(y * res + x) as usize]
    };
    for y in 0..res as i64 {
        for x in 0..res as i64 {
            // f64 throughout (see the twin comment in `bake`)
            let g = |x: i64, y: i64| hgt(x, y) as f64;
            let dx = (g(x + 1, y - 1) + 2.0 * g(x + 1, y) + g(x + 1, y + 1))
                - (g(x - 1, y - 1) + 2.0 * g(x - 1, y) + g(x - 1, y + 1));
            let dy = (g(x - 1, y + 1) + 2.0 * g(x, y + 1) + g(x + 1, y + 1))
                - (g(x - 1, y - 1) + 2.0 * g(x, y - 1) + g(x + 1, y - 1));
            // black_box pins the codegen: the auto-vectorized form of this
            // loop produced process-history-dependent results on macOS ARM
            // (same binary, same inputs, two stable outcomes)
            let (dx, dy) = (std::hint::black_box(dx), std::hint::black_box(dy));
            let (dx, dy) = (
                (dx * strength as f64).clamp(-0.85, 0.85),
                (dy * strength as f64).clamp(-0.85, 0.85),
            );
            let n = sobel_normal(dx, dy);
            normal.put(x as u32, y as u32, n);
        }
    }
    BakedTexture {
        key: format!("face:{skin}:{age}:{seed}:{res}"),
        base_color: base,
        normal,
        orm,
    }
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
            base: None,
            paint: Vec::new(),
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
    fn paint_layers_composite() {
        let mut sp = spec("none");
        sp.base = Some("#e8ddc4".into());
        sp.resolution = 128;
        sp.paint = vec![
            PaintLayer::Band {
                v: 0.0,
                height: 0.1,
                color: "#7a1f1f".into(),
                motif: Some("meander".into()),
                motif_color: Some("#e8b54a".into()),
                motif_scale: 1.0,
            },
            PaintLayer::Folds {
                strength: 1.0,
                count: 8,
            },
        ];
        let a = bake(&sp).unwrap();
        let b = bake(&sp).unwrap();
        assert_eq!(
            a.base_color.data, b.base_color.data,
            "paint must be deterministic"
        );
        // band region (v near 0 = top rows) is red-dominant; mid is cream
        let band_px = a.base_color.texel(10, 3);
        let mid_px = a.base_color.texel(10, 64);
        assert!(band_px.x > band_px.y * 1.5, "band should be red: {band_px}");
        assert!(mid_px.y > 0.5, "mid should be cream: {mid_px}");
        // different paint = different dedup key
        let mut sp2 = sp.clone();
        sp2.paint.pop();
        assert_ne!(bake(&sp2).unwrap().key, a.key);
        // bad motif rejected
        let mut sp3 = sp.clone();
        sp3.paint = vec![PaintLayer::MotifGrid {
            motif: "paisley".into(),
            color: "#000000".into(),
            scale: 1.0,
            v_min: 0.0,
            v_max: 1.0,
        }];
        assert!(bake(&sp3).is_err());
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
