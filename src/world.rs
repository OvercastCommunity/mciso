use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use fastanvil::{Chunk, JavaChunk, Region};
use rayon::prelude::*;

use crate::state::{self, Connectable};

pub struct World {
    chunks: HashMap<(i32, i32), JavaChunk>,
}

pub struct Surface {
    pub palette: Vec<String>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy)]
pub struct Block {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: u32,
}

fn register_extended_id_fallback() {
    use std::sync::OnceLock;
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        static UNKNOWN: Mutex<Vec<(u16, &'static fastanvil::Block)>> = Mutex::new(Vec::new());
        let _ = fastanvil::pre13::set_custom_block_callback(Box::new(|id, _data| {
            let mut known = UNKNOWN.lock().unwrap();
            let block = match known.iter().find(|(k, _)| *k == id) {
                Some(&(_, b)) => b,
                None => {
                    let b: &'static fastanvil::Block = Box::leak(Box::new(fastanvil::Block::new(
                        format!("minecraft:unknown_{id}"),
                    )));
                    known.push((id, b));
                    b
                }
            };
            Some(block)
        }));
    });
}

pub fn load_world(dir: &Path) -> Result<World> {
    register_extended_id_fallback();
    let region_dir = if dir.join("region").is_dir() {
        dir.join("region")
    } else {
        dir.to_path_buf()
    };
    let mut regions: Vec<(i32, i32, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&region_dir)
        .with_context(|| format!("reading {}", region_dir.display()))?
    {
        let path = entry?.path();
        if let Some((rx, rz)) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(parse_region_coords)
        {
            regions.push((rx, rz, path));
        }
    }
    if regions.is_empty() {
        bail!("no region files (r.X.Z.mca) in {}", region_dir.display());
    }

    let chunks: HashMap<(i32, i32), JavaChunk> = regions
        .par_iter()
        .map(|&(rx, rz, ref path)| load_region(rx, rz, path))
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();
    Ok(World { chunks })
}

pub fn world_from_regions(
    regions: Vec<(i32, i32, Vec<u8>)>,
    progress: &(impl Fn(usize) + Sync),
) -> Result<World> {
    register_extended_id_fallback();
    if regions.is_empty() {
        bail!("no region files supplied");
    }
    let done = std::sync::atomic::AtomicUsize::new(0);
    let parse_one = |(rx, rz, data): (i32, i32, Vec<u8>)| {
        let out = match Region::from_stream(std::io::Cursor::new(data.as_slice())) {
            Ok(mut region) => read_region(rx, rz, &mut region),
            Err(e) => {
                eprintln!("WARN skipping region ({rx}, {rz}): {e:#}");
                Vec::new()
            }
        };
        drop(data);
        progress(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1);
        out
    };
    let per_region: Vec<Vec<((i32, i32), JavaChunk)>> = if cfg!(target_arch = "wasm32") {
        regions.into_iter().map(parse_one).collect()
    } else {
        regions.into_par_iter().map(parse_one).collect()
    };
    let chunks: HashMap<(i32, i32), JavaChunk> = per_region.into_iter().flatten().collect();
    Ok(World { chunks })
}

pub fn parse_region_coords(name: &str) -> Option<(i32, i32)> {
    let mut parts = name.strip_prefix("r.")?.strip_suffix(".mca")?.split('.');
    let rx = parts.next()?.parse().ok()?;
    let rz = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((rx, rz))
}

fn load_region(rx: i32, rz: i32, path: &Path) -> Vec<((i32, i32), JavaChunk)> {
    let mut region = match File::open(path)
        .map_err(anyhow::Error::from)
        .and_then(|f| Region::from_stream(f).map_err(anyhow::Error::from))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("WARN skipping region {}: {e:#}", path.display());
            return Vec::new();
        }
    };
    read_region(rx, rz, &mut region)
}

