//! Procedural clip driver for bosses. Reuses the monster clip machinery
//! (`idle`/locomotion/`death` verbatim; `stagger` = monster `hurt` renamed)
//! and adds boss-only telegraphed clips (`telegraph`, the archetype
//! signature attack, `phase_transition`, `enrage`) built from the same
//! rotation/translation keyframe primitives. Also emits the per-phase
//! ability metadata block consumed by the game (timings only — no rng, no
//! HashMap iteration, fully deterministic).

use core::f32::consts::TAU;

use glam::{Quat, Vec3};

// Reuse the monster clip machinery wholesale — both the clip-level
// constructors AND the low-level keyframe helpers (`keys`/`rot_channel`/etc.)
// are promoted `pub(crate)` in `monster::anim`, so the boss driver shares one
// canonical keyframe implementation instead of forking a parallel one.
use crate::generators::monster::anim::{
    bind_of, body_scale, death_clip, env, hurt_clip, idle_clip, keys, locomotion_clip, rot_channel,
    trans_channel,
};
use crate::generators::monster::rig::MonsterRig;
use crate::gltf::AnimationClip;
use crate::recipe::{BossArchetype, BossParams};

use super::meta::{AbilityMeta, PhaseMeta, WeakPointMeta};

/// Joints named `neck{i}_1` (the neck mid-joint), in ascending skeleton
/// index order — deterministic (skeleton joints are a `Vec`, never a map).
fn neck_mid_joints(rig: &MonsterRig) -> Vec<usize> {
    rig.skeleton
        .joints
        .iter()
        .enumerate()
        .filter(|(_, j)| j.name.starts_with("neck") && j.name.ends_with("_1"))
        .map(|(i, _)| i)
        .collect()
}

/// Head joints named `neck{i}_head`, in ascending skeleton index order.
fn neck_head_joints(rig: &MonsterRig) -> Vec<usize> {
    rig.skeleton
        .joints
        .iter()
        .enumerate()
        .filter(|(_, j)| j.name.starts_with("neck") && j.name.ends_with("_head"))
        .map(|(i, _)| i)
        .collect()
}

/// Archetype -> signature attack clip name.
fn signature_clip(a: BossArchetype) -> &'static str {
    match a {
        BossArchetype::Hydra | BossArchetype::DragonLord => "breath",
        BossArchetype::Colossus => "slam",
        BossArchetype::Lich | BossArchetype::SwarmQueen => "summon",
    }
}

/// Full boss clip set: the shared monster clips (`idle`/locomotion/`death`,
/// `stagger` from `hurt`) plus the boss-only telegraph / signature attack /
/// phase_transition / enrage clips.
pub fn build_boss_clips(rig: &MonsterRig, p: &BossParams) -> Vec<AnimationClip> {
    let mut stagger = hurt_clip(rig);
    stagger.name = "stagger".into();

    // NOTE: the `enrage` clip is an AMBIENT stance the game loops whenever
    // `PhaseMeta.enrage == true` (see `build_phase_meta`), NOT an ability with
    // its own telegraph/active/recover window — so it is intentionally not
    // referenced by any `AbilityMeta.clip`. It is emitted here so the game has
    // the pose available; it is not a dangling/forgotten clip.
    let mut idle = idle_clip(rig);
    if matches!(p.archetype, BossArchetype::Lich) {
        add_implement_orbit(rig, &mut idle);
    }

    let mut clips = vec![
        idle,
        locomotion_clip(rig),
        telegraph_clip(rig),
        signature_attack_clip(rig, p.archetype),
        phase_transition_clip(rig),
        enrage_clip(rig),
        stagger,
        death_clip(rig),
    ];
    // `signature_attack_clip` already names the clip per archetype; keep
    // ordering stable for determinism (`Vec`, no reordering below).
    clips.retain(|c| !c.channels.is_empty() || c.name == "idle");
    clips
}

