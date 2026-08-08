pub mod artist;
pub mod assets;
pub mod cleanup;
pub mod compress;
pub mod maps;
pub mod render;
pub mod sprite;
pub mod state;
pub mod surface;
#[cfg(target_arch = "wasm32")]
mod wasm;
pub mod world;

use anyhow::{Context, Result};

pub fn render_view(
    surface: &world::Surface,
    rotation: render::Rotation,
    artist: &artist::TextureArtist,
    max: (u32, u32),
) -> Result<(image::RgbaImage, cleanup::Crop, i32)> {
    let (mut image, mut ht) = render::render(surface, rotation, artist, None, max);
    let transparent = "rendered fully transparent";
    let mut crop = cleanup::crop_to_content(&image, 2 * ht as u32).context(transparent)?;
    let best = render::refit_tier(crop.w, crop.h, ht, max);
    if best != ht {
        (image, ht) = render::render(surface, rotation, artist, Some(best), max);
        crop = cleanup::crop_to_content(&image, 2 * ht as u32).context(transparent)?;
    }
    Ok((image, crop, ht))
}
