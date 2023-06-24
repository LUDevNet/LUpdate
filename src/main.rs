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

    pub fn paths(&self) -> Paths {
        let res_name = self.project.res.as_ref().unwrap_or(&self.general.res);
        let src = self.project.src.as_deref().unwrap_or(&self.general.src);
        let src_dir = self.dir.join(&src);
        let dir = self.project.dir.as_deref().unwrap_or(self.name);
        let proj_dir = src_dir.join(&dir);
        let res_dir = match res_name.as_str() {
            "" => proj_dir.clone(),
            path => proj_dir.join(path),
        };
        let dir_name = proj_dir
            .file_name()
            .expect("project dir to have a name")
            .to_str()
            .expect("project dir have ASCII name");

        let prefix = self
            .project
            .prefix
            .as_deref()
            .unwrap_or(dir_name)
            .replace('/', "\\");
        let res_prefix = {
            let mut p = prefix.clone();
            if !res_name.is_empty() {
                for part in res_name.split(&['/', '\\']) {
                    if !p.is_empty() {
                        p.push('\\');
                    }
                    p.push_str(part);
                }
            }
            p
        };
        // Cache dir
        let cache_key = self.project.key.as_deref().unwrap_or(self.name);
        let cache_dir_parent = self.dir.join(&self.project.cache);
        let cache_dir = cache_dir_parent.join(&cache_key);
        Paths {
            proj_dir,
            cache_dir,
            cache_dir_parent,
            res_dir,
            prefix,
            res_prefix,
        }
    }
}

#[derive(Debug)]
pub struct Paths {
    proj_dir: PathBuf,
    cache_dir: PathBuf,
    cache_dir_parent: PathBuf,
    prefix: String,
    res_dir: PathBuf,
    res_prefix: String,
}

impl Paths {
    fn res_prefix_path(&self) -> String {
        match self.res_prefix.as_str() {
            "" => String::new(),
            path => format!("{path}\\"),
        }
    }
}

fn main() -> color_eyre::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .filter_module("globset", LevelFilter::Info)
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
