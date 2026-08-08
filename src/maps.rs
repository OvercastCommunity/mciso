use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

pub struct MapFolder {
    pub name: String,
    pub world_dir: PathBuf,
}

pub fn find_maps(input_dir: &Path, since: Option<&str>) -> Result<Vec<MapFolder>> {
    let folders = all_map_folders(input_dir);
    let mut maps: Vec<(&PathBuf, MapFolder)> = folders
        .iter()
        .flat_map(|f| map_jobs(f).into_iter().map(move |m| (f, m)))
        .collect();
    dedup_names(&mut maps);
    let keep = since
        .map(|commit| changed_map_folders(input_dir, commit))
        .transpose()?;
    Ok(maps
        .into_iter()
        .filter(|(folder, _)| keep.as_ref().is_none_or(|k| k.contains(*folder)))
        .map(|(_, map)| map)
        .collect())
}

fn dedup_names(maps: &mut [(&PathBuf, MapFolder)]) {
    let mut taken: HashSet<String> = maps.iter().map(|(_, m)| m.name.clone()).collect();
    let mut seen: HashSet<String> = HashSet::new();
    for (_, map) in maps.iter_mut() {
        if seen.insert(map.name.clone()) {
            continue;
        }
        let mut n = 2;
        let renamed = loop {
            let candidate = format!("{}_{n}", map.name);
            if !taken.contains(&candidate) {
                break candidate;
            }
            n += 1;
        };
        eprintln!(
            "WARN duplicate slug {} at {}; renaming to {renamed}",
            map.name,
            map.world_dir.display()
        );
        taken.insert(renamed.clone());
        map.name = renamed;
    }
}

fn map_jobs(folder: &Path) -> Vec<MapFolder> {
    let variants = std::fs::read_to_string(folder.join("map.xml"))
        .map_err(anyhow::Error::from)
        .and_then(|xml| parse_variants(&xml));
    match variants {
        Ok(variants) => variants
            .into_iter()
            .filter_map(|v| {
                let dir = match v.world {
                    Some(w) => {
                        let dir = folder.join(&w);
                        if !dir.is_dir() {
                            eprintln!(
                                "WARN {}: variant {} declares missing world {w}; skipping",
                                folder.display(),
                                v.id
                            );
                            return None;
                        }
                        dir
                    }
                    None => folder.to_path_buf(),
                };
                Some(MapFolder {
                    name: v.id,
                    world_dir: find_world(&dir),
                })
            })
            .collect(),
        Err(e) => {
            eprintln!(
                "WARN {}: {e:#}; falling back to folder name",
                folder.display()
            );
            vec![MapFolder {
                name: folder
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                world_dir: find_world(folder),
            }]
        }
    }
}

struct Variant {
    id: String,
    world: Option<String>,
}

fn parse_variants(xml: &str) -> Result<Vec<Variant>> {
    let doc = roxmltree::Document::parse(xml).context("parsing map.xml")?;
    let root = doc.root_element();
    let name = child_text(root, "name")
        .or_else(|| root.attribute("name").map(normalize))
        .context("map.xml has no <name>")?;
    let root_slug = child_text(root, "slug");
    let default_slug = || {
        cond_slug(root, "default")
            .map(|(s, _)| s)
            .or_else(|| root_slug.clone())
            .unwrap_or_else(|| slugify(&name))
    };

    let mut els = Vec::new();
    collect_variant_elements(root, &mut els);

    let mut out: Vec<Variant> = Vec::new();
    let mut has_default = false;
    for el in els {
        let Some(id) = el.attribute("id") else {
            continue;
        };
        let world = el.attribute("world").map(str::to_owned);
        if id == "default" {
            has_default = true;
            out.insert(
                0,
                Variant {
                    id: default_slug(),
                    world,
                },
            );
        } else {
            let text = normalize(el.text().unwrap_or(""));
            let override_name = el.attribute("override") == Some("true");
            let full_name = if override_name {
                text
            } else {
                format!("{name}: {text}")
            };
            let slug = el
                .attribute("slug")
                .map(str::to_owned)
                .or_else(|| {
                    cond_slug(root, id)
                        .map(|(s, explicit)| if explicit { s } else { format!("{s}_{id}") })
                })
                .unwrap_or_else(|| match (override_name, &root_slug) {
                    (true, _) => slugify(&full_name),
                    (false, Some(rs)) => format!("{rs}_{id}"),
                    (false, None) => slugify(&full_name),
                });
            out.push(Variant { id: slug, world });
        }
    }
    if !has_default {
        out.insert(
            0,
            Variant {
                id: default_slug(),
                world: None,
            },
        );
    }
    Ok(out)
}

