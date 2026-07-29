use std::path::Path;

use serde::Deserialize;

pub const USER_AGENT: &str = concat!("owenthepuppy/uml/", env!("CARGO_PKG_VERSION"));

pub fn is_valid_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(_)), None)
    )
}
pub fn is_safe_relative_path(path: &str) -> bool {
    let p = Path::new(path);
    !p.as_os_str().is_empty()
        && p.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
pub fn open_path(path: &Path) -> anyhow::Result<()> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}
#[derive(Deserialize, Clone)]
pub struct Hashes {
    pub sha512: String,
}
