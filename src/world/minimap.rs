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
                c *= 0.85 + t * 0.35;
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
    // river + road overlays
    let to_px = |wx: f32, wz: f32| -> (f32, f32) {
        (
            (wx + m.size_x * 0.5) / m.size_x * w as f32,
            (wz + m.size_z * 0.5) / m.size_z * h as f32,
        )
    };
    let mut draw_poly = |points: &[glam::Vec3], color: [u8; 3], r: f32| {
        for seg in points.windows(2) {
            let (x0, y0) = to_px(seg[0].x, seg[0].z);
            let (x1, y1) = to_px(seg[1].x, seg[1].z);
            let steps = ((x1 - x0).hypot(y1 - y0).ceil() as usize).max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                dot(
                    &mut out,
                    w,
                    h,
                    x0 + (x1 - x0) * t,
                    y0 + (y1 - y0) * t,
                    r,
                    color,
                );
            }
        }
    };
    for p in &m.network.rivers {
        draw_poly(&p.points, [64, 132, 176], 1.6);
    }
    for p in &m.network.roads {
        draw_poly(&p.points, [148, 106, 62], 1.1);
    }
    for b in &m.network.bridges {
        let (px, py) = to_px(b.pos.x, b.pos.y);
        dot(&mut out, w, h, px, py, 2.4, [235, 225, 200]);
    }
    // POI markers (outlined dots, color per kind)
    for s in &m.pois {
        let px = (s.x + m.size_x * 0.5) / m.size_x * w as f32;
        let py = (s.z + m.size_z * 0.5) / m.size_z * h as f32;
        let (col, r): ([u8; 3], f32) = match s.kind {
            super::poi::PoiKind::City => ([226, 48, 44], 6.0),
            super::poi::PoiKind::Village => ([238, 150, 46], 4.0),
            super::poi::PoiKind::Castle => ([164, 70, 224], 5.0),
            super::poi::PoiKind::Watchtower => ([245, 245, 245], 3.0),
            super::poi::PoiKind::Dungeon => ([20, 20, 24], 4.0),
            super::poi::PoiKind::Boss => ([255, 32, 96], 6.5),
        };
        dot(&mut out, w, h, px, py, r + 1.5, [250, 250, 250]);
        dot(&mut out, w, h, px, py, r, col);
    }
    (w, h, out)
}

/// Paint a filled disc onto a rendered map (POI markers etc.).
pub fn dot(img: &mut [u8], w: usize, h: usize, x: f32, y: f32, r: f32, color: [u8; 3]) {
    let (cx, cy) = (x, y);
    let r2 = r * r;
    let (x0, x1) = (
        ((cx - r).floor() as i64).max(0),
        ((cx + r).ceil() as i64).min(w as i64 - 1),
    );
    let (y0, y1) = (
        ((cy - r).floor() as i64).max(0),
        ((cy + r).ceil() as i64).min(h as i64 - 1),
    );
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
