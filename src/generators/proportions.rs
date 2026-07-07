//! Proportion canon: derives body measurements (in meters, given overall
//! `height`) from a `bulk` scalar plus categorical `build`/`frame` recipe
//! params. Consumers (character/monster/boss generators) read these fields
//! instead of scattering `h * 0.0xx` magic constants.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Build {
    Slim,
    #[default]
    Average,
    Heavy,
    Heroic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Frame {
    Masculine,
    Feminine,
    #[default]
    Neutral,
}

/// Body measurements derived from height/bulk/build/frame. All lengths in
/// meters unless noted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proportions {
    /// Height of one head unit (height / head-count-canon).
    pub head_h: f32,
    pub head_r: f32,
    /// Half-width, hips-to-shoulder.
    pub shoulder_w: f32,
    pub hip_w: f32,
    /// 0..1 taper factor applied to waist/belly radii.
    pub waist: f32,
    pub arm_r: f32,
    pub leg_r: f32,
    pub arm_len: f32,
    pub leg_len: f32,
    pub neck_len: f32,
    pub hand_r: f32,
    pub torso_lean: f32,
}

impl Proportions {
    pub fn derive(height: f32, bulk: f32, build: Build, frame: Frame) -> Self {
        let head_h = height / 7.4;
        // limb-radius / waist multipliers
        let (bw, ww) = match build {
            Build::Slim => (0.85, 0.86),
            Build::Average => (1.0, 1.0),
            Build::Heavy => (1.22, 1.22),
            Build::Heroic => (1.12, 0.94),
        };
        // shoulder / hip multipliers
        let (sh, hp) = match frame {
            Frame::Masculine => (1.08, 0.94),
            Frame::Feminine => (0.90, 1.06),
            Frame::Neutral => (1.0, 1.0),
        };
        Self {
            head_h,
            head_r: head_h * 0.62,
            shoulder_w: height * 0.118 * sh * (0.9 + 0.1 * bulk),
            hip_w: height * 0.095 * hp,
            waist: 0.82 * ww,
            arm_r: height * 0.036 * bulk * bw,
            leg_r: height * 0.052 * bulk * bw,
            arm_len: height * 0.44,
            leg_len: height * 0.47,
            neck_len: height * 0.045,
            hand_r: height * 0.040,
            torso_lean: 0.008 * height,
        }
    }

    /// Average-build, Neutral-frame baseline for the given height/bulk.
    /// Used by callers that need a reference to compute deviation
    /// (k-factor) scalars against, without retyping the derive() formulas.
    pub fn baseline(height: f32, bulk: f32) -> Self {
        Self::derive(height, bulk, Build::Average, Frame::Neutral)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canon_head_count_in_range() {
        for (b, f) in [
            (Build::Slim, Frame::Feminine),
            (Build::Average, Frame::Neutral),
            (Build::Heavy, Frame::Masculine),
            (Build::Heroic, Frame::Masculine),
        ] {
            let p = Proportions::derive(1.7, 1.0, b, f);
            let heads = 1.7 / p.head_h;
            assert!((7.2..=7.6).contains(&heads), "{heads} heads");
        }
    }
    #[test]
    fn frame_drives_shoulder_hip_ratio() {
        let m = Proportions::derive(1.7, 1.0, Build::Average, Frame::Masculine);
        let f = Proportions::derive(1.7, 1.0, Build::Average, Frame::Feminine);
        assert!(m.shoulder_w / m.hip_w > f.shoulder_w / f.hip_w);
    }
    #[test]
    fn build_scales_limb_radius_monotonically() {
        let s = Proportions::derive(1.7, 1.0, Build::Slim, Frame::Neutral);
        let a = Proportions::derive(1.7, 1.0, Build::Average, Frame::Neutral);
        let h = Proportions::derive(1.7, 1.0, Build::Heavy, Frame::Neutral);
        assert!(s.arm_r < a.arm_r && a.arm_r < h.arm_r);
        assert!(s.leg_r < a.leg_r && a.leg_r < h.leg_r);
    }
    #[test]
    fn baseline_matches_average_neutral_derive() {
        let baseline = Proportions::baseline(1.7, 1.0);
        let derived = Proportions::derive(1.7, 1.0, Build::Average, Frame::Neutral);
        assert_eq!(baseline, derived);
    }
}
