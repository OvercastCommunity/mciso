use std::borrow::Cow;

pub fn strip_ns(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

pub fn split_key(key: &str) -> (&str, &str) {
    key.split_once('|').unwrap_or((key, ""))
}

pub fn props(props_str: &str) -> impl Iterator<Item = (&str, &str)> + '_ {
    props_str.split(',').filter_map(|kv| kv.split_once('='))
}

pub fn is_air(name: &str) -> bool {
    matches!(
        strip_ns(name),
        "air"
            | "cave_air"
            | "void_air"
            | "structure_void"
            | "light"
            | "barrier"
            | "moving_piston"
            | "piston_extension"
    )
}

const PROP_DENYLIST: &[&str] = &[
    "distance",
    "persistent",
    "waterlogged",
    "instrument",
    "note",
    "triggered",
    "enabled",
    "unstable",
    "occupied",
    "stage",
    "locked",
    "has_record",
    "has_book",
];

#[derive(Default)]
pub struct Props {
    pub(crate) axis: Option<char>,
    pub(crate) snowy: bool,
    pub(crate) lit: bool,
    pub(crate) facing: Option<(i32, i32)>,
    pub(crate) rotation: Option<u32>,
    pub(crate) part_foot: bool,
}

impl Props {
    pub(crate) fn parse(s: &str) -> Props {
        let mut p = Props::default();
        for (k, v) in props(s) {
            match (k, v) {
                ("axis", "x" | "z") => p.axis = v.chars().next(),
                ("snowy", "true") => p.snowy = true,
                ("lit", "true") => p.lit = true,
                ("rotation", _) => p.rotation = v.parse().ok(),
                ("part", "foot") => p.part_foot = true,
                ("facing", _) => {
                    p.facing = match v {
                        "north" => Some((0, -1)),
                        "south" => Some((0, 1)),
                        "west" => Some((-1, 0)),
                        "east" => Some((1, 0)),
                        _ => None,
                    }
                }
                _ => {}
            }
        }
        p
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Connectable {
    Wall,
    Fence,
    Pane,
    Wire,
}

pub(crate) fn connectable(name: &str) -> Option<Connectable> {
    let n = strip_ns(name);
    if n.ends_with("_wall") {
        Some(Connectable::Wall)
    } else if n == "fence" || n.ends_with("_fence") {
        Some(Connectable::Fence)
    } else if n.ends_with("_pane") || matches!(n, "iron_bars" | "bars") {
        Some(Connectable::Pane)
    } else if n == "redstone_wire" {
        Some(Connectable::Wire)
    } else {
        None
    }
}

pub(crate) fn connects(kind: Connectable, neighbor: &str, neighbor_occludes: bool) -> bool {
    let n = strip_ns(neighbor);
    if kind == Connectable::Wire {
        return matches!(
            n,
            "redstone_wire"
                | "repeater"
                | "comparator"
                | "redstone_torch"
                | "redstone_wall_torch"
                | "redstone_block"
                | "lever"
                | "target"
                | "daylight_detector"
        );
    }
    if neighbor_occludes {
        return true;
    }
    match connectable(n) {
        Some(Connectable::Wire) => false,
        Some(k) => (k == Connectable::Fence) == (kind == Connectable::Fence),
        None => kind != Connectable::Pane && (n == "fence_gate" || n.ends_with("_fence_gate")),
    }
}

fn connection_props(kind: Connectable, n: bool, s: bool, w: bool, e: bool) -> String {
    let tf = |b: bool| if b { "true" } else { "false" };
    match kind {
        Connectable::Wall => {
            let ln = |b: bool| if b { "low" } else { "none" };
            let up = !((n && s && !e && !w) || (e && w && !n && !s));
            format!(
                "east={},north={},south={},up={},west={}",
                ln(e),
                ln(n),
                ln(s),
                tf(up),
                ln(w)
            )
        }
        Connectable::Wire => {
            let sn = |b: bool| if b { "side" } else { "none" };
            format!(
                "east={},north={},south={},west={}",
                sn(e),
                sn(n),
                sn(s),
                sn(w)
            )
        }
        _ => format!(
            "east={},north={},south={},west={}",
            tf(e),
            tf(n),
            tf(s),
            tf(w)
        ),
    }
}

const DOUBLE_PLANTS: &[&str] = &[
    "sunflower",
    "lilac",
    "tall_grass",
    "large_fern",
    "rose_bush",
    "peony",
];

fn merge_upper_half(
    name: &str,
    encoded: &str,
    below: impl FnOnce() -> Option<(String, String)>,
) -> Option<String> {
    if !encoded.contains("half=upper") {
        return None;
    }
    let n = strip_ns(name);
    let is_door = n.ends_with("_door") && !encoded.contains("facing=");
    let is_plant = DOUBLE_PLANTS.contains(&n);
    if !is_door && !is_plant {
        return None;
    }
    let (bname, benc) = below()?;
    if is_door {
        if bname != name {
            return None;
        }
        let bprops = split_key(&benc).1;
        let facing = bprops.split(',').find(|p| p.starts_with("facing="))?;
        let open = bprops.split(',').find(|p| p.starts_with("open="))?;
        let hinge = split_key(encoded)
            .1
            .split(',')
            .find(|p| p.starts_with("hinge="))
            .unwrap_or("hinge=left");
        Some(format!("{name}|{facing},half=upper,{hinge},{open}"))
    } else {
        let bn = strip_ns(&bname);
        DOUBLE_PLANTS
            .contains(&bn)
            .then(|| format!("{bname}|half=upper"))
    }
}

pub(crate) fn modernize(name: &str) -> (Cow<'_, str>, Option<&'static str>) {
    let stripped = strip_ns(name);
    let extra = match stripped {
        "lit_redstone_lamp" | "lit_redstone_ore" => Some("lit=true"),
        "unlit_redstone_torch" => Some("lit=false"),
        _ => None,
    };
    let modern = legacy_alias(stripped);
    if modern == stripped {
        (Cow::Borrowed(name), extra)
    } else if name.len() == stripped.len() {
        (Cow::Borrowed(modern), extra)
    } else {
        (Cow::Owned(format!("minecraft:{modern}")), extra)
    }
}

pub(crate) fn legacy_alias(block: &str) -> &str {
    match block {
        "brick_block" => "bricks",
        "bricks_slab" => "brick_slab",
        "chain" => "iron_chain",
        "concrete" => "white_concrete",
        "concrete_powder" => "white_concrete_powder",
        "daylight_detector_inverted" => "daylight_detector",
        "end_bricks" => "end_stone_bricks",
        "grass_path" => "dirt_path",
        "lit_redstone_lamp" => "redstone_lamp",
        "magma" => "magma_block",
        "nether_brick" => "nether_bricks",
        "purpur_double_slab" => "purpur_slab",
        "red_nether_brick" => "red_nether_bricks",
        "reeds" => "sugar_cane",
        "silver_glazed_terracotta" => "light_gray_glazed_terracotta",
        "silver_shulker_box" => "light_gray_shulker_box",
        "stonebrick" => "stone_bricks",
        "web" => "cobweb",
        "deadbush" => "dead_bush",
        "fence" => "oak_fence",
        "fence_gate" => "oak_fence_gate",
        "stained_glass_pane" => "white_stained_glass_pane",
        "golden_rail" => "powered_rail",
        "lit_redstone_ore" => "redstone_ore",
        "melon_block" => "melon",
        "mob_spawner" => "spawner",
        "monster_egg" => "infested_stone",
        "noteblock" => "note_block",
        "planks" => "oak_planks",
        "portal" => "nether_portal",
        "powered_comparator" | "unpowered_comparator" => "comparator",
        "unpowered_repeater" | "powered_repeater" => "repeater",
        "quartz_ore" => "nether_quartz_ore",
        "unlit_redstone_torch" => "redstone_torch",
        "slime" => "slime_block",
        "stone_slab" => "smooth_stone_slab",
        "stone_wooden_slab" | "wooden_slab" => "oak_slab",
        "wooden_door" => "oak_door",
        "wooden_pressure_plate" => "oak_pressure_plate",
        n => n,
    }
}

pub(crate) struct StateParts<'a> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) encoded: Cow<'a, str>,
    pub(crate) extra: Option<&'static str>,
}

pub(crate) fn state_parts<'a>(
    name: &'a str,
    encoded: &'a str,
    below: impl FnOnce() -> Option<(String, String)>,
) -> StateParts<'a> {
    let (name, extra) = modernize(name);
    match merge_upper_half(&name, encoded, below) {
        Some(m) => {
            let n = split_key(&m).0.to_owned();
            StateParts {
                name: n.into(),
                encoded: m.into(),
                extra,
            }
        }
        None => StateParts {
            name,
            encoded: Cow::Borrowed(encoded),
            extra,
        },
    }
}

