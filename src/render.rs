//! Headless software rasterizer: renders an [`Asset`] to PNG so quality can
//! be verified without a GPU. Perspective camera, z-buffer, Lambert +
//! hemisphere ambient + rim lighting, 2x supersampling, gamma-correct output.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};
use std::io::BufWriter;
use std::path::Path;

use crate::gltf::Asset;
use crate::palette::to_srgb8;

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub fov_y: f32,
}

struct Framebuffer {
    w: usize,
    color: Vec<Vec3>,
    depth: Vec<f32>,
}

const SUN_DIR: Vec3 = Vec3::new(-0.45, 0.8, 0.35);
const SKY: Vec3 = Vec3::new(0.42, 0.55, 0.70);
const GROUND_BOUNCE: Vec3 = Vec3::new(0.22, 0.19, 0.16);

fn background(x: f32, y: f32) -> Vec3 {
    // vertical gradient with subtle vignette
    let t = y;
    let top = Vec3::new(0.09, 0.11, 0.16);
    let bottom = Vec3::new(0.16, 0.17, 0.20);
    let mut c = top + (bottom - top) * t;
    let dx = x - 0.5;
    let dy = y - 0.5;
    c *= 1.0 - 0.35 * (dx * dx + dy * dy);
    c
}

fn shade(n: Vec3, albedo: Vec3, emissive: Vec3, view: Vec3, metallic: f32, roughness: f32) -> Vec3 {
    let sun = SUN_DIR.normalize();
    let ndl = n.dot(sun).max(0.0);
    // hemisphere ambient
    let hemi = SKY * (n.y * 0.5 + 0.5) + GROUND_BOUNCE * (1.0 - (n.y * 0.5 + 0.5));
    // Blinn-Phong specular scaled by material params
    let h = (sun + view).normalize();
    let spec_pow = 8.0 + (1.0 - roughness) * 120.0;
    let spec = n.dot(h).max(0.0).powf(spec_pow) * (0.06 + metallic * 0.5 + (1.0 - roughness) * 0.25);
    // rim for silhouette pop
    let rim = (1.0 - n.dot(view).max(0.0)).powf(3.0) * 0.18;
    let direct = Vec3::splat(1.15) * ndl;
    albedo * (direct + hemi * 0.55) + Vec3::splat(spec) * ndl.max(0.15) + SKY * rim + emissive * 1.4
}

