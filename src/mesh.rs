//! Mesh building blocks: an indexed triangle mesh with vertex colors,
//! optional skinning attributes, and helpers for construction/merging.

use glam::{Mat4, Vec3};

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<Vec3>,
    pub indices: Vec<u32>,
    /// Per-vertex joint indices/weights (rigid binding uses one joint at 1.0).
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn is_skinned(&self) -> bool {
        !self.joints.is_empty()
    }

    /// Push a vertex; returns its index.
    pub fn push_vertex(&mut self, p: Vec3, n: Vec3, c: Vec3) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        self.colors.push(c);
        (self.positions.len() - 1) as u32
    }

    pub fn push_tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Append a flat-shaded triangle (own vertices, face normal).
    pub fn add_flat_tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Vec3) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = self.push_vertex(a, n, color);
        let j = self.push_vertex(b, n, color);
        let k = self.push_vertex(c, n, color);
        self.push_tri(i, j, k);
    }

    /// Append a flat-shaded quad (two triangles), vertices CCW.
    pub fn add_flat_quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Vec3) {
        self.add_flat_tri(a, b, c, color);
        self.add_flat_tri(a, c, d, color);
    }

    /// Recompute smooth normals by area-weighted face accumulation.
    pub fn recompute_smooth_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.positions.len()];
        for t in self.indices.chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let n = (self.positions[b] - self.positions[a])
                .cross(self.positions[c] - self.positions[a]);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        self.normals = acc.into_iter().map(|n| n.normalize_or(Vec3::Y)).collect();
    }

    /// Transform all positions (and normals with inverse-transpose).
    pub fn transform(&mut self, m: Mat4) {
        let nm = m.inverse().transpose();
        for p in &mut self.positions {
            *p = m.transform_point3(*p);
        }
        for n in &mut self.normals {
            *n = nm.transform_vector3(*n).normalize_or(Vec3::Y);
        }
    }

    pub fn translate(&mut self, v: Vec3) {
        for p in &mut self.positions {
            *p += v;
        }
    }

    /// Merge another mesh into this one (skinning attrs padded if mixed).
    pub fn merge(&mut self, other: &Mesh) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|i| i + base));
        if self.is_skinned() || other.is_skinned() {
            self.joints.resize(base as usize, [0; 4]);
            self.weights.resize(base as usize, [1.0, 0.0, 0.0, 0.0]);
            let extra = other.positions.len();
            if other.is_skinned() {
                self.joints.extend_from_slice(&other.joints);
                self.weights.extend_from_slice(&other.weights);
            } else {
                self.joints.extend(std::iter::repeat([0; 4]).take(extra));
                self.weights.extend(std::iter::repeat([1.0, 0.0, 0.0, 0.0]).take(extra));
            }
        }
    }

    /// Rigidly bind every vertex currently in the mesh to one joint.
    pub fn bind_all_to_joint(&mut self, joint: u16) {
        self.joints = vec![[joint, 0, 0, 0]; self.positions.len()];
        self.weights = vec![[1.0, 0.0, 0.0, 0.0]; self.positions.len()];
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in &self.positions {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        (lo, hi)
    }

    /// Sanity invariants used by tests and debug assertions.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.positions.len();
        if self.normals.len() != n || self.colors.len() != n {
            return Err("attribute count mismatch".into());
        }
        if self.indices.len() % 3 != 0 {
            return Err("index count not multiple of 3".into());
        }
        for &i in &self.indices {
            if i as usize >= n {
                return Err(format!("index {i} out of bounds ({n} vertices)"));
            }
        }
        for p in &self.positions {
            if !p.is_finite() {
                return Err("non-finite position".into());
            }
        }
        for v in &self.normals {
            if !v.is_finite() {
                return Err("non-finite normal".into());
            }
        }
        if self.is_skinned() && (self.joints.len() != n || self.weights.len() != n) {
            return Err("skin attribute count mismatch".into());
        }
        Ok(())
    }
}

/// A closed "lathe": revolve a profile (radius, height) pairs around Y.
/// `segments` radial steps; smooth normals. Good for pots, trunks, bodies.
pub fn lathe(profile: &[(f32, f32)], segments: u32, color: impl Fn(usize, f32) -> Vec3) -> Mesh {
    let mut m = Mesh::new();
    let segs = segments.max(3);
    for (ri, &(r, h)) in profile.iter().enumerate() {
        for s in 0..segs {
            let a = s as f32 / segs as f32 * core::f32::consts::TAU;
            let p = Vec3::new(a.cos() * r, h, a.sin() * r);
            m.push_vertex(p, Vec3::Y, color(ri, a));
        }
    }
    for ri in 0..profile.len() - 1 {
        for s in 0..segs {
            let s1 = (s + 1) % segs;
            let a = (ri as u32) * segs + s;
            let b = (ri as u32) * segs + s1;
            let c = (ri as u32 + 1) * segs + s1;
            let d = (ri as u32 + 1) * segs + s;
            m.push_tri(a, c, b);
            m.push_tri(a, d, c);
        }
    }
    m.recompute_smooth_normals();
    m
}

