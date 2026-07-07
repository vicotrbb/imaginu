use clap::{Parser, Subcommand};
use std::path::PathBuf;

use imaginu::recipe::Recipe;
use imaginu::render::{auto_camera, render_png};

#[derive(Parser)]
#[command(
    name = "imaginu",
    version,
    about = "AI-drivable 3D asset compiler: JSON recipe -> GLB (+ PNG preview)"
)]
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
    /// Byte-level structural validation of GLB files (accessors, samplers,
    /// images, morphs, skins, instancing).
    Validate {
        /// GLB files to check
        files: Vec<PathBuf>,
    },
    /// Compile a `{"kind":"world"}` recipe into a directory of seamless,
    /// streamable chunk GLBs + manifest.json.
    World {
        /// Recipe file path, or inline JSON starting with '{'
        recipe: String,
        /// Output directory (created if missing)
        #[arg(short, long, default_value = "world_out")]
        out: PathBuf,
        /// Build only chunk "x,z" (lazy iteration; manifest still covers the
        /// full grid)
        #[arg(long)]
        chunk: Option<String>,
        /// Render an oblique PNG preview next to each built chunk GLB
        #[arg(long)]
        preview: bool,
        /// Render a top-down minimap PNG (zones + hillshade + water + POIs)
        /// to <out>/map.png
        #[arg(long)]
        map: bool,
        /// Only write manifest.json + map.png — no chunk builds (fast layout
        /// iteration)
        #[arg(long)]
        map_only: bool,
        /// Render an oblique full-map beauty shot to <out>/overview.png
        /// (stitched downsampled world + water + rivers + POI geometry)
        #[arg(long)]
        overview: bool,
        /// Render a flyover MP4 along "x0,z0:x1,z1" (world coords) to
        /// <out>/flyover.mp4 — real chunks near the path, ffmpeg required
        #[arg(long)]
        flyover: Option<String>,
    },
    /// Validate a world output directory (manifest + all chunk GLBs).
    ValidateWorld { dir: PathBuf },
    /// Compile a `{"kind":"dungeon"}` recipe into a themed layout: one merged
    /// GLB for a single room, otherwise a directory of per-room GLBs +
    /// manifest.json.
    Dungeon {
        /// Recipe file path, or inline JSON starting with '{'
        recipe: String,
        /// Output directory (created if missing)
        #[arg(short, long, default_value = "dungeon_out")]
        out: PathBuf,
        /// Render a near-top-down, ceiling-less beauty shot of the interior
        /// to <out>/overview.png
        #[arg(long)]
        overview: bool,
    },
    /// Validate a dungeon output directory (manifest + all room GLBs).
    ValidateDungeon { dir: PathBuf },
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
        Cmd::Generate {
            recipe,
            out,
            preview,
            lods,
        } => {
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
                let (pitch, zoom) = if asset.name == "terrain" {
                    (33.0, 0.82)
                } else {
                    (20.0, 0.95)
                };
                let cam = auto_camera(&asset, 35.0, pitch, zoom);
                render_png(&asset, &cam, 900, 640, &png).map_err(|e| e.to_string())?;
                println!("wrote {}", png.display());
            }
            Ok(())
        }
        Cmd::Render {
            recipe,
            out_dir,
            width,
            height,
            animation,
            at,
            expression,
        } => {
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
                        let path = out_dir.join(format!("{}_{}_{}.png", asset.name, clip_name, i));
                        render_png(&posed, &cam, width, height, &path)
                            .map_err(|e| e.to_string())?;
                        println!("wrote {} (t={t:.2}s)", path.display());
                    }
                }
            }
            Ok(())
        }
        Cmd::Showcase {
            recipe,
            out,
            size,
            duration,
            fps,
            pitch,
            keep_frames,
            animation,
        } => {
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
                        render_png(&posed, &cam, size, size, &path).map_err(|e| e.to_string())?;
                    }
                    _ => {
                        let yaw = f as f32 / frames as f32 * 360.0;
                        let cam = auto_camera(&asset, yaw, cam_pitch, zoom);
                        render_png(&asset, &cam, size, size, &path).map_err(|e| e.to_string())?;
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
                .args([
                    "-pix_fmt",
                    "yuv420p",
                    "-crf",
                    "18",
                    "-movflags",
                    "+faststart",
                ])
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
        Cmd::World {
            recipe,
            out,
            chunk,
            preview,
            map,
            map_only,
            overview,
            flyover,
        } => {
            let json = if recipe.trim_start().starts_with('{') {
                recipe.clone()
            } else {
                std::fs::read_to_string(&recipe)
                    .map_err(|e| format!("cannot read {recipe}: {e}"))?
            };
            let params = imaginu::world::WorldParams::parse(&json)?;
            let model = imaginu::world::model::WorldModel::new(&params)?;
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            let man = imaginu::world::manifest::create(&model);
            let man_json = serde_json::to_string_pretty(&man).map_err(|e| e.to_string())?;
            std::fs::write(out.join("manifest.json"), man_json).map_err(|e| e.to_string())?;
            if map || map_only {
                let px = ((model.size_x / 8.0) as usize).clamp(256, 1600);
                let (w, h, rgb) = imaginu::world::minimap::render(&model, px);
                imaginu::world::minimap::write_png(&out.join("map.png"), w, h, &rgb)?;
                println!("wrote {}/map.png ({w}x{h})", out.display());
            }
            if overview {
                let grid = ((model.size_x / 10.0) as usize).clamp(160, 720);
                let asset = imaginu::world::overview::world_asset(&model, grid);
                let cam = auto_camera(&asset, 40.0, 42.0, 1.0);
                render_png(&asset, &cam, 1400, 1000, &out.join("overview.png"))
                    .map_err(|e| e.to_string())?;
                println!("wrote {}/overview.png", out.display());
            }
            if let Some(seg) = &flyover {
                fly_over(&model, seg, &out)?;
            }
            if map_only {
                return Ok(());
            }
            let jobs: Vec<(u32, u32)> = match &chunk {
                Some(s) => {
                    let (a, b) = s
                        .split_once(',')
                        .ok_or_else(|| format!("--chunk wants \"x,z\", got '{s}'"))?;
                    let cx: u32 = a.trim().parse().map_err(|_| format!("bad chunk x '{a}'"))?;
                    let cz: u32 = b.trim().parse().map_err(|_| format!("bad chunk z '{b}'"))?;
                    if cx >= model.nx || cz >= model.nz {
                        return Err(format!(
                            "chunk {cx},{cz} outside {}×{} grid",
                            model.nx, model.nz
                        ));
                    }
                    vec![(cx, cz)]
                }
                None => (0..model.nz)
                    .flat_map(|cz| (0..model.nx).map(move |cx| (cx, cz)))
                    .collect(),
            };
            let n_workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(jobs.len().max(1));
            let next = std::sync::atomic::AtomicUsize::new(0);
            let errors = std::sync::Mutex::new(Vec::<String>::new());
            let done = std::sync::atomic::AtomicUsize::new(0);
            std::thread::scope(|scope| {
                for _ in 0..n_workers {
                    scope.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= jobs.len() {
                                break;
                            }
                            let (cx, cz) = jobs[i];
                            let mut asset = imaginu::world::chunk::build(&model, cx, cz);
                            if model.p.lods > 0 {
                                asset.generate_lods(model.p.lods);
                            }
                            let glb = imaginu::gltf::to_glb(&asset);
                            let path = out.join(imaginu::world::manifest::chunk_file(cx, cz));
                            if let Err(e) = std::fs::write(&path, &glb) {
                                errors
                                    .lock()
                                    .unwrap()
                                    .push(format!("{}: {e}", path.display()));
                                continue;
                            }
                            if preview {
                                let cam = auto_camera(&asset, 35.0, 38.0, 0.85);
                                let png = path.with_extension("png");
                                if let Err(e) = render_png(&asset, &cam, 800, 600, &png) {
                                    errors
                                        .lock()
                                        .unwrap()
                                        .push(format!("{}: {e}", png.display()));
                                }
                            }
                            let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                            if d.is_multiple_of(16) || d == jobs.len() {
                                eprintln!("  built {d}/{} chunks", jobs.len());
                            }
                        }
                    });
                }
            });
            let errors = errors.into_inner().unwrap();
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
            // POI GLBs (skipped in lazy --chunk mode)
            if chunk.is_none() {
                for (i, site) in model.pois.iter().enumerate() {
                    let asset = imaginu::world::poi::build_asset(site, &model.pal);
                    let glb = imaginu::gltf::to_glb(&asset);
                    let path = out.join(imaginu::world::poi::poi_file(site, i));
                    std::fs::write(&path, &glb).map_err(|e| e.to_string())?;
                    if preview {
                        let cam = auto_camera(&asset, 35.0, 24.0, 0.9);
                        render_png(&asset, &cam, 800, 600, &path.with_extension("png"))
                            .map_err(|e| e.to_string())?;
                    }
                }
                for (i, b) in model.network.bridges.iter().enumerate() {
                    let asset = imaginu::world::poi::bridge_asset(b, &model.pal);
                    let glb = imaginu::gltf::to_glb(&asset);
                    std::fs::write(out.join(format!("poi_bridge_{i}.glb")), &glb)
                        .map_err(|e| e.to_string())?;
                }
                if !model.pois.is_empty() || !model.network.bridges.is_empty() {
                    println!(
                        "wrote {} POI GLB(s) + {} bridge(s)",
                        model.pois.len(),
                        model.network.bridges.len()
                    );
                }
            }
            println!(
                "wrote {}/manifest.json + {} chunk GLB(s) ({}×{} grid, {}×{} m)",
                out.display(),
                jobs.len(),
                model.nx,
                model.nz,
                model.size_x,
                model.size_z
            );
            Ok(())
        }
        Cmd::ValidateWorld { dir } => {
            let summary = imaginu::world::manifest::validate_dir(&dir)?;
            println!("OK   {}  {}", dir.display(), summary);
            Ok(())
        }
        Cmd::Dungeon {
            recipe,
            out,
            overview,
        } => {
            let r = load_recipe(&recipe)?;
            let params = match &r {
                Recipe::Dungeon { params, .. } => params.clone(),
                _ => {
                    return Err(
                        "recipe is not a dungeon (expected \"kind\":\"dungeon\")".to_string()
                    );
                }
            };
            let pal_name = r.resolved_palette().to_string();
            build_dungeon(&params, &pal_name, &out, overview)?;
            Ok(())
        }
        Cmd::ValidateDungeon { dir } => {
            let summary = imaginu::generators::dungeon::manifest::validate_dir(&dir)?;
            println!("OK   {}  {}", dir.display(), summary);
            Ok(())
        }
        Cmd::Validate { files } => {
            let mut failures = 0;
            for f in &files {
                match imaginu::validate::validate_glb(f) {
                    Ok(summary) => println!("OK   {}  {}", f.display(), summary),
                    Err(e) => {
                        println!("FAIL {}  {}", f.display(), e);
                        failures += 1;
                    }
                }
            }
            if failures > 0 {
                return Err(format!("{failures} file(s) failed validation"));
            }
            Ok(())
        }
    }
}

