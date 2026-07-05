//! CPU animation evaluation + skinning: sample a clip at time t, produce
//! joint matrices, and deform a skinned mesh — so the software renderer can
//! SHOW animation frames, not just bind poses.

use glam::{Mat4, Quat, Vec3};

use crate::gltf::{AnimationClip, Asset, ChannelData, Skeleton};
use crate::mesh::Mesh;

/// Sample one channel at time `t` (clamped to the key range).
fn sample_channel(times: &[f32], t: f32) -> (usize, usize, f32) {
    if times.is_empty() {
        return (0, 0, 0.0);
    }
    let t = t.clamp(times[0], *times.last().unwrap());
    let mut i = 0;
    while i + 1 < times.len() && times[i + 1] < t {
        i += 1;
    }
    let j = (i + 1).min(times.len() - 1);
    let span = times[j] - times[i];
    let u = if span > 1e-9 { (t - times[i]) / span } else { 0.0 };
    (i, j, u)
}

/// Global joint transforms for `skel` posed by `clip` at time `t`.
/// Joints without channels stay in bind pose.
pub fn pose_at(skel: &Skeleton, clip: &AnimationClip, t: f32) -> Vec<Mat4> {
    let n = skel.joints.len();
    let mut local_t: Vec<Vec3> = skel.joints.iter().map(|j| j.translation).collect();
    let mut local_r: Vec<Quat> = skel.joints.iter().map(|j| j.rotation).collect();
    for ch in &clip.channels {
        if ch.joint >= n {
            continue;
        }
        let (i, j, u) = sample_channel(&ch.times, t);
        match &ch.data {
            ChannelData::Rotation(qs) => {
                local_r[ch.joint] = qs[i].slerp(qs[j], u);
            }
            ChannelData::Translation(ts) => {
                local_t[ch.joint] = ts[i].lerp(ts[j], u);
            }
        }
    }
    let mut globals = vec![Mat4::IDENTITY; n];
    for i in 0..n {
        let local = Mat4::from_rotation_translation(local_r[i], local_t[i]);
        globals[i] = match skel.joints[i].parent {
            // parents always precede children in our skeletons
            Some(p) => globals[p] * local,
            None => local,
        };
    }
    globals
}

/// Linear-blend skin a mesh: `globals` from [`pose_at`], `ibms` the inverse
/// bind matrices. Unskinned meshes pass through untouched.
pub fn skin_mesh(mesh: &Mesh, globals: &[Mat4], ibms: &[Mat4]) -> Mesh {
    if !mesh.is_skinned() {
        return mesh.clone();
    }
    let mats: Vec<Mat4> = globals.iter().zip(ibms).map(|(g, ibm)| *g * *ibm).collect();
    let mut out = mesh.clone();
    for (vi, p) in mesh.positions.iter().enumerate() {
        let mut np = Vec3::ZERO;
        let mut nn = Vec3::ZERO;
        for k in 0..4 {
            let w = mesh.weights[vi][k];
            if w <= 0.0 {
                continue;
            }
            let m = &mats[mesh.joints[vi][k] as usize];
            np += m.transform_point3(*p) * w;
            nn += m.transform_vector3(mesh.normals[vi]) * w;
        }
        out.positions[vi] = np;
        out.normals[vi] = nn.normalize_or(Vec3::Y);
    }
    out
}

/// Pose a whole asset at `(clip_name, t)`: returns a copy with every part's
/// mesh deformed (or an error if the clip doesn't exist).
pub fn pose_asset(asset: &Asset, clip_name: &str, t: f32) -> Result<Asset, String> {
    let skel = asset
        .skeleton
        .as_ref()
        .ok_or_else(|| "asset has no skeleton".to_string())?;
    let clip = asset
        .animations
        .iter()
        .find(|c| c.name == clip_name)
        .ok_or_else(|| {
            format!(
                "no clip '{clip_name}' (available: {})",
                asset.animations.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            )
        })?;
    let globals = pose_at(skel, clip, t);
    let ibms: Vec<Mat4> = (0..skel.joints.len()).map(|i| skel.global(i).inverse()).collect();
    let mut posed = asset.clone();
    for part in &mut posed.parts {
        part.mesh = skin_mesh(&part.mesh, &globals, &ibms);
    }
    Ok(posed)
}

/// Duration of a clip = max key time over all channels.
pub fn clip_duration(clip: &AnimationClip) -> f32 {
    clip.channels
        .iter()
        .filter_map(|c| c.times.last().copied())
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf::{Channel, Joint};
    use crate::mesh::cuboid;

    fn two_bone_skel() -> Skeleton {
        Skeleton {
            joints: vec![
                Joint { name: "root".into(), parent: None, translation: Vec3::ZERO, rotation: Quat::IDENTITY },
                Joint { name: "tip".into(), parent: Some(0), translation: Vec3::Y, rotation: Quat::IDENTITY },
            ],
        }
    }

    #[test]
    fn identity_clip_keeps_bind_pose() {
        let skel = two_bone_skel();
        let clip = AnimationClip {
            name: "idle".into(),
            channels: vec![Channel {
                joint: 1,
                times: vec![0.0, 1.0],
                data: ChannelData::Rotation(vec![Quat::IDENTITY, Quat::IDENTITY]),
            }],
        };
        let g = pose_at(&skel, &clip, 0.5);
        assert!((g[1].transform_point3(Vec3::ZERO) - Vec3::Y).length() < 1e-6);
    }

    #[test]
    fn rotation_moves_skinned_vertices() {
        let skel = two_bone_skel();
        let clip = AnimationClip {
            name: "bend".into(),
            channels: vec![Channel {
                joint: 0,
                times: vec![0.0, 1.0],
                data: ChannelData::Rotation(vec![
                    Quat::IDENTITY,
                    Quat::from_rotation_z(core::f32::consts::FRAC_PI_2),
                ]),
            }],
        };
        let mut m = cuboid(Vec3::Y, Vec3::splat(0.1), Vec3::ONE);
        m.bind_all_to_joint(0);
        let globals = pose_at(&skel, &clip, 1.0);
        let ibms: Vec<Mat4> = (0..2).map(|i| skel.global(i).inverse()).collect();
        let s = skin_mesh(&m, &globals, &ibms);
        // a box at +Y rigid to a root rotated 90° about Z lands at -X
        let c = s.positions.iter().sum::<Vec3>() / s.positions.len() as f32;
        assert!((c - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-4, "centroid {c}");
        // halfway through, it's partway around
        let mid = skin_mesh(&m, &pose_at(&skel, &clip, 0.5), &ibms);
        let cm = mid.positions.iter().sum::<Vec3>() / mid.positions.len() as f32;
        assert!(cm.x < -0.1 && cm.y > 0.1, "mid centroid {cm}");
    }

    #[test]
    fn unskinned_passthrough() {
        let m = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        let s = skin_mesh(&m, &[Mat4::IDENTITY], &[Mat4::IDENTITY]);
        assert_eq!(m.positions, s.positions);
    }
}
