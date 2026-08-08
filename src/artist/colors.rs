use crate::state;

pub(super) fn color_for(name: &str) -> [u8; 3] {
    let name = state::strip_ns(name);
    if let Some(rgb) = exact(name) {
        return rgb;
    }
    heuristic(name).unwrap_or([127, 127, 127])
}

fn exact(name: &str) -> Option<[u8; 3]> {
    Some(match name {
        "stone" | "smooth_stone" => [125, 125, 125],
        "grass_block" | "grass" => [110, 156, 66],
        "dirt" | "coarse_dirt" | "rooted_dirt" => [134, 96, 67],
        "podzol" | "mycelium" => [111, 84, 59],
        "cobblestone" | "mossy_cobblestone" => [110, 110, 110],
        "bedrock" => [60, 60, 60],
        "sand" => [219, 209, 160],
        "red_sand" => [190, 102, 33],
        "gravel" => [136, 126, 126],
        "water" | "bubble_column" => [63, 118, 228],
        "lava" => [207, 92, 20],
        "ice" | "frosted_ice" => [145, 183, 253],
        "packed_ice" | "blue_ice" => [125, 173, 255],
        "snow" | "snow_block" | "powder_snow" => [249, 254, 254],
        "clay" => [160, 166, 179],
        "obsidian" | "crying_obsidian" => [21, 18, 30],
        "netherrack" => [111, 54, 52],
        "soul_sand" | "soul_soil" => [81, 62, 50],
        "glowstone" => [249, 212, 156],
        "end_stone" => [219, 222, 158],
        "sandstone" | "smooth_sandstone" | "cut_sandstone" | "chiseled_sandstone" => {
            [216, 202, 155]
        }
        "granite" | "polished_granite" => [153, 114, 99],
        "diorite" | "polished_diorite" => [188, 188, 188],
        "andesite" | "polished_andesite" => [136, 136, 137],
        "deepslate" | "cobbled_deepslate" | "polished_deepslate" => [80, 80, 82],
        "tuff" => [108, 109, 102],
        "calcite" => [223, 224, 220],
        "dripstone_block" => [134, 107, 92],
        "moss_block" | "moss_carpet" => [89, 109, 45],
        "mud" => [60, 57, 60],
        "muddy_mangrove_roots" | "mangrove_roots" => [70, 58, 45],
        "farmland" => [113, 78, 55],
        "dirt_path" => [148, 121, 65],
        "terracotta" => [152, 94, 67],
        "bricks" => [150, 97, 83],
        "bookshelf" => [180, 144, 90],
        "crafting_table" => [123, 79, 46],
        "furnace" | "blast_furnace" | "smoker" | "dispenser" | "dropper" => [110, 110, 110],
        "chest" | "trapped_chest" | "barrel" => [162, 115, 44],
        "pumpkin" | "carved_pumpkin" | "jack_o_lantern" => [198, 118, 24],
        "melon" => [111, 144, 31],
        "cactus" => [85, 127, 43],
        "sugar_cane" => [148, 192, 101],
        "vine" | "glow_lichen" => [58, 88, 30],
        "cobweb" => [228, 233, 234],
        "torch" | "wall_torch" | "lantern" => [255, 216, 128],
        "coal_ore" | "deepslate_coal_ore" | "coal_block" => [55, 55, 55],
        "iron_ore" | "deepslate_iron_ore" | "raw_iron_block" => [216, 175, 147],
        "gold_ore" | "deepslate_gold_ore" | "raw_gold_block" => [246, 208, 61],
        "diamond_ore" | "deepslate_diamond_ore" => [98, 219, 214],
        "emerald_ore" | "deepslate_emerald_ore" => [23, 221, 98],
        "lapis_ore" | "deepslate_lapis_ore" => [22, 64, 201],
        "redstone_ore" | "deepslate_redstone_ore" => [140, 20, 6],
        "copper_ore" | "deepslate_copper_ore" | "raw_copper_block" => [192, 108, 80],
        "iron_block" => [220, 220, 220],
        "gold_block" => [246, 208, 61],
        "diamond_block" => [98, 219, 214],
        "emerald_block" => [42, 203, 87],
        "lapis_block" => [30, 66, 172],
        "redstone_block" => [175, 24, 5],
        "netherite_block" => [66, 61, 63],
        "quartz_block" | "smooth_quartz" | "chiseled_quartz_block" | "quartz_pillar" => {
            [235, 229, 222]
        }
        "hay_block" => [166, 136, 38],
        "bone_block" => [209, 206, 179],
        "sea_lantern" => [172, 199, 190],
        "prismarine" => [99, 156, 151],
        "prismarine_bricks" | "dark_prismarine" => [70, 129, 116],
        "sponge" | "wet_sponge" => [196, 192, 75],
        "slime_block" => [111, 192, 91],
        "honey_block" => [251, 185, 52],
        "tnt" => [219, 68, 26],
        "mushroom_stem" => [203, 196, 185],
        "brown_mushroom_block" => [149, 111, 81],
        "red_mushroom_block" => [200, 46, 45],
        _ => return None,
    })
}