/// Flyover MP4 along a world-space segment: real chunks near the path,
/// eased camera, ffmpeg encode. Deterministic like everything else.
fn fly_over(
    model: &imaginu::world::model::WorldModel,
    seg: &str,
    out: &std::path::Path,
) -> Result<(), String> {
    let nums: Vec<f32> = seg
        .split([':', ','])
        .map(|s| {
            s.trim()
                .parse::<f32>()
                .map_err(|_| format!("bad flyover coord '{s}'"))
        })
        .collect::<Result<_, _>>()?;
    if nums.len() != 4 {
        return Err("--flyover wants \"x0,z0:x1,z1\"".into());
    }
    let (a, b) = (
        glam::Vec2::new(nums[0], nums[1]),
        glam::Vec2::new(nums[2], nums[3]),
    );
    eprintln!("building flyover corridor…");
    let asset = imaginu::world::overview::corridor_asset(model, a, b, 380.0);
    let (fps, frames, w, h) = (24u32, 96usize, 960usize, 560usize);
    let dir = out.join("flyover.frames");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    eprint!("rendering {frames} frames ");
    for f in 0..frames {
        let t = f as f32 / (frames - 1) as f32;
        // ease in/out so the flight feels filmed, not scripted
        let te = t * t * (3.0 - 2.0 * t);
        let p = a + (b - a) * te;
        // constant-distance lookahead so the shot never tips straight down
        let fly_dir = (b - a).normalize_or_zero();
        let ahead = p + fly_dir * 240.0;
        let ground = model.height(p.x, p.y).max(model.p.sea_level);
        let g2 = model.height(ahead.x, ahead.y).max(model.p.sea_level);
        let eye = glam::Vec3::new(p.x, ground + 70.0, p.y);
        let target = glam::Vec3::new(ahead.x, g2 + 14.0, ahead.y);
        let cam = imaginu::render::Camera {
            eye,
            target,
            fov_y: 55f32.to_radians(),
        };
        render_png(&asset, &cam, w, h, &dir.join(format!("frame_{f:04}.png")))
            .map_err(|e| e.to_string())?;
        if f % 8 == 0 {
            eprint!(".");
        }
    }
    eprintln!(" done");
    let mp4 = out.join("flyover.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-framerate"])
        .arg(fps.to_string())
        .arg("-i")
        .arg(dir.join("frame_%04d.png"))
        .args([
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "18",
            "-movflags",
            "+faststart",
        ])
        .arg(&mp4)
        .status()
        .map_err(|e| format!("ffmpeg not found or failed to start: {e}"))?;
    if !status.success() {
        return Err("ffmpeg failed to encode the flyover".into());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    println!("wrote {}", mp4.display());
    Ok(())
}

