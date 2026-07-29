use std::path::Path;

use crate::{
    download::{self, verify_sha512},
    util::Hashes,
};
use serde::Deserialize;

const USER_AGENT: &str = "owenthepuppy/uml/0.3.0";

#[derive(Deserialize)]
pub struct Version {
    pub files: Vec<VersionFile>,
}
#[derive(Deserialize, Clone)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    pub hashes: Hashes,
    pub primary: bool,
}
pub fn get_versions(slug: &str, loader: &str, game_version: &str) -> anyhow::Result<Vec<Version>> {
    let versions = match ureq::get(format!(
        "https://api.modrinth.com/v2/project/{slug}/version"
    ))
    .header("User-Agent", USER_AGENT)
    .query("loaders", format!("[\"{loader}\"]"))
    .query("game_versions", format!("[\"{game_version}\"]"))
    .call()
    {
        Ok(mut resp) => resp.body_mut().read_json()?,
        Err(ureq::Error::StatusCode(404)) => {
            anyhow::bail!("mod not found: {slug}");
        }
        Err(e) => return Err(e.into()),
    };
    Ok(versions)
}
pub fn get_version(slug: &str, loader: &str, game_version: &str) -> anyhow::Result<VersionFile> {
    let mod_versions = get_versions(slug, loader, game_version)?;
    let latest_version = mod_versions
        .first()
        .ok_or_else(|| anyhow::anyhow!("no compatible version for {slug}"))?;
    let file = latest_version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| latest_version.files.first())
        .ok_or_else(|| anyhow::anyhow!("version has no files"))?;
    Ok(file.clone())
}
pub fn download_mod(file: &VersionFile, mods_dir: &Path) -> anyhow::Result<()> {
    let dest = mods_dir.join(&file.filename);
    download::download(&file.url, &dest)?;
    if !verify_sha512(&dest, &file.hashes.sha512)? {
        anyhow::bail!(
            "hash mismatch after download: {:?}. This file is NOT DELETED AND WILL LOAD ON NEXT LAUNCH. DO NOT LAUNCH MINECRAFT, until you are SURE this is safe.",
            dest
        );
    }
    Ok(())
}