fn heuristic(name: &str) -> Option<[u8; 3]> {
    const RULES: &[(&str, [u8; 3])] = &[
        ("leaves", [58, 95, 34]),
        ("azalea", [93, 118, 48]),
        ("spruce", [114, 84, 48]),
        ("birch", [196, 179, 123]),
        ("jungle", [160, 115, 80]),
        ("acacia", [168, 90, 50]),
        ("dark_oak", [66, 43, 20]),
        ("mangrove", [117, 54, 48]),
        ("cherry", [226, 178, 172]),
        ("bamboo", [193, 173, 80]),
        ("crimson", [148, 63, 97]),
        ("warped", [43, 104, 99]),
        ("oak", [162, 130, 78]),
        ("wool", [200, 200, 200]),
        ("carpet", [200, 200, 200]),
        ("concrete", [160, 160, 160]),
        ("terracotta", [152, 94, 67]),
        ("glass", [200, 220, 225]),
        ("coral", [190, 100, 140]),
        ("shulker", [140, 100, 140]),
        ("candle", [230, 220, 190]),
        ("white", [233, 236, 236]),
        ("orange", [240, 118, 19]),
        ("magenta", [189, 68, 179]),
        ("light_blue", [58, 175, 217]),
        ("yellow", [248, 197, 39]),
        ("lime", [112, 185, 25]),
        ("pink", [237, 141, 172]),
        ("gray", [62, 68, 71]),
        ("cyan", [21, 137, 145]),
        ("purple", [121, 42, 172]),
        ("blue", [53, 57, 157]),
        ("brown", [114, 71, 40]),
        ("green", [84, 109, 27]),
        ("red", [161, 39, 34]),
        ("black", [20, 21, 25]),
        ("deepslate", [80, 80, 82]),
        ("blackstone", [42, 36, 41]),
        ("basalt", [80, 81, 86]),
        ("nether_brick", [44, 21, 26]),
        ("nether", [111, 54, 52]),
        ("end_stone", [219, 222, 158]),
        ("purpur", [170, 126, 170]),
        ("copper", [192, 108, 80]),
        ("amethyst", [133, 97, 191]),
        ("sculk", [13, 30, 36]),
        ("stone_brick", [122, 122, 122]),
        ("sandstone", [216, 202, 155]),
        ("brick", [150, 97, 83]),
        ("mud", [60, 57, 60]),
        ("sapling", [90, 140, 60]),
        ("flower", [220, 180, 90]),
        ("tulip", [220, 180, 90]),
        ("orchid", [46, 165, 246]),
        ("rose", [190, 60, 50]),
        ("fern", [90, 130, 60]),
        ("bush", [80, 110, 50]),
        ("kelp", [70, 120, 60]),
        ("seagrass", [70, 130, 70]),
        ("pickle", [120, 140, 70]),
        ("mushroom", [180, 140, 110]),
        ("fungus", [160, 90, 100]),
        ("roots", [130, 90, 70]),
        ("wart", [130, 40, 50]),
        ("rail", [140, 120, 100]),
        ("stone", [125, 125, 125]),
        ("planks", [162, 130, 78]),
        ("log", [110, 88, 55]),
        ("wood", [110, 88, 55]),
        ("hyphae", [110, 60, 80]),
        ("fence", [140, 110, 70]),
        ("door", [150, 120, 75]),
        ("sign", [150, 120, 75]),
        ("wheat", [200, 180, 90]),
        ("carrot", [90, 140, 50]),
        ("potato", [90, 140, 50]),
        ("beetroot", [110, 130, 60]),
        ("snow", [249, 254, 254]),
        ("ice", [145, 183, 253]),
        ("iron", [220, 220, 220]),
        ("gold", [246, 208, 61]),
    ];
    RULES
        .iter()
        .find(|(needle, _)| name.contains(needle))
        .map(|(_, rgb)| *rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_blocks_fall_back_to_gray() {
        assert_eq!(color_for("minecraft:some_future_block"), [127, 127, 127]);
    }

    #[test]
    fn heuristics_cover_variants() {
        assert_eq!(color_for("minecraft:stripped_spruce_log"), [114, 84, 48]);
        assert_ne!(color_for("minecraft:oak_leaves"), [127, 127, 127]);
    }

    #[test]
    fn heuristic_order_keeps_specific_rules_before_generic() {
        assert_eq!(color_for("dark_oak_planks"), [66, 43, 20]);
        assert_eq!(color_for("mossy_stone_bricks"), [122, 122, 122]);
        assert_eq!(color_for("nether_bricks"), [44, 21, 26]);
        assert_eq!(color_for("light_blue_bed"), [58, 175, 217]);
    }
}
