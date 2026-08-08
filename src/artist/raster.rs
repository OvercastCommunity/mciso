use std::sync::Arc;

use image::RgbaImage;

use super::model::{Dir, Element, Face as ModelFace, Placement};
use super::TextureArtist;
use crate::sprite::{blend_over, Sprite, SHADE_LEFT, SHADE_RIGHT, SHADE_TOP};

fn cell(half_tile: u32) -> usize {
    (2 * half_tile) as usize
}

impl TextureArtist {
    pub(super) fn model_sprite(
        &self,
        placements: &[Placement],
        turns: u32,
        base: &str,
        half_tile: u32,
    ) -> Option<Sprite> {
        let (cell, sc) = (cell(half_tile), half_tile as f32 / 16.0);
        struct Prim<'a> {
            key: f32,
            elem: &'a Element,
            xp: u32,
            q: u32,
        }
        let mut prims: Vec<Prim> = Vec::new();
        for p in placements {
            let q = (p.y + turns) % 4;
            for elem in &p.model.elements {
                let a = tf(elem.from, p.x, q);
                let b = tf(elem.to, p.x, q);
                prims.push(Prim {
                    key: a[0] + a[1] + a[2] + b[0] + b[1] + b[2],
                    elem,
                    xp: p.x,
                    q,
                });
            }
        }
        prims.sort_by(|a, b| a.key.total_cmp(&b.key));

        let mut px = vec![0u8; cell * cell * 4];
        let mut drew = false;
        for prim in &prims {
            let e = prim.elem;
            if e.rotation
                .is_some_and(|r| r.axis == 'y' && r.angle.abs() > 1.0)
            {
                drew |= self.draw_rotated_quads(&mut px, e, prim.q, base, half_tile);
                continue;
            }
            let a = tf(e.from, prim.xp, prim.q);
            let b = tf(e.to, prim.xp, prim.q);
            let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
            let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
            for view in [Dir::Up, Dir::South, Dir::East] {
                let mdir = model_dir(view, prim.xp, prim.q);
                let Some(face) = &e.faces[mdir as usize] else {
                    continue;
                };
                let Some(tex) = self.texture(&face.texture) else {
                    continue;
                };
                let shade = match (e.shade, view) {
                    (false, _) => 1.0,
                    (true, Dir::Up) => SHADE_TOP,
                    (true, Dir::South) => SHADE_LEFT,
                    _ => SHADE_RIGHT,
                };
                let tint = face.tintindex.map(|_| model_tint(base));
                drew = true;
                for y in 0..cell {
                    for x in 0..cell {
                        let (sx, sy) = ((x as f32 + 0.5) / sc, (y as f32 + 0.5) / sc);
                        let r = match view {
                            Dir::Up => {
                                let d = sx - 16.0;
                                let s = 2.0 * (sy - 16.0 + hi[1]);
                                [(s + d) / 2.0, hi[1], (s - d) / 2.0]
                            }
                            Dir::South => {
                                let rx = sx - 16.0 + hi[2];
                                [rx, 16.0 + 0.5 * (rx + hi[2]) - sy, hi[2]]
                            }
                            _ => {
                                let rz = 16.0 + hi[0] - sx;
                                [hi[0], 16.0 + 0.5 * (hi[0] + rz) - sy, rz]
                            }
                        };
                        const EPS: f32 = 1e-3;
                        if r[0] < lo[0] - EPS
                            || r[0] > hi[0] + EPS
                            || r[1] < lo[1] - EPS
                            || r[1] > hi[1] + EPS
                            || r[2] < lo[2] - EPS
                            || r[2] > hi[2] + EPS
                        {
                            continue;
                        }
                        let m = inv_tf(r, prim.xp, prim.q);
                        let (fa, fb) = face_ab(mdir, m, e.from, e.to);
                        let rgba = sample(face, &tex, mdir, fa, fb, e.from, e.to);
                        put(&mut px, x, y, rgba, tint, shade, cell);
                    }
                }
            }
        }
        (drew && px.chunks_exact(4).any(|p| p[3] != 0)).then_some(px)
    }

    fn draw_rotated_quads(
        &self,
        px: &mut Sprite,
        e: &Element,
        q: u32,
        base: &str,
        half_tile: u32,
    ) -> bool {
        let (cell, sc) = (cell(half_tile), half_tile as f32 / 16.0);
        let rot = e.rotation.unwrap();
        let (sin, cos) = rot.angle.to_radians().sin_cos();
        let scale = if rot.rescale {
            1.0 / cos.abs().max(0.01)
        } else {
            1.0
        };
        let place = |p: [f32; 2]| -> [f32; 2] {
            let (dx, dz) = (p[0] - rot.origin[0], p[1] - rot.origin[2]);
            let mut out = [
                rot.origin[0] + scale * (dx * cos + dz * sin),
                rot.origin[2] + scale * (dz * cos - dx * sin),
            ];
            for _ in 0..q {
                out = [16.0 - out[1], out[0]];
            }
            out
        };
        let (f, t) = (e.from, e.to);
        let (y0, y1) = (f[1], t[1]);
        if y1 - y0 < 1e-3 {
            return false;
        }
        let segments = [
            (Dir::North, [t[0], f[2]], [f[0], f[2]]),
            (Dir::South, [f[0], t[2]], [t[0], t[2]]),
            (Dir::West, [f[0], f[2]], [f[0], t[2]]),
            (Dir::East, [t[0], t[2]], [t[0], f[2]]),
        ];
        let mut drew = false;
        for (dir, s1, s2) in segments {
            let Some(face) = &e.faces[dir as usize] else {
                continue;
            };
            let Some(tex) = self.texture(&face.texture) else {
                continue;
            };
            let (e1, e2) = (place(s1), place(s2));
            let (sx1, sx2) = (16.0 + e1[0] - e1[1], 16.0 + e2[0] - e2[1]);
            if (sx2 - sx1).abs() < 1e-3 {
                continue;
            }
            let shade = if e.shade { SHADE_LEFT } else { 1.0 };
            let tint = face.tintindex.map(|_| model_tint(base));
            drew = true;
            for x in 0..cell {
                let sx = (x as f32 + 0.5) / sc;
                let s = (sx - sx1) / (sx2 - sx1);
                if !(0.0..=1.0).contains(&s) {
                    continue;
                }
                let rx = e1[0] + s * (e2[0] - e1[0]);
                let rz = e1[1] + s * (e2[1] - e1[1]);
                let top = 16.0 + 0.5 * (rx + rz) - y1;
                for y in 0..cell {
                    let sy = (y as f32 + 0.5) / sc;
                    let b = (sy - top) / (y1 - y0);
                    if !(0.0..1.0).contains(&b) {
                        continue;
                    }
                    let rgba = sample(face, &tex, dir, s, b, f, t);
                    put(px, x, y, rgba, tint, shade, cell);
                }
            }
        }
        drew
    }
}

