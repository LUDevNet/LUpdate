use argh::FromArgs;
use assembly_pack::{
    pki::{self, writer::write_pki_file},
    txt::gen::push_command,
};
use color_eyre::eyre::Context;
use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter},
};

use crate::ProjectArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// generate a PKI file from a directory tree
#[argh(subcommand, name = "pki")]
pub struct Args {
    // /// the generator configuration
// #[argh(positional)]
// generator_config: PathBuf,
}

pub fn run(args: ProjectArgs<Args>) -> color_eyre::Result<()> {
    let src_dir = args.dir.join(args.general.src);
    let dir_name = args.project.dir.as_deref().unwrap_or(args.name);
    let proj_dir = src_dir.join(&dir_name);
    let res_name = "res";
    let res_dir = proj_dir.join(res_name);
    let cfg_path = proj_dir.join(&args.project.config);

    log::info!("Loading generator config from {:?}", cfg_path.display());
    let cfg_file = File::open(&cfg_path).wrap_err("Failed to load generator_config file")?;

    let cache_dir = args.dir.join(&args.project.cache);
    let directory = cache_dir.join(&args.project.key.as_deref().unwrap_or(args.name));

    let pki_name = &args.project.pki.index;
    let output = directory.join(pki_name).with_extension("pki");

    let mf_name = &args.project.pki.manifest;
    let manifest = directory.join(mf_name).with_extension("txt");

    let prefix = format!("{}\\{}\\", dir_name, res_name);
    let mut config = pki::gen::Config {
        directory: res_dir,
        output,
        manifest,
        prefix,
        pack_files: vec![],
    };
    let cfg_reader = BufReader::new(cfg_file);
    for next_line in cfg_reader.lines() {
        let line = next_line.wrap_err("failed to read config line")?;
        if let Some(cmd) = assembly_pack::txt::gen::parse_line(&line) {
            push_command(&mut config, cmd);
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
