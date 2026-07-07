//! Structural GLB checks: chunk layout, accessor/bufferView bounds,
//! per-primitive attribute counts, animation sampler pairing, embedded PNG
//! magic, morph-target counts, skin consistency, instancing attributes.

use std::path::Path;

pub fn validate_glb(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    validate_glb_bytes(&data)
}

pub fn validate_glb_bytes(data: &[u8]) -> Result<String, String> {
    use serde_json::Value;
    if data.len() < 20 || &data[0..4] != b"glTF" {
        return Err("not a GLB (missing magic)".into());
    }
    let total = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if total != data.len() {
        return Err(format!("length field {total} != file size {}", data.len()));
    }
    let json_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    if &data[16..20] != b"JSON" {
        return Err("first chunk is not JSON".into());
    }
    let doc: Value =
        serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| format!("bad JSON: {e}"))?;
    let bin_hdr = 20 + json_len;
    let bin_len = u32::from_le_bytes(data[bin_hdr..bin_hdr + 4].try_into().unwrap()) as usize;
    if &data[bin_hdr + 4..bin_hdr + 8] != b"BIN\0" {
        return Err("missing BIN chunk".into());
    }
    let bin = &data[bin_hdr + 8..bin_hdr + 8 + bin_len];

    let views = doc["bufferViews"].as_array().cloned().unwrap_or_default();
    let accessors = doc["accessors"].as_array().cloned().unwrap_or_default();
    let comp_size = |ct: u64| match ct {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => 0,
    };
    let type_count = |t: &str| match t {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        "MAT4" => 16,
        _ => 0,
    };
    for (i, a) in accessors.iter().enumerate() {
        let count = a["count"].as_u64().unwrap_or(0);
        if count == 0 {
            return Err(format!("accessor {i} has zero count"));
        }
        let bv = a["bufferView"].as_u64().unwrap_or(u64::MAX) as usize;
        let v = views
            .get(bv)
            .ok_or(format!("accessor {i}: bad bufferView"))?;
        let off = v["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = v["byteLength"].as_u64().unwrap_or(0) as usize;
        if off + len > bin.len() {
            return Err(format!("bufferView {bv} out of BIN bounds"));
        }
        let need = count
            * comp_size(a["componentType"].as_u64().unwrap_or(0))
            * type_count(a["type"].as_str().unwrap_or(""));
        if need > len as u64 {
            return Err(format!("accessor {i} needs {need} bytes, view has {len}"));
        }
    }
    let acc_count = |idx: &Value| -> u64 {
        accessors[idx.as_u64().unwrap() as usize]["count"]
            .as_u64()
            .unwrap()
    };
    let mut tris = 0u64;
    for mesh in doc["meshes"].as_array().unwrap_or(&Vec::new()) {
        for prim in mesh["primitives"].as_array().unwrap_or(&Vec::new()) {
            let attrs = prim["attributes"]
                .as_object()
                .ok_or("primitive without attributes")?;
            let pos = attrs.get("POSITION").ok_or("primitive without POSITION")?;
            let n = acc_count(pos);
            for (k, v) in attrs {
                if acc_count(v) != n {
                    return Err(format!("attribute {k} count != POSITION count"));
                }
            }
            tris += acc_count(&prim["indices"]) / 3;
            if let Some(targets) = prim["targets"].as_array() {
                let weights = mesh["weights"].as_array().map(|w| w.len()).unwrap_or(0);
                if weights != targets.len() {
                    return Err("mesh.weights count != morph target count".into());
                }
                for t in targets {
                    if acc_count(&t["POSITION"]) != n {
                        return Err("morph target count != POSITION count".into());
                    }
                }
            }
        }
    }
    for anim in doc["animations"].as_array().unwrap_or(&Vec::new()) {
        let samplers = anim["samplers"].as_array().unwrap();
        for s in samplers {
            if acc_count(&s["input"]) != acc_count(&s["output"]) {
                return Err("animation sampler input/output count mismatch".into());
            }
        }
        for ch in anim["channels"].as_array().unwrap() {
            let s = ch["sampler"].as_u64().unwrap() as usize;
            if s >= samplers.len() {
                return Err("channel points at missing sampler".into());
            }
        }
    }
    for (i, img) in doc["images"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .enumerate()
    {
        let bv = img["bufferView"].as_u64().unwrap() as usize;
        let off = views[bv]["byteOffset"].as_u64().unwrap() as usize;
        if &bin[off + 1..off + 4] != b"PNG" {
            return Err(format!("image {i} is not a PNG"));
        }
    }
    if let Some(skins) = doc["skins"].as_array() {
        for s in skins {
            let joints = s["joints"].as_array().unwrap().len() as u64;
            if acc_count(&s["inverseBindMatrices"]) != joints {
                return Err("skin IBM count != joint count".into());
            }
        }
    }
    for node in doc["nodes"].as_array().unwrap_or(&Vec::new()) {
        if let Some(inst) = node["extensions"]["EXT_mesh_gpu_instancing"]["attributes"].as_object()
        {
            let n = acc_count(&inst["TRANSLATION"]);
            for k in ["ROTATION", "SCALE"] {
                if acc_count(&inst[k]) != n {
                    return Err(format!("instancing {k} count mismatch"));
                }
            }
        }
    }
    let n_anims = doc["animations"].as_array().map(|a| a.len()).unwrap_or(0);
    let n_imgs = doc["images"].as_array().map(|a| a.len()).unwrap_or(0);
    Ok(format!("{tris} tris, {n_anims} clips, {n_imgs} images"))
}

/// Structural GLB validation plus boss-specific checks on the
/// `nodes[0].extras.imaginu_boss` metadata block (format `imaginu-boss/1`).
pub fn validate_boss_bytes(data: &[u8]) -> Result<String, String> {
    use serde_json::Value;
    validate_glb_bytes(data)?;

    let json_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let doc: Value =
        serde_json::from_slice(&data[20..20 + json_len]).map_err(|e| format!("bad JSON: {e}"))?;

    let boss = doc["nodes"][0]["extras"]["imaginu_boss"].as_object();
    let boss = match boss {
        Some(b) => b,
        None => return Err("not a boss asset: missing imaginu_boss extras".into()),
    };

    if boss.get("format").and_then(|v| v.as_str()) != Some("imaginu-boss/1") {
        return Err(format!(
            "unexpected imaginu_boss format: {:?}",
            boss.get("format")
        ));
    }

    // Collect valid joint names from the skin (skins[0].joints -> node names).
    let mut joint_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(skin) = doc["skins"].get(0) {
        if let Some(joints) = skin["joints"].as_array() {
            for j in joints {
                if let Some(idx) = j.as_u64() {
                    if let Some(name) = doc["nodes"][idx as usize]["name"].as_str() {
                        joint_names.insert(name.to_string());
                    }
                }
            }
        }
    }

    let archetype = boss
        .get("archetype")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let phases = boss
        .get("phases")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if phases.is_empty() {
        return Err("imaginu_boss: phases must be non-empty".into());
    }
    let mut last_id: Option<u64> = None;
    for p in &phases {
        let id = p["id"].as_u64().ok_or("imaginu_boss: phase missing id")?;
        if let Some(prev) = last_id {
            if id < prev {
                return Err(format!(
                    "imaginu_boss: phases not ordered by ascending id ({prev} then {id})"
                ));
            }
        }
        last_id = Some(id);
        for ability in p["abilities"].as_array().unwrap_or(&Vec::new()) {
            for key in ["telegraph_s", "active_s", "recover_s"] {
                let v = ability[key]
                    .as_f64()
                    .ok_or_else(|| format!("imaginu_boss: ability missing {key}"))?;
                if v < 0.0 {
                    return Err(format!("imaginu_boss: ability {key} must be >= 0, got {v}"));
                }
            }
        }
    }

    let weak_points = boss
        .get("weak_points")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for wp in &weak_points {
        let joint = wp["joint"]
            .as_str()
            .ok_or("imaginu_boss: weak point missing joint")?;
        if !joint_names.contains(joint) {
            return Err(format!(
                "imaginu_boss: weak point references nonexistent joint {joint:?}"
            ));
        }
    }

    let parts = boss
        .get("parts")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for part in &parts {
        let joint = part["joint"]
            .as_str()
            .ok_or("imaginu_boss: part missing joint")?;
        if !joint_names.contains(joint) {
            return Err(format!(
                "imaginu_boss: part references nonexistent joint {joint:?}"
            ));
        }
    }

    let radius = boss["arena"]["recommended_radius"]
        .as_f64()
        .ok_or("imaginu_boss: arena.recommended_radius missing")?;
    if radius <= 0.0 {
        return Err(format!(
            "imaginu_boss: arena.recommended_radius must be > 0, got {radius}"
        ));
    }

    Ok(format!(
        "boss: {archetype}, {} phases, {} weak points, {} parts",
        phases.len(),
        weak_points.len(),
        parts.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_boss_accepts_hydra() {
        let asset = crate::recipe::Recipe::parse(r#"{"kind":"boss","archetype":"hydra"}"#)
            .unwrap()
            .build()
            .unwrap();
        let glb = crate::gltf::to_glb(&asset);
        let summary = validate_boss_bytes(&glb);
        assert!(summary.is_ok(), "{:?}", summary);
        assert!(summary.unwrap().starts_with("boss: hydra"));
    }

    #[test]
    fn validate_boss_rejects_non_boss_asset() {
        let asset = crate::recipe::Recipe::parse(r#"{"kind":"tree","style":"oak"}"#)
            .unwrap()
            .build()
            .unwrap();
        let glb = crate::gltf::to_glb(&asset);
        let err = validate_boss_bytes(&glb).unwrap_err();
        assert!(err.contains("missing imaginu_boss"), "{err}");
    }

    #[test]
    fn validate_boss_rejects_bad_joint() {
        let asset = crate::recipe::Recipe::parse(r#"{"kind":"boss","archetype":"hydra"}"#)
            .unwrap()
            .build()
            .unwrap();
        let glb = crate::gltf::to_glb(&asset);

        // Doctor the JSON chunk: repoint the first weak point's joint at a
        // nonexistent name, then re-encode as a GLB with the same BIN chunk.
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let mut doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        doc["nodes"][0]["extras"]["imaginu_boss"]["weak_points"][0]["joint"] =
            serde_json::Value::String("__no_such_joint__".into());
        let new_json = serde_json::to_vec(&doc).unwrap();
        let padded_len = new_json.len().div_ceil(4) * 4;
        let mut new_json = new_json;
        new_json.resize(padded_len, b' ');

        let bin_hdr = 20 + json_len;
        let bin_chunk = &glb[bin_hdr..];

        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes()); // version
        let total = 12 + 8 + new_json.len() + bin_chunk.len();
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(new_json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&new_json);
        out.extend_from_slice(bin_chunk);

        let err = validate_boss_bytes(&out).unwrap_err();
        assert!(err.contains("nonexistent joint"), "{err}");
    }
}
