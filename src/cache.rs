use argh::FromArgs;
use assembly_pack::{
    common::fs::{scan_dir, FileInfo, FsVisitor},
    md5::{self, MD5Sum},
    sd0::fs::Converter,
    txt::{FileLine, Manifest, VersionLine},
};
use color_eyre::eyre::Context;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::File,
    io::{BufWriter, ErrorKind, Write},
    path::PathBuf,
};

use crate::ProjectArgs;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "cache")]
/// scans a file tree, generating sd0 compressed files and a manifest file
pub struct Args {
    // /// the directory to scan for files
    // #[argh(positional)]
    // path: PathBuf,

    // /// the directory of the cache
    // #[argh(option)]
    // output: Option<PathBuf>,

    // /// a prefix to names
    // #[argh(option, default = "String::new()")]
    // prefix: String,

    // /// name of the patcher directory
    // #[argh(option, default = "String::from(\"luclient\")")]
    // patcherdir: String,
    /// version number
    #[argh(option, short = 'v', default = "1")]
    version: u32,

    /// version name
    #[argh(option, short = 'n')]
    name: Option<String>,

    /// don't ignore pk files
    #[argh(switch, short = 'i')]
    include_pk: bool,
}

struct Visitor {
    conv: Converter,
    output: PathBuf,
    ignore_pk: bool,
    manifest: Manifest,
}

fn hash_to_path(hash: &MD5Sum) -> String {
    let hash = format!("{:?}", hash);
    let mut chars = hash.chars();
    let c1 = chars.next().unwrap();
    let c2 = chars.next().unwrap();
    format!("{}/{}/{}.sd0", c1, c2, hash)
}

impl FsVisitor for Visitor {
    fn visit_file(&mut self, info: FileInfo) {
        let input = info.real();
        if self.ignore_pk && input.extension() == Some(OsStr::new("pk")) {
            return;
        }

        let path = info.path();

        let in_meta = match md5::md5sum(input) {
            Ok(meta) => meta,
            Err(e) => {
                log::error!("Failed to check {}:\n\t{}", input.display(), e);
                return;
            }
        };
        let outpath = self.output.join(hash_to_path(&in_meta.hash));

        let line = match md5::md5sum(&outpath) {
            Ok(meta) => FileLine::new(in_meta, meta),
            Err(e) => {
                if e.kind() != ErrorKind::NotFound {
                    log::error!("Failed to access {}:\n\t{}", outpath.display(), e);
                    return;
                }

                // Continue with conversion if it was just not found
                let parent = outpath.parent().unwrap();
                if let Err(e) = std::fs::create_dir_all(parent) {
                    log::error!("Failed to create dir {}:\n\t{}", parent.display(), e);
                    return;
                }
                log::info!("Converting {} to {}", input.display(), outpath.display());
                match self.conv.convert_file(input, &outpath) {
                    Err(e) => {
                        log::error!(
                            "Error converting {} to {}:\n\t{}",
                            input.display(),
                            outpath.display(),
                            e
                        );
                        return;
                    }
                    Ok(line) => line,
                }
            }
        };
        //std::fs::rename(&output, &outpath).unwrap();

        println!("{},{}", path, line);
    }
}

pub fn run(args: ProjectArgs<Args>) -> color_eyre::Result<()> {
    let cache_dir = args.dir.join(&args.project.cache);
    let key: &str = args.project.key.as_deref().unwrap_or(args.name);
    let output = cache_dir.join(&key);

    let src_dir = args.dir.join(args.general.src);
    let dir_name = args.project.dir.as_deref().unwrap_or(args.name).to_owned();
    let proj_dir = src_dir.join(&dir_name);

    std::fs::create_dir_all(&output).wrap_err("Failed to create output dir")?;

    let mf_name = &args.project.pki.manifest;
    let manifest = output.join(mf_name).with_extension("txt");

    let vnum = args.cmd.version;
    let vname = args.cmd.name.unwrap_or_else(|| vnum.to_string());
    let version = VersionLine::new(vnum, vname);

    let mut visitor = Visitor {
        manifest: Manifest {
            version,
            files: BTreeMap::new(),
        },
        conv: Converter {
            generate_segment_index: false,
        },
        output,
        ignore_pk: !args.cmd.include_pk,
    };

    log::info!("Scanning {} as {}", proj_dir.display(), dir_name);
    scan_dir(&mut visitor, dir_name, &proj_dir, true);

    let mf_file = File::create(&manifest).context("Failed to create manifest file")?;
    let mut mf_writer = BufWriter::new(mf_file);

    log::info!("Writing manifest to {}", manifest.display());
    writeln!(mf_writer, "[version]")?;
    writeln!(mf_writer, "{}", &visitor.manifest.version)?;
    writeln!(mf_writer, "[files]")?;
    for (k, v) in visitor.manifest.files {
        writeln!(mf_writer, "{},{}", k, v)?;
    }

    Ok(())
}
