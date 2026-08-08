use image::RgbaImage;

const WORD: usize = 64;

#[derive(Clone)]
pub(super) struct Bitmap {
    width: usize,
    height: usize,
    wpr: usize,
    data: Vec<u64>,
}

impl Bitmap {
    pub(super) fn new(width: usize, height: usize) -> Bitmap {
        let wpr = width.div_ceil(WORD);
        Bitmap {
            width,
            height,
            wpr,
            data: vec![0; wpr * height],
        }
    }

    pub(super) fn from_alpha(img: &RgbaImage) -> Bitmap {
        use rayon::prelude::*;
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut out = Bitmap::new(w, h);
        let raw = img.as_raw();
        let wpr = out.wpr;
        if wpr == 0 {
            return out;
        }
        out.data
            .par_chunks_mut(wpr)
            .enumerate()
            .for_each(|(y, row)| {
                for (wi, word) in row.iter_mut().enumerate() {
                    let x0 = wi * WORD;
                    let mut v = 0u64;
                    for i in 0..WORD.min(w - x0) {
                        v |= ((raw[(y * w + x0 + i) * 4 + 3] != 0) as u64) << i;
                    }
                    *word = v;
                }
            });
        out
    }

    pub(super) fn from_alpha_downsampled(img: &RgbaImage, factor: usize) -> Bitmap {
        use rayon::prelude::*;
        assert!(factor > 0);
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut out = Bitmap::new(w.div_ceil(factor), h.div_ceil(factor));
        let raw = img.as_raw();
        let wpr = out.wpr;
        if wpr == 0 || h == 0 {
            return out;
        }
        out.data
            .par_chunks_mut(wpr)
            .enumerate()
            .for_each(|(oy, orow)| {
                for y in oy * factor..((oy + 1) * factor).min(h) {
                    let row = &raw[y * w * 4..(y + 1) * w * 4];
                    for (ox, blk) in row.chunks(factor * 4).enumerate() {
                        if blk.chunks_exact(4).any(|p| p[3] != 0) {
                            orow[ox / WORD] |= 1u64 << (ox % WORD);
                        }
                    }
                }
            });
        out
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    pub(super) fn height(&self) -> usize {
        self.height
    }

    pub(super) fn get(&self, x: usize, y: usize) -> bool {
        debug_assert!(x < self.width && y < self.height);
        self.data[y * self.wpr + x / WORD] >> (x % WORD) & 1 != 0
    }

    pub(super) fn set(&mut self, x: usize, y: usize, v: bool) {
        debug_assert!(x < self.width && y < self.height);
        let word = &mut self.data[y * self.wpr + x / WORD];
        let bit = 1u64 << (x % WORD);
        if v {
            *word |= bit;
        } else {
            *word &= !bit;
        }
    }

    pub(super) fn pad(&self, n: usize) -> Bitmap {
        let mut out = Bitmap::new(self.width + 2 * n, self.height + 2 * n);
        for y in 0..self.height {
            let src = &self.data[y * self.wpr..(y + 1) * self.wpr];
            let dst = &mut out.data[(y + n) * out.wpr..(y + n + 1) * out.wpr];
            shift_row_into(src, dst, n as isize);
        }
        out.mask_tails();
        out
    }

    pub(super) fn unpad(&self, n: usize) -> Bitmap {
        assert!(self.width >= 2 * n && self.height >= 2 * n);
        let mut out = Bitmap::new(self.width - 2 * n, self.height - 2 * n);
        for y in 0..out.height {
            let src = &self.data[(y + n) * self.wpr..(y + n + 1) * self.wpr];
            let dst = &mut out.data[y * out.wpr..(y + 1) * out.wpr];
            shift_row_into(src, dst, -(n as isize));
        }
        out.mask_tails();
        out
    }

    #[cfg(test)]
    pub(super) fn downsample(&self, factor: usize) -> Bitmap {
        assert!(factor > 0);
        let mut out = Bitmap::new(self.width.div_ceil(factor), self.height.div_ceil(factor));
        let mut acc = vec![0u64; self.wpr];
        for oy in 0..out.height {
            acc.fill(0);
            for y in oy * factor..((oy + 1) * factor).min(self.height) {
                for (a, w) in acc
                    .iter_mut()
                    .zip(&self.data[y * self.wpr..(y + 1) * self.wpr])
                {
                    *a |= w;
                }
            }
            for ox in 0..out.width {
                let x0 = ox * factor;
                let x1 = ((ox + 1) * factor).min(self.width);
                if (x0..x1).any(|x| acc[x / WORD] >> (x % WORD) & 1 != 0) {
                    out.set(ox, oy, true);
                }
            }
        }
        out
    }

    pub(super) fn dilate(&self, k: &Kernel) -> Bitmap {
        self.dilate_runs(&k.runs)
    }

    pub(super) fn erode(&self, k: &Kernel) -> Bitmap {
        let reflected: Vec<Run> = k
            .runs
            .iter()
            .map(|r| Run {
                dy: -r.dy,
                x0: -r.x1,
                x1: -r.x0,
            })
            .collect();
        self.complement().dilate_runs(&reflected).complement()
    }

    pub(super) fn and(&self, other: &Bitmap) -> Bitmap {
        assert_eq!((self.width, self.height), (other.width, other.height));
        let mut out = self.clone();
        for (d, s) in out.data.iter_mut().zip(&other.data) {
            *d &= s;
        }
        out
    }

    pub(super) fn any(&self) -> bool {
        self.data.iter().any(|&w| w != 0)
    }

    pub(super) fn bbox(&self) -> Option<(usize, usize, usize, usize)> {
        let mut bounds: Option<(usize, usize, usize, usize)> = None;
        for y in 0..self.height {
            let row = &self.data[y * self.wpr..(y + 1) * self.wpr];
            let Some(first_w) = row.iter().position(|&w| w != 0) else {
                continue;
            };
            let last_w = row.iter().rposition(|&w| w != 0).unwrap();
            let min_x = first_w * WORD + row[first_w].trailing_zeros() as usize;
            let max_x = last_w * WORD + (WORD - 1 - row[last_w].leading_zeros() as usize);
            bounds = Some(match bounds {
                None => (min_x, y, max_x, y),
                Some((x0, y0, x1, _)) => (x0.min(min_x), y0, x1.max(max_x), y),
            });
        }
        bounds
    }

    fn complement(&self) -> Bitmap {
        let mut out = self.clone();
        for w in &mut out.data {
            *w = !*w;
        }
        out.mask_tails();
        out
    }

    fn mask_tails(&mut self) {
        let rem = self.width % WORD;
        if rem == 0 || self.wpr == 0 {
            return;
        }
        let mask = (1u64 << rem) - 1;
        for y in 0..self.height {
            self.data[y * self.wpr + self.wpr - 1] &= mask;
        }
    }

    fn dilate_runs(&self, runs: &[Run]) -> Bitmap {
        let mut out = Bitmap::new(self.width, self.height);
        for run in runs {
            let span = self.span_or(run.x0, run.x1);
            for y in 0..self.height as isize {
                let sy = y - run.dy as isize;
                if sy < 0 || sy >= self.height as isize {
                    continue;
                }
                let src = &span.data[sy as usize * self.wpr..(sy as usize + 1) * self.wpr];
                let dst = &mut out.data[y as usize * self.wpr..(y as usize + 1) * self.wpr];
                for (d, s) in dst.iter_mut().zip(src) {
                    *d |= s;
                }
            }
        }
        out.mask_tails();
        out
    }

    fn span_or(&self, x0: i32, x1: i32) -> Bitmap {
        let mut out = Bitmap::new(self.width, self.height);
        if x1 >= 0 {
            let a0 = x0.max(0);
            let part = self
                .shift_x(a0 as isize)
                .span_dir((x1 - a0) as usize + 1, 1);
            out.or_assign(&part);
        }
        if x0 < 0 {
            let b1 = x1.min(-1);
            let part = self
                .shift_x(b1 as isize)
                .span_dir((b1 - x0) as usize + 1, -1);
            out.or_assign(&part);
        }
        out
    }

    fn span_dir(mut self, len: usize, dir: isize) -> Bitmap {
        let mut done = 1;
        while done < len {
            let step = done.min(len - done);
            let shifted = self.shift_x(dir * step as isize);
            self.or_assign(&shifted);
            done += step;
        }
        self
    }

    fn shift_x(&self, s: isize) -> Bitmap {
        if s == 0 {
            return self.clone();
        }
        let mut out = Bitmap::new(self.width, self.height);
        for y in 0..self.height {
            let src = &self.data[y * self.wpr..(y + 1) * self.wpr];
            let dst = &mut out.data[y * self.wpr..(y + 1) * self.wpr];
            shift_row_into(src, dst, s);
        }
        out.mask_tails();
        out
    }

    fn or_assign(&mut self, other: &Bitmap) {
        for (d, s) in self.data.iter_mut().zip(&other.data) {
            *d |= s;
        }
    }
}

fn shift_row_into(src: &[u64], dst: &mut [u64], s: isize) {
    let at = |i: isize| -> u64 {
        if i >= 0 && (i as usize) < src.len() {
            src[i as usize]
        } else {
            0
        }
    };
    if s >= 0 {
        let (wo, b) = ((s as usize / WORD) as isize, s as usize % WORD);
        for (i, d) in dst.iter_mut().enumerate() {
            let i = i as isize;
            *d = if b == 0 {
                at(i - wo)
            } else {
                at(i - wo) << b | at(i - wo - 1) >> (WORD - b)
            };
        }
    } else {
        let t = -s as usize;
        let (wo, b) = ((t / WORD) as isize, t % WORD);
        for (i, d) in dst.iter_mut().enumerate() {
            let i = i as isize;
            *d = if b == 0 {
                at(i + wo)
            } else {
                at(i + wo) >> b | at(i + wo + 1) << (WORD - b)
            };
        }
    }
}

#[derive(Clone, Copy)]
struct Run {
    dy: i32,
    x0: i32,
    x1: i32,
}

pub(super) struct Kernel {
    runs: Vec<Run>,
}

impl Kernel {
    pub(super) fn from_mask(mask: &[Vec<bool>]) -> Kernel {
        let h = mask.len();
        let w = mask.first().map_or(0, |r| r.len());
        let (ax, ay) = (w as i32 / 2, h as i32 / 2);
        let mut runs = Vec::new();
        for (ky, row) in mask.iter().enumerate() {
            let mut x = 0;
            while x < row.len() {
                if row[x] {
                    let start = x;
                    while x < row.len() && row[x] {
                        x += 1;
                    }
                    runs.push(Run {
                        dy: ky as i32 - ay,
                        x0: start as i32 - ax,
                        x1: x as i32 - 1 - ax,
                    });
                } else {
                    x += 1;
                }
            }
        }
        Kernel { runs }
    }

