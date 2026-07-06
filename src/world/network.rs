//! World-scale road & river networks. Rivers trace downhill on a coarse
//! global heightfield from mountain springs to lakes/sea; roads connect
//! settlements over an A* search with slope penalties; bridges spawn where
//! roads cross rivers. All polylines are computed ONCE from the recipe (part
//! of the world function), then carved/flattened into every chunk they
//! cross through pure world-coordinate sampling — so networks cross chunk
//! borders seamlessly.

use std::collections::{BinaryHeap, HashMap};

use glam::{Vec2, Vec3};

use super::model::WorldModel;
use super::zones::ZoneKind;

#[derive(Clone, Debug)]
pub struct Polyline3 {
    /// World-space points; y = river BED height or road DECK height.
    pub points: Vec<Vec3>,
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct Bridge {
    pub pos: Vec2,
    pub yaw: f32,
    pub len: f32,
    pub deck: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum SegKind {
    River,
    Road,
}

/// Spatial hash over polyline segments for O(1) proximity queries in the
/// height/color hot path. Deterministic: lookup only, insertion ordered.
struct SegGrid {
    cell: f32,
    map: HashMap<(i32, i32), Vec<(SegKind, u32, u32)>>,
}

impl SegGrid {
    fn new(cell: f32) -> Self {
        Self { cell, map: HashMap::new() }
    }
    fn insert(&mut self, kind: SegKind, poly: u32, seg: u32, a: Vec2, b: Vec2, reach: f32) {
        let min = a.min(b) - Vec2::splat(reach);
        let max = a.max(b) + Vec2::splat(reach);
        let (x0, z0) = ((min.x / self.cell).floor() as i32, (min.y / self.cell).floor() as i32);
        let (x1, z1) = ((max.x / self.cell).floor() as i32, (max.y / self.cell).floor() as i32);
        for gz in z0..=z1 {
            for gx in x0..=x1 {
                self.map.entry((gx, gz)).or_default().push((kind, poly, seg));
            }
        }
    }
    fn at(&self, p: Vec2) -> Option<&Vec<(SegKind, u32, u32)>> {
        self.map.get(&((p.x / self.cell).floor() as i32, (p.y / self.cell).floor() as i32))
    }
}

pub struct Network {
    pub rivers: Vec<Polyline3>,
    pub roads: Vec<Polyline3>,
    pub bridges: Vec<Bridge>,
    grid: SegGrid,
    /// Carve depth for rivers.
    pub river_depth: f32,
}

impl Network {
    pub fn empty() -> Self {
        Self {
            rivers: Vec::new(),
            roads: Vec::new(),
            bridges: Vec::new(),
            grid: SegGrid::new(64.0),
            river_depth: 2.2,
        }
    }

    /// Height modifier: river carve then road embankment. Pure function of
    /// world position (+ the precomputed network).
    pub fn apply_height(&self, wx: f32, wz: f32, mut h: f32) -> f32 {
        let p = Vec2::new(wx, wz);
        let Some(segs) = self.grid.at(p) else { return h };
        // rivers first: carve the channel toward the bed
        for &(kind, pi, si) in segs {
            if kind != SegKind::River {
                continue;
            }
            let poly = &self.rivers[pi as usize];
            let (d, t) = seg_closest(poly, si as usize, p);
            let w = poly.width;
            if d < w {
                let bed = seg_lerp_y(poly, si as usize, t);
                let s = 1.0 - d / w;
                let s = s * s * (3.0 - 2.0 * s);
                let target = bed + (1.0 - s) * self.river_depth * 0.4;
                h = h.min(h + (target - h) * s);
            }
        }
        // roads: flatten/embank toward the deck, feathered skirt
        for &(kind, pi, si) in segs {
            if kind != SegKind::Road {
                continue;
            }
            let poly = &self.roads[pi as usize];
            let (d, t) = seg_closest(poly, si as usize, p);
            let w = poly.width;
            let reach = w * 2.4;
            if d < reach {
                let deck = seg_lerp_y(poly, si as usize, t);
                let mut s = if d <= w { 1.0 } else { 1.0 - (d - w) / (reach - w) };
                s = (s * s * (3.0 - 2.0 * s)).clamp(0.0, 1.0);
                // keep the gully open under bridges
                for b in &self.bridges {
                    let bd = (p - b.pos).length();
                    let half = b.len * 0.5;
                    if bd < half {
                        s *= (bd / half).powi(2);
                    }
                }
                h += (deck - h) * s * 0.95;
            }
        }
        h
    }

    /// Dirt tint strength for roads (0..1) at a world position.
    pub fn road_mask(&self, wx: f32, wz: f32) -> f32 {
        let p = Vec2::new(wx, wz);
        let Some(segs) = self.grid.at(p) else { return 0.0 };
        let mut m: f32 = 0.0;
        for &(kind, pi, si) in segs {
            if kind != SegKind::Road {
                continue;
            }
            let poly = &self.roads[pi as usize];
            let (d, _) = seg_closest(poly, si as usize, p);
            if d < poly.width * 1.1 {
                let s = 1.0 - (d / (poly.width * 1.1)).powi(2);
                m = m.max(s.clamp(0.0, 1.0));
            }
        }
        m
    }

    /// 1 inside a river channel, 0 outside (for scatter suppression).
    pub fn river_mask(&self, wx: f32, wz: f32) -> f32 {
        let p = Vec2::new(wx, wz);
        let Some(segs) = self.grid.at(p) else { return 0.0 };
        for &(kind, pi, si) in segs {
            if kind != SegKind::River {
                continue;
            }
            let poly = &self.rivers[pi as usize];
            let (d, _) = seg_closest(poly, si as usize, p);
            if d < poly.width * 1.3 {
                return 1.0;
            }
        }
        0.0
    }

    /// True if any river segment passes within `r` of (wx, wz) — a real
    /// disc query over the spatial index (used by POI placement).
    pub fn river_within(&self, wx: f32, wz: f32, r: f32) -> bool {
        let p = Vec2::new(wx, wz);
        let cell = self.grid.cell;
        let (x0, z0) = (((wx - r) / cell).floor() as i32, ((wz - r) / cell).floor() as i32);
        let (x1, z1) = (((wx + r) / cell).floor() as i32, ((wz + r) / cell).floor() as i32);
        for gz in z0..=z1 {
            for gx in x0..=x1 {
                let Some(segs) = self.grid.map.get(&(gx, gz)) else { continue };
                for &(kind, pi, si) in segs {
                    if kind != SegKind::River {
                        continue;
                    }
                    let poly = &self.rivers[pi as usize];
                    let (d, _) = seg_closest(poly, si as usize, p);
                    if d < r + poly.width {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// River segments overlapping a world-space rectangle (for per-chunk
    /// water ribbons), clipped to it.
    pub fn river_ribbons_in(
        &self,
        min: Vec2,
        max: Vec2,
    ) -> Vec<(Vec3, Vec3, f32)> {
        let mut out = Vec::new();
        for poly in &self.rivers {
            for i in 0..poly.points.len().saturating_sub(1) {
                let (a, b) = (poly.points[i], poly.points[i + 1]);
                if let Some((ca, cb)) = clip_seg(
                    Vec2::new(a.x, a.z),
                    Vec2::new(b.x, b.z),
                    min,
                    max,
                ) {
                    let ta = if (b - a).length() > 1e-5 {
                        (ca - Vec2::new(a.x, a.z)).length() / (Vec2::new(b.x, b.z) - Vec2::new(a.x, a.z)).length()
                    } else {
                        0.0
                    };
                    let tb = if (b - a).length() > 1e-5 {
                        (cb - Vec2::new(a.x, a.z)).length() / (Vec2::new(b.x, b.z) - Vec2::new(a.x, a.z)).length()
                    } else {
                        1.0
                    };
                    let ya = a.y + (b.y - a.y) * ta;
                    let yb = a.y + (b.y - a.y) * tb;
                    out.push((
                        Vec3::new(ca.x, ya, ca.y),
                        Vec3::new(cb.x, yb, cb.y),
                        poly.width,
                    ));
                }
            }
        }
        out
    }
}

fn seg_closest(poly: &Polyline3, si: usize, p: Vec2) -> (f32, f32) {
    let a = poly.points[si];
    let b = poly.points[si + 1];
    let (a2, b2) = (Vec2::new(a.x, a.z), Vec2::new(b.x, b.z));
    let ab = b2 - a2;
    let len2 = ab.length_squared().max(1e-9);
    let t = ((p - a2).dot(ab) / len2).clamp(0.0, 1.0);
    ((p - (a2 + ab * t)).length(), t)
}

fn seg_lerp_y(poly: &Polyline3, si: usize, t: f32) -> f32 {
    let a = poly.points[si];
    let b = poly.points[si + 1];
    a.y + (b.y - a.y) * t
}

/// Liang-Barsky segment/rect clip in 2D.
fn clip_seg(a: Vec2, b: Vec2, min: Vec2, max: Vec2) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - min.x),
        (d.x, max.x - a.x),
        (-d.y, a.y - min.y),
        (d.y, max.y - a.y),
    ] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            t0 = t0.max(r);
        } else {
            t1 = t1.min(r);
        }
        if t0 > t1 {
            return None;
        }
    }
    Some((a + d * t0, a + d * t1))
}

/// Phase 1: rivers only (before POI placement, so settlements can avoid
/// and cluster near water). `model.network` must be empty.
pub fn build_rivers(model: &WorldModel, n_rivers: u32) -> Network {
    build_inner(model, n_rivers, false)
}

/// Phase 2: add roads + bridges. `model` has rivers in its network and
/// POIs placed; returns the complete network (rivers cloned over).
pub fn with_roads(model: &WorldModel) -> Network {
    let mut net = build_inner(model, 0, model.p.roads);
    net.rivers = model.network.rivers.clone();
    // inside settlement discs the terrain is flattened to poi.ground — make
    // road decks agree exactly (same smoothstep blend as the height fn)
    for road in &mut net.roads {
        for p in &mut road.points {
            for s in &model.pois {
                if matches!(s.kind, super::poi::PoiKind::Dungeon) {
                    continue;
                }
                let d = (p.x - s.x).hypot(p.z - s.z);
                let r_out = s.radius * 1.7;
                if d < r_out {
                    let t = ((r_out - d) / (r_out - s.radius)).clamp(0.0, 1.0);
                    let t = t * t * (3.0 - 2.0 * t);
                    p.y += (s.ground - p.y) * t * 0.96;
                }
            }
        }
    }
    // bridges + index were built against the empty river set; redo them
    net.finish_bridges_and_index();
    net
}

impl Network {
    fn finish_bridges_and_index(&mut self) {
        self.bridges.clear();
        self.grid = SegGrid::new(64.0);
        for road in &self.roads {
            for i in 0..road.points.len().saturating_sub(1) {
                let (ra, rb) = (road.points[i], road.points[i + 1]);
                for river in &self.rivers {
                    for j in 0..river.points.len().saturating_sub(1) {
                        let (va, vb) = (river.points[j], river.points[j + 1]);
                        if let Some(x) = seg_intersect_2d(
                            Vec2::new(ra.x, ra.z),
                            Vec2::new(rb.x, rb.z),
                            Vec2::new(va.x, va.z),
                            Vec2::new(vb.x, vb.z),
                        ) {
                            if self.bridges.iter().any(|b| (b.pos - x).length() < 40.0) {
                                continue;
                            }
                            let dir = Vec2::new(rb.x - ra.x, rb.z - ra.z).normalize_or_zero();
                            self.bridges.push(Bridge {
                                pos: x,
                                yaw: (-dir.y).atan2(dir.x),
                                len: (river.width * 3.4).clamp(10.0, 26.0),
                                deck: ra.y.max(rb.y),
                            });
                        }
                    }
                }
            }
        }
        for (pi, poly) in self.rivers.iter().enumerate() {
            for si in 0..poly.points.len().saturating_sub(1) {
                let (a, b) = (poly.points[si], poly.points[si + 1]);
                self.grid.insert(
                    SegKind::River,
                    pi as u32,
                    si as u32,
                    Vec2::new(a.x, a.z),
                    Vec2::new(b.x, b.z),
                    poly.width + 2.0,
                );
            }
        }
        for (pi, poly) in self.roads.iter().enumerate() {
            for si in 0..poly.points.len().saturating_sub(1) {
                let (a, b) = (poly.points[si], poly.points[si + 1]);
                self.grid.insert(
                    SegKind::Road,
                    pi as u32,
                    si as u32,
                    Vec2::new(a.x, a.z),
                    Vec2::new(b.x, b.z),
                    poly.width * 2.4 + 2.0,
                );
            }
        }
    }
}

fn build_inner(model: &WorldModel, n_rivers: u32, roads_on: bool) -> Network {
    let mut net = Network::empty();
    let size = model.size_x;
    let sea = model.p.sea_level;
    // coarse global heightfield (base + poi flattening, no network)
    let step = (size / 192.0).clamp(12.0, 48.0);
    let n = (size / step) as i32 + 1;
    let hx = |i: i32| (i as f32) * step - size * 0.5;
    let mut hgrid = vec![0.0f32; (n * n) as usize];
    for jz in 0..n {
        for jx in 0..n {
            hgrid[(jz * n + jx) as usize] = model.height(hx(jx), hx(jz));
        }
    }
    let hat = |jx: i32, jz: i32| -> f32 {
        hgrid[(jz.clamp(0, n - 1) * n + jx.clamp(0, n - 1)) as usize]
    };
    // priority-flood depression fill: rivers trace on the FILLED surface so
    // steepest descent always drains to the sea or the map edge instead of
    // dying in the first noise basin (tiny epsilon forces through-flow)
    let filled = {
        let mut filled = vec![f32::INFINITY; (n * n) as usize];
        let mut heap: BinaryHeap<(std::cmp::Reverse<i64>, i32, i32)> = BinaryHeap::new();
        let idx = |x: i32, z: i32| (z * n + x) as usize;
        let key = |h: f32| std::cmp::Reverse((h * 4096.0) as i64);
        for j in 0..n {
            for (x, z) in [(j, 0), (j, n - 1), (0, j), (n - 1, j)] {
                if filled[idx(x, z)].is_infinite() {
                    filled[idx(x, z)] = hgrid[idx(x, z)];
                    heap.push((key(hgrid[idx(x, z)]), x, z));
                }
            }
        }
        while let Some((_, x, z)) = heap.pop() {
            let hc = filled[idx(x, z)];
            for (dx, dz) in
                [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
            {
                let (kx, kz) = (x + dx, z + dz);
                if kx < 0 || kz < 0 || kx >= n || kz >= n || !filled[idx(kx, kz)].is_infinite()
                {
                    continue;
                }
                let hv = hgrid[idx(kx, kz)].max(hc + 0.02);
                filled[idx(kx, kz)] = hv;
                heap.push((key(hv), kx, kz));
            }
        }
        filled
    };
    let fat = |jx: i32, jz: i32| -> f32 {
        filled[(jz.clamp(0, n - 1) * n + jx.clamp(0, n - 1)) as usize]
    };

    // ---- rivers ------------------------------------------------------
    // springs: high cells weighted by mountain zone, separated
    let mut springs: Vec<(f32, i32, i32)> = Vec::new();
    for jz in 2..n - 2 {
        for jx in 2..n - 2 {
            let h = hat(jx, jz);
            if h < sea + model.amp * 0.5 {
                continue;
            }
            let zw = model.zones.weights(hx(jx), hx(jz));
            let s = h * (0.4 + zw[ZoneKind::Mountains.index()]);
            springs.push((s, jx, jz));
        }
    }
    springs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then((a.1, a.2).cmp(&(b.1, b.2))));
    let mut used: Vec<(f32, f32)> = Vec::new();
    let mut count = 0;
    for &(_, jx, jz) in &springs {
        if count >= n_rivers {
            break;
        }
        let (sx, sz) = (hx(jx), hx(jz));
        if used.iter().any(|(ux, uz)| ((ux - sx).powi(2) + (uz - sz).powi(2)).sqrt() < size * 0.18)
        {
            continue;
        }
        if let Some(poly) = trace_river(model, &fat, step, n, jx, jz, sea) {
            used.push((sx, sz));
            net.rivers.push(poly);
            count += 1;
        }
    }

    // ---- roads -------------------------------------------------------
    if roads_on {
        let hubs: Vec<(f32, f32)> = model
            .pois
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    super::poi::PoiKind::City | super::poi::PoiKind::Village | super::poi::PoiKind::Castle
                )
            })
            .map(|s| (s.x, s.z))
            .collect();
        // MST over straight-line distance, then A* each edge
        if hubs.len() >= 2 {
            let mut in_tree = vec![false; hubs.len()];
            in_tree[0] = true;
            for _ in 1..hubs.len() {
                let mut best: Option<(f32, usize, usize)> = None;
                for (i, hi) in hubs.iter().enumerate() {
                    if !in_tree[i] {
                        continue;
                    }
                    for (j, hj) in hubs.iter().enumerate() {
                        if in_tree[j] {
                            continue;
                        }
                        let d = (hi.0 - hj.0).hypot(hi.1 - hj.1);
                        if best.is_none() || d < best.unwrap().0 {
                            best = Some((d, i, j));
                        }
                    }
                }
                let Some((_, i, j)) = best else { break };
                in_tree[j] = true;
                if let Some(poly) = route_road(model, &hat, step, n, hubs[i], hubs[j], sea) {
                    net.roads.push(poly);
                }
            }
        }
    }

    // ---- bridges: road × river crossings ------------------------------
    for road in &net.roads {
        for i in 0..road.points.len().saturating_sub(1) {
            let (ra, rb) = (road.points[i], road.points[i + 1]);
            for river in &net.rivers {
                for j in 0..river.points.len().saturating_sub(1) {
                    let (va, vb) = (river.points[j], river.points[j + 1]);
                    if let Some(x) = seg_intersect_2d(
                        Vec2::new(ra.x, ra.z),
                        Vec2::new(rb.x, rb.z),
                        Vec2::new(va.x, va.z),
                        Vec2::new(vb.x, vb.z),
                    ) {
                        // merge with an existing nearby bridge
                        if net.bridges.iter().any(|b| (b.pos - x).length() < 40.0) {
                            continue;
                        }
                        let dir = Vec2::new(rb.x - ra.x, rb.z - ra.z).normalize_or_zero();
                        let deck = ra.y.max(rb.y);
                        net.bridges.push(Bridge {
                            pos: x,
                            yaw: (-dir.y).atan2(dir.x),
                            len: (river.width * 3.4).clamp(10.0, 26.0),
                            deck,
                        });
                    }
                }
            }
        }
    }

    // ---- spatial index -------------------------------------------------
    for (pi, poly) in net.rivers.iter().enumerate() {
        for si in 0..poly.points.len().saturating_sub(1) {
            let (a, b) = (poly.points[si], poly.points[si + 1]);
            net.grid.insert(
                SegKind::River,
                pi as u32,
                si as u32,
                Vec2::new(a.x, a.z),
                Vec2::new(b.x, b.z),
                poly.width + 2.0,
            );
        }
    }
    for (pi, poly) in net.roads.iter().enumerate() {
        for si in 0..poly.points.len().saturating_sub(1) {
            let (a, b) = (poly.points[si], poly.points[si + 1]);
            net.grid.insert(
                SegKind::Road,
                pi as u32,
                si as u32,
                Vec2::new(a.x, a.z),
                Vec2::new(b.x, b.z),
                poly.width * 2.4 + 2.0,
            );
        }
    }
    net
}

