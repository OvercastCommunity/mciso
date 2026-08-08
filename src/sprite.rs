pub type Sprite = Vec<u8>;

pub const SHADE_TOP: f64 = 1.0;
pub const SHADE_LEFT: f64 = 0.85;
pub const SHADE_RIGHT: f64 = 0.65;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

pub(crate) fn quarter_turns(rotation: Rotation) -> u32 {
    match rotation {
        Rotation::TopLeft => 0,
        Rotation::TopRight => 1,
        Rotation::BottomRight => 2,
        Rotation::BottomLeft => 3,
    }
}

pub(crate) fn blend_over(dst: &mut [u8; 4], src: &[u8; 4]) {
    let sa = src[3] as u32;
    let da = dst[3] as u32 * (255 - sa) / 255;
    let oa = sa + da;
    if oa == 0 {
        return;
    }
    for c in 0..3 {
        dst[c] = ((src[c] as u32 * sa + dst[c] as u32 * da) / oa) as u8;
    }
    dst[3] = oa as u8;
}
