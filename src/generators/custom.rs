//! `custom` recipes: a declarative geometry DSL that lets an AI agent build
//! ANY object — primitives, transforms, noise displacement, radial/linear
//! arrays, per-node colors, arbitrary bones, keyframe animations, physics.

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::gltf::{
    AnimationClip, Asset, Channel, ChannelData, Collider, Joint, Material, Part, Physics,
    Skeleton,
};
use crate::mesh::{Mesh, cuboid, icosphere, lathe, to_flat_shaded, tube};
use crate::noise::Noise2;

// ---------- schema ----------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ColorSpec {
    Hex(String),
    Rgb([f32; 3]),
}

impl ColorSpec {
    pub fn to_linear(&self) -> Result<Vec3, String> {
        match self {
            ColorSpec::Rgb(c) => Ok(Vec3::from_array(*c).clamp(Vec3::ZERO, Vec3::splat(4.0))),
            ColorSpec::Hex(h) => {
                let h = h.trim_start_matches('#');
                if h.len() != 6 {
                    return Err(format!("bad hex color '{h}'"));
                }
                let v = u32::from_str_radix(h, 16).map_err(|e| format!("bad hex: {e}"))?;
                Ok(crate::palette::srgb(
                    (v >> 16) as u8,
                    (v >> 8) as u8,
                    v as u8,
                ))
            }
        }
    }
}