fn tf(mut p: [f32; 3], xp: u32, q: u32) -> [f32; 3] {
    for _ in 0..xp % 4 {
        p = [p[0], p[2], 16.0 - p[1]];
    }
    for _ in 0..q % 4 {
        p = [16.0 - p[2], p[1], p[0]];
    }
    p
}

fn inv_tf(mut p: [f32; 3], xp: u32, q: u32) -> [f32; 3] {
    for _ in 0..(4 - q % 4) % 4 {
        p = [16.0 - p[2], p[1], p[0]];
    }
    for _ in 0..(4 - xp % 4) % 4 {
        p = [p[0], p[2], 16.0 - p[1]];
    }
    p
}

fn ydir(d: Dir) -> Dir {
    match d {
        Dir::North => Dir::East,
        Dir::East => Dir::South,
        Dir::South => Dir::West,
        Dir::West => Dir::North,
        o => o,
    }
}

fn xdir(d: Dir) -> Dir {
    match d {
        Dir::Up => Dir::North,
        Dir::North => Dir::Down,
        Dir::Down => Dir::South,
        Dir::South => Dir::Up,
        o => o,
    }
}

fn model_dir(view: Dir, xp: u32, q: u32) -> Dir {
    let mut d = view;
    for _ in 0..(4 - q % 4) % 4 {
        d = ydir(d);
    }
    for _ in 0..(4 - xp % 4) % 4 {
        d = xdir(d);
    }
    d
}

