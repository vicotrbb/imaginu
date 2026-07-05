use clap::{Parser, Subcommand};
use std::path::PathBuf;

use imaginu::recipe::Recipe;
use imaginu::render::{auto_camera, render_png};

#[derive(Parser)]
#[command(name = "imaginu", version, about = "AI-drivable 3D asset compiler: JSON recipe -> GLB (+ PNG preview)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compile a recipe (file path or inline JSON) to a GLB.
    Generate {
        /// Recipe file path, or inline JSON starting with '{'
        recipe: String,
        /// Output GLB path
        #[arg(short, long, default_value = "asset.glb")]
        out: PathBuf,
        /// Also render a PNG preview next to the GLB
        #[arg(long)]
        preview: bool,
        /// Emit N decimated LOD levels into the GLB (MSFT_lod extension)
        #[arg(long, default_value_t = 0)]
        lods: u32,
    },
    /// Render turntable PNGs (4 angles) of a recipe, without keeping the GLB.
    /// With --animation, renders 4 clip phases (t = 0, ¼, ½, ¾) instead.
    Render {
        recipe: String,
        /// Output directory for PNGs
        #[arg(short, long, default_value = ".")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 900)]
        width: usize,
        #[arg(long, default_value_t = 640)]
        height: usize,
        /// Pose the asset with this animation clip
        #[arg(long)]
        animation: Option<String>,
        /// Single explicit clip time (seconds) instead of the 4 phases
        #[arg(long)]
        at: Option<f32>,
        /// Apply a morph target (e.g. smile, blink, angry, surprised) at full weight
        #[arg(long)]
        expression: Option<String>,
    },
    /// Render a loop-perfect turntable video of an asset (for showcasing).
    Showcase {
        /// Recipe file path, or inline JSON starting with '{'
        recipe: String,
        /// Output video path (mp4; requires ffmpeg on PATH)
        #[arg(short, long, default_value = "showcase.mp4")]
        out: PathBuf,
        #[arg(long, default_value_t = 800)]
        size: usize,
        /// Seconds for one full rotation
        #[arg(long, default_value_t = 4.0)]
        duration: f32,
        #[arg(long, default_value_t = 30)]
        fps: u32,
        /// Camera pitch in degrees
        #[arg(long, default_value_t = 18.0)]
        pitch: f32,
        /// Keep the rendered PNG frames next to the video
        #[arg(long)]
        keep_frames: bool,
        /// Play this animation clip (fixed ¾ camera) instead of a turntable
        #[arg(long)]
        animation: Option<String>,
    },
    /// Print the recipe JSON schema cheat-sheet (for AI agents).
    Schema,
}

