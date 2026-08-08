use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn snap(label: &str, t0: Instant, a0: u64, b0: u64) {
    let (a, b) = (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    println!(
        "{label}: {:.2}s, {} allocs, {:.1} MB allocated",
        t0.elapsed().as_secs_f64(),
        a - a0,
        (b - b0) as f64 / 1e6
    );
}

fn main() {
    let world_dir = std::env::args()
        .nth(1)
        .expect("usage: profile_build <world-dir>");
    let region_dir = std::path::Path::new(&world_dir).join("region");
    let mut regions = Vec::new();
    for entry in std::fs::read_dir(&region_dir).unwrap() {
        let path = entry.unwrap().path();
        if let Some((rx, rz)) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(mciso::world::parse_region_coords)
        {
            regions.push((rx, rz, std::fs::read(&path).unwrap()));
        }
    }
    println!(
        "{} regions, {} threads",
        regions.len(),
        rayon::current_num_threads()
    );

    let (t0, a0, b0) = (
        Instant::now(),
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    let world = mciso::world::world_from_regions(regions, &|_| {}).unwrap();
    snap("parse  ", t0, a0, b0);

    let artist = mciso::artist::TextureArtist::default_packs();
    let (t0, a0, b0) = (
        Instant::now(),
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    let surface = world.extract_surface(&|name| artist.occludes(name));
    snap("extract", t0, a0, b0);
    println!("blocks: {}", surface.blocks.len());
}