fn face_ab(dir: Dir, p: [f32; 3], f: [f32; 3], t: [f32; 3]) -> (f32, f32) {
    let frac = |v: f32, lo: f32, hi: f32| {
        if hi - lo > 1e-3 {
            (v - lo) / (hi - lo)
        } else {
            0.5
        }
    };
    match dir {
        Dir::Up => (frac(p[0], f[0], t[0]), frac(p[2], f[2], t[2])),
        Dir::Down => (frac(p[0], f[0], t[0]), 1.0 - frac(p[2], f[2], t[2])),
        Dir::North => (1.0 - frac(p[0], f[0], t[0]), 1.0 - frac(p[1], f[1], t[1])),
        Dir::South => (frac(p[0], f[0], t[0]), 1.0 - frac(p[1], f[1], t[1])),
        Dir::West => (frac(p[2], f[2], t[2]), 1.0 - frac(p[1], f[1], t[1])),
        Dir::East => (1.0 - frac(p[2], f[2], t[2]), 1.0 - frac(p[1], f[1], t[1])),
    }
}

fn default_uv(dir: Dir, f: [f32; 3], t: [f32; 3]) -> [f32; 4] {
    match dir {
        Dir::Up => [f[0], f[2], t[0], t[2]],
        Dir::Down => [f[0], 16.0 - t[2], t[0], 16.0 - f[2]],
        Dir::North => [16.0 - t[0], 16.0 - t[1], 16.0 - f[0], 16.0 - f[1]],
        Dir::South => [f[0], 16.0 - t[1], t[0], 16.0 - f[1]],
        Dir::West => [f[2], 16.0 - t[1], t[2], 16.0 - f[1]],
        Dir::East => [16.0 - t[2], 16.0 - t[1], 16.0 - f[2], 16.0 - f[1]],
    }
}

fn sample(
    face: &ModelFace,
    tex: &RgbaImage,
    dir: Dir,
    a: f32,
    b: f32,
    f: [f32; 3],
    t: [f32; 3],
) -> [u8; 4] {
    let (a, b) = match face.rotation {
        90 => (b, 1.0 - a),
        180 => (1.0 - a, 1.0 - b),
        270 => (1.0 - b, a),
        _ => (a, b),
    };
    let uv = face.uv.unwrap_or_else(|| default_uv(dir, f, t));
    let u = (uv[0] + (uv[2] - uv[0]) * a.clamp(0.0, 1.0)) / 16.0;
    let v = (uv[1] + (uv[3] - uv[1]) * b.clamp(0.0, 1.0)) / 16.0;
    let tx = ((u * tex.width() as f32) as i64).clamp(0, tex.width() as i64 - 1) as u32;
    let ty = ((v * tex.height() as f32) as i64).clamp(0, tex.height() as i64 - 1) as u32;
    tex.get_pixel(tx, ty).0
}

fn shade_tint(rgba: [u8; 4], tint: Option<[u8; 3]>, shade: f64) -> [u8; 4] {
    let tint = tint.unwrap_or([255, 255, 255]);
    let mut out = [0u8; 4];
    for c in 0..3 {
        out[c] = (rgba[c] as f64 * tint[c] as f64 / 255.0 * shade).round() as u8;
    }
    out[3] = rgba[3];
    out
}

fn put(
    px: &mut Sprite,
    x: usize,
    y: usize,
    rgba: [u8; 4],
    tint: Option<[u8; 3]>,
    shade: f64,
    cell: usize,
) {
    if rgba[3] == 0 {
        return;
    }
    let src = shade_tint(rgba, tint, shade);
    let dst: &mut [u8; 4] = (&mut px[(y * cell + x) * 4..][..4]).try_into().unwrap();
    if src[3] == 255 {
        *dst = src;
    } else {
        blend_over(dst, &src);
    }
}

fn model_tint(base: &str) -> [u8; 3] {
    match base {
        "redstone_wire" => [175, 0, 0],
        "water" | "flowing_water" | "bubble_column" => super::WATER_TINT,
        n if n.ends_with("_leaves") || matches!(n, "vine" | "lily_pad") => super::FOLIAGE_TINT,
        n if n.ends_with("banner") => {
            let dye = n
                .strip_suffix("_wall_banner")
                .or_else(|| n.strip_suffix("_banner"))
                .unwrap_or("white");
            dye_color(dye)
        }
        _ => super::GRASS_TINT,
    }
}

