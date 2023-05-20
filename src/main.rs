use std::path::PathBuf;

use argh::FromArgs;
use color_eyre::{eyre::eyre, Help};
use config::{Config, GeneralConfig, ProjectConfig};
use log::LevelFilter;

mod cache;
mod config;
mod pack;
mod pki;

#[derive(FromArgs, PartialEq, Debug)]
/// CLI to update a patch server
struct Args {
    #[argh(subcommand)]
    nested: Commands,
    #[argh(option, short = 'p')]
    /// select a specific project
    project: Option<String>,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum Commands {
    Cache(cache::Args),
    Pack(pack::Args),
    PKI(pki::Args),
}

#[derive(PartialEq, Debug)]
/// arguments with a project
pub struct ProjectArgs<'a, A> {
    dir: PathBuf,
    general: GeneralConfig,
    project: &'a ProjectConfig,
    name: &'a str,
    cmd: A,
}

impl<'a, A> ProjectArgs<'a, A> {
    pub fn new(
        dir: PathBuf,
        general: GeneralConfig,
        project: &'a ProjectConfig,
        name: &'a str,
        cmd: A,
    ) -> Self {
        Self {
            dir,
            general,
            project,
            name,
            cmd,
        }
    }
}

fn main() -> color_eyre::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let args: Args = argh::from_env();
    //let dir = std::env::current_dir().wrap_err("Failed to get current directory")?;
    let dir = PathBuf::from(".");
    let config = Config::from_file(dir.join("LUpdate.toml"))?;

    let (name, project) = if let Some(key) = args.project.as_ref() {
        if let Some(p) = config.project.get(key) {
            (key, p)
        } else {
            return Err(eyre!("Project {:?} not found!", key));
        }
    } else if config.project.len() == 1 {
        config.project.iter().next().unwrap()
    } else {
        return Err(eyre!("More than one project found!"))
            .with_suggestion(|| "Please specify using `-p <name>".to_string());
    };

    log::info!("Using project {:?}", name);

    match args.nested {
        Commands::Cache(cmd) => {
            cache::run(ProjectArgs::new(dir, config.general, project, name, cmd))
        }
        Commands::Pack(cmd) => pack::run(ProjectArgs::new(dir, config.general, project, name, cmd)),
        Commands::PKI(cmd) => pki::run(ProjectArgs::new(dir, config.general, project, name, cmd)),
    }
}
