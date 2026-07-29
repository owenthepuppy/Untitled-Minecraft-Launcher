use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{content, modrinth};
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Serialize, Deserialize)]
pub struct Overlay {
    pub name: String,
    pub mods: Vec<OverlayMod>,
}

#[derive(Serialize, Deserialize)]
pub struct OverlayMod {
    pub source: String,
    pub id: String,
    #[serde(default)]
    pub pin: Option<String>, // NOT IMPLEMENTED YET
}
pub fn load(overlays_dir: &Path, name: &str) -> anyhow::Result<Overlay> {
    let path = overlays_dir.join(format!("{name}.umloverlay"));
    if !path.exists() {
        anyhow::bail!("no overlay named {name}");
    }
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

pub fn save(overlays_dir: &Path, overlay: &Overlay) -> anyhow::Result<()> {
    let path = overlays_dir.join(format!("{}.umloverlay", overlay.name));
    let json = serde_json::to_string_pretty(overlay)?;
    std::fs::write(path, json)?;
    Ok(())
}
pub fn apply(
    overlays_dir: &Path,
    name: &str,
    mods_dir: &Path,
    loader: &str,
    game_version: &str,
) -> anyhow::Result<()> {
    let overlay = load(overlays_dir, name)?;
    let existing = content::list(mods_dir)?;
    let all_present = overlay
        .mods
        .iter()
        .all(|m| existing.iter().any(|e| e.id == m.id));
    if all_present {
        println!(
            "Overlay '{}' is already fully applied, nothing to do.",
            overlay.name
        );
        return Ok(());
    }
    let pb = ProgressBar::new(overlay.mods.len() as u64);
    pb.set_style(ProgressStyle::with_template("{bar:40} {pos}/{len} {msg}").unwrap());
    for m in overlay.mods {
        if existing.iter().any(|e| e.id == m.id) {
            println!("Skipping {} (already installed).", m.id);
            continue;
        }
        pb.println(format!("Downloading {}...", m.id)); // not println!
        let file = modrinth::get_version(&m.id, loader, game_version)?;
        modrinth::download_mod(&file, &mods_dir)?;
        pb.inc(1);
    }
    pb.finish_with_message("done");
    Ok(())
}
