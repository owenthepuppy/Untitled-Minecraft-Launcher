use serde::Deserialize;
use std::collections::HashMap;
use ureq::get;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
pub struct Manifest {
    pub versions: Vec<Version>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Version {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
    pub compliance_level: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientDetails {
    pub url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainDownloads {
    pub client: ClientDetails,
}
#[derive(Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: LibraryDownloads,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Artifact,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Artifact {
    pub url: String,
    pub sha1: Option<String>,
    pub path: String,
}

#[derive(Deserialize)]
pub struct Rule {
    action: String,
    #[serde(default)]
    os: Option<Os>,
    #[serde(default)]
    features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize)]
struct Os {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arch: Option<String>,
}
#[derive(Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
}
#[derive(Deserialize)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
pub struct AssetObject {
    pub hash: String,
}

#[derive(Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDetail {
    pub downloads: MainDownloads,
    pub libraries: Vec<Library>,
    pub asset_index: AssetIndex,
    pub arguments: Arguments,
    pub main_class: String,
}

impl Library {
    pub fn unconditional(name: String, url: String, path: String) -> Self {
        Library {
            downloads: LibraryDownloads {
                artifact: Artifact {
                    url,
                    path,
                    sha1: None,
                },
            },
            rules: Vec::new(),
            name: name,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

fn os_applies(os: &Os) -> bool {
    if let Some(name) = &os.name {
        if name != "linux" {
            return false;
        }
    }
    if let Some(arch) = &os.arch {
        if arch != "x86_64" {
            return false;
        }
    }
    true
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    let mut allowed = rules.is_empty();
    for r in rules {
        let applies = r.features.is_none() && r.os.as_ref().map_or(true, os_applies);
        if applies {
            allowed = r.action == "allow";
        }
    }
    allowed
}

pub fn wanted(lib: &Library) -> bool {
    rules_allow(&lib.rules)
}

pub fn fetch_version(version_id: &str) -> anyhow::Result<(VersionDetail, String)> {
    let manifest: Manifest = get(MANIFEST_URL).call()?.body_mut().read_json()?;
    let version = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| anyhow::anyhow!("version {version_id} not found"))?;
    let detail: VersionDetail = get(&version.url).call()?.body_mut().read_json()?;
    Ok((detail, version.kind.clone()))
}
pub fn version_exists(version_id: &str) -> anyhow::Result<bool> {
    let manifest: Manifest = get(MANIFEST_URL).call()?.body_mut().read_json()?;
    Ok(manifest.versions.iter().any(|v| v.id == version_id))
}