/// Tapered tube following a path of (point, radius); smooth normals, capped
/// with a tip vertex at the end. Good for trunks, branches, limbs.
pub fn tube(path: &[(Vec3, f32)], segments: u32, color: impl Fn(usize) -> Vec3) -> Mesh {
    let mut m = Mesh::new();
    let segs = segments.max(3);
    // parallel-transport-ish frames
    let mut prev_x = Vec3::X;
    for (ri, &(p, r)) in path.iter().enumerate() {
        let dir = if ri + 1 < path.len() {
            (path[ri + 1].0 - p).normalize_or(Vec3::Y)
        } else {
            (p - path[ri - 1].0).normalize_or(Vec3::Y)
        };
        let x = (prev_x - dir * prev_x.dot(dir)).normalize_or(dir.any_orthonormal_vector());
        let z = dir.cross(x).normalize_or(Vec3::Z);
        prev_x = x;
        for s in 0..segs {
            let a = s as f32 / segs as f32 * core::f32::consts::TAU;
            let offset = x * a.cos() * r + z * a.sin() * r;
            m.push_vertex(p + offset, offset.normalize_or(Vec3::Y), color(ri));
        }
    }
    for ri in 0..path.len() - 1 {
        for s in 0..segs {
            let s1 = (s + 1) % segs;
            let a = (ri as u32) * segs + s;
            let b = (ri as u32) * segs + s1;
            let c = (ri as u32 + 1) * segs + s1;
            let d = (ri as u32 + 1) * segs + s;
            m.push_tri(a, b, c);
            m.push_tri(a, c, d);
        }
    }
    // cap tip
    let (tip_p, _) = *path.last().unwrap();
    let last_ring = ((path.len() - 1) as u32) * segs;
    let dir_end = (tip_p - path[path.len() - 2].0).normalize_or(Vec3::Y);
    let tip = m.push_vertex(
        tip_p + dir_end * path.last().unwrap().1,
        dir_end,
        color(path.len() - 1),
    );
    for s in 0..segs {
        let s1 = (s + 1) % segs;
        m.push_tri(last_ring + s, last_ring + s1, tip);
    }
    m.recompute_smooth_normals();
    m
}

/// Rebuild with per-face vertices and face normals (faceted low-poly look).
pub fn to_flat_shaded(src: &Mesh) -> Mesh {
    let mut m = Mesh::new();
    for t in src.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let col = (src.colors[a] + src.colors[b] + src.colors[c]) / 3.0;
        m.add_flat_tri(src.positions[a], src.positions[b], src.positions[c], col);
    }
    if src.is_skinned() {
        m.joints = src
            .indices
            .iter()
            .map(|&i| src.joints[i as usize])
            .collect();
        m.weights = src
            .indices
            .iter()
            .map(|&i| src.weights[i as usize])
            .collect();
    }
    m
}

/// Axis-aligned box, flat-shaded.
pub fn cuboid(center: Vec3, half: Vec3, color: Vec3) -> Mesh {
    let mut m = Mesh::new();
    let (c, h) = (center, half);
    let v = |sx: f32, sy: f32, sz: f32| c + Vec3::new(sx * h.x, sy * h.y, sz * h.z);
    // 8 corners
    let p000 = v(-1.0, -1.0, -1.0);
    let p100 = v(1.0, -1.0, -1.0);
    let p110 = v(1.0, 1.0, -1.0);
    let p010 = v(-1.0, 1.0, -1.0);
    let p001 = v(-1.0, -1.0, 1.0);
    let p101 = v(1.0, -1.0, 1.0);
    let p111 = v(1.0, 1.0, 1.0);
    let p011 = v(-1.0, 1.0, 1.0);
    m.add_flat_quad(p001, p101, p111, p011, color); // +Z
    m.add_flat_quad(p100, p000, p010, p110, color); // -Z
    m.add_flat_quad(p101, p100, p110, p111, color); // +X
    m.add_flat_quad(p000, p001, p011, p010, color); // -X
    m.add_flat_quad(p010, p011, p111, p110, color); // +Y
    m.add_flat_quad(p000, p100, p101, p001, color); // -Y
    m
}

/// Icosphere with `subdiv` subdivisions, smooth-shaded.
pub fn icosphere(radius: f32, subdiv: u32, color: Vec3) -> Mesh {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut verts = vec![
        Vec3::new(-1.0, t, 0.0),
        Vec3::new(1.0, t, 0.0),
        Vec3::new(-1.0, -t, 0.0),
        Vec3::new(1.0, -t, 0.0),
        Vec3::new(0.0, -1.0, t),
        Vec3::new(0.0, 1.0, t),
        Vec3::new(0.0, -1.0, -t),
        Vec3::new(0.0, 1.0, -t),
        Vec3::new(t, 0.0, -1.0),
        Vec3::new(t, 0.0, 1.0),
        Vec3::new(-t, 0.0, -1.0),
        Vec3::new(-t, 0.0, 1.0),
    ];
    for v in &mut verts {
        *v = v.normalize();
    }
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    use std::collections::HashMap;
    for _ in 0..subdiv {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut mid = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                let m = ((verts[a as usize] + verts[b as usize]) / 2.0).normalize();
                verts.push(m);
                (verts.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = mid(a, b, &mut verts);
            let bc = mid(b, c, &mut verts);
            let ca = mid(c, a, &mut verts);
            next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }
    let mut m = Mesh::new();
    for v in &verts {
        m.push_vertex(*v * radius, *v, color);
    }
    for [a, b, c] in faces {
        m.push_tri(a, b, c);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_valid() {
        for m in [
            cuboid(Vec3::ZERO, Vec3::ONE, Vec3::splat(0.5)),
            icosphere(1.0, 2, Vec3::splat(0.5)),
            lathe(&[(0.0, 0.0), (1.0, 0.5), (0.8, 1.0), (0.0, 1.4)], 12, |_, _| Vec3::ONE),
        ] {
            m.validate().unwrap();
            assert!(m.triangle_count() > 0);
        }
    }

    #[test]
    fn merge_reindexes() {
        let mut a = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        let b = icosphere(1.0, 1, Vec3::ONE);
        let n = a.vertex_count();
        a.merge(&b);
        a.validate().unwrap();
        assert_eq!(a.vertex_count(), n + b.vertex_count());
    }
}
