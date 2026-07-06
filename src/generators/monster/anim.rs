//! Procedural clip driver for monsters. A generic set of eased, multi-axis
//! channels parameterized by the rig's `GaitDesc` — phase-offset leg swing
//! for locomotion, spine counter-rotation, plus attack / hurt / death and an
//! optional roar. The locomotion clip is named per `gait.style`.

use core::f32::consts::TAU;

use glam::{Quat, Vec3};

use crate::gltf::{AnimationClip, Channel, ChannelData};
use crate::recipe::MonsterParams;

use super::rig::{Gait, MonsterRig};

/// Evenly spaced key times over [0, dur].
fn keys(n: usize, dur: f32) -> Vec<f32> {
    (0..=n).map(|i| i as f32 / n as f32 * dur).collect()
}

/// Rotation channel driven by a normalized-phase function.
fn rot_channel(joint: usize, times: &[f32], f: impl Fn(f32) -> Quat) -> Channel {
    let dur = *times.last().unwrap();
    Channel {
        joint,
        times: times.to_vec(),
        data: ChannelData::Rotation(times.iter().map(|&t| f(t / dur)).collect()),
    }
}

/// Translation channel offset from a bind position by a phase function.
fn trans_channel(joint: usize, times: &[f32], bind: Vec3, f: impl Fn(f32) -> Vec3) -> Channel {
    let dur = *times.last().unwrap();
    Channel {
        joint,
        times: times.to_vec(),
        data: ChannelData::Translation(times.iter().map(|&t| bind + f(t / dur)).collect()),
    }
}

/// Smooth one-shot envelope: eases 0→1 over [a, b] and holds.
fn env(p: f32, a: f32, b: f32) -> f32 {
    let t = ((p - a) / (b - a).max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bind_of(rig: &MonsterRig, joint: usize) -> Vec3 {
    rig.skeleton.joints[joint].translation
}

/// Build the full clip set for a rig. Always: `idle`, a locomotion clip named
/// per `gait.style`, `attack`, `hurt`, `death`. Plus `roar` iff the plan has
/// a head joint.
pub fn build_clips(rig: &MonsterRig, _p: &MonsterParams) -> Vec<AnimationClip> {
    let g = &rig.gait;
    let mut clips = vec![
        idle_clip(rig),
        locomotion_clip(rig),
        attack_clip(rig),
        hurt_clip(rig),
        death_clip(rig),
    ];
    if g.head.is_some() {
        clips.push(roar_clip(rig));
    }
    clips
}

/// Subtle breathing: spine bob + head sway.
fn idle_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 2.6;
    let t = keys(16, dur);
    let mut channels = Vec::new();
    // spine breathing along the chain (skip hips root)
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let ph = i as f32 * 0.3;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(0.02 * (p * TAU + ph).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_y(0.06 * (p * TAU + 1.0).sin())
        }));
    }
    for (i, &j) in g.tail.iter().enumerate() {
        let ph = i as f32 * 0.5;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.09 * (p * TAU + ph).sin())
        }));
    }
    // breathing bob on the hips
    let hips = g.spine[0];
    let bob = 0.006 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * bob * (p * TAU).sin()
    }));
    AnimationClip {
        name: "idle".into(),
        channels,
    }
}

/// Characteristic body length for scaling translational motion.
fn body_scale(rig: &MonsterRig) -> f32 {
    (rig.bounds.1 - rig.bounds.0).length().max(1.0)
}

/// Dispatch the locomotion clip on the gait style. All share the generic
/// phase-offset leg swing; the name follows the style.
fn locomotion_clip(rig: &MonsterRig) -> AnimationClip {
    let name = match rig.gait.style {
        Gait::Walk => "walk",
        Gait::Slither => "slither",
        Gait::Fly => "fly",
        Gait::Crawl => "crawl",
        Gait::Pulse => "pulse",
    };
    match rig.gait.style {
        Gait::Slither => slither_clip(rig, name),
        Gait::Fly => fly_clip(rig, name),
        Gait::Pulse => pulse_clip(rig, name),
        // Walk and Crawl both use the legged gait (crawl is lower + slower).
        _ => legged_clip(rig, name),
    }
}

