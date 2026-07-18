use crate::{
    download::download,
    download::verify_sha512,
    instance::{Loader, create, load, save},
};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::Write, path::Path};
use zip::ZipArchive;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct Index {
    format_version: u32,
    game: String, // now what is modrinth up to?
    version_id: String,
    name: String,
    files: Vec<PackFile>,
    dependencies: HashMap<String, String>,
}
#[derive(Deserialize)]
struct PackFile {
    path: String,
    hashes: Hashes,
    downloads: Vec<String>,
    #[serde(default)]
    env: Option<Env>,
}
#[derive(Deserialize)]
struct Hashes {
    sha512: String,
}

#[derive(Deserialize)]
struct Env {
    client: String,
}

fn extract_overrides(
    archive: &mut ZipArchive<File>,
    prefix: &str,
    instance_dir: &Path,
) -> anyhow::Result<()> {
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        let Some(rel) = name.strip_prefix(prefix) else {
            continue;
        };
        if rel.is_empty() || entry.is_dir() {
            continue;
        }
        if rel.contains("..") || rel.starts_with('/') {
            anyhow::bail!("suspicious path in pack: {name}");
        }

        let dest = instance_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

pub fn import(
    pack: &Path,
    instances_dir: &Path,
    name: Option<&str>,
    yes: bool,
) -> anyhow::Result<()> {
    let mut archive = ZipArchive::new(File::open(pack)?)?;
    let index_file = archive.by_name("modrinth.index.json")?;
    let index: Index = serde_json::from_reader(index_file)?;
    if index.format_version != 1 {
        anyhow::bail!("unsupported mrpack format version {}", index.format_version);
    }

    let instance_name = match name {
        Some(n) => n.to_string(),
        None => crate::instance::sanitize(&index.name),
    };
    let mc = index
        .dependencies
        .get("minecraft")
        .ok_or_else(|| anyhow::anyhow!("pack has no minecraft version"))?;

    let loader = if let Some(v) = index.dependencies.get("fabric-loader") {
        Some(Loader {
            kind: "fabric".to_string(),
            version: v.clone(),
        })
    } else if index.dependencies.contains_key("forge")
        || index.dependencies.contains_key("neoforge")
    {
        anyhow::bail!("Forge modpacks are not currently supported.");
    } else if index.dependencies.contains_key("quilt-loader") {
        anyhow::bail!("Quilt modpacks are not currently supported.");
    } else {
        println!(
            "This modpack either doesn't have a modloader, or uses one that is unsupported. This is fine, but any mods won't load."
        );
        None
    };

    let as_name = match name {
        Some(n) => format!(" as {n}"),
        None => String::new(),
    };

    let wanted: Vec<_> = index
        .files
        .iter()
        .filter(|f| f.env.as_ref().map_or(true, |e| e.client != "unsupported"))
        .collect();

    let loader_bit = match &loader {
        Some(l) => format!(", which will use {} with {} mods", l.kind, wanted.len()),
        None => String::new(),
    };

    println!(
        "You are importing the modpack {}{}, this modpack will use minecraft version {}{}.",
        index.name, as_name, mc, loader_bit
    );
    if !yes {
        print!("Continue? [y/N] ");
        std::io::stdout().flush()?;

        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;

        if !answer.trim().eq_ignore_ascii_case("y") {
            anyhow::bail!("aborted");
        }
    }
    println!("Creating instance...");
    let instance_dir = create(instances_dir, &instance_name, mc)?;
    let (mut cfg, _) = load(instances_dir, &instance_name)?;
    cfg.loader = loader;
    save(&instance_dir, &cfg)?;
    println!("Downloading files...");
    let pb = ProgressBar::new(wanted.len() as u64);
    pb.set_style(ProgressStyle::with_template("{bar:40} {pos}/{len} {msg}").unwrap());

    for f in &wanted {
        if f.path.contains("..") || f.path.starts_with('/') {
            anyhow::bail!("suspicious path in pack: {}", f.path);
        }
        pb.set_message(f.path.clone());
        let dest = instance_dir.join(&f.path);
        if !dest.exists() || !verify_sha512(&dest, &f.hashes.sha512)? {
            let url = f
                .downloads
                .first()
                .ok_or_else(|| anyhow::anyhow!("no download url for {}", f.path))?;
            download(url, &dest)?;
            if !verify_sha512(&dest, &f.hashes.sha512)? {
                anyhow::bail!("hash mismatch after download: {}", f.path);
            }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();
    println!("Copying overrides...");
    extract_overrides(&mut archive, "overrides/", &instance_dir)?;
    extract_overrides(&mut archive, "client-overrides/", &instance_dir)?;
    println!("The modpack has (probably) been installed successfully!");
    Ok(())
}