fn load_recipe(s: &str) -> Result<Recipe, String> {
    let json = if s.trim_start().starts_with('{') {
        s.to_string()
    } else {
        std::fs::read_to_string(s).map_err(|e| format!("cannot read {s}: {e}"))?
    };
    Recipe::parse(&json)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    match Cli::parse().cmd {
        Cmd::Generate { recipe, out, preview, lods } => {
            let r = load_recipe(&recipe)?;
            let mut asset = r.build()?;
            if lods > 0 {
                asset.generate_lods(lods);
            }
            let glb = imaginu::gltf::to_glb(&asset);
            std::fs::write(&out, &glb).map_err(|e| e.to_string())?;
            let tris: usize = asset.parts.iter().map(|p| p.mesh.triangle_count()).sum();
            println!(
                "wrote {} ({} KiB, {} triangles, {} animation clip(s))",
                out.display(),
                glb.len() / 1024,
                tris,
                asset.animations.len()
            );
            if preview {
                let png = out.with_extension("png");
                let (pitch, zoom) = if asset.name == "terrain" { (33.0, 0.82) } else { (20.0, 0.95) };
                let cam = auto_camera(&asset, 35.0, pitch, zoom);
                render_png(&asset, &cam, 900, 640, &png).map_err(|e| e.to_string())?;
                println!("wrote {}", png.display());
            }
            Ok(())
        }
        Cmd::Render { recipe, out_dir, width, height, animation, at, expression } => {
            let r = load_recipe(&recipe)?;
            let mut asset = r.build()?;
            if let Some(expr) = &expression {
                let mut found = false;
                for part in &mut asset.parts {
                    if let Some(m) = part.mesh.morphs.iter().find(|m| m.name == *expr) {
                        let deltas = m.deltas.clone();
                        for (p, d) in part.mesh.positions.iter_mut().zip(&deltas) {
                            *p += *d;
                        }
                        found = true;
                    }
                }
                if !found {
                    return Err(format!("no morph target '{expr}' in this asset"));
                }
            }
            std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
            match &animation {
                None => {
                    for (i, yaw) in [25.0f32, 115.0, 205.0, 295.0].iter().enumerate() {
                        let cam = auto_camera(&asset, *yaw, 20.0, 1.0);
                        let path = out_dir.join(format!("{}_{}.png", asset.name, i));
                        render_png(&asset, &cam, width, height, &path)
                            .map_err(|e| e.to_string())?;
                        println!("wrote {}", path.display());
                    }
                }
                Some(clip_name) => {
                    let clip = asset
                        .animations
                        .iter()
                        .find(|c| c.name == *clip_name)
                        .ok_or_else(|| format!("no clip '{clip_name}'"))?;
                    let dur = imaginu::anim::clip_duration(clip);
                    let times: Vec<f32> = match at {
                        Some(t) => vec![t],
                        None => (0..4).map(|i| i as f32 / 4.0 * dur).collect(),
                    };
                    // camera framed on the bind pose so frames don't jump
                    let cam = auto_camera(&asset, 35.0, 12.0, 1.0);
                    for (i, t) in times.iter().enumerate() {
                        let posed = imaginu::anim::pose_asset(&asset, clip_name, *t)?;
                        let path =
                            out_dir.join(format!("{}_{}_{}.png", asset.name, clip_name, i));
                        render_png(&posed, &cam, width, height, &path)
                            .map_err(|e| e.to_string())?;
                        println!("wrote {} (t={t:.2}s)", path.display());
                    }
                }
            }
            Ok(())
        }
        Cmd::Showcase { recipe, out, size, duration, fps, pitch, keep_frames, animation } => {
            let r = load_recipe(&recipe)?;
            let asset = r.build()?;
            let fps = fps.clamp(10, 60);
            let mut duration = duration.clamp(1.0, 30.0);
            let clip_dur = match &animation {
                Some(name) => {
                    let clip = asset
                        .animations
                        .iter()
                        .find(|c| c.name == *name)
                        .ok_or_else(|| format!("no clip '{name}'"))?;
                    let d = imaginu::anim::clip_duration(clip).max(0.1);
                    // round video length to whole clip loops so it loops clean
                    duration = (duration / d).round().max(1.0) * d;
                    Some(d)
                }
                None => None,
            };
            let frames = (duration * fps as f32) as usize;
            let dir = out.with_extension("frames");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let (cam_pitch, zoom) = if asset.name == "terrain" {
                (pitch.max(30.0), 0.85)
            } else {
                (pitch, 0.95)
            };
            eprint!("rendering {frames} frames ");
            for f in 0..frames {
                let path = dir.join(format!("frame_{f:04}.png"));
                match (&animation, clip_dur) {
                    (Some(name), Some(d)) => {
                        let t = (f as f32 / fps as f32) % d;
                        let posed = imaginu::anim::pose_asset(&asset, name, t)?;
                        // fixed ¾ camera framed on the bind pose
                        let cam = auto_camera(&asset, 35.0, cam_pitch.min(15.0), zoom);
                        render_png(&posed, &cam, size, size, &path)
                            .map_err(|e| e.to_string())?;
                    }
                    _ => {
                        let yaw = f as f32 / frames as f32 * 360.0;
                        let cam = auto_camera(&asset, yaw, cam_pitch, zoom);
                        render_png(&asset, &cam, size, size, &path)
                            .map_err(|e| e.to_string())?;
                    }
                }
                if f % 10 == 0 {
                    eprint!(".");
                }
            }
            eprintln!(" done");
            let status = std::process::Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-framerate"])
                .arg(fps.to_string())
                .arg("-i")
                .arg(dir.join("frame_%04d.png"))
                .args(["-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart"])
                .arg(&out)
                .status()
                .map_err(|e| format!("ffmpeg not found or failed to start: {e}"))?;
            if !status.success() {
                return Err("ffmpeg failed to encode the video".into());
            }
            if !keep_frames {
                std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            }
            let bytes = std::fs::metadata(&out).map_err(|e| e.to_string())?.len();
            println!(
                "wrote {} ({} KiB, {frames} frames, {}x{size} @ {fps}fps, loop-perfect)",
                out.display(),
                bytes / 1024,
                size
            );
            Ok(())
        }
        Cmd::Schema => {
            println!("{}", SCHEMA_HELP);
            Ok(())
        }
    }
}

const SCHEMA_HELP: &str = r##"imaginu recipe cheat-sheet (JSON, all fields except "kind" optional):

palettes: verdant | autumn | arctic | volcanic | desert | mystic

{"kind":"terrain","palette":"verdant","seed":1,"size":48,"resolution":110,
 "mountainousness":1.0,"water_level":0.28,"scatter":true,
 "shape":"hills|mountains|island|archipelago|canyon|mesa|crater|valley|dunes",
 "terrace":0,"skirt":true,"offset_x":0,"offset_z":0}
 // any size (4..4096). For seamless world tiles: skirt=false and offset_x/z
 // = chunk world position; adjacent chunks share identical edge heights.

{"kind":"tree","style":"oak|pine|palm|dead","height":6,"seed":1}

{"kind":"rock","size":1.0,"jaggedness":0.6,"seed":1}

{"kind":"crystal","size":1.0,"count":7,"palette":"mystic","seed":1}

