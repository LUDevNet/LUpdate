//! This tool is used to pre-package PK files given
//! a patcher dir with sd0 files, a trunk manifest file
//! and a package index.
//!
//! It outputs a filtered
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use argh::FromArgs;
use assembly_pack::{
    crc::calculate_crc,
    pk::fs::{PKHandle, PKWriter},
    pki::core::PackIndexFile,
    txt::{FileMeta, Manifest},
};
use color_eyre::eyre::Context;
use globset::Glob;

use crate::ProjectArgs;

#[derive(FromArgs, PartialEq, Debug)]
/// pack files into PK archives
#[argh(subcommand, name = "pack")]
pub struct Args {
    #[argh(option, short = 'f', default = "String::from(\"*front*\")")]
    /// string that needs to be contained in the pack file name
    pub filter: String,
}

struct Writer<'a> {
    path: &'a Path,
}

impl<'a> PKWriter for Writer<'a> {
    fn write<W: std::io::Write>(&mut self, writer: &mut W) -> std::io::Result<()> {
        let file = File::open(self.path)?;
        let mut reader = BufReader::new(file);
        std::io::copy(&mut reader, writer)?;
        Ok(())
    }
}

fn win_join(base: &Path, path: &str) -> PathBuf {
    path.split('\\').fold(base.to_owned(), |mut l, r| {
        l.push(r);
        l
    })
}

pub fn run(args: ProjectArgs<Args>) -> color_eyre::Result<()> {
    let cache_dir = args.dir.join(&args.project.cache);
    let key: &str = args.project.key.as_deref().unwrap_or(args.name);
    let output = cache_dir.join(&key);

    let src_dir = args.dir.join(args.general.src);

    let mf_name = &args.project.manifest;
    let manifest_path = output.join(mf_name).with_extension("txt");
    log::info!("manifest: {}", manifest_path.display());
    let manifest = Manifest::from_file(&manifest_path)?;

    let pki_name = &args.project.pki;
    let pack_index_path = output.join(pki_name).with_extension("pki");
    log::info!("pack index: {}", pack_index_path.display());
    let pack_index = PackIndexFile::from_file(&pack_index_path)?;

    log::info!("patchdir: {}", output.display());

    let glob = Glob::new(&args.cmd.filter).context("failed to process filter glob")?;
    let matcher = glob.compile_matcher();

    let export: HashSet<usize> = pack_index
        .archives
        .iter()
        .enumerate()
        .filter_map(|(index, archive)| {
            if matcher.is_match(&archive.path) {
                Some(index)
            } else {
                None
            }
        })
        .collect();

    let mut pack_files = BTreeMap::new();

    for (name, file) in manifest.files {
        let crc = calculate_crc(name.as_bytes());

        if let Some(lookup) = pack_index.files.get(&crc) {
            // File is to be packed
            let pk_id = lookup.pack_file as usize;
            if export.contains(&pk_id) {
                // File is in a pack we want
                let pk = pack_files.entry(pk_id).or_insert_with(|| {
                    let name = &pack_index.archives[pk_id];
                    let path = win_join(&src_dir, &name.path);
                    println!("Opening PK {}", path.display());

                    // FIXME: Don't delete, update
                    let _ = std::fs::remove_file(&path);

                    PKHandle::open(&path).unwrap()
                });

                let is_compressed = lookup.category & 0xFF > 0;
                let raw = FileMeta {
                    size: file.filesize,
                    hash: file.hash,
                };
                let compressed = FileMeta {
                    size: file.compressed_filesize,
                    hash: file.compressed_hash,
                };

                let path = if is_compressed {
                    output.join(file.to_path())
                } else {
                    win_join(&src_dir, &name)
                };

                let mut writer = Writer { path: &path };
                pk.put_file(crc, &mut writer, raw, compressed, is_compressed)?;
            }
        }
    }

    for (k, mut pk) in pack_files.into_iter() {
        let path = &pack_index.archives[k].path;
        println!("Closing out PK {}", path);
        pk.finish()?;
    }

    Ok(())
}
