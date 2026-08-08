mod morph;

use image::RgbaImage;

use self::morph::{connected_components, Bitmap, Kernel};

const PAD: usize = 150;
const MARGIN: usize = 10;
const PRUNE_VALVE: f64 = 0.05;
const SCALE: usize = 4;

#[derive(Clone, Copy)]
pub struct Crop {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

pub fn crop_to_content(img: &RgbaImage, cell: u32) -> Option<Crop> {
    let reduced = Bitmap::from_alpha_downsampled(img, SCALE);
    if !reduced.any() {
        return None;
    }
    let alpha = reduced.pad(PAD / SCALE);

    let hex = Kernel::hexagon(1, (cell / SCALE as u32).max(3));
    let snow = Kernel::snowflake(
        (229 * cell / 32 / SCALE as u32).max(3),
        (10 * cell / 32).div_ceil(SCALE as u32).max(1),
    );

    let mut a = alpha.erode(&hex);
    for _ in 0..3 {
        a = a.dilate(&hex);
    }
    a = a.erode(&hex);
    let connected = a.clone();
    a = a.erode(&hex);
    a = a.erode(&snow).dilate(&snow);

    let eroded = a.any();
    if !eroded {
        a = alpha.clone();
    }

    let labels = connected_components(&connected.dilate(&hex));
    if labels.areas.len() > 2 {
        let biggest = *labels.areas[1..].iter().max().unwrap() as f64;
        let denom = 150.0 * 150.0 / (SCALE * SCALE) as f64 * 200.0 * (cell as f64 / 32.0).powi(2);
        let min_size = (biggest / 8.0).max((biggest / 2.0).min(biggest / denom * biggest));
        let w = a.width();
        for y in 0..a.height() {
            for x in 0..w {
                let label = labels.labels[y * w + x];
                if label != 0 && (labels.areas[label as usize] as f64) < min_size {
                    a.set(x, y, false);
                }
            }
        }
    }

    if eroded {
        a = a.dilate(&hex).dilate(&hex);
        a = a.and(&alpha);
    }

    let a = a.unpad(PAD / SCALE);
    let (x0, y0, x1, y1) = match a.bbox() {
        Some((x0, y0, x1, y1)) => (
            x0 * SCALE,
            y0 * SCALE,
            (x1 * SCALE + SCALE - 1).min(img.width() as usize - 1),
            (y1 * SCALE + SCALE - 1).min(img.height() as usize - 1),
        ),
        None => Bitmap::from_alpha(img).bbox()?,
    };

    let mut cx = x0.saturating_sub(MARGIN);
    let mut cy = y0.saturating_sub(MARGIN);
    let mut cx2 = (x1 + MARGIN).min(img.width() as usize);
    let mut cy2 = (y1 + MARGIN).min(img.height() as usize);

    let inside = |x: usize, y: usize| {
        x * SCALE < cx2 && (x + 1) * SCALE > cx && y * SCALE < cy2 && (y + 1) * SCALE > cy
    };
    let (mut total, mut outside) = (0u64, 0u64);
    for y in 0..reduced.height() {
        for x in 0..reduced.width() {
            if reduced.get(x, y) {
                total += 1;
                if !inside(x, y) {
                    outside += 1;
                }
            }
        }
    }
    if outside as f64 > total as f64 * PRUNE_VALVE {
        if let Some((px0, py0, px1, py1)) = connected.unpad(PAD / SCALE).bbox() {
            cx = cx.min((px0 * SCALE).saturating_sub(MARGIN));
            cy = cy.min((py0 * SCALE).saturating_sub(MARGIN));
            cx2 = cx2.max((px1 * SCALE + SCALE - 1 + MARGIN).min(img.width() as usize));
            cy2 = cy2.max((py1 * SCALE + SCALE - 1 + MARGIN).min(img.height() as usize));
        }
    }
    Some(Crop {
        x: cx,
        y: cy,
        w: cx2 - cx,
        h: cy2 - cy,
    })
}

pub fn crop_copy(img: &RgbaImage, c: Crop) -> RgbaImage {
    let src = img.as_raw();
    let src_w = img.width() as usize;
    let mut out = Vec::with_capacity(c.w * c.h * 4);
    for row in 0..c.h {
        let base = ((c.y + row) * src_w + c.x) * 4;
        out.extend_from_slice(&src[base..base + c.w * 4]);
    }
    RgbaImage::from_raw(c.w as u32, c.h as u32, out).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn opaque_rect(img: &mut RgbaImage, x0: u32, y0: u32, w: u32, h: u32) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                img.put_pixel(x, y, Rgba([200, 100, 50, 255]));
            }
        }
    }

    #[test]
    fn fully_transparent_returns_none() {
        assert!(crop_to_content(&RgbaImage::new(64, 64), 32).is_none());
    }

    #[test]
    fn speck_is_excluded_and_blob_pixels_unchanged() {
        let mut img = RgbaImage::new(1200, 600);
        opaque_rect(&mut img, 60, 60, 400, 400);
        opaque_rect(&mut img, 1100, 300, 3, 3);

        let c = crop_to_content(&img, 32).expect("has content");
        assert!(c.w >= 400 && c.h >= 400);
        assert!(
            c.w < 800,
            "speck at x=1100 must not stretch the crop (got {}x{})",
            c.w,
            c.h
        );
        let cropped = crop_copy(&img, c);
        for y in 0..cropped.height() {
            for x in 0..cropped.width() {
                assert_eq!(
                    cropped.get_pixel(x, y),
                    img.get_pixel(c.x as u32 + x, c.y as u32 + y)
                );
            }
        }
    }

    #[test]
    fn scatter_platform_map_keeps_all_islands() {
        let mut img = RgbaImage::new(1800, 1000);
        opaque_rect(&mut img, 60, 60, 400, 400);
        for x in [800, 1100, 1400] {
            for y in [100, 450, 800] {
                opaque_rect(&mut img, x, y, 100, 100);
            }
        }
        let c = crop_to_content(&img, 32).expect("has content");
        assert!(c.x <= 60 && c.y <= 60, "big island kept ({},{})", c.x, c.y);
        assert!(
            c.x + c.w >= 1500 && c.y + c.h >= 900,
            "far islands must survive the valve: {}x{}+{}+{}",
            c.w,
            c.h,
            c.x,
            c.y
        );
    }

    #[test]
    fn hexagon_1_23_matches_python_kernel_exactly() {
        let expected_cols = |row: usize| -> std::ops::Range<usize> {
            match row {
                0..=5 => 10 - 2 * row..13 + 2 * row,
                6..=17 => 0..23,
                18..=22 => 10 - 2 * (22 - row)..13 + 2 * (22 - row),
                _ => unreachable!(),
            }
        };
        let mut b = Bitmap::new(23, 23);
        b.set(11, 11, true);
        let d = b.dilate(&Kernel::hexagon(1, 23));
        for y in 0..23 {
            for x in 0..23 {
                assert_eq!(
                    d.get(x, y),
                    expected_cols(y).contains(&x),
                    "kernel mismatch at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn thin_map_falls_back_to_original_bounds() {
        let mut img = RgbaImage::new(600, 120);
        opaque_rect(&mut img, 50, 50, 500, 20);
        let c = crop_to_content(&img, 32).expect("has content");
        assert!(c.w >= 500, "strip must survive: {}x{}", c.w, c.h);
    }

    #[test]
    fn margin_is_applied_and_clamped() {
        let mut img = RgbaImage::new(400, 400);
        opaque_rect(&mut img, 100, 100, 150, 150);
        let c = crop_to_content(&img, 32).expect("has content");
        let max = 150 + 19 + 2 * (SCALE - 1);
        assert!(c.w >= 150 && c.w <= max);
        assert!(c.h >= 150 && c.h <= max);
    }
}
