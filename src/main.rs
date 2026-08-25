use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use mciso::{artist, cleanup, compress, maps, render, render_view, world};
use rayon::prelude::*;

#[derive(Parser)]
#[command(
    version,
    about = "Render Minecraft worlds from MAPS_DIR as four cropped isometric views and thumbnails",
    after_help = "Environment:\n  MCISO_TIMING       Print per-stage timings\n  MCISO_CROP_AUDIT   Check crops against content instead of writing outputs\n  MCISO_AUDIT_DUMP   Directory for crop-audit overlay thumbnails"
)]
struct Args {
    #[arg(
        short,
        long,
        value_name = "MAPS_DIR",
        help = "Directory scanned recursively for map folders (map.xml roots)"
    )]
    input: PathBuf,

    #[arg(
        short,
        long,
        value_name = "OUT_DIR",
        required_unless_present = "list",
        help = "Output directory, created if missing"
    )]
    output: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "COMMIT",
        help = "Only render maps changed since this git commit in MAPS_DIR"
    )]
    since: Option<String>,

    #[arg(long, help = "List discovered maps (name<TAB>world-dir) and exit")]
    list: bool,

    #[arg(
        short,
        long,
        help = "Re-render maps whose outputs already exist (never removes stale files)"
    )]
    force: bool,

    #[arg(
        short,
        long,
        value_name = "N",
        help = "Worker threads [default: all cores]"
    )]
    jobs: Option<usize>,

    #[arg(
        short,
        long,
        help = "Suppress per-map progress; print only warnings and the summary"
    )]
    quiet: bool,

    #[arg(
        long,
        value_name = "WxH",
        default_value = "1920x1080",
        value_parser = parse_size,
        help = "Cap output image size; also drives the adaptive render tile size"
    )]
    max_size: (u32, u32),

    #[arg(
        long,
        value_name = "N",
        default_value_t = 256,
        value_parser = clap::value_parser!(u16).range(2..=256),
        help = "Palette size for indexed PNG output (thumbnails cap at 64)"
    )]
    colors: u16,

    #[arg(
        long,
        help = "Write full-color PNGs instead of indexed (skip quantization)"
    )]
    rgba: bool,

    #[arg(
        long,
        help = "Average texels when scaling textures down (default keeps the pixelated look)"
    )]
    smooth: bool,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let dim = |v: &str| v.parse::<u32>().ok().filter(|&v| v > 0);
    s.split_once(['x', 'X'])
        .and_then(|(w, h)| Some((dim(w)?, dim(h)?)))
        .ok_or_else(|| format!("expected WxH, e.g. 1920x1080, got {s}"))
}

const ROTATIONS: [(&str, render::Rotation); 4] = [
    ("tr", render::Rotation::TopLeft),
    ("br", render::Rotation::TopRight),
    ("bl", render::Rotation::BottomRight),
    ("tl", render::Rotation::BottomLeft),
];

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.input.is_dir(),
        "input {} is not a directory",
        args.input.display()
    );

    let maps = maps::find_maps(&args.input, args.since.as_deref())?;
    if args.list {
        for map in &maps {
            println!("{}\t{}", map.name, map.world_dir.display());
        }
        return Ok(());
    }

    let out_dir = args.output.as_deref().expect("clap enforces --output");
    fs::create_dir_all(out_dir)?;
    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .context("configuring worker threads")?;
    }
    println!("Found {} {} to render", maps.len(), map_word(maps.len()));

    let artist = artist::TextureArtist::default_packs().with_smooth(args.smooth);
    let enc = compress::Encoding {
        max_w: args.max_size.0,
        max_h: args.max_size.1,
        colors: (!args.rgba).then_some(args.colors),
    };
    let results: Vec<(&maps::MapFolder, Result<bool>)> = maps
        .par_iter()
        .map(|map| {
            (
                map,
                process_map(map, out_dir, &artist, &enc, args.force, args.quiet),
            )
        })
        .collect();

    let skipped = results
        .iter()
        .filter(|(_, r)| matches!(r, Ok(false)))
        .count();
    let failures: Vec<String> = results
        .iter()
        .filter_map(|(map, r)| r.as_ref().err().map(|e| format!("{}: {e:#}", map.name)))
        .collect();

    for failure in &failures {
        eprintln!("ERROR {failure}");
    }
    println!(
        "Rendered {} {}, {} up to date, {} failed",
        maps.len() - skipped - failures.len(),
        map_word(maps.len() - skipped - failures.len()),
        skipped,
        failures.len(),
    );
    if !failures.is_empty() {
        anyhow::bail!("{} of {} maps failed", failures.len(), maps.len());
    }
    Ok(())
}

