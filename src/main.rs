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
    },
    /// Render turntable PNGs (4 angles) of a recipe, without keeping the GLB.
    Render {
        recipe: String,
        /// Output directory for PNGs
        #[arg(short, long, default_value = ".")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 900)]
        width: usize,
        #[arg(long, default_value_t = 640)]
        height: usize,
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
        Cmd::Generate { recipe, out, preview } => {
            let r = load_recipe(&recipe)?;
            let asset = r.build()?;
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
        Cmd::Render { recipe, out_dir, width, height } => {
            let r = load_recipe(&recipe)?;
            let asset = r.build()?;
            std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
            for (i, yaw) in [25.0f32, 115.0, 205.0, 295.0].iter().enumerate() {
                let cam = auto_camera(&asset, *yaw, 20.0, 1.0);
                let path = out_dir.join(format!("{}_{}.png", asset.name, i));
                render_png(&asset, &cam, width, height, &path).map_err(|e| e.to_string())?;
                println!("wrote {}", path.display());
            }
            Ok(())
        }
        Cmd::Schema => {
            println!("{}", SCHEMA_HELP);
            Ok(())
        }
    }
}

const SCHEMA_HELP: &str = r#"imaginu recipe cheat-sheet (JSON, all fields except "kind" optional):

palettes: verdant | autumn | arctic | volcanic | desert | mystic

{"kind":"terrain","palette":"verdant","seed":1,"size":48,"resolution":110,
 "mountainousness":1.0,"water_level":0.28,"scatter":true}

{"kind":"tree","style":"oak|pine|palm|dead","height":6,"seed":1}

{"kind":"rock","size":1.0,"jaggedness":0.6,"seed":1}

{"kind":"crystal","size":1.0,"count":7,"palette":"mystic","seed":1}

{"kind":"building","width":6,"floors":1,"seed":1}

{"kind":"prop","prop":"barrel|crate|lantern|campfire","size":1.0,"seed":1}

{"kind":"character","class":"villager|warrior|mage|rogue","height":1.7,
 "bulk":1.0,"animate":true,"seed":1}

Output GLB embeds physics metadata at nodes[0].extras.imaginu_physics:
{collider:{type:box|sphere|capsule|trimesh|heightfield,...},mass,friction,restitution}
Characters include a 17-joint skeleton with "idle" and "walk" clips."#;
