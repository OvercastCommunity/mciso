mod colors;
mod entity_models;
mod model;
mod raster;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use image::RgbaImage;

use self::model::{Dir, Element, ModelStore};
use self::raster::{flat_sprite, textured_sprite, FaceTex, Faces};
use crate::assets::Pack;
use crate::sprite::{quarter_turns, Rotation, Sprite};
use crate::state::{self, Props};

const GRASS_TINT: [u8; 3] = [145, 189, 89];
const FOLIAGE_TINT: [u8; 3] = [119, 171, 47];
const WATER_TINT: [u8; 3] = [63, 118, 228];

pub struct TextureArtist {
    packs: Vec<Pack>,
    models: Option<ModelStore>,
    textures: Mutex<HashMap<String, Option<Arc<RgbaImage>>>>,
    sprites: Mutex<HashMap<(String, u32, u32), Arc<Sprite>>>,
    occlusion: Mutex<HashMap<String, bool>>,
    unresolved: Mutex<HashSet<String>>,
}

impl TextureArtist {
    pub fn new(dirs: Vec<PathBuf>) -> TextureArtist {
        TextureArtist::from_packs(dirs.into_iter().map(Pack::Dir).collect())
    }

    fn from_packs(packs: Vec<Pack>) -> TextureArtist {
        TextureArtist {
            packs,
            models: None,
            textures: Mutex::new(HashMap::new()),
            sprites: Mutex::new(HashMap::new()),
            occlusion: Mutex::new(HashMap::new()),
            unresolved: Mutex::new(HashSet::new()),
        }
    }

    fn with_models(packs: Vec<Pack>, models: ModelStore) -> TextureArtist {
        TextureArtist {
            models: Some(models),
            ..TextureArtist::from_packs(packs)
        }
    }

    pub fn from_bundle(mut files: HashMap<String, Vec<u8>>) -> TextureArtist {
        let mc = files
            .extract_if(|k, _| k.starts_with("mc/"))
            .map(|(k, v)| (k["mc/".len()..].to_owned(), v))
            .collect();
        TextureArtist::with_models(
            vec![Pack::Mem {
                base: "block".to_owned(),
                files,
            }],
            ModelStore::from_pack(Pack::Mem {
                base: String::new(),
                files: mc,
            }),
        )
    }

    pub fn default_packs() -> TextureArtist {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = root.join("textures_modern/assets/minecraft/textures/block");
        if !dir.is_dir() {
            eprintln!(
                "WARN texture pack missing at {}; run `git submodule update --init`; using flat colors",
                dir.display()
            );
            return TextureArtist::new(vec![]);
        }
        TextureArtist::with_models(vec![Pack::Dir(dir)], ModelStore::default_assets())
    }

    pub fn sprite(&self, name: &str, rotation: Rotation, half_tile: u32) -> Arc<Sprite> {
        let turns = if name.contains('|') {
            quarter_turns(rotation)
        } else {
            0
        };
        let key = (name.to_owned(), turns, half_tile);
        if let Some(s) = self.sprites.lock().unwrap().get(&key) {
            return Arc::clone(s);
        }
        let sprite = Arc::new(self.build_sprite(name, turns, half_tile));
        self.sprites
            .lock()
            .unwrap()
            .insert(key, Arc::clone(&sprite));
        sprite
    }

    fn build_sprite(&self, name: &str, turns: u32, half_tile: u32) -> Sprite {
        let stripped = state::strip_ns(name);
        let (base, props_str) = state::split_key(stripped);
        let props = Props::parse(props_str);
        let has_texture = |stem: &str| self.texture(stem).is_some();
        if let Some(placements) = entity_models::placements(base, &props, &has_texture) {
            if let Some(sprite) = self.model_sprite(&placements, turns, base, half_tile) {
                return sprite;
            }
        }
        if let Some(placements) = self.models.as_ref().and_then(|m| m.placements(stripped)) {
            if let Some(sprite) = self.model_sprite(&placements, turns, base, half_tile) {
                return sprite;
            }
        }
        match self.resolve(base, &props, turns) {
            Some(faces) => textured_sprite(&faces, half_tile),
            None => {
                if !self.packs.is_empty() && self.unresolved.lock().unwrap().insert(base.to_owned())
                {
                    eprintln!("WARN no texture for {name}; using flat color");
                }
                flat_sprite(base, half_tile)
            }
        }
    }