fn rasterize(asset: &Asset, cam: &Camera, w: usize, h: usize) -> Framebuffer {
    let mut fb = Framebuffer {
        w,
        color: (0..w * h)
            .map(|i| background((i % w) as f32 / w as f32, (i / w) as f32 / h as f32))
            .collect(),
        depth: vec![f32::INFINITY; w * h],
    };
    let aspect = w as f32 / h as f32;
    let view_m = Mat4::look_at_rh(cam.eye, cam.target, Vec3::Y);
    let proj = Mat4::perspective_rh(cam.fov_y, aspect, 0.05, 500.0);
    let vp = proj * view_m;

    for part in &asset.parts {
        let m = &part.mesh;
        let mat = &part.material;
        let tex = mat.texture.as_deref().filter(|_| m.has_uvs());
        // pre-transform to clip space
        let clip: Vec<Vec4> = m.positions.iter().map(|p| vp * p.extend(1.0)).collect();
        for tri in m.indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let (c0, c1, c2) = (clip[i0], clip[i1], clip[i2]);
            // reject triangles behind the near plane (no clipping — cameras
            // are framed so geometry stays in front)
            if c0.w <= 0.01 || c1.w <= 0.01 || c2.w <= 0.01 {
                continue;
            }
            let ndc = |c: Vec4| c.xyz() / c.w;
            let (p0, p1, p2) = (ndc(c0), ndc(c1), ndc(c2));
            let sx = |p: Vec3| (p.x * 0.5 + 0.5) * w as f32;
            let sy = |p: Vec3| (0.5 - p.y * 0.5) * h as f32;
            let (x0, y0) = (sx(p0), sy(p0));
            let (x1, y1) = (sx(p1), sy(p1));
            let (x2, y2) = (sx(p2), sy(p2));
            let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
            if area.abs() < 1e-6 {
                continue;
            }
            // backface culling only for single-sided materials
            if !mat.double_sided && area > 0.0 {
                continue;
            }
            let min_x = x0.min(x1).min(x2).floor().max(0.0) as usize;
            let max_x = (x0.max(x1).max(x2).ceil() as usize).min(w.saturating_sub(1));
            let min_y = y0.min(y1).min(y2).floor().max(0.0) as usize;
            let max_y = (y0.max(y1).max(y2).ceil() as usize).min(h.saturating_sub(1));
            if min_x > max_x || min_y > max_y {
                continue;
            }
            let inv_area = 1.0 / area;
            let (iw0, iw1, iw2) = (1.0 / c0.w, 1.0 / c1.w, 1.0 / c2.w);
            for py in min_y..=max_y {
                for px in min_x..=max_x {
                    let fx = px as f32 + 0.5;
                    let fy = py as f32 + 0.5;
                    let w0 = ((x1 - fx) * (y2 - fy) - (x2 - fx) * (y1 - fy)) * inv_area;
                    let w1 = ((x2 - fx) * (y0 - fy) - (x0 - fx) * (y2 - fy)) * inv_area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    // perspective-correct interpolation
                    let iw = w0 * iw0 + w1 * iw1 + w2 * iw2;
                    let z = w0 * p0.z + w1 * p1.z + w2 * p2.z;
                    let idx = py * w + px;
                    if z >= fb.depth[idx] {
                        continue;
                    }
                    let pc = |a: Vec3, b: Vec3, c: Vec3| {
                        (a * w0 * iw0 + b * w1 * iw1 + c * w2 * iw2) / iw
                    };
                    let n = pc(m.normals[i0], m.normals[i1], m.normals[i2]).normalize_or(Vec3::Y);
                    let n = if mat.double_sided && area > 0.0 { -n } else { n };
                    let mut albedo = pc(m.colors[i0], m.colors[i1], m.colors[i2]);
                    let world = pc(m.positions[i0], m.positions[i1], m.positions[i2]);
                    let view = (cam.eye - world).normalize_or(Vec3::Y);
                    let (mut sn, mut rough, mut metal) = (n, mat.roughness, mat.metallic);
                    if let Some(t) = tex {
                        let uvw = |a: f32, b: f32, c: f32| {
                            (a * w0 * iw0 + b * w1 * iw1 + c * w2 * iw2) / iw
                        };
                        let u = uvw(m.uvs[i0].x, m.uvs[i1].x, m.uvs[i2].x);
                        let v = uvw(m.uvs[i0].y, m.uvs[i1].y, m.uvs[i2].y);
                        let (u, v) = (u.rem_euclid(1.0), v.rem_euclid(1.0));
                        albedo *= crate::palette::srgb_to_linear(t.base_color.sample(u, v));
                        let orm = t.orm.sample(u, v);
                        albedo *= orm.x;
                        rough = orm.y;
                        metal = orm.z;
                        // tangent-space normal mapping
                        let t4 = pc(
                            Vec3::new(m.tangents[i0].x, m.tangents[i0].y, m.tangents[i0].z),
                            Vec3::new(m.tangents[i1].x, m.tangents[i1].y, m.tangents[i1].z),
                            Vec3::new(m.tangents[i2].x, m.tangents[i2].y, m.tangents[i2].z),
                        );
                        let tn = (t4 - n * t4.dot(n)).normalize_or(n.any_orthonormal_vector());
                        let bn = n.cross(tn) * m.tangents[i0].w;
                        let nm = t.normal.sample(u, v) * 2.0 - Vec3::ONE;
                        sn = (tn * nm.x + bn * nm.y + n * nm.z).normalize_or(n);
                    }
                    fb.depth[idx] = z;
                    fb.color[idx] = shade(sn, albedo, mat.emissive, view, metal, rough);
                }
            }
        }
    }
    fb
}

/// Render with 2x supersampling and write a PNG.
pub fn render_png(
    asset: &Asset,
    cam: &Camera,
    width: usize,
    height: usize,
    path: &Path,
) -> std::io::Result<()> {
    let fb = rasterize(asset, cam, width * 2, height * 2);
    let mut pixels = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            let mut c = Vec3::ZERO;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                c += fb.color[(y * 2 + dy) * fb.w + (x * 2 + dx)];
            }
            // simple filmic-ish tonemap then sRGB
            let c = c / 4.0;
            let mapped = c / (c + Vec3::splat(0.9)) * 1.35;
            pixels.extend_from_slice(&to_srgb8(mapped));
        }
    }
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(&pixels)?;
    Ok(())
}

/// Frame an asset automatically: orbit at `yaw_deg` and `pitch_deg`,
/// distance chosen from the bounding sphere.
pub fn auto_camera(asset: &Asset, yaw_deg: f32, pitch_deg: f32, zoom: f32) -> Camera {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for p in &asset.parts {
        let (l, h) = p.mesh.bounds();
        lo = lo.min(l);
        hi = hi.max(h);
    }
    let center = (lo + hi) / 2.0;
    let radius = (hi - lo).length() / 2.0;
    let fov = 40f32.to_radians();
    let dist = radius / (fov / 2.0).tan() * 1.15 * zoom;
    let yaw = yaw_deg.to_radians();
    let pitch = pitch_deg.to_radians();
    let dir = Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    );
    Camera { eye: center + dir * dist, target: center, fov_y: fov }
}
