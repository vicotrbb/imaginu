//! Hand-written glTF 2.0 GLB writer: multi-primitive meshes with PBR
//! vertex-color materials, skins, animation clips, and physics metadata
//! in node `extras` (read by the Babylon.js side).

use glam::{Mat4, Quat, Vec3};
use serde_json::{Map, Value, json};
use std::sync::Arc;

use crate::mesh::Mesh;
use crate::texture::BakedTexture;

#[derive(Clone, Debug)]
pub struct Material {
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: Vec3,
    pub double_sided: bool,
    /// Baked procedural texture set (baseColor + normal + ORM). When set,
    /// the mesh must carry UVs/tangents; factors multiply the textures.
    pub texture: Option<Arc<BakedTexture>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.9,
            emissive: Vec3::ZERO,
            double_sided: false,
            texture: None,
        }
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
    Box {
        half_extents: Vec3,
    },
    Sphere {
        radius: f32,
    },
    Capsule {
        radius: f32,
        height: f32,
    },
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

/// A mesh stamped many times via `EXT_mesh_gpu_instancing` (dense scatter
/// at a fraction of the file size and draw calls).
#[derive(Clone, Debug)]
pub struct InstancedPart {
    pub part: Part,
    /// (translation, rotation, scale) per instance.
    pub transforms: Vec<(Vec3, Quat, Vec3)>,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub parts: Vec<Part>,
    pub skeleton: Option<Skeleton>,
    pub animations: Vec<AnimationClip>,
    pub physics: Option<Physics>,
    /// Decimated LOD levels (coarsest last), exported via `MSFT_lod`.
    pub lods: Vec<Vec<Part>>,
    /// GPU-instanced scatter meshes.
    pub instanced: Vec<InstancedPart>,
}

impl Asset {
    pub fn static_mesh(name: &str, parts: Vec<Part>, physics: Option<Physics>) -> Self {
        Self {
            name: name.into(),
            parts,
            skeleton: None,
            animations: Vec::new(),
            physics,
            lods: Vec::new(),
            instanced: Vec::new(),
        }
    }