    fn resolve(&self, base: &str, props: &Props, turns: u32) -> Option<Faces> {
        let lit = props.lit || base.starts_with("lit_");
        let name = alias(base);
        let (mut top_tint, mut side_tint) = tints(name);
        let (top, side) = match name {
            _ if props.snowy && matches!(name, "grass_block" | "podzol" | "mycelium") => {
                (top_tint, side_tint) = (None, None);
                (self.texture("snow"), self.texture("grass_block_snow"))
            }
            "water" | "flowing_water" | "bubble_column" => {
                let t = self.texture("water_still");
                (t.clone(), t)
            }
            "lava" | "flowing_lava" => {
                let t = self.texture("lava_still");
                (t.clone(), t)
            }
            "bookshelf" => (self.texture("oak_planks"), self.texture("bookshelf")),
            "farmland" => (self.texture("farmland"), self.texture("dirt")),
            "carved_pumpkin" | "jack_o_lantern" => {
                (self.texture("pumpkin_top"), self.texture("pumpkin_side"))
            }
            n if n.ends_with("_wood") => {
                let t = self.texture(&format!("{}_log", n.strip_suffix("_wood").unwrap()));
                (t.clone(), t)
            }
            n if n.ends_with("_hyphae") => {
                let t = self.texture(&format!("{}_stem", n.strip_suffix("_hyphae").unwrap()));
                (t.clone(), t)
            }
            n => {
                let mut found = (None, None);
                if lit {
                    found = self.top_side(&format!("{n}_on"));
                }
                if found.0.is_none() && found.1.is_none() {
                    found = self.top_side(n);
                }
                if found.0.is_none() && found.1.is_none() {
                    if let Some(base) = base_block(n) {
                        found = self.top_side(&base);
                    }
                }
                found
            }
        };
        let (top, side) = match (top, side) {
            (Some(t), Some(s)) => (t, s),
            (Some(t), None) => (t.clone(), t),
            (None, Some(s)) => (s.clone(), s),
            (None, None) => return None,
        };
        let front = props.facing.and_then(|_| match name {
            "carved_pumpkin" | "jack_o_lantern" => self.texture(name),
            n => lit
                .then(|| self.texture(&format!("{n}_front_on")))
                .flatten()
                .or_else(|| self.texture(&format!("{n}_front"))),
        });

        let mk = |tex: &Arc<RgbaImage>, tint, rot90| FaceTex {
            tex: Arc::clone(tex),
            tint,
            rot90,
        };
        let mut faces = Faces {
            top: mk(&top, top_tint, false),
            left: mk(&side, side_tint, false),
            right: mk(&side, side_tint, false),
        };
        if let Some(axis) = props.axis {
            let along_rx = (axis == 'x') == turns.is_multiple_of(2);
            faces.top = mk(&side, side_tint, along_rx);
            if along_rx {
                faces.right = mk(&top, top_tint, false);
                faces.left = mk(&side, side_tint, true);
            } else {
                faces.left = mk(&top, top_tint, false);
                faces.right = mk(&side, side_tint, true);
            }
        }
        if let (Some(dir), Some(front)) = (props.facing, front.as_ref()) {
            let (left_n, right_n) = side_normals(turns);
            if dir == left_n {
                faces.left = mk(front, side_tint, false);
            }
            if dir == right_n {
                faces.right = mk(front, side_tint, false);
            }
        }
        Some(faces)
    }

    fn top_side(&self, n: &str) -> (Option<Arc<RgbaImage>>, Option<Arc<RgbaImage>>) {
        (
            self.texture(&format!("{n}_top"))
                .or_else(|| self.texture(n)),
            self.texture(&format!("{n}_side"))
                .or_else(|| self.texture(n)),
        )
    }

    fn texture(&self, stem: &str) -> Option<Arc<RgbaImage>> {
        if let Some(cached) = self.textures.lock().unwrap().get(stem) {
            return cached.clone();
        }
        let loaded = self.packs.iter().find_map(|pack| {
            let bytes = pack.read(&format!("{stem}.png"))?;
            let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
            let img = if img.height() > img.width() {
                image::imageops::crop_imm(&img, 0, 0, img.width(), img.width()).to_image()
            } else {
                img
            };
            Some(Arc::new(img))
        });
        self.textures
            .lock()
            .unwrap()
            .insert(stem.to_owned(), loaded.clone());
        loaded
    }

    pub fn occludes(&self, name: &str) -> bool {
        let base_key = state::split_key(name).0;
        if let Some(&v) = self.occlusion.lock().unwrap().get(base_key) {
            return v;
        }
        let base = state::strip_ns(base_key);
        let v = if matches!(
            base,
            "water" | "flowing_water" | "bubble_column" | "lava" | "flowing_lava"
        ) || base.starts_with("unknown_")
        {
            true
        } else {
            match &self.models {
                None => true,
                Some(models) => models.placements(base).is_some_and(|ps| {
                    ps.iter()
                        .any(|p| p.model.elements.iter().any(|e| self.full_opaque_cube(e)))
                }),
            }
        };
        self.occlusion
            .lock()
            .unwrap()
            .insert(base_key.to_owned(), v);
        v
    }

    fn full_opaque_cube(&self, e: &Element) -> bool {
        e.from == [0.0; 3]
            && e.to == [16.0; 3]
            && Dir::ALL.iter().all(|d| {
                e.faces[*d as usize]
                    .as_ref()
                    .and_then(|f| self.texture(&f.texture))
                    .is_some_and(|t| t.pixels().all(|p| p.0[3] == 255))
            })
    }
}

