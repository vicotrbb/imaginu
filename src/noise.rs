//! Seeded gradient noise, fBm and domain warping. Hand-rolled for
//! determinism across platforms and zero heavyweight deps.

use glam::Vec2;

/// Permutation-table Perlin-style gradient noise in 2D, output roughly [-1, 1].
pub struct Noise2 {
    perm: [u8; 512],
}

impl Noise2 {
    pub fn new(seed: u64) -> Self {
        let mut p: [u8; 256] = core::array::from_fn(|i| i as u8);
        // xorshift-based Fisher-Yates so identical on every platform
        let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        for i in (1..256).rev() {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let j = (s % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm }
    }

    fn grad(&self, ix: i32, iy: i32) -> Vec2 {
        let h = self.perm[(self.perm[(ix & 255) as usize] as usize + (iy & 255) as usize) & 511];
        // 16 well-distributed unit gradients
        let a = (h as f32) * (core::f32::consts::TAU / 16.0);
        Vec2::new(a.cos(), a.sin())
    }

    pub fn sample(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (ix, iy) = (x0 as i32, y0 as i32);
        let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let (u, v) = (fade(fx), fade(fy));
        let dot = |gx: i32, gy: i32, dx: f32, dy: f32| {
            let g = self.grad(gx, gy);
            g.x * dx + g.y * dy
        };
        let n00 = dot(ix, iy, fx, fy);
        let n10 = dot(ix + 1, iy, fx - 1.0, fy);
        let n01 = dot(ix, iy + 1, fx, fy - 1.0);
        let n11 = dot(ix + 1, iy + 1, fx - 1.0, fy - 1.0);
        let nx0 = n00 + u * (n10 - n00);
        let nx1 = n01 + u * (n11 - n01);
        (nx0 + v * (nx1 - nx0)) * 1.9
    }

    /// Gradient noise with the lattice wrapped modulo `px`/`py` — output is
    /// exactly periodic with period `px`/`py` in x/y.
    fn sample_tiled(&self, x: f32, y: f32, px: i32, py: i32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let fx = x - x0;
        let fy = y - y0;
        let (ix, iy) = (x0 as i32, y0 as i32);
        let wrap = |gx: i32, gy: i32| (gx.rem_euclid(px.max(1)), gy.rem_euclid(py.max(1)));
        let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        let (u, v) = (fade(fx), fade(fy));
        let dot = |gx: i32, gy: i32, dx: f32, dy: f32| {
            let (gx, gy) = wrap(gx, gy);
            let g = self.grad(gx, gy);
            g.x * dx + g.y * dy
        };
        let n00 = dot(ix, iy, fx, fy);
        let n10 = dot(ix + 1, iy, fx - 1.0, fy);
        let n01 = dot(ix, iy + 1, fx, fy - 1.0);
        let n11 = dot(ix + 1, iy + 1, fx - 1.0, fy - 1.0);
        let nx0 = n00 + u * (n10 - n00);
        let nx1 = n01 + u * (n11 - n01);
        (nx0 + v * (nx1 - nx0)) * 1.9
    }

    /// Seamlessly tiling fBm over the unit square: `u`/`v` in [0,1),
    /// `fu`/`fv` integer base frequencies (doubled per octave, so every
    /// octave stays exactly periodic). Uniform statistics across the tile —
    /// unlike blend-based tiling, which dampens the middle.
    pub fn fbm_tiled(&self, u: f32, v: f32, fu: u32, fv: u32, octaves: u32, gain: f32) -> f32 {
        let mut amp = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        let (mut pu, mut pv) = (fu.max(1), fv.max(1));
        for _ in 0..octaves {
            sum += amp * self.sample_tiled(u * pu as f32, v * pv as f32, pu as i32, pv as i32);
            norm += amp;
            amp *= gain;
            pu = (pu * 2).min(1 << 24);
            pv = (pv * 2).min(1 << 24);
        }
        sum / norm
    }

    /// Fractal Brownian motion, `octaves` layers, output roughly [-1, 1].
    pub fn fbm(&self, x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += amp * self.sample(x * freq, y * freq);
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        sum / norm
    }

    /// Ridged multifractal — sharp mountain crests.
    pub fn ridged(&self, x: f32, y: f32, octaves: u32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        for _ in 0..octaves {
            let n = 1.0 - self.sample(x * freq, y * freq).abs();
            sum += n * n * amp;
            amp *= 0.5;
            freq *= 2.1;
        }
        sum
    }

    /// fBm evaluated through a domain warp for organic, flowing shapes.
    pub fn warped_fbm(&self, x: f32, y: f32, octaves: u32, warp: f32) -> f32 {
        let qx = self.fbm(x + 5.2, y + 1.3, 4, 2.0, 0.5);
        let qy = self.fbm(x + 9.7, y + 8.3, 4, 2.0, 0.5);
        self.fbm(x + warp * qx, y + warp * qy, octaves, 2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a = Noise2::new(42);
        let b = Noise2::new(42);
        assert_eq!(a.sample(1.37, 4.2), b.sample(1.37, 4.2));
    }

    #[test]
    fn bounded() {
        let n = Noise2::new(7);
        for i in 0..2000 {
            let v = n.fbm(i as f32 * 0.173, i as f32 * 0.091, 6, 2.0, 0.5);
            assert!(v.is_finite() && v.abs() <= 1.5, "v={v}");
        }
    }
}
