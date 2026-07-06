//! Top-down minimap PNG: zone tints, hillshaded heights, water — the
//! primary debugging view for world layout. Deterministic.

use std::io::BufWriter;
use std::path::Path;

use glam::Vec3;

use super::model::WorldModel;
use super::zones::{KINDS, map_color};

/// Render the map as RGB8. One height sample per pixel; hillshade from the
/// height buffer so the whole thing costs O(pixels).
pub fn render(m: &WorldModel, px: usize) -> (usize, usize, Vec<u8>) {
    let (w, h) = (px.max(64), px.max(64));
    let step = m.size_x / w as f32;
    let at = |i: usize, j: usize| -> (f32, f32) {
        (
            (i as f32 + 0.5) * step - m.size_x * 0.5,
            (j as f32 + 0.5) * (m.size_z / h as f32) - m.size_z * 0.5,
        )
    };
    let mut heights = vec![0.0f32; w * h];
    for j in 0..h {
        for i in 0..w {
            let (wx, wz) = at(i, j);
            heights[j * w + i] = m.height(wx, wz);
        }
    }
    let sea = m.p.sea_level;
    let light = Vec3::new(-0.55, 0.75, -0.35).normalize();
    let mut out = vec![0u8; w * h * 3];
    for j in 0..h {
        for i in 0..w {
            let (wx, wz) = at(i, j);
            let hv = heights[j * w + i];
            let mut c;
            if hv < sea {
                // depth-shaded water
                let depth = ((sea - hv) / 12.0).clamp(0.0, 1.0);
                c = m.pal.water * (1.0 - depth * 0.55) + Vec3::new(0.0, 0.02, 0.05) * depth;
            } else {
                let zw = m.zones.weights(wx, wz);
                c = Vec3::ZERO;
                for (k, kind) in KINDS.iter().enumerate() {
                    if zw[k] > 1e-4 {
                        c += map_color(*kind) * zw[k];
                    }
                }
                // altitude lightening + snow tips
                let t = ((hv - sea) / (m.amp * 3.5)).clamp(0.0, 1.0).powf(0.5);
                c = c * (0.85 + t * 0.35);
                let snow = ((hv - sea - m.amp * 2.1) / (m.amp * 0.5)).clamp(0.0, 1.0);
                c = c.lerp(Vec3::splat(0.95), snow * 0.8);
                // hillshade from the height buffer
                let gx = heights[j * w + (i + 1).min(w - 1)] - heights[j * w + i.saturating_sub(1)];
                let gz = heights[(j + 1).min(h - 1) * w + i] - heights[j.saturating_sub(1) * w + i];
                let n = Vec3::new(-gx, 2.0 * step, -gz).normalize();
                let shade = n.dot(light).clamp(0.0, 1.0);
                c *= 0.55 + shade * 0.6;
            }
            let idx = (j * w + i) * 3;
            // gamma for display
            out[idx] = (c.x.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            out[idx + 1] = (c.y.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            out[idx + 2] = (c.z.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
        }
    }
    (w, h, out)
}

/// Paint a filled disc onto a rendered map (POI markers etc.).
pub fn dot(img: &mut [u8], w: usize, h: usize, x: f32, y: f32, r: f32, color: [u8; 3]) {
    let (cx, cy) = (x, y);
    let r2 = r * r;
    let (x0, x1) = (((cx - r).floor() as i64).max(0), ((cx + r).ceil() as i64).min(w as i64 - 1));
    let (y0, y1) = (((cy - r).floor() as i64).max(0), ((cy + r).ceil() as i64).min(h as i64 - 1));
    for py in y0..=y1 {
        for px in x0..=x1 {
            let d2 = (px as f32 - cx).powi(2) + (py as f32 - cy).powi(2);
            if d2 <= r2 {
                let idx = (py as usize * w + px as usize) * 3;
                img[idx] = color[0];
                img[idx + 1] = color[1];
                img[idx + 2] = color[2];
            }
        }
    }
}

pub fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgb).map_err(|e| e.to_string())?;
    Ok(())
}