fn alias(name: &str) -> &str {
    match name {
        "grass" => "grass_block",
        "snow_layer" => "snow",
        "stone_brick" => "stone_bricks",
        "nether_brick" => "nether_bricks",
        "brick" => "bricks",
        "quartz" => "quartz_block",
        "purpur" => "purpur_block",
        "red_flower" => "poppy",
        "yellow_flower" => "dandelion",
        "double_plant" => "tall_grass",
        "trapdoor" => "oak_trapdoor",
        "sign" | "wall_sign" | "standing_sign" => "oak_planks",
        "chest" | "trapped_chest" => "oak_planks",
        "lit_furnace" => "furnace",
        "lit_pumpkin" => "jack_o_lantern",
        "stained_glass" | "stained_glass_pane" | "glass_pane" => "glass",
        n => n,
    }
}

fn base_block(name: &str) -> Option<String> {
    const SUFFIXES: &[&str] = &[
        "_stairs",
        "_slab",
        "_wall",
        "_fence_gate",
        "_fence",
        "_pressure_plate",
        "_button",
        "_carpet",
    ];
    let (suffix, base) = SUFFIXES
        .iter()
        .find_map(|s| Some((*s, name.strip_suffix(s)?)))?;
    let base = alias(if base == "stone" && suffix == "_slab" {
        "smooth_stone"
    } else {
        base
    });
    const WOODS: &[&str] = &[
        "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak",
        "bamboo", "crimson", "warped",
    ];
    Some(if WOODS.contains(&base) {
        format!("{base}_planks")
    } else if suffix == "_carpet" {
        format!("{base}_wool")
    } else {
        base.to_owned()
    })
}

fn side_normals(turns: u32) -> ((i32, i32), (i32, i32)) {
    match turns % 4 {
        0 => ((0, 1), (1, 0)),
        1 => ((1, 0), (0, -1)),
        2 => ((0, -1), (-1, 0)),
        _ => ((-1, 0), (0, 1)),
    }
}

