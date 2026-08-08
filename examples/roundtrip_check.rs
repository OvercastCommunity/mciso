use mciso::compress::{encode_outputs, Encoding};
use mciso::render::Rotation;
use mciso::{artist, render_view, surface, world};

fn main() -> anyhow::Result<()> {
    let artist = artist::TextureArtist::default_packs();
    let enc = Encoding {
        max_w: 1920,
        max_h: 1080,
        colors: Some(256),
    };
    for arg in std::env::args().skip(1) {
        let w = world::load_world(arg.as_ref())?;
        let direct = w.extract_surface(&|name| artist.occludes(name));
        let decoded = surface::decode(&surface::encode(&direct))?;
        for (rotation, side) in [
            (Rotation::TopLeft, "tl"),
            (Rotation::TopRight, "tr"),
            (Rotation::BottomRight, "br"),
            (Rotation::BottomLeft, "bl"),
        ] {
            let png = |s: &world::Surface| -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
                let (image, crop, _) = render_view(s, rotation, &artist, (1920, 1080))?;
                Ok(encode_outputs(&image, crop, &enc)?)
            };
            let a = png(&direct)?;
            let b = png(&decoded)?;
            println!(
                "{arg} {side}: main {} thumb {}",
                if a.0 == b.0 { "IDENTICAL" } else { "DIFFERS" },
                if a.1 == b.1 { "IDENTICAL" } else { "DIFFERS" },
            );
        }
    }
    Ok(())
}
