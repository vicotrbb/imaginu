//! Boolean mesh operations (union / subtract / intersect) via BSP trees —
//! the csg.js algorithm. Deterministic; vertex colors interpolate through
//! edge splits; results come back flat-shaded (crisp cut edges).

use glam::Vec3;

use crate::mesh::Mesh;

const EPS: f32 = 1e-5;

#[derive(Clone, Copy, Debug)]
struct Vtx {
    pos: Vec3,
    color: Vec3,
}

impl Vtx {
    fn lerp(&self, o: &Vtx, t: f32) -> Vtx {
        Vtx { pos: self.pos.lerp(o.pos, t), color: self.color.lerp(o.color, t) }
    }
}

#[derive(Clone, Copy, Debug)]
struct Plane {
    n: Vec3,
    w: f32,
}

#[derive(Clone, Debug)]
struct Polygon {
    verts: Vec<Vtx>,
    plane: Plane,
}

impl Plane {
    fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Option<Plane> {
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return None;
        }
        let n = n.normalize();
        Some(Plane { n, w: n.dot(a) })
    }

    fn flip(&mut self) {
        self.n = -self.n;
        self.w = -self.w;
    }
}

impl Polygon {
    fn flip(&mut self) {
        self.verts.reverse();
        self.plane.flip();
    }
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

fn split_polygon(
    plane: &Plane,
    poly: &Polygon,
    coplanar_front: &mut Vec<Polygon>,
    coplanar_back: &mut Vec<Polygon>,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
) {
    let mut polygon_type = 0u8;
    let mut types = Vec::with_capacity(poly.verts.len());
    for v in &poly.verts {
        let t = plane.n.dot(v.pos) - plane.w;
        let ty = if t < -EPS {
            BACK
        } else if t > EPS {
            FRONT
        } else {
            COPLANAR
        };
        polygon_type |= ty;
        types.push(ty);
    }
    match polygon_type {
        COPLANAR => {
            if plane.n.dot(poly.plane.n) > 0.0 {
                coplanar_front.push(poly.clone());
            } else {
                coplanar_back.push(poly.clone());
            }
        }
        FRONT => front.push(poly.clone()),
        BACK => back.push(poly.clone()),
        _ => {
            let mut f: Vec<Vtx> = Vec::new();
            let mut b: Vec<Vtx> = Vec::new();
            let n = poly.verts.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let (ti, tj) = (types[i], types[j]);
                let (vi, vj) = (poly.verts[i], poly.verts[j]);
                if ti != BACK {
                    f.push(vi);
                }
                if ti != FRONT {
                    b.push(vi);
                }
                if (ti | tj) == SPANNING {
                    let t = (plane.w - plane.n.dot(vi.pos)) / plane.n.dot(vj.pos - vi.pos);
                    let v = vi.lerp(&vj, t);
                    f.push(v);
                    b.push(v);
                }
            }
            if f.len() >= 3 {
                front.push(Polygon { verts: f, plane: poly.plane });
            }
            if b.len() >= 3 {
                back.push(Polygon { verts: b, plane: poly.plane });
            }
        }
    }
}

#[derive(Default)]
struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<Polygon>,
}