fn d_one() -> f32 { 1.0 }
fn d_one3() -> [f32; 3] { [1.0, 1.0, 1.0] }
fn d_segments() -> u32 { 12 }
fn d_subdiv() -> u32 { 2 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransformSpec {
    #[serde(default)]
    pub translate: [f32; 3],
    /// Euler XYZ, degrees.
    #[serde(default)]
    pub rotate_deg: [f32; 3],
    #[serde(default = "d_one3")]
    pub scale: [f32; 3],
}

impl Default for TransformSpec {
    fn default() -> Self {
        Self { translate: [0.0; 3], rotate_deg: [0.0; 3], scale: [1.0; 3] }
    }
}

impl TransformSpec {
    fn matrix(&self) -> Mat4 {
        let r = Quat::from_euler(
            EulerRot::XYZ,
            self.rotate_deg[0].to_radians(),
            self.rotate_deg[1].to_radians(),
            self.rotate_deg[2].to_radians(),
        );
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            r,
            Vec3::from_array(self.translate),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DisplaceSpec {
    #[serde(default = "d_one")]
    pub amplitude: f32,
    #[serde(default = "d_one")]
    pub frequency: f32,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepeatSpec {
    pub count: u32,
    /// Radial array around Y at `radius`, otherwise linear along `step`.
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub step: [f32; 3],
    /// Rotate each radial copy to face outward.
    #[serde(default)]
    pub orient: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum ShapeSpec {
    Box {
        #[serde(default = "d_one3")]
        size: [f32; 3],
    },
    Sphere {
        #[serde(default = "d_one")]
        radius: f32,
        #[serde(default = "d_subdiv")]
        subdiv: u32,
    },
    /// Revolve a [radius, height] profile around Y.
    Lathe {
        profile: Vec<[f32; 2]>,
        #[serde(default = "d_segments")]
        segments: u32,
    },
    Cylinder {
        #[serde(default = "d_one")]
        radius: f32,
        #[serde(default = "d_one")]
        height: f32,
        #[serde(default = "d_segments")]
        segments: u32,
    },
    Cone {
        #[serde(default = "d_one")]
        radius: f32,
        #[serde(default = "d_one")]
        height: f32,
        #[serde(default = "d_segments")]
        segments: u32,
    },
    /// Tapered tube along a 3D path; radius may be one value per point.
    Tube {
        path: Vec<[f32; 3]>,
        radius: Vec<f32>,
        #[serde(default = "d_segments")]
        segments: u32,
    },
    /// n-sided prism with an optional pointed tip (crystals, pillars).
    Prism {
        #[serde(default = "d_segments")]
        sides: u32,
        #[serde(default = "d_one")]
        radius: f32,
        #[serde(default = "d_one")]
        height: f32,
        #[serde(default)]
        point: f32,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeSpec {
    #[serde(flatten)]
    pub shape: ShapeSpec,
    pub color: ColorSpec,
    /// Second color: vertical gradient bottom->top when present.
    #[serde(default)]
    pub color_top: Option<ColorSpec>,
    #[serde(default)]
    pub transform: TransformSpec,
    #[serde(default)]
    pub displace: Option<DisplaceSpec>,
    /// Facet the node (per-face normals). Default true: crisp low-poly look.
    #[serde(default = "crate::recipe::d_true_pub")]
    pub flat: bool,
    #[serde(default)]
    pub repeat: Option<RepeatSpec>,
    /// Bone name this node is rigidly bound to (requires `bones`).
    #[serde(default)]
    pub bone: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaterialSpec {
    #[serde(default)]
    pub metallic: f32,
    #[serde(default = "d_rough")]
    pub roughness: f32,
    #[serde(default)]
    pub emissive: Option<ColorSpec>,
    #[serde(default)]
    pub emissive_strength: Option<f32>,
    #[serde(default)]
    pub double_sided: bool,
}
fn d_rough() -> f32 { 0.9 }

impl Default for MaterialSpec {
    fn default() -> Self {
        Self {
            metallic: 0.0,
            roughness: 0.9,
            emissive: None,
            emissive_strength: None,
            double_sided: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PartSpec {
    #[serde(default)]
    pub material: MaterialSpec,
    pub nodes: Vec<NodeSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BoneSpec {
    pub name: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub translation: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChannelSpec {
    pub bone: String,
    /// "rotation" or "translation"
    pub path: String,
    /// rotation: spin axis; keys are degrees. translation: keys are offsets
    /// along this axis (or absolute xyz keys via `keys_xyz`).
    #[serde(default)]
    pub axis: Option<[f32; 3]>,
    /// evenly spaced keyframe values over the clip duration
    #[serde(default)]
    pub keys: Vec<f32>,
    /// explicit xyz translation keyframes (overrides axis/keys)
    #[serde(default)]
    pub keys_xyz: Vec<[f32; 3]>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnimationSpec {
    pub name: String,
    #[serde(default = "d_one")]
    pub duration: f32,
    pub channels: Vec<ChannelSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PhysicsSpec {
    /// box | sphere | capsule | trimesh | heightfield | auto (fit box)
    pub collider: String,
    #[serde(default)]
    pub half_extents: Option<[f32; 3]>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default)]
    pub mass: f32,
    #[serde(default = "d_frict")]
    pub friction: f32,
    #[serde(default)]
    pub restitution: f32,
}
fn d_frict() -> f32 { 0.6 }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomParams {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub seed: u64,
    pub parts: Vec<PartSpec>,
    #[serde(default)]
    pub bones: Vec<BoneSpec>,
    #[serde(default)]
    pub animations: Vec<AnimationSpec>,
    #[serde(default)]
    pub physics: Option<PhysicsSpec>,
}

// ---------- interpreter ----------

fn build_shape(spec: &ShapeSpec, color: Vec3) -> Result<Mesh, String> {
    Ok(match spec {
        ShapeSpec::Box { size } => cuboid(
            Vec3::ZERO,
            Vec3::from_array(*size) / 2.0,
            color,
        ),
        ShapeSpec::Sphere { radius, subdiv } => icosphere(*radius, (*subdiv).min(4), color),
        ShapeSpec::Lathe { profile, segments } => {
            if profile.len() < 2 {
                return Err("lathe profile needs >= 2 points".into());
            }
            let pts: Vec<(f32, f32)> = profile.iter().map(|p| (p[0], p[1])).collect();
            lathe(&pts, *segments, |_, _| color)
        }
        ShapeSpec::Cylinder { radius, height, segments } => lathe(
            &[
                (0.0, 0.0),
                (*radius, 0.0),
                (*radius, *height),
                (0.0, *height),
            ],
            *segments,
            |_, _| color,
        ),
        ShapeSpec::Cone { radius, height, segments } => lathe(
            &[(0.0, 0.0), (*radius, 0.0), (0.0, *height)],
            *segments,
            |_, _| color,
        ),
        ShapeSpec::Tube { path, radius, segments } => {
            if path.len() < 2 {
                return Err("tube path needs >= 2 points".into());
            }
            let pts: Vec<(Vec3, f32)> = path
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let r = if radius.is_empty() {
                        0.1
                    } else {
                        radius[i.min(radius.len() - 1)]
                    };
                    (Vec3::from_array(*p), r)
                })
                .collect();
            tube(&pts, *segments, |_| color)
        }
        ShapeSpec::Prism { sides, radius, height, point } => {
            let sides = (*sides).clamp(3, 32);
            let mut m = Mesh::new();
            let ring: Vec<Vec3> = (0..sides)
                .map(|i| {
                    let a = i as f32 / sides as f32 * core::f32::consts::TAU;
                    Vec3::new(a.cos() * radius, 0.0, a.sin() * radius)
                })
                .collect();
            let top: Vec<Vec3> = ring.iter().map(|p| *p + Vec3::Y * *height).collect();
            let tip = Vec3::new(0.0, height + point.max(0.0), 0.0);
            for i in 0..sides as usize {
                let j = (i + 1) % sides as usize;
                m.add_flat_quad(ring[i], ring[j], top[j], top[i], color);
                if *point > 0.0 {
                    m.add_flat_tri(top[i], top[j], tip, color);
                }
                m.add_flat_tri(ring[j], ring[i], Vec3::ZERO, color);
            }
            if *point <= 0.0 {
                for i in 0..sides as usize {
                    let j = (i + 1) % sides as usize;
                    m.add_flat_tri(top[i], top[j], Vec3::new(0.0, *height, 0.0), color);
                }
            }
            m
        }
    })
}

fn build_node(node: &NodeSpec, seed: u64) -> Result<Mesh, String> {
    let color = node.color.to_linear()?;
    let mut m = build_shape(&node.shape, color)?;

    if let Some(d) = &node.displace {
        let n = Noise2::new(seed ^ d.seed.wrapping_add(0x51ED));
        let f = d.frequency.max(0.01);
        for i in 0..m.positions.len() {
            let p = m.positions[i];
            let v = n.fbm(p.x * f + p.y * 0.7, p.z * f - p.y * 0.3, 4, 2.0, 0.5);
            m.positions[i] += m.normals[i] * v * d.amplitude;
        }
        m.recompute_smooth_normals();
    }

    if let Some(top) = &node.color_top {
        let top = top.to_linear()?;
        let (lo, hi) = m.bounds();
        let span = (hi.y - lo.y).max(1e-4);
        for i in 0..m.colors.len() {
            let t = (m.positions[i].y - lo.y) / span;
            m.colors[i] = crate::palette::lerp(color, top, t);
        }
    }

    if node.flat {
        m = to_flat_shaded(&m);
    }

    let base = node.transform.matrix();
    let mut out = Mesh::new();
    match &node.repeat {
        None => {
            m.transform(base);
            out = m;
        }
        Some(rep) => {
            let count = rep.count.clamp(1, 512);
            for i in 0..count {
                let mut copy = m.clone();
                let placement = if rep.radius != 0.0 {
                    let a = i as f32 / count as f32 * core::f32::consts::TAU;
                    let pos = Vec3::new(a.cos() * rep.radius, 0.0, a.sin() * rep.radius);
                    let rot = if rep.orient {
                        Mat4::from_rotation_y(-a)
                    } else {
                        Mat4::IDENTITY
                    };
                    Mat4::from_translation(pos) * rot
                } else {
                    Mat4::from_translation(Vec3::from_array(rep.step) * i as f32)
                };
                copy.transform(placement * base);
                out.merge(&copy);
            }
        }
    }
    Ok(out)
}

pub fn generate(p: &CustomParams) -> Result<Asset, String> {
    // bones -> skeleton
    let mut skeleton = None;
    let mut bone_index = std::collections::HashMap::new();
    if !p.bones.is_empty() {
        let mut joints = Vec::new();
        for (i, b) in p.bones.iter().enumerate() {
            let parent = match &b.parent {
                None => None,
                Some(name) => Some(
                    *bone_index
                        .get(name.as_str())
                        .ok_or_else(|| format!("bone '{}' declared before parent '{name}'", b.name))?,
                ),
            };
            joints.push(Joint {
                name: b.name.clone(),
                parent,
                translation: Vec3::from_array(b.translation),
                rotation: Quat::IDENTITY,
            });
            bone_index.insert(b.name.as_str(), i);
        }
        skeleton = Some(Skeleton { joints });
    }

    let mut parts = Vec::new();
    for ps in &p.parts {
        let mut mesh = Mesh::new();
        for node in &ps.nodes {
            let mut m = build_node(node, p.seed)?;
            if let Some(bone) = &node.bone {
                let bi = *bone_index
                    .get(bone.as_str())
                    .ok_or_else(|| format!("unknown bone '{bone}'"))?;
                m.bind_all_to_joint(bi as u16);
            } else if skeleton.is_some() {
                m.bind_all_to_joint(0);
            }
            mesh.merge(&m);
        }
        let emissive = match &ps.material.emissive {
            Some(c) => c.to_linear()? * ps.material.emissive_strength.unwrap_or(1.0),
            None => Vec3::ZERO,
        };
        parts.push(Part {
            mesh,
            material: Material {
                metallic: ps.material.metallic.clamp(0.0, 1.0),
                roughness: ps.material.roughness.clamp(0.03, 1.0),
                emissive,
                double_sided: ps.material.double_sided,
            },
        });
    }

    // animations
    let mut animations = Vec::new();
    for a in &p.animations {
        let mut channels = Vec::new();
        for ch in &a.channels {
            let bi = *bone_index
                .get(ch.bone.as_str())
                .ok_or_else(|| format!("unknown bone '{}' in animation", ch.bone))?;
            let nk = ch.keys.len().max(ch.keys_xyz.len());
            if nk < 2 {
                return Err(format!("channel on '{}' needs >= 2 keys", ch.bone));
            }
            let times: Vec<f32> =
                (0..nk).map(|i| i as f32 / (nk - 1) as f32 * a.duration.max(0.05)).collect();
            let bind_t = skeleton
                .as_ref()
                .map(|s| s.joints[bi].translation)
                .unwrap_or(Vec3::ZERO);
            let data = match ch.path.as_str() {
                "rotation" => {
                    let axis = Vec3::from_array(ch.axis.unwrap_or([0.0, 1.0, 0.0]))
                        .normalize_or(Vec3::Y);
                    ChannelData::Rotation(
                        ch.keys
                            .iter()
                            .map(|deg| Quat::from_axis_angle(axis, deg.to_radians()))
                            .collect(),
                    )
                }
                "translation" => {
                    if !ch.keys_xyz.is_empty() {
                        ChannelData::Translation(
                            ch.keys_xyz
                                .iter()
                                .map(|k| bind_t + Vec3::from_array(*k))
                                .collect(),
                        )
                    } else {
                        let axis = Vec3::from_array(ch.axis.unwrap_or([0.0, 1.0, 0.0]))
                            .normalize_or(Vec3::Y);
                        ChannelData::Translation(
                            ch.keys.iter().map(|k| bind_t + axis * *k).collect(),
                        )
                    }
                }
                other => return Err(format!("unknown channel path '{other}'")),
            };
            channels.push(Channel { joint: bi, times, data });
        }
        animations.push(AnimationClip { name: a.name.clone(), channels });
    }

    // physics
    let physics = match &p.physics {
        None => None,
        Some(ph) => {
            let collider = match ph.collider.as_str() {
                "box" => Collider::Box {
                    half_extents: Vec3::from_array(ph.half_extents.unwrap_or([1.0, 1.0, 1.0])),
                },
                "sphere" => Collider::Sphere { radius: ph.radius.unwrap_or(1.0) },
                "capsule" => Collider::Capsule {
                    radius: ph.radius.unwrap_or(0.5),
                    height: ph.height.unwrap_or(1.0),
                },
                "trimesh" => Collider::TriMesh,
                "heightfield" => Collider::Heightfield,
                "auto" => {
                    let mut lo = Vec3::splat(f32::INFINITY);
                    let mut hi = Vec3::splat(f32::NEG_INFINITY);
                    for part in &parts {
                        if part.mesh.vertex_count() == 0 {
                            continue;
                        }
                        let (l, h) = part.mesh.bounds();
                        lo = lo.min(l);
                        hi = hi.max(h);
                    }
                    Collider::Box { half_extents: (hi - lo) / 2.0 }
                }
                other => return Err(format!("unknown collider '{other}'")),
            };
            Some(Physics {
                collider,
                mass: ph.mass,
                friction: ph.friction,
                restitution: ph.restitution,
            })
        }
    };

    Ok(Asset {
        name: p.name.clone().unwrap_or_else(|| "custom".into()),
        parts,
        skeleton,
        animations,
        physics,
    })
}