fn read_region<S: std::io::Read + std::io::Seek>(
    rx: i32,
    rz: i32,
    region: &mut Region<S>,
) -> Vec<((i32, i32), JavaChunk)> {
    let mut out = Vec::new();
    for chunk_data in region.iter() {
        let chunk_data = match chunk_data {
            Ok(c) => c,
            Err(e) => {
                eprintln!("WARN skipping chunk in region ({rx}, {rz}): {e}");
                continue;
            }
        };
        let (lx, lz) = (chunk_data.x, chunk_data.z);
        match JavaChunk::from_bytes(&chunk_data.data) {
            Ok(chunk) if status_ok(&chunk.status()) => {
                out.push(((rx * 32 + lx as i32, rz * 32 + lz as i32), chunk));
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("WARN skipping chunk ({lx}, {lz}) in region ({rx}, {rz}): {e}");
            }
        }
    }
    out
}

fn status_ok(status: &str) -> bool {
    status.contains("full") || matches!(status, "postprocessed" | "mobs_spawned" | "finalized" | "")
}

fn is_surface_block(x: i32, y: i32, z: i32, passable_at: &impl Fn(i32, i32, i32) -> bool) -> bool {
    passable_at(x, y + 1, z)
        || passable_at(x - 1, y, z)
        || passable_at(x + 1, y, z)
        || passable_at(x, y, z - 1)
        || passable_at(x, y, z + 1)
}

#[derive(Default)]
struct Interner {
    map: HashMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.map.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.to_owned());
        self.map.insert(name.to_owned(), id);
        id
    }
}

impl World {
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn extract_surface(&self, occludes: &(impl Fn(&str) -> bool + Sync)) -> Surface {
        self.extract_surface_traced(occludes, &|_| {})
    }

    pub fn extract_surface_traced(
        &self,
        occludes: &(impl Fn(&str) -> bool + Sync),
        progress: &(impl Fn(usize) + Sync),
    ) -> Surface {
        let done = std::sync::atomic::AtomicUsize::new(0);
        let per_chunk: Vec<(Vec<Block>, Vec<String>)> = self
            .chunks
            .par_iter()
            .map(|(&(cx, cz), chunk)| {
                let out = self.chunk_surface(cx, cz, chunk, occludes);
                progress(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1);
                out
            })
            .collect();
        let mut interner = Interner::default();
        let total = per_chunk.iter().map(|(b, _)| b.len()).sum();
        let mut blocks = Vec::with_capacity(total);
        for (mut chunk_blocks, names) in per_chunk {
            let remap: Vec<u32> = names.iter().map(|n| interner.intern(n)).collect();
            for b in &mut chunk_blocks {
                b.state = remap[b.state as usize];
            }
            blocks.append(&mut chunk_blocks);
        }
        Surface {
            palette: interner.names,
            blocks,
        }
    }

