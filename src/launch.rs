use crate::{
    auth::Account,
    meta::{Arg, ArgValue, VersionDetail, rules_allow, wanted},
};
use std::{collections::HashMap, fs::create_dir_all, path::Path, process::Command};

fn flatten(args: &[Arg]) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Arg::Plain(s) => out.push(s.clone()),
            Arg::Conditional { rules, value } => {
                if !rules_allow(rules) {
                    continue;
                }
                match value {
                    ArgValue::One(s) => out.push(s.clone()),
                    ArgValue::Many(v) => out.extend(v.iter().cloned()),
                }
            }
        }
    }
    out
}
fn substitute(tokens: Vec<String>, vars: &HashMap<&str, String>) -> Vec<String> {
    tokens
        .into_iter()
        .map(|mut t| {
            for (k, v) in vars {
                t = t.replace(k, v);
            }
            t
        })
        .collect()
}
pub fn run(
    detail: &VersionDetail,
    game_dir: &Path,
    shared_dir: &Path,
    account: &Account,
    version_type: &str,
    version_id: &str,
) -> anyhow::Result<()> {
    let client_jar = shared_dir
        .join("versions")
        .join(version_id)
        .join(format!("{version_id}.jar"));

    let mut vars = HashMap::new();

    let mut classpath: Vec<String> = detail
        .libraries
        .iter()
        .filter(|l| wanted(l))
        .map(|l| {
            shared_dir
                .join("libraries")
                .join(&l.downloads.artifact.path)
                .display()
                .to_string()
        })
        .collect();
    classpath.push(client_jar.display().to_string());

    vars.insert("${classpath}", classpath.join(":"));
    vars.insert(
        "${natives_directory}",
        shared_dir.join("natives").display().to_string(),
    );
    vars.insert("${auth_player_name}", account.name.clone());
    vars.insert("${launcher_name}", "uml".to_string());
    vars.insert("${launcher_version}", "1.0".to_string());
    vars.insert("${version_name}", version_id.to_string());
    vars.insert("${game_directory}", game_dir.display().to_string());
    vars.insert(
        "${assets_root}",
        shared_dir.join("assets").display().to_string(),
    );
    vars.insert("${assets_index_name}", detail.asset_index.id.clone());
    vars.insert("${auth_uuid}", account.uuid.clone());
    vars.insert("${auth_access_token}", account.access_token.clone());
    vars.insert("${clientid}", String::new());
    vars.insert("${auth_xuid}", account.xuid.clone());
    vars.insert("${version_type}", version_type.to_string());

    let jvm_args = substitute(flatten(&detail.arguments.jvm), &vars);
    let game_args = substitute(flatten(&detail.arguments.game), &vars);

    for sub in ["java", "jna", "lwjgl", "netty"] {
        create_dir_all(shared_dir.join("natives").join(sub))?;
    }

    let status = Command::new("java") // This is what makes UML, UML! the heart of the program.
        .current_dir(game_dir)
        .args(&jvm_args)
        .arg(&detail.main_class)
        .args(&game_args)
        .status()?;
    if !status.success() {
        anyhow::bail!("minecraft exited with {status}");
    }

    Ok(())
}