fn map_word(count: usize) -> &'static str {
    if count == 1 {
        "map"
    } else {
        "maps"
    }
}

fn audit_crop(name: &str, side: &str, ht: i32, image: &image::RgbaImage, crop: cleanup::Crop) {
    let (mut total, mut outside) = (0u64, 0u64);
    let (mut bx0, mut by0, mut bx1, mut by1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (x, y, p) in image.enumerate_pixels() {
        if p.0[3] == 0 {
            continue;
        }
        total += 1;
        let (x, y) = (x as usize, y as usize);
        if x < crop.x || x >= crop.x + crop.w || y < crop.y || y >= crop.y + crop.h {
            outside += 1;
            bx0 = bx0.min(x);
            by0 = by0.min(y);
            bx1 = bx1.max(x);
            by1 = by1.max(y);
        }
    }
    if let Some(dir) = std::env::var_os("MCISO_AUDIT_DUMP") {
        let f = ((image.width().max(image.height())).div_ceil(1600)).max(1);
        let (w, h) = ((image.width() / f).max(1), (image.height() / f).max(1));
        let mut small = image::imageops::thumbnail(image, w, h);
        let red = image::Rgba([255u8, 0, 0, 255]);
        let (rx0, ry0) = (crop.x as u32 / f, crop.y as u32 / f);
        let rx1 = ((crop.x + crop.w) as u32 / f).min(w - 1);
        let ry1 = ((crop.y + crop.h) as u32 / f).min(h - 1);
        for x in rx0..=rx1 {
            small.put_pixel(x, ry0, red);
            small.put_pixel(x, ry1, red);
        }
        for y in ry0..=ry1 {
            small.put_pixel(rx0, y, red);
            small.put_pixel(rx1, y, red);
        }
        small
            .save(std::path::Path::new(&dir).join(format!("{name}_{side}.png")))
            .ok();
    }
    if outside > 0 {
        println!(
            "AUDIT {name}_{side} ht={ht} img={}x{} crop=({},{} {}x{}) dropped {outside}/{total} px ({:.3}%) dropped-bbox=({bx0},{by0})..({bx1},{by1})",
            image.width(), image.height(), crop.x, crop.y, crop.w, crop.h,
            outside as f64 / total as f64 * 100.0,
        );
    }
}

fn process_map(
    map: &maps::MapFolder,
    out_dir: &Path,
    artist: &artist::TextureArtist,
    enc: &compress::Encoding,
    force: bool,
    quiet: bool,
) -> Result<bool> {
    let todo: Vec<_> = ROTATIONS
        .iter()
        .filter(|(side, _)| force || !out_dir.join(format!("{}_{}.png", map.name, side)).exists())
        .collect();
    if todo.is_empty() {
        return Ok(false);
    }

    let timing = std::env::var_os("MCISO_TIMING").is_some();
    let stage = |label: &str, t: std::time::Instant| {
        if timing {
            println!(
                "  [timing] {} {} {:.2}s",
                map.name,
                label,
                t.elapsed().as_secs_f64()
            );
        }
    };

    let start = std::time::Instant::now();
    if !quiet {
        println!("Rendering {} ({})", map.name, map.world_dir.display());
    }
    let t = std::time::Instant::now();
    let world = world::load_world(&map.world_dir)
        .with_context(|| format!("loading world {}", map.world_dir.display()))?;
    stage("load", t);
    let t = std::time::Instant::now();
    let surface = world.extract_surface(&|name| artist.occludes(name));
    stage("surface", t);
    if surface.blocks.is_empty() {
        anyhow::bail!("world has no visible blocks");
    }

    todo.par_iter()
        .try_for_each(|(side, rotation)| -> Result<()> {
            let t = std::time::Instant::now();
            let max = (enc.max_w, enc.max_h);
            let (image, crop, ht) = render_view(&surface, *rotation, artist, max)
                .with_context(|| format!("{}_{}", map.name, side))?;
            stage(&format!("render_{side}"), t);
            if std::env::var_os("MCISO_CROP_AUDIT").is_some() {
                audit_crop(&map.name, side, ht, &image, crop);
                return Ok(());
            }
            let t = std::time::Instant::now();
            let out_file = out_dir.join(format!("{}_{}.png", map.name, side));
            compress::write_outputs(&image, crop, &out_file, enc)
                .with_context(|| format!("writing {}", out_file.display()))?;
            stage(&format!("compress_{side}"), t);
            if !quiet {
                println!("Finished {}_{}", map.name, side);
            }
            Ok(())
        })?;

    let secs = start.elapsed().as_secs();
    if !quiet {
        println!(
            "Completed {} in {:02}:{:02}:{:02}",
            map.name,
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        );
    }
    Ok(true)
}
