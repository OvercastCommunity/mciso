use std::sync::Arc;

use image::RgbaImage;
use rayon::prelude::*;

use crate::artist::TextureArtist;
pub use crate::sprite::Rotation;
use crate::sprite::{blend_over, quarter_turns, Sprite};
use crate::world::Surface;

pub const HALF_TILE: i32 = 16;

pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1080;

const TIERS: [i32; 4] = [2 * HALF_TILE, HALF_TILE, HALF_TILE / 2, HALF_TILE / 4];

fn pick_tier(fits: impl Fn(i32) -> bool) -> i32 {
    TIERS
        .into_iter()
        .find(|&ht| fits(ht))
        .unwrap_or(HALF_TILE / 8)
}

pub fn refit_tier(crop_w: usize, crop_h: usize, ht: i32, (max_w, max_h): (u32, u32)) -> i32 {
    pick_tier(|t| {
        crop_w * t as usize <= max_w as usize * ht as usize
            && crop_h * t as usize <= max_h as usize * ht as usize
    })
}

fn rotate(x: i32, z: i32, turns: u32) -> (i32, i32) {
    match turns % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

fn project(rx: i32, ry: i32, rz: i32) -> (i32, i32) {
    (
        (rx - rz) * HALF_TILE,
        (rx + rz) * (HALF_TILE / 2) - ry * HALF_TILE,
    )
}

fn paint_key(rx: i32, rz: i32, ry: i32) -> (i32, i32, i32) {
    (rx + rz + ry, rx, rz)
}

struct Item {
    rx: i32,
    rz: i32,
    ry: i32,
    state: u32,
}

pub struct Scene {
    items: Vec<Item>,
    bounds: (i32, i32, i32, i32),
    palette: Vec<String>,
    rotation: Rotation,
}

impl Scene {
    pub fn new(surface: &Surface, rotation: Rotation) -> Scene {
        let turns = quarter_turns(rotation);
        let mut items: Vec<Item> = surface
            .blocks
            .par_iter()
            .map(|b| {
                let (rx, rz) = rotate(b.x, b.z, turns);
                Item {
                    rx,
                    rz,
                    ry: b.y,
                    state: b.state,
                }
            })
            .collect();
        items.par_sort_unstable_by_key(|i| paint_key(i.rx, i.rz, i.ry));
        let bounds = items
            .par_iter()
            .map(|i| {
                let (sx, sy) = project(i.rx, i.ry, i.rz);
                (sx, sy, sx, sy)
            })
            .reduce(
                || (i32::MAX, i32::MAX, i32::MIN, i32::MIN),
                |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
            );
        Scene {
            items,
            bounds,
            palette: surface.palette.clone(),
            rotation,
        }
    }

    pub fn fit_tier(&self, (max_w, max_h): (u32, u32)) -> i32 {
        let (min_x, min_y, max_x, max_y) = self.bounds;
        pick_tier(|ht| {
            (max_x - min_x) / HALF_TILE * ht + 2 * ht <= max_w as i32
                && (max_y - min_y) / (HALF_TILE / 2) * (ht / 2) + 2 * ht <= max_h as i32
        })
    }

    pub fn size(&self, ht: i32) -> (u32, u32) {
        if self.items.is_empty() {
            return (1, 1);
        }
        let rescale = |v: i32| v / (HALF_TILE / 2) * ht / 2;
        let (min_x, min_y, max_x, max_y) = self.bounds;
        let cell = 2 * ht;
        (
            (rescale(max_x) - rescale(min_x) + cell) as u32,
            (rescale(max_y) - rescale(min_y) + cell) as u32,
        )
    }

    pub fn render(
        &self,
        artist: &TextureArtist,
        ht: i32,
        viewport: Option<(i32, i32, u32, u32)>,
    ) -> RgbaImage {
        let cell = (2 * ht) as usize;
        let rescale = |v: i32| v / (HALF_TILE / 2) * ht / 2;
        let (min_x, min_y) = (rescale(self.bounds.0), rescale(self.bounds.1));
        let (full_w, full_h) = self.size(ht);
        let (ox, oy, width, height) = match viewport {
            Some((x, y, w, h)) => (min_x + x, min_y + y, w as usize, h as usize),
            None => (min_x, min_y, full_w as usize, full_h as usize),
        };
        if width as u64 * height as u64 * 4 > 1 << 30 {
            eprintln!("WARN render {width}x{height} exceeds size limit; skipping");
            return RgbaImage::new(1, 1);
        }
        let mut img = RgbaImage::new(width.max(1) as u32, height.max(1) as u32);
        if width == 0 || height == 0 {
            return img;
        }

        struct Placed {
            x: i32,
            y: i32,
            state: u32,
        }
        let placed: Vec<Placed> = self
            .items
            .par_iter()
            .filter_map(|item| {
                let (sx, sy) = project(item.rx, item.ry, item.rz);
                let x = rescale(sx) - ox;
                let y = rescale(sy) - oy;
                (x > -(cell as i32) && x < width as i32 && y > -(cell as i32) && y < height as i32)
                    .then_some(Placed {
                        x,
                        y,
                        state: item.state,
                    })
            })
            .collect();

        let sprites: Vec<Arc<Sprite>> = self
            .palette
            .iter()
            .map(|n| artist.sprite(n, self.rotation, ht as u32))
            .collect();
        let partial: Vec<bool> = sprites
            .iter()
            .map(|s| s.chunks_exact(4).any(|p| p[3] != 0 && p[3] != 255))
            .collect();

        let band_rows = cell.max(64);
        let nbands = height.div_ceil(band_rows);
        let band_range = |y: i32| {
            let lo = y.max(0) as usize / band_rows;
            let hi = (y + cell as i32 - 1).min(height as i32 - 1) as usize / band_rows;
            lo..=hi
        };
        let mut offsets = vec![0u32; nbands + 1];
        for p in &placed {
            for b in band_range(p.y) {
                offsets[b + 1] += 1;
            }
        }
        for b in 0..nbands {
            offsets[b + 1] += offsets[b];
        }
        let mut index = vec![0u32; offsets[nbands] as usize];
        let mut cursor: Vec<u32> = offsets[..nbands].to_vec();
        for (i, p) in placed.iter().enumerate() {
            for b in band_range(p.y) {
                index[cursor[b] as usize] = i as u32;
                cursor[b] += 1;
            }
        }
        let buf: &mut [u8] = &mut img;
        buf.par_chunks_mut(band_rows * width * 4)
            .enumerate()
            .for_each(|(bi, band)| {
                let band_y0 = bi * band_rows;
                let band_h = band.len() / (width * 4);
                for &i in &index[offsets[bi] as usize..offsets[bi + 1] as usize] {
                    let p = &placed[i as usize];
                    blit_band(
                        band,
                        width,
                        band_y0,
                        band_h,
                        &sprites[p.state as usize],
                        partial[p.state as usize],
                        p.x,
                        p.y,
                        cell,
                    );
                }
            });
        img
    }
}

pub fn render(
    surface: &Surface,
    rotation: Rotation,
    artist: &TextureArtist,
    half_tile: Option<i32>,
    max: (u32, u32),
) -> (RgbaImage, i32) {
    if surface.blocks.is_empty() {
        return (RgbaImage::new(1, 1), HALF_TILE);
    }
    let scene = Scene::new(surface, rotation);
    let ht = half_tile.unwrap_or_else(|| scene.fit_tier(max));
    (scene.render(artist, ht, None), ht)
}

#[allow(clippy::too_many_arguments)]
fn blit_band(
    band: &mut [u8],
    img_w: usize,
    band_y0: usize,
    band_h: usize,
    sprite: &Sprite,
    partial: bool,
    x0: i32,
    y0: i32,
    cell: usize,
) {
    let c0 = (-x0).max(0) as usize;
    let c1 = cell.min((img_w as i32 - x0).max(0) as usize);
    if c0 >= c1 {
        return;
    }
    let y_lo = y0.max(band_y0 as i32).max(0) as usize;
    let y_hi = (y0 + cell as i32).min((band_y0 + band_h) as i32).max(0) as usize;
    for dy in y_lo..y_hi {
        let src_row = &sprite[((dy as i32 - y0) as usize * cell + c0) * 4..][..(c1 - c0) * 4];
        let dst_row =
            &mut band[((dy - band_y0) * img_w + (x0 + c0 as i32) as usize) * 4..][..(c1 - c0) * 4];
        if partial {
            for (dst, src) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(4)) {
                if src[3] == 0 {
                    continue;
                }
                if src[3] == 255 {
                    dst.copy_from_slice(src);
                } else {
                    blend_over(dst.try_into().unwrap(), src.try_into().unwrap());
                }
            }
        } else {
            for (dst, src) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(4)) {
                let s = u32::from_le_bytes(src.try_into().unwrap());
                let d = u32::from_le_bytes(dst.try_into().unwrap());
                let m = 0u32.wrapping_sub((s >> 24 != 0) as u32);
                dst.copy_from_slice(&((s & m) | (d & !m)).to_le_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Block;

    #[test]
    fn four_turns_are_identity_and_distances_survive() {
        for (x, z) in [(0, 0), (3, -7), (-12, 5), (100, 41)] {
            assert_eq!(rotate(x, z, 4), (x, z));
        }
        let pairs: [((i32, i32), (i32, i32)); 2] = [((1, 2), (5, -3)), ((-4, 0), (7, 7))];
        for ((ax, az), (bx, bz)) in pairs {
            let d0 = (ax - bx).pow(2) + (az - bz).pow(2);
            for turns in 1..4 {
                let (rax, raz) = rotate(ax, az, turns);
                let (rbx, rbz) = rotate(bx, bz, turns);
                assert_eq!((rax - rbx).pow(2) + (raz - rbz).pow(2), d0);
            }
        }
    }

    #[test]
    fn projection_of_known_neighbors() {
        let base = project(0, 0, 0);
        assert_eq!(base, (0, 0));
        assert_eq!(project(1, 0, 0), (HALF_TILE, HALF_TILE / 2));
        assert_eq!(project(0, 1, 0), (0, -HALF_TILE));
        assert_eq!(project(0, 0, 1), (-HALF_TILE, HALF_TILE / 2));
    }

    #[test]
    fn painter_order_puts_nearer_blocks_later() {
        let far = (1, 1, 1);
        let near = (3, 3, 3);
        assert_eq!(
            project(far.0, far.2, far.1),
            project(near.0, near.2, near.1)
        );
        assert!(paint_key(near.0, near.1, near.2) > paint_key(far.0, far.1, far.2));
        assert!(paint_key(1, 1, 2) > paint_key(1, 1, 1));
        assert!(paint_key(2, 1, 1) > paint_key(1, 1, 1));
        assert!(paint_key(1, 2, 1) > paint_key(1, 1, 1));
    }

    #[test]
    fn single_block_renders_three_shaded_faces() {
        let surface = Surface {
            palette: vec!["minecraft:stone".into()],
            blocks: vec![Block {
                x: 0,
                y: 0,
                z: 0,
                state: 0,
            }],
        };
        let (img, ht) = render(
            &surface,
            Rotation::TopLeft,
            &TextureArtist::new(vec![]),
            None,
            (MAX_WIDTH, MAX_HEIGHT),
        );
        assert_eq!(ht, 32);
        assert_eq!((img.width(), img.height()), (64, 64));
        let expect = |shade: f64| (125.0 * shade).round() as u8;
        for shade in [1.0, 0.85, 0.65] {
            let v = expect(shade);
            assert!(
                img.pixels().any(|p| p.0 == [v, v, v, 255]),
                "face shade {shade} missing"
            );
        }
        assert!(
            img.pixels().any(|p| p.0[3] == 0),
            "corners stay transparent"
        );
    }

    fn stair_surface() -> Surface {
        Surface {
            palette: vec!["minecraft:stone".into(), "minecraft:grass_block".into()],
            blocks: (0..6)
                .map(|i| Block {
                    x: i,
                    y: i / 2,
                    z: -i,
                    state: (i % 2) as u32,
                })
                .collect(),
        }
    }

    #[test]
    fn scene_size_matches_full_render_dimensions() {
        let surface = stair_surface();
        let artist = TextureArtist::new(vec![]);
        for ht in [4, 8, 16] {
            let scene = Scene::new(&surface, Rotation::TopRight);
            let img = scene.render(&artist, ht, None);
            assert_eq!(scene.size(ht), (img.width(), img.height()));
        }
    }

    #[test]
    fn viewport_matches_full_render_crop() {
        let surface = stair_surface();
        let artist = TextureArtist::new(vec![]);
        let scene = Scene::new(&surface, Rotation::TopLeft);
        let full = scene.render(&artist, 8, None);
        let (fw, fh) = (full.width() as i32, full.height() as i32);
        let (vx, vy, vw, vh) = (5, 3, fw as u32 - 8, fh as u32 - 6);
        let vp = scene.render(&artist, 8, Some((vx, vy, vw, vh)));
        for y in 0..vh {
            for x in 0..vw {
                assert_eq!(
                    vp.get_pixel(x, y),
                    full.get_pixel(x + vx as u32, y + vy as u32),
                    "mismatch at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn viewport_beyond_image_pads_with_transparency() {
        let surface = stair_surface();
        let artist = TextureArtist::new(vec![]);
        let scene = Scene::new(&surface, Rotation::BottomLeft);
        let full = scene.render(&artist, 8, None);
        let (fw, fh) = (full.width() as i32, full.height() as i32);
        let vp = scene.render(
            &artist,
            8,
            Some((-7, -9, (fw + 20) as u32, (fh + 25) as u32)),
        );
        for y in 0..vp.height() as i32 {
            for x in 0..vp.width() as i32 {
                let (sx, sy) = (x - 7, y - 9);
                let expect = if sx >= 0 && sy >= 0 && sx < fw && sy < fh {
                    *full.get_pixel(sx as u32, sy as u32)
                } else {
                    image::Rgba([0, 0, 0, 0])
                };
                assert_eq!(*vp.get_pixel(x as u32, y as u32), expect, "at {x},{y}");
            }
        }
    }

    #[test]
    fn rotations_show_same_block_count_from_different_sides() {
        let surface = Surface {
            palette: vec!["minecraft:stone".into(), "minecraft:grass_block".into()],
            blocks: vec![
                Block {
                    x: 0,
                    y: 0,
                    z: 0,
                    state: 0,
                },
                Block {
                    x: 1,
                    y: 0,
                    z: 0,
                    state: 0,
                },
                Block {
                    x: 1,
                    y: 1,
                    z: 0,
                    state: 1,
                },
            ],
        };
        for rot in [
            Rotation::TopLeft,
            Rotation::TopRight,
            Rotation::BottomRight,
            Rotation::BottomLeft,
        ] {
            let (img, _) = render(
                &surface,
                rot,
                &TextureArtist::new(vec![]),
                None,
                (MAX_WIDTH, MAX_HEIGHT),
            );
            assert!(img.pixels().any(|p| p.0[3] == 255));
        }
    }
}
