use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

use crate::assets::Pack;
use crate::state;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Dir {
    pub(super) const ALL: [Dir; 6] = [
        Dir::Down,
        Dir::Up,
        Dir::North,
        Dir::South,
        Dir::West,
        Dir::East,
    ];

    fn from_name(s: &str) -> Option<Dir> {
        Some(match s {
            "down" => Dir::Down,
            "up" => Dir::Up,
            "north" => Dir::North,
            "south" => Dir::South,
            "west" => Dir::West,
            "east" => Dir::East,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct Face {
    pub(super) texture: String,
    pub(super) uv: Option<[f32; 4]>,
    pub(super) rotation: i32,
    pub(super) tintindex: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ElemRotation {
    pub(super) origin: [f32; 3],
    pub(super) axis: char,
    pub(super) angle: f32,
    pub(super) rescale: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Element {
    pub(super) from: [f32; 3],
    pub(super) to: [f32; 3],
    pub(super) rotation: Option<ElemRotation>,
    pub(super) shade: bool,
    pub(super) faces: [Option<Face>; 6],
}

pub(super) struct BlockModel {
    pub(super) elements: Vec<Element>,
}

pub(super) struct Placement {
    pub(super) model: Arc<BlockModel>,
    pub(super) x: u32,
    pub(super) y: u32,
    #[allow(dead_code)]
    pub(super) uvlock: bool,
}

pub(super) struct ModelStore {
    pack: Pack,
    blockstates: Mutex<HashMap<String, Option<Arc<RawBlockstate>>>>,
    models: Mutex<HashMap<String, Option<Arc<BlockModel>>>>,
}

impl ModelStore {
    pub(super) fn from_pack(pack: Pack) -> ModelStore {
        ModelStore {
            pack,
            blockstates: Mutex::new(HashMap::new()),
            models: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn default_assets() -> ModelStore {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mcassets/assets/minecraft");
        if !root.is_dir() {
            eprintln!(
                "WARN vanilla assets missing at {}; model rendering disabled",
                root.display()
            );
        }
        ModelStore::from_pack(Pack::Dir(root))
    }

    pub(super) fn placements(&self, key: &str) -> Option<Vec<Placement>> {
        let (block, props_str) = state::split_key(state::strip_ns(key));
        let block = state::legacy_alias(block);
        let props: HashMap<&str, &str> = state::props(props_str).collect();
        let state = self.blockstate(block)?;
        let mut out = Vec::new();
        if let Some(variants) = &state.variants {
            let mut best: Option<(usize, &VariantList)> = None;
            for (vkey, variant) in variants {
                let mut matched = 0usize;
                let mut ok = true;
                for (k, v) in state::props(vkey) {
                    match props.get(k) {
                        Some(&pv) if pv == v => matched += 1,
                        Some(_) => {
                            ok = false;
                            break;
                        }
                        None => {}
                    }
                }
                if ok && best.as_ref().is_none_or(|(m, _)| matched > *m) {
                    best = Some((matched, variant));
                }
            }
            out.extend(self.placement(best?.1));
        } else if let Some(parts) = &state.multipart {
            for part in parts {
                let applies = part
                    .when
                    .as_ref()
                    .is_none_or(|w| condition_holds(w, &props));
                if applies {
                    out.extend(self.placement(&part.apply));
                }
            }
        }
        (!out.is_empty()).then_some(out)
    }

    fn placement(&self, list: &VariantList) -> Option<Placement> {
        let v = match list {
            VariantList::One(v) => v,
            VariantList::Many(vs) => vs.first()?,
        };
        Some(Placement {
            model: self.model(&v.model)?,
            x: (v.x.rem_euclid(360) / 90) as u32,
            y: (v.y.rem_euclid(360) / 90) as u32,
            uvlock: v.uvlock,
        })
    }

    fn blockstate(&self, block: &str) -> Option<Arc<RawBlockstate>> {
        if let Some(c) = self.blockstates.lock().unwrap().get(block) {
            return c.clone();
        }
        let rel = format!("blockstates/{block}.json");
        let loaded = self
            .pack
            .read(&rel)
            .and_then(|bytes| match serde_json::from_slice(&bytes) {
                Ok(bs) => Some(Arc::new(bs)),
                Err(e) => {
                    eprintln!("WARN invalid blockstate {rel}: {e}");
                    None
                }
            });
        self.blockstates
            .lock()
            .unwrap()
            .insert(block.to_owned(), loaded.clone());
        loaded
    }

    fn model(&self, id: &str) -> Option<Arc<BlockModel>> {
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        if let Some(c) = self.models.lock().unwrap().get(id) {
            return c.clone();
        }
        let loaded = self.build_model(id).map(Arc::new);
        self.models
            .lock()
            .unwrap()
            .insert(id.to_owned(), loaded.clone());
        loaded
    }

    fn build_model(&self, id: &str) -> Option<BlockModel> {
        let mut textures: HashMap<String, String> = HashMap::new();
        let mut elements: Option<Vec<RawElement>> = None;
        let mut current = Some(id.to_owned());
        for _ in 0..16 {
            let Some(id) = current.take() else { break };
            let raw = self.raw_model(&id)?;
            if let Some(tex) = &raw.textures {
                for (k, v) in tex {
                    textures
                        .entry(k.clone())
                        .or_insert_with(|| v.as_str().to_owned());
                }
            }
            if elements.is_none() {
                elements = raw.elements.clone();
            }
            current = raw
                .parent
                .as_ref()
                .map(|p| p.strip_prefix("minecraft:").unwrap_or(p).to_owned());
        }
        let elements = elements?
            .iter()
            .map(|e| flatten_element(e, &textures))
            .collect();
        Some(BlockModel { elements })
    }

    fn raw_model(&self, id: &str) -> Option<RawModel> {
        let rel = format!("models/{id}.json");
        let bytes = self.pack.read(&rel)?;
        match serde_json::from_slice(&bytes) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("WARN invalid model {rel}: {e}");
                None
            }
        }
    }
}

fn flatten_element(e: &RawElement, textures: &HashMap<String, String>) -> Element {
    let mut faces: [Option<Face>; 6] = Default::default();
    for (name, rf) in &e.faces {
        let Some(dir) = Dir::from_name(name) else {
            continue;
        };
        faces[dir as usize] = Some(Face {
            texture: resolve_texture(&rf.texture, textures),
            uv: rf.uv,
            rotation: rf.rotation.rem_euclid(360),
            tintindex: rf.tintindex,
        });
    }
    Element {
        from: e.from,
        to: e.to,
        shade: e.shade,
        rotation: e.rotation.as_ref().map(|r| ElemRotation {
            origin: r.origin,
            axis: r.axis.chars().next().unwrap_or('y'),
            angle: r.angle,
            rescale: r.rescale,
        }),
        faces,
    }
}

fn resolve_texture<'a>(mut tex: &'a str, textures: &'a HashMap<String, String>) -> String {
    for _ in 0..16 {
        let Some(var) = tex.strip_prefix('#') else {
            let t = tex.strip_prefix("minecraft:").unwrap_or(tex);
            return t.strip_prefix("block/").unwrap_or(t).to_owned();
        };
        match textures.get(var) {
            Some(next) => tex = next,
            None => break,
        }
    }
    tex.to_owned()
}

fn condition_holds(when: &serde_json::Value, props: &HashMap<&str, &str>) -> bool {
    match when {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Array(alts)) = map.get("OR") {
                return alts.iter().any(|c| condition_holds(c, props));
            }
            if let Some(serde_json::Value::Array(all)) = map.get("AND") {
                return all.iter().all(|c| condition_holds(c, props));
            }
            map.iter().all(|(k, v)| {
                let want = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                props
                    .get(k.as_str())
                    .is_some_and(|pv| want.split('|').any(|alt| alt == *pv))
            })
        }
        _ => false,
    }
}

#[derive(Deserialize)]
struct RawBlockstate {
    variants: Option<BTreeMap<String, VariantList>>,
    multipart: Option<Vec<RawPart>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum VariantList {
    One(RawVariant),
    Many(Vec<RawVariant>),
}

#[derive(Deserialize)]
struct RawVariant {
    model: String,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    uvlock: bool,
}

#[derive(Deserialize)]
struct RawPart {
    apply: VariantList,
    when: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawModel {
    parent: Option<String>,
    textures: Option<HashMap<String, TextureRef>>,
    elements: Option<Vec<RawElement>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TextureRef {
    Plain(String),
    Sprite { sprite: String },
}

impl TextureRef {
    fn as_str(&self) -> &str {
        match self {
            TextureRef::Plain(s) | TextureRef::Sprite { sprite: s } => s,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize, Clone)]
struct RawElement {
    from: [f32; 3],
    to: [f32; 3],
    rotation: Option<RawRotation>,
    #[serde(default = "default_true")]
    shade: bool,
    #[serde(default)]
    faces: HashMap<String, RawFace>,
}

#[derive(Deserialize, Clone)]
struct RawRotation {
    origin: [f32; 3],
    axis: String,
    angle: f32,
    #[serde(default)]
    rescale: bool,
}

#[derive(Deserialize, Clone)]
struct RawFace {
    uv: Option<[f32; 4]>,
    texture: String,
    #[serde(default)]
    rotation: i32,
    tintindex: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> ModelStore {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mcassets/assets/minecraft");
        assert!(root.is_dir(), "mcassets missing -- extract the client jar");
        ModelStore::default_assets()
    }

    fn face(e: &Element, d: Dir) -> &Face {
        e.faces[d as usize].as_ref().expect("face present")
    }

    #[test]
    fn legacy_names_alias_to_modern_geometry() {
        let s = store();
        let planks = s.placements("minecraft:planks").unwrap();
        assert_eq!(
            face(&planks[0].model.elements[0], Dir::Up).texture,
            "oak_planks"
        );
        let chain = s.placements("minecraft:chain|axis=y").unwrap();
        assert!(!chain[0].model.elements.is_empty());
        let slab = s.placements("stone_wooden_slab|type=bottom").unwrap();
        assert_eq!(slab[0].model.elements[0].to[1], 8.0);
        assert!(s.placements("minecraft:bed").is_none());
    }

    #[test]
    fn sprite_object_texture_values_parse() {
        let glass = store().placements("minecraft:glass").unwrap();
        assert_eq!(face(&glass[0].model.elements[0], Dir::Up).texture, "glass");
    }

    #[test]
    fn stone_is_one_full_cube_with_stone_on_every_face() {
        let p = store().placements("minecraft:stone").unwrap();
        assert_eq!(p.len(), 1);
        let m = &p[0].model;
        assert_eq!(m.elements.len(), 1);
        let e = &m.elements[0];
        assert_eq!((e.from, e.to), ([0.0; 3], [16.0, 16.0, 16.0]));
        for d in Dir::ALL {
            assert_eq!(face(e, d).texture, "stone");
        }
        assert_eq!((p[0].x, p[0].y), (0, 0));
    }

    #[test]
    fn stairs_variant_matches_and_carries_rotation() {
        let s = store();
        let p = s
            .placements("minecraft:oak_stairs|facing=east,half=bottom,shape=straight")
            .unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].model.elements.len(), 2);
        assert_eq!((p[0].x, p[0].y), (0, 0));
        assert_eq!(face(&p[0].model.elements[0], Dir::Up).texture, "oak_planks");
        let west = s
            .placements("minecraft:oak_stairs|facing=west,half=bottom,shape=straight")
            .unwrap();
        assert_eq!(west[0].y, 2);
        let top = s
            .placements("minecraft:oak_stairs|facing=east,half=top,shape=straight")
            .unwrap();
        assert_eq!(top[0].x, 2);
    }

    #[test]
    fn partial_props_still_pick_a_best_effort_variant() {
        let p = store()
            .placements("minecraft:oak_stairs|facing=east")
            .unwrap();
        assert_eq!(p.len(), 1);
        assert!(!p[0].model.elements.is_empty());
    }

    #[test]
    fn slab_types_pick_bottom_and_double_models() {
        let s = store();
        let bottom = s.placements("minecraft:oak_slab|type=bottom").unwrap();
        assert_eq!(bottom[0].model.elements[0].to, [16.0, 8.0, 16.0]);
        let double = s.placements("minecraft:oak_slab|type=double").unwrap();
        assert_eq!(double[0].model.elements[0].to, [16.0, 16.0, 16.0]);
    }

    #[test]
    fn fence_multipart_adds_arms_per_connection() {
        let s = store();
        let post_only = s.placements("minecraft:oak_fence").unwrap();
        assert_eq!(post_only.len(), 1);
        let two = s
            .placements("minecraft:oak_fence|east=true,north=true,south=false,west=false")
            .unwrap();
        assert_eq!(two.len(), 3);
        assert_eq!(two[2].y, 1);
        assert!(two[1].uvlock);
    }

    #[test]
    fn grass_block_weighted_list_takes_first_and_snowy_swaps_model() {
        let s = store();
        let plain = s.placements("minecraft:grass_block").unwrap();
        assert_eq!(plain[0].y, 0);
        assert_eq!(
            face(&plain[0].model.elements[0], Dir::Up).texture,
            "grass_block_top"
        );
        let snowy = s.placements("minecraft:grass_block|snowy=true").unwrap();
        assert_eq!(
            face(&snowy[0].model.elements[0], Dir::North).texture,
            "grass_block_snow"
        );
    }

    #[test]
    fn cross_model_keeps_the_45_degree_rotation_and_tint() {
        let p = store().placements("minecraft:fern").unwrap();
        let e = &p[0].model.elements[0];
        let rot = e.rotation.expect("cross elements are rotated");
        assert_eq!(rot.axis, 'y');
        assert_eq!(rot.angle, 45.0);
        assert!(rot.rescale);
        let f = face(e, Dir::North);
        assert_eq!(f.texture, "fern");
        assert_eq!(f.tintindex, Some(0));
    }

    #[test]
    fn synthesized_connection_props_match_multipart_geometry() {
        let s = store();
        let run = s
            .placements(
                "minecraft:cobblestone_wall|east=none,north=low,south=low,up=false,west=none",
            )
            .unwrap();
        assert_eq!(run.len(), 2);
        let post = s
            .placements(
                "minecraft:cobblestone_wall|east=none,north=none,south=none,up=true,west=none",
            )
            .unwrap();
        assert_eq!(post.len(), 1);
        let fence = s
            .placements("minecraft:fence|east=true,north=false,south=false,west=false")
            .unwrap();
        assert_eq!(fence.len(), 2);
        assert!(s
            .placements(
                "minecraft:stained_glass_pane|east=false,north=false,south=false,west=false"
            )
            .is_some());
    }

    #[test]
    fn legacy_slab_names_alias_to_modern_blockstates() {
        let s = store();
        let brick = s.placements("minecraft:bricks_slab|type=bottom").unwrap();
        assert_eq!(brick[0].model.elements[0].to[1], 8.0);
        assert_eq!(face(&brick[0].model.elements[0], Dir::Up).texture, "bricks");
        let stone = s.placements("minecraft:stone_slab|type=bottom").unwrap();
        assert_eq!(
            face(&stone[0].model.elements[0], Dir::North).texture,
            "smooth_stone_slab_side"
        );
    }

    #[test]
    fn decoded_button_mounts_pick_distinct_variant_rotations() {
        let s = store();
        let wall = s
            .placements("minecraft:stone_button|face=wall,facing=south")
            .unwrap();
        assert_eq!((wall[0].x, wall[0].y), (1, 2));
        let floor = s
            .placements("minecraft:oak_button|face=floor,facing=north")
            .unwrap();
        assert_eq!((floor[0].x, floor[0].y), (0, 0));
    }

    #[test]
    fn decoded_orientation_props_pick_distinct_variants() {
        let s = store();
        let wall = s
            .placements("minecraft:lever|face=wall,facing=south,powered=false")
            .unwrap();
        assert_eq!((wall[0].x, wall[0].y), (1, 2));
        let floor = s
            .placements("minecraft:lever|face=floor,facing=north,powered=false")
            .unwrap();
        assert_eq!((floor[0].x, floor[0].y), (0, 0));
        let lower = s
            .placements("minecraft:oak_door|facing=west,half=lower,hinge=left,open=false")
            .unwrap();
        let upper = s
            .placements("minecraft:oak_door|facing=west,half=upper,hinge=left,open=false")
            .unwrap();
        assert_eq!(lower[0].y, 2);
        assert_eq!(upper[0].y, 2);
        assert!(!Arc::ptr_eq(&lower[0].model, &upper[0].model));
        assert!(s
            .placements(
                "minecraft:brown_mushroom_block|down=false,east=true,north=true,south=false,up=true,west=false"
            )
            .is_some());
        assert!(s
            .placements("minecraft:vine|east=false,north=true,south=false,up=false,west=false")
            .is_some());
        assert!(s
            .placements("minecraft:redstone_wire|east=side,north=none,south=none,west=side")
            .is_some());
    }

    #[test]
    fn missing_blockstate_returns_none() {
        assert!(store().placements("minecraft:not_a_block").is_none());
    }
}