    pub(super) fn hexagon(blocks: u32, block_px: u32) -> Kernel {
        let size = (blocks * block_px) as usize;
        let center = round_half_even(size as f64 / 2.0);
        let inner_min = round_half_even(size as f64 / 4.0);
        let inner_max = round_half_even(size as f64 * 3.0 / 4.0);

        let mut mask = vec![vec![false; size]; size];
        for row in mask.iter_mut().take(inner_max).skip(inner_min) {
            row.fill(true);
        }
        let mut cap = |row: usize, displace: usize| {
            let lo = (center as isize - displace as isize - 1).max(0) as usize;
            let hi = (center + displace).min(size);
            mask[row][lo..hi].fill(true);
        };
        for i in 0..inner_min {
            cap(i, 2 * i + 1);
            let mirror = inner_max + i;
            if mirror < size {
                cap(mirror, 2 * (inner_min - i - 1) + 1);
            }
        }
        Kernel::from_mask(&mask)
    }

    pub(super) fn snowflake(size: u32, width: u32) -> Kernel {
        let size = size as usize;
        let c = (size as f64 - 1.0) / 2.0;
        let half = width as f64 / 2.0;
        let angles = [0f64, 63.5, -63.5];
        let dirs: Vec<(f64, f64)> = angles
            .iter()
            .map(|a| {
                let r = a.to_radians();
                (r.sin(), r.cos())
            })
            .collect();
        let mut mask = vec![vec![false; size]; size];
        for (y, row) in mask.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                let (px, py) = (x as f64 - c, y as f64 - c);
                *cell = dirs
                    .iter()
                    .any(|(ux, uy)| (px * uy - py * ux).abs() <= half);
            }
        }
        Kernel::from_mask(&mask)
    }
}