{"kind":"building","width":6,"floors":1,"seed":1}

{"kind":"prop","prop":"barrel|crate|lantern|campfire","size":1.0,"seed":1}

{"kind":"character","class":"villager|warrior|mage|rogue","height":1.7,
 "bulk":1.0,"animate":true,"seed":1,
 "hair":"short|ponytail|bun|bald","skin_tone":0,"expressions":true}
 // smooth subdivision bodies, mitten hands, faces (eyes/brows/nose/mouth),
 // facial morph targets: smile, blink, angry, surprised (glTF blend shapes)

{"kind":"custom","name":"anything","seed":1,
 "physics":{"collider":"auto|box|sphere|capsule|trimesh","mass":0,
            "friction":0.6,"restitution":0},
 "bones":[{"name":"root"},{"name":"arm","parent":"root","translation":[0,2,0]}],
 "animations":[{"name":"spin","duration":2,"channels":[
   {"bone":"arm","path":"rotation","axis":[0,0,1],"keys":[0,180,360]},
   {"bone":"arm","path":"rotation","keys_euler":[[0,0,0],[30,45,0],[0,0,0]],
    "ease":"cubic_in_out"},
   {"bone":"root","path":"translation","axis":[0,1,0],"keys":[0,0.4,0]}]}],
 "parts":[{"material":{"metallic":0,"roughness":0.9,
                       "emissive":"#ffaa33","emissive_strength":1.5,
                       "texture":{"pattern":"wood|rock|fabric|metal|plaster|noise",
                                  "scale":1.0,"seed":1,"normal_strength":1.0,
                                  "resolution":1024,"colors":["#5a3c26","#9c7248"]}},
   "nodes":[
     {"shape":"box","size":[1,1,1],"color":"#8a6242"},
     {"shape":"sphere","radius":1,"subdiv":2,"color":[0.5,0.7,0.9],
      "displace":{"amplitude":0.2,"frequency":2},"flat":true},
     {"shape":"lathe","profile":[[0.5,0],[0.3,1]],"segments":12,"color":"#fff0d0",
      "color_top":"#803020"},
     {"shape":"cylinder","radius":0.5,"height":2,"color":"#999999"},
     {"shape":"cone","radius":1,"height":2,"color":"#777777"},
     {"shape":"tube","path":[[0,0,0],[0,1,0.3],[0,2,0]],"radius":[0.2,0.15,0.05],
      "color":"#665544"},
     {"shape":"prism","sides":6,"radius":0.3,"height":1,"point":0.4,"color":"#66ffee"},
     {"shape":"curve","points":[[0,0,0],[1,1,0],[2,1,1]],"radius":[0.2,0.05],
      "samples":24,"color":"#775533"},
     {"shape":"box","size":[4,3,2],"color":"#999999","bevel":0.1,
      "subdivide":0,"smooth":false,
      "csg":[{"op":"subtract","shape":"cylinder","radius":1,"height":3,
              "color":"#999999","transform":{"rotate_deg":[-90,0,0],
                                             "translate":[0,0,1.5]}}]},
     {"shape":"box","size":[0.2,1,0.2],"color":"#ffffff","bone":"arm",
      "transform":{"translate":[0,1,0],"rotate_deg":[0,45,0],"scale":[1,1,1]},
      "repeat":{"count":8,"radius":2.0,"orient":true}}]}]}
 // every node: optional transform/displace/flat/repeat/bone/color_top/uv
 // material.texture bakes a seamless procedural PBR texture set (baseColor +
 // normal map + occlusion/roughness/metallic PNGs) into the GLB. scale =
 // world units per tile; colors = optional dark->light #hex ramp override.
 // node colors MULTIPLY the texture - use "#ffffff" to show it unchanged.
 // node.uv picks the projection: box (default) | cylinder | planar
 // node "skin":"smooth" = automatic multi-joint weights over all bones
 // (seamless organic bending); "bone":"name" = rigid binding.
 // geometry v2: "bevel":w chamfers box/prism edges; "subdivide":n (+"smooth")
 // rounds organically; "csg":[{"op":"subtract|union|intersect", <node>}]
 // carves arches/windows/holes; "curve" sweeps a Catmull-Rom tube.
 // LODs: imaginu generate <recipe> --lods 3 embeds decimated levels
 // (MSFT_lod + screencoverage extras; Babylon switches automatically).

Output GLB embeds physics metadata at nodes[0].extras.imaginu_physics:
{collider:{type:box|sphere|capsule|trimesh|heightfield,...},mass,friction,restitution}
Characters include a 17-joint skeleton, smooth multi-joint skinning, and clips:
idle, walk, run, attack, sit, wave, death, dance.
Channel easing: "ease":"cubic_in|cubic_out|cubic_in_out" (baked to dense keys);
multi-axis rotation via "keys_euler":[[x,y,z]deg,...].
See animation frames: imaginu render <recipe> --animation walk [--at 0.5]
Film a clip:         imaginu showcase <recipe> --animation dance -o dance.mp4"##;
