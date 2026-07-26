use std::path::Path;
pub struct ModEntry {
    pub filename: String,
    pub enabled: bool,
    pub id: String,
    pub name: Option<String>,
}
#[derive(serde::Deserialize)]
struct FabricModJson {
    id: String,
    #[serde(default)]
    name: Option<String>,
}

fn read_mod_meta(jar_path: &Path) -> Option<(String, Option<String>)> {
    let file = std::fs::File::open(jar_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let entry = archive.by_name("fabric.mod.json").ok()?;
    let meta: FabricModJson = serde_json::from_reader(entry).ok()?;
    Some((meta.id, meta.name))
}
fn entry_from_name(dir: &Path, name: String) -> Option<ModEntry> {
    let jar = dir.join(&name);
    let enabled = if name.ends_with(".jar") {
        true
    } else if name.ends_with(".jar.disabled") {
        false
    } else {
        return None;
    };
    let (id, pretty) = match read_mod_meta(&jar) {
        Some(info) => info,
        None => {
            let base = name
                .strip_suffix(".disabled")
                .unwrap_or(&name)
                .strip_suffix(".jar")
                .unwrap_or(&name)
                .to_string();
            (base, None)
        }
    };
    Some(ModEntry {
        filename: name,
        enabled,
        id,
        name: pretty,
    })
}
pub fn list(dir: &Path) -> anyhow::Result<Vec<ModEntry>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut mods = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let name = e?.file_name().to_string_lossy().to_string();
        if let Some(entry) = entry_from_name(dir, name) {
            mods.push(entry);
        }
    }
    mods.sort_by_key(|m| m.filename.clone());
    Ok(mods)
}
pub fn disable(dir: &Path, item: &str) -> anyhow::Result<()> {
    let entries = list(dir)?;
    let entry = entries
        .iter()
        .find(|m| m.id == item || m.filename == item)
        .ok_or_else(|| anyhow::anyhow!("no mod matching {item}"))?;
    if !entry.enabled {
        println!("{item} is already disabled");
        return Ok(());
    }
    let new_name = format!("{}.disabled", entry.filename);
    std::fs::rename(dir.join(&entry.filename), dir.join(&new_name))?;
    Ok(())
}

pub fn enable(dir: &Path, item: &str) -> anyhow::Result<()> {
    let entries = list(dir)?;
    let entry = entries
        .iter()
        .find(|m| m.id == item || m.filename == item)
        .ok_or_else(|| anyhow::anyhow!("no mod matching {item}"))?;
    if entry.enabled {
        println!("{item} is already enabled");
        return Ok(());
    }
    let new_name = entry.filename.strip_suffix(".disabled").unwrap();
    std::fs::rename(dir.join(&entry.filename), dir.join(new_name))?;
    Ok(())
}

pub fn delete(dir: &Path, item: &str) -> anyhow::Result<()> {
    let entries = list(dir)?;
    let entry = entries
        .iter()
        .find(|m| m.id == item || m.filename == item)
        .ok_or_else(|| anyhow::anyhow!("no mod matching {item}"))?;
    std::fs::remove_file(dir.join(&entry.filename))?;
    Ok(())
}
pub fn add(dir: &Path, source: &Path) -> anyhow::Result<()> {
    if !source.is_file() {
        anyhow::bail!("not a file: {}", source.display());
    }
    let filename = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("invalid source path"))?;
    let dest = dir.join(filename);
    if dest.exists() {
        anyhow::bail!("a mod named {:?} already exists", filename);
    }
    std::fs::copy(source, &dest)?;
    Ok(())
}