fn seg_intersect_2d(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denom = r.perp_dot(s);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = (c - a).perp_dot(s) / denom;
    let u = (c - a).perp_dot(r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a + r * t)
    } else {
        None
    }
}

/// Steepest-descent with momentum on the coarse grid; returns bed polyline.
fn trace_river(
    model: &WorldModel,
    hat: &impl Fn(i32, i32) -> f32,
    step: f32,
    n: i32,
    mut jx: i32,
    mut jz: i32,
    sea: f32,
) -> Option<Polyline3> {
    let size = model.size_x;
    let hx = |i: i32| (i as f32) * step - size * 0.5;
    let mut pts: Vec<Vec3> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    for _ in 0..(n * 4) {
        let h = hat(jx, jz);
        pts.push(Vec3::new(hx(jx), h, hx(jz)));
        if h < sea - 0.5 {
            break; // reached sea/lake
        }
        if !visited.insert((jx, jz)) {
            break;
        }
        // steepest descent over 8 neighbors; tiny deterministic tiebreak
        let mut best = (jx, jz, h);
        for (dx, dz) in
            [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
        {
            let (kx, kz) = (jx + dx, jz + dz);
            if kx < 0 || kz < 0 || kx >= n || kz >= n {
                continue;
            }
            let hh = hat(kx, kz);
            if hh < best.2 {
                best = (kx, kz, hh);
            }
        }
        if (best.0, best.1) == (jx, jz) {
            break; // basin (should not happen on the filled surface)
        }
        jx = best.0;
        jz = best.1;
        if jx == 0 || jz == 0 || jx == n - 1 || jz == n - 1 {
            pts.push(Vec3::new(hx(jx), hat(jx, jz), hx(jz)));
            break; // flows off the map
        }
    }
    if pts.len() < 8 {
        return None;
    }
    // smooth (Chaikin ×2) then enforce a monotonically descending bed
    let mut pts = chaikin(&chaikin(&pts));
    let mut bed = pts[0].y;
    for p in pts.iter_mut() {
        bed = bed.min(p.y) - 0.02;
        p.y = bed - 1.2; // bed sits below the traced surface
    }
    Some(Polyline3 { points: pts, width: 6.5 })
}

/// A* over the coarse grid, slope-penalized, water-averse.
fn route_road(
    model: &WorldModel,
    hat: &impl Fn(i32, i32) -> f32,
    step: f32,
    n: i32,
    from: (f32, f32),
    to: (f32, f32),
    sea: f32,
) -> Option<Polyline3> {
    let size = model.size_x;
    let hx = |i: i32| (i as f32) * step - size * 0.5;
    let node = |x: f32| (((x + size * 0.5) / step).round() as i32).clamp(1, n - 2);
    let (sx, sz) = (node(from.0), node(from.1));
    let (tx, tz) = (node(to.0), node(to.1));
    let idx = |x: i32, z: i32| (z * n + x) as usize;
    let mut g = vec![f32::INFINITY; (n * n) as usize];
    let mut prev = vec![u32::MAX; (n * n) as usize];
    let mut heap: BinaryHeap<(i64, i32, i32)> = BinaryHeap::new();
    let heur = |x: i32, z: i32| ((x - tx).abs().max((z - tz).abs())) as f32 * step;
    g[idx(sx, sz)] = 0.0;
    heap.push((-((heur(sx, sz) * 16.0) as i64), sx, sz));
    let mut expanded = 0u32;
    while let Some((_, x, z)) = heap.pop() {
        if (x, z) == (tx, tz) {
            break;
        }
        expanded += 1;
        if expanded > 400_000 {
            return None;
        }
        let gc = g[idx(x, z)];
        let hc = hat(x, z);
        for (dx, dz) in
            [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
        {
            let (kx, kz) = (x + dx, z + dz);
            if kx < 1 || kz < 1 || kx >= n - 1 || kz >= n - 1 {
                continue;
            }
            let hn = hat(kx, kz);
            let dist = step * ((dx * dx + dz * dz) as f32).sqrt();
            let slope = (hn - hc).abs() / dist;
            let mut cost = dist * (1.0 + slope * slope * 60.0);
            if hn < sea + 0.4 {
                cost *= 14.0; // fords/sea crossings are a last resort
            }
            // crossing a river costs a bridge: allowed, but minimized
            if model.network.river_mask(hx(kx), hx(kz)) > 0.0 {
                cost *= 5.0;
            }
            let ng = gc + cost;
            if ng < g[idx(kx, kz)] {
                g[idx(kx, kz)] = ng;
                prev[idx(kx, kz)] = idx(x, z) as u32;
                heap.push((-(((ng + heur(kx, kz)) * 16.0) as i64), kx, kz));
            }
        }
    }
    if prev[idx(tx, tz)] == u32::MAX {
        return None;
    }
    let mut cells = vec![(tx, tz)];
    let mut cur = idx(tx, tz);
    while cur != idx(sx, sz) {
        cur = prev[cur] as usize;
        cells.push((cur as i32 % n, cur as i32 / n));
        if cells.len() > (n * n) as usize {
            return None;
        }
    }
    cells.reverse();
    let mut pts: Vec<Vec3> = cells
        .iter()
        .map(|&(x, z)| Vec3::new(hx(x), hat(x, z), hx(z)))
        .collect();
    // smooth the course and the deck profile
    pts = chaikin(&chaikin(&pts));
    for _ in 0..8 {
        let snap: Vec<f32> = pts.iter().map(|p| p.y).collect();
        for i in 1..pts.len() - 1 {
            pts[i].y = (snap[i - 1] + snap[i] * 2.0 + snap[i + 1]) / 4.0;
        }
    }
    // pin the deck to the flattened ground at both settlements
    let hf = model.height(from.0, from.1);
    let ht = model.height(to.0, to.1);
    if let Some(first) = pts.first_mut() {
        first.y = hf;
    }
    if let Some(last) = pts.last_mut() {
        last.y = ht;
    }
    Some(Polyline3 { points: pts, width: 3.2 })
}

fn chaikin(pts: &[Vec3]) -> Vec<Vec3> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() * 2);
    out.push(pts[0]);
    for i in 0..pts.len() - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        out.push(a * 0.75 + b * 0.25);
        out.push(a * 0.25 + b * 0.75);
    }
    out.push(*pts.last().unwrap());
    out
}
