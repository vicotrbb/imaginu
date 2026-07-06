//! Monster generator — a generalization of the character body pipeline past
//! the fixed humanoid: a data-driven [`rig::MonsterRig`] (joints +
//! fold-order-ranked SDF primitives + gait descriptor) fed to one shared
//! organic pass (smooth-min compose -> surface-net mesh -> family-restricted
//! skin -> procedural clips).

mod anim;
mod body;
mod rig;

use glam::Vec3;

use crate::gltf::{Asset, Material, Part};
use crate::mesh::Mesh;
use crate::palette::Palette;
use crate::recipe::MonsterParams;

use rig::MonsterRig;

pub fn generate(p: &MonsterParams, pal: &Palette) -> Asset {
    let r = rig::build_rig(p);
    let mut mesh = body::build_body(&r, p, pal);
    skin_body(&mut mesh, &r);
    let phys = body::fit_collider(&r, p);
    let animations = if p.animate {
        anim::build_clips(&r, p)
    } else {
        Vec::new()
    };
    mesh.validate().expect("monster mesh invalid");
    // clamp already yields 0.0 for negative/sentinel emissive
    let emissive = pal.accent * p.emissive.clamp(0.0, 1.0) * 0.6;
    Asset {
        name: "monster".into(),
        parts: vec![Part {
            mesh,
            material: Material {
                roughness: 0.75,
                emissive,
                ..Default::default()
            },
        }],
        skeleton: Some(r.skeleton),
        animations,
        physics: Some(phys),
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}

/// Segment set (proximal-joint pairs) → per-joint influence chain.
type Pairs = Vec<(usize, usize)>;

/// Inverse-distance weights over up to 4 segments, each attributing weight to
/// its proximal joint `pair.0` (mirrors `character::seg_w`).
fn seg_w(p: Vec3, world: &[Vec3], pairs: &[(usize, usize)]) -> ([u16; 4], [f32; 4]) {
    let mut joints = [0u16; 4];
    let mut weights = [0f32; 4];
    let mut sum = 0.0;
    for (i, &(ja, jb)) in pairs.iter().enumerate().take(4) {
        let (a, b) = (world[ja], world[jb]);
        let ab = b - a;
        let t = if ab.length_squared() < 1e-12 {
            0.0
        } else {
            ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        };
        let d = p.distance(a + ab * t);
        let wgt = 1.0 / (d + 1e-4).powi(4);
        joints[i] = ja as u16;
        weights[i] = wgt;
        sum += wgt;
    }
    if sum > 0.0 {
        for w in &mut weights {
            *w /= sum;
        }
    } else {
        weights[0] = 1.0;
    }
    (joints, weights)
}

/// Strongest two influences of a 4-slot weight set (for junction blending).
fn top2(j: [u16; 4], w: [f32; 4]) -> [(u16, f32); 2] {
    let mut v: Vec<(u16, f32)> = j.iter().copied().zip(w.iter().copied()).collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    [v[0], v[1]]
}

/// Family-restricted smooth skinning. Each vertex is bound only to the bone
/// chain of the region that owns it — the trunk to the spine chain, each leg
/// to its own segments, the tail to the tail chain — with a smoothstep blend
/// ONLY across a true trunk/limb junction. A hip vertex can never grab a leg
/// bone, so strides can't drag a membrane along (the phase-2 flying-shoulder
/// lesson, generalized). This does NOT use the global `skeleton_segments`.
fn skin_body(mesh: &mut Mesh, rig: &MonsterRig) {
    let world = rig.world();
    let g = &rig.gait;
    let scale = (rig.bounds.1 - rig.bounds.0).length().max(1.0);

    // trunk: the full spine chain (hips..head)
    let trunk_pairs: Pairs = g.spine.windows(2).map(|w| (w[0], w[1])).collect();
    let hips = g.spine[0];

    // tail: anchored at the hips so its base rides the pelvis
    let mut tail_pairs: Pairs = Vec::new();
    if !g.tail.is_empty() {
        tail_pairs.push((hips, g.tail[0]));
        for w in g.tail.windows(2) {
            tail_pairs.push((w[0], w[1]));
        }
    }

    // one segment chain per leg (root..foot)
    let leg_pairs: Vec<Pairs> = g
        .legs
        .iter()
        .map(|chain| chain.windows(2).map(|w| (w[0], w[1])).collect())
        .collect();

    // Family classification uses the primitive SDF, NOT bone-segment distance:
    // the belly underside is deep INSIDE the torso ellipsoid (very negative)
    // yet lies close to a buried leg-bone line — segment distance would hand
    // the belly to a leg and shatter it mid-stride. Map each prim to a family:
    // 0 = trunk (rank <= 1), 1..=nlegs = legs, nlegs+1 = tail.
    let n_legs = g.legs.len();
    let tail_fam = n_legs + 1;
    let prim_fam: Vec<usize> = rig
        .prims
        .iter()
        .map(|d| {
            if d.fold_rank <= 1 {
                0
            } else if let Some(i) = g
                .legs
                .iter()
                .position(|chain| chain.contains(&d.joint_a) || chain.contains(&d.joint_b))
            {
                i + 1
            } else {
                tail_fam
            }
        })
        .collect();
    let n_fam = tail_fam + 1;

    mesh.joints = Vec::with_capacity(mesh.positions.len());
    mesh.weights = Vec::with_capacity(mesh.positions.len());

    // ONE half-width drives both the firmly-trunk/firmly-limb cutoffs AND the
    // junction smoothstep, so `s` reaches exactly 0 and 1 at the classification
    // boundary (±near) — no ~12% weight jump between adjacent surface-net verts.
    let near = 0.02 * scale;

    for i in 0..mesh.positions.len() {
        let p = mesh.positions[i];
        // per-family nearest primitive SDF
        let mut fam_d = vec![f32::INFINITY; n_fam];
        for (pi, d) in rig.prims.iter().enumerate() {
            let e = body::eval_prim(d, &world, p);
            let f = prim_fam[pi];
            if e < fam_d[f] {
                fam_d[f] = e;
            }
        }
        let trunk_d = fam_d[0];
        // best (nearest) limb family among legs + tail
        let mut limb_d = f32::INFINITY;
        let mut limb_fam = 1usize;
        for (f, &d) in fam_d.iter().enumerate().skip(1) {
            if d < limb_d {
                limb_d = d;
                limb_fam = f;
            }
        }
        let limb_pairs: &Pairs = if limb_fam == tail_fam {
            &tail_pairs
        } else {
            &leg_pairs[limb_fam - 1]
        };

        let trunk = seg_w(p, &world, &trunk_pairs);
        let (j, wt) = if limb_pairs.is_empty() || trunk_d + near < limb_d {
            // firmly trunk
            trunk
        } else if limb_d + near < trunk_d {
            // firmly limb
            seg_w(p, &world, limb_pairs)
        } else {
            // junction: smoothstep blend of the two families. Half-width =
            // `near` so s = 0 at delta = -near and s = 1 at delta = +near,
            // meeting the firmly-trunk/firmly-limb branches continuously.
            let s = ((trunk_d - limb_d) / near * 0.5 + 0.5).clamp(0.0, 1.0);
            let s = s * s * (3.0 - 2.0 * s);
            if s <= 0.001 {
                trunk
            } else if s >= 0.999 {
                seg_w(p, &world, limb_pairs)
            } else {
                let t2 = top2(trunk.0, trunk.1);
                let l = seg_w(p, &world, limb_pairs);
                let l2 = top2(l.0, l.1);
                let joints = [t2[0].0, t2[1].0, l2[0].0, l2[1].0];
                let mut weights = [
                    t2[0].1 * (1.0 - s),
                    t2[1].1 * (1.0 - s),
                    l2[0].1 * s,
                    l2[1].1 * s,
                ];
                let sum: f32 = weights.iter().sum();
                for w in &mut weights {
                    *w /= sum;
                }
                (joints, weights)
            }
        };
        mesh.joints.push(j);
        mesh.weights.push(wt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Largest per-edge length ratio (posed / bind) — a stretched-triangle
    /// probe for skinning webs.
    fn max_edge_stretch(bind: &Mesh, posed: &Mesh) -> f32 {
        let mut m = 0.0f32;
        for tri in bind.indices.chunks(3) {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let bl = bind.positions[a as usize].distance(bind.positions[b as usize]);
                let pl = posed.positions[a as usize].distance(posed.positions[b as usize]);
                if bl > 1e-5 {
                    m = m.max(pl / bl);
                }
            }
        }
        m
    }

    #[test]
    fn locomotion_does_not_shatter_mesh() {
        let p = MonsterParams::default();
        let pal = crate::palette::by_name("verdant");
        let asset = generate(&p, &pal);
        let bind = &asset.parts[0].mesh;

        // pose at the mid-frame of the real walk clip
        let walk = asset
            .animations
            .iter()
            .find(|c| c.name == "walk")
            .expect("walk clip exists");
        let dur = crate::anim::clip_duration(walk);
        let posed_asset = crate::anim::pose_asset(&asset, "walk", dur * 0.5).unwrap();
        let posed = &posed_asset.parts[0].mesh;

        // the pose must actually move vertices
        let moved = bind
            .positions
            .iter()
            .zip(&posed.positions)
            .any(|(a, b)| a.distance(*b) > 0.01);
        assert!(moved, "walk pose should deform the mesh");
        let stretch = max_edge_stretch(bind, posed);
        assert!(
            stretch < 2.5,
            "edge stretch {stretch} indicates skinning web"
        );
    }
}
