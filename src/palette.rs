//! Curated palettes and color utilities. Colors are linear-space RGB
//! (glTF expects linear vertex colors; the renderer gamma-encodes on output).

use glam::Vec3;

/// sRGB u8 -> linear Vec3.
pub fn srgb(r: u8, g: u8, b: u8) -> Vec3 {
    let f = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec3::new(f(r), f(g), f(b))
}

/// Linear -> sRGB u8 (for PNG output).
pub fn to_srgb8(c: Vec3) -> [u8; 3] {
    let f = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    [f(c.x), f(c.y), f(c.z)]
}

/// sRGB-encoded 0..1 Vec3 -> linear (texture sampling).
pub fn srgb_to_linear(c: Vec3) -> Vec3 {
    let f = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec3::new(f(c.x), f(c.y), f(c.z))
}

/// Linear RGB -> `#rrggbb` (for building TextureSpecs from palette colors).
pub fn to_hex(c: Vec3) -> String {
    let [r, g, b] = to_srgb8(c);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Parse a `#rrggbb` hex string to linear RGB.
pub fn hex(s: &str) -> Result<Vec3, String> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 {
        return Err(format!("bad hex color '{s}'"));
    }
    let v = u32::from_str_radix(h, 16).map_err(|e| format!("bad hex '{s}': {e}"))?;
    Ok(srgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Multi-stop gradient sampling, stops assumed evenly spaced.
pub fn ramp(stops: &[Vec3], t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (t as usize).min(stops.len() - 2);
    lerp(stops[i], stops[i + 1], t - i as f32)
}

/// Small hue/value jitter for organic variation, keeps colors harmonious.
pub fn vary(c: Vec3, amount: f32, r: f32) -> Vec3 {
    let v = 1.0 + (r * 2.0 - 1.0) * amount;
    (c * v).clamp(Vec3::ZERO, Vec3::ONE)
}

pub struct Palette {
    pub name: &'static str,
    /// low -> high terrain ramp (also reused as generic gradient)
    pub terrain: [Vec3; 6],
    pub foliage: [Vec3; 3],
    pub trunk: Vec3,
    pub rock: [Vec3; 2],
    pub water: Vec3,
    pub accent: Vec3,
}

pub fn by_name(name: &str) -> Palette {
    match name {
        "autumn" => Palette {
            name: "autumn",
            terrain: [
                srgb(96, 108, 56),
                srgb(133, 117, 62),
                srgb(169, 132, 60),
                srgb(188, 108, 37),
                srgb(120, 96, 76),
                srgb(240, 238, 228),
            ],
            foliage: [srgb(204, 102, 26), srgb(217, 148, 47), srgb(158, 66, 36)],
            trunk: srgb(92, 62, 44),
            rock: [srgb(122, 110, 100), srgb(88, 80, 74)],
            water: srgb(48, 86, 96),
            accent: srgb(214, 40, 40),
        },
        "arctic" => Palette {
            name: "arctic",
            terrain: [
                srgb(70, 96, 110),
                srgb(132, 160, 170),
                srgb(198, 214, 220),
                srgb(228, 238, 242),
                srgb(200, 212, 222),
                srgb(250, 252, 254),
            ],
            foliage: [srgb(58, 96, 84), srgb(74, 118, 102), srgb(46, 76, 70)],
            trunk: srgb(70, 56, 50),
            rock: [srgb(108, 118, 130), srgb(76, 84, 96)],
            water: srgb(58, 108, 138),
            accent: srgb(120, 200, 226),
        },
        "volcanic" => Palette {
            name: "volcanic",
            terrain: [
                srgb(40, 34, 36),
                srgb(62, 50, 50),
                srgb(88, 70, 64),
                srgb(56, 48, 50),
                srgb(36, 30, 32),
                srgb(214, 214, 216),
            ],
            foliage: [srgb(96, 84, 44), srgb(120, 100, 52), srgb(76, 66, 38)],
            trunk: srgb(44, 36, 34),
            rock: [srgb(58, 52, 54), srgb(34, 30, 32)],
            water: srgb(226, 88, 20),
            accent: srgb(255, 120, 28),
        },
        "desert" => Palette {
            name: "desert",
            terrain: [
                srgb(214, 178, 122),
                srgb(226, 192, 134),
                srgb(206, 158, 100),
                srgb(178, 126, 82),
                srgb(150, 104, 72),
                srgb(244, 234, 214),
            ],
            foliage: [srgb(112, 132, 66), srgb(90, 112, 58), srgb(130, 146, 80)],
            trunk: srgb(110, 82, 56),
            rock: [srgb(190, 148, 106), srgb(142, 106, 78)],
            water: srgb(62, 142, 148),
            accent: srgb(226, 122, 60),
        },
        "mystic" => Palette {
            name: "mystic",
            terrain: [
                srgb(44, 48, 82),
                srgb(66, 62, 110),
                srgb(96, 76, 138),
                srgb(130, 96, 160),
                srgb(88, 74, 120),
                srgb(226, 214, 244),
            ],
            foliage: [srgb(96, 60, 160), srgb(140, 84, 190), srgb(66, 46, 120)],
            trunk: srgb(52, 42, 66),
            rock: [srgb(92, 86, 122), srgb(62, 58, 88)],
            water: srgb(70, 190, 200),
            accent: srgb(90, 240, 220),
        },
        // "verdant" default
        _ => Palette {
            name: "verdant",
            terrain: [
                srgb(210, 196, 148),
                srgb(122, 156, 74),
                srgb(88, 128, 62),
                srgb(64, 96, 56),
                srgb(124, 116, 106),
                srgb(246, 248, 246),
            ],
            foliage: [srgb(74, 128, 58), srgb(96, 150, 68), srgb(56, 104, 52)],
            trunk: srgb(98, 70, 48),
            rock: [srgb(134, 128, 118), srgb(96, 92, 86)],
            water: srgb(52, 110, 130),
            accent: srgb(220, 90, 66),
        },
    }
}

pub const PALETTES: [&str; 6] = [
    "verdant", "autumn", "arctic", "volcanic", "desert", "mystic",
];