fn tints(name: &str) -> (Option<[u8; 3]>, Option<[u8; 3]>) {
    match name {
        "grass_block" => (Some(GRASS_TINT), None),
        "water" | "flowing_water" | "bubble_column" => (Some(WATER_TINT), Some(WATER_TINT)),
        "vine" | "lily_pad" => (Some(FOLIAGE_TINT), Some(FOLIAGE_TINT)),
        "short_grass" | "tall_grass" | "fern" | "large_fern" | "sugar_cane" => {
            (Some(GRASS_TINT), Some(GRASS_TINT))
        }
        n if n.ends_with("_leaves") => (Some(FOLIAGE_TINT), Some(FOLIAGE_TINT)),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba(rgba))
    }

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    impl std::ops::Deref for TempDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    fn pack(files: &[(&str, &RgbaImage)]) -> (TempDir, TextureArtist) {
        let dir = std::env::temp_dir().join(format!(
            "mciso_texture_test_{}_{:p}",
            std::process::id(),
            files.as_ptr()
        ));
        fs::create_dir_all(&dir).unwrap();
        for (name, img) in files {
            img.save(dir.join(format!("{name}.png"))).unwrap();
        }
        let artist = TextureArtist::new(vec![dir.clone()]);
        (TempDir(dir), artist)
    }

    const CELL: usize = 64;

    fn pixel(sprite: &Sprite, x: usize, y: usize) -> [u8; 4] {
        let i = (y * CELL + x) * 4;
        sprite[i..i + 4].try_into().unwrap()
    }

    fn spr(artist: &TextureArtist, name: &str) -> Arc<Sprite> {
        artist.sprite(name, Rotation::TopLeft, 32)
    }

    #[test]
    fn faces_sample_top_and_side_with_shading() {
        let top = solid(16, 16, [200, 100, 40, 255]);
        let side = solid(16, 16, [100, 200, 60, 255]);
        let (_dir, artist) = pack(&[("foo_top", &top), ("foo_side", &side)]);
        let s = spr(&artist, "minecraft:foo");
        assert_eq!(pixel(&s, 32, 8), [200, 100, 40, 255]);
        let shade = |v: u8, f: f64| (v as f64 * f).round() as u8;
        assert_eq!(
            pixel(&s, 8, 40),
            [shade(100, 0.85), shade(200, 0.85), shade(60, 0.85), 255]
        );
        assert_eq!(
            pixel(&s, 56, 40),
            [shade(100, 0.65), shade(200, 0.65), shade(60, 0.65), 255]
        );
    }

    #[test]
    fn single_texture_covers_all_faces_and_animation_uses_first_frame() {
        let mut strip = solid(16, 32, [0, 0, 255, 255]);
        for y in 0..16 {
            for x in 0..16 {
                strip.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
        let (_dir, artist) = pack(&[("anim", &strip)]);
        let s = spr(&artist, "anim");
        assert_eq!(pixel(&s, 32, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&s, 8, 40), [217, 0, 0, 255]);
    }

    #[test]
    fn top_face_inverse_map_hits_texture_corners() {
        let mut top = solid(16, 16, [10, 10, 10, 255]);
        top.put_pixel(0, 0, image::Rgba([250, 0, 0, 255]));
        top.put_pixel(15, 15, image::Rgba([0, 250, 0, 255]));
        let (_dir, artist) = pack(&[("marked_top", &top), ("marked_side", &top)]);
        let s = spr(&artist, "marked");
        assert_eq!(pixel(&s, 32, 0), [250, 0, 0, 255]);
        assert_eq!(pixel(&s, 32, 30), [0, 250, 0, 255]);
    }

    #[test]
    fn grass_block_tints_top_only_and_legacy_grass_aliases() {
        let white = solid(16, 16, [255, 255, 255, 255]);
        let (_dir, artist) = pack(&[("grass_block_top", &white), ("grass_block_side", &white)]);
        for name in ["minecraft:grass_block", "minecraft:grass"] {
            let s = artist.sprite(name, Rotation::TopLeft, 32);
            assert_eq!(pixel(&s, 32, 8), [145, 189, 89, 255]);
            assert_eq!(pixel(&s, 8, 40), [217, 217, 217, 255]);
        }
    }

    #[test]
    fn structural_blocks_reduce_to_base_textures() {
        let planks = solid(16, 16, [1, 2, 3, 255]);
        let bricks = solid(16, 16, [9, 8, 7, 255]);
        let (_dir, artist) = pack(&[("spruce_planks", &planks), ("stone_bricks", &bricks)]);
        for name in ["spruce_slab", "spruce_stairs", "spruce_fence"] {
            assert_eq!(
                pixel(&artist.sprite(name, Rotation::TopLeft, 32), 32, 8),
                [1, 2, 3, 255],
                "{name}"
            );
        }
        for name in [
            "minecraft:stone_bricks",
            "stone_brick_stairs",
            "stone_brick_slab",
        ] {
            assert_eq!(
                pixel(&artist.sprite(name, Rotation::TopLeft, 32), 32, 8),
                [9, 8, 7, 255],
                "{name}"
            );
        }
    }

    #[test]
    fn horizontal_log_puts_end_grain_on_the_axis_face_per_rotation() {
        let end = solid(16, 16, [200, 0, 0, 255]);
        let bark = solid(16, 16, [0, 100, 0, 255]);
        let (_dir, artist) = pack(&[("oak_log_top", &end), ("oak_log", &bark)]);
        let s = artist.sprite("minecraft:oak_log|axis=x", Rotation::TopLeft, 32);
        assert_eq!(pixel(&s, 56, 40), [130, 0, 0, 255]);
        assert_eq!(pixel(&s, 8, 40), [0, 85, 0, 255]);
        assert_eq!(pixel(&s, 32, 8), [0, 100, 0, 255]);
        let s = artist.sprite("minecraft:oak_log|axis=x", Rotation::TopRight, 32);
        assert_eq!(pixel(&s, 8, 40), [170, 0, 0, 255]);
        assert_eq!(pixel(&s, 56, 40), [0, 65, 0, 255]);
        let s = artist.sprite("minecraft:oak_log", Rotation::TopLeft, 32);
        assert_eq!(pixel(&s, 32, 8), [200, 0, 0, 255]);
    }

    #[test]
    fn snowy_grass_swaps_textures_and_drops_the_tint() {
        let white = solid(16, 16, [255, 255, 255, 255]);
        let snow = solid(16, 16, [240, 240, 240, 255]);
        let snowy_side = solid(16, 16, [120, 120, 120, 255]);
        let (_dir, artist) = pack(&[
            ("grass_block_top", &white),
            ("grass_block_side", &white),
            ("snow", &snow),
            ("grass_block_snow", &snowy_side),
        ]);
        let s = spr(&artist, "minecraft:grass_block|snowy=true");
        assert_eq!(pixel(&s, 32, 8), [240, 240, 240, 255]);
        assert_eq!(pixel(&s, 8, 40), [102, 102, 102, 255]);
        let plain = spr(&artist, "minecraft:grass_block");
        assert_eq!(pixel(&plain, 32, 8), [145, 189, 89, 255]);
    }

    #[test]
    fn lit_blocks_prefer_the_on_texture() {
        let off = solid(16, 16, [30, 30, 30, 255]);
        let on = solid(16, 16, [250, 200, 100, 255]);
        let (_dir, artist) = pack(&[("redstone_lamp", &off), ("redstone_lamp_on", &on)]);
        assert_eq!(
            pixel(&spr(&artist, "minecraft:redstone_lamp|lit=true"), 32, 8),
            [250, 200, 100, 255]
        );
        assert_eq!(
            pixel(&spr(&artist, "minecraft:redstone_lamp"), 32, 8),
            [30, 30, 30, 255]
        );
    }

    #[test]
    fn facing_blocks_show_their_front_on_the_matching_face() {
        let top = solid(16, 16, [10, 10, 10, 255]);
        let side = solid(16, 16, [100, 100, 100, 255]);
        let front = solid(16, 16, [200, 0, 200, 255]);
        let (_dir, artist) = pack(&[
            ("furnace_top", &top),
            ("furnace_side", &side),
            ("furnace_front", &front),
        ]);
        let s = spr(&artist, "minecraft:furnace|facing=east");
        assert_eq!(pixel(&s, 56, 40), [130, 0, 130, 255]);
        assert_eq!(pixel(&s, 8, 40), [85, 85, 85, 255]);
        let s = spr(&artist, "minecraft:furnace|facing=south");
        assert_eq!(pixel(&s, 8, 40), [170, 0, 170, 255]);
        assert_eq!(pixel(&s, 56, 40), [65, 65, 65, 255]);
        let s = spr(&artist, "minecraft:furnace|facing=north");
        assert_eq!(pixel(&s, 8, 40), [85, 85, 85, 255]);
        assert_eq!(pixel(&s, 56, 40), [65, 65, 65, 255]);
    }

    fn model_pack(files: &[(&str, &RgbaImage)]) -> (TempDir, TextureArtist) {
        let (dir, _) = pack(files);
        let artist = TextureArtist::with_models(
            vec![Pack::Dir(dir.to_path_buf())],
            ModelStore::default_assets(),
        );
        (dir, artist)
    }

    #[test]
    fn slab_model_renders_half_height() {
        let planks = solid(16, 16, [200, 100, 40, 255]);
        let (_dir, artist) = model_pack(&[("oak_planks", &planks)]);
        let s = spr(&artist, "minecraft:oak_slab|type=bottom");
        assert_eq!(pixel(&s, 32, 24), [200, 100, 40, 255]);
        assert_eq!(pixel(&s, 32, 8), [0, 0, 0, 0]);
        assert_eq!(pixel(&s, 8, 44), [170, 85, 34, 255]);
        let full = spr(&artist, "minecraft:oak_slab|type=double");
        assert_eq!(pixel(&full, 32, 8), [200, 100, 40, 255]);
    }

    #[test]
    fn stairs_step_follows_facing() {
        let planks = solid(16, 16, [200, 100, 40, 255]);
        let (_dir, artist) = model_pack(&[("oak_planks", &planks)]);
        let east = artist.sprite(
            "minecraft:oak_stairs|facing=east,half=bottom,shape=straight",
            Rotation::TopLeft,
            32,
        );
        assert_eq!(pixel(&east, 56, 20), [130, 65, 26, 255]);
        let west = artist.sprite(
            "minecraft:oak_stairs|facing=west,half=bottom,shape=straight",
            Rotation::TopLeft,
            32,
        );
        assert_eq!(pixel(&west, 56, 20)[3], 0);
    }

    #[test]
    fn fence_renders_a_narrow_post_not_a_cube() {
        let planks = solid(16, 16, [200, 100, 40, 255]);
        let (_dir, artist) = model_pack(&[("oak_planks", &planks)]);
        let s = spr(&artist, "minecraft:oak_fence");
        assert_eq!(pixel(&s, 32, 36), [130, 65, 26, 255]);
        assert_eq!(pixel(&s, 4, 40)[3], 0);
    }

    #[test]
    fn grass_model_tints_top_and_overlay_but_not_base_side() {
        let top = solid(16, 16, [255, 255, 255, 255]);
        let side = solid(16, 16, [100, 100, 100, 255]);
        let clear = solid(16, 16, [0, 0, 0, 0]);
        let (_dir, artist) = model_pack(&[
            ("grass_block_top", &top),
            ("grass_block_side", &side),
            ("grass_block_side_overlay", &clear),
            ("dirt", &side),
        ]);
        let s = spr(&artist, "minecraft:grass_block|snowy=false");
        assert_eq!(pixel(&s, 32, 8), [145, 189, 89, 255]);
        assert_eq!(pixel(&s, 8, 40), [85, 85, 85, 255]);
    }

    #[test]
    fn cross_plants_draw_rotated_quads_with_tint_and_no_shading() {
        let fern = solid(16, 16, [10, 200, 10, 255]);
        let (_dir, artist) = model_pack(&[("fern", &fern)]);
        let s = spr(&artist, "minecraft:fern");
        assert_eq!(pixel(&s, 32, 32), [6, 148, 3, 255]);
        assert_eq!(pixel(&s, 32, 60)[3], 0);
    }

    #[test]
    fn occlusion_follows_model_shape_and_texture_alpha() {
        let opaque = solid(16, 16, [50, 50, 50, 255]);
        let mut cutout = solid(16, 16, [50, 200, 50, 255]);
        cutout.put_pixel(3, 3, image::Rgba([0, 0, 0, 0]));
        let (_dir, artist) = model_pack(&[
            ("stone", &opaque),
            ("oak_planks", &opaque),
            ("oak_leaves", &cutout),
        ]);
        assert!(artist.occludes("minecraft:stone"));
        assert!(!artist.occludes("minecraft:oak_fence"));
        assert!(!artist.occludes("minecraft:oak_slab|type=bottom"));
        assert!(!artist.occludes("minecraft:oak_leaves"));
        assert!(artist.occludes("minecraft:water"));
        assert!(!artist.occludes("minecraft:white_banner"));
        let heuristic = TextureArtist::new(vec![]);
        assert!(heuristic.occludes("minecraft:white_banner"));
    }

    #[test]
    fn chest_renders_entity_atlas_box_with_front_and_latch() {
        let root =
            TempDir(std::env::temp_dir().join(format!("mciso_chest_test_{}", std::process::id())));
        let block = root.join("block");
        let chest_dir = root.join("entity/chest");
        fs::create_dir_all(&block).unwrap();
        fs::create_dir_all(&chest_dir).unwrap();
        let mut atlas = RgbaImage::new(64, 64);
        let mut fill = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]| {
            for y in y0..y1 {
                for x in x0..x1 {
                    atlas.put_pixel(x, y, image::Rgba(c));
                }
            }
        };
        fill(28, 0, 42, 14, [255, 0, 0, 255]);
        fill(28, 14, 42, 19, [0, 255, 0, 255]);
        fill(42, 14, 56, 19, [0, 0, 255, 255]);
        fill(28, 33, 42, 43, [255, 255, 0, 255]);
        fill(42, 33, 56, 43, [255, 0, 255, 255]);
        fill(0, 0, 6, 6, [128, 128, 128, 255]);
        atlas.save(chest_dir.join("normal.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = artist.sprite("minecraft:chest", Rotation::TopLeft, 32);
        let has = |s: &Sprite, c: [u8; 4]| (0..CELL * CELL).any(|i| s[i * 4..i * 4 + 4] == c);
        assert!(has(&s, [255, 0, 0, 255]), "lid top, shade 1.0");
        assert!(has(&s, [0, 0, 217, 255]), "lid front on left face");
        assert!(has(&s, [217, 0, 217, 255]), "base front on left face");
        assert!(has(&s, [109, 109, 109, 255]), "latch on left face");
        assert!(has(&s, [0, 166, 0, 255]), "lid side on right face");
        assert!(has(&s, [166, 166, 0, 255]), "base side on right face");
        let e = artist.sprite("minecraft:chest|facing=east", Rotation::TopLeft, 32);
        assert!(has(&e, [0, 0, 166, 255]), "lid front on right face");
    }

    #[test]
    fn unresolved_names_fall_back_to_flat_colors() {
        let artist = TextureArtist::new(vec![]);
        let s = spr(&artist, "minecraft:stone");
        let expect = |shade: f64| (125.0 * shade).round() as u8;
        for shade in [1.0, 0.85, 0.65] {
            let v = expect(shade);
            assert!((0..CELL * CELL).any(|i| { s[i * 4..i * 4 + 4] == [v, v, v, 255] }));
        }
        assert!((0..CELL * CELL).any(|i| s[i * 4 + 3] == 0));
    }

    #[test]
    fn sprites_are_cached_per_name_and_shared_across_rotations_when_plain() {
        let artist = TextureArtist::new(vec![]);
        let a = artist.sprite("minecraft:stone", Rotation::TopLeft, 32);
        let b = artist.sprite("minecraft:stone", Rotation::TopRight, 32);
        assert!(Arc::ptr_eq(&a, &b));
        let end = solid(16, 16, [200, 0, 0, 255]);
        let bark = solid(16, 16, [0, 100, 0, 255]);
        let (_dir, artist) = pack(&[("oak_log_top", &end), ("oak_log", &bark)]);
        let a = artist.sprite("minecraft:oak_log|axis=x", Rotation::TopLeft, 32);
        let b = artist.sprite("minecraft:oak_log|axis=x", Rotation::TopRight, 32);
        let a2 = artist.sprite("minecraft:oak_log|axis=x", Rotation::TopLeft, 32);
        assert!(!Arc::ptr_eq(&a, &b));
        assert!(Arc::ptr_eq(&a, &a2));
    }

    fn entity_root(tag: &str) -> (TempDir, PathBuf) {
        let root = std::env::temp_dir().join(format!("mciso_{tag}_test_{}", std::process::id()));
        let block = root.join("block");
        fs::create_dir_all(&block).unwrap();
        (TempDir(root), block)
    }

    fn fill(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 4]) {
        for y in y0..y1 {
            for x in x0..x1 {
                img.put_pixel(x, y, image::Rgba(c));
            }
        }
    }

    fn has(s: &Sprite, c: [u8; 4]) -> bool {
        (0..CELL * CELL).any(|i| s[i * 4..i * 4 + 4] == c)
    }

    #[test]
    fn signs_render_boards_from_the_sign_atlas() {
        let (root, block) = entity_root("sign");
        let signs = root.join("entity/signs/hanging");
        fs::create_dir_all(&signs).unwrap();
        let mut atlas = RgbaImage::new(64, 32);
        fill(&mut atlas, 2, 2, 26, 14, [255, 0, 0, 255]);
        fill(&mut atlas, 28, 2, 52, 14, [0, 255, 0, 255]);
        fill(&mut atlas, 2, 16, 4, 30, [0, 0, 255, 255]);
        atlas.save(root.join("entity/signs/oak.png")).unwrap();
        let mut hanging = RgbaImage::new(64, 32);
        fill(&mut hanging, 2, 14, 18, 24, [255, 0, 255, 255]);
        hanging.save(signs.join("oak.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = artist.sprite("minecraft:oak_sign|rotation=0", Rotation::TopLeft, 32);
        assert!(has(&s, [217, 0, 0, 255]), "board front");
        assert!(has(&s, [0, 0, 217, 255]), "post front");
        let n = artist.sprite("minecraft:oak_sign|rotation=8", Rotation::TopLeft, 32);
        assert!(has(&n, [0, 217, 0, 255]), "board back");
        assert!(!has(&n, [217, 0, 0, 255]), "front hidden");
        let w = artist.sprite(
            "minecraft:oak_wall_sign|facing=south",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&w, [217, 0, 0, 255]), "wall board front");
        assert!(!has(&w, [0, 0, 217, 255]), "no post");
        let h = artist.sprite(
            "minecraft:oak_hanging_sign|attached=false",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&h, [217, 0, 217, 255]), "hanging board front");
    }

    #[test]
    fn bed_halves_pick_their_atlas_boxes_and_follow_facing() {
        let (root, block) = entity_root("bed");
        let beds = root.join("entity/bed");
        fs::create_dir_all(&beds).unwrap();
        let mut atlas = RgbaImage::new(64, 64);
        fill(&mut atlas, 6, 6, 22, 22, [255, 0, 0, 255]);
        fill(&mut atlas, 6, 28, 22, 44, [0, 255, 0, 255]);
        fill(&mut atlas, 6, 0, 22, 6, [0, 0, 255, 255]);
        fill(&mut atlas, 22, 22, 38, 28, [255, 255, 0, 255]);
        atlas.save(beds.join("red.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let head = artist.sprite(
            "minecraft:red_bed|facing=south,part=head",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&head, [255, 0, 0, 255]), "head mattress top");
        assert!(has(&head, [0, 0, 217, 255]), "head end on left face");
        let foot = artist.sprite(
            "minecraft:red_bed|facing=north,part=foot",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&foot, [0, 255, 0, 255]), "foot mattress top");
        assert!(has(&foot, [217, 217, 0, 255]), "foot end on left face");
        let east = artist.sprite(
            "minecraft:red_bed|facing=east,part=head",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&east, [0, 0, 166, 255]), "head end on right face");
        assert!(!has(&east, [0, 0, 217, 255]), "not on left face");
    }

    #[test]
    fn banners_tint_grayscale_cloth_with_dye_colors() {
        let (root, block) = entity_root("banner");
        let banner = root.join("entity/banner");
        fs::create_dir_all(&banner).unwrap();
        let mut atlas = RgbaImage::new(64, 64);
        fill(&mut atlas, 1, 1, 21, 41, [200, 200, 200, 255]);
        fill(&mut atlas, 22, 1, 42, 41, [100, 100, 100, 255]);
        atlas.save(banner.join("base.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let red = artist.sprite("minecraft:red_banner|rotation=0", Rotation::TopLeft, 32);
        assert!(has(&red, [117, 31, 25, 255]), "red-tinted cloth front");
        let blue = artist.sprite(
            "minecraft:blue_wall_banner|facing=south",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&blue, [40, 45, 113, 255]), "blue-tinted wall cloth");
    }

    #[test]
    fn skulls_render_head_boxes_from_mob_skins() {
        let (root, block) = entity_root("skull");
        let dir = root.join("entity/skeleton");
        fs::create_dir_all(&dir).unwrap();
        let mut atlas = RgbaImage::new(64, 32);
        fill(&mut atlas, 8, 0, 16, 8, [0, 255, 0, 255]);
        fill(&mut atlas, 8, 8, 16, 16, [255, 0, 0, 255]);
        fill(&mut atlas, 24, 8, 32, 16, [0, 0, 255, 255]);
        atlas.save(dir.join("skeleton.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = artist.sprite("minecraft:skeleton_skull|rotation=0", Rotation::TopLeft, 32);
        assert!(has(&s, [0, 255, 0, 255]), "head top");
        assert!(has(&s, [217, 0, 0, 255]), "face toward camera");
        let n = artist.sprite("minecraft:skeleton_skull|rotation=8", Rotation::TopLeft, 32);
        assert!(has(&n, [0, 0, 217, 255]), "back of head");
        assert!(!has(&n, [217, 0, 0, 255]), "face hidden");
        let w = artist.sprite(
            "minecraft:skeleton_wall_skull|facing=south",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&w, [217, 0, 0, 255]), "wall face");
    }

    #[test]
    fn copper_chest_variants_reuse_the_chest_box() {
        let (root, block) = entity_root("copper_chest");
        let dir = root.join("entity/chest");
        fs::create_dir_all(&dir).unwrap();
        let mut atlas = RgbaImage::new(64, 64);
        fill(&mut atlas, 28, 0, 42, 14, [255, 0, 0, 255]);
        atlas.save(dir.join("copper_oxidized.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = artist.sprite(
            "minecraft:waxed_oxidized_copper_chest|facing=south",
            Rotation::TopLeft,
            32,
        );
        assert!(has(&s, [255, 0, 0, 255]), "lid top from copper atlas");
    }

    #[test]
    fn conduit_renders_centered_box_from_base_atlas() {
        let (root, block) = entity_root("conduit");
        let dir = root.join("entity/conduit");
        fs::create_dir_all(&dir).unwrap();
        let mut atlas = RgbaImage::new(32, 16);
        fill(&mut atlas, 8, 0, 16, 8, [0, 250, 0, 255]);
        fill(&mut atlas, 24, 8, 32, 16, [250, 0, 0, 255]);
        atlas.save(dir.join("base.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = spr(&artist, "minecraft:conduit");
        assert!(has(&s, [0, 250, 0, 255]), "box top");
        assert!(has(&s, [213, 0, 0, 255]), "south side");
    }

    #[test]
    fn end_portal_is_open_sided_and_gateway_is_full_cube() {
        let (root, block) = entity_root("end_portal");
        let dir = root.join("entity/end_portal");
        fs::create_dir_all(&dir).unwrap();
        solid(16, 16, [200, 0, 200, 255])
            .save(dir.join("end_portal.png"))
            .unwrap();

        let artist = TextureArtist::new(vec![block]);
        let p = spr(&artist, "minecraft:end_portal");
        assert!(has(&p, [200, 0, 200, 255]), "portal surface");
        assert!(!has(&p, [170, 0, 170, 255]), "portal sides stay open");
        let g = spr(&artist, "minecraft:end_gateway");
        assert!(has(&g, [170, 0, 170, 255]), "gateway south face");
    }

    #[test]
    fn decorated_pot_mixes_base_and_side_atlases() {
        let (root, block) = entity_root("pot");
        let dir = root.join("entity/decorated_pot");
        fs::create_dir_all(&dir).unwrap();
        let mut base = RgbaImage::new(32, 32);
        fill(&mut base, 0, 13, 14, 27, [250, 0, 0, 255]);
        base.save(dir.join("decorated_pot_base.png")).unwrap();
        solid(16, 16, [0, 0, 250, 255])
            .save(dir.join("decorated_pot_side.png"))
            .unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = spr(&artist, "minecraft:decorated_pot");
        assert!(has(&s, [250, 0, 0, 255]), "pot top from base atlas");
        assert!(has(&s, [0, 0, 213, 255]), "pot side from side texture");
    }

    #[test]
    fn dragon_head_renders_head_and_snout_from_dragon_atlas() {
        let (root, block) = entity_root("dragon");
        let dir = root.join("entity/enderdragon");
        fs::create_dir_all(&dir).unwrap();
        let mut atlas = RgbaImage::new(256, 256);
        fill(&mut atlas, 128, 30, 144, 46, [0, 250, 0, 255]);
        fill(&mut atlas, 192, 44, 204, 60, [250, 0, 0, 255]);
        atlas.save(dir.join("dragon.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = artist.sprite("minecraft:dragon_head|rotation=0", Rotation::TopLeft, 32);
        assert!(has(&s, [0, 250, 0, 255]), "head top");
        assert!(has(&s, [250, 0, 0, 255]), "snout top");
    }

    #[test]
    fn copper_golem_statue_stacks_head_body_feet() {
        let (root, block) = entity_root("golem");
        let dir = root.join("entity/copper_golem");
        fs::create_dir_all(&dir).unwrap();
        let mut atlas = RgbaImage::new(64, 64);
        fill(&mut atlas, 10, 0, 18, 10, [0, 250, 0, 255]);
        fill(&mut atlas, 6, 21, 14, 30, [250, 0, 0, 255]);
        atlas.save(dir.join("copper_golem_oxidized.png")).unwrap();

        let artist = TextureArtist::new(vec![block]);
        let s = spr(&artist, "minecraft:waxed_oxidized_copper_golem_statue");
        assert!(has(&s, [0, 250, 0, 255]), "head top");
        assert!(has(&s, [213, 0, 0, 255]), "body front toward camera");
    }
}
