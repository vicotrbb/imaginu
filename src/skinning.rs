//! Smooth multi-joint skinning: distance-based automatic weights (up to 4
//! joints per vertex) so organic meshes bend without seams at joints.
//! Rigid binding (`Mesh::bind_all_to_joint`) remains for hard parts.

use glam::Vec3;

use crate::gltf::Skeleton;
use crate::mesh::Mesh;

/// A capsule-ish influence region for one joint: vertices weight by their
/// distance to the segment `a`..`b` (world/bind space). Zero-length segments
/// (a == b) act as point influences — useful for leaf joints.
#[derive(Clone, Copy, Debug)]
pub struct BoneSeg {
    pub joint: u16,
    pub a: Vec3,
    pub b: Vec3,
}

fn seg_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-12 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Assign up to 4 joint weights per vertex with inverse-distance falloff
/// `w = 1/(d + eps)^falloff`. Deterministic: ties resolve by joint index.
pub fn smooth_bind(mesh: &mut Mesh, segs: &[BoneSeg], falloff: f32) {
    assert!(
        !segs.is_empty(),
        "smooth_bind needs at least one bone segment"
    );
    let falloff = falloff.clamp(0.5, 8.0);
    let n = mesh.positions.len();
    mesh.joints = Vec::with_capacity(n);
    mesh.weights = Vec::with_capacity(n);
    let mut dists: Vec<(f32, u16)> = Vec::with_capacity(segs.len());
    for &p in &mesh.positions {
        dists.clear();
        for s in segs {
            dists.push((seg_distance(p, s.a, s.b), s.joint));
        }
        // 4 nearest; ties by joint index for determinism
        dists.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap().then(x.1.cmp(&y.1)));
        let take = dists.len().min(4);
        let mut joints = [0u16; 4];
        let mut weights = [0f32; 4];
        let mut sum = 0.0;
        for (i, &(d, j)) in dists[..take].iter().enumerate() {
            let w = 1.0 / (d + 1e-4).powf(falloff);
            joints[i] = j;
            weights[i] = w;
            sum += w;
        }
        for w in &mut weights {
            *w /= sum;
        }
        mesh.joints.push(joints);
        mesh.weights.push(weights);
    }
}

/// Derive one influence segment per joint from a skeleton's bind pose:
/// joint → midpoint-average of its children (or a point influence for
/// leaves). Works for any bone hierarchy, including custom-DSL ones.
pub fn skeleton_segments(skel: &Skeleton) -> Vec<BoneSeg> {
    let world: Vec<Vec3> = (0..skel.joints.len())
        .map(|i| skel.global(i).transform_point3(Vec3::ZERO))
        .collect();
    let mut child_sum = vec![(Vec3::ZERO, 0u32); skel.joints.len()];
    for (i, j) in skel.joints.iter().enumerate() {
        if let Some(p) = j.parent {
            child_sum[p].0 += world[i];
            child_sum[p].1 += 1;
        }
    }
    (0..skel.joints.len())
        .map(|i| {
            let (sum, count) = child_sum[i];
            let b = if count > 0 {
                sum / count as f32
            } else {
                world[i]
            };
            BoneSeg {
                joint: i as u16,
                a: world[i],
                b,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tube;

    #[test]
    fn two_bone_tube_splits_weights() {
        // vertical tube spanning two stacked bone segments
        let mut m = tube(
            &[(Vec3::ZERO, 0.2), (Vec3::Y, 0.2), (Vec3::Y * 2.0, 0.2)],
            8,
            |_| Vec3::ONE,
        );
        let segs = [
            BoneSeg {
                joint: 0,
                a: Vec3::ZERO,
                b: Vec3::Y,
            },
            BoneSeg {
                joint: 1,
                a: Vec3::Y,
                b: Vec3::Y * 2.0,
            },
        ];
        smooth_bind(&mut m, &segs, 2.5);
        m.validate().unwrap();
        for (i, p) in m.positions.iter().enumerate() {
            let w: f32 = m.weights[i].iter().sum();
            assert!((w - 1.0).abs() < 1e-4, "weights must sum to 1, got {w}");
            // near the joint between the bones, influence is split
            if (p.y - 1.0).abs() < 0.05 {
                let w0: f32 = m.weights[i]
                    .iter()
                    .zip(m.joints[i])
                    .filter(|&(_, j)| j == 0)
                    .map(|(w, _)| w)
                    .sum();
                assert!(
                    (0.25..=0.75).contains(&w0),
                    "mid vertex should share weight, joint0 got {w0}"
                );
            }
            // deep inside a bone, that bone dominates
            if (p.y - 0.3).abs() < 0.05 {
                assert_eq!(m.joints[i][0], 0);
                assert!(m.weights[i][0] > 0.8);
            }
        }
    }

    #[test]
    fn deterministic_weights() {
        let mk = || {
            let mut m = tube(&[(Vec3::ZERO, 0.3), (Vec3::Y * 2.0, 0.3)], 6, |_| Vec3::ONE);
            let segs = [
                BoneSeg {
                    joint: 0,
                    a: Vec3::ZERO,
                    b: Vec3::Y,
                },
                BoneSeg {
                    joint: 1,
                    a: Vec3::Y,
                    b: Vec3::Y * 2.0,
                },
            ];
            smooth_bind(&mut m, &segs, 2.5);
            (m.joints, m.weights)
        };
        assert_eq!(mk(), mk());
    }
}
