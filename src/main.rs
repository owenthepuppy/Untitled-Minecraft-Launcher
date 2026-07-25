#[cfg(not(target_os = "linux"))]
compile_error!("UML only supports Linux.");

mod auth; // this is painful, ai helped a bit btw (i try to use ai as little as possible, but this part's just ANNOYING)
mod download;
mod fabric;
mod instance;
mod launch;
mod meta;
mod mrpack;
mod prism;
mod util;
use clap::{Parser, Subcommand};
use launch::run;
use meta::fetch_version;
use std::path::{Path, PathBuf};

use crate::util::{is_valid_name, open_path};

/// Untitled Minecraft Launcher
#[derive(Parser)]
#[command(name = "uml")]
#[command(
    after_help = "For any instance names with spaces, other than the launch command, enclose it in double quotes."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Clone, clap::ValueEnum)]
enum LoaderKind {
    Fabric,
}
#[derive(Subcommand)]
enum Command {
    /// Launch an instance
    #[command(alias = "l")]
    Launch {
        #[arg(trailing_var_arg = true)]
        name: Vec<String>,
        // disabled until i redo the account system
        // #[arg(long)]
        // offline: bool,
    },
    /// Manage instances
    Instances {
        #[command(subcommand)]
        action: InstanceCmd,
    },
    /// Open an instance folder
    Folder { name: String },
}

#[derive(Subcommand)]
enum InstanceCmd {
    /// Create a new instance
    New {
        name: String,
        #[arg(long)]
        version: String,
    },
    /// Rename an instance
    Rename {
        current_name: String,
        new_name: String,
    },
    /// Import a Modrinth modpack
    Import {
        pack: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Import a Prism Launcher instance
    ImportPrism {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Install a mod loader on an instance
    Mod {
        name: String,
        #[arg(long, value_enum, default_value_t = LoaderKind::Fabric)]
        loader: LoaderKind,
        #[arg(long)]
        loader_version: Option<String>,
    },
    /// Unmod an instance
    Unmod { name: String },
    /// Remove an instance
    Remove {
        name: String,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// List instances
    List,
}
fn data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(Path::new(&xdg).join("uml"));
        }
    }
    let home = std::env::var("HOME")?;
    Ok(Path::new(&home).join(".local").join("share").join("uml"))
}

fn main() -> anyhow::Result<()> {
    let uml_dir = data_dir()?;
    let instances_dir = uml_dir.join("instances");
    let shared_dir = uml_dir.join("shared");

    let cli = Cli::parse();
    match cli.command {
        Command::Folder { name } => {
            if !is_valid_name(&name) {
                anyhow::bail!("invalid instance name: {name:?}");
            }
            let dir = instances_dir.join(&name);
            if !dir.exists() {
                anyhow::bail!("no instance named {name}");
            }
            open_path(&dir)?;
        }
        Command::Instances { action } => match action {
            InstanceCmd::New { name, version } => {
                if !meta::version_exists(&version)? {
                    anyhow::bail!("no such Minecraft version: {version}");
                }
                instance::create(&instances_dir, &name, &version)?;
            }
            InstanceCmd::Rename {
                current_name,
                new_name,
            } => {
                instance::rename(&instances_dir, &current_name, &new_name)?;
                println!("Renamed {} to {}.", current_name, new_name);
            }
            InstanceCmd::Import { pack, name, yes } => {
                mrpack::import(&pack, &instances_dir, name.as_deref(), yes)?;
            }
            InstanceCmd::ImportPrism { path, name } => {
                prism::import(&path, &instances_dir, name.as_deref())?;
            }
            InstanceCmd::Remove { name, yes } => {
                instance::remove(&instances_dir, &name, yes)?;
                println!("Removed {name}.");
            }
            InstanceCmd::List => {
                for n in instance::list(&instances_dir)? {
                    println!("{n}");
                }
            }
            InstanceCmd::Unmod { name } => {
                let (mut cfg, dir) = instance::load(&instances_dir, &name)?;
                if cfg.loader.is_none() {
                    anyhow::bail!("{name} has no loader");
                }
                cfg.loader = None;
                instance::save(&dir, &cfg)?;
                println!("Removed loader from {name}.");
            }
            InstanceCmd::Mod {
                name,
                loader,
                loader_version,
            } => {
                let (mut cfg, dir) = instance::load(&instances_dir, &name)?;

                let (kind, version) = match loader {
                    LoaderKind::Fabric => {
                        let v = match loader_version {
                            Some(v) => {
                                fabric::check_loader(&cfg.version, &v)?;
                                v
                            }
                            None => fabric::latest_loader(&cfg.version)?,
                        };
                        ("fabric", v)
                    }
                };

                cfg.loader = Some(instance::Loader {
                    kind: kind.to_string(),
                    version: version.clone(),
                });
                instance::save(&dir, &cfg)?;
                std::fs::create_dir_all(dir.join("mods"))?;
                println!("Installed {kind} {version} on {name}.");
            }
        },
        Command::Launch { name } => {
            //, offline } => {
            let name = name.join(" ");
            let (cfg, game_dir) = instance::load(&instances_dir, &name)?;
            let game_version = cfg.version;
            let (mut detail, version_type) = fetch_version(&game_version)?;

            // let account = if !offline {
            let account = auth::login()?;
            // } else {
            //     auth::gen_offline("TESTING-USER")
            // };

            if let Some(loader) = &cfg.loader {
                let profile = match loader.kind.as_str() {
                    "fabric" => fabric::fetch_profile(&game_version, &loader.version)?,
                    other => anyhow::bail!("unknown loader: {other}"),
                };
                fabric::merge(&mut detail, profile)?;
            }

            download::client(&detail, &shared_dir, &game_version)?;
            download::libraries(&detail, &shared_dir)?;
            download::assets(&detail, &shared_dir)?;

            // Congrats! you found the main part of the program!
            run(
                &detail,
                &game_dir,
                &shared_dir,
                &account,
                &version_type,
                &game_version,
            )?;
        }
    }
    Ok(())
}
