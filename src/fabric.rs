use std::collections::HashSet;

use crate::meta::{Library, VersionDetail};
use anyhow::Context;
use serde::Deserialize;
use ureq::get;

const META: &str = "https://meta.fabricmc.net/v2/versions/loader";

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub main_class: String,
    pub libraries: Vec<FabricLibrary>,
}

#[derive(Deserialize)]
pub struct FabricLibrary {
    name: String,
    url: String,
}
fn loaders(mc_version: &str) -> anyhow::Result<Vec<LoaderEntry>> {
    let url = format!("{META}/{mc_version}");
    Ok(get(&url).call()?.body_mut().read_json()?)
}

pub fn latest_loader(mc: &str) -> anyhow::Result<String> {
    let entries = loaders(mc)?;
    entries
        .iter()
        .find(|e| e.loader.stable)
        .or(entries.first())
        .map(|e| e.loader.version.clone())
        .ok_or_else(|| anyhow::anyhow!("no fabric loader for {mc}"))
}

pub fn check_loader(mc: &str, loader: &str) -> anyhow::Result<()> {
    if !loaders(mc)?.iter().any(|e| e.loader.version == loader) {
        anyhow::bail!("fabric loader {loader} isn't available for {mc}");
    }
    Ok(())
}
fn maven_to_path(coord: &str) -> anyhow::Result<String> {
    let mut parts = coord.split(':');
    let group = parts.next().context("bad coord")?;
    let artifact = parts.next().context("bad coord")?;
    let version = parts.next().context("bad coord")?;
    Ok(format!(
        "{}/{artifact}/{version}/{artifact}-{version}.jar",
        group.replace('.', "/")
    ))
}
pub fn fetch_profile(mc: &str, loader: &str) -> anyhow::Result<Profile> {
    let url = format!("{META}/{mc}/{loader}/profile/json");
    Ok(get(&url).call()?.body_mut().read_json()?)
}
fn coord_key(name: &str) -> &str {
    match name.match_indices(':').nth(1) {
        Some((i, _)) => &name[..i],
        None => name,
    }
}
pub fn merge(detail: &mut VersionDetail, profile: Profile) -> anyhow::Result<()> {
    let mut libs = Vec::new();
    for fl in &profile.libraries {
        let path = maven_to_path(&fl.name)?;
        let url = format!("{}{path}", fl.url);
        libs.push(Library::unconditional(fl.name.clone(), url, path));
    }
    let keys: HashSet<&str> = libs.iter().map(|l| coord_key(&l.name)).collect();
    detail
        .libraries
        .retain(|l| !keys.contains(coord_key(&l.name)));
    libs.append(&mut detail.libraries);
    detail.libraries = libs;
    detail.main_class = profile.main_class;
    Ok(())
}