pub(crate) fn plain_key(name: &str, encoded: &str, extra: Option<&str>) -> String {
    let mut key = state_key(name, encoded);
    if let Some(extra) = extra {
        key.push(if key.contains('|') { ',' } else { '|' });
        key.push_str(extra);
    }
    key
}

pub(crate) fn connected_key(
    p: &StateParts,
    kind: Connectable,
    conn: impl Fn(Connectable, i32, i32) -> bool,
) -> String {
    let props = connection_props(
        kind,
        conn(kind, 0, -1),
        conn(kind, 0, 1),
        conn(kind, -1, 0),
        conn(kind, 1, 0),
    );
    let mut key = plain_key(&p.name, &p.encoded, p.extra);
    key.push(if key.contains('|') { ',' } else { '|' });
    key.push_str(&props);
    key
}

fn state_key(name: &str, encoded: &str) -> String {
    let mut key = name.to_owned();
    let mut sep = '|';
    for (k, v) in props(split_key(encoded).1) {
        if PROP_DENYLIST.contains(&k) {
            continue;
        }
        key.push(sep);
        key.push_str(k);
        key.push('=');
        key.push_str(v);
        sep = ',';
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_split_and_prop_parse_helpers() {
        assert_eq!(split_key("minecraft:stone"), ("minecraft:stone", ""));
        assert_eq!(
            split_key("minecraft:oak_log|axis=x"),
            ("minecraft:oak_log", "axis=x")
        );
        assert_eq!(strip_ns("minecraft:stone"), "stone");
        assert_eq!(strip_ns("stone"), "stone");
        assert_eq!(
            props("axis=x,snowy=true").collect::<Vec<_>>(),
            [("axis", "x"), ("snowy", "true")]
        );
        assert_eq!(props("").count(), 0);
    }

    #[test]
    fn air_detected_with_and_without_namespace() {
        assert!(is_air("minecraft:air"));
        assert!(is_air("cave_air"));
        assert!(!is_air("minecraft:stone"));
    }

    #[test]
    fn state_key_keeps_render_relevant_properties_only() {
        assert_eq!(
            state_key("minecraft:stone", "minecraft:stone|"),
            "minecraft:stone"
        );
        assert_eq!(
            state_key("minecraft:oak_log", "minecraft:oak_log|axis=x"),
            "minecraft:oak_log|axis=x"
        );
        assert_eq!(
            state_key("minecraft:oak_log", "minecraft:oak_log|axis=y"),
            "minecraft:oak_log|axis=y"
        );
        assert_eq!(
            state_key("minecraft:grass_block", "minecraft:grass_block|snowy=false"),
            "minecraft:grass_block|snowy=false"
        );
        assert_eq!(
            state_key("minecraft:oak_sign", "minecraft:oak_sign|rotation=8"),
            "minecraft:oak_sign|rotation=8"
        );
        assert_eq!(
            state_key("minecraft:grass_block", "minecraft:grass_block|snowy=true"),
            "minecraft:grass_block|snowy=true"
        );
        assert_eq!(
            state_key(
                "minecraft:furnace",
                "minecraft:furnace|facing=north,lit=false"
            ),
            "minecraft:furnace|facing=north,lit=false"
        );
        assert_eq!(
            state_key(
                "minecraft:furnace",
                "minecraft:furnace|facing=north,lit=true"
            ),
            "minecraft:furnace|facing=north,lit=true"
        );
        assert_eq!(
            state_key(
                "minecraft:oak_leaves",
                "minecraft:oak_leaves|distance=1,persistent=true"
            ),
            "minecraft:oak_leaves"
        );
    }

    #[test]
    fn connectable_families_and_join_rules() {
        assert!(matches!(
            connectable("minecraft:cobblestone_wall"),
            Some(Connectable::Wall)
        ));
        assert!(matches!(
            connectable("minecraft:fence"),
            Some(Connectable::Fence)
        ));
        assert!(matches!(
            connectable("minecraft:nether_brick_fence"),
            Some(Connectable::Fence)
        ));
        assert!(matches!(
            connectable("minecraft:stained_glass_pane"),
            Some(Connectable::Pane)
        ));
        assert!(matches!(
            connectable("minecraft:iron_bars"),
            Some(Connectable::Pane)
        ));
        assert!(connectable("minecraft:fence_gate").is_none());
        assert!(connectable("minecraft:stone").is_none());

        assert!(connects(Connectable::Wall, "minecraft:stone", true));
        assert!(connects(Connectable::Wall, "minecraft:iron_bars", false));
        assert!(connects(
            Connectable::Pane,
            "minecraft:andesite_wall",
            false
        ));
        assert!(!connects(Connectable::Wall, "minecraft:fence", false));
        assert!(connects(Connectable::Fence, "minecraft:oak_fence", false));
        assert!(connects(Connectable::Fence, "minecraft:fence_gate", false));
        assert!(!connects(
            Connectable::Fence,
            "minecraft:cobblestone_wall",
            false
        ));
        assert!(!connects(Connectable::Pane, "minecraft:torch", false));
    }

    #[test]
    fn upper_halves_inherit_facing_and_species_from_below() {
        assert_eq!(
            merge_upper_half(
                "minecraft:oak_door",
                "oak_door|half=upper,hinge=right",
                || Some((
                    "minecraft:oak_door".to_owned(),
                    "oak_door|facing=west,half=lower,open=true".to_owned()
                ))
            )
            .unwrap(),
            "minecraft:oak_door|facing=west,half=upper,hinge=right,open=true"
        );
        assert!(merge_upper_half(
            "minecraft:oak_door",
            "oak_door|facing=west,half=upper,hinge=left,open=false",
            || panic!("should not look below")
        )
        .is_none());
        assert_eq!(
            merge_upper_half("minecraft:sunflower", "sunflower|half=upper", || Some((
                "minecraft:rose_bush".to_owned(),
                "rose_bush|half=lower".to_owned()
            )))
            .unwrap(),
            "minecraft:rose_bush|half=upper"
        );
        assert!(merge_upper_half(
            "minecraft:oak_door",
            "oak_door|half=upper,hinge=left",
            || Some(("minecraft:stone".to_owned(), "stone|".to_owned()))
        )
        .is_none());
    }

    #[test]
    fn redstone_wire_connects_to_components_only() {
        assert!(matches!(
            connectable("minecraft:redstone_wire"),
            Some(Connectable::Wire)
        ));
        assert!(connects(
            Connectable::Wire,
            "minecraft:redstone_wire",
            false
        ));
        assert!(connects(Connectable::Wire, "minecraft:repeater", false));
        assert!(!connects(Connectable::Wire, "minecraft:stone", true));
        assert!(!connects(
            Connectable::Wall,
            "minecraft:redstone_wire",
            false
        ));
        assert_eq!(
            connection_props(Connectable::Wire, true, false, false, true),
            "east=side,north=side,south=none,west=none"
        );
    }

    #[test]
    fn connection_props_wall_post_rule_and_fence_bools() {
        assert_eq!(
            connection_props(Connectable::Wall, false, false, false, false),
            "east=none,north=none,south=none,up=true,west=none"
        );
        assert_eq!(
            connection_props(Connectable::Wall, true, true, false, false),
            "east=none,north=low,south=low,up=false,west=none"
        );
        assert_eq!(
            connection_props(Connectable::Wall, false, false, true, true),
            "east=low,north=none,south=none,up=false,west=low"
        );
        assert_eq!(
            connection_props(Connectable::Wall, true, false, true, false),
            "east=none,north=low,south=none,up=true,west=low"
        );
        assert_eq!(
            connection_props(Connectable::Fence, true, false, false, true),
            "east=true,north=true,south=false,west=false"
        );
    }

    #[test]
    fn legacy_ids_modernize_at_intern_time() {
        assert_eq!(modernize("minecraft:planks").0, "minecraft:oak_planks");
        assert_eq!(modernize("fence").0, "oak_fence");
        let (name, extra) = modernize("minecraft:stone");
        assert!(matches!(name, Cow::Borrowed("minecraft:stone")));
        assert_eq!(extra, None);
        let (name, extra) = modernize("minecraft:lit_redstone_lamp");
        assert_eq!(name, "minecraft:redstone_lamp");
        assert_eq!(extra, Some("lit=true"));
        assert_eq!(
            modernize("minecraft:unlit_redstone_torch").1,
            Some("lit=false")
        );
    }

    #[test]
    fn plain_key_appends_implied_properties() {
        assert_eq!(
            plain_key(
                "minecraft:redstone_lamp",
                "lit_redstone_lamp|",
                Some("lit=true")
            ),
            "minecraft:redstone_lamp|lit=true"
        );
        assert_eq!(
            plain_key("minecraft:oak_log", "minecraft:oak_log|axis=x", None),
            "minecraft:oak_log|axis=x"
        );
    }

    #[test]
    fn props_struct_parses_render_props() {
        let p = Props::parse("axis=x,snowy=true,lit=true,facing=west");
        assert_eq!(p.axis, Some('x'));
        assert!(p.snowy && p.lit);
        assert_eq!(p.facing, Some((-1, 0)));
        let p = Props::parse("rotation=8,part=foot");
        assert_eq!(p.rotation, Some(8));
        assert!(p.part_foot);
        let p = Props::parse("");
        assert!(p.axis.is_none() && p.facing.is_none() && !p.snowy);
    }
}
