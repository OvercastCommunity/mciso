#![allow(clippy::missing_safety_doc)]

use std::sync::Mutex;

use crate::artist::TextureArtist;
use crate::compress::{encode_outputs, Encoding};
use crate::render::{Rotation, Scene};
use crate::world::{self, Surface};
use crate::{assets, render_view};

#[link(wasm_import_module = "env")]
extern "C" {
    fn mciso_console(ptr: *const u8, len: usize);
    fn mciso_spawn(thread: u32);
}

fn log(msg: &str) {
    unsafe { mciso_console(msg.as_ptr(), msg.len()) }
}

struct State {
    artist: Option<TextureArtist>,
    regions: Vec<(i32, i32, Vec<u8>)>,
    surface: Option<Surface>,
    scene: Option<(u32, Scene)>,
    result: Vec<u8>,
}

static STATE: Mutex<State> = Mutex::new(State {
    artist: None,
    regions: Vec::new(),
    surface: None,
    scene: None,
    result: Vec::new(),
});

fn rotation(turns: u32) -> Rotation {
    match turns % 4 {
        0 => Rotation::TopLeft,
        1 => Rotation::TopRight,
        2 => Rotation::BottomRight,
        _ => Rotation::BottomLeft,
    }
}

fn tier(ht: i32) -> i32 {
    ht.clamp(2, 64) & !1
}

fn ensure_scene(state: &mut State, turns: u32) -> bool {
    let turns = turns % 4;
    if state.scene.as_ref().is_some_and(|(t, _)| *t == turns) {
        return true;
    }
    state.scene = None;
    let Some(surface) = &state.surface else {
        log("mciso_build not called");
        return false;
    };
    state.scene = Some((turns, Scene::new(surface, rotation(turns))));
    true
}

#[no_mangle]
pub extern "C" fn mciso_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    ptr
}

unsafe fn take_buf(ptr: *mut u8, len: usize) -> Vec<u8> {
    Vec::from_raw_parts(ptr, len, len)
}

#[no_mangle]
pub extern "C" fn mciso_pool(threads: u32) -> u32 {
    std::panic::set_hook(Box::new(|info| log(&info.to_string())));
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1) as usize)
        .use_current_thread()
        .spawn_handler(|thread| {
            unsafe { mciso_spawn(Box::into_raw(Box::new(thread)) as u32) };
            Ok(())
        })
        .build_global()
        .is_ok() as u32
}

#[no_mangle]
pub unsafe extern "C" fn mciso_init(ptr: *mut u8, len: usize) -> u32 {
    let bytes = take_buf(ptr, len);
    let Some(files) = assets::parse_bundle(&bytes) else {
        log("bad asset bundle");
        return 0;
    };
    let count = files.len();
    STATE.lock().unwrap().artist = Some(TextureArtist::from_bundle(files));
    count as u32
}

#[no_mangle]
pub unsafe extern "C" fn mciso_add_region(rx: i32, rz: i32, ptr: *mut u8, len: usize) {
    let mut state = STATE.lock().unwrap();
    state.surface = None;
    state.scene = None;
    state.regions.push((rx, rz, take_buf(ptr, len)));
}

#[no_mangle]
pub unsafe extern "C" fn mciso_load_surface(ptr: *mut u8, len: usize) -> u32 {
    let bytes = take_buf(ptr, len);
    let mut state = STATE.lock().unwrap();
    state.regions.clear();
    state.scene = None;
    state.surface = None;
    match crate::surface::decode(&bytes) {
        Ok(surface) => {
            let blocks = surface.blocks.len();
            state.surface = Some(surface);
            blocks.min(u32::MAX as usize) as u32
        }
        Err(e) => {
            log(&format!("surface blob rejected: {e:#}"));
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn mciso_clear() {
    let mut state = STATE.lock().unwrap();
    state.regions.clear();
    state.surface = None;
    state.scene = None;
    state.result = Vec::new();
}

#[no_mangle]
pub unsafe extern "C" fn mciso_worker_entry(thread: u32) {
    Box::from_raw(thread as *mut rayon::ThreadBuilder).run();
}

#[no_mangle]
pub extern "C" fn mciso_build() -> u32 {
    let mut state = STATE.lock().unwrap();
    if state.artist.is_none() {
        log("mciso_init not called");
        return 0;
    }
    let total = state.regions.len();
    let regions = std::mem::take(&mut state.regions);
    let artist = state.artist.as_ref().unwrap();
    let world = match world::world_from_regions(regions, &|done| {
        log(&format!("parsed region {done}/{total}"));
    }) {
        Ok(world) => world,
        Err(e) => {
            log(&format!("loading world: {e:#}"));
            return 0;
        }
    };
    let chunks = world.chunk_count();
    let step = (chunks / 20).max(1);
    let surface = world.extract_surface_traced(&|name| artist.occludes(name), &|done| {
        if done % step == 0 || done == chunks {
            log(&format!("surface {}%", done * 100 / chunks.max(1)));
        }
    });
    let blocks = surface.blocks.len();
    state.surface = Some(surface);
    state.scene = None;
    blocks.min(u32::MAX as usize) as u32
}

#[no_mangle]
pub extern "C" fn mciso_view_size(turns: u32, ht: i32) -> u64 {
    let mut state = STATE.lock().unwrap();
    if !ensure_scene(&mut state, turns) {
        return 0;
    }
    let (w, h) = state.scene.as_ref().unwrap().1.size(tier(ht));
    (w as u64) << 32 | h as u64
}

#[no_mangle]
pub extern "C" fn mciso_render_viewport(
    turns: u32,
    ht: i32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> u32 {
    let (w, h) = (w.min(8192), h.min(8192));
    let mut state = STATE.lock().unwrap();
    if !ensure_scene(&mut state, turns) {
        return 0;
    }
    let State {
        artist: Some(artist),
        scene: Some((_, scene)),
        ..
    } = &*state
    else {
        log("mciso_init not called");
        return 0;
    };
    let img = scene.render(artist, tier(ht), Some((x, y, w, h)));
    state.result = img.into_raw();
    state.result.len() as u32
}

#[no_mangle]
pub extern "C" fn mciso_render(turns: u32, max_w: u32, max_h: u32, colors: u32) -> u32 {
    let mut state = STATE.lock().unwrap();
    let (Some(artist), Some(surface)) = (&state.artist, &state.surface) else {
        log("mciso_build not called");
        return 0;
    };
    let enc = Encoding {
        max_w,
        max_h,
        colors: (colors > 0).then_some(colors.min(256) as u16),
        overshoot: false,
    };
    let png = render_view(surface, rotation(turns), artist, (max_w, max_h))
        .and_then(|(image, crop, _)| encode_outputs(&image, crop, &enc));
    match png {
        Ok((main, _)) => {
            state.result = main;
            state.result.len() as u32
        }
        Err(e) => {
            log(&format!("render failed: {e:#}"));
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn mciso_result_ptr() -> *const u8 {
    STATE.lock().unwrap().result.as_ptr()
}
