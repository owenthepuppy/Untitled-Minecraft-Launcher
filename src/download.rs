use crate::meta::{AssetObjects, VersionDetail, wanted};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::Digest;
use std::fs::{File, create_dir_all};
use std::io::copy;
use std::path::Path;
use ureq::get;
pub fn download(url: &str, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let mut res = get(url).call()?;
    let mut file = File::create(path)?;
    copy(&mut res.body_mut().as_reader(), &mut file)?;
    Ok(())
}

pub fn client(detail: &VersionDetail, shared_dir: &Path, version_id: &str) -> anyhow::Result<()> {
    let out = shared_dir
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.jar"));
    if !out.exists() {
        println!("Downloading client: {}...", detail.downloads.client.url);
        download(&detail.downloads.client.url, &out)?;
    }
    Ok(())
}
pub fn libraries(detail: &VersionDetail, shared_dir: &Path) -> anyhow::Result<()> {
    println!("Downloading libraries...");
    let libs_dir = shared_dir.join("libraries");
    let needed: Vec<_> = detail
        .libraries
        .iter()
        .filter(|l| wanted(l))
        .filter(|l| !libs_dir.join(&l.downloads.artifact.path).exists())
        .collect();

    if !needed.is_empty() {
        let pb = ProgressBar::new(needed.len() as u64);
        pb.set_style(ProgressStyle::with_template("{bar:40} {pos}/{len} {msg}").unwrap());
        for lib in &needed {
            let a = &lib.downloads.artifact;
            pb.set_message(a.path.clone());
            download(&a.url, &libs_dir.join(&a.path))?;
            pb.inc(1);
        }
        pb.finish_and_clear();
    }
    Ok(())
}
pub fn assets(detail: &VersionDetail, shared_dir: &Path) -> anyhow::Result<()> {
    println!("Downloading assets...");
    let index: AssetObjects = get(&detail.asset_index.url)
        .call()?
        .body_mut()
        .read_json()?;

    let index_path = shared_dir
        .join("assets")
        .join("indexes")
        .join(format!("{}.json", detail.asset_index.id));
    if !index_path.exists() {
        download(&detail.asset_index.url, &index_path)?;
    }

    let objects_dir = shared_dir.join("assets").join("objects");
    let pb = ProgressBar::new(index.objects.len() as u64);
    pb.set_style(ProgressStyle::with_template("{bar:40} {pos}/{len} ({eta})").unwrap());
    index
        .objects
        .par_iter()
        .progress_with(pb)
        .try_for_each(|(_, obj)| -> anyhow::Result<()> {
            let sub = &obj.hash[..2];
            let url = format!(
                "https://resources.download.minecraft.net/{sub}/{}",
                obj.hash
            );
            let path = objects_dir.join(sub).join(&obj.hash);
            if !path.exists() {
                download(&url, &path)?;
            }
            Ok(())
        })?;
    Ok(())
}
pub fn verify_sha512(path: &Path, expected: &str) -> anyhow::Result<bool> {
    let mut hasher = sha2::Sha512::new();
    std::io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()) == expected)
}
