use argh::FromArgs;
use assembly_pack::{
    common::{
        fs::{scan_dir, FileInfo, FsVisitor},
        FileMetaPair,
    },
    crc::calculate_crc,
    md5::{self, MD5Sum},
    sd0::fs::Converter,
    txt::{FileLine, Manifest, VersionLine},
};
use color_eyre::eyre::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::{BufRead, BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use crate::{cache::quickcheck::scan_quickcheck, config::ProjectConfig, Paths, ProjectArgs};

use self::quickcheck::QuickCheck;

mod manifest;
mod quickcheck;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "cache")]
/// scans a file tree, generating sd0 compressed files and a manifest file
pub struct Args {
    /// version number
    #[argh(option, short = 'v', default = "1")]
    version: u32,

    /// version name
    #[argh(option, short = 'n')]
    name: Option<String>,

    /// don't ignore pk files
    #[argh(switch, short = 'i')]
    include_pk: bool,

    /// assume the filenames passed to -F / --files are already relative to `dir`
    ///
    /// i.e. don't strip the prefix (e.g. client\)
    #[argh(switch, short = 'r')]
    relative: bool,

    /// name of a file containing one path per line
    #[argh(option, short = 'F')]
    files: Option<PathBuf>,
}

fn hash_to_path(hash: &MD5Sum) -> String {
    const SEP: char = std::path::MAIN_SEPARATOR;
    let hash = format!("{:?}", hash);
    let mut chars = hash.chars();
    let c1 = chars.next().unwrap();
    let c2 = chars.next().unwrap();
    format!("{}{SEP}{}{SEP}{}.sd0", c1, c2, hash)
}

#[derive(Default, Debug)]
struct Stats {
    quickcheck: usize,
    compress: usize,
    updated: usize,
    total: usize,
    ignored: usize,
}

struct Visitor {
    stats: Stats,
    include_glob: GlobSet,
    exclude_glob: GlobSet,
    quickcheck: BTreeMap<u32, QuickCheck>,
    quickcheck_out: BufWriter<File>,
    conv: Converter,
    output: PathBuf,
    /// The previous manifest
    prev: BTreeMap<String, FileLine>,
    /// The new manifest
    manifest: Manifest,
}

impl Visitor {
    fn compress(&mut self, input: &Path, outpath: &Path) -> Option<FileMetaPair> {
        // Continue with conversion if it was just not found
        let parent = outpath.parent().unwrap();
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::error!("Failed to create dir {}:\n\t{}", parent.display(), e);
            return None;
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
                return None;
            }
            Ok(line) => {
                self.stats.compress += 1;
                Some(line)
            }
        }
    }

    fn visit(&mut self, path: String, input: &Path, meta: Option<Metadata>) {
        if !self.include_glob.is_match(&path) || self.exclude_glob.is_match(&path) {
            self.stats.ignored += 1;
            return;
        }
        self.stats.total += 1;
        let crc = calculate_crc(path.as_bytes());
        let mtime = meta
            .as_ref()
            .and_then(|meta| meta.modified().ok())
            .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
            .as_ref()
            .map(Duration::as_secs_f64);
        let _size = meta.as_ref().map(Metadata::len);
        let quickcheck = self.quickcheck.remove(&crc);

        let in_meta = match quickcheck {
            // FIXME: size check
            Some(qc) if (mtime.is_some() && qc.mtime == mtime) => {
                self.stats.quickcheck += 1;
                qc.meta
            }
            _ => match md5::md5sum(input) {
                Ok(meta) => meta,
                Err(e) => {
                    log::error!("Failed to check {}:\n\t{}", input.display(), e);
                    return;
                }
            },
        };

        let old_meta_pair = self.prev.remove(&path);
        let mut meta_pair = old_meta_pair.filter(|(p, _)| p.raw == in_meta);

        if let (Some(old), None) = (old_meta_pair.as_ref(), meta_pair.as_ref()) {
            self.stats.updated += 1;
            log::debug!(
                "File {} was updated from {} to {}",
                path,
                old.0.raw.hash,
                in_meta.hash
            );
        }

        let outpath = self.output.join(hash_to_path(&in_meta.hash));

        if meta_pair.is_none() {
            let line = match md5::md5sum(&outpath) {
                Ok(meta) => FileMetaPair {
                    raw: in_meta,
                    compressed: meta,
                },
                Err(e) => {
                    if e.kind() != ErrorKind::NotFound {
                        log::error!("Failed to access {}:\n\t{}", outpath.display(), e);
                        return;
                    }
                    let Some(meta_pair) = self.compress(input, &outpath) else {
                        return
                    };
                    meta_pair
                }
            };
            let linesum = md5::MD5Sum::compute(&format!("{path},{line}"));
            meta_pair = Some((line, linesum));
        }
        if let Some((meta_pair, linesum)) = meta_pair {
            let qc = QuickCheck {
                path: path.clone(),
                mtime,
                meta: in_meta,
            };
            qc.write(&mut self.quickcheck_out).unwrap();

            self.manifest.files.insert(path, (meta_pair, linesum));
        }
    }

    fn do_scan_file(&mut self, real_proj_dir: &Path, line: &str, strip_prefix: &str) {
        let path = line.replace('/', "\\");
        let in_proj_path = match path.strip_prefix(strip_prefix) {
            Some(o) => o.trim(),
            None => {
                log::warn!("Missing prefix {}: {}", strip_prefix, path);
                return;
            }
        };
        let real = {
            let mut p = real_proj_dir.to_owned();
            p.extend(in_proj_path.split('\\'));
            p
        };
        let meta = match std::fs::metadata(&real) {
            Ok(meta) => Some(meta),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // If the file is explicitly listed but was no found, remove it
                log::warn!("File {:?} not found!", path);
                let crc = calculate_crc(path.as_bytes());
                if let Some(_) = self.quickcheck.remove(&crc) {
                    log::debug!("Removed {:?} from quickcheck", path);
                }
                if let Some(_) = self.prev.remove(&path) {
                    log::info!("Removed {:?} from manifest", path);
                }
                return; // don't visit this file
            }
            Err(e) => {
                log::debug!("Failed to get file metadata: {}", e);
                None
            }
        };
        self.visit(path, &real, meta);
    }

    /// Small generic function that calls [`do_scan_file`] on every line of its input
    fn do_scan_files<R: Read>(
        &mut self,
        file_list_reader: R,
        paths: &Paths,
        relative: bool,
    ) -> color_eyre::Result<()> {
        let files = BufReader::new(file_list_reader);
        let strip_prefix = match relative {
            true => "", // don't try to strip a prefix on relative paths
            false => &paths.strip_prefix,
        };
        for line in files.lines() {
            self.do_scan_file(&paths.proj_dir, line?.as_str(), strip_prefix);
        }
        Ok(())
    }

    fn scan_files(
        &mut self,
        file_list_path: &Path,
        paths: &Paths,
        relative: bool,
    ) -> color_eyre::Result<()> {
        if file_list_path == Path::new("-") {
            self.do_scan_files(std::io::stdin(), paths, relative)
        } else {
            let file_list_reader = File::open(&file_list_path).wrap_err_with(|| {
                format!("Failed to open files list: {}", file_list_path.display())
            })?;
            self.do_scan_files(file_list_reader, paths, relative)
        }
    }
}

