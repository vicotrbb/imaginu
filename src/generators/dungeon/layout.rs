//! Deterministic dungeon layout: grid room placement → MST corridor graph
//! (Prim over room centers) → `loops` fraction of extra edges → doors at
//! wall crossings → spawn placement. Every coordinate snaps to an integer
//! meter — the seam-law analog that keeps the layout bit-exact.

use glam::Vec3;

use crate::palette::Palette;
use crate::recipe::{DungeonParams, DungeonSize, DungeonTheme};

use super::super::rng;
use super::model::{Corridor, Door, DungeonModel, Room, RoomKind, SpawnKind, SpawnPoint};
use rand::Rng;

/// Target room count before clamping (an explicit `rooms` always wins).
fn target_rooms(p: &DungeonParams, r: &mut super::super::Rand) -> usize {
    if let Some(n) = p.rooms {
        return (n as usize).clamp(1, 60);
    }
    let (base, jitter) = match p.size {
        DungeonSize::Small => (5, 2),
        DungeonSize::Medium => (9, 3),
        DungeonSize::Large => (15, 4),
    };
    (base + r.gen_range(0..jitter)).clamp(1, 60)
}

/// Room footprint size range (min..=max, integer meters) per theme.
fn room_span(theme: DungeonTheme) -> (i32, i32) {
    match theme {
        DungeonTheme::Cavern => (7, 12),
        DungeonTheme::Temple => (8, 13),
        DungeonTheme::Fortress => (7, 11),
        _ => (6, 10),
    }
}

pub fn build(p: DungeonParams, pal: Palette) -> Result<DungeonModel, String> {
    let mut r = rng(p.seed);
    let n = target_rooms(&p, &mut r);
    let theme = p.theme;
    let ceil = DungeonModel::ceiling(theme);
    let (smin, smax) = room_span(theme);

    // Grid of cells; each room lives inside its own cell so rooms never
    // overlap and corridors always have clearance between them.
    let cols = (n as f32).sqrt().ceil().max(1.0) as usize;
    let cell = (smax + 6) as f32; // room + spacing

    let mut rooms: Vec<Room> = Vec::with_capacity(n);
    for idx in 0..n {
        let i = idx % cols;
        let j = idx / cols;
        let boss_hint = idx == n.saturating_sub(1);
        let (mut w, mut d) = (r.gen_range(smin..=smax), r.gen_range(smin..=smax));
        if boss_hint {
            w = (w + 3).min(cell as i32 - 2);
            d = (d + 3).min(cell as i32 - 2);
        }
        let free_x = (cell as i32 - w - 2).max(0);
        let free_z = (cell as i32 - d - 2).max(0);
        let ox = i as i32 * cell as i32 + 1 + r.gen_range(0..=free_x);
        let oz = j as i32 * cell as i32 + 1 + r.gen_range(0..=free_z);
        rooms.push(Room {
            id: idx,
            kind: RoomKind::Normal,
            min: Vec3::new(ox as f32, 0.0, oz as f32),
            max: Vec3::new((ox + w) as f32, ceil, (oz + d) as f32),
        });
    }

    // ---- corridor graph: MST (Prim) over integer room centers ----
    let centers: Vec<Vec3> = rooms.iter().map(|rm| rm.center_i()).collect();
    let dist = |a: usize, b: usize| centers[a].distance(centers[b]);

    let mut in_tree = vec![false; n];
    let mut mst_deg = vec![0usize; n];
    let mut edges: Vec<(usize, usize)> = Vec::new();
    if n > 0 {
        in_tree[0] = true;
        for _ in 1..n {
            // cheapest edge from the tree to a new vertex
            let mut best: Option<(f32, usize, usize)> = None;
            for (a, &a_in) in in_tree.iter().enumerate() {
                if !a_in {
                    continue;
                }
                for (b, &b_in) in in_tree.iter().enumerate() {
                    if b_in {
                        continue;
                    }
                    let dd = dist(a, b);
                    if best.map(|(bd, ..)| dd < bd).unwrap_or(true) {
                        best = Some((dd, a, b));
                    }
                }
            }
            if let Some((_, a, b)) = best {
                in_tree[b] = true;
                mst_deg[a] += 1;
                mst_deg[b] += 1;
                edges.push((a, b));
            }
        }
    }

    // ---- extra loop edges: shortest non-tree pairs, `loops` fraction ----
    let mut have: std::collections::BTreeSet<(usize, usize)> =
        edges.iter().map(|&(a, b)| (a.min(b), a.max(b))).collect();
    let mut cand: Vec<(f32, usize, usize)> = Vec::new();
    for a in 0..n {
        for b in (a + 1)..n {
            if !have.contains(&(a, b)) {
                cand.push((dist(a, b), a, b));
            }
        }
    }
    cand.sort_by(|x, y| {
        x.0.partial_cmp(&y.0)
            .unwrap()
            .then(x.1.cmp(&y.1))
            .then(x.2.cmp(&y.2))
    });
    let extra = (p.loops * n as f32).round() as usize;
    for &(_, a, b) in cand.iter().take(extra) {
        if have.insert((a.min(b), a.max(b))) {
            edges.push((a, b));
        }
    }

    // ---- realize corridors (L-shaped, door-to-door) + doors ----
    let mut corridors: Vec<Corridor> = Vec::with_capacity(edges.len());
    let mut doors: Vec<Door> = Vec::new();
    for (ci, &(a, b)) in edges.iter().enumerate() {
        let ca = centers[a];
        let cb = centers[b];
        let corner = Vec3::new(cb.x, 0.0, ca.z);
        let door_a = door_on_room(&rooms[a], corner);
        let door_b = door_on_room(&rooms[b], corner);
        doors.push(Door {
            room: a,
            corridor: ci,
            pos: door_a,
        });
        doors.push(Door {
            room: b,
            corridor: ci,
            pos: door_b,
        });
        // dedup collinear points so straight corridors don't carry a corner
        let mut path = vec![door_a, corner, door_b];
        path.dedup();
        corridors.push(Corridor { a, b, path });
    }

    // ---- room kinds + spawns ----
    // entrance = room 0 (grid origin), boss = farthest room from it.
    if !rooms.is_empty() {
        rooms[0].kind = RoomKind::Entrance;
        let e = centers[0];
        let boss = (0..n)
            .max_by(|&x, &y| {
                centers[x]
                    .distance(e)
                    .partial_cmp(&centers[y].distance(e))
                    .unwrap()
            })
            .unwrap();
        if boss != 0 {
            rooms[boss].kind = RoomKind::Boss;
        }
        // treasure = MST leaves (degree 1) that aren't entrance/boss.
        for i in 0..n {
            if rooms[i].kind == RoomKind::Normal && mst_deg[i] <= 1 {
                rooms[i].kind = RoomKind::Treasure;
            }
        }
    }

    let mut spawns: Vec<SpawnPoint> = Vec::with_capacity(n);
    for rm in &rooms {
        let pos = Vec3::new(rm.center_i().x, 0.0, rm.center_i().z);
        let kind = match rm.kind {
            RoomKind::Entrance => SpawnKind::Player,
            RoomKind::Boss => SpawnKind::Boss,
            RoomKind::Treasure => SpawnKind::Loot,
            RoomKind::Normal => SpawnKind::Enemy,
        };
        spawns.push(SpawnPoint { kind, pos });
    }

    // ---- overall bounds ----
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for rm in &rooms {
        lo = lo.min(rm.min);
        hi = hi.max(rm.max);
    }
    for c in &corridors {
        for pt in &c.path {
            lo = lo.min(*pt);
            hi = hi.max(*pt + Vec3::new(0.0, ceil, 0.0));
        }
    }
    if !lo.is_finite() {
        lo = Vec3::ZERO;
        hi = Vec3::ZERO;
    }

    Ok(DungeonModel {
        p,
        pal,
        rooms,
        corridors,
        doors,
        spawns,
        bounds: (lo, hi),
    })
}

