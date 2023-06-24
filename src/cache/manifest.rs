use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use assembly_pack::txt::Manifest;

pub(super) fn write_manifest(manifest: Manifest, path: &Path) -> io::Result<()> {
    let mf_file = File::create(path)?;
    let mut mf_writer = BufWriter::new(mf_file);

    log::info!("Writing manifest to {}", path.display());
    writeln!(mf_writer, "[version]")?;
    writeln!(mf_writer, "{}", &manifest.version)?;
    writeln!(mf_writer, "[files]")?;
    for (k, (v, s)) in manifest.files {
        writeln!(mf_writer, "{},{},{}", k, v, s)?;
    }

    Ok(())
}