/// Build a dungeon from resolved params + palette into `out`. A single-room
/// dungeon writes one merged `dungeon.glb`; anything larger writes per-room
/// GLBs + `manifest.json`. `--overview` adds a ceiling-less top-down render.
/// Factored out of the CLI arm so it is unit-testable.
fn build_dungeon(
    params: &imaginu::recipe::DungeonParams,
    pal_name: &str,
    out: &std::path::Path,
    overview: bool,
) -> Result<(), String> {
    use imaginu::generators::dungeon;
    if !imaginu::palette::PALETTES.contains(&pal_name) {
        return Err(format!(
            "unknown palette '{pal_name}' (available: {})",
            imaginu::palette::PALETTES.join(", ")
        ));
    }
    let pal = imaginu::palette::by_name(pal_name);
    let model = dungeon::model::DungeonModel::new(params, &pal)?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    if model.rooms.len() <= 1 {
        let asset = dungeon::merged_asset(&model);
        let glb = imaginu::gltf::to_glb(&asset);
        let path = out.join("dungeon.glb");
        std::fs::write(&path, &glb).map_err(|e| e.to_string())?;
        let tris: usize = asset.parts.iter().map(|p| p.mesh.triangle_count()).sum();
        println!(
            "wrote {} ({} KiB, {tris} triangles)",
            path.display(),
            glb.len() / 1024
        );
    } else {
        dungeon::manifest::write_dir(&model, out)?;
        println!(
            "wrote {}/manifest.json + {} room GLB(s) ({} corridors, {} doors)",
            out.display(),
            model.rooms.len(),
            model.corridors.len(),
            model.doors.len()
        );
    }
    if overview {
        let asset = dungeon::overview_asset(&model);
        let cam = auto_camera(&asset, 40.0, 68.0, 1.0);
        let path = out.join("overview.png");
        render_png(&asset, &cam, 1400, 1000, &path).map_err(|e| e.to_string())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

const SCHEMA_HELP: &str = r##"imaginu recipe cheat-sheet (JSON, all fields except "kind" optional):

palettes: verdant | autumn | arctic | volcanic | desert | mystic | necrotic | infernal | fungal
  (necrotic/infernal/fungal carry emissive accents for undead/fire/cavern reads)

{"kind":"terrain","palette":"verdant","seed":1,"size":48,"resolution":110,
 "mountainousness":1.0,"water_level":0.28,"scatter":true,"scatter_density":1.0,
 "shape":"hills|mountains|island|archipelago|canyon|mesa|crater|valley|dunes",
 "terrace":0,"skirt":true,"offset_x":0,"offset_z":0,
 "erosion":0.6,"rivers":2,
 "paths":[{"points":[[-20,-18],[0,0],[20,15]],"width":2.5}],
 "texture":{"pattern":"rock","scale":9,"colors":["#8a6a4c","#d8b287"]}}
 // any size (4..4096). For seamless world tiles: skirt=false and offset_x/z
 // = chunk world position; adjacent chunks share identical edge heights.
 // erosion (hydraulic droplets) + rivers here are chunk-local diorama
 // features — for seamless multi-chunk maps with world-scale erosion and
 // rivers use {"kind":"world"} instead. paths = flattened
 // dirt splines. texture drapes a baked material (strata on cliffs).
 // scatter exports as GPU instances (EXT_mesh_gpu_instancing): dense
 // forests at a fraction of the file size.

{"kind":"world","name":"everdale","seed":1,"palette":"verdant",
 "size":2048,"chunk_size":256,"chunk_resolution":128,
 "mountainousness":1.0,"sea_level":0.0,"scatter":true,"scatter_density":1.0,
 "zone_size":900,
 "zones":[{"kind":"forest","weight":2},{"kind":"plains","weight":2},
          {"kind":"mountains","weight":1.2},
          {"kind":"lake","at":[300,-500],"radius":400}],
 "pois":[{"kind":"city","count":2},{"kind":"village","count":5},
         {"kind":"castle","at":[500,-800],"name":"Castle Hightower"},
         {"kind":"watchtower","count":3},{"kind":"dungeon","count":2}],
 "rivers":4,"roads":true,
 "erosion":0.5,"adaptive_resolution":true,"lods":0}
 // whole streaming map: imaginu world <recipe> -o mapdir/ writes
 // manifest.json + one GLB per chunk (chunk-local origin; place each at its
 // manifest "position"). Heights/colors are pure functions of world coords +
 // seed: adjacent chunks share bit-identical edges, and chunks build lazily
 // (--chunk x,z) or in parallel. sea_level is an absolute elevation (m).
 // zones: mountains|forest|plains|desert|swamp|lake|coast|badlands, seeded
 // Voronoi regions (~zone_size m across) with smooth blending — each brings
 // its own height character, ground colors and scatter mix. "at"+"radius"
 // pins a zone at a world position. --map renders <out>/map.png (zones +
 // hillshade + water + POI markers); --map-only skips chunk builds.
 // pois: city|village|castle|watchtower|dungeon — a deterministic solver
 // scores slope/zone/altitude/prominence/water, flattens sites into the
 // world height function (seamless across chunk borders), names them, and
 // exports each as its own GLB with world transform + spawn points in the
 // manifest (omit "pois" for area-scaled defaults, [] for none).
 // rivers: traced downhill on a depression-filled global heightfield from
 // mountain springs to lakes/sea, carved into every chunk they cross, with
 // per-chunk water ribbons. roads: A* between settlements (slope-penalized,
 // river-averse), flattened into the terrain; where a road must cross a
 // river a stone bridge GLB spawns (manifest poi kind "bridge", rotation
 // baked). Polylines land in manifest roads/rivers.
 // erosion: a global coarse heightmap is eroded ONCE and C1-upsampled, so
 // gullies/fans span chunks without breaking edge identity. Ground color:
 // zone splat blend + cliff strata bands + dry-grass patches + dune
 // ripple + shoreline foam + waterfall whitening + scree under cliffs.
 // adaptive_resolution: flat chunks halve, mountains/POIs double (edges
 // stitch crack-free); manifest lists per-chunk resolution. lods: N
 // embeds decimated MSFT_lod levels per chunk.
 // Check output: imaginu validate-world mapdir/

{"kind":"tree","style":"oak|pine|palm|dead","height":6,"seed":1}

{"kind":"rock","size":1.0,"jaggedness":0.6,"seed":1}

{"kind":"crystal","size":1.0,"count":7,"palette":"mystic","seed":1}

{"kind":"building","width":6,"floors":1,"seed":1}

{"kind":"prop","prop":"barrel|crate|lantern|campfire","size":1.0,"seed":1}

{"kind":"character","class":"villager|warrior|mage|rogue","height":1.7,
 "bulk":1.0,"animate":true,"seed":1,"detail":1.0,
 "hair":"short|ponytail|bun|bald|long|topknot","beard":"none|mustache|short|long",
 "hair_color":"#eae7e0","skin_tone":0,"expressions":true,
 "outfit":"robe|tunic|plain","ornamentation":0.6,"age":0.8,
 "accessories":["necklace","belt_knot","staff"],
 "trim_motif":"meander|zigzag|dots|diamonds|scroll|runes"}
 // outfits: lofted painted garment stacks (under-robe, open coat, hanging
 // sleeves, sash + tail, mantle) with hem/cuff trim bands, brocade motifs,
 // painted cloth folds - skinned to the skeleton, deform with every clip
 // detail: 0.5..2.0 tessellation multiplier (2.0 = hero close-ups, ~24k tris)
 // body v5: sculpted boots w/ soles+toe caps, framed buckle, hip pouch,
 // collar, shirt buttons, fingered mittens; class gear: cuirass rivets +
 // rim + bracers (warrior), draped hood + hip dagger (rogue), hat band +
 // smooth robe skirt (mage)
 // smooth subdivision bodies, mitten hands, faces (eyes/brows/nose/mouth),
 // facial morph targets: smile, blink, angry, surprised (glTF blend shapes)

{"kind":"monster","body":"quadruped_beast","class":"none","size":1.0,"seed":1,
 "horns":0,"spikes":0,"plates":0,"tail":-1,"wings":-1,"eyes":-1,"maw":-1,
 "menace":0,"age":0,"emissive":-1,"detail":1.0,"animate":true}
 // body (alias "species"): biped_brute | quadruped_beast | serpent (alias
 //   wyrm) | arachnid | winged_flyer | ooze (alias blob) | insectoid |
 //   aberration - each a skeleton template driving limb count, gait, collider
 // class: none | predator | brute | elemental | undead | aberration | swarm -
 //   a preset bundle over the knobs (explicit fields still win); elemental,
 //   undead, aberration also default the palette to infernal/necrotic/fungal
 // size (alias "bulk"): scales geometry, collider, and mass (mass ~ size^3)
 // knobs 0..1: horns, spikes (dorsal ridge), plates (armor), menace, age,
 //   emissive (glowing accent markings); maw = jaw/teeth prominence
 // tail/wings/maw/emissive: -1 = let the body plan/class decide (0 disables)
 // eyes: -1 = plan default; else 0..12 emissive eyes (glow with the accent)
 // gaits/clips per plan: idle + walk|slither|fly|crawl|pulse + attack, hurt,
 //   death (+ roar when the plan has a head). Family-restricted skinning.
 // collider auto-fits the plan (capsule/box/elongated-capsule/trimesh).

{"kind":"dungeon","type":"crypt","size":"medium","seed":1,
 "rooms":null,"loops":0.3,"density":0.5,"detail":1.0}
 // type: crypt | cavern | sewer | mine | temple | fortress - each sets the
 //   palette, wall material, prop set, and shape bias. cavern is meshed as
 //   ORGANIC SDF caves (blobby chambers + curved tunnels); the other five are
 //   orthogonal rooms. Default palette per theme (crypt->necrotic,
 //   cavern/sewer->fungal, mine/fortress->volcanic, temple->mystic); an
 //   explicit "palette" overrides.
 // size: small | medium | large (target room count / footprint)
 // rooms: optional explicit room cap (overrides size). loops: 0..1 extra
 //   corridor edges beyond the spanning tree. density: 0..1 dressing amount.
 // Layout is a pure function of seed: BSP rooms + MST corridors (+loops),
 //   integer-meter aligned. Dressing: pillars, torch brackets (emissive
 //   lighting cues), doors, portcullis, sarcophagi, chests, rubble.
 // A one-room dungeon builds a single GLB; multi-room writes a directory:
 //   imaginu dungeon <recipe> -o out [--overview]   (per-room GLBs +
 //   manifest.json with rooms/corridors/doors/spawn_points/colliders)
 //   imaginu validate-dungeon <dir>                 (structural round-trip)

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
     {"shape":"loft","path":[[0,0,0],[0,1,0],[0,1.4,0]],"rx":[0.55,0.35,0.3],
      "rz":[0.45,0.3,0.26],"arc":300,"arc_offset":180,"segments":24,
      "color":"#ccbbaa","flat":false},
     {"shape":"box","size":[4,3,2],"color":"#999999","bevel":0.1,
      "subdivide":0,"smooth":false,
      "csg":[{"op":"subtract","shape":"cylinder","radius":1,"height":3,
              "color":"#999999","transform":{"rotate_deg":[-90,0,0],
                                             "translate":[0,0,1.5]}}]},
     {"shape":"box","size":[0.2,1,0.2],"color":"#ffffff","bone":"arm",
      "transform":{"translate":[0,1,0],"rotate_deg":[0,45,0],"scale":[1,1,1]},
      "repeat":{"count":8,"radius":2.0,"orient":true}}]}]}
 // every node: optional transform/displace/flat/repeat/bone/color_top/uv
 // material.texture.paint: UV-space layers composited over the base -
 //   {"op":"band","v":0.0,"height":0.08,"color":"#7a1f1f","motif":"meander",
 //    "motif_color":"#e8b54a","motif_scale":1}          hem/cuff border
 //   {"op":"motif_grid","motif":"diamonds|dots|scroll|runes|zigzag",
 //    "color":"#b03a2e","scale":2,"v_min":0.2,"v_max":0.7}
 //   {"op":"stripes","count":6,"width":0.5,"color":"#334455","axis":"u|v"}
 //   {"op":"gradient","from":"#ffffff","to":"#888888"}
 //   {"op":"folds","strength":1,"count":10}   painted cloth drape + relief
 //   {"op":"weathering","strength":0.4}       hem grime
 // pattern "none" + "base":"#hex" = flat cloth ground for painting.
 // On loft shapes, v runs hem(0) -> collar(1): bands land exactly on hems.
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

