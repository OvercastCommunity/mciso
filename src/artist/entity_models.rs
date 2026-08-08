use std::sync::Arc;

use super::model::{BlockModel, Element, Face, Placement};
use crate::state::Props;

pub(super) fn placements(
    base: &str,
    props: &Props,
    has_texture: &dyn Fn(&str) -> bool,
) -> Option<Vec<Placement>> {
    chest(base, props, has_texture)
        .or_else(|| sign(base, props, has_texture))
        .or_else(|| bed(base, props, has_texture))
        .or_else(|| banner(base, props, has_texture))
        .or_else(|| skull(base, props, has_texture))
        .or_else(|| dragon_head(base, props, has_texture))
        .or_else(|| conduit(base, has_texture))
        .or_else(|| decorated_pot(base, props, has_texture))
        .or_else(|| end_portal(base, has_texture))
        .or_else(|| copper_golem(base, props, has_texture))
}

fn atlas_uv(aw: u32, ah: u32) -> impl Fn(u32, u32, u32, u32) -> [f32; 4] {
    move |x0, y0, x1, y1| {
        [
            x0 as f32 * 16.0 / aw as f32,
            y0 as f32 * 16.0 / ah as f32,
            x1 as f32 * 16.0 / aw as f32,
            y1 as f32 * 16.0 / ah as f32,
        ]
    }
}

fn face_full(
    tex: &str,
    uv: Option<[f32; 4]>,
    rotation: i32,
    tintindex: Option<i32>,
) -> Option<Face> {
    Some(Face {
        texture: tex.to_owned(),
        uv,
        rotation,
        tintindex,
    })
}

fn boxed(from: [f32; 3], to: [f32; 3], faces: [Option<Face>; 6]) -> Element {
    Element {
        from,
        to,
        rotation: None,
        shade: true,
        faces,
    }
}

fn placed(elements: Vec<Element>, y: u32) -> Option<Vec<Placement>> {
    Some(vec![Placement {
        model: Arc::new(BlockModel { elements }),
        x: 0,
        y,
        uvlock: false,
    }])
}

fn entity_turns(props: &Props) -> u32 {
    if let Some(r) = props.rotation {
        return ((r + 2) / 4 + 2) % 4;
    }
    match props.facing {
        Some((0, -1)) => 0,
        Some((1, 0)) => 1,
        Some((-1, 0)) => 3,
        _ => 2,
    }
}

fn chest(base: &str, props: &Props, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let stem = match base.strip_prefix("waxed_").unwrap_or(base) {
        "chest" => "../entity/chest/normal",
        "trapped_chest" => "../entity/chest/trapped",
        "ender_chest" => "../entity/chest/ender",
        "copper_chest" => "../entity/chest/copper",
        "exposed_copper_chest" => "../entity/chest/copper_exposed",
        "weathered_copper_chest" => "../entity/chest/copper_weathered",
        "oxidized_copper_chest" => "../entity/chest/copper_oxidized",
        _ => return None,
    };
    if !has_texture(stem) {
        return None;
    }
    let auv = atlas_uv(64, 64);
    let uv = move |x0: u32, y0: u32, x1: u32, y1: u32| auv(x0, y1, x1, y0);
    let face = |uv: [f32; 4]| face_full(stem, Some(uv), 0, None);
    let base_side = face(uv(28, 33, 42, 43));
    let lid_side = face(uv(28, 14, 42, 19));
    let elements = vec![
        boxed(
            [1.0, 0.0, 1.0],
            [15.0, 10.0, 15.0],
            [
                None,
                face(uv(14, 19, 28, 33)),
                face(uv(42, 33, 56, 43)),
                base_side.clone(),
                base_side.clone(),
                base_side,
            ],
        ),
        boxed(
            [1.0, 9.0, 1.0],
            [15.0, 14.0, 15.0],
            [
                None,
                face(uv(28, 0, 42, 14)),
                face(uv(42, 14, 56, 19)),
                lid_side.clone(),
                lid_side.clone(),
                lid_side,
            ],
        ),
        boxed(
            [7.0, 7.0, 0.0],
            [9.0, 11.0, 1.0],
            [
                None,
                face(uv(1, 0, 3, 1)),
                face(uv(1, 1, 3, 5)),
                None,
                face(uv(0, 1, 1, 5)),
                face(uv(3, 1, 4, 5)),
            ],
        ),
    ];
    placed(elements, entity_turns(props))
}

