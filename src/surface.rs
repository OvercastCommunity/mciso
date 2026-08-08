use anyhow::{bail, ensure, Context, Result};

use crate::world::{Block, Surface};

const MAGIC: &[u8; 4] = b"MSRF";
const VERSION: u8 = 1;

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn get_varint(input: &mut &[u8]) -> Result<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let (&b, rest) = input.split_first().context("truncated varint")?;
        *input = rest;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    bail!("varint overflow")
}

pub fn encode(surface: &Surface) -> Vec<u8> {
    let mut blocks: Vec<Block> = surface.blocks.clone();
    blocks.sort_unstable_by_key(|b| (b.z, b.x, b.y));
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    put_varint(&mut out, surface.palette.len() as u64);
    for name in &surface.palette {
        put_varint(&mut out, name.len() as u64);
        out.extend_from_slice(name.as_bytes());
    }
    put_varint(&mut out, blocks.len() as u64);
    let (mut px, mut pz) = (0i64, 0i64);
    let mut i = 0;
    while i < blocks.len() {
        let (x, z) = (blocks[i].x, blocks[i].z);
        let mut j = i;
        while j < blocks.len() && blocks[j].x == x && blocks[j].z == z {
            j += 1;
        }
        put_varint(&mut out, zigzag(z as i64 - pz));
        put_varint(&mut out, zigzag(x as i64 - px));
        put_varint(&mut out, (j - i) as u64);
        let mut py = 0i64;
        for b in &blocks[i..j] {
            put_varint(&mut out, zigzag(b.y as i64 - py));
            put_varint(&mut out, b.state as u64);
            py = b.y as i64;
        }
        (px, pz) = (x as i64, z as i64);
        i = j;
    }
    out
}

pub fn decode(bytes: &[u8]) -> Result<Surface> {
    let mut input = bytes;
    let header = input
        .split_off(..MAGIC.len() + 1)
        .context("truncated header")?;
    ensure!(&header[..MAGIC.len()] == MAGIC, "not a surface blob");
    ensure!(
        header[MAGIC.len()] == VERSION,
        "unsupported surface version {}",
        header[MAGIC.len()]
    );
    let palette_len = get_varint(&mut input)? as usize;
    ensure!(palette_len <= bytes.len(), "palette length implausible");
    let mut palette = Vec::with_capacity(palette_len);
    for _ in 0..palette_len {
        let len = get_varint(&mut input)? as usize;
        let raw = input.split_off(..len).context("truncated palette entry")?;
        palette.push(String::from_utf8(raw.to_vec()).context("palette entry not utf-8")?);
    }
    let total = get_varint(&mut input)? as usize;
    ensure!(total <= bytes.len() * 4, "block count implausible");
    let mut blocks = Vec::with_capacity(total);
    let (mut px, mut pz) = (0i64, 0i64);
    while blocks.len() < total {
        let z = pz
            .checked_add(unzigzag(get_varint(&mut input)?))
            .and_then(|v| i32::try_from(v).ok())
            .context("z out of range")?;
        let x = px
            .checked_add(unzigzag(get_varint(&mut input)?))
            .and_then(|v| i32::try_from(v).ok())
            .context("x out of range")?;
        let count = get_varint(&mut input)? as usize;
        ensure!(count <= total - blocks.len(), "column overruns block count");
        let mut py = 0i64;
        for _ in 0..count {
            let y = py
                .checked_add(unzigzag(get_varint(&mut input)?))
                .and_then(|v| i32::try_from(v).ok())
                .context("y out of range")?;
            let state = get_varint(&mut input)? as u32;
            ensure!(
                (state as usize) < palette.len(),
                "state {state} outside palette"
            );
            blocks.push(Block { x, y, z, state });
            py = y as i64;
        }
        (px, pz) = (x as i64, z as i64);
    }
    ensure!(input.is_empty(), "trailing bytes after blocks");
    Ok(Surface { palette, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Surface {
        Surface {
            palette: vec![
                "minecraft:stone".into(),
                "minecraft:oak_fence|north=true|south=false".into(),
            ],
            blocks: vec![
                Block {
                    x: 5,
                    y: 64,
                    z: -3,
                    state: 0,
                },
                Block {
                    x: 5,
                    y: 65,
                    z: -3,
                    state: 1,
                },
                Block {
                    x: -100,
                    y: -20,
                    z: 7,
                    state: 0,
                },
                Block {
                    x: 5,
                    y: 80,
                    z: -3,
                    state: 0,
                },
            ],
        }
    }

    fn sorted(mut blocks: Vec<Block>) -> Vec<(i32, i32, i32, u32)> {
        blocks.sort_unstable_by_key(|b| (b.z, b.x, b.y));
        blocks
            .into_iter()
            .map(|b| (b.x, b.y, b.z, b.state))
            .collect()
    }

    #[test]
    fn roundtrip() {
        let surface = sample();
        let decoded = decode(&encode(&surface)).unwrap();
        assert_eq!(decoded.palette, surface.palette);
        assert_eq!(sorted(decoded.blocks), sorted(surface.blocks));
    }

    #[test]
    fn empty_roundtrip() {
        let surface = Surface {
            palette: vec![],
            blocks: vec![],
        };
        let decoded = decode(&encode(&surface)).unwrap();
        assert!(decoded.palette.is_empty());
        assert!(decoded.blocks.is_empty());
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode(b"").is_err());
        assert!(decode(b"MSRF").is_err());
        assert!(decode(b"MSRF\x02\x00\x00").is_err());
        assert!(decode(b"PK\x03\x04junk").is_err());
    }

    #[test]
    fn rejects_truncation_and_trailing() {
        let bytes = encode(&sample());
        for cut in 1..bytes.len() {
            assert!(
                decode(&bytes[..cut]).is_err(),
                "accepted prefix of {cut} bytes"
            );
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(decode(&extra).is_err());
    }

    #[test]
    fn rejects_state_outside_palette() {
        let surface = Surface {
            palette: vec!["minecraft:stone".into()],
            blocks: vec![Block {
                x: 0,
                y: 0,
                z: 0,
                state: 9,
            }],
        };
        assert!(decode(&encode(&surface)).is_err());
    }
}
