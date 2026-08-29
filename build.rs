use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=textures_modern");
    println!("cargo::rerun-if-changed=mcassets");
    if env::var_os("CARGO_FEATURE_EMBED_ASSETS").is_none() {
        return;
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let textures = root.join("textures_modern/assets/minecraft/textures");
    let mc = root.join("mcassets/assets/minecraft");
    let mut entries = Vec::new();
    collect(&textures.join("block"), "block", ".png", &mut entries);
    collect(&textures.join("entity"), "entity", ".png", &mut entries);
    collect(
        &mc.join("blockstates"),
        "mc/blockstates",
        ".json",
        &mut entries,
    );
    collect(
        &mc.join("models/block"),
        "mc/models/block",
        ".json",
        &mut entries,
    );
    let mut bundle = b"MCB1".to_vec();
    for (path, data) in &entries {
        bundle.extend((path.len() as u32).to_le_bytes());
        bundle.extend(path.as_bytes());
        bundle.extend((data.len() as u32).to_le_bytes());
        bundle.extend(data);
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("assets.bundle");
    fs::write(&out, bundle).expect("writing embedded asset bundle");
}

fn collect(dir: &Path, prefix: &str, ext: &str, entries: &mut Vec<(String, Vec<u8>)>) {
    let listing = fs::read_dir(dir).unwrap_or_else(|e| {
        panic!(
            "embed-assets: reading {}: {e}; run `git submodule update --init`",
            dir.display()
        )
    });
    for entry in listing {
        let path = entry.unwrap().path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if path.is_dir() {
            collect(&path, &format!("{prefix}/{name}"), ext, entries);
        } else if name.ends_with(ext) {
            let data = fs::read(&path).unwrap();
            entries.push((format!("{prefix}/{name}"), data));
        }
    }
}
