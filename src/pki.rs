use argh::FromArgs;
use assembly_pack::{
    pki::{self, gen::Config, writer::write_pki_file},
    txt::gen::{push_command, Command, DirSpec},
};
use color_eyre::eyre::Context;
use indexmap::IndexMap;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs::File,
    io::{BufRead, BufReader, BufWriter},
};

use crate::ProjectArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// generate a PKI file from a directory tree
#[argh(subcommand, name = "pki")]
pub struct Args {}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackConfig {
    #[serde(default)]
    compress: bool,
    #[serde(default)]
    dirs: Vec<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    exclude_files: Vec<String>,
    #[serde(default)]
    exclude_dirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Cfg {
    pack: IndexMap<String, PackConfig>,
}

fn hidden_glob(filename: &str) -> Option<DirSpec> {
    if let Some((l, r)) = filename.rsplit_once('\\') {
        if r.contains('*') {
            return Some(DirSpec {
                directory: l.to_string(),
                recurse_subdirectories: false,
                filter_wildcard: r.to_string(),
            });
        }
    }
    None
}

fn process_cfg(config: &mut Config, cfg: Cfg) {
    for (k, v) in cfg.pack {
        let cmd = Command::Pack {
            filename: format!("pack\\{}.pk", k),
            force_compression: v.compress,
        };
        push_command(config, cmd);

        for dir in v.dirs {
            let cmd = Command::AddDir(DirSpec {
                directory: dir,
                recurse_subdirectories: true,
                filter_wildcard: String::new(),
            });
            push_command(config, cmd);
        }

        for dir in v.exclude_dirs {
            let cmd = Command::RemDir(DirSpec {
                directory: dir,
                recurse_subdirectories: true,
                filter_wildcard: String::new(),
            });
            push_command(config, cmd);
        }

        for filename in v.files {
            let cmd = if let Some(dir) = hidden_glob(&filename) {
                Command::AddDir(dir)
            } else {
                Command::AddFile { filename }
            };
            push_command(config, cmd);
        }

        for filename in v.exclude_files {
            let cmd = if let Some(dir) = hidden_glob(&filename) {
                Command::RemDir(dir)
            } else {
                Command::RemFile { filename }
            };
            push_command(config, cmd);
        }

        push_command(config, Command::EndPack);
    }
}

pub fn run(args: ProjectArgs<Args>) -> color_eyre::Result<()> {
    let src_dir = args.dir.join(args.general.src);
    let dir_name = args.project.dir.as_deref().unwrap_or(args.name);
    let proj_dir = src_dir.join(&dir_name);
    let res_name = "res";
    let res_dir = proj_dir.join(res_name);
    let cfg_path = proj_dir.join(&args.project.config);

    let cache_dir = args.dir.join(&args.project.cache);
    let directory = cache_dir.join(&args.project.key.as_deref().unwrap_or(args.name));

    let pki_name = &args.project.pki;
    let output = directory.join(pki_name).with_extension("pki");

    let mf_name = &args.project.manifest;
    let manifest = directory.join(mf_name).with_extension("txt");

    let prefix = format!("{}\\{}\\", dir_name, res_name);
    let mut config = pki::gen::Config {
        directory: res_dir,
        output,
        manifest,
        prefix,
        pack_files: vec![],
    };

    log::info!("Loading generator config from {:?}", cfg_path.display());

    if cfg_path.extension() == Some(OsStr::new("toml")) {
        let cfg_text = std::fs::read_to_string(&cfg_path)?;
        let cfg: Cfg = toml::from_str(&cfg_text)?;
        process_cfg(&mut config, cfg);
    } else {
        let cfg_file = File::open(&cfg_path).wrap_err("Failed to load generator_config file")?;
        let cfg_reader = BufReader::new(cfg_file);
        for next_line in cfg_reader.lines() {
            let line = next_line.wrap_err("failed to read config line")?;
            if let Some(cmd) = assembly_pack::txt::gen::parse_line(&line) {
                push_command(&mut config, cmd);
            }
        }
    }

    let output = config.output.clone();
    let pki = config.run();

    log::info!("number of archives: {}", pki.archives.len());
    log::info!("number of files: {}", pki.files.len());
    log::info!("Writing to {}", output.display());

    let file = File::create(&output).context("Failed to create output file")?;

    let mut writer = BufWriter::new(file);
    write_pki_file(&mut writer, &pki).context("Failed to write PKI file")?;

    Ok(())
}