fn dye_color(dye: &str) -> [u8; 3] {
    match dye {
        "orange" => [249, 128, 29],
        "magenta" => [199, 78, 189],
        "light_blue" => [58, 179, 218],
        "yellow" => [254, 216, 61],
        "lime" => [128, 199, 31],
        "pink" => [243, 139, 170],
        "gray" => [71, 79, 82],
        "light_gray" => [157, 157, 151],
        "cyan" => [22, 156, 156],
        "purple" => [137, 50, 184],
        "blue" => [60, 68, 170],
        "brown" => [131, 84, 50],
        "green" => [94, 124, 22],
        "red" => [176, 46, 38],
        "black" => [29, 29, 33],
        _ => [249, 255, 254],
    }
}

pub(super) struct FaceTex {
    pub(super) tex: Arc<RgbaImage>,
    pub(super) tint: Option<[u8; 3]>,
    pub(super) rot90: bool,
}

pub(super) struct Faces {
    pub(super) top: FaceTex,
    pub(super) left: FaceTex,
    pub(super) right: FaceTex,
}

enum Face {
    Top,
    Left,
    Right,
}

fn face_at(x: usize, y: usize, half_tile: u32) -> Option<Face> {
    let (px, py) = (
        (x as f64 + 0.5) * 16.0 / half_tile as f64,
        (y as f64 + 0.5) * 16.0 / half_tile as f64,
    );
    if (px - 16.0).abs() / 16.0 + (py - 8.0).abs() / 8.0 <= 1.0 {
        Some(Face::Top)
    } else if px < 16.0 && (8.0 + px / 2.0..24.0 + px / 2.0).contains(&py) {
        Some(Face::Left)
    } else if px >= 16.0 && (8.0 + (32.0 - px) / 2.0..24.0 + (32.0 - px) / 2.0).contains(&py) {
        Some(Face::Right)
    } else {
        None
    }
}

pub(super) fn textured_sprite(faces: &Faces, half_tile: u32) -> Sprite {
    let cell = cell(half_tile);
    let mut px = vec![0u8; cell * cell * 4];
    for y in 0..cell {
        for x in 0..cell {
            let Some(face) = face_at(x, y, half_tile) else {
                continue;
            };
            let (pxc, pyc) = (
                (x as f64 + 0.5) * 16.0 / half_tile as f64,
                (y as f64 + 0.5) * 16.0 / half_tile as f64,
            );
            let (ft, shade, u, v) = match face {
                Face::Top => (
                    &faces.top,
                    SHADE_TOP,
                    (pxc - 16.0 + 2.0 * pyc) / 2.0,
                    (2.0 * pyc - pxc + 16.0) / 2.0,
                ),
                Face::Left => (&faces.left, SHADE_LEFT, pxc, pyc - 8.0 - pxc / 2.0),
                Face::Right => (
                    &faces.right,
                    SHADE_RIGHT,
                    pxc - 16.0,
                    pyc - 8.0 - (32.0 - pxc) / 2.0,
                ),
            };
            let (u, v) = if ft.rot90 { (v, 16.0 - u) } else { (u, v) };
            let tex = &ft.tex;
            let tx = ((u.clamp(0.0, 15.999) / 16.0) * tex.width() as f64) as u32;
            let ty = ((v.clamp(0.0, 15.999) / 16.0) * tex.height() as f64) as u32;
            let texel = tex.get_pixel(tx.min(tex.width() - 1), ty.min(tex.height() - 1));
            let i = (y * cell + x) * 4;
            px[i..i + 4].copy_from_slice(&shade_tint(texel.0, ft.tint, shade));
        }
    }
    px
}

pub(super) fn flat_sprite(name: &str, half_tile: u32) -> Sprite {
    let [r, g, b] = super::colors::color_for(name);
    let cell = cell(half_tile);
    let mut px = vec![0u8; cell * cell * 4];
    for y in 0..cell {
        for x in 0..cell {
            let Some(face) = face_at(x, y, half_tile) else {
                continue;
            };
            let shade = match face {
                Face::Top => SHADE_TOP,
                Face::Left => SHADE_LEFT,
                Face::Right => SHADE_RIGHT,
            };
            let i = (y * cell + x) * 4;
            px[i..i + 4].copy_from_slice(&shade_tint([r, g, b, 255], None, shade));
        }
    }
    px
}
