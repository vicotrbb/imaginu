//! The deterministic world model. Heights, colors and zones are pure
//! functions of world coordinates + seed — the seam law. Nothing here may
//! depend on which chunk is being built or in what order.

use glam::Vec3;

use crate::noise::Noise2;
use crate::palette::{self, Palette, ramp, vary};

use super::WorldParams;
use super::zones::{KINDS, NK, ZoneField, ZoneKind, ground_ramp};

pub struct WorldModel {
    pub p: WorldParams,
    pub pal: Palette,
    pub n: Noise2,
    pub zones: ZoneField,
    /// Placed POI sites (computed once from the recipe; part of the pure
    /// world function — all chunks see the same list).
    pub pois: Vec<super::poi::PoiSite>,
    /// Road/river polylines + bridges (computed once, like POIs).
    pub network: super::network::Network,
    /// Global erosion delta field (computed once, Catmull-Rom upsampled).
    pub erosion: Option<super::erosion::ErosionField>,
    /// Grid dimensions in chunks.
    pub nx: u32,
    pub nz: u32,
    /// Total world extent in meters (size snapped to whole chunks).
    pub size_x: f32,
    pub size_z: f32,
    /// Master height amplitude in meters.
    pub amp: f32,
}

/// Soft terracing: stepped strata with cubic-eased risers (C1 inside steps).
fn soft_steps(h: f32, step: f32) -> f32 {
    if step <= 1e-6 {
        return h;
    }
    let q = (h / step).floor() * step;
    q + ((h - q) / step).powi(3) * step
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
        // integer-meter chunk sizes keep edge world-coordinates exact in f32,
        // which is what makes shared chunk edges bit-identical
        p.chunk_size = p.chunk_size.clamp(32.0, 1024.0).round();
        p.chunk_resolution = p.chunk_resolution.clamp(16, 512);
        let n_chunks = (p.size / p.chunk_size).round().clamp(1.0, 64.0) as u32;
        let size = n_chunks as f32 * p.chunk_size;
        let specs = p.pois.clone();
        let mut model = Self {
            pal: palette::by_name(&p.palette),
            n: Noise2::new(p.seed),
            zones: ZoneField::new(p.seed, &p.zones, p.zone_size),
            pois: Vec::new(),
            network: super::network::Network::empty(),
            erosion: None,
            nx: n_chunks,
            nz: n_chunks,
            size_x: size,
            size_z: size,
            amp: 70.0 * p.mountainousness.clamp(0.05, 3.0),
            p,
        };
        // global erosion: erode a coarse world heightmap once, keep the
        // delta — gullies and sediment fans span chunks seamlessly
        if model.p.erosion > 0.0 {
            let size = model.size_x;
            let step = (size / 384.0).clamp(8.0, 24.0);
            let n = (size / step).ceil() as i32 + 1;
            let mut grid = vec![0.0f32; (n * n) as usize];
            for jz in 0..n {
                for jx in 0..n {
                    grid[(jz * n + jx) as usize] =
                        model.height(jx as f32 * step - size * 0.5, jz as f32 * step - size * 0.5);
                }
            }
            let before = grid.clone();
            super::erosion::erode_global(
                &mut grid,
                n as usize,
                model.p.erosion.clamp(0.0, 1.0),
                model.p.seed,
            );
            let delta: Vec<f32> = grid.iter().zip(&before).map(|(a, b)| a - b).collect();
            model.erosion = Some(super::erosion::ErosionField {
                step,
                n,
                half: size * 0.5,
                delta,
            });
        }
        // order matters: rivers first (hydrology), then POIs avoid rivers
        // and flatten the ground, then roads connect POIs (bridging rivers)
        let km2 = (model.size_x / 1000.0) * (model.size_z / 1000.0);
        let n_rivers = model
            .p
            .rivers
            .unwrap_or(((km2 / 6.0).ceil() as u32).min(12));
        model.network = super::network::build_rivers(&model, n_rivers);
        model.pois = super::poi::place(&model, specs.as_deref());
        model.network = super::network::with_roads(&model);
        Ok(model)
    }

    /// World-space center of chunk (cx, cz). The world spans
    /// [-size/2, size/2] so the map is centered on the origin.
    pub fn chunk_origin(&self, cx: u32, cz: u32) -> (f32, f32) {
        (
            (cx as f32 + 0.5) * self.p.chunk_size - self.size_x * 0.5,
            (cz as f32 + 0.5) * self.p.chunk_size - self.size_z * 0.5,
        )
    }

    /// Terrain height in meters at a world position: shared spectral fields
    /// combined per-zone, blended by the smooth zone weights. Pure function
    /// of (wx, wz, seed) — the heart of the seam law.
    pub fn height(&self, wx: f32, wz: f32) -> f32 {
        let zw = self.zones.weights(wx, wz);
        self.height_with_weights(wx, wz, &zw)
    }

    pub fn height_with_weights(&self, wx: f32, wz: f32, zw: &[f32; NK]) -> f32 {
        let n = &self.n;
        let amp = self.amp;
        let sea = self.p.sea_level;
        // shared spectral fields, each sampled once and reused by all zones
        let cont = n.fbm(wx / 2400.0, wz / 2400.0, 4, 2.0, 0.5);
        let hills = n.warped_fbm(wx / 420.0 + 7.3, wz / 420.0 - 3.1, 5, 0.8);
        let ridge = (n.ridged(wx / 900.0 + 13.7, wz / 900.0 + 4.2, 5) - 0.35).max(0.0);
        let detail = n.fbm(wx / 60.0 + 31.0, wz / 60.0 + 17.0, 3, 2.0, 0.5);
        // long anisotropic dune ridges
        let dune = 1.0 - n.sample(wx / 95.0 + wz / 30.0, wz / 260.0).abs();
        let mesa = n.fbm(wx / 520.0 + 87.0, wz / 520.0 + 13.0, 3, 2.0, 0.5);

        let zi = |k: ZoneKind| zw[k.index()];
        let mut h = sea + cont * amp * 0.10 + detail * 2.5;
        // crags: mid-frequency ridging so single chunks show real relief
        let crag = (n.ridged(wx / 210.0 + 51.3, wz / 210.0 + 9.8, 4) - 0.40).max(0.0);
        // micro-relief (~13 m) so smooth-shaded ground never reads as wax:
        // strong on mountain rock, subtle undulation elsewhere
        let micro = n.fbm(wx / 13.0 + 7.0, wz / 13.0 + 3.0, 3, 2.0, 0.5);
        h += zi(ZoneKind::Mountains)
            * (amp * 0.45
                + hills * amp * 0.25
                + ridge * amp * 1.55
                + crag * amp * 0.45 * (0.35 + ridge)
                + cont * amp * 0.20);
        h += zi(ZoneKind::Forest) * (10.0 + hills * amp * 0.26 + cont * amp * 0.10);
        h += zi(ZoneKind::Plains) * (7.0 + hills * amp * 0.09 + cont * amp * 0.05);
        h += zi(ZoneKind::Desert) * (9.0 + dune * amp * 0.16 + hills * amp * 0.05);
        // swamp hovers just above sea level: hummocks + pools
        h += zi(ZoneKind::Swamp) * (1.6 + hills * 1.6 + detail * 1.4);
        // lake scoops a bowl below sea level; the global sea plane fills it
        h += zi(ZoneKind::Lake) * (-14.0 + hills * 2.0 - cont * 3.0);
        // coast dips below sea where the continent field is low → bays
        h += zi(ZoneKind::Coast) * (2.0 + hills * amp * 0.05 + cont * amp * 0.08);
        h += zi(ZoneKind::Badlands)
            * (10.0
                + soft_steps(
                    ((mesa + 0.35) * 1.4).clamp(0.0, 1.3) * amp * 0.45,
                    amp * 0.11,
                ));
        h += micro
            * (0.7
                + (zi(ZoneKind::Mountains) * (1.0 + ridge)) * 3.2
                + zi(ZoneKind::Badlands) * 1.6);
        // steepen the crossing through the waterline so shores read as crisp
        // banks instead of a z-fighting speckle band (pure function of h)
        let d = h - sea;
        let w = 3.0;
        let mut h = if d.abs() < w {
            sea + w * d.signum() * (d.abs() / w).powf(0.6)
        } else {
            h
        };
        // world-scale erosion delta (C1 upsample of the global field)
        if let Some(e) = &self.erosion {
            h += e.sample(wx, wz);
        }
        // POI flattening: blend toward each site's plateau with a smooth
        // skirt — part of the world function, so a city split across four
        // chunks stays seamless
        for s in &self.pois {
            if matches!(s.kind, super::poi::PoiKind::Dungeon) {
                continue;
            }
            let (dx, dz) = (wx - s.x, wz - s.z);
            let r_out = s.radius * 1.7;
            let d2 = dx * dx + dz * dz;
            if d2 < r_out * r_out {
                let t = ((r_out - d2.sqrt()) / (r_out - s.radius)).clamp(0.0, 1.0);
                let t = t * t * (3.0 - 2.0 * t);
                h += (s.ground - h) * t * 0.96;
            }
        }
        // rivers carve, roads embank (pure lookups into the fixed network)
        self.network.apply_height(wx, wz, h)
    }

    /// Ground albedo at a world position given its height and slope
    /// (slope = |dh| per meter). Zone ground ramps blended by the same
    /// smooth weights as the heights.
    pub fn color(&self, wx: f32, wz: f32, h: f32, slope: f32) -> Vec3 {
        let zw = self.zones.weights(wx, wz);
        let pal = &self.pal;
        let sea = self.p.sea_level;
        // normalize altitude over the real zone height span (mountain peaks
        // reach ~3.5·amp); fast initial rise so lowlands read as ground
        let t = ((h - sea) / (self.amp * 3.5)).clamp(0.0, 1.0).powf(0.5);
        let mut c = Vec3::ZERO;
        for (i, k) in KINDS.iter().enumerate() {
            if zw[i] > 1e-4 {
                c += ramp(&ground_ramp(*k), t) * zw[i];
            }
        }
        // cliffs go dark rock; threshold high enough that ordinary mountain
        // flanks keep their zone color (only real crags flip)
        let rockiness = ((slope - 0.85) * 1.8).clamp(0.0, 1.0);
        c = c * (1.0 - rockiness * 0.85) + pal.rock[1] * 0.9 * rockiness * 0.85;
        // snow: an absolute elevation band, so only true peaks whiten no
        // matter how tall the massif gets
        let snow = ((h - sea - self.amp * 2.1) / (self.amp * 0.5)).clamp(0.0, 1.0)
            * (1.0 - rockiness * 0.6)
            * (zw[ZoneKind::Mountains.index()] * 1.6).clamp(0.0, 1.0);
        c = c * (1.0 - snow) + pal.terrain[5] * snow;
        // cliff strata: horizontal banding on rocky faces
        if rockiness > 0.05 {
            let band =
                ((h * 0.55 + self.n.sample(wx * 0.02 + 9.0, wz * 0.02) * 6.0).sin()) * 0.5 + 0.5;
            let strata = crate::palette::lerp(pal.rock[1] * 0.8, pal.rock[0] * 1.08, band);
            c = crate::palette::lerp(c, strata, rockiness * 0.55);
        }
        // dry-grass patches break up grassland monotony
        let grassy = zw[ZoneKind::Plains.index()]
            + zw[ZoneKind::Forest.index()]
            + zw[ZoneKind::Lake.index()]
            + zw[ZoneKind::Coast.index()];
        if grassy > 0.3 && rockiness < 0.3 {
            let patch =
                (self.n.fbm(wx / 23.0 + 5.0, wz / 23.0 - 2.0, 2, 2.0, 0.5) * 1.6).clamp(0.0, 1.0);
            c = crate::palette::lerp(c, c * Vec3::new(1.16, 1.06, 0.70), patch * grassy * 0.45);
        }
        // dune ripple shading
        let desert = zw[ZoneKind::Desert.index()];
        if desert > 0.2 {
            let ripple = self.n.sample(wx / 7.0 + wz / 2.3, wz / 34.0);
            c *= 1.0 + ripple * 0.06 * desert;
        }
        // shoreline foam: a thin, broken surf line right at the waterline —
        // flat beaches only (wide banks turned into chalky margins)
        let foam = (1.0 - ((h - sea - 0.12) / 0.16).abs()).clamp(0.0, 1.0)
            * (1.0 - (slope / 0.25).min(1.0));
        if foam > 0.0 {
            let fnz = (self.n.sample(wx * 0.9 + 3.0, wz * 0.9) - 0.15).max(0.0);
            c = crate::palette::lerp(c, Vec3::splat(0.82), foam * fnz * 0.28);
        }
        let shore = (1.0 - ((h - sea).abs() / 2.2)).clamp(0.0, 1.0);
        c = c * (1.0 - shore * 0.55) + pal.terrain[0] * shore * 0.55;
        // dirt roads
        let rm = self.network.road_mask(wx, wz);
        if rm > 0.0 {
            c = crate::palette::lerp(c, pal.trunk * 0.8, rm * 0.7);
        }
        // packed-dirt ground inside settlements
        for s in &self.pois {
            if matches!(
                s.kind,
                super::poi::PoiKind::City
                    | super::poi::PoiKind::Village
                    | super::poi::PoiKind::Castle
            ) {
                let (dx, dz) = (wx - s.x, wz - s.z);
                let r_out = s.radius * 1.15;
                let d2 = dx * dx + dz * dz;
                if d2 < r_out * r_out {
                    let t = ((r_out - d2.sqrt()) / (r_out * 0.35)).clamp(0.0, 1.0);
                    c = crate::palette::lerp(c, pal.trunk * 0.85, t * 0.55);
                }
            }
        }
        vary(
            c,
            0.10,
            self.n.sample(wx * 0.13 + 31.0, wz * 0.13 + 17.0) * 0.5 + 0.5,
        )
    }

    /// Per-chunk mesh resolution: a pure function of chunk coords, so any
    /// chunk can compute its neighbor's choice and stitch edges without
    /// cracks. Flat plains halve, mountains/POIs/networks double.
    pub fn chunk_res(&self, cx: u32, cz: u32) -> u32 {
        let base = self.p.chunk_resolution;
        if !self.p.adaptive_resolution {
            return base;
        }
        let (ox, oz) = self.chunk_origin(cx, cz);
        let cs = self.p.chunk_size;
        let near_poi = self.pois.iter().any(|s| {
            (s.x - ox).abs() < cs * 0.5 + s.radius * 1.7
                && (s.z - oz).abs() < cs * 0.5 + s.radius * 1.7
        });
        // roughness probe: 9×9 heights
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        let mut steep = 0.0f32;
        let mut prev = 0.0;
        for jz in 0..=8 {
            for jx in 0..=8 {
                let h = self.height(
                    ox + (jx as f32 / 8.0 - 0.5) * cs,
                    oz + (jz as f32 / 8.0 - 0.5) * cs,
                );
                lo = lo.min(h);
                hi = hi.max(h);
                if jx > 0 {
                    steep = steep.max((h - prev).abs() / (cs / 8.0));
                }
                prev = h;
            }
        }
        let relief = hi - lo;
        let has_net =
            self.network.river_within(ox, oz, cs * 0.75) || self.network.road_mask(ox, oz) > 0.0;
        if near_poi || relief > self.amp * 0.8 || steep > 0.6 {
            (base * 2).min(256)
        } else if relief < self.amp * 0.10 && steep < 0.12 && !has_net {
            (base / 2).max(32)
        } else {
            base
        }
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