/// Phase-offset leg swing across `gait.legs` with knee bend behind the body
/// and a gentle spine counter-rotation — a diagonal-sequence walk.
fn legged_clip(rig: &MonsterRig, name: &str) -> AnimationClip {
    let g = &rig.gait;
    let dur = if name == "crawl" { 1.4 } else { 1.0 };
    let t = keys(20, dur);
    let swing = if name == "crawl" { 0.35 } else { 0.55 };
    let mut channels = Vec::new();
    // diagonal gait: legs offset by a half-cycle in a front-left/rear-right
    // vs front-right/rear-left pattern. Offsets by leg index give a natural
    // sequence for the canonical FL, FR, RL, RR ordering.
    let offsets = [0.0f32, 0.5, 0.5, 0.0];
    for (li, chain) in g.legs.iter().enumerate() {
        let off = offsets.get(li).copied().unwrap_or(li as f32 * 0.25) * TAU;
        // upper: fore-aft swing
        if let Some(&up) = chain.first() {
            channels.push(rot_channel(up, &t, move |p| {
                Quat::from_rotation_x(swing * (p * TAU + off).sin())
            }));
        }
        // lower: bend as the leg passes behind (positive knee tuck)
        if chain.len() >= 2 {
            let lo = chain[1];
            channels.push(rot_channel(lo, &t, move |p| {
                Quat::from_rotation_x(-(0.8 * (p * TAU + off + 0.6).sin()).max(0.0) - 0.05)
            }));
        }
        // foot: subtle counter-roll to plant
        if chain.len() >= 3 {
            let ft = chain[2];
            channels.push(rot_channel(ft, &t, move |p| {
                Quat::from_rotation_x(0.2 * (p * TAU + off + 1.2).sin())
            }));
        }
    }
    // spine counter-rotation (yaw) and a small head bob
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.05 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(amp * (p * TAU).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(0.04 * (p * TAU * 2.0).sin())
        }));
    }
    for (i, &j) in g.tail.iter().enumerate() {
        let ph = i as f32 * 0.6;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.12 * (p * TAU + ph).sin())
        }));
    }
    // body bob: twice per cycle (each diagonal plant)
    let hips = g.spine[0];
    let bob = 0.02 * body_scale(rig) * if name == "crawl" { 0.4 } else { 1.0 };
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * bob * (p * TAU * 2.0).sin().abs()
    }));
    AnimationClip {
        name: name.into(),
        channels,
    }
}

/// Sinusoidal wave travelling down the spine (serpents/wyrms).
fn slither_clip(rig: &MonsterRig, name: &str) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.6;
    let t = keys(24, dur);
    let mut channels = Vec::new();
    let n = g.spine.len().max(1) as f32;
    for (i, &j) in g.spine.iter().enumerate() {
        let ph = i as f32 / n * TAU;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.4 * (p * TAU + ph).sin())
        }));
    }
    for (i, &j) in g.tail.iter().enumerate() {
        let ph = (g.spine.len() + i) as f32 / n * TAU;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.5 * (p * TAU + ph).sin())
        }));
    }
    AnimationClip {
        name: name.into(),
        channels,
    }
}

/// Wing flap on `gait.wings` plus a body pitch bob (flyers).
fn fly_clip(rig: &MonsterRig, name: &str) -> AnimationClip {
    let g = &rig.gait;
    let dur = 0.9;
    let t = keys(20, dur);
    let mut channels = Vec::new();
    for (i, &j) in g.wings.iter().enumerate() {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_z(side * (0.7 * (p * TAU).sin()))
        }));
    }
    // if there are legs, tuck them
    for chain in &g.legs {
        if let Some(&up) = chain.first() {
            channels.push(rot_channel(up, &t, move |_| Quat::from_rotation_x(0.5)));
        }
    }
    let hips = g.spine[0];
    let bob = 0.03 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * bob * (p * TAU).sin()
    }));
    if channels.is_empty() {
        // never emit an empty clip
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.05 * (p * TAU).sin())
        }));
    }
    AnimationClip {
        name: name.into(),
        channels,
    }
}

/// Uniform vertical squash-and-stretch bob for oozes (no limbs to swing).
fn pulse_clip(rig: &MonsterRig, name: &str) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.3;
    let t = keys(16, dur);
    let hips = g.spine[0];
    let amp = 0.04 * body_scale(rig);
    let channels = vec![trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * amp * (p * TAU).sin()
    })];
    AnimationClip {
        name: name.into(),
        channels,
    }
}

