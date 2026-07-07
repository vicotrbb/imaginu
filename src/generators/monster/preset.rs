//! M7 — `MonsterClass` presets. A preset is a bundle of knob values applied
//! BEFORE the plan builds, filling only fields the user left unset so explicit
//! recipe knobs always win. Sentinel floats (`< 0.0`) and the eye sentinel
//! (`== -1`) are the unambiguous "unset" markers; the zero-default knobs
//! (horns/spikes/plates/menace/age) and the `size`/`1.0` default are treated as
//! unset only when still at their default value.
//!
//! Palette preference (Elemental->infernal, Undead->necrotic,
//! Aberration->fungal) lives on the `Recipe`, not on `MonsterParams`, so it is
//! handled in `recipe::Recipe::build` — see `preferred_palette` there.

use crate::recipe::{MonsterClass, MonsterParams};

/// Fill a sentinel float (`-1.0` = unset) if the user did not set it.
fn set_sentinel(field: &mut f32, val: f32) {
    if *field < 0.0 {
        *field = val;
    }
}

/// Fill a zero-default knob (default `0.0`) if still at its default.
fn set_if_zero(field: &mut f32, val: f32) {
    if *field == 0.0 {
        *field = val;
    }
}

/// Apply the class preset in place. `None` leaves every knob untouched.
pub fn apply_preset(p: &mut MonsterParams) {
    match p.class {
        MonsterClass::None => {}
        MonsterClass::Predator => {
            set_sentinel(&mut p.maw, 0.8);
            set_if_zero(&mut p.spikes, 0.3);
            set_if_zero(&mut p.menace, 0.5);
        }
        MonsterClass::Brute => {
            if p.size == 1.0 {
                p.size = 1.4;
            }
            set_if_zero(&mut p.plates, 0.6);
            set_if_zero(&mut p.menace, 0.6);
        }
        MonsterClass::Elemental => {
            set_sentinel(&mut p.emissive, 0.8);
            set_if_zero(&mut p.horns, 0.5);
        }
        MonsterClass::Undead => {
            set_sentinel(&mut p.emissive, 0.3);
            set_if_zero(&mut p.age, 0.7);
            set_if_zero(&mut p.spikes, 0.2);
        }
        MonsterClass::Aberration => {
            if p.eyes == -1 {
                p.eyes = 5;
            }
            set_sentinel(&mut p.tail, 0.0);
            set_sentinel(&mut p.emissive, 0.4);
        }
        MonsterClass::Swarm => {
            if p.size == 1.0 {
                p.size = 0.6;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_from(json: &str) -> MonsterParams {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn undead_preset_sets_defaults_but_respects_overrides() {
        let mut p = params_from(r#"{"kind":"monster","class":"undead"}"#);
        apply_preset(&mut p);
        assert!(p.emissive > 0.0);
        // explicit override wins
        let mut p2 = params_from(r#"{"kind":"monster","class":"undead","emissive":0.0}"#);
        apply_preset(&mut p2);
        assert_eq!(p2.emissive, 0.0);
    }

    #[test]
    fn brute_size_only_overrides_the_default() {
        let mut p = params_from(r#"{"kind":"monster","class":"brute"}"#);
        apply_preset(&mut p);
        assert_eq!(p.size, 1.4);
        // explicit size wins
        let mut p2 = params_from(r#"{"kind":"monster","class":"brute","size":2.0}"#);
        apply_preset(&mut p2);
        assert_eq!(p2.size, 2.0);
    }

    #[test]
    fn none_preset_is_inert() {
        let mut p = params_from(r#"{"kind":"monster"}"#);
        let before = serde_json::to_string(&p).unwrap();
        apply_preset(&mut p);
        assert_eq!(before, serde_json::to_string(&p).unwrap());
    }
}
