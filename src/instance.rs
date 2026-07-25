use serde::{Deserialize, Serialize};
use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
};

use crate::util::is_valid_name;
#[derive(Serialize, Deserialize)]
pub struct Instance {
    pub version: String,
    #[serde(default)]
    pub loader: Option<Loader>,
}

#[derive(Serialize, Deserialize)]
pub struct Loader {
    pub kind: String,
    pub version: String,
}

pub fn create(instances_dir: &Path, name: &str, version: &str) -> anyhow::Result<PathBuf> {
    if !is_valid_name(name) {
        anyhow::bail!("invalid instance name: {name:?}");
    }
    let directory = instances_dir.join(name);
    if directory.join("instance.json").exists() {
        anyhow::bail!("instance already exists here!");
    }
    create_dir_all(&directory)?;
    let cfg = Instance {
        version: version.to_string(),
        loader: None,
    };
    save(&directory, &cfg)?;
    Ok(directory)
}
pub fn save(dir: &Path, cfg: &Instance) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(dir.join("instance.json"), json)?;
    Ok(())
}

pub fn load(root: &Path, name: &str) -> anyhow::Result<(Instance, PathBuf)> {
    let dir = root.join(name);
    let cfg_path = dir.join("instance.json");
    if !cfg_path.exists() {
        anyhow::bail!("no instance named {name}");
    }
    let json = std::fs::read_to_string(cfg_path)?;
    Ok((serde_json::from_str(&json)?, dir))
}
pub fn list(root: &Path) -> anyhow::Result<Vec<String>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().join("instance.json").exists() {
            out.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    Ok(out)
}
pub fn remove(instances_dir: &Path, name: &str, yes: bool) -> anyhow::Result<()> {
    if !is_valid_name(name) {
        anyhow::bail!("invalid instance name: {name:?}");
    }
    let dir = instances_dir.join(&name);
    if !dir.exists() {
        anyhow::bail!("no instance named {name}");
    }
    if !yes {
        print!("Delete {name} and all its worlds? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut a = String::new();
        std::io::stdin().read_line(&mut a)?;
        if !a.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("aborted");
        }
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
pub fn rename(instances_dir: &Path, old_name: &str, new_name: &str) -> anyhow::Result<()> {
    if !is_valid_name(old_name) {
        anyhow::bail!("invalid original instance name: {old_name:?}");
    }
    if !is_valid_name(new_name) {
        anyhow::bail!("invalid new instance name: {new_name:?}");
    }
    let instance_dir = instances_dir.join(&old_name);
    let new_dir = instances_dir.join(&new_name);
    if !instance_dir.exists() {
        anyhow::bail!("original instance name doesn't exist");
    }
    if new_dir.exists() {
        anyhow::bail!("new instance name already exists");
    }
    std::fs::rename(instance_dir, new_dir)?;

    Ok(())
}
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.', ' '])
        .to_string()
}