/// Lunge forward + head/maw snap.
fn attack_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 0.85;
    let t = keys(20, dur);
    let lunge = move |p: f32| env(p, 0.2, 0.4) - env(p, 0.5, 0.95);
    let mut channels = Vec::new();
    // spine rears then drives forward
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.25 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * lunge(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(0.5 * lunge(p))
        }));
    }
    // front legs plant/pull, rear legs push
    for (li, chain) in g.legs.iter().enumerate() {
        if let Some(&up) = chain.first() {
            let dir = if li < 2 { -0.4 } else { 0.5 };
            channels.push(rot_channel(up, &t, move |p| {
                Quat::from_rotation_x(dir * lunge(p))
            }));
        }
    }
    let hips = g.spine[0];
    let reach = 0.08 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        // +Z forward per the quadruped layout
        Vec3::new(0.0, 0.0, reach * lunge(p))
    }));
    AnimationClip {
        name: "attack".into(),
        channels,
    }
}

/// Sharp recoil then settle.
fn hurt_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 0.5;
    let t = keys(14, dur);
    let hit = move |p: f32| env(p, 0.0, 0.15) - env(p, 0.2, 1.0);
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.3 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(amp * hit(p)) * Quat::from_rotation_z(0.12 * hit(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(0.35 * hit(p))
        }));
    }
    let hips = g.spine[0];
    let dip = 0.03 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::new(0.0, -dip * hit(p), -dip * hit(p))
    }));
    AnimationClip {
        name: "hurt".into(),
        channels,
    }
}

/// Buckle and topple to the side, then settle.
fn death_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.4;
    let t = keys(22, dur);
    let buckle = move |p: f32| env(p, 0.0, 0.35);
    let fall = move |p: f32| env(p, 0.3, 0.9);
    let mut channels = Vec::new();
    // legs collapse
    for chain in &g.legs {
        if let Some(&up) = chain.first() {
            channels.push(rot_channel(up, &t, move |p| {
                Quat::from_rotation_x(0.6 * buckle(p))
            }));
        }
        if chain.len() >= 2 {
            let lo = chain[1];
            channels.push(rot_channel(lo, &t, move |p| {
                Quat::from_rotation_x(-1.1 * buckle(p))
            }));
        }
    }
    // spine sags
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.2 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * fall(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(-0.8 * fall(p))
        }));
    }
    // hips roll onto their side and drop to the ground
    let hips = g.spine[0];
    let hips_bind = bind_of(rig, hips);
    let drop = hips_bind.y * 0.75;
    channels.push(Channel {
        joint: hips,
        times: t.clone(),
        data: ChannelData::Rotation(
            t.iter()
                .map(|&tt| Quat::from_rotation_z(1.5 * fall(tt / dur)))
                .collect(),
        ),
    });
    channels.push(trans_channel(hips, &t, hips_bind, move |p| {
        Vec3::Y * -drop * fall(p)
    }));
    AnimationClip {
        name: "death".into(),
        channels,
    }
}

/// Head raise + jaw/maw open, chest swell.
fn roar_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.5;
    let t = keys(20, dur);
    let up = move |p: f32| env(p, 0.1, 0.35) - env(p, 0.75, 1.0);
    let mut channels = Vec::new();
    // rear the neck/head back and up
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.3 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * up(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            // raise + a small tremor while roaring
            Quat::from_rotation_x(-0.5 * up(p) + 0.04 * (p * TAU * 6.0).sin() * up(p))
        }));
    }
    let hips = g.spine[0];
    let rise = 0.03 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * rise * up(p)
    }));
    AnimationClip {
        name: "roar".into(),
        channels,
    }
}

#[cfg(test)]
mod tests {
    use super::super::rig::build_rig;
    use super::*;

    #[test]
    fn quadruped_has_expected_clips() {
        let p = MonsterParams::default();
        let rig = build_rig(&p);
        let clips = build_clips(&rig, &p);
        let names: Vec<_> = clips.iter().map(|c| c.name.as_str()).collect();
        for want in ["idle", "walk", "attack", "hurt", "death"] {
            assert!(names.contains(&want), "missing clip {want}");
        }
        // every channel targets a real joint, durations > 0
        for c in &clips {
            assert!(crate::anim::clip_duration(c) > 0.0);
            for ch in &c.channels {
                assert!((ch.joint) < rig.skeleton.joints.len());
            }
        }
    }
}