/// Lich only: a slow full-revolution spin on each `implement_pivot` joint
/// (co-located with `core`, parent of an `implement.N` tip offset by the
/// orbit radius — see `boss::rig::plan_lich`), appended onto the shared
/// `idle_clip`'s channels. The duration is derived from the idle clip's own
/// channel times (not a private constant reused across modules) and each
/// pivot completes exactly one revolution over that duration, so `rot(0) ==
/// rot(dur)` and the loop never pops.
fn add_implement_orbit(rig: &MonsterRig, clip: &mut AnimationClip) {
    let dur = clip
        .channels
        .iter()
        .filter_map(|c| c.times.last().copied())
        .fold(0.0f32, f32::max)
        .max(1.0);
    let t = keys(20, dur);
    let pivots: Vec<usize> = rig
        .skeleton
        .joints
        .iter()
        .enumerate()
        .filter(|(_, j)| j.name == "implement_pivot")
        .map(|(i, _)| i)
        .collect();
    let n = pivots.len().max(1) as f32;
    for (i, &j) in pivots.iter().enumerate() {
        let ph = i as f32 / n * TAU;
        clip.channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(p * TAU + ph)
        }));
    }
}

/// Wind-up: the body coils back and tenses, then holds with a tremor —
/// a readable "something big is coming" beat before the signature attack.
fn telegraph_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.1;
    let t = keys(18, dur);
    let coil = move |p: f32| env(p, 0.0, 0.45);
    let tremor = move |p: f32| env(p, 0.45, 0.6) * (1.0 - env(p, 0.85, 1.0));
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.35 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * coil(p) + 0.03 * tremor(p) * (p * TAU * 10.0).sin())
        }));
    }
    for &j in &neck_mid_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.4 * coil(p))
        }));
    }
    for &j in &neck_head_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.3 * coil(p) + 0.05 * tremor(p) * (p * TAU * 12.0).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(-0.4 * coil(p))
        }));
    }
    let hips = g.spine[0];
    let pull = 0.05 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::new(0.0, 0.0, -pull * coil(p))
    }));
    if channels.is_empty() {
        // never emit an empty clip: fall back to a hips tremor.
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.05 * coil(p))
        }));
    }
    AnimationClip {
        name: "telegraph".into(),
        channels,
    }
}

/// Signature attack, named per archetype. All variants reuse the same
/// lunge/thrust envelope shape as the monster `attack_clip`, driven off the
/// necks (breath), the whole body (slam), or a channeled hold (summon).
fn signature_attack_clip(rig: &MonsterRig, arch: BossArchetype) -> AnimationClip {
    let name = signature_clip(arch);
    match name {
        "breath" => breath_clip(rig),
        "slam" => slam_clip(rig),
        _ => summon_clip(rig),
    }
}

/// Necks snap forward and heads thrust out — a breath weapon release.
fn breath_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 0.9;
    let t = keys(18, dur);
    let thrust = move |p: f32| env(p, 0.1, 0.3) - env(p, 0.45, 0.9);
    let mut channels = Vec::new();
    for &j in &neck_mid_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(0.5 * thrust(p))
        }));
    }
    for &j in &neck_head_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(0.55 * thrust(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(0.55 * thrust(p))
        }));
    }
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.2 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(amp * thrust(p))
        }));
    }
    if channels.is_empty() {
        let hips = g.spine[0];
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.3 * thrust(p))
        }));
    }
    AnimationClip {
        name: "breath".into(),
        channels,
    }
}

/// The whole body rears then drives down/forward — a ground-shaking slam.
fn slam_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.0;
    let t = keys(20, dur);
    let rear = move |p: f32| env(p, 0.0, 0.35);
    let drop = move |p: f32| env(p, 0.35, 0.55) - env(p, 0.6, 1.0);
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.4 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * rear(p) + amp * 1.4 * drop(p))
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(-0.3 * rear(p) + 0.6 * drop(p))
        }));
    }
    let hips = g.spine[0];
    let sink = 0.12 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::new(0.0, -sink * drop(p).max(0.0), sink * 0.6 * drop(p).max(0.0))
    }));
    AnimationClip {
        name: "slam".into(),
        channels,
    }
}