fn sign(base: &str, props: &Props, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let (wood, wall, hanging) = if let Some(w) = base.strip_suffix("_wall_hanging_sign") {
        (w, true, true)
    } else if let Some(w) = base.strip_suffix("_hanging_sign") {
        (w, false, true)
    } else if let Some(w) = base.strip_suffix("_wall_sign") {
        (w, true, false)
    } else if let Some(w) = base.strip_suffix("_sign") {
        (w, false, false)
    } else {
        match base {
            "sign" | "standing_sign" => ("oak", false, false),
            "wall_sign" => ("oak", true, false),
            _ => return None,
        }
    };
    let stem = if hanging {
        format!("../entity/signs/hanging/{wood}")
    } else {
        format!("../entity/signs/{wood}")
    };
    if !has_texture(&stem) {
        return None;
    }
    let uv = atlas_uv(64, 32);
    let face = |uv: [f32; 4]| face_full(&stem, Some(uv), 0, None);
    let mut elements = Vec::new();
    if hanging {
        elements.push(boxed(
            [0.0, 0.0, 7.0],
            [16.0, 10.0, 9.0],
            [
                None,
                face(uv(2, 12, 18, 14)),
                face(uv(2, 14, 18, 24)),
                face(uv(20, 14, 36, 24)),
                face(uv(0, 14, 2, 24)),
                face(uv(18, 14, 20, 24)),
            ],
        ));
    } else {
        let board_y = if wall { (4.5, 12.5) } else { (8.0, 16.0) };
        elements.push(boxed(
            [0.0, board_y.0, if wall { 14.0 } else { 7.0 }],
            [16.0, board_y.1, if wall { 16.0 } else { 9.0 }],
            [
                None,
                face(uv(2, 0, 26, 2)),
                face(uv(2, 2, 26, 14)),
                face(uv(28, 2, 52, 14)),
                face(uv(0, 2, 2, 14)),
                face(uv(26, 2, 28, 14)),
            ],
        ));
        if !wall {
            elements.push(boxed(
                [7.0, 0.0, 7.0],
                [9.0, 8.0, 9.0],
                [
                    None,
                    None,
                    face(uv(2, 16, 4, 30)),
                    face(uv(2, 16, 4, 30)),
                    face(uv(0, 16, 2, 30)),
                    face(uv(4, 16, 6, 30)),
                ],
            ));
        }
    }
    placed(elements, entity_turns(props))
}

fn dragon_head(
    base: &str,
    props: &Props,
    has_texture: &dyn Fn(&str) -> bool,
) -> Option<Vec<Placement>> {
    let wall = match base {
        "dragon_head" => false,
        "dragon_wall_head" => true,
        _ => return None,
    };
    let stem = "../entity/enderdragon/dragon";
    if !has_texture(stem) {
        return None;
    }
    let uv = atlas_uv(256, 256);
    let face = |uv: [f32; 4]| face_full(stem, Some(uv), 0, None);
    let dy = if wall { 4.0 } else { 0.0 };
    let head = boxed(
        [2.0, dy, 4.0],
        [14.0, dy + 12.0, 16.0],
        [
            face(uv(144, 30, 160, 46)),
            face(uv(128, 30, 144, 46)),
            face(uv(128, 46, 144, 62)),
            face(uv(160, 46, 176, 62)),
            face(uv(112, 46, 128, 62)),
            face(uv(144, 46, 160, 62)),
        ],
    );
    let snout = boxed(
        [4.0, dy + 3.0, 0.0],
        [12.0, dy + 7.0, 4.0],
        [
            face(uv(204, 44, 216, 60)),
            face(uv(192, 44, 204, 60)),
            face(uv(192, 60, 204, 65)),
            None,
            face(uv(176, 60, 192, 65)),
            face(uv(176, 60, 192, 65)),
        ],
    );
    placed(vec![head, snout], entity_turns(props))
}

fn conduit(base: &str, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    if base != "conduit" {
        return None;
    }
    let stem = "../entity/conduit/base";
    if !has_texture(stem) {
        return None;
    }
    let uv = atlas_uv(32, 16);
    let face = |uv: [f32; 4]| face_full(stem, Some(uv), 0, None);
    let elements = vec![boxed(
        [4.0, 4.0, 4.0],
        [12.0, 12.0, 12.0],
        [
            face(uv(16, 0, 24, 8)),
            face(uv(8, 0, 16, 8)),
            face(uv(8, 8, 16, 16)),
            face(uv(24, 8, 32, 16)),
            face(uv(0, 8, 8, 16)),
            face(uv(16, 8, 24, 16)),
        ],
    )];
    placed(elements, 0)
}

