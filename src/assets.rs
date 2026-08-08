use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

pub enum Pack {
    Dir(PathBuf),
    Mem {
        base: String,
        files: HashMap<String, Vec<u8>>,
    },
}

impl Pack {
    pub fn read(&self, rel: &str) -> Option<Cow<'_, [u8]>> {
        match self {
            Pack::Dir(root) => std::fs::read(root.join(rel)).ok().map(Cow::Owned),
            Pack::Mem { base, files } => files
                .get(&normalize(base, rel)?)
                .map(|v| Cow::Borrowed(v.as_slice())),
        }
    }
}

fn normalize(base: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for p in base.split('/').chain(rel.split('/')) {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

const MAGIC: &[u8; 4] = b"MCB1";

pub fn write_bundle(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    for (path, data) in entries {
        out.extend((path.len() as u32).to_le_bytes());
        out.extend(path.as_bytes());
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
    }
    out
}

pub fn parse_bundle(bytes: &[u8]) -> Option<HashMap<String, Vec<u8>>> {
    let mut rest = bytes.strip_prefix(MAGIC)?;
    let mut files = HashMap::new();
    while !rest.is_empty() {
        let (path, r) = take(rest)?;
        let (data, r) = take(r)?;
        files.insert(String::from_utf8(path.to_vec()).ok()?, data.to_vec());
        rest = r;
    }
    Some(files)
}

fn take(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = u32::from_le_bytes(bytes.get(..4)?.try_into().unwrap()) as usize;
    let rest = &bytes[4..];
    (rest.len() >= len).then(|| rest.split_at(len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_resolves_dots_and_rejects_escapes() {
        assert_eq!(normalize("block", "stone.png").unwrap(), "block/stone.png");
        assert_eq!(
            normalize("block", "../entity/chest/normal.png").unwrap(),
            "entity/chest/normal.png"
        );
        assert_eq!(
            normalize("", "models/block/cube.json").unwrap(),
            "models/block/cube.json"
        );
        assert_eq!(normalize("a", "./b//c").unwrap(), "a/b/c");
        assert!(normalize("", "../escape.png").is_none());
    }

    #[test]
    fn bundle_round_trips() {
        let entries = vec![
            ("block/stone.png".to_owned(), vec![1u8, 2, 3]),
            ("mc/blockstates/stone.json".to_owned(), b"{}".to_vec()),
            ("empty.bin".to_owned(), vec![]),
        ];
        let files = parse_bundle(&write_bundle(&entries)).unwrap();
        assert_eq!(files.len(), 3);
        for (path, data) in &entries {
            assert_eq!(files.get(path), Some(data));
        }
    }

    #[test]
    fn truncated_or_foreign_bundles_are_rejected() {
        assert!(parse_bundle(b"PNG!").is_none());
        let good = write_bundle(&[("a".to_owned(), vec![9; 8])]);
        assert!(parse_bundle(&good[..good.len() - 3]).is_none());
    }

    #[test]
    fn mem_pack_reads_through_base_and_dotdot() {
        let files = HashMap::from([
            ("block/stone.png".to_owned(), vec![1u8]),
            ("entity/chest/normal.png".to_owned(), vec![2u8]),
        ]);
        let pack = Pack::Mem {
            base: "block".to_owned(),
            files,
        };
        assert_eq!(pack.read("stone.png").unwrap().as_ref(), &[1]);
        assert_eq!(
            pack.read("../entity/chest/normal.png").unwrap().as_ref(),
            &[2]
        );
        assert!(pack.read("missing.png").is_none());
    }
}
