//! Archetype preset: fills unset (sentinel) boss knobs per archetype so a bare
//! `{"archetype":"hydra"}` reads right; explicit user knobs always win.

use crate::recipe::{BossArchetype, BossParams};

fn set_neg(f: &mut f32, val: f32) {
    if *f < 0.0 {
        *f = val;
    }
}
fn set_neg_i(f: &mut i32, val: i32) {
    if *f < 0 {
        *f = val;
    }
}

pub fn apply_archetype_preset(p: &mut BossParams) {
    match p.archetype {
        BossArchetype::Hydra => {
            set_neg(&mut p.spikes, 0.5);
            set_neg(&mut p.maw, 0.7);
            set_neg_i(&mut p.eyes, 2);
            set_neg(&mut p.menace, 0.6);
            set_neg(&mut p.emissive, 0.5);
        }
        BossArchetype::Colossus => {
            set_neg(&mut p.armor, 1.0);
            set_neg(&mut p.plates, 1.0);
            set_neg(&mut p.menace, 0.8);
            set_neg(&mut p.emissive, 0.4);
            set_neg(&mut p.horns, 0.3);
        }
        BossArchetype::Lich => {
            set_neg(&mut p.regalia, 1.0);
            set_neg(&mut p.crown, 1.0);
            set_neg_i(&mut p.eyes, 2);
            set_neg(&mut p.emissive, 0.7);
        }
        BossArchetype::SwarmQueen => {
            set_neg_i(&mut p.eyes, 6);
            set_neg(&mut p.spikes, 0.6);
            set_neg(&mut p.emissive, 0.5);
        }
        BossArchetype::DragonLord => {
            set_neg(&mut p.wings, 1.0);
            set_neg(&mut p.horns, 0.8);
            set_neg(&mut p.spikes, 0.6);
            set_neg(&mut p.maw, 0.8);
            set_neg(&mut p.emissive, 0.6);
        }
    }
    // Any remaining sentinels resolve to "off" downstream; clamp size sanity.
    if p.size <= 0.0 {
        p.size = 3.0;
    }
    p.phases = p.phases.clamp(1, 4);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::BossParams;

    #[test]
    fn preset_fills_sentinels_explicit_wins() {
        let mut p: BossParams =
            serde_json::from_str(r#"{"archetype":"colossus","horns":0.9}"#).unwrap();
        apply_archetype_preset(&mut p);
        assert!((p.horns - 0.9).abs() < 1e-6, "explicit horns wins");
        assert!(p.plates >= 0.0, "colossus preset filled plate sentinel");
        assert!(p.armor >= 0.0, "colossus preset filled armor");
    }
}
