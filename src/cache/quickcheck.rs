use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
};

use assembly_pack::{crc::calculate_crc, md5::MD5Sum, txt::FileMeta};

pub(super) struct QuickCheck {
    pub path: String,
    pub mtime: Option<f64>,
    pub meta: FileMeta,
}

impl QuickCheck {
    pub fn write(&self, out: &mut BufWriter<File>) -> io::Result<()> {
        out.write_all(self.path.as_bytes())?;
        out.write(b",")?;
        if let Some(mtime) = self.mtime {
            write!(out, "{}", mtime)?;
        }
        out.write(b",")?;
        write!(out, "{}", self.meta.size)?;
        out.write(b",")?;
        writeln!(out, "{}", self.meta.hash)?;
        Ok(())
    }
}

pub(super) fn scan_quickcheck<R: io::Read>(reader: &mut R) -> BTreeMap<u32, QuickCheck> {
    let mut quickcheck = BTreeMap::new();
    let mut reader = BufReader::new(reader);
    let mut buffer = String::new();
    while let Ok(len) = reader.read_line(&mut buffer) {
        if len == 0 {
            break;
        }
        let mut fields = buffer.split(',');
        let path = fields.next().unwrap();
        let crc = calculate_crc(path.as_bytes());
        let mtime_str = fields.next().unwrap();
        if let Ok(mtime) = mtime_str.parse() {
            let size: u32 = fields.next().unwrap().parse().unwrap();
            let hash: MD5Sum = fields.next().unwrap().trim().parse().unwrap();
            quickcheck.insert(
                crc,
                QuickCheck {
                    path: path.to_owned(),
                    mtime: Some(mtime),
                    meta: FileMeta { size, hash },
                },
            );
        }
        buffer.clear();
    }
    quickcheck
}
