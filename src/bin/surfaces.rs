use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context, Result};
use mciso::{artist, maps, surface, world};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(
        args.next()
            .context("usage: surfaces <maps-dir> [out-dir]")?,
    );
    let out_dir = PathBuf::from(args.next().unwrap_or_else(|| "demo/maps".to_owned()));
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let artist = artist::TextureArtist::default_packs();
    let maps = maps::find_maps(&input, None)?;
    let total = maps.len();
    let mut index = Vec::new();
    for (i, map) in maps.iter().enumerate() {
        match build(map, &out_dir, &artist) {
            Ok(blocks) => {
                println!("[{}/{total}] {} ({blocks} blocks)", i + 1, map.name);
                index.push(serde_json::json!({ "name": map.name, "blocks": blocks }));
            }
            Err(e) => eprintln!("WARN skipping {}: {e:#}", map.name),
        }
    }
    index.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    std::fs::write(out_dir.join("index.json"), serde_json::to_vec(&index)?)
        .context("writing index.json")?;
    println!("wrote {} surfaces to {}", index.len(), out_dir.display());
    Ok(())
}

fn build(map: &maps::MapFolder, out_dir: &Path, artist: &artist::TextureArtist) -> Result<usize> {
    let world = world::load_world(&map.world_dir)
        .with_context(|| format!("loading world {}", map.world_dir.display()))?;
    let s = world.extract_surface(&|name| artist.occludes(name));
    ensure!(!s.blocks.is_empty(), "world has no visible blocks");
    let path = out_dir.join(format!("{}.surf", map.name));
    std::fs::write(&path, surface::encode(&s))
        .with_context(|| format!("writing {}", path.display()))?;
    let status = Command::new("gzip")
        .args(["-9", "-f"])
        .arg(&path)
        .status()
        .context("running gzip")?;
    ensure!(status.success(), "gzip failed");
    Ok(s.blocks.len())
}
