//! World-scale hydraulic erosion. A coarse global heightmap is eroded ONCE
//! (deterministic droplet sim, per-cell displacement budgets), and the
//! resulting delta field is upsampled with Catmull-Rom interpolation inside
//! the pure world height function — so erosion features (gullies, fans)
//! span chunk borders without breaking edge identity.

use crate::generators::{range, rng};

pub struct ErosionField {
    pub step: f32,
    pub n: i32,
    pub half: f32,
    pub delta: Vec<f32>,
}

impl ErosionField {
    /// Catmull-Rom (C1) upsample of the delta grid at a world position.
    pub fn sample(&self, wx: f32, wz: f32) -> f32 {
        let n = self.n;
        let gx = ((wx + self.half) / self.step).clamp(0.0, (n - 1) as f32 - 1e-3);
        let gz = ((wz + self.half) / self.step).clamp(0.0, (n - 1) as f32 - 1e-3);
        let (ix, iz) = (gx.floor() as i32, gz.floor() as i32);
        let (fx, fz) = (gx - ix as f32, gz - iz as f32);
        let at = |x: i32, z: i32| -> f32 {
            self.delta[(z.clamp(0, n - 1) * n + x.clamp(0, n - 1)) as usize]
        };
        let cr = |p0: f32, p1: f32, p2: f32, p3: f32, t: f32| -> f32 {
            0.5 * ((2.0 * p1)
                + (-p0 + p2) * t
                + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
                + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
        };
        let mut rows = [0.0f32; 4];
        for (r, row) in rows.iter_mut().enumerate() {
            let z = iz - 1 + r as i32;
            *row = cr(at(ix - 1, z), at(ix, z), at(ix + 1, z), at(ix + 2, z), fx);
        }
        cr(rows[0], rows[1], rows[2], rows[3], fz)
    }
}

/// Droplet erosion on a global grid (n×n, row-major). Same guard rails as
/// the chunk-local sim: normalized heights, per-step caps, per-cell budget.
pub fn erode_global(grid: &mut [f32], n: usize, amount: f32, seed: u64) {
    let mut r = rng(seed ^ 0x6E0B_A1);
    let h_min = grid.iter().cloned().fold(f32::INFINITY, f32::min);
    let h_max = grid.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let span = (h_max - h_min).max(1e-6);
    for h in grid.iter_mut() {
        *h = (*h - h_min) / span;
    }
    let orig: Vec<f32> = grid.to_vec();
    let budget = 0.02 + 0.03 * amount;
    let drops = (((n * n) as f32) * 1.2 * amount) as usize;
    let drops = drops.min(500_000);
    let (inertia, capacity_f, min_slope) = (0.1f32, 4.0f32, 0.01f32);
    let (deposit_f, erode_f, evaporate, gravity) = (0.3f32, 0.3f32, 0.02f32, 4.0f32);
    const MAX_STEP: f32 = 0.002;
    for _ in 0..drops {
        let mut px = range(&mut r, 1.0, (n - 2) as f32);
        let mut pz = range(&mut r, 1.0, (n - 2) as f32);
        let (mut dx, mut dz) = (0.0f32, 0.0f32);
        let mut vel = 1.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;
        for _ in 0..64 {
            let (ix, iz) = (px as usize, pz as usize);
            if ix >= n - 1 || iz >= n - 1 {
                break;
            }
            let (fx, fz) = (px - ix as f32, pz - iz as f32);
            let g = |x: usize, z: usize| grid[z * n + x];
            let (h00, h10, h01, h11) = (g(ix, iz), g(ix + 1, iz), g(ix, iz + 1), g(ix + 1, iz + 1));
            let grad_x = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
            let grad_z = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;
            let h_old = h00 * (1.0 - fx) * (1.0 - fz)
                + h10 * fx * (1.0 - fz)
                + h01 * (1.0 - fx) * fz
                + h11 * fx * fz;
            dx = dx * inertia - grad_x * (1.0 - inertia);
            dz = dz * inertia - grad_z * (1.0 - inertia);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-8 {
                break;
            }
            dx /= len;
            dz /= len;
            px += dx;
            pz += dz;
            if px < 1.0 || pz < 1.0 || px >= (n - 2) as f32 || pz >= (n - 2) as f32 {
                break;
            }
            let (jx, jz) = (px as usize, pz as usize);
            let (gx, gz) = (px - jx as f32, pz - jz as f32);
            let h_new = g(jx, jz) * (1.0 - gx) * (1.0 - gz)
                + g(jx + 1, jz) * gx * (1.0 - gz)
                + g(jx, jz + 1) * (1.0 - gx) * gz
                + g(jx + 1, jz + 1) * gx * gz;
            let dh = h_new - h_old;
            let capacity = (-dh).max(min_slope) * vel * water * capacity_f;
            let splat = |grid: &mut [f32], amt: f32| {
                for (dxi, dzi, w) in [
                    (0i64, 0i64, 0.40f32),
                    (1, 0, 0.10),
                    (-1, 0, 0.10),
                    (0, 1, 0.10),
                    (0, -1, 0.10),
                    (1, 1, 0.05),
                    (-1, -1, 0.05),
                    (1, -1, 0.05),
                    (-1, 1, 0.05),
                ] {
                    let (qx, qz) = (ix as i64 + dxi, iz as i64 + dzi);
                    if qx >= 0 && qz >= 0 && qx < n as i64 && qz < n as i64 {
                        let idx = qz as usize * n + qx as usize;
                        grid[idx] = (grid[idx] + amt * w)
                            .clamp(orig[idx] - budget, orig[idx] + budget);
                    }
                }
            };
            if sediment > capacity || dh > 0.0 {
                let amt = if dh > 0.0 {
                    dh.min(sediment)
                } else {
                    (sediment - capacity) * deposit_f
                }
                .min(MAX_STEP);
                splat(grid, amt);
                sediment -= amt;
            } else {
                let amt = ((capacity - sediment) * erode_f).min(-dh).min(MAX_STEP);
                splat(grid, -amt);
                sediment += amt;
            }
            vel = (vel * vel + (-dh) * gravity).max(0.0).sqrt().min(8.0);
            water *= 1.0 - evaporate;
            if water < 0.01 {
                break;
            }
        }
    }
    // one gentle smoothing pass so gullies read as flow lines, not noise
    let snapshot = grid.to_vec();
    for iz in 1..n - 1 {
        for ix in 1..n - 1 {
            let idx = iz * n + ix;
            let sum =
                snapshot[idx - 1] + snapshot[idx + 1] + snapshot[idx - n] + snapshot[idx + n];
            grid[idx] = std::hint::black_box(snapshot[idx] * 0.6 + sum * 0.1);
        }
    }
    for h in grid.iter_mut() {
        *h = *h * span + h_min;
    }
}