    /// Generate `n` decimated LOD levels (each ~35% of the previous).
    pub fn generate_lods(&mut self, n: u32) {
        self.lods.clear();
        for i in 1..=n.min(4) {
            let ratio = 0.35f32.powi(i as i32);
            let parts: Vec<Part> = self
                .parts
                .iter()
                .map(|p| Part {
                    mesh: crate::subdiv::decimate(&p.mesh, ratio),
                    material: p.material.clone(),
                })
                .collect();
            self.lods.push(parts);
        }
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
        while !bin.len().is_multiple_of(4) {
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

    let f32s_to_bytes =
        |data: &[f32]| -> Vec<u8> { data.iter().flat_map(|f| f.to_le_bytes()).collect() };

    let mut meshes_json: Vec<Value> = Vec::new();
    let mut materials_json: Vec<Value> = Vec::new();
    let mut images_json: Vec<Value> = Vec::new();
    let mut textures_json: Vec<Value> = Vec::new();
    // dedup: identical texture specs share one image set
    let mut tex_by_key: Vec<(String, [usize; 3])> = Vec::new();
    let skinned = asset.skeleton.is_some();

    // mesh 0 = full-detail parts; meshes 1.. = LOD levels
    let mut groups: Vec<(String, &[Part])> = vec![(asset.name.clone(), asset.parts.as_slice())];
    for (i, lod) in asset.lods.iter().enumerate() {
        groups.push((format!("{}_LOD{}", asset.name, i + 1), lod.as_slice()));
    }
    // instanced scatter meshes follow the LOD meshes
    let inst_mesh_base = groups.len();
    for (i, ip) in asset.instanced.iter().enumerate() {
        groups.push((
            format!("{}_scatter{}", asset.name, i),
            std::slice::from_ref(&ip.part),
        ));
    }

    for (group_name, group_parts) in &groups {
        let mut primitives: Vec<Value> = Vec::new();
        for part in group_parts.iter() {
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
                let j_bytes: Vec<u8> = joints
                    .iter()
                    .flatten()
                    .flat_map(|j| j.to_le_bytes())
                    .collect();
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
            let textured = mat.texture.is_some() && m.has_uvs();
            if textured {
                let tex = mat.texture.as_ref().unwrap();
                // TEXCOORD_0
                let uv_flat: Vec<f32> = m.uvs.iter().flat_map(|t| [t.x, t.y]).collect();
                let vuv = push_view(&mut bin, &f32s_to_bytes(&uv_flat), Some(34962));
                accessors.push(json!({
                    "bufferView": vuv, "componentType": 5126, "count": m.uvs.len(), "type": "VEC2",
                }));
                attrs.insert("TEXCOORD_0".into(), json!(accessors.len() - 1));
                // TANGENT
                let tan_flat: Vec<f32> = m
                    .tangents
                    .iter()
                    .flat_map(|t| [t.x, t.y, t.z, t.w])
                    .collect();
                let vtan = push_view(&mut bin, &f32s_to_bytes(&tan_flat), Some(34962));
                accessors.push(json!({
                    "bufferView": vtan, "componentType": 5126,
                    "count": m.tangents.len(), "type": "VEC4",
                }));
                attrs.insert("TANGENT".into(), json!(accessors.len() - 1));
                // images + textures (deduped by spec key)
                let triple = match tex_by_key.iter().find(|(k, _)| *k == tex.key) {
                    Some((_, t)) => *t,
                    None => {
                        let mut mk =
                            |img: &crate::texture::Rgb8Image, bin: &mut Vec<u8>| -> usize {
                                let v = push_view(bin, &img.to_png_bytes(), None);
                                images_json.push(json!({"bufferView": v, "mimeType": "image/png"}));
                                textures_json
                                    .push(json!({"source": images_json.len() - 1, "sampler": 0}));
                                textures_json.len() - 1
                            };
                        let t = [
                            mk(&tex.base_color, &mut bin),
                            mk(&tex.normal, &mut bin),
                            mk(&tex.orm, &mut bin),
                        ];
                        tex_by_key.push((tex.key.clone(), t));
                        t
                    }
                };
                materials_json.push(json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "baseColorTexture": {"index": triple[0], "texCoord": 0},
                        "metallicRoughnessTexture": {"index": triple[2], "texCoord": 0},
                        "metallicFactor": 1.0,
                        "roughnessFactor": 1.0,
                    },
                    "normalTexture": {"index": triple[1], "texCoord": 0},
                    "occlusionTexture": {"index": triple[2], "texCoord": 0},
                    "emissiveFactor": [mat.emissive.x, mat.emissive.y, mat.emissive.z],
                    "doubleSided": mat.double_sided,
                }));
            } else {
                materials_json.push(json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": [1.0, 1.0, 1.0, 1.0],
                        "metallicFactor": mat.metallic,
                        "roughnessFactor": mat.roughness,
                    },
                    "emissiveFactor": [mat.emissive.x, mat.emissive.y, mat.emissive.z],
                    "doubleSided": mat.double_sided,
                }));
            }

            let mut prim = Map::new();
            prim.insert("attributes".into(), Value::Object(attrs));
            prim.insert("indices".into(), json!(a_idx));
            prim.insert("material".into(), json!(materials_json.len() - 1));
            prim.insert("mode".into(), json!(4));
            if !m.morphs.is_empty() {
                let mut targets = Vec::new();
                for mt in &m.morphs {
                    let flat: Vec<f32> = mt.deltas.iter().flat_map(|d| [d.x, d.y, d.z]).collect();
                    let (mut lo, mut hi) =
                        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                    for d in &mt.deltas {
                        lo = lo.min(*d);
                        hi = hi.max(*d);
                    }
                    let v = push_view(&mut bin, &f32s_to_bytes(&flat), Some(34962));
                    accessors.push(json!({
                        "bufferView": v, "componentType": 5126, "count": mt.deltas.len(),
                        "type": "VEC3", "min": [lo.x, lo.y, lo.z], "max": [hi.x, hi.y, hi.z],
                    }));
                    targets.push(json!({"POSITION": accessors.len() - 1}));
                }
                prim.insert("targets".into(), Value::Array(targets));
            }
            primitives.push(Value::Object(prim));
        }

        let mut mesh_obj = Map::new();
        mesh_obj.insert("name".into(), json!(group_name));
        mesh_obj.insert("primitives".into(), Value::Array(primitives));
        // morph metadata comes from the first part that has targets
        if let Some(part) = group_parts.iter().find(|p| !p.mesh.morphs.is_empty()) {
            let names: Vec<&str> = part.mesh.morphs.iter().map(|m| m.name.as_str()).collect();
            mesh_obj.insert("weights".into(), json!(vec![0.0; names.len()]));
            mesh_obj.insert("extras".into(), json!({"targetNames": names}));
        }
        meshes_json.push(Value::Object(mesh_obj));
    } // end mesh groups

    // Nodes: mesh node (+ joint hierarchy if skinned)
    let mut nodes: Vec<Value> = Vec::new();
    let mut scene_nodes: Vec<usize> = Vec::new();
    let mut skins_json: Vec<Value> = Vec::new();
    let mut animations_json: Vec<Value> = Vec::new();

    let mut mesh_node = Map::new();
    mesh_node.insert("name".into(), json!(asset.name));
    mesh_node.insert("mesh".into(), json!(0));
    let mut extras = Map::new();
    if let Some(p) = &asset.physics {
        extras.insert("imaginu_physics".into(), physics_json(p));
    }
    if !asset.lods.is_empty() {
        // LOD nodes occupy indices 1..=n; MSFT_lod lists them coarsest-last
        let ids: Vec<usize> = (1..=asset.lods.len()).collect();
        mesh_node.insert("extensions".into(), json!({"MSFT_lod": {"ids": ids}}));
        // screen coverage hints: halve per level
        let cov: Vec<f32> = (0..=asset.lods.len())
            .map(|i| 0.5f32.powi(i as i32 + 1))
            .collect();
        extras.insert("MSFT_screencoverage".into(), json!(cov));
    }
    if !extras.is_empty() {
        mesh_node.insert("extras".into(), Value::Object(extras));
    }
    nodes.push(Value::Object(mesh_node));
    scene_nodes.push(0);
    for i in 0..asset.lods.len() {
        let mut n = Map::new();
        n.insert("name".into(), json!(format!("{}_LOD{}", asset.name, i + 1)));
        n.insert("mesh".into(), json!(i + 1));
        nodes.push(Value::Object(n));
    }
    // instanced scatter nodes (EXT_mesh_gpu_instancing)
    for (i, ip) in asset.instanced.iter().enumerate() {
        let t_flat: Vec<f32> = ip
            .transforms
            .iter()
            .flat_map(|(t, _, _)| [t.x, t.y, t.z])
            .collect();
        let r_flat: Vec<f32> = ip
            .transforms
            .iter()
            .flat_map(|(_, r, _)| [r.x, r.y, r.z, r.w])
            .collect();
        let s_flat: Vec<f32> = ip
            .transforms
            .iter()
            .flat_map(|(_, _, s)| [s.x, s.y, s.z])
            .collect();
        let n_inst = ip.transforms.len();
        let vt = push_view(&mut bin, &f32s_to_bytes(&t_flat), None);
        accessors.push(json!({
            "bufferView": vt, "componentType": 5126, "count": n_inst, "type": "VEC3",
        }));
        let a_t = accessors.len() - 1;
        let vr = push_view(&mut bin, &f32s_to_bytes(&r_flat), None);
        accessors.push(json!({
            "bufferView": vr, "componentType": 5126, "count": n_inst, "type": "VEC4",
        }));
        let a_r = accessors.len() - 1;
        let vs = push_view(&mut bin, &f32s_to_bytes(&s_flat), None);
        accessors.push(json!({
            "bufferView": vs, "componentType": 5126, "count": n_inst, "type": "VEC3",
        }));
        let a_s = accessors.len() - 1;
        let mut n = Map::new();
        n.insert("name".into(), json!(format!("{}_scatter{}", asset.name, i)));
        n.insert("mesh".into(), json!(inst_mesh_base + i));
        n.insert(
            "extensions".into(),
            json!({"EXT_mesh_gpu_instancing": {"attributes": {
                "TRANSLATION": a_t, "ROTATION": a_r, "SCALE": a_s,
            }}}),
        );
        scene_nodes.push(nodes.len());
        nodes.push(Value::Object(n));
    }

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
        // attach skin to the mesh node and any LOD nodes
        for node in nodes.iter_mut().take(1 + asset.lods.len()) {
            if let Value::Object(n) = node {
                n.insert("skin".into(), json!(0));
            }
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
                        let flat: Vec<f32> = qs.iter().flat_map(|q| [q.x, q.y, q.z, q.w]).collect();
                        let v = push_view(&mut bin, &f32s_to_bytes(&flat), None);
                        accessors.push(json!({
                            "bufferView": v, "componentType": 5126,
                            "count": qs.len(), "type": "VEC4",
                        }));
                        ("rotation", accessors.len() - 1)
                    }
                    ChannelData::Translation(ts) => {
                        let flat: Vec<f32> = ts.iter().flat_map(|t| [t.x, t.y, t.z]).collect();
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
    if !textures_json.is_empty() {
        root.insert("images".into(), Value::Array(images_json));
        root.insert("textures".into(), Value::Array(textures_json));
        // one shared trilinear repeat sampler
        root.insert(
            "samplers".into(),
            json!([{"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497}]),
        );
    }
    root.insert("accessors".into(), Value::Array(accessors));
    root.insert("bufferViews".into(), Value::Array(buffer_views));
    root.insert("buffers".into(), json!([{"byteLength": bin.len()}]));
    if !skins_json.is_empty() {
        root.insert("skins".into(), Value::Array(skins_json));
    }
    if !animations_json.is_empty() {
        root.insert("animations".into(), Value::Array(animations_json));
    }
    let mut exts: Vec<&str> = Vec::new();
    if !asset.lods.is_empty() {
        exts.push("MSFT_lod");
    }
    if !asset.instanced.is_empty() {
        exts.push("EXT_mesh_gpu_instancing");
    }
    if !exts.is_empty() {
        root.insert("extensionsUsed".into(), json!(exts));
    }

    let mut json_bytes = serde_json::to_vec(&Value::Object(root)).unwrap();
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
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
            vec![Part {
                mesh: cuboid(Vec3::ZERO, Vec3::ONE, Vec3::splat(0.5)),
                material: Material::default(),
            }],
            Some(Physics {
                collider: Collider::Box {
                    half_extents: Vec3::ONE,
                },
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

    #[test]
    fn lods_export_msft_lod() {
        let mut asset = Asset::static_mesh(
            "rock",
            vec![Part {
                mesh: crate::mesh::icosphere(1.0, 3, Vec3::ONE),
                material: Material::default(),
            }],
            None,
        );
        asset.generate_lods(2);
        assert_eq!(asset.lods.len(), 2);
        assert!(asset.lods[1][0].mesh.triangle_count() < asset.lods[0][0].mesh.triangle_count());
        let glb = to_glb(&asset);
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert_eq!(doc["meshes"].as_array().unwrap().len(), 3);
        assert_eq!(
            doc["nodes"][0]["extensions"]["MSFT_lod"]["ids"],
            json!([1, 2])
        );
        assert_eq!(doc["extensionsUsed"][0], "MSFT_lod");
        assert_eq!(doc["nodes"][1]["mesh"], 1);
        assert_eq!(doc["meshes"][1]["name"], "rock_LOD1");
        // scene only references the root node
        assert_eq!(doc["scenes"][0]["nodes"], json!([0]));
    }

    #[test]
    fn morph_targets_export() {
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        mesh.morphs = vec![crate::mesh::MorphTarget {
            name: "smile".into(),
            deltas: vec![Vec3::new(0.0, 0.1, 0.0); mesh.vertex_count()],
        }];
        let asset = Asset::static_mesh(
            "m",
            vec![Part {
                mesh,
                material: Material::default(),
            }],
            None,
        );
        let glb = to_glb(&asset);
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let mesh_j = &doc["meshes"][0];
        assert_eq!(mesh_j["extras"]["targetNames"][0], "smile");
        assert_eq!(mesh_j["weights"][0], 0.0);
        let target = &mesh_j["primitives"][0]["targets"][0];
        let acc = target["POSITION"].as_u64().unwrap() as usize;
        let pos_acc = mesh_j["primitives"][0]["attributes"]["POSITION"]
            .as_u64()
            .unwrap() as usize;
        assert_eq!(
            doc["accessors"][acc]["count"],
            doc["accessors"][pos_acc]["count"]
        );
    }

    #[test]
    fn textured_glb_structure() {
        let spec = crate::texture::TextureSpec {
            pattern: "wood".into(),
            scale: 1.0,
            seed: 1,
            normal_strength: 1.0,
            resolution: 64,
            colors: Vec::new(),
            base: None,
            paint: Vec::new(),
        };
        let baked = std::sync::Arc::new(crate::texture::bake(&spec).unwrap());
        let mut mesh = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        crate::uv::box_project(&mut mesh, 1.0);
        let asset = Asset::static_mesh(
            "tex",
            vec![Part {
                mesh,
                material: Material {
                    texture: Some(baked),
                    ..Default::default()
                },
            }],
            None,
        );
        let glb = to_glb(&asset);
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        assert_eq!(doc["images"].as_array().unwrap().len(), 3);
        assert_eq!(doc["textures"].as_array().unwrap().len(), 3);
        let prim = &doc["meshes"][0]["primitives"][0];
        let uv_acc = prim["attributes"]["TEXCOORD_0"].as_u64().unwrap() as usize;
        let pos_acc = prim["attributes"]["POSITION"].as_u64().unwrap() as usize;
        assert_eq!(
            doc["accessors"][uv_acc]["count"],
            doc["accessors"][pos_acc]["count"]
        );
        assert!(prim["attributes"]["TANGENT"].is_u64());
        let m = &doc["materials"][0];
        assert!(m["pbrMetallicRoughness"]["baseColorTexture"]["index"].is_u64());
        assert!(m["normalTexture"]["index"].is_u64());
        // embedded PNG magic at the image bufferView offset
        let bv = doc["images"][0]["bufferView"].as_u64().unwrap() as usize;
        let off = doc["bufferViews"][bv]["byteOffset"].as_u64().unwrap() as usize;
        let bin_start = 20 + json_len + 8;
        assert_eq!(&glb[bin_start + off + 1..bin_start + off + 4], b"PNG");
    }
}