fn cond_applies(el: roxmltree::Node, vid: Option<&str>) -> Option<bool> {
    let is_if = match el.tag_name().name() {
        "if" => true,
        "unless" => false,
        _ => return None,
    };
    if el.attribute("max-server-version").is_some() {
        return Some(!is_if);
    }
    match (el.attribute("variant"), vid) {
        (Some(v), Some(id)) => Some(v.split_whitespace().any(|x| x == id) == is_if),
        (Some(_), None) => Some(true),
        (None, _) => Some(is_if),
    }
}

fn cond_slug(el: roxmltree::Node, vid: &str) -> Option<(String, bool)> {
    for c in el.children().filter(|c| c.is_element()) {
        if cond_applies(c, Some(vid)) != Some(true) {
            continue;
        }
        let explicit = c.tag_name().name() == "if"
            && c.attribute("variant")
                .is_some_and(|v| v.split_whitespace().any(|x| x == vid));
        if let Some(s) = child_text(c, "slug") {
            return Some((s, explicit));
        }
        if let Some((s, e)) = cond_slug(c, vid) {
            return Some((s, e || explicit));
        }
    }
    None
}

fn collect_variant_elements<'a, 'input>(
    el: roxmltree::Node<'a, 'input>,
    out: &mut Vec<roxmltree::Node<'a, 'input>>,
) {
    for child in el.children().filter(|c| c.is_element()) {
        if child.tag_name().name() == "variant" {
            out.push(child);
        } else if cond_applies(child, None) == Some(true) {
            collect_variant_elements(child, out);
        }
    }
}

