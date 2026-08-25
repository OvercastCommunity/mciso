use std::path::Path;

use anyhow::{Context, Result};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};

use crate::cleanup::{crop_copy, Crop};
use crate::render::{MAX_HEIGHT, MAX_WIDTH};

const MAX_THUMB_WIDTH: u32 = 350;
const MAX_THUMB_HEIGHT: u32 = 250;
const THUMB_COLORS: u16 = 64;

pub struct Encoding {
    pub max_w: u32,
    pub max_h: u32,
    pub colors: Option<u16>,
}

impl Default for Encoding {
    fn default() -> Self {
        Self {
            max_w: MAX_WIDTH,
            max_h: MAX_HEIGHT,
            colors: Some(256),
        }
    }
}

pub fn write_outputs(img: &RgbaImage, crop: Crop, out_file: &Path, enc: &Encoding) -> Result<()> {
    let (main, thumb) = encode_outputs(img, crop, enc)?;
    std::fs::write(out_file, main).with_context(|| format!("creating {}", out_file.display()))?;
    let stem = out_file
        .file_stem()
        .and_then(|s| s.to_str())
        .context("output file has no name")?;
    let thumb_file = out_file.with_file_name(format!("{stem}-thumb.png"));
    std::fs::write(&thumb_file, thumb).with_context(|| format!("creating {}", thumb_file.display()))
}

pub fn encode_outputs(img: &RgbaImage, crop: Crop, enc: &Encoding) -> Result<(Vec<u8>, Vec<u8>)> {
    let main = match resize_to_fit(img, crop, enc.max_w, enc.max_h) {
        Some(m) => m,
        None => crop_copy(img, crop),
    };
    let main_png = encode_png(&main, enc.colors)?;
    let full = Crop {
        x: 0,
        y: 0,
        w: main.width() as usize,
        h: main.height() as usize,
    };
    let thumb = resize_to_fit(&main, full, MAX_THUMB_WIDTH, MAX_THUMB_HEIGHT);
    let thumb_png = {
        let actual_thumb = encode_png(
            thumb.as_ref().unwrap_or(&main),
            enc.colors.map(|c| c.min(THUMB_COLORS)),
        )?;
        // Prefer full-res image if the thumbnail ends up larger than the main image
        if actual_thumb.len() > main_png.len() {
            main_png.clone()
        } else {
            actual_thumb
        }
    };
    Ok((main_png, thumb_png))
}

fn encode_png(out: &RgbaImage, colors: Option<u16>) -> Result<Vec<u8>> {
    let Some(colors) = colors else {
        return encode_rgba_png(out);
    };
    encode_indexed_png(out, colors).or_else(|e| {
        eprintln!("WARN quantization failed; writing RGBA instead: {e:#}");
        encode_rgba_png(out)
    })
}

fn encode_indexed_png(out: &RgbaImage, colors: u16) -> Result<Vec<u8>> {
    use rgb::FromSlice;
    let (w, h) = (out.width() as usize, out.height() as usize);
    let mut liq = imagequant::new();
    liq.set_speed(10)?;
    liq.set_max_colors(colors as u32)?;
    let mut img = liq.new_image_borrowed(out.as_raw().as_rgba(), w, h, 0.0)?;
    let mut res = liq.quantize(&mut img)?;
    res.set_dithering_level(0.0)?;
    let (palette, indexed) = res.remapped(&mut img)?;

    let mut bytes = Vec::new();
    let mut enc = png::Encoder::new(&mut bytes, w as u32, h as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(
        palette
            .iter()
            .flat_map(|c| [c.r, c.g, c.b])
            .collect::<Vec<u8>>(),
    );
    enc.set_trns(palette.iter().map(|c| c.a).collect::<Vec<u8>>());
    let mut writer = enc.write_header().context("writing png header")?;
    writer
        .write_image_data(&indexed)
        .context("writing png data")?;
    writer.finish().context("finishing png")?;
    Ok(bytes)
}

fn encode_rgba_png(out: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Default, PngFilter::Adaptive);
    encoder
        .write_image(
            out.as_raw(),
            out.width(),
            out.height(),
            ExtendedColorType::Rgba8,
        )
        .context("encoding png")?;
    Ok(bytes)
}

fn resize_to_fit(img: &RgbaImage, c: Crop, max_w: u32, max_h: u32) -> Option<RgbaImage> {
    let ratio = (c.w as f64 / max_w as f64).max(c.h as f64 / max_h as f64);
    if ratio <= 1.0 {
        return None;
    }
    let w = (c.w as f64 / ratio).round() as u32;
    let h = (c.h as f64 / ratio).round() as u32;
    let factor = ratio.floor() as usize;
    let src = if factor >= 2 && c.w >= factor && c.h >= factor {
        box_reduce(img, c, factor)
    } else {
        crop_copy(img, c)
    };
    Some(image::imageops::resize(&src, w, h, FilterType::Triangle))
}

