use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "demo/assets.bin".to_owned());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let textures = root.join("textures_modern/assets/minecraft/textures");
    let mc = root.join("mcassets/assets/minecraft");
    let mut entries = Vec::new();
    collect(&textures.join("block"), "block", ".png", &mut entries)?;
    collect(&textures.join("entity"), "entity", ".png", &mut entries)?;
    collect(
        &mc.join("blockstates"),
        "mc/blockstates",
        ".json",
        &mut entries,
    )?;
    collect(
        &mc.join("models/block"),
        "mc/models/block",
        ".json",
        &mut entries,
    )?;
    let bundle = mciso::assets::write_bundle(&entries);
    std::fs::write(&out, &bundle).with_context(|| format!("writing {out}"))?;
    println!(
        "wrote {} files ({:.1} MB) to {out}",
        entries.len(),
        bundle.len() as f64 / 1e6
    );
    Ok(())
}

fn collect(
    dir: &Path,
    prefix: &str,
    ext: &str,
    entries: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if path.is_dir() {
            collect(&path, &format!("{prefix}/{name}"), ext, entries)?;
        } else if name.ends_with(ext) {
            let mut data =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            if ext == ".json" {
                let v: serde_json::Value = serde_json::from_slice(&data)
                    .with_context(|| format!("parsing {}", path.display()))?;
                data = serde_json::to_vec(&v)?;
            }
            entries.push((format!("{prefix}/{name}"), data));
        }
    }
    Ok(())
}