    fn chunk_surface(
        &self,
        cx: i32,
        cz: i32,
        chunk: &JavaChunk,
        occludes: &impl Fn(&str) -> bool,
    ) -> (Vec<Block>, Vec<String>) {
        let neighbors = [
            self.chunks.get(&(cx - 1, cz)),
            self.chunks.get(&(cx + 1, cz)),
            self.chunks.get(&(cx, cz - 1)),
            self.chunks.get(&(cx, cz + 1)),
        ];
        let chunk_at = |ax: i32, az: i32| -> Option<&JavaChunk> {
            match (ax >> 4 == cx, az >> 4 == cz) {
                (true, true) => Some(chunk),
                (false, true) => neighbors[usize::from(ax >> 4 > cx)],
                (true, false) => neighbors[2 + usize::from(az >> 4 > cz)],
                (false, false) => None,
            }
        };
        let occ_cache = std::cell::RefCell::new(HashMap::<String, bool>::new());
        let occluding = |name: &str| -> bool {
            let mut cache = occ_cache.borrow_mut();
            match cache.get(name) {
                Some(&v) => v,
                None => {
                    let v = occludes(name);
                    cache.insert(name.to_owned(), v);
                    v
                }
            }
        };
        let passable_at = |ax: i32, ay: i32, az: i32| -> bool {
            match chunk_at(ax, az) {
                None => true,
                Some(c) => c
                    .block((ax & 15) as usize, ay as isize, (az & 15) as usize)
                    .is_none_or(|b| {
                        let name = b.name();
                        state::is_air(name) || !occluding(name)
                    }),
            }
        };

        let mut local: HashMap<String, u32> = HashMap::new();
        let mut names: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let y_range = chunk.y_range();
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let (x, z) = (cx * 16 + lx, cz * 16 + lz);
                for y in y_range.clone() {
                    let Some(block) = chunk.block(lx as usize, y, lz as usize) else {
                        continue;
                    };
                    let name = block.name();
                    if state::is_air(name) {
                        continue;
                    }
                    if !is_surface_block(x, y as i32, z, &passable_at) {
                        continue;
                    }
                    let encoded = block.encoded_description();
                    let p = state::state_parts(name, encoded, || {
                        chunk.block(lx as usize, y - 1, lz as usize).map(|b| {
                            (
                                state::modernize(b.name()).0.into_owned(),
                                b.encoded_description().to_owned(),
                            )
                        })
                    });
                    let kind =
                        state::connectable(&p.name).filter(|_| !p.encoded.contains("north="));
                    let conn = |kind: Connectable, dx: i32, dz: i32| -> bool {
                        let (ax, az) = (x + dx, z + dz);
                        chunk_at(ax, az).is_some_and(|c| {
                            c.block((ax & 15) as usize, y, (az & 15) as usize)
                                .is_some_and(|b| {
                                    let bn = b.name();
                                    !state::is_air(bn) && state::connects(kind, bn, occluding(bn))
                                })
                        })
                    };
                    let lookup: Cow<str> = match kind {
                        None => Cow::Borrowed(p.encoded.as_ref()),
                        Some(kind) => state::connected_key(&p, kind, conn).into(),
                    };
                    let state = match local.get(lookup.as_ref()) {
                        Some(&id) => id,
                        None => {
                            let id = names.len() as u32;
                            names.push(match kind {
                                None => state::plain_key(&p.name, &p.encoded, p.extra),
                                Some(_) => lookup.clone().into_owned(),
                            });
                            local.insert(lookup.into_owned(), id);
                            id
                        }
                    };
                    out.push(Block {
                        x,
                        y: y as i32,
                        z,
                        state,
                    });
                }
            }
        }
        (out, names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_filename_parsing() {
        assert_eq!(parse_region_coords("r.0.0.mca"), Some((0, 0)));
        assert_eq!(parse_region_coords("r.-1.2.mca"), Some((-1, 2)));
        assert_eq!(parse_region_coords("r.12.-34.mca"), Some((12, -34)));
        assert_eq!(parse_region_coords("r.1.mca"), None);
        assert_eq!(parse_region_coords("r.1.2.3.mca"), None);
        assert_eq!(parse_region_coords("r.a.b.mca"), None);
        assert_eq!(parse_region_coords("level.dat"), None);
        assert_eq!(parse_region_coords("x.1.2.mca"), None);
    }

    #[test]
    fn interner_dedupes() {
        let mut i = Interner::default();
        assert_eq!(i.intern("minecraft:stone"), 0);
        assert_eq!(i.intern("minecraft:dirt"), 1);
        assert_eq!(i.intern("minecraft:stone"), 0);
        assert_eq!(i.names, vec!["minecraft:stone", "minecraft:dirt"]);
    }

    #[test]
    fn surface_visibility_predicate() {
        assert!(!is_surface_block(5, 60, 5, &|_, _, _| false));
        assert!(is_surface_block(5, 60, 5, &|_, y, _| y == 61));
        assert!(is_surface_block(5, 60, 5, &|x, _, _| x == 4));
        assert!(!is_surface_block(5, 60, 5, &|_, y, _| y == 59));
        assert!(is_surface_block(5, 60, 5, &|_, _, z| z == 6));
    }

    #[test]
    fn chunk_status_acceptance() {
        for ok in [
            "full",
            "minecraft:full",
            "fullchunk",
            "postprocessed",
            "mobs_spawned",
            "finalized",
            "",
        ] {
            assert!(status_ok(ok), "{ok:?} should be accepted");
        }
        for bad in ["empty", "structure_starts", "features", "light", "carvers"] {
            assert!(!status_ok(bad), "{bad:?} should be rejected");
        }
    }
}
