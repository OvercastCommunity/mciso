use crate::{Block, BlockArchetype};

macro_rules! coloured_block {
    ($a:expr, $b:expr) => {{
        let col = match $b & 0b1111 {
            0 => "white",
            1 => "orange",
            2 => "magenta",
            3 => "light_blue",
            4 => "yellow",
            5 => "lime",
            6 => "pink",
            7 => "gray",
            8 => "light_gray",
            9 => "cyan",
            10 => "purple",
            11 => "blue",
            12 => "brown",
            13 => "green",
            14 => "red",
            15 => "black",
            _ => "unknown",
        };
        Block {
            name: format!("minecraft:{col}_{}", $a),
            encoded: format!("minecraft:{col}_{}|", $a),
            archetype: BlockArchetype::Normal,
        }
    }};
}

/// Initialize a `Block` from the given `block_id` and `data_value`.
pub fn init_default_block(block_id: u16, data_value: u8) -> Block {
    assert!(
        block_id < 256,
        "init_default_block function only supports block ids in the 0..=255 range"
    );
    let block_name = block_name(block_id as u8);

    modern_block(block_name, data_value)
}

fn modern_block(block_name: &'static str, data_value: u8) -> Block {
    let encoded = format!("{}|", block_name);
    let ns = |s| format!("minecraft:{s}"); // add namespace
    let enc0 = |s| format!("minecraft:{s}|");

    // This function will get very large and complicated, need some way to break
    // it down. Could definitely use some macros for things like the wood/leaf types.

    match block_name {
        "double_wooden_slab" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "oak",
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                4 => "acacia",
                5 => "dark_oak",
                6 | 7 => "invalid_double_wooden_slab",
                _ => "unknown",
            };
            Block {
                name: format!("{kind}_slab"),
                encoded: format!("{kind}_slab|type=double"),
                archetype: BlockArchetype::Normal,
            }
        }
        "wooden_slab" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "oak",
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                4 => "acacia",
                5 => "dark_oak",
                6 | 7 => "invalid_wooden_slab",
                _ => "unknown",
            };
            let top = (data_value & 0b1000) >> 3;
            let top = match top {
                0 => "bottom",
                1 => "top",
                _ => "unknown",
            };
            Block {
                name: format!("{kind}_slab"),
                encoded: format!("{kind}_slab|type={top}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: block_name() emits red_flower/yellow_flower, so the old
        // "flower" arm was dead code and every flower collapsed to a poppy.
        "yellow_flower" => Block {
            name: ns("dandelion"),
            encoded: enc0("dandelion"),
            archetype: BlockArchetype::Normal,
        },
        "red_flower" => {
            let kind = data_value & 0b1111;
            let kind = match kind {
                0 => "poppy",
                1 => "blue_orchid",
                2 => "allium",
                3 => "azure_bluet",
                4 => "red_tulip",
                5 => "orange_tulip",
                6 => "white_tulip",
                7 => "pink_tulip",
                8 => "oxeye_daisy",
                9..=15 => "invalid_flower",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "red_sandstone" => {
            let kind = data_value & 0b0011;
            let kind = match kind {
                0 => "red_sandstone",
                1 => "chiseled_red_sandstone",
                2 => "smooth_red_sandstone",
                3 => "invalid_red_sandstone",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "sandstone" => {
            let kind = data_value & 0b0011;
            let kind = match kind {
                0 => "sandstone",
                1 => "chiseled_sandstone",
                2 => "smooth_sandstone",
                3 => "invalid_sandstone",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "double_stone_slab" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "stone",
                1 => "sandstone",
                2 => "stone_wooden",
                3 => "cobblestone",
                4 => "bricks",
                5 => "stone_brick",
                6 => "nether_brick",
                7 => "quartz",
                _ => "unknown",
            };
            Block {
                name: format!("{kind}_slab"),
                encoded: format!("{kind}_slab|type=double"),
                archetype: BlockArchetype::Normal,
            }
        }
        "stone_slab" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "stone",
                1 => "sandstone",
                2 => "stone_wooden",
                3 => "cobblestone",
                4 => "bricks",
                5 => "stone_brick",
                6 => "nether_brick",
                7 => "quartz",
                _ => "unknown",
            };
            let top = (data_value & 0b1000) >> 3;
            let top = match top {
                0 => "bottom",
                1 => "top",
                _ => "unknown",
            };
            Block {
                name: format!("{kind}_slab"),
                encoded: format!("{kind}_slab|type={top}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "double_stone_slab2" => Block {
            name: ns("red_sandstone_slab"),
            encoded: enc0("red_sandstone_slab|type=double"),
            archetype: BlockArchetype::Normal,
        },
        "stone_slab2" => {
            let top = (data_value & 0b1000) >> 3;
            // mciso patch: upstream matched the pre-shift value 8 here, so top
            // slabs decoded as type=unknown
            let top = match top {
                0 => "bottom",
                1 => "top",
                _ => "unknown",
            };
            Block {
                name: ns("red_sandstone_slab"),
                encoded: format!("red_sandstone_slab|type={top}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "stained_glass" => {
            coloured_block!("stained_glass", data_value)
        }
        "wool" => {
            coloured_block!("wool", data_value)
        }
        "carpet" => {
            coloured_block!("carpet", data_value)
        }
        "sand" => {
            let kind = data_value & 0b0001;
            let kind = match kind {
                0 => "sand",
                1 => "red_sand",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "sapling" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "oak_sapling",
                1 => "spruce_sapling",
                2 => "birch_sapling",
                3 => "jungle_sapling",
                4 => "acacia_sapling",
                5 => "dark_oak_sapling",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "dirt" => {
            let kind = data_value & 0b0011;
            let kind = match kind {
                0 => "dirt",
                1 => "coarse_dirt",
                2 => "podzol",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "stone" => {
            let kind = data_value & 0b0111;
            let kind = match kind {
                0 => "stone",
                1 => "granite",
                2 => "polished_granite",
                3 => "diorite",
                4 => "polished_diorite",
                5 => "andesite",
                6 => "polished_andesite",
                _ => "unknown",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "leaves" => {
            let leaf = data_value & 0b0011;
            let leaf = match leaf {
                0 => "oak_leaves",
                1 => "spruce_leaves",
                2 => "birch_leaves",
                3 => "jungle_leaves",
                _ => "unknown",
            };
            Block {
                name: ns(leaf),
                encoded: enc0(leaf),
                archetype: BlockArchetype::Normal,
            }
        }
        "leaves2" => {
            let leaf = data_value & 0b0011;
            let leaf = match leaf {
                0 => "acacia_leaves",
                1 => "dark_oak_leaves",
                2 | 3 => "invalid_leaves",
                _ => "unknown",
            };
            Block {
                name: ns(leaf),
                encoded: enc0(leaf),
                archetype: BlockArchetype::Normal,
            }
        }
        "log" => {
            let log = data_value & 0b0011;
            let axis = (data_value & 0b1100) >> 2;
            let axis = match axis {
                0 => "y",
                1 => "x",
                2 => "z",
                3 => "z", // this actually represents all bark.
                _ => "unknown",
            };
            let log = match log {
                0 => "oak_log",
                1 => "spruce_log",
                2 => "birch_log",
                3 => "jungle_log",
                _ => "unknown",
            };
            Block {
                name: ns(log),
                encoded: format!("minecraft:{log}|axis={axis}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "log2" => {
            let log = data_value & 0b0011;
            let axis = (data_value & 0b1100) >> 2;
            let axis = match axis {
                0 => "y",
                1 => "x",
                2 => "z",
                3 => "z", // this actually represents all bark.
                _ => "unknown",
            };
            let log = match log {
                0 => "acacia_log",
                1 => "dark_oak_log",
                2 | 3 => "invalid_log",
                _ => "unknown",
            };
            Block {
                name: ns(log),
                encoded: format!("minecraft:{log}|axis={axis}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "snow_layer" => {
            let layers = (data_value & 0b0111) + 1;
            Block {
                name: ns("snow"),
                encoded: format!("minecraft:snow|layers={layers}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "stained_hardened_clay" => {
            let col = match data_value & 0b1111 {
                0 => "white",
                1 => "orange",
                2 => "magenta",
                3 => "light_blue",
                4 => "yellow",
                5 => "lime",
                6 => "pink",
                7 => "gray",
                8 => "light_gray",
                9 => "cyan",
                10 => "purple",
                11 => "blue",
                12 => "brown",
                13 => "green",
                14 => "red",
                15 => "black",
                _ => "unknown",
            };
            Block {
                name: format!("minecraft:{col}_terracotta"),
                encoded: format!("minecraft:{col}_terracotta|"),
                archetype: BlockArchetype::Normal,
            }
        }
        "hardened_clay" => Block {
            name: ns("terracotta"),
            encoded,
            archetype: BlockArchetype::Normal,
        },
        // mciso patch: 1-high tallgrass data (0=dead shrub, 1=grass, 2=fern);
        // modern "tall_grass" is the 2-high plant, the short one is short_grass.
        "tallgrass" => {
            let kind = match data_value & 0b0011 {
                0 => "dead_bush",
                2 => "fern",
                _ => "short_grass",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "waterlily" => Block {
            name: ns("lily_pad"),
            encoded,
            archetype: BlockArchetype::Normal,
        },
        // mciso patch: facing metadata (2=north, 3=south, 4=west, 5=east) for
        // wall-attached / fronted blocks.
        // mciso patch: lit furnaces are a separate legacy id but a `lit` prop
        // on the one modern furnace blockstate.
        "lit_furnace" => {
            let facing = match data_value & 0b0111 {
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            Block {
                name: ns("furnace"),
                encoded: format!("furnace|facing={facing},lit=true"),
                archetype: BlockArchetype::Normal,
            }
        }
        "chest" | "trapped_chest" | "ender_chest" | "furnace" | "ladder" => {
            let facing = match data_value & 0b0111 {
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            Block {
                name: ns(block_name),
                encoded: format!("{block_name}|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: button mount metadata (0=ceiling, 1-4=wall pointing
        // east/west/south/north, 5=floor); without it the best-effort variant
        // match renders every button on the ceiling.
        "stone_button" | "oak_button" => {
            let (face, facing) = match data_value & 0b0111 {
                0 => ("ceiling", "north"),
                1 => ("wall", "east"),
                2 => ("wall", "west"),
                3 => ("wall", "south"),
                4 => ("wall", "north"),
                _ => ("floor", "north"),
            };
            Block {
                name: ns(block_name),
                encoded: format!("{block_name}|face={face},facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: signs (all oak pre-1.13); standing signs store
        // rotation 0-15 clockwise from south, wall signs the 2-5 facing.
        "standing_sign" => Block {
            name: ns("oak_sign"),
            encoded: format!("oak_sign|rotation={}", data_value & 0b1111),
            archetype: BlockArchetype::Normal,
        },
        "wall_sign" => {
            let facing = match data_value & 0b0111 {
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            Block {
                name: ns("oak_wall_sign"),
                encoded: format!("oak_wall_sign|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: beds (always red pre-1.12; the color lived in the tile
        // entity through 1.12): bits 0-1 = head direction, bit 8 = head half.
        "bed" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                3 => "east",
                _ => "north",
            };
            let part = if data_value & 0b1000 != 0 {
                "head"
            } else {
                "foot"
            };
            Block {
                name: ns("red_bed"),
                encoded: format!("red_bed|facing={facing},part={part}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: banner color lived in the tile entity pre-1.13, so
        // white is the best static guess; same rotation/facing split as signs.
        "standing_banner" => Block {
            name: ns("white_banner"),
            encoded: format!("white_banner|rotation={}", data_value & 0b1111),
            archetype: BlockArchetype::Normal,
        },
        "wall_banner" => {
            let facing = match data_value & 0b0111 {
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            Block {
                name: ns("white_wall_banner"),
                encoded: format!("white_wall_banner|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: dispenser/dropper/command blocks add down/up to the
        // facing values.
        "dispenser" | "dropper" | "command_block" | "repeating_command_block"
        | "chain_command_block" => {
            let facing = match data_value & 0b0111 {
                0 => "down",
                1 => "up",
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "north",
            };
            Block {
                name: ns(block_name),
                encoded: format!("{block_name}|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: lever EnumOrientation (0/7 ceiling, 1-4 wall pointing
        // east/west/south/north, 5/6 floor), bit 8 = powered; without it every
        // lever hung from the ceiling (same failure as the buttons).
        "lever" => {
            let (face, facing) = match data_value & 0b0111 {
                0 => ("ceiling", "east"),
                1 => ("wall", "east"),
                2 => ("wall", "west"),
                3 => ("wall", "south"),
                4 => ("wall", "north"),
                5 => ("floor", "north"),
                6 => ("floor", "east"),
                _ => ("ceiling", "south"),
            };
            let powered = data_value & 0b1000 != 0;
            Block {
                name: ns("lever"),
                encoded: format!("lever|face={face},facing={facing},powered={powered}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: trapdoor bits 0-1 = facing north/south/west/east,
        // bit 4 = open, bit 8 = top half.
        "trapdoor" | "iron_trapdoor" => {
            let name = if block_name == "trapdoor" {
                "oak_trapdoor"
            } else {
                block_name
            };
            let facing = match data_value & 0b0011 {
                0 => "north",
                1 => "south",
                2 => "west",
                _ => "east",
            };
            let half = if data_value & 0b1000 != 0 {
                "top"
            } else {
                "bottom"
            };
            let open = data_value & 0b0100 != 0;
            Block {
                name: ns(name),
                encoded: format!("{name}|facing={facing},half={half},open={open}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: door lower halves store facing (0=east,1=south,2=west,
        // 3=north — byHorizontalIndex rotated CCW) and open bit 4; upper halves
        // (bit 8) only the hinge — the surface pass copies facing/open up from
        // the block below (world.rs).
        n if n == "wooden_door" || n == "iron_door" || n.ends_with("_door") => {
            let name = if n == "wooden_door" { "oak_door" } else { n };
            let encoded = if data_value & 0b1000 != 0 {
                let hinge = if data_value & 0b0001 != 0 {
                    "right"
                } else {
                    "left"
                };
                format!("{name}|half=upper,hinge={hinge}")
            } else {
                let facing = match data_value & 0b0011 {
                    0 => "east",
                    1 => "south",
                    2 => "west",
                    _ => "north",
                };
                let open = data_value & 0b0100 != 0;
                format!("{name}|facing={facing},half=lower,open={open}")
            };
            Block {
                name: ns(name),
                encoded,
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: fence gates use byHorizontalIndex facing
        // (0=south,1=west,2=north,3=east) plus open bit 4.
        n if n == "fence_gate" || n.ends_with("_fence_gate") => {
            let name = if n == "fence_gate" { "oak_fence_gate" } else { n };
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let open = data_value & 0b0100 != 0;
            Block {
                name: ns(name),
                encoded: format!("{name}|facing={facing},in_wall=false,open={open}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: piston facing 0=down,1=up,2=north,3=south,4=west,5=east;
        // bit 8 = extended (base) or sticky (head).
        "sticky_piston" | "piston" | "piston_head" => {
            let facing = match data_value & 0b0111 {
                0 => "down",
                1 => "up",
                2 => "north",
                3 => "south",
                4 => "west",
                _ => "east",
            };
            let encoded = if block_name == "piston_head" {
                let kind = if data_value & 0b1000 != 0 {
                    "sticky"
                } else {
                    "normal"
                };
                format!("piston_head|facing={facing},short=false,type={kind}")
            } else {
                let extended = data_value & 0b1000 != 0;
                format!("{block_name}|extended={extended},facing={facing}")
            };
            Block {
                name: ns(block_name),
                encoded,
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: anvil byHorizontalIndex facing bits 0-1, damage bits 2-3.
        "anvil" => {
            let name = match (data_value & 0b1100) >> 2 {
                1 => "chipped_anvil",
                2 => "damaged_anvil",
                _ => "anvil",
            };
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            Block {
                name: ns(name),
                encoded: format!("{name}|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: plank wood types; without this every plank rendered oak.
        "planks" => {
            let kind = match data_value & 0b0111 {
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                4 => "acacia",
                5 => "dark_oak",
                _ => "oak",
            };
            Block {
                name: format!("minecraft:{kind}_planks"),
                encoded: format!("minecraft:{kind}_planks|"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: skull type and floor rotation live in the tile entity,
        // so every legacy skull is a skeleton skull; data 1 = floor,
        // 2-5 = wall facing north/south/west/east.
        "skull" => {
            let facing = match data_value & 0b0111 {
                3 => Some("south"),
                4 => Some("west"),
                5 => Some("east"),
                2 => Some("north"),
                _ => None,
            };
            match facing {
                Some(f) => Block {
                    name: ns("skeleton_wall_skull"),
                    encoded: format!("skeleton_wall_skull|facing={f}"),
                    archetype: BlockArchetype::Normal,
                },
                None => Block {
                    name: ns("skeleton_skull"),
                    encoded: "skeleton_skull|rotation=0".to_owned(),
                    archetype: BlockArchetype::Normal,
                },
            }
        }
        // mciso patch: tripwire hook byHorizontalIndex facing, attached bit 4.
        "tripwire_hook" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let attached = data_value & 0b0100 != 0;
            Block {
                name: ns("tripwire_hook"),
                encoded: format!("tripwire_hook|attached={attached},facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: repeater/comparator byHorizontalIndex facing bits 0-1;
        // repeater delay bits 2-3, comparator subtract-mode bit 4.
        "unpowered_repeater" | "powered_repeater" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let delay = ((data_value & 0b1100) >> 2) + 1;
            let powered = block_name == "powered_repeater";
            Block {
                name: ns("repeater"),
                encoded: format!("repeater|delay={delay},facing={facing},powered={powered}"),
                archetype: BlockArchetype::Normal,
            }
        }
        "unpowered_comparator" | "powered_comparator" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let mode = if data_value & 0b0100 != 0 {
                "subtract"
            } else {
                "compare"
            };
            let powered = block_name == "powered_comparator";
            Block {
                name: ns("comparator"),
                encoded: format!("comparator|facing={facing},mode={mode},powered={powered}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: cocoa byHorizontalIndex facing bits 0-1, age bits 2-3.
        "cocoa" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let age = (data_value & 0b1100) >> 2;
            Block {
                name: ns("cocoa"),
                encoded: format!("cocoa|age={age},facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: end portal frame byHorizontalIndex facing, eye bit 4.
        "end_portal_frame" => {
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                2 => "north",
                _ => "east",
            };
            let eye = data_value & 0b0100 != 0;
            Block {
                name: ns("end_portal_frame"),
                encoded: format!("end_portal_frame|eye={eye},facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: hopper output facing (0=down, 2-5=north/south/west/east).
        "hopper" => {
            let facing = match data_value & 0b0111 {
                2 => "north",
                3 => "south",
                4 => "west",
                5 => "east",
                _ => "down",
            };
            Block {
                name: ns("hopper"),
                encoded: format!("hopper|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: crop growth stages are the data value itself; without
        // them the variant match picked age=0 and farms rendered empty.
        "wheat" | "carrots" | "potatoes" => Block {
            name: ns(block_name),
            encoded: format!("{block_name}|age={}", data_value & 0b0111),
            archetype: BlockArchetype::Normal,
        },
        "nether_wart" | "beetroots" => Block {
            name: ns(block_name),
            encoded: format!("{block_name}|age={}", data_value & 0b0011),
            archetype: BlockArchetype::Normal,
        },
        // mciso patch: stone brick / prismarine variants.
        "stonebrick" => {
            let kind = match data_value & 0b0011 {
                1 => "mossy_stone_bricks",
                2 => "cracked_stone_bricks",
                3 => "chiseled_stone_bricks",
                _ => "stone_bricks",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        "prismarine" => {
            let kind = match data_value & 0b0011 {
                1 => "prismarine_bricks",
                2 => "dark_prismarine",
                _ => "prismarine",
            };
            Block {
                name: ns(kind),
                encoded: enc0(kind),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: double plants: the lower half stores the species, the
        // upper half (bit 8) only a marker — the surface pass copies the
        // species up from the block below (world.rs).
        "double_plant" => {
            if data_value & 0b1000 != 0 {
                Block {
                    name: ns("sunflower"),
                    encoded: "sunflower|half=upper".to_owned(),
                    archetype: BlockArchetype::Normal,
                }
            } else {
                let kind = match data_value & 0b0111 {
                    1 => "lilac",
                    2 => "tall_grass",
                    3 => "large_fern",
                    4 => "rose_bush",
                    5 => "peony",
                    _ => "sunflower",
                };
                Block {
                    name: ns(kind),
                    encoded: format!("{kind}|half=lower"),
                    archetype: BlockArchetype::Normal,
                }
            }
        }
        // mciso patch: vine data is a bitmask of attached faces (1=south,
        // 2=west, 4=north, 8=east); 0 means only the block above. The full
        // prop set lets the multipart blockstate match.
        "vine" => {
            let tf = |b: bool| if b { "true" } else { "false" };
            let (s, w, n, e) = (
                data_value & 1 != 0,
                data_value & 2 != 0,
                data_value & 4 != 0,
                data_value & 8 != 0,
            );
            Block {
                name: ns("vine"),
                encoded: format!(
                    "vine|east={},north={},south={},up={},west={}",
                    tf(e),
                    tf(n),
                    tf(s),
                    tf(data_value == 0),
                    tf(w)
                ),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: mushroom block data picks which faces show cap vs pores
        // (1-9 = cap on top plus the outward sides at that position, 14 = cap
        // all, 10/15 = stem); the modern multipart needs all six face props.
        "brown_mushroom_block" | "red_mushroom_block" => {
            let (name, n, s, w, e, u, d) = match data_value {
                10 => ("mushroom_stem", true, true, true, true, false, false),
                15 => ("mushroom_stem", true, true, true, true, true, true),
                14 => (block_name, true, true, true, true, true, true),
                v @ 1..=9 => (
                    block_name,
                    matches!(v, 1..=3),
                    matches!(v, 7..=9),
                    matches!(v, 1 | 4 | 7),
                    matches!(v, 3 | 6 | 9),
                    true,
                    false,
                ),
                _ => (block_name, false, false, false, false, false, false),
            };
            let tf = |b: bool| if b { "true" } else { "false" };
            Block {
                name: ns(name),
                encoded: format!(
                    "{name}|down={},east={},north={},south={},up={},west={}",
                    tf(d),
                    tf(e),
                    tf(n),
                    tf(s),
                    tf(u),
                    tf(w)
                ),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: fire's floor/side split is computed at runtime from
        // neighbors; a floor flame is the right guess for a static render.
        "fire" => Block {
            name: ns("fire"),
            encoded: "fire|east=false,north=false,south=false,up=false,west=false".to_owned(),
            archetype: BlockArchetype::Normal,
        },
        // mciso patch: every pre-1.13 pumpkin had a carved face
        // (0=south, 1=west, 2=north, 3=east).
        "pumpkin" | "lit_pumpkin" => {
            let name = if block_name == "pumpkin" {
                "carved_pumpkin"
            } else {
                "jack_o_lantern"
            };
            let facing = match data_value & 0b0011 {
                0 => "south",
                1 => "west",
                3 => "east",
                _ => "north",
            };
            Block {
                name: ns(name),
                encoded: format!("{name}|facing={facing}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: torch metadata 1-4 = wall mount pointing east/west/
        // south/north, 5 = standing; redstone torches also carry their lit
        // state in the id (75 unlit / 76 lit).
        "torch" | "redstone_torch" | "unlit_redstone_torch" => {
            let redstone = block_name != "torch";
            let lit = if block_name == "unlit_redstone_torch" {
                "false"
            } else {
                "true"
            };
            let wall_facing = match data_value & 0b0111 {
                1 => Some("east"),
                2 => Some("west"),
                3 => Some("south"),
                4 => Some("north"),
                _ => None,
            };
            let (name, encoded) = match (redstone, wall_facing) {
                (false, None) => ("torch".to_owned(), "torch|".to_owned()),
                (false, Some(f)) => ("wall_torch".to_owned(), format!("wall_torch|facing={f}")),
                (true, None) => ("redstone_torch".to_owned(), format!("redstone_torch|lit={lit}")),
                (true, Some(f)) => (
                    "redstone_wall_torch".to_owned(),
                    format!("redstone_wall_torch|facing={f},lit={lit}"),
                ),
            };
            Block {
                name: format!("minecraft:{name}"),
                encoded,
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: rail shape metadata; powered-family rails only use the
        // flat/ascending shapes (bit 3 is the powered state, dropped).
        "rail" | "golden_rail" | "detector_rail" | "activator_rail" => {
            let raw = if block_name == "rail" {
                data_value & 0b1111
            } else {
                data_value & 0b0111
            };
            let shape = match raw {
                1 => "east_west",
                2 => "ascending_east",
                3 => "ascending_west",
                4 => "ascending_north",
                5 => "ascending_south",
                6 => "south_east",
                7 => "south_west",
                8 => "north_west",
                9 => "north_east",
                _ => "north_south",
            };
            Block {
                name: ns(block_name),
                encoded: format!("{block_name}|shape={shape}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: pillar axis (0=y, 4=x, 8=z).
        "hay_block" | "bone_block" | "purpur_pillar" => {
            let axis = match (data_value & 0b1100) >> 2 {
                1 => "x",
                2 => "z",
                _ => "y",
            };
            Block {
                name: ns(block_name),
                encoded: format!("{block_name}|axis={axis}"),
                archetype: BlockArchetype::Normal,
            }
        }
        // mciso patch: quartz block variants (1 = chiseled, 2-4 = pillar y/x/z).
        "quartz_block" => match data_value & 0b0111 {
            1 => Block {
                name: ns("chiseled_quartz_block"),
                encoded: enc0("chiseled_quartz_block"),
                archetype: BlockArchetype::Normal,
            },
            v @ 2..=4 => {
                let axis = match v {
                    3 => "x",
                    4 => "z",
                    _ => "y",
                };
                Block {
                    name: ns("quartz_pillar"),
                    encoded: format!("quartz_pillar|axis={axis}"),
                    archetype: BlockArchetype::Normal,
                }
            }
            _ => Block {
                name: ns("quartz_block"),
                encoded,
                archetype: BlockArchetype::Normal,
            },
        },
        // mciso patch: stairs metadata is bits 0-1 = ascending east/west/south/
        // north and bit 2 = upside down; shape was computed at runtime (default
        // straight). Id 67 predates smooth stone stairs and is cobblestone.
        n if n.ends_with("_stairs") => {
            let name = if n == "stone_stairs" {
                "cobblestone_stairs"
            } else {
                n
            };
            let facing = match data_value & 0b11 {
                0 => "east",
                1 => "west",
                2 => "south",
                _ => "north",
            };
            let half = if data_value & 0b100 == 0 {
                "bottom"
            } else {
                "top"
            };
            Block {
                name: ns(name),
                encoded: format!("{name}|facing={facing},half={half},shape=straight"),
                archetype: BlockArchetype::Normal,
            }
        }
        _ => Block {
            name: ns(block_name),
            encoded,
            archetype: BlockArchetype::Normal,
        },
    }
}

/// Return the block name for a given pre-1.13 block id. The returned name does not contain the
/// `minecraft:` prefix.
///
/// Block ids can be up 12 bits, but here we only support 8-bit block ids, which are the ones used
/// in vanilla minecraft.
pub fn block_name(block_id: u8) -> &'static str {
    match block_id {
        0 => "air",
        1 => "stone",
        2 => "grass",
        3 => "dirt",
        4 => "cobblestone",
        5 => "planks",
        6 => "sapling",
        7 => "bedrock",
        8 => "flowing_water",
        9 => "water",
        10 => "flowing_lava",
        11 => "lava",
        12 => "sand",
        13 => "gravel",
        14 => "gold_ore",
        15 => "iron_ore",
        16 => "coal_ore",
        17 => "log",
        18 => "leaves",
        19 => "sponge",
        20 => "glass",
        21 => "lapis_ore",
        22 => "lapis_block",
        23 => "dispenser",
        24 => "sandstone",
        25 => "noteblock",
        26 => "bed",
        27 => "golden_rail",
        28 => "detector_rail",
        29 => "sticky_piston",
        30 => "web",
        31 => "tallgrass",
        32 => "deadbush",
        33 => "piston",
        34 => "piston_head",
        35 => "wool",
        36 => "piston_extension",
        37 => "yellow_flower",
        38 => "red_flower",
        39 => "brown_mushroom",
        40 => "red_mushroom",
        41 => "gold_block",
        42 => "iron_block",
        43 => "double_stone_slab",
        44 => "stone_slab",
        45 => "brick_block",
        46 => "tnt",
        47 => "bookshelf",
        48 => "mossy_cobblestone",
        49 => "obsidian",
        50 => "torch",
        51 => "fire",
        52 => "mob_spawner",
        53 => "oak_stairs",
        54 => "chest",
        55 => "redstone_wire",
        56 => "diamond_ore",
        57 => "diamond_block",
        58 => "crafting_table",
        59 => "wheat",
        60 => "farmland",
        61 => "furnace",
        62 => "lit_furnace",
        63 => "standing_sign",
        64 => "wooden_door",
        65 => "ladder",
        66 => "rail",
        67 => "stone_stairs",
        68 => "wall_sign",
        69 => "lever",
        70 => "stone_pressure_plate",
        71 => "iron_door",
        72 => "wooden_pressure_plate",
        73 => "redstone_ore",
        74 => "lit_redstone_ore",
        75 => "unlit_redstone_torch",
        76 => "redstone_torch",
        77 => "stone_button",
        78 => "snow_layer",
        79 => "ice",
        80 => "snow",
        81 => "cactus",
        82 => "clay",
        83 => "reeds",
        84 => "jukebox",
        85 => "fence",
        86 => "pumpkin",
        87 => "netherrack",
        88 => "soul_sand",
        89 => "glowstone",
        90 => "portal",
        91 => "lit_pumpkin",
        92 => "cake",
        93 => "unpowered_repeater",
        94 => "powered_repeater",
        95 => "stained_glass",
        96 => "trapdoor",
        97 => "monster_egg",
        98 => "stonebrick",
        99 => "brown_mushroom_block",
        100 => "red_mushroom_block",
        101 => "iron_bars",
        102 => "glass_pane",
        103 => "melon_block",
        104 => "pumpkin_stem",
        105 => "melon_stem",
        106 => "vine",
        107 => "fence_gate",
        108 => "brick_stairs",
        109 => "stone_brick_stairs",
        110 => "mycelium",
        111 => "waterlily",
        112 => "nether_brick",
        113 => "nether_brick_fence",
        114 => "nether_brick_stairs",
        115 => "nether_wart",
        116 => "enchanting_table",
        117 => "brewing_stand",
        118 => "cauldron",
        119 => "end_portal",
        120 => "end_portal_frame",
        121 => "end_stone",
        122 => "dragon_egg",
        123 => "redstone_lamp",
        124 => "lit_redstone_lamp",
        125 => "double_wooden_slab",
        126 => "wooden_slab",
        127 => "cocoa",
        128 => "sandstone_stairs",
        129 => "emerald_ore",
        130 => "ender_chest",
        131 => "tripwire_hook",
        132 => "tripwire",
        133 => "emerald_block",
        134 => "spruce_stairs",
        135 => "birch_stairs",
        136 => "jungle_stairs",
        137 => "command_block",
        138 => "beacon",
        139 => "cobblestone_wall",
        140 => "flower_pot",
        141 => "carrots",
        142 => "potatoes",
        143 => "oak_button",
        144 => "skull",
        145 => "anvil",
        146 => "trapped_chest",
        147 => "light_weighted_pressure_plate",
        148 => "heavy_weighted_pressure_plate",
        149 => "unpowered_comparator",
        150 => "powered_comparator",
        151 => "daylight_detector",
        152 => "redstone_block",
        153 => "quartz_ore",
        154 => "hopper",
        155 => "quartz_block",
        156 => "quartz_stairs",
        157 => "activator_rail",
        158 => "dropper",
        159 => "stained_hardened_clay",
        160 => "stained_glass_pane",
        161 => "leaves2",
        162 => "log2",
        163 => "acacia_stairs",
        164 => "dark_oak_stairs",
        165 => "slime",
        166 => "barrier",
        167 => "iron_trapdoor",
        168 => "prismarine",
        169 => "sea_lantern",
        170 => "hay_block",
        171 => "carpet",
        172 => "hardened_clay",
        173 => "coal_block",
        174 => "packed_ice",
        175 => "double_plant",
        176 => "standing_banner",
        177 => "wall_banner",
        178 => "daylight_detector_inverted",
        179 => "red_sandstone",
        180 => "red_sandstone_stairs",
        181 => "double_stone_slab2",
        182 => "stone_slab2",
        183 => "spruce_fence_gate",
        184 => "birch_fence_gate",
        185 => "jungle_fence_gate",
        186 => "dark_oak_fence_gate",
        187 => "acacia_fence_gate",
        188 => "spruce_fence",
        189 => "birch_fence",
        190 => "jungle_fence",
        191 => "dark_oak_fence",
        192 => "acacia_fence",
        193 => "spruce_door",
        194 => "birch_door",
        195 => "jungle_door",
        196 => "acacia_door",
        197 => "dark_oak_door",
        198 => "end_rod",
        199 => "chorus_plant",
        200 => "chorus_flower",
        201 => "purpur_block",
        202 => "purpur_pillar",
        203 => "purpur_stairs",
        204 => "purpur_double_slab",
        205 => "purpur_slab",
        206 => "end_bricks",
        207 => "beetroots",
        208 => "grass_path",
        209 => "end_gateway",
        210 => "repeating_command_block",
        211 => "chain_command_block",
        212 => "frosted_ice",
        213 => "magma",
        214 => "nether_wart_block",
        215 => "red_nether_brick",
        216 => "bone_block",
        217 => "structure_void",
        218 => "observer",
        219 => "white_shulker_box",
        220 => "orange_shulker_box",
        221 => "magenta_shulker_box",
        222 => "light_blue_shulker_box",
        223 => "yellow_shulker_box",
        224 => "lime_shulker_box",
        225 => "pink_shulker_box",
        226 => "gray_shulker_box",
        227 => "silver_shulker_box",
        228 => "cyan_shulker_box",
        229 => "purple_shulker_box",
        230 => "blue_shulker_box",
        231 => "brown_shulker_box",
        232 => "green_shulker_box",
        233 => "red_shulker_box",
        234 => "black_shulker_box",
        235 => "white_glazed_terracotta",
        236 => "orange_glazed_terracotta",
        237 => "magenta_glazed_terracotta",
        238 => "light_blue_glazed_terracotta",
        239 => "yellow_glazed_terracotta",
        240 => "lime_glazed_terracotta",
        241 => "pink_glazed_terracotta",
        242 => "gray_glazed_terracotta",
        243 => "silver_glazed_terracotta",
        244 => "cyan_glazed_terracotta",
        245 => "purple_glazed_terracotta",
        246 => "blue_glazed_terracotta",
        247 => "brown_glazed_terracotta",
        248 => "green_glazed_terracotta",
        249 => "red_glazed_terracotta",
        250 => "black_glazed_terracotta",
        251 => "concrete",
        252 => "concrete_powder",
        253 => "",
        254 => "",
        255 => "structure_block",
    }
}
