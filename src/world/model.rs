//! The deterministic world model. Heights, colors and (later) zones are
//! pure functions of world coordinates + seed — the seam law. Nothing here
//! may depend on which chunk is being built or in what order.

use glam::Vec3;

use crate::noise::Noise2;
use crate::palette::{self, Palette, ramp, vary};

use super::WorldParams;

pub struct WorldModel {
    pub p: WorldParams,
    pub pal: Palette,
    pub n: Noise2,
    /// Grid dimensions in chunks.
    pub nx: u32,
    pub nz: u32,
    /// Total world extent in meters (size snapped to whole chunks).
    pub size_x: f32,
    pub size_z: f32,
    /// Master height amplitude in meters.
    pub amp: f32,
}

impl WorldModel {
    pub fn new(p: &WorldParams) -> Result<Self, String> {
        if !palette::PALETTES.contains(&p.palette.as_str()) {
            return Err(format!(
                "unknown palette '{}' (available: {})",
                p.palette,
                palette::PALETTES.join(", ")
            ));
        }
        let mut p = p.clone();
        p.chunk_size = p.chunk_size.clamp(32.0, 1024.0);
        p.chunk_resolution = p.chunk_resolution.clamp(16, 512);
        let n_chunks = (p.size / p.chunk_size).round().clamp(1.0, 64.0) as u32;
        let size = n_chunks as f32 * p.chunk_size;
        Ok(Self {
            pal: palette::by_name(&p.palette),
            n: Noise2::new(p.seed),
            nx: n_chunks,
            nz: n_chunks,
            size_x: size,
            size_z: size,
            amp: 70.0 * p.mountainousness.clamp(0.05, 3.0),
            p,
        })
    }

    /// World-space center of chunk (cx, cz). The world spans
    /// [-size/2, size/2] so the map is centered on the origin.
    /// Edge coordinates land on exact float values (chunk sizes are
    /// integer-ish), which is what makes shared edges bit-identical.
    pub fn chunk_origin(&self, cx: u32, cz: u32) -> (f32, f32) {
        (
            (cx as f32 + 0.5) * self.p.chunk_size - self.size_x * 0.5,
            (cz as f32 + 0.5) * self.p.chunk_size - self.size_z * 0.5,
        )
    }

    /// Terrain height in meters at a world position. Pure function of
    /// (wx, wz, seed) — the heart of the seam law.
    pub fn height(&self, wx: f32, wz: f32) -> f32 {
        let n = &self.n;
        let amp = self.amp;
        // continent swells (~2.4 km wavelength): decides land vs lowland
        let cont = n.fbm(wx / 2400.0, wz / 2400.0, 4, 2.0, 0.5);
        // rolling hills, domain-warped for organic flow (~420 m)
        let hills = n.warped_fbm(wx / 420.0 + 7.3, wz / 420.0 - 3.1, 5, 0.8);
        // ridged crests, masked to high-continent areas (~900 m)
        let ridge = n.ridged(wx / 900.0 + 13.7, wz / 900.0 + 4.2, 5) - 0.65;
        let mask = ((cont + hills * 0.35 - 0.02) * 2.2).clamp(0.0, 1.0);
        // fine relief so plains never read as vinyl (~60 m)
        let detail = n.fbm(wx / 60.0 + 31.0, wz / 60.0 + 17.0, 3, 2.0, 0.5);
        let h = cont * amp * 0.55 + hills * amp * 0.30 + ridge * mask * amp * 1.25 + detail * 3.0;
        // steepen the crossing through the waterline so shores read as crisp
        // banks instead of a z-fighting speckle band (pure function of h)
        let d = h - self.p.sea_level;
        let w = 3.0;
        if d.abs() < w { self.p.sea_level + w * d.signum() * (d.abs() / w).powf(0.6) } else { h }
    }

    /// Ground albedo at a world position given its height and slope
    /// (slope = |dh| per meter). Pure function of world coords.
    pub fn color(&self, wx: f32, wz: f32, h: f32, slope: f32) -> Vec3 {
        let pal = &self.pal;
        // fast initial rise: lowlands read as grass, only the shore stays sand
        let t = ((h - self.p.sea_level) / (self.amp * 1.5)).clamp(0.0, 1.0).powf(0.55);
        let mut c = ramp(&pal.terrain[0..4], t);
        let rockiness = ((slope - 0.55) * 2.0).clamp(0.0, 1.0);
        c = c * (1.0 - rockiness) + pal.terrain[4] * rockiness;
        let snow = ((t - 0.74) * 8.0).clamp(0.0, 1.0) * (1.0 - rockiness * 0.6);
        c = c * (1.0 - snow) + pal.terrain[5] * snow;
        let shore = (1.0 - ((h - self.p.sea_level).abs() / 2.2)).clamp(0.0, 1.0);
        c = c * (1.0 - shore * 0.7) + pal.terrain[0] * shore * 0.7;
        vary(c, 0.10, self.n.sample(wx * 0.13 + 31.0, wz * 0.13 + 17.0) * 0.5 + 0.5)
    }

    /// Stable per-chunk RNG seed (independent of build order).
    pub fn chunk_seed(&self, cx: u32, cz: u32) -> u64 {
        self.p
            .seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((cx as u64) << 32 | cz as u64)
            .wrapping_mul(0xBF58_476D_1CE4_E5B9)
    }
}
