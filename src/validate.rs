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
        let v = views.get(bv).ok_or(format!("accessor {i}: bad bufferView"))?;
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
    let acc_count =
        |idx: &Value| -> u64 { accessors[idx.as_u64().unwrap() as usize]["count"].as_u64().unwrap() };
    let mut tris = 0u64;
    for mesh in doc["meshes"].as_array().unwrap_or(&Vec::new()) {
        for prim in mesh["primitives"].as_array().unwrap_or(&Vec::new()) {
            let attrs = prim["attributes"].as_object().ok_or("primitive without attributes")?;
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
    for (i, img) in doc["images"].as_array().unwrap_or(&Vec::new()).iter().enumerate() {
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
