//! Hand-written glTF 2.0 GLB writer: multi-primitive meshes with PBR
//! vertex-color materials, skins, animation clips, and physics metadata
//! in node `extras` (read by the Babylon.js side).

use glam::{Mat4, Quat, Vec3};
use serde_json::{Map, Value, json};

use crate::mesh::Mesh;

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self { metallic: 0.0, roughness: 0.9, emissive: Vec3::ZERO, double_sided: false }
    }
}

#[derive(Clone, Debug)]
pub struct Part {
    pub mesh: Mesh,
    pub material: Material,
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Debug, Default)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
}

impl Skeleton {
    /// Global bind transform of a joint.
    pub fn global(&self, i: usize) -> Mat4 {
        let j = &self.joints[i];
        let local = Mat4::from_rotation_translation(j.rotation, j.translation);
        match j.parent {
            Some(p) => self.global(p) * local,
            None => local,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChannelData {
    Rotation(Vec<Quat>),
    Translation(Vec<Vec3>),
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub joint: usize,
    pub times: Vec<f32>,
    pub data: ChannelData,
}

#[derive(Clone, Debug)]
pub struct AnimationClip {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// Physics metadata embedded in the root node's `extras.imaginu_physics`.
#[derive(Clone, Debug)]
pub enum Collider {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
    /// Use the render mesh (or a decimated copy) as a static collider.
    TriMesh,
    /// Heightfield terrain collider.
    Heightfield,
}

#[derive(Clone, Debug)]
pub struct Physics {
    pub collider: Collider,
    /// 0.0 mass = static body.
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub parts: Vec<Part>,
    pub skeleton: Option<Skeleton>,
    pub animations: Vec<AnimationClip>,
    pub physics: Option<Physics>,
}

impl Asset {
    pub fn static_mesh(name: &str, parts: Vec<Part>, physics: Option<Physics>) -> Self {
        Self { name: name.into(), parts, skeleton: None, animations: Vec::new(), physics }
    }

    pub fn validate(&self) -> Result<(), String> {
        for p in &self.parts {
            p.mesh.validate()?;
        }
        Ok(())
    }
}

fn physics_json(p: &Physics) -> Value {
    let collider = match &p.collider {
        Collider::Box { half_extents } => json!({
            "type": "box", "halfExtents": [half_extents.x, half_extents.y, half_extents.z]
        }),
        Collider::Sphere { radius } => json!({"type": "sphere", "radius": radius}),
        Collider::Capsule { radius, height } => {
            json!({"type": "capsule", "radius": radius, "height": height})
        }
        Collider::TriMesh => json!({"type": "trimesh"}),
        Collider::Heightfield => json!({"type": "heightfield"}),
    };
    json!({
        "collider": collider,
        "mass": p.mass,
        "friction": p.friction,
        "restitution": p.restitution,
    })
}

/// Serializes an [`Asset`] to GLB bytes.
pub fn to_glb(asset: &Asset) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();

    let mut push_view = |bin: &mut Vec<u8>, bytes: &[u8], target: Option<u32>| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(bytes);
        let mut v = Map::new();
        v.insert("buffer".into(), json!(0));
        v.insert("byteOffset".into(), json!(offset));
        v.insert("byteLength".into(), json!(bytes.len()));
        if let Some(t) = target {
            v.insert("target".into(), json!(t));
        }
        buffer_views.push(Value::Object(v));
        buffer_views.len() - 1
    };

    let f32s_to_bytes = |data: &[f32]| -> Vec<u8> {
        data.iter().flat_map(|f| f.to_le_bytes()).collect()
    };

    let mut meshes_json: Vec<Value> = Vec::new();
    let mut materials_json: Vec<Value> = Vec::new();
    let mut primitives: Vec<Value> = Vec::new();
    let skinned = asset.skeleton.is_some();

    for part in &asset.parts {
        let m = &part.mesh;
        // glTF forbids zero-count accessors; skip empty parts
        if m.positions.is_empty() || m.indices.is_empty() {
            continue;
        }
        // positions
        let pos_flat: Vec<f32> = m.positions.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
        let (lo, hi) = m.bounds();
        let vp = push_view(&mut bin, &f32s_to_bytes(&pos_flat), Some(34962));
        accessors.push(json!({
            "bufferView": vp, "componentType": 5126, "count": m.positions.len(),
            "type": "VEC3", "min": [lo.x, lo.y, lo.z], "max": [hi.x, hi.y, hi.z],
        }));
        let a_pos = accessors.len() - 1;
        // normals
        let nrm_flat: Vec<f32> = m.normals.iter().flat_map(|n| [n.x, n.y, n.z]).collect();
        let vn = push_view(&mut bin, &f32s_to_bytes(&nrm_flat), Some(34962));
        accessors.push(json!({
            "bufferView": vn, "componentType": 5126, "count": m.normals.len(), "type": "VEC3",
        }));
        let a_nrm = accessors.len() - 1;
        // colors
        let col_flat: Vec<f32> = m.colors.iter().flat_map(|c| [c.x, c.y, c.z]).collect();
        let vc = push_view(&mut bin, &f32s_to_bytes(&col_flat), Some(34962));
        accessors.push(json!({
            "bufferView": vc, "componentType": 5126, "count": m.colors.len(), "type": "VEC3",
        }));
        let a_col = accessors.len() - 1;
        // indices
        let idx_bytes: Vec<u8> = m.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let vi = push_view(&mut bin, &idx_bytes, Some(34963));
        accessors.push(json!({
            "bufferView": vi, "componentType": 5125, "count": m.indices.len(), "type": "SCALAR",
        }));
        let a_idx = accessors.len() - 1;

        let mut attrs = Map::new();
        attrs.insert("POSITION".into(), json!(a_pos));
        attrs.insert("NORMAL".into(), json!(a_nrm));
        attrs.insert("COLOR_0".into(), json!(a_col));

        if skinned {
            let joints = if m.is_skinned() {
                m.joints.clone()
            } else {
                vec![[0u16; 4]; m.positions.len()]
            };
            let weights = if m.is_skinned() {
                m.weights.clone()
            } else {
                vec![[1.0f32, 0.0, 0.0, 0.0]; m.positions.len()]
            };
            let j_bytes: Vec<u8> =
                joints.iter().flatten().flat_map(|j| j.to_le_bytes()).collect();
            let vj = push_view(&mut bin, &j_bytes, Some(34962));
            accessors.push(json!({
                "bufferView": vj, "componentType": 5123, "count": joints.len(), "type": "VEC4",
            }));
            attrs.insert("JOINTS_0".into(), json!(accessors.len() - 1));
            let w_flat: Vec<f32> = weights.iter().flatten().copied().collect();
            let vw = push_view(&mut bin, &f32s_to_bytes(&w_flat), Some(34962));
            accessors.push(json!({
                "bufferView": vw, "componentType": 5126, "count": weights.len(), "type": "VEC4",
            }));
            attrs.insert("WEIGHTS_0".into(), json!(accessors.len() - 1));
        }

        let mat = &part.material;
        materials_json.push(json!({
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                "metallicFactor": mat.metallic,
                "roughnessFactor": mat.roughness,
            },
            "emissiveFactor": [mat.emissive.x, mat.emissive.y, mat.emissive.z],
            "doubleSided": mat.double_sided,
        }));

        primitives.push(json!({
            "attributes": Value::Object(attrs),
            "indices": a_idx,
            "material": materials_json.len() - 1,
            "mode": 4,
        }));
    }

    meshes_json.push(json!({"name": asset.name, "primitives": primitives}));

    // Nodes: mesh node (+ joint hierarchy if skinned)
    let mut nodes: Vec<Value> = Vec::new();
    let mut scene_nodes: Vec<usize> = Vec::new();
    let mut skins_json: Vec<Value> = Vec::new();
    let mut animations_json: Vec<Value> = Vec::new();

    let mut mesh_node = Map::new();
    mesh_node.insert("name".into(), json!(asset.name));
    mesh_node.insert("mesh".into(), json!(0));
    if let Some(p) = &asset.physics {
        mesh_node.insert("extras".into(), json!({"imaginu_physics": physics_json(p)}));
    }
    nodes.push(Value::Object(mesh_node));
    scene_nodes.push(0);

    if let Some(skel) = &asset.skeleton {
        let joint_base = nodes.len();
        // create joint nodes
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); skel.joints.len()];
        for (i, j) in skel.joints.iter().enumerate() {
            if let Some(p) = j.parent {
                children[p].push(joint_base + i);
            }
        }
        for (i, j) in skel.joints.iter().enumerate() {
            let mut n = Map::new();
            n.insert("name".into(), json!(j.name));
            n.insert(
                "translation".into(),
                json!([j.translation.x, j.translation.y, j.translation.z]),
            );
            n.insert(
                "rotation".into(),
                json!([j.rotation.x, j.rotation.y, j.rotation.z, j.rotation.w]),
            );
            if !children[i].is_empty() {
                n.insert("children".into(), json!(children[i]));
            }
            nodes.push(Value::Object(n));
        }
        // roots of the skeleton join the scene
        for (i, j) in skel.joints.iter().enumerate() {
            if j.parent.is_none() {
                scene_nodes.push(joint_base + i);
            }
        }
        // inverse bind matrices
        let ibms: Vec<f32> = (0..skel.joints.len())
            .flat_map(|i| skel.global(i).inverse().to_cols_array())
            .collect();
        let vibm = push_view(&mut bin, &f32s_to_bytes(&ibms), None);
        accessors.push(json!({
            "bufferView": vibm, "componentType": 5126,
            "count": skel.joints.len(), "type": "MAT4",
        }));
        let a_ibm = accessors.len() - 1;
        skins_json.push(json!({
            "inverseBindMatrices": a_ibm,
            "joints": (0..skel.joints.len()).map(|i| joint_base + i).collect::<Vec<_>>(),
        }));
        // attach skin to mesh node
        if let Value::Object(n) = &mut nodes[0] {
            n.insert("skin".into(), json!(0));
        }