/// A channeled ritual hold: body rises, head/necks tip up, a steady tremor
/// through the hold — reads as "summoning" without needing extra rig parts.
fn summon_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.3;
    let t = keys(22, dur);
    let rise = move |p: f32| env(p, 0.0, 0.4);
    let hold_tremor = move |p: f32| env(p, 0.4, 0.5) * (1.0 - env(p, 0.9, 1.0));
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.2 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * rise(p))
        }));
    }
    for &j in &neck_mid_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.25 * rise(p))
        }));
    }
    for &j in &neck_head_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.2 * rise(p))
                * Quat::from_rotation_y(0.06 * hold_tremor(p) * (p * TAU * 8.0).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(-0.3 * rise(p))
        }));
    }
    let hips = g.spine[0];
    let lift = 0.05 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * lift * rise(p)
    }));
    if channels.is_empty() {
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.05 * rise(p))
        }));
    }
    AnimationClip {
        name: "summon".into(),
        channels,
    }
}

/// "Armor sheds, core exposed" — since only rotation/translation channels
/// exist (no scale, see `crate::anim`/`crate::gltf::ChannelData`), this reads
/// as a dramatic pose change instead of literal scale-to-zero: a violent
/// rear-back, a full-body shudder, then a settle into a taller, more open
/// stance. The "core exposed" semantics live in `PhaseMeta`, not the pose.
fn phase_transition_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.6;
    let t = keys(28, dur);
    let rear = move |p: f32| env(p, 0.0, 0.3) - env(p, 0.55, 0.7);
    let shudder = move |p: f32| env(p, 0.3, 0.4) * (1.0 - env(p, 0.75, 0.95));
    let settle = move |p: f32| env(p, 0.7, 1.0);
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let amp = 0.5 / (i as f32);
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-amp * rear(p) + amp * 0.3 * settle(p))
                * Quat::from_rotation_z(0.08 * shudder(p) * (p * TAU * 14.0).sin())
        }));
    }
    for &j in &neck_mid_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.45 * rear(p) + 0.15 * settle(p))
                * Quat::from_rotation_y(0.1 * shudder(p) * (p * TAU * 16.0).sin())
        }));
    }
    for &j in &neck_head_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(-0.35 * rear(p) + 0.1 * settle(p))
                * Quat::from_rotation_z(0.12 * shudder(p) * (p * TAU * 18.0).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_x(-0.4 * rear(p) + 0.15 * settle(p))
        }));
    }
    for (i, &j) in g.tail.iter().enumerate() {
        let ph = i as f32 * 0.4;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.2 * shudder(p) * (p * TAU * 10.0 + ph).sin())
        }));
    }
    let hips = g.spine[0];
    let rise = 0.06 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * rise * settle(p)
    }));
    if channels.is_empty() {
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.05 * rear(p))
        }));
    }
    AnimationClip {
        name: "phase_transition".into(),
        channels,
    }
}

/// Post-transition aggressive stance: sharper, higher-amplitude than idle,
/// with a continuous tremor — a legible "the boss is angrier now" loop.
fn enrage_clip(rig: &MonsterRig) -> AnimationClip {
    let g = &rig.gait;
    let dur = 1.0;
    let t = keys(18, dur);
    let mut channels = Vec::new();
    for (i, &j) in g.spine.iter().enumerate().skip(1) {
        let ph = i as f32 * 0.3;
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(0.08 * (p * TAU * 2.0 + ph).sin())
        }));
    }
    for &j in &neck_mid_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_x(0.1 * (p * TAU * 2.0).sin())
        }));
    }
    for &j in &neck_head_joints(rig) {
        channels.push(rot_channel(j, &t, move |p| {
            Quat::from_rotation_y(0.14 * (p * TAU * 3.0).sin())
        }));
    }
    if let Some(h) = g.head {
        channels.push(rot_channel(h, &t, move |p| {
            Quat::from_rotation_y(0.2 * (p * TAU * 3.0).sin())
        }));
    }
    let hips = g.spine[0];
    let bob = 0.03 * body_scale(rig);
    channels.push(trans_channel(hips, &t, bind_of(rig, hips), move |p| {
        Vec3::Y * bob * (p * TAU * 4.0).sin().abs()
    }));
    if channels.is_empty() {
        channels.push(rot_channel(hips, &t, move |p| {
            Quat::from_rotation_x(0.05 * (p * TAU).sin())
        }));
    }
    AnimationClip {
        name: "enrage".into(),
        channels,
    }
}

