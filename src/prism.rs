use crate::instance::{Loader, create, load, save};
use serde::Deserialize;
use std::path::Path;
#[derive(Deserialize)]
struct MmcPack {
    components: Vec<Component>,
}

#[derive(Deserialize)]
struct Component {
    uid: String,
    #[serde(default)]
    version: Option<String>,
}

fn find<'a>(pack: &'a MmcPack, uid: &str) -> Option<&'a str> {
    pack.components
        .iter()
        .find(|c| c.uid == uid)
        .and_then(|c| c.version.as_deref())
}

fn copy_dir(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir(&from, &to)?; // recurse into subdirectories
        } else {
            std::fs::copy(&from, &to)?; // std's single-file copy
        }
    }
    Ok(())
}

pub fn import(prism_dir: &Path, instances_dir: &Path, name: Option<&str>) -> anyhow::Result<()> {
    let json = std::fs::read_to_string(prism_dir.join("mmc-pack.json"))?;
    let pack: MmcPack = serde_json::from_str(&json)?;

    let mc =
        find(&pack, "net.minecraft").ok_or_else(|| anyhow::anyhow!("no minecraft component"))?;

    let loader = if let Some(v) = find(&pack, "net.fabricmc.fabric-loader") {
        Some(Loader {
            kind: "fabric".into(),
            version: v.to_string(),
        })
    } else if find(&pack, "net.minecraftforge").is_some() || find(&pack, "net.neoforged").is_some()
    {
        anyhow::bail!("Forge instances aren't supported yet");
    } else if find(&pack, "org.quiltmc.quilt-loader").is_some() {
        anyhow::bail!("Quilt instances aren't supported yet");
    } else {
        None
    };

    let instance_name = match name {
        Some(n) => n.to_string(),
        None => prism_dir
            .file_name()
            .and_then(|s| s.to_str())
            .map(crate::instance::sanitize)
            .ok_or_else(|| anyhow::anyhow!("bad prism path"))?,
    };

    let dir = create(instances_dir, &instance_name, mc)?;
    let (mut cfg, _) = load(instances_dir, &instance_name)?;
    cfg.loader = loader;
    save(&dir, &cfg)?;

    let src = prism_dir.join("minecraft");
    if !src.exists() {
        anyhow::bail!("no minecraft in {}", prism_dir.display());
    }
    copy_dir(&src, &dir)?;

    println!("Imported {instance_name} (minecraft {mc})");
    Ok(())
}