fn decorated_pot(
    base: &str,
    props: &Props,
    has_texture: &dyn Fn(&str) -> bool,
) -> Option<Vec<Placement>> {
    if base != "decorated_pot" {
        return None;
    }
    let side = "../entity/decorated_pot/decorated_pot_side";
    let pot_base = "../entity/decorated_pot/decorated_pot_base";
    if !has_texture(side) || !has_texture(pot_base) {
        return None;
    }
    let uv = atlas_uv(32, 32);
    let bface = |uv: [f32; 4]| face_full(pot_base, Some(uv), 0, None);
    let sface = || face_full(side, None, 0, None);
    let body = boxed(
        [1.0, 0.0, 1.0],
        [15.0, 13.0, 15.0],
        [
            bface(uv(14, 13, 28, 27)),
            bface(uv(0, 13, 14, 27)),
            sface(),
            sface(),
            sface(),
            sface(),
        ],
    );
    let neck_side = bface(uv(0, 8, 8, 11));
    let neck = boxed(
        [4.0, 13.0, 4.0],
        [12.0, 16.0, 12.0],
        [
            None,
            bface(uv(8, 0, 16, 8)),
            neck_side.clone(),
            neck_side.clone(),
            neck_side.clone(),
            neck_side,
        ],
    );
    placed(vec![body, neck], entity_turns(props))
}

fn end_portal(base: &str, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let full = match base {
        "end_portal" => false,
        "end_gateway" => true,
        _ => return None,
    };
    let stem = "../entity/end_portal/end_portal";
    if !has_texture(stem) {
        return None;
    }
    let face = || face_full(stem, None, 0, None);
    let element = if full {
        boxed(
            [0.0, 0.0, 0.0],
            [16.0, 16.0, 16.0],
            [face(), face(), face(), face(), face(), face()],
        )
    } else {
        boxed(
            [0.0, 0.0, 0.0],
            [16.0, 12.0, 16.0],
            [None, face(), None, None, None, None],
        )
    };
    placed(vec![element], 0)
}

fn copper_golem(
    base: &str,
    props: &Props,
    has_texture: &dyn Fn(&str) -> bool,
) -> Option<Vec<Placement>> {
    let suffix = match base.strip_prefix("waxed_").unwrap_or(base) {
        "copper_golem_statue" => "",
        "exposed_copper_golem_statue" => "_exposed",
        "weathered_copper_golem_statue" => "_weathered",
        "oxidized_copper_golem_statue" => "_oxidized",
        _ => return None,
    };
    let stem = format!("../entity/copper_golem/copper_golem{suffix}");
    if !has_texture(&stem) {
        return None;
    }
    let uv = atlas_uv(64, 64);
    let face = |uv: [f32; 4]| face_full(&stem, Some(uv), 0, None);
    let head = boxed(
        [4.0, 9.0, 3.0],
        [12.0, 13.0, 13.0],
        [
            face(uv(18, 0, 26, 10)),
            face(uv(10, 0, 18, 10)),
            face(uv(10, 10, 18, 14)),
            face(uv(28, 10, 36, 14)),
            face(uv(0, 10, 10, 14)),
            face(uv(18, 10, 28, 14)),
        ],
    );
    let body = boxed(
        [4.0, 3.0, 5.0],
        [12.0, 9.0, 11.0],
        [
            None,
            face(uv(6, 15, 14, 21)),
            face(uv(6, 21, 14, 30)),
            face(uv(20, 21, 28, 30)),
            face(uv(0, 21, 6, 30)),
            face(uv(14, 21, 20, 30)),
        ],
    );
    let foot_side = face(uv(0, 31, 8, 35));
    let feet = boxed(
        [4.0, 0.0, 4.0],
        [12.0, 3.0, 12.0],
        [
            None,
            face(uv(4, 27, 12, 31)),
            foot_side.clone(),
            foot_side.clone(),
            foot_side.clone(),
            foot_side,
        ],
    );
    let rod_side = face(uv(58, 0, 60, 3));
    let antenna = boxed(
        [7.0, 13.0, 7.0],
        [9.0, 16.0, 9.0],
        [
            None,
            face(uv(60, 0, 62, 2)),
            rod_side.clone(),
            rod_side.clone(),
            rod_side.clone(),
            rod_side,
        ],
    );
    placed(vec![head, body, feet, antenna], entity_turns(props))
}