fn round_half_even(v: f64) -> usize {
    let r = v.round();
    if (v - v.floor() - 0.5).abs() < f64::EPSILON && r as u64 % 2 == 1 {
        (r - 1.0) as usize
    } else {
        r as usize
    }
}

pub(super) struct Labels {
    pub(super) labels: Vec<u32>,
    pub(super) areas: Vec<u64>,
}

pub(super) fn connected_components(bitmap: &Bitmap) -> Labels {
    let (w, h) = (bitmap.width, bitmap.height);
    let mut labels = vec![0u32; w * h];
    let mut areas = vec![0u64];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            if !bitmap.get(sx, sy) || labels[sy * w + sx] != 0 {
                continue;
            }
            let label = areas.len() as u32;
            let mut area = 0u64;
            labels[sy * w + sx] = label;
            stack.push((sx, sy));
            while let Some((x, y)) = stack.pop() {
                area += 1;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                            continue;
                        }
                        let (nx, ny) = (nx as usize, ny as usize);
                        if bitmap.get(nx, ny) && labels[ny * w + nx] == 0 {
                            labels[ny * w + nx] = label;
                            stack.push((nx, ny));
                        }
                    }
                }
            }
            areas.push(area);
        }
    }
    Labels { labels, areas }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn chance(&mut self, percent: u64) -> bool {
            self.next() % 100 < percent
        }
    }

    fn random_bitmap(rng: &mut Rng, w: usize, h: usize, density: u64) -> Bitmap {
        let mut b = Bitmap::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if rng.chance(density) {
                    b.set(x, y, true);
                }
            }
        }
        b
    }

    fn random_mask(rng: &mut Rng, w: usize, h: usize) -> Vec<Vec<bool>> {
        let mut mask = vec![vec![false; w]; h];
        for row in mask.iter_mut() {
            for cell in row.iter_mut() {
                *cell = rng.chance(30);
            }
        }
        mask[h / 2][w / 2] = true;
        mask
    }

    fn offsets(mask: &[Vec<bool>]) -> Vec<(i32, i32)> {
        let (ax, ay) = (mask[0].len() as i32 / 2, mask.len() as i32 / 2);
        let mut out = Vec::new();
        for (ky, row) in mask.iter().enumerate() {
            for (kx, &m) in row.iter().enumerate() {
                if m {
                    out.push((kx as i32 - ax, ky as i32 - ay));
                }
            }
        }
        out
    }

    fn naive_dilate(b: &Bitmap, mask: &[Vec<bool>]) -> Bitmap {
        let mut out = Bitmap::new(b.width(), b.height());
        let get = |x: i32, y: i32| {
            x >= 0
                && y >= 0
                && (x as usize) < b.width()
                && (y as usize) < b.height()
                && b.get(x as usize, y as usize)
        };
        for y in 0..b.height() {
            for x in 0..b.width() {
                let v = offsets(mask)
                    .iter()
                    .any(|&(dx, dy)| get(x as i32 - dx, y as i32 - dy));
                out.set(x, y, v);
            }
        }
        out
    }

    fn naive_erode(b: &Bitmap, mask: &[Vec<bool>]) -> Bitmap {
        let mut out = Bitmap::new(b.width(), b.height());
        let get = |x: i32, y: i32| {
            x < 0
                || y < 0
                || (x as usize) >= b.width()
                || (y as usize) >= b.height()
                || b.get(x as usize, y as usize)
        };
        for y in 0..b.height() {
            for x in 0..b.width() {
                let v = offsets(mask)
                    .iter()
                    .all(|&(dx, dy)| get(x as i32 + dx, y as i32 + dy));
                out.set(x, y, v);
            }
        }
        out
    }

    fn assert_same(a: &Bitmap, b: &Bitmap, what: &str) {
        assert_eq!((a.width(), a.height()), (b.width(), b.height()));
        for y in 0..a.height() {
            for x in 0..a.width() {
                assert_eq!(a.get(x, y), b.get(x, y), "{what} differs at ({x}, {y})");
            }
        }
    }

    fn subset(a: &Bitmap, b: &Bitmap) -> bool {
        (0..a.height()).all(|y| (0..a.width()).all(|x| !a.get(x, y) || b.get(x, y)))
    }

    #[test]
    fn dilate_erode_match_naive_reference() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for &w in &[63usize, 64, 65, 130] {
            for trial in 0..4 {
                let b = random_bitmap(&mut rng, w, 40, 30);
                let mask = random_mask(&mut rng, 5 + trial % 3, 3 + trial);
                let k = Kernel::from_mask(&mask);
                assert_same(&b.dilate(&k), &naive_dilate(&b, &mask), "dilate");
                assert_same(&b.erode(&k), &naive_erode(&b, &mask), "erode");
            }
        }
    }

    #[test]
    fn downsample_matches_naive_or_reduce() {
        let mut rng = Rng(0xfeed_beef_1234_5678);
        for &(w, h) in &[(63usize, 40usize), (64, 41), (130, 30), (7, 7)] {
            for &factor in &[1usize, 3, 4] {
                let b = random_bitmap(&mut rng, w, h, 20);
                let d = b.downsample(factor);
                assert_eq!(d.width(), w.div_ceil(factor));
                assert_eq!(d.height(), h.div_ceil(factor));
                for oy in 0..d.height() {
                    for ox in 0..d.width() {
                        let mut any = false;
                        for y in oy * factor..((oy + 1) * factor).min(h) {
                            for x in ox * factor..((ox + 1) * factor).min(w) {
                                any |= b.get(x, y);
                            }
                        }
                        assert_eq!(d.get(ox, oy), any, "block ({ox},{oy}) factor {factor}");
                    }
                }
            }
        }
    }

    #[test]
    fn from_alpha_downsampled_matches_two_step() {
        let mut rng = Rng(0xabcd_ef01_2345_6789);
        for &(w, h) in &[(63u32, 40u32), (64, 41), (130, 30), (7, 7), (5, 9)] {
            for &factor in &[1usize, 3, 4] {
                let mut img = RgbaImage::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        if rng.chance(25) {
                            img.put_pixel(x, y, image::Rgba([9, 9, 9, (rng.next() % 256) as u8]));
                        }
                    }
                }
                let fused = Bitmap::from_alpha_downsampled(&img, factor);
                let two_step = Bitmap::from_alpha(&img).downsample(factor);
                assert_same(&fused, &two_step, &format!("{w}x{h} factor {factor}"));
            }
        }
    }

    #[test]
    fn closing_grows_opening_shrinks() {
        let mut rng = Rng(42);
        for trial in 0..6 {
            let b = random_bitmap(&mut rng, 70, 50, 40);
            let k = Kernel::from_mask(&random_mask(&mut rng, 3 + trial, 4));
            assert!(
                subset(&b, &b.dilate(&k).erode(&k)),
                "closing must contain input"
            );
            assert!(
                subset(&b.erode(&k).dilate(&k), &b),
                "opening must fit in input"
            );
        }
    }

    #[test]
    fn border_semantics() {
        let box3 = Kernel::from_mask(&vec![vec![true; 3]; 3]);
        let mut full = Bitmap::new(20, 15);
        for y in 0..15 {
            for x in 0..20 {
                full.set(x, y, true);
            }
        }
        assert_same(&full.erode(&box3), &full, "erode of full");
        assert!(!Bitmap::new(20, 15).dilate(&box3).any());
    }

    #[test]
    fn connected_components_areas_and_diagonals() {
        let mut b = Bitmap::new(12, 8);
        for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2), (3, 3)] {
            b.set(x, y, true);
        }
        for x in 6..10 {
            b.set(x, 1, true);
        }
        b.set(10, 6, true);
        let l = connected_components(&b);
        assert_eq!(l.areas.len(), 4, "3 components + unused slot 0");
        assert_eq!(
            l.labels[12 + 1],
            l.labels[3 * 12 + 3],
            "diagonal pair joined"
        );
        let mut areas = l.areas[1..].to_vec();
        areas.sort_unstable();
        assert_eq!(areas, vec![1, 4, 5]);
        assert_eq!(l.labels[0], 0, "background is 0");
    }

    #[test]
    fn pad_unpad_round_trip() {
        let mut rng = Rng(7);
        let b = random_bitmap(&mut rng, 100, 30, 50);
        for &n in &[1usize, 64, 150] {
            let p = b.pad(n);
            assert_eq!(
                (p.width(), p.height()),
                (b.width() + 2 * n, b.height() + 2 * n)
            );
            assert!(!p.get(0, 0) && !p.get(p.width() - 1, p.height() - 1));
            assert_same(&p.unpad(n), &b, "pad/unpad round trip");
        }
    }

    #[test]
    fn bbox_known_pattern() {
        let mut b = Bitmap::new(130, 40);
        assert_eq!(b.bbox(), None);
        b.set(5, 3, true);
        b.set(127, 30, true);
        b.set(64, 10, true);
        assert_eq!(b.bbox(), Some((5, 3, 127, 30)));
    }

    #[test]
    fn from_alpha_thresholds_at_zero() {
        let mut img = RgbaImage::new(4, 2);
        img.put_pixel(1, 0, image::Rgba([9, 9, 9, 1]));
        img.put_pixel(3, 1, image::Rgba([9, 9, 9, 255]));
        let b = Bitmap::from_alpha(&img);
        assert!(b.get(1, 0) && b.get(3, 1));
        assert!(!b.get(0, 0) && !b.get(2, 1));
    }

    #[test]
    fn hexagon_matches_python_shape() {
        let k = Kernel::hexagon(1, 23);
        let mut b = Bitmap::new(23, 23);
        b.set(11, 11, true);
        let d = b.dilate(&k);
        let row_width = |y: usize| (0..23).filter(|&x| d.get(x, y)).count();
        assert_eq!(row_width(0), 3);
        assert_eq!(row_width(1), 7);
        assert_eq!(row_width(5), 23);
        assert_eq!(row_width(11), 23);
        assert_eq!(row_width(22), 3);
        for y in 0..23 {
            assert_eq!(row_width(y), (0..23).filter(|&x| d.get(x, 22 - y)).count());
        }
    }

    #[test]
    fn snowflake_symmetry_and_center_width() {
        let k = Kernel::snowflake(41, 5);
        let mut b = Bitmap::new(41, 41);
        b.set(20, 20, true);
        let d = b.dilate(&k);
        for y in 0..41 {
            for x in 0..41 {
                assert_eq!(d.get(x, y), d.get(40 - x, y), "mirror x");
                assert_eq!(d.get(x, y), d.get(x, 40 - y), "mirror y (180 + x-mirror)");
            }
        }
        let top = (0..41).filter(|&x| d.get(x, 0)).count();
        assert_eq!(top, 5);
    }
}