fn box_reduce(img: &RgbaImage, c: Crop, f: usize) -> RgbaImage {
    use rayon::prelude::*;
    let (ow, oh) = (c.w / f, c.h / f);
    let raw = img.as_raw();
    let src_w = img.width() as usize;
    let span = ow * f * 4;
    let n = (f * f) as u32;
    let mut out = vec![0u8; ow * oh * 4];
    out.par_chunks_mut(ow * 4).enumerate().for_each_init(
        || vec![0u32; span],
        |acc, (oy, orow)| {
            acc.fill(0);
            for y in c.y + oy * f..c.y + (oy + 1) * f {
                let row = &raw[(y * src_w + c.x) * 4..][..span];
                for (a, &v) in acc.iter_mut().zip(row) {
                    *a += v as u32;
                }
            }
            for (px, blk) in orow.chunks_exact_mut(4).zip(acc.chunks_exact(f * 4)) {
                let mut sum = [0u32; 4];
                for col in blk.chunks_exact(4) {
                    for (s, &v) in sum.iter_mut().zip(col) {
                        *s += v;
                    }
                }
                for (o, s) in px.iter_mut().zip(sum) {
                    *o = ((s + n / 2) / n) as u8;
                }
            }
        },
    );
    RgbaImage::from_raw(ow as u32, oh as u32, out).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full(img: &RgbaImage) -> Crop {
        Crop {
            x: 0,
            y: 0,
            w: img.width() as usize,
            h: img.height() as usize,
        }
    }

    fn dims(img: &RgbaImage, max_w: u32, max_h: u32) -> (u32, u32) {
        match resize_to_fit(img, full(img), max_w, max_h) {
            Some(r) => (r.width(), r.height()),
            None => (img.width(), img.height()),
        }
    }

    #[test]
    fn wide_image_scales_to_fit_width() {
        let img = RgbaImage::new(8000, 2000);
        assert_eq!(dims(&img, MAX_WIDTH, MAX_HEIGHT), (1920, 480));
    }

    #[test]
    fn tall_image_scales_to_fit_height() {
        let img = RgbaImage::new(2000, 8640);
        assert_eq!(dims(&img, MAX_WIDTH, MAX_HEIGHT), (250, 1080));
    }

    #[test]
    fn small_image_passes_through_untouched() {
        let img = RgbaImage::new(800, 600);
        assert!(resize_to_fit(&img, full(&img), MAX_WIDTH, MAX_HEIGHT).is_none());
    }

    #[test]
    fn box_reduce_averages_blocks_and_truncates_ragged_edges() {
        let mut img = RgbaImage::new(5, 3);
        img.put_pixel(0, 0, image::Rgba([100, 0, 0, 255]));
        img.put_pixel(1, 1, image::Rgba([100, 0, 0, 255]));
        let r = box_reduce(&img, full(&img), 2);
        assert_eq!((r.width(), r.height()), (2, 1));
        assert_eq!(r.get_pixel(0, 0).0, [50, 0, 0, 128]);
        assert_eq!(r.get_pixel(1, 0).0, [0, 0, 0, 0]);
    }

    #[test]
    fn box_reduce_of_offset_crop_matches_materialized_crop() {
        let mut img = RgbaImage::new(31, 23);
        for y in 0..23 {
            for x in 0..31 {
                let v = ((x * 37 + y * 101) % 256) as u8;
                img.put_pixel(x, y, image::Rgba([v, v.wrapping_mul(3), 255 - v, 255]));
            }
        }
        let c = Crop {
            x: 5,
            y: 3,
            w: 21,
            h: 17,
        };
        let direct = box_reduce(&img, c, 3);
        let copied = crop_copy(&img, c);
        let via_copy = box_reduce(&copied, full(&copied), 3);
        assert_eq!(direct.as_raw(), via_copy.as_raw());
    }

    #[test]
    fn writes_both_files_with_expected_sizes() {
        let dir = std::env::temp_dir().join(format!("mciso-compress-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("demo_tl.png");

        let mut img = RgbaImage::new(8000, 2000);
        img.put_pixel(10, 10, image::Rgba([1, 2, 3, 255]));
        write_outputs(&img, full(&img), &out, &Encoding::default()).unwrap();

        let main = image::open(&out).unwrap();
        assert_eq!((main.width(), main.height()), (1920, 480));
        let thumb = image::open(dir.join("demo_tl-thumb.png")).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (350, 88));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