/// One `PhaseMeta` block per `p.phases` (clamped 1..=4 upstream by the
/// archetype preset, but clamp again here so callers that skip the preset
/// still get a sane, non-empty result). The last phase enrages. Each block's
/// abilities reference clip names built by `build_boss_clips` with
/// non-negative telegraph/active/recover timings.
pub fn build_phase_meta(p: &BossParams, weak_points: &[WeakPointMeta]) -> Vec<PhaseMeta> {
    let n = p.phases.clamp(1, 4);
    let sig = signature_clip(p.archetype).to_string();
    (0..n)
        .map(|i| {
            let is_last = i + 1 == n;
            let hp_fraction = 1.0 - i as f32 / n as f32;
            let active_weak_points: Vec<String> = weak_points
                .iter()
                .filter(|w| w.phase <= i + 1)
                .map(|w| w.name.clone())
                .collect();
            let mut abilities = vec![
                AbilityMeta {
                    name: "telegraphed_strike".into(),
                    telegraph_s: 0.9,
                    active_s: 0.4,
                    recover_s: 0.5,
                    clip: "telegraph".into(),
                },
                AbilityMeta {
                    name: sig.clone(),
                    telegraph_s: 0.2,
                    active_s: 0.5,
                    recover_s: 0.4,
                    clip: sig.clone(),
                },
            ];
            if is_last {
                abilities.push(AbilityMeta {
                    name: "enrage".into(),
                    telegraph_s: 1.3,
                    active_s: 0.0,
                    recover_s: 0.3,
                    clip: "phase_transition".into(),
                });
            }
            PhaseMeta {
                id: i + 1,
                name: format!("phase_{}", i + 1),
                hp_fraction,
                enrage: is_last,
                active_weak_points,
                abilities,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::boss::rig::build_boss_rig;
    use crate::recipe::BossParams;

    #[test]
    fn boss_clip_set_has_signature_and_transition() {
        let p: BossParams = serde_json::from_str(r#"{"archetype":"colossus"}"#).unwrap();
        let br = build_boss_rig(&p);
        let clips = build_boss_clips(&br.rig, &p);
        let names: Vec<_> = clips.iter().map(|c| c.name.as_str()).collect();
        for want in [
            "idle",
            "walk", // colossus locomotion (biped, gait style = Walk)
            "telegraph",
            "phase_transition",
            "enrage",
            "stagger",
            "death",
        ] {
            assert!(names.contains(&want), "missing clip {want}: {names:?}");
        }
        assert!(
            names
                .iter()
                .any(|n| ["slam", "breath", "summon"].contains(n)),
            "has a signature attack"
        );
    }

    #[test]
    fn lich_idle_orbits_implements() {
        let p: BossParams = serde_json::from_str(r#"{"archetype":"lich"}"#).unwrap();
        let br = build_boss_rig(&p);
        let clips = build_boss_clips(&br.rig, &p);
        let idle = clips.iter().find(|c| c.name == "idle").expect("has idle");
        let pivots = br
            .rig
            .skeleton
            .joints
            .iter()
            .filter(|j| j.name == "implement_pivot")
            .count();
        assert!(pivots >= 2, "lich has implement pivots: {pivots}");
        let orbit_channels = idle
            .channels
            .iter()
            .filter(|c| br.rig.skeleton.joints[c.joint].name == "implement_pivot")
            .count();
        assert_eq!(
            orbit_channels, pivots,
            "every implement pivot gets an idle orbit channel"
        );
        assert!(
            clips.iter().any(|c| c.name == "summon"),
            "lich signature attack is summon"
        );
    }

    #[test]
    fn phase_meta_matches_phase_count() {
        let p: BossParams = serde_json::from_str(r#"{"archetype":"hydra","phases":2}"#).unwrap();
        let br = build_boss_rig(&p);
        let phases = build_phase_meta(&p, &br.weak_points);
        assert_eq!(phases.len(), 2);
        assert!(phases[1].enrage, "last phase enrages");
        assert!(
            phases
                .iter()
                .all(|ph| ph.abilities.iter().all(|a| a.telegraph_s >= 0.0))
        );
    }
}