impl FsVisitor for Visitor {
    fn visit_file(&mut self, info: FileInfo) {
        self.visit(info.path(), info.real(), info.metadata().ok())
    }
}

fn include_glob(project: &ProjectConfig) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    if project.include.is_empty() {
        builder.add(Glob::new("**")?);
    } else {
        for pattern in &project.include {
            builder.add(Glob::new(pattern)?);
        }
    }
    builder.build()
}

fn exclude_glob(project: &ProjectConfig) -> Result<GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in &project.exclude {
        builder.add(Glob::new(pattern)?);
    }
    builder.build()
}

pub fn run(args: ProjectArgs<Args>) -> color_eyre::Result<()> {
    let paths = args.paths();

    let quickcheck_path = paths
        .cache_dir_parent
        .join(format!("{}.quickcheck.txt", args.name));
    let output = paths.cache_dir.clone();
    std::fs::create_dir_all(&output).wrap_err("Failed to create output dir")?;

    let include_glob = include_glob(&args.project)?;
    let exclude_glob = exclude_glob(&args.project)?;

    let mut _quickcheck = File::options()
        .create(true)
        .write(true)
        .read(true)
        .open(&quickcheck_path)?;
    let quickcheck = scan_quickcheck(&mut _quickcheck);
    _quickcheck.seek(SeekFrom::Start(0))?;
    _quickcheck.set_len(0)?; // clear the file

    let proj_dir = &paths.proj_dir;

    let mf_name = &args.project.manifest;
    let manifest = output.join(mf_name).with_extension("txt");

    let vnum = args.cmd.version;
    let vname = args.cmd.name.unwrap_or_else(|| vnum.to_string());
    let version = VersionLine::new(vnum, vname);

    let prev = match std::fs::metadata(&manifest) {
        Ok(m) if m.is_file() => {
            let mf = Manifest::from_file(&manifest)?;
            log::info!(
                "Loaded previous manifest v{}: {}",
                mf.version.version,
                mf.version.name
            );
            mf.files
        }
        _ => BTreeMap::new(),
    };

    let mut visitor = Visitor {
        include_glob,
        exclude_glob,
        stats: Stats::default(),
        quickcheck,
        quickcheck_out: BufWriter::new(_quickcheck),
        prev,
        manifest: Manifest {
            version,
            files: BTreeMap::new(),
        },
        conv: Converter {
            generate_segment_index: false,
        },
        output,
    };

    log::info!("Scanning {} as {}", proj_dir.display(), paths.prefix);

    if let Some(file_list_path) = args.cmd.files {
        visitor.scan_files(&file_list_path, &paths, args.cmd.relative)?;
        // Write out untouched manifest files
        for (key, value) in visitor.prev {
            visitor.manifest.files.insert(key, value);
        }
        // Write out untouched quickcheck files
        for (_key, value) in visitor.quickcheck {
            value.write(&mut visitor.quickcheck_out)?;
        }
    } else {
        scan_dir(&mut visitor, paths.prefix, &proj_dir, true);
        for (k, _v) in visitor.prev {
            log::info!("File {} was removed", k);
        }
    }

    manifest::write_manifest(visitor.manifest, &manifest).context("Failed to write manifest")?;

    log::info!("{:?}", visitor.stats);

    Ok(())
}