impl BspNode {
    fn new(polygons: Vec<Polygon>) -> BspNode {
        let mut n = BspNode::default();
        if !polygons.is_empty() {
            n.build(polygons);
        }
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(p) = &mut self.plane {
            p.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, polygons: Vec<Polygon>) -> Vec<Polygon> {
        let Some(plane) = self.plane else {
            return polygons;
        };
        let mut front = Vec::new();
        let mut back = Vec::new();
        let (mut cf, mut cb) = (Vec::new(), Vec::new());
        for p in &polygons {
            split_polygon(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
        }
        front.append(&mut cf);
        back.append(&mut cb);
        let mut front = match &self.front {
            Some(f) => f.clip_polygons(front),
            None => front,
        };
        let back = match &self.back {
            Some(b) => b.clip_polygons(back),
            None => Vec::new(), // no back subtree: back polys are inside → dropped
        };
        front.extend(back);
        front
    }

    fn clip_to(&mut self, bsp: &BspNode) {
        self.polygons = bsp.clip_polygons(std::mem::take(&mut self.polygons));
        if let Some(f) = &mut self.front {
            f.clip_to(bsp);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(bsp);
        }
    }

    fn all_polygons(&self) -> Vec<Polygon> {
        let mut out = self.polygons.clone();
        if let Some(f) = &self.front {
            out.extend(f.all_polygons());
        }
        if let Some(b) = &self.back {
            out.extend(b.all_polygons());
        }
        out
    }

    fn build(&mut self, polygons: Vec<Polygon>) {
        if polygons.is_empty() {
            return;
        }
        let plane = *self.plane.get_or_insert(polygons[0].plane);
        let mut front = Vec::new();
        let mut back = Vec::new();
        let (mut cf, mut cb) = (Vec::new(), Vec::new());
        for p in &polygons {
            split_polygon(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
        }
        self.polygons.append(&mut cf);
        self.polygons.append(&mut cb);
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(back);
        }
    }
}

fn mesh_to_polygons(m: &Mesh) -> Vec<Polygon> {
    let mut out = Vec::with_capacity(m.triangle_count());
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        if let Some(plane) =
            Plane::from_points(m.positions[a], m.positions[b], m.positions[c])
        {
            out.push(Polygon {
                verts: vec![
                    Vtx { pos: m.positions[a], color: m.colors[a] },
                    Vtx { pos: m.positions[b], color: m.colors[b] },
                    Vtx { pos: m.positions[c], color: m.colors[c] },
                ],
                plane,
            });
        }
    }
    out
}

fn polygons_to_mesh(polys: &[Polygon]) -> Mesh {
    let mut m = Mesh::new();
    for p in polys {
        // fan-triangulate, flat-shaded with the polygon plane normal
        for i in 1..p.verts.len() - 1 {
            let (a, b, c) = (p.verts[0], p.verts[i], p.verts[i + 1]);
            let i0 = m.push_vertex(a.pos, p.plane.n, a.color);
            let i1 = m.push_vertex(b.pos, p.plane.n, b.color);
            let i2 = m.push_vertex(c.pos, p.plane.n, c.color);
            m.push_tri(i0, i1, i2);
        }
    }
    m
}

pub fn union(a: &Mesh, b: &Mesh) -> Mesh {
    let mut na = BspNode::new(mesh_to_polygons(a));
    let mut nb = BspNode::new(mesh_to_polygons(b));
    na.clip_to(&nb);
    nb.clip_to(&na);
    nb.invert();
    nb.clip_to(&na);
    nb.invert();
    let mut polys = na.all_polygons();
    polys.extend(nb.all_polygons());
    polygons_to_mesh(&polys)
}

pub fn subtract(a: &Mesh, b: &Mesh) -> Mesh {
    let mut na = BspNode::new(mesh_to_polygons(a));
    let mut nb = BspNode::new(mesh_to_polygons(b));
    na.invert();
    na.clip_to(&nb);
    nb.clip_to(&na);
    nb.invert();
    nb.clip_to(&na);
    nb.invert();
    let mut polys = na.all_polygons();
    polys.extend(nb.all_polygons());
    let mut m = polygons_to_mesh(&polys);
    // subtract leaves `a` inverted — flip back
    for i in (0..m.indices.len()).step_by(3) {
        m.indices.swap(i + 1, i + 2);
    }
    for n in &mut m.normals {
        *n = -*n;
    }
    m
}

pub fn intersect(a: &Mesh, b: &Mesh) -> Mesh {
    let mut na = BspNode::new(mesh_to_polygons(a));
    let mut nb = BspNode::new(mesh_to_polygons(b));
    na.invert();
    nb.clip_to(&na);
    nb.invert();
    na.clip_to(&nb);
    nb.clip_to(&na);
    let mut polys = na.all_polygons();
    polys.extend(nb.all_polygons());
    let mut m = polygons_to_mesh(&polys);
    for i in (0..m.indices.len()).step_by(3) {
        m.indices.swap(i + 1, i + 2);
    }
    for n in &mut m.normals {
        *n = -*n;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{cuboid, icosphere};

    #[test]
    fn subtract_carves_a_hole() {
        let a = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        let b = cuboid(Vec3::new(0.0, 0.0, 1.0), Vec3::splat(0.5), Vec3::ONE);
        let m = subtract(&a, &b);
        m.validate().unwrap();
        assert!(m.triangle_count() > 12, "cut adds geometry: {}", m.triangle_count());
        // nothing remains inside the cut region (front face pocket)
        let (lo, hi) = m.bounds();
        assert!((hi - Vec3::new(1.0, 1.0, 1.0)).length() < 1e-4, "{hi}");
        assert!((lo - Vec3::new(-1.0, -1.0, -1.0)).length() < 1e-4, "{lo}");
        // centroid of front-face verts pulled back (pocket exists)
        let pocket = m.positions.iter().any(|p| {
            (p.z - 0.5).abs() < 1e-4 && p.x.abs() < 0.51 && p.y.abs() < 0.51
        });
        assert!(pocket, "expected pocket floor at z=0.5");
    }

    #[test]
    fn union_and_intersect_bounds() {
        let a = icosphere(1.0, 2, Vec3::ONE);
        let mut b = icosphere(1.0, 2, Vec3::ONE);
        b.translate(Vec3::X);
        let u = union(&a, &b);
        u.validate().unwrap();
        let (lo, hi) = u.bounds();
        assert!(hi.x > 1.9 && lo.x < -0.9);
        let i = intersect(&a, &b);
        i.validate().unwrap();
        let (lo, hi) = i.bounds();
        assert!(hi.x <= 1.01 && lo.x >= -0.01, "{lo} {hi}");
    }

    #[test]
    fn deterministic() {
        let a = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        let b = icosphere(0.8, 2, Vec3::splat(0.5));
        let m1 = subtract(&a, &b);
        let m2 = subtract(&a, &b);
        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.indices, m2.indices);
    }
}