fn child_text(el: roxmltree::Node, tag: &str) -> Option<String> {
    el.children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
        .and_then(|c| c.text())
        .map(normalize)
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slugify(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfd()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '_')
        .map(|c| {
            if c == ' ' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

fn all_map_folders(input_dir: &Path) -> BTreeSet<PathBuf> {
    let mut found = BTreeSet::new();
    let mut pending = vec![input_dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if dir.join("map.xml").is_file() {
            found.insert(dir);
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        pending.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    found
}

fn changed_map_folders(input_dir: &Path, since: &str) -> Result<BTreeSet<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", since])
        .current_dir(input_dir)
        .output()
        .context("running git diff")?;
    anyhow::ensure!(
        output.status.success(),
        "git diff failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".mca"))
        .filter_map(|line| map_folder_of(&input_dir.join(line)))
        .collect())
}

fn map_folder_of(mca_file: &Path) -> Option<PathBuf> {
    mca_file
        .ancestors()
        .skip(1)
        .find(|dir| dir.join("map.xml").is_file())
        .map(Path::to_path_buf)
}

fn find_world(map_folder: &Path) -> PathBuf {
    for name in ["_imaging_edit", "DIM-1", "DIM1"] {
        let candidate = map_folder.join(name);
        if candidate.is_dir() {
            return candidate;
        }
    }
    map_folder.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_matches_pgm() {
        assert_eq!(slugify("Water Drop: Directrix"), "water_drop_directrix");
        assert_eq!(
            slugify("The Grand Dueling Tournament II"),
            "the_grand_dueling_tournament_ii"
        );
        assert_eq!(slugify("Éclair au Café"), "eclair_au_cafe");
        assert_eq!(slugify("A_B  C"), "a_b__c");
    }

    fn ids(xml: &str) -> Vec<(String, Option<String>)> {
        parse_variants(xml)
            .unwrap()
            .into_iter()
            .map(|v| (v.id, v.world))
            .collect()
    }

    #[test]
    fn no_variants_yields_default_from_name() {
        assert_eq!(
            ids("<map><name>Runic Altar</name></map>"),
            [("runic_altar".into(), None)]
        );
    }

    #[test]
    fn root_slug_overrides_name_and_prefixes_variants() {
        assert_eq!(
            ids(r#"<map><name>Big Name</name><slug>tiny</slug>
                 <variant id="winter" world="w">Winter</variant></map>"#),
            [
                ("tiny".into(), None),
                ("tiny_winter".into(), Some("w".into()))
            ]
        );
    }

    #[test]
    fn conditional_slugs_split_default_from_variant() {
        assert_eq!(
            ids(r#"<map><name>Boletum</name>
                 <if variant="default"><slug>boletum_remix</slug></if>
                 <variant id="classic" world="classic" slug="boletum">Classic</variant></map>"#),
            [
                ("boletum_remix".into(), None),
                ("boletum".into(), Some("classic".into()))
            ]
        );
        assert_eq!(
            ids(r#"<map><name>Boombox</name>
                 <unless variant="classic"><slug>boombox_redux</slug></unless>
                 <variant id="classic" world="classic" slug="boombox">Classic</variant></map>"#),
            [
                ("boombox_redux".into(), None),
                ("boombox".into(), Some("classic".into()))
            ]
        );
    }

    #[test]
    fn unless_slug_does_not_leak_onto_unlisted_variants() {
        assert_eq!(
            ids(r#"<map><name>Boombox</name>
                 <unless variant="classic"><slug>boombox_redux</slug></unless>
                 <variant id="classic" world="classic" slug="boombox">Classic</variant>
                 <variant id="night" world="night">Night</variant></map>"#),
            [
                ("boombox_redux".into(), None),
                ("boombox".into(), Some("classic".into())),
                ("boombox_redux_night".into(), Some("night".into()))
            ]
        );
    }

    #[test]
    fn conditional_slug_beats_root_slug_consistently() {
        assert_eq!(
            ids(r#"<map><name>Boombox</name><slug>base</slug>
                 <unless variant="classic"><slug>redux</slug></unless>
                 <variant id="classic" world="classic">Classic</variant>
                 <variant id="night" world="night">Night</variant></map>"#),
            [
                ("redux".into(), None),
                ("base_classic".into(), Some("classic".into())),
                ("redux_night".into(), Some("night".into()))
            ]
        );
    }

    #[test]
    fn variant_declared_inside_conditional_is_kept() {
        assert_eq!(
            ids(r#"<map><name>Runic Altar</name>
                 <if variant="ctf"><variant id="ctf" world="ctf">CTF</variant></if></map>"#),
            [
                ("runic_altar".into(), None),
                ("runic_altar_ctf".into(), Some("ctf".into()))
            ]
        );
    }

    #[test]
    fn dedup_suffixes_avoid_names_other_maps_own() {
        let folder = PathBuf::from("f");
        let mut maps: Vec<(&PathBuf, MapFolder)> = ["foo", "foo", "foo_2", "foo_2"]
            .iter()
            .map(|n| {
                (
                    &folder,
                    MapFolder {
                        name: (*n).into(),
                        world_dir: PathBuf::new(),
                    },
                )
            })
            .collect();
        dedup_names(&mut maps);
        let names: Vec<_> = maps.iter().map(|(_, m)| m.name.as_str()).collect();
        assert_eq!(names, ["foo", "foo_3", "foo_2", "foo_2_2"]);
    }

    #[test]
    fn override_variant_replaces_name() {
        assert_eq!(
            ids(r#"<map><name>Water Drop: Directrix</name>
                 <variant id="default" world="default"/>
                 <variant id="ranch" world="ranch" override="true">Water Drop: King Ranch</variant>
                 </map>"#),
            [
                ("water_drop_directrix".into(), Some("default".into())),
                ("water_drop_king_ranch".into(), Some("ranch".into()))
            ]
        );
    }

    #[test]
    fn suffix_variant_and_slug_attr() {
        assert_eq!(
            ids(r#"<map><name>Runic Altar</name>
                 <variant id="ctf">CTF</variant>
                 <variant id="x" slug="custom_slug">X</variant></map>"#),
            [
                ("runic_altar".into(), None),
                ("runic_altar_ctf".into(), None),
                ("custom_slug".into(), None)
            ]
        );
    }

    #[test]
    fn server_version_conditionals_pick_newest() {
        assert_eq!(
            ids(r#"<map><name>Runic Altar</name>
                 <if min-server-version="1.21.11"><variant world="modern" id="default"/></if>
                 <unless min-server-version="1.21.11"><variant world="legacy" id="default"/></unless>
                 </map>"#),
            [("runic_altar".into(), Some("modern".into()))]
        );
    }

    #[test]
    fn map_folder_of_walks_up_to_map_xml() {
        let dir = std::env::temp_dir().join("mciso_test_map_folder");
        let region = dir.join("world/region");
        std::fs::create_dir_all(&region).unwrap();
        std::fs::write(dir.join("map.xml"), "").unwrap();
        let mca = region.join("r.0.0.mca");
        assert_eq!(map_folder_of(&mca), Some(dir.clone()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
