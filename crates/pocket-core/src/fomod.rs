//! FOMOD installers (`fomod/ModuleConfig.xml`): many Cyberpunk mods ship variants and let the user pick. Supported:
//! required files, install steps with groups and plugins (SelectExactlyOne, SelectAtMostOne, SelectAtLeastOne,
//! SelectAll, SelectAny), plugin types (Required, Recommended, Optional, NotUsable), condition flags, and
//! conditional file installs driven by those flags (And/Or). Not supported: step visibility conditions (every step is
//! shown) and file or game-version dependencies (treated as met). The file is often UTF-16; both encodings are read.

use crate::cyberpunk::layout::{placement, Placement};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileMap { pub source: String, pub destination: String, pub folder: bool, pub priority: i32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin { pub name: String, pub description: String, pub image: Option<String>, pub files: Vec<FileMap>, pub flags: Vec<(String, String)>, pub kind: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group { pub name: String, pub kind: String, pub plugins: Vec<Plugin> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step { pub name: String, pub groups: Vec<Group> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conditional { pub flags: Vec<(String, String)>, pub any: bool, pub files: Vec<FileMap> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fomod {
    pub name: String,
    /// The folder in the archive that holds `fomod/` (sources are relative to it).
    pub root: String,
    pub required: Vec<FileMap>,
    pub steps: Vec<Step>,
    pub conditional: Vec<Conditional>,
}

/// An archive without an installer that holds several variants side by side ("2K/…" and "4K/…", each laid out
/// for the game, landing on the same paths): an installer with one choice, so the user picks one instead of every
/// variant going in and the last one silently winning. Folders that clash with no other go in whatever is picked.
pub fn variants(entries: &[String], mod_name: &str) -> Option<Fomod> {
    use crate::cyberpunk::layout::plan_install;
    let files: Vec<String> = entries.iter().map(|e| e.replace('\\', "/")).filter(|e| !e.ends_with('/')).collect();
    let anchors = ["archive", "bin", "r6", "red4ext", "engine", "mods"];
    // drop wrapper folders every file shares ("ModName/…")
    let mut depth = 0;
    loop {
        let seg = |f: &String| f.split('/').nth(depth).map(str::to_lowercase);
        let first = files.first().and_then(seg)?;
        let all_deeper = files.iter().all(|f| f.split('/').count() > depth + 1 && seg(f).as_deref() == Some(first.as_str()));
        if !all_deeper || anchors.contains(&first.as_str()) { break; }
        depth += 1;
    }
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &files {
        let parts: Vec<&str> = f.split('/').collect();
        if parts.len() <= depth + 1 { groups.entry(String::new()).or_default().push(f.clone()); continue; }
        groups.entry(parts[..=depth].join("/")).or_default().push(f.clone());
    }
    if groups.len() < 2 { return None; }
    let planned: Vec<(String, Vec<Placement>)> = groups.iter()
        .filter(|(k, _)| !k.is_empty() && !anchors.contains(&k.rsplit('/').next().unwrap_or("").to_lowercase().as_str()))
        .map(|(k, v)| (k.clone(), plan_install(v, mod_name).files)).filter(|(_, p)| !p.is_empty()).collect();
    let targets = |p: &[Placement]| p.iter().map(|x| x.target.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
    let clashes = |i: usize| planned.iter().enumerate().any(|(j, (_, q))| j != i && !targets(&planned[i].1).is_disjoint(&targets(q)));
    let (var, shared): (Vec<usize>, Vec<usize>) = (0..planned.len()).partition(|i| clashes(*i));
    if var.len() < 2 { return None; }
    let maps = |p: &[Placement]| p.iter().map(|x| FileMap { source: x.source.clone(), destination: x.target.clone(), folder: false, priority: 0 }).collect::<Vec<_>>();
    let mut required: Vec<FileMap> = shared.iter().flat_map(|i| maps(&planned[*i].1)).collect();
    // loose files beside the variant folders (and anchored folders like archive/) go in too
    for (k, v) in &groups {
        if k.is_empty() || anchors.contains(&k.rsplit('/').next().unwrap_or("").to_lowercase().as_str()) { required.extend(maps(&plan_install(v, mod_name).files)); }
    }
    let plugins = var.iter().map(|i| {
        let (k, p) = &planned[*i];
        Plugin { name: k.rsplit('/').next().unwrap_or(k).to_string(), description: format!("{} files", p.len()), image: None, files: maps(p), flags: Vec::new(), kind: "Optional".into() }
    }).collect();
    Some(Fomod { name: mod_name.to_string(), root: String::new(), required,
        steps: vec![Step { name: "Choose a variant".into(), groups: vec![Group { name: "This archive holds several versions of the mod. Pick one.".into(), kind: "SelectExactlyOne".into(), plugins }] }],
        conditional: Vec::new() })
}

/// The ModuleConfig.xml entry in an archive listing, if any.
pub fn find_config(entries: &[String]) -> Option<String> {
    entries.iter().find(|e| e.replace('\\', "/").to_lowercase().ends_with("fomod/moduleconfig.xml")).cloned()
}

pub fn decode(bytes: &[u8]) -> String {
    let utf16 = |b: &[u8], le: bool| -> String {
        let units: Vec<u16> = b.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, true),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, false),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        // UTF-16 without a BOM: every other byte is zero for ASCII XML
        b if b.len() > 4 && b[1] == 0 && b[3] == 0 => utf16(b, true),
        b => String::from_utf8_lossy(b).into_owned(),
    }
}

fn child<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<roxmltree::Node<'a, 'a>> { n.children().find(|c| c.has_tag_name(name)) }
fn children<'a>(n: roxmltree::Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = roxmltree::Node<'a, 'a>> + 'a { n.children().filter(move |c| c.has_tag_name(name)) }

fn files_of(n: Option<roxmltree::Node>) -> Vec<FileMap> {
    let Some(n) = n else { return Vec::new() };
    n.children().filter(|c| c.has_tag_name("file") || c.has_tag_name("folder")).map(|c| FileMap {
        source: c.attribute("source").unwrap_or("").replace('\\', "/").trim_matches('/').to_string(),
        destination: c.attribute("destination").map(|d| d.replace('\\', "/").trim_matches('/').to_string()).unwrap_or_else(|| "\u{0}".into()),
        folder: c.has_tag_name("folder"),
        priority: c.attribute("priority").and_then(|p| p.parse().ok()).unwrap_or(0),
    }).collect()
}

pub fn parse(xml: &str, config_path: &str) -> Result<Fomod> {
    let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).map_err(|e| Error::Invalid(format!("The mod's installer (ModuleConfig.xml) could not be read: {e}")))?;
    let root_el = doc.root_element();
    let root = config_path.replace('\\', "/");
    let root = root[..root.to_lowercase().rfind("fomod/moduleconfig.xml").unwrap_or(0)].trim_end_matches('/').to_string();
    let name = child(root_el, "moduleName").and_then(|n| n.text()).unwrap_or("").trim().to_string();
    let required = files_of(child(root_el, "requiredInstallFiles"));
    let mut steps = Vec::new();
    if let Some(st) = child(root_el, "installSteps") {
        for s in children(st, "installStep") {
            let mut groups = Vec::new();
            for g in child(s, "optionalFileGroups").into_iter().flat_map(|o| children(o, "group")) {
                let plugins = child(g, "plugins").into_iter().flat_map(|p| children(p, "plugin")).map(|p| Plugin {
                    name: p.attribute("name").unwrap_or("").to_string(),
                    description: child(p, "description").and_then(|d| d.text()).unwrap_or("").trim().to_string(),
                    image: child(p, "image").and_then(|i| i.attribute("path")).map(|x| x.replace('\\', "/")),
                    files: files_of(child(p, "files")),
                    flags: child(p, "conditionFlags").into_iter().flat_map(|f| children(f, "flag")).map(|f| (f.attribute("name").unwrap_or("").to_string(), f.text().unwrap_or("").trim().to_string())).collect(),
                    kind: child(p, "typeDescriptor").and_then(|t| t.descendants().find(|d| d.has_tag_name("type"))).and_then(|t| t.attribute("name")).unwrap_or("Optional").to_string(),
                }).collect();
                groups.push(Group { name: g.attribute("name").unwrap_or("").to_string(), kind: g.attribute("type").unwrap_or("SelectAny").to_string(), plugins });
            }
            steps.push(Step { name: s.attribute("name").unwrap_or("").to_string(), groups });
        }
    }
    let mut conditional = Vec::new();
    for p in child(root_el, "conditionalFileInstalls").and_then(|c| child(c, "patterns")).into_iter().flat_map(|p| children(p, "pattern")) {
        let deps = child(p, "dependencies");
        let any = deps.and_then(|d| d.attribute("operator")).is_some_and(|o| o.eq_ignore_ascii_case("or"));
        let flags = deps.into_iter().flat_map(|d| d.descendants().filter(|x| x.has_tag_name("flagDependency")))
            .map(|f| (f.attribute("flag").unwrap_or("").to_string(), f.attribute("value").unwrap_or("").to_string())).collect();
        conditional.push(Conditional { flags, any, files: files_of(child(p, "files")) });
    }
    Ok(Fomod { name, root, required, steps, conditional })
}

/// The choice a fresh dialog starts with: required and recommended plugins, and the first of each pick-one group.
pub fn defaults(f: &Fomod) -> Vec<[usize; 3]> {
    let mut out = Vec::new();
    for (si, s) in f.steps.iter().enumerate() {
        for (gi, g) in s.groups.iter().enumerate() {
            let mut picked: Vec<usize> = g.plugins.iter().enumerate().filter(|(_, p)| p.kind == "Required" || p.kind == "Recommended" || g.kind == "SelectAll").map(|(i, _)| i).collect();
            if picked.is_empty() && (g.kind == "SelectExactlyOne" || g.kind == "SelectAtLeastOne") {
                if let Some(i) = g.plugins.iter().position(|p| p.kind != "NotUsable") { picked.push(i); }
            }
            if g.kind == "SelectExactlyOne" || g.kind == "SelectAtMostOne" { picked.truncate(1); }
            out.extend(picked.into_iter().map(|pi| [si, gi, pi]));
        }
    }
    out
}

/// Where each chosen file goes: `(path in the archive, path in the game)`, the later or higher-priority file winning.
pub fn resolve(f: &Fomod, choices: &[[usize; 3]], entries: &[String]) -> Result<Vec<Placement>> {
    let mut maps: Vec<FileMap> = f.required.clone();
    let mut flags: BTreeMap<String, String> = BTreeMap::new();
    for [si, gi, pi] in choices {
        let p = f.steps.get(*si).and_then(|s| s.groups.get(*gi)).and_then(|g| g.plugins.get(*pi))
            .ok_or_else(|| Error::Invalid("That installer choice does not exist.".into()))?;
        maps.extend(p.files.iter().cloned());
        for (k, v) in &p.flags { flags.insert(k.clone(), v.clone()); }
    }
    for c in &f.conditional {
        let met = |(k, v): &(String, String)| flags.get(k).is_some_and(|x| x == v);
        let ok = if c.any { c.flags.iter().any(met) } else { c.flags.iter().all(met) };
        if ok { maps.extend(c.files.iter().cloned()); }
    }
    maps.sort_by_key(|m| m.priority);

    let norm = |s: &str| s.replace('\\', "/").trim_matches('/').to_lowercase();
    let prefix = if f.root.is_empty() { String::new() } else { format!("{}/", norm(&f.root)) };
    let index: Vec<(String, &String)> = entries.iter().map(|e| (norm(e), e)).collect();
    let mut out: BTreeMap<String, Placement> = BTreeMap::new();
    for m in maps {
        let src = format!("{prefix}{}", norm(&m.source));
        let dest = if m.destination == "\u{0}" { if m.folder { String::new() } else { m.source.clone() } } else { m.destination.clone() };
        if m.folder {
            let under = if src.ends_with('/') || src.is_empty() { src.clone() } else { format!("{src}/") };
            for (k, orig) in &index {
                if let Some(rest) = k.strip_prefix(&under) {
                    let real_rest = &orig.replace('\\', "/")[orig.len() - rest.len()..];
                    let target = if dest.is_empty() { real_rest.to_string() } else { format!("{dest}/{real_rest}") };
                    if let Some(p) = placement(orig, &target) { out.insert(p.target.to_lowercase(), p); }
                }
            }
        } else if let Some((_, orig)) = index.iter().find(|(k, _)| *k == src) {
            if let Some(p) = placement(orig, &dest) { out.insert(p.target.to_lowercase(), p); }
        }
    }
    if out.is_empty() { return Err(Error::Invalid("Those choices install no files. Pick at least one option.".into())); }
    Ok(out.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_folders_become_one_choice() {
        let e: Vec<String> = ["Jacket/2K/archive/pc/mod/jacket.archive", "Jacket/4K/archive/pc/mod/jacket.archive", "Jacket/Shared/r6/tweaks/jacket.yaml", "Jacket/readme.txt"]
            .iter().map(|s| s.to_string()).collect();
        let f = variants(&e, "Jacket").expect("variants");
        let g = &f.steps[0].groups[0];
        assert_eq!(g.plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["2K", "4K"]);
        let picked = resolve(&f, &[[0, 0, 1]], &e).unwrap();
        let t: Vec<(&str, &str)> = picked.iter().map(|p| (p.source.as_str(), p.target.as_str())).collect();
        assert!(t.contains(&("Jacket/4K/archive/pc/mod/jacket.archive", "archive/pc/mod/jacket.archive")), "{t:?}");
        assert!(t.iter().any(|(_, d)| d.ends_with("jacket.yaml")), "shared files go in too: {t:?}");
        assert!(!t.iter().any(|(s, _)| s.contains("2K")));
        // one layout only: no choice
        assert!(variants(&["M/archive/pc/mod/a.archive".to_string(), "M/r6/scripts/a.reds".to_string()], "M").is_none());
    }

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <moduleName>Night Jacket</moduleName>
  <requiredInstallFiles><file source="common\readme.txt" destination="" /><folder source="core" destination="" /></requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Colour">
      <optionalFileGroups order="Explicit">
        <group name="Pick one" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Black"><description>Black leather</description><image path="fomod\black.png"/>
              <files><file source="black\jacket.archive" destination="archive\pc\mod\jacket.archive" /></files>
              <conditionFlags><flag name="colour">black</flag></conditionFlags>
              <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
            <plugin name="Red"><description>Red</description>
              <files><file source="red\jacket.archive" destination="archive\pc\mod\jacket.archive" /></files>
              <conditionFlags><flag name="colour">red</flag></conditionFlags>
              <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
  <conditionalFileInstalls><patterns><pattern>
    <dependencies operator="And"><flagDependency flag="colour" value="red" /></dependencies>
    <files><file source="extras\red.xl" destination="archive\pc\mod\red.xl" /></files>
  </pattern></patterns></conditionalFileInstalls>
</config>"#;

    fn entries() -> Vec<String> {
        ["Night Jacket/fomod/ModuleConfig.xml", "Night Jacket/fomod/black.png", "Night Jacket/common/readme.txt", "Night Jacket/core/r6/scripts/NJ/nj.reds",
         "Night Jacket/black/jacket.archive", "Night Jacket/red/jacket.archive", "Night Jacket/extras/red.xl"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn utf16_with_bom_is_read() {
        let mut b = vec![0xFF, 0xFE];
        for u in XML.encode_utf16() { b.extend_from_slice(&u.to_le_bytes()); }
        assert!(decode(&b).contains("<moduleName>Night Jacket</moduleName>"));
    }

    #[test]
    fn parses_and_resolves_choices_and_flags() {
        let e = entries();
        let cfg = find_config(&e).unwrap();
        let f = parse(XML, &cfg).unwrap();
        assert_eq!((f.name.as_str(), f.root.as_str()), ("Night Jacket", "Night Jacket"));
        assert_eq!(f.steps[0].groups[0].plugins.len(), 2);
        assert_eq!(defaults(&f), [[0, 0, 0]], "the first of a pick-one group");

        let black = resolve(&f, &[[0, 0, 0]], &e).unwrap();
        let t: Vec<(&str, &str)> = black.iter().map(|p| (p.source.as_str(), p.target.as_str())).collect();
        assert!(t.contains(&("Night Jacket/black/jacket.archive", "archive/pc/mod/jacket.archive")));
        assert!(t.contains(&("Night Jacket/core/r6/scripts/NJ/nj.reds", "r6/scripts/NJ/nj.reds")));
        assert!(!t.iter().any(|(_, d)| d.ends_with("red.xl")), "the red extra needs the red flag");
        assert!(!t.iter().any(|(_, d)| d.ends_with("readme.txt")), "docs are left out");

        let red = resolve(&f, &[[0, 0, 1]], &e).unwrap();
        assert!(red.iter().any(|p| p.source == "Night Jacket/red/jacket.archive"));
        assert!(red.iter().any(|p| p.target == "archive/pc/mod/red.xl"));
    }
}
