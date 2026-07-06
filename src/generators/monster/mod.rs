//! Monster generator — a generalization of the character body pipeline past
//! the fixed humanoid: a data-driven [`rig::MonsterRig`] (joints +
//! fold-order-ranked SDF primitives + gait descriptor) fed to one shared
//! organic pass (smooth-min compose -> surface-net mesh -> family-restricted
//! skin -> procedural clips).

mod anim;
mod body;
mod preset;
mod rig;

use glam::Vec3;

use crate::gltf::{Asset, Material, Part};
use crate::mesh::Mesh;
use crate::palette::Palette;
use crate::recipe::MonsterParams;

use rig::MonsterRig;

pub fn generate(p: &MonsterParams, pal: &Palette) -> Asset {
    // Apply the class preset onto a private copy so the recipe's params are
    // never mutated in place; explicit user knobs still win inside the preset.
    let mut owned = p.clone();
    preset::apply_preset(&mut owned);
    let p = &owned;

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
    // clamp already yields 0.0 for negative/sentinel emissive. Eyes must glow
    // regardless of the body emissive knob, so floor the material emission
    // whenever the rig actually carries eye primitives.
    let eye_glow = r.prims.iter().any(|d| d.tint == rig::PrimTint::Eye);
    let emissive_amt = p
        .emissive
        .clamp(0.0, 1.0)
        .max(if eye_glow { 0.3 } else { 0.0 });
    let emissive = pal.accent * emissive_amt * 0.6;
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

/// Inverse-distance weights over the 4 NEAREST segments (each attributing
/// weight to its proximal joint `pair.0`). Picking the nearest 4 — rather than
/// the first 4 — is essential for long chains (e.g. a 10-segment serpent
/// spine), where a tail vertex must weight tail segments, not head ones.
/// Deterministic: ties resolve by joint index.
fn seg_w(p: Vec3, world: &[Vec3], pairs: &[(usize, usize)]) -> ([u16; 4], [f32; 4]) {
    let mut dists: Vec<(f32, u16)> = pairs
        .iter()
        .map(|&(ja, jb)| {
            let (a, b) = (world[ja], world[jb]);
            let ab = b - a;
            let t = if ab.length_squared() < 1e-12 {
                0.0
            } else {
                ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            };
            (p.distance(a + ab * t), ja as u16)
        })
        .collect();
    dists.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap().then(x.1.cmp(&y.1)));
    let mut joints = [0u16; 4];
    let mut weights = [0f32; 4];
    let mut sum = 0.0;
    for (i, &(d, ja)) in dists.iter().take(4).enumerate() {
        let wgt = 1.0 / (d + 1e-4).powi(4);
        joints[i] = ja;
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

/// Find-with-path-compression for the limb union-find.
fn uf_find(uf: &mut [usize], x: usize) -> usize {
    let mut r = x;
    while uf[r] != r {
        r = uf[r];
    }
    let mut c = x;
    while uf[c] != r {
        let nx = uf[c];
        uf[c] = r;
        c = nx;
    }
    r
}

/// Trunk weight: the spine chain, or a rigid bind to the root when the trunk
/// is a single joint (e.g. an ooze with no spine to slide along).
fn trunk_weight(
    p: Vec3,
    world: &[Vec3],
    pairs: &[(usize, usize)],
    root: usize,
) -> ([u16; 4], [f32; 4]) {
    if pairs.is_empty() {
        ([root as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
    } else {
        seg_w(p, world, pairs)
    }
}

/// Family-restricted smooth skinning, generalized to any body plan. Family 0
/// is the trunk (every rank<=1 primitive, bound to the spine chain). Each
/// rank>=2 primitive is a limb; limbs are separated into families by JOINT
/// CONNECTIVITY (union-find over the non-trunk joints they touch), so legs,
/// arms, wings, tail, and tentacles each become their own family automatically
/// — no per-plan bookkeeping. Classification is by nearest-primitive SDF (NOT
/// bone-segment distance: the belly is deep inside the torso yet close to a
/// buried limb bone; segment distance would hand it to a limb and shatter it
/// mid-stride), then each vertex binds to its family's own bone segments with
/// a smoothstep junction blend to the trunk. A trunk vertex can never grab a
/// limb bone. Does NOT use the global `skeleton_segments`.
fn skin_body(mesh: &mut Mesh, rig: &MonsterRig) {
    use std::collections::{HashMap, HashSet};
    let world = rig.world();
    let g = &rig.gait;
    let skel = &rig.skeleton;
    let n = skel.joints.len();
    let scale = (rig.bounds.1 - rig.bounds.0).length().max(1.0);

    // trunk joints = the spine chain + anything a rank<=1 primitive references
    let mut is_trunk = vec![false; n];
    for &j in &g.spine {
        is_trunk[j] = true;
    }
    for d in &rig.prims {
        if d.fold_rank <= 1 {
            is_trunk[d.joint_a] = true;
            is_trunk[d.joint_b] = true;
        }
    }

    // union non-trunk joints touched together by the same rank>=2 primitive
    let mut uf: Vec<usize> = (0..n).collect();
    for d in &rig.prims {
        if d.fold_rank >= 2 && !is_trunk[d.joint_a] && !is_trunk[d.joint_b] {
            let (ra, rb) = (uf_find(&mut uf, d.joint_a), uf_find(&mut uf, d.joint_b));
            if ra != rb {
                uf[ra] = rb;
            }
        }
    }

    // assign a 1-based family id per limb component (deterministic: prim order)
    let mut fam_of_root: HashMap<usize, usize> = HashMap::new();
    let mut fam_joints: Vec<Vec<usize>> = Vec::new();
    let mut prim_fam: Vec<usize> = vec![0; rig.prims.len()];
    for (pi, d) in rig.prims.iter().enumerate() {
        if d.fold_rank <= 1 {
            continue;
        }
        let limb_j = if !is_trunk[d.joint_a] {
            Some(d.joint_a)
        } else if !is_trunk[d.joint_b] {
            Some(d.joint_b)
        } else {
            None
        };
        if let Some(j) = limb_j {
            let root = uf_find(&mut uf, j);
            let next = fam_joints.len() + 1;
            let fam = *fam_of_root.entry(root).or_insert(next);
            if fam > fam_joints.len() {
                fam_joints.push(Vec::new());
            }
            prim_fam[pi] = fam;
        }
    }
    for (j, &trunk) in is_trunk.iter().enumerate() {
        if trunk {
            continue;
        }
        let root = uf_find(&mut uf, j);
        if let Some(&fam) = fam_of_root.get(&root) {
            fam_joints[fam - 1].push(j);
        }
    }
    let n_fam = fam_joints.len() + 1;

    // trunk bone segments (spine chain)
    let trunk_pairs: Pairs = g.spine.windows(2).map(|w| (w[0], w[1])).collect();
    let trunk_root = *g.spine.first().unwrap_or(&0);

    // per-family bone segments: internal parent->child edges of the component
    let limb_pairs: Vec<Pairs> = fam_joints
        .iter()
        .map(|joints| {
            let set: HashSet<usize> = joints.iter().copied().collect();
            let mut pairs = Pairs::new();
            for &j in joints {
                if let Some(pj) = skel.joints[j].parent {
                    if set.contains(&pj) {
                        pairs.push((pj, j));
                    }
                }
            }
            pairs
        })
        .collect();

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
        // nearest limb family
        let mut limb_d = f32::INFINITY;
        let mut limb_fam = 1usize;
        for (f, &d) in fam_d.iter().enumerate().skip(1) {
            if d < limb_d {
                limb_d = d;
                limb_fam = f;
            }
        }
        let has_limb = n_fam > 1 && limb_d.is_finite();

        let trunk = trunk_weight(p, &world, &trunk_pairs, trunk_root);
        let (j, wt) = if !has_limb || trunk_d + near < limb_d {
            // firmly trunk
            trunk
        } else if limb_d + near < trunk_d {
            // firmly limb
            seg_w(p, &world, &limb_pairs[limb_fam - 1])
        } else {
            let limb_pairs = &limb_pairs[limb_fam - 1];
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