fn skull(base: &str, props: &Props, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let (kind, wall) = if let Some(k) = base.strip_suffix("_wall_skull") {
        (k, true)
    } else if let Some(k) = base.strip_suffix("_skull") {
        (k, false)
    } else if let Some(k) = base.strip_suffix("_wall_head") {
        (k, true)
    } else if let Some(k) = base.strip_suffix("_head") {
        (k, false)
    } else if base == "skull" {
        ("skeleton", false)
    } else {
        return None;
    };
    let (stem, tall) = match kind {
        "skeleton" => ("../entity/skeleton/skeleton", false),
        "wither_skeleton" => ("../entity/skeleton/wither_skeleton", false),
        "zombie" => ("../entity/zombie/zombie", true),
        "creeper" => ("../entity/creeper/creeper", false),
        "piglin" => ("../entity/piglin/piglin", true),
        "player" => ("../entity/player/wide/steve", true),
        _ => return None,
    };
    if !has_texture(stem) {
        return None;
    }
    let uv = atlas_uv(64, if tall { 64 } else { 32 });
    let face = |uv: [f32; 4]| face_full(stem, Some(uv), 0, None);
    let (from, to) = if wall {
        ([4.0, 4.0, 8.0], [12.0, 12.0, 16.0])
    } else {
        ([4.0, 0.0, 4.0], [12.0, 8.0, 12.0])
    };
    let elements = vec![boxed(
        from,
        to,
        [
            face(uv(16, 0, 24, 8)),
            face(uv(8, 0, 16, 8)),
            face(uv(8, 8, 16, 16)),
            face(uv(24, 8, 32, 16)),
            face(uv(0, 8, 8, 16)),
            face(uv(16, 8, 24, 16)),
        ],
    )];
    placed(elements, entity_turns(props))
}

fn bed(base: &str, props: &Props, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let color = match base.strip_suffix("_bed") {
        Some(c) => c,
        None if base == "bed" => "red",
        None => return None,
    };
    let stem = format!("../entity/bed/{color}");
    if !has_texture(&stem) {
        return None;
    }
    let uv = atlas_uv(64, 64);
    let face = |uv: [f32; 4], rot: i32| face_full(&stem, Some(uv), rot, None);
    let foot = props.part_foot;
    let mattress = if foot {
        boxed(
            [0.0, 3.0, 0.0],
            [16.0, 9.0, 16.0],
            [
                None,
                face(uv(6, 28, 22, 44), 0),
                None,
                face(uv(22, 22, 38, 28), 0),
                face(uv(0, 28, 6, 44), 90),
                face(uv(22, 28, 28, 44), 270),
            ],
        )
    } else {
        boxed(
            [0.0, 3.0, 0.0],
            [16.0, 9.0, 16.0],
            [
                None,
                face(uv(6, 6, 22, 22), 0),
                face(uv(6, 0, 22, 6), 0),
                None,
                face(uv(0, 6, 6, 22), 90),
                face(uv(22, 6, 28, 22), 270),
            ],
        )
    };
    let mut elements = vec![mattress];
    let leg = |x0: f32, z0: f32| {
        let f = face(uv(52, 2, 55, 5), 0);
        boxed(
            [x0, 0.0, z0],
            [x0 + 3.0, 3.0, z0 + 3.0],
            [None, None, f.clone(), f.clone(), f.clone(), f],
        )
    };
    let z = if foot { 13.0 } else { 0.0 };
    elements.push(leg(0.0, z));
    elements.push(leg(13.0, z));
    placed(elements, entity_turns(props))
}

fn banner(base: &str, props: &Props, has_texture: &dyn Fn(&str) -> bool) -> Option<Vec<Placement>> {
    let wall = match base.strip_suffix("_wall_banner") {
        Some(_) => true,
        None if base == "wall_banner" => true,
        None => match base.strip_suffix("_banner") {
            Some(_) => false,
            None if base == "standing_banner" => false,
            None => return None,
        },
    };
    let stem = "../entity/banner/base";
    if !has_texture(stem) {
        return None;
    }
    let cloth_face = |uv: [f32; 4]| face_full(stem, Some(uv), 0, Some(0));
    let wood_face = || face_full("oak_planks", None, 0, None);
    let uv = atlas_uv(64, 64);
    let zc = if wall { 13.6 } else { 6.2 };
    let mut elements = vec![boxed(
        [1.5, 0.0, zc],
        [14.5, 15.0, zc + 0.6],
        [
            None,
            None,
            cloth_face(uv(1, 1, 21, 41)),
            cloth_face(uv(22, 1, 42, 41)),
            None,
            None,
        ],
    )];
    let zb = if wall { 13.0 } else { 7.0 };
    elements.push(boxed(
        [1.0, 15.0, zb],
        [15.0, 16.0, zb + 2.0],
        [
            None,
            wood_face(),
            wood_face(),
            wood_face(),
            wood_face(),
            wood_face(),
        ],
    ));
    if !wall {
        elements.push(boxed(
            [7.0, 0.0, 7.0],
            [9.0, 15.0, 9.0],
            [
                None,
                None,
                wood_face(),
                wood_face(),
                wood_face(),
                wood_face(),
            ],
        ));
    }
    placed(elements, entity_turns(props))
}