        for clip in &asset.animations {
            let mut samplers: Vec<Value> = Vec::new();
            let mut channels_json: Vec<Value> = Vec::new();
            for ch in &clip.channels {
                let t_min = ch.times.iter().cloned().fold(f32::INFINITY, f32::min);
                let t_max = ch.times.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let vt = push_view(&mut bin, &f32s_to_bytes(&ch.times), None);
                accessors.push(json!({
                    "bufferView": vt, "componentType": 5126, "count": ch.times.len(),
                    "type": "SCALAR", "min": [t_min], "max": [t_max],
                }));
                let a_in = accessors.len() - 1;
                let (path, a_out) = match &ch.data {
                    ChannelData::Rotation(qs) => {
                        let flat: Vec<f32> =
                            qs.iter().flat_map(|q| [q.x, q.y, q.z, q.w]).collect();
                        let v = push_view(&mut bin, &f32s_to_bytes(&flat), None);
                        accessors.push(json!({
                            "bufferView": v, "componentType": 5126,
                            "count": qs.len(), "type": "VEC4",
                        }));
                        ("rotation", accessors.len() - 1)
                    }
                    ChannelData::Translation(ts) => {
                        let flat: Vec<f32> =
                            ts.iter().flat_map(|t| [t.x, t.y, t.z]).collect();
                        let v = push_view(&mut bin, &f32s_to_bytes(&flat), None);
                        accessors.push(json!({
                            "bufferView": v, "componentType": 5126,
                            "count": ts.len(), "type": "VEC3",
                        }));
                        ("translation", accessors.len() - 1)
                    }
                };
                samplers.push(json!({
                    "input": a_in, "output": a_out, "interpolation": "LINEAR",
                }));
                channels_json.push(json!({
                    "sampler": samplers.len() - 1,
                    "target": {"node": joint_base + ch.joint, "path": path},
                }));
            }
            animations_json.push(json!({
                "name": clip.name, "samplers": samplers, "channels": channels_json,
            }));
        }
    }

    let mut root = Map::new();
    root.insert(
        "asset".into(),
        json!({"version": "2.0", "generator": "imaginu 0.1"}),
    );
    root.insert("scene".into(), json!(0));
    root.insert("scenes".into(), json!([{"nodes": scene_nodes}]));
    root.insert("nodes".into(), Value::Array(nodes));
    root.insert("meshes".into(), Value::Array(meshes_json));
    root.insert("materials".into(), Value::Array(materials_json));
    root.insert("accessors".into(), Value::Array(accessors));
    root.insert("bufferViews".into(), Value::Array(buffer_views));
    root.insert("buffers".into(), json!([{"byteLength": bin.len()}]));
    if !skins_json.is_empty() {
        root.insert("skins".into(), Value::Array(skins_json));
    }
    if !animations_json.is_empty() {
        root.insert("animations".into(), Value::Array(animations_json));
    }

    let mut json_bytes = serde_json::to_vec(&Value::Object(root)).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }

    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::cuboid;

    fn sample_asset() -> Asset {
        Asset::static_mesh(
            "box",
            vec![Part { mesh: cuboid(Vec3::ZERO, Vec3::ONE, Vec3::splat(0.5)), material: Material::default() }],
            Some(Physics {
                collider: Collider::Box { half_extents: Vec3::ONE },
                mass: 1.0,
                friction: 0.5,
                restitution: 0.2,
            }),
        )
    }

    #[test]
    fn glb_structurally_valid() {
        let glb = to_glb(&sample_asset());
        assert_eq!(&glb[0..4], b"glTF");
        let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        assert_eq!(total, glb.len());
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        let doc: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert_eq!(doc["asset"]["version"], "2.0");
        assert!(doc["nodes"][0]["extras"]["imaginu_physics"]["mass"].is_number());
        // BIN chunk header follows
        let bin_hdr = 20 + json_len;
        assert_eq!(&glb[bin_hdr + 4..bin_hdr + 8], b"BIN\0");
    }

    #[test]
    fn deterministic_bytes() {
        assert_eq!(to_glb(&sample_asset()), to_glb(&sample_asset()));
    }
}