#[cfg(test)]
mod tests {
    use super::*;
    use imaginu::recipe::DungeonParams;

    fn dungeon_params(json: &str) -> DungeonParams {
        match Recipe::parse(json).unwrap() {
            Recipe::Dungeon { params, .. } => params,
            _ => panic!("expected a dungeon recipe"),
        }
    }

    #[test]
    fn build_dungeon_writes_a_validatable_directory() {
        let params = dungeon_params(r#"{"kind":"dungeon","type":"crypt","size":"medium"}"#);
        let dir = std::env::temp_dir().join(format!("imaginu_maintest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        build_dungeon(&params, "necrotic", &dir, false).unwrap();
        let manifest = dir.join("manifest.json");
        assert!(manifest.exists(), "manifest.json must exist");
        let summary =
            imaginu::generators::dungeon::manifest::validate_dir(&dir).expect("dir validates");
        assert!(summary.contains("rooms"), "summary: {summary}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_dungeon_single_room_writes_one_glb() {
        let params = dungeon_params(r#"{"kind":"dungeon","type":"crypt","rooms":1}"#);
        let dir = std::env::temp_dir().join(format!("imaginu_maintest1_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        build_dungeon(&params, "necrotic", &dir, false).unwrap();
        assert!(
            dir.join("dungeon.glb").exists(),
            "single-room GLB must exist"
        );
        assert!(!dir.join("manifest.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