/// Where the segment from a room's center toward `toward` pierces the room
/// wall — integer-aligned since centers and rooms are integer.
fn door_on_room(room: &Room, toward: Vec3) -> Vec3 {
    let c = room.center_i();
    let dir = toward - c;
    if dir.x.abs() >= dir.z.abs() {
        let x = if dir.x >= 0.0 { room.max.x } else { room.min.x };
        Vec3::new(x, 0.0, c.z)
    } else {
        let z = if dir.z >= 0.0 { room.max.z } else { room.min.z };
        Vec3::new(c.x, 0.0, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::by_name;
    use crate::recipe::DungeonParams;

    fn params(json: &str) -> DungeonParams {
        serde_json::from_str(json).unwrap()
    }

    /// Union-find reachability from the entrance over the corridor graph.
    fn all_rooms_connected(m: &DungeonModel) -> bool {
        let n = m.rooms.len();
        if n == 0 {
            return true;
        }
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            if p[x] != x {
                let r = find(p, p[x]);
                p[x] = r;
            }
            p[x]
        }
        for c in &m.corridors {
            let (ra, rb) = (find(&mut parent, c.a), find(&mut parent, c.b));
            parent[ra] = rb;
        }
        let root = find(&mut parent, 0);
        (0..n).all(|i| find(&mut parent, i) == root)
    }

    #[test]
    fn layout_is_pure_of_seed() {
        let p = params(r#"{"kind":"dungeon","type":"crypt","seed":42}"#);
        let a = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        let b = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        assert_eq!(a.rooms.len(), b.rooms.len());
        assert_eq!(a.rooms[0].min, b.rooms[0].min);
        assert_eq!(a.corridors.len(), b.corridors.len());
        assert_eq!(a.doors.len(), b.doors.len());
    }

    #[test]
    fn layout_is_connected_and_integer_aligned() {
        let p = params(r#"{"kind":"dungeon","type":"crypt","size":"medium"}"#);
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        assert!(m.rooms.len() >= 3);
        assert!(all_rooms_connected(&m));
        for r in &m.rooms {
            assert_eq!(r.min.x, r.min.x.round());
            assert_eq!(r.min.z, r.min.z.round());
            assert_eq!(r.max.x, r.max.x.round());
            assert_eq!(r.max.z, r.max.z.round());
        }
        // corridor points are integer-aligned too
        for c in &m.corridors {
            for pt in &c.path {
                assert_eq!(pt.x, pt.x.round());
                assert_eq!(pt.z, pt.z.round());
            }
        }
    }

    #[test]
    fn hostile_input_clamps() {
        // absurd values must not hang or panic
        let p = params(
            r#"{"kind":"dungeon","type":"crypt","rooms":100000,"loops":50.0,
                "density":-9.0,"detail":1e30}"#,
        );
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        assert!(m.rooms.len() <= 60);
        assert!(all_rooms_connected(&m));
    }
}
