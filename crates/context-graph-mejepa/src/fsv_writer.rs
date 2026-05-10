use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub const FSV_DIR: &str = "/tmp/contextgraph-mejepa-predictor-fsv";

pub fn ensure_fsv_dir() -> io::Result<PathBuf> {
    let path = PathBuf::from(FSV_DIR);
    fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn clear_fsv_dir() -> io::Result<()> {
    let root = ensure_fsv_dir()?;
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub fn write_evidence_file<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let file = opts.open(path)?;
    serde_json::to_writer_pretty(file, value).map_err(io::Error::other)
}

pub fn read_evidence_file<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let file = File::open(path)?;
    serde_json::from_reader(file).map_err(io::Error::other)
}

pub fn write_readback_assert<T>(path: &Path, value: &T) -> io::Result<T>
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    write_evidence_file(path, value)?;
    let readback: T = read_evidence_file(path)?;
    if &readback != value {
        return Err(io::Error::other(format!(
            "FSV readback mismatch for {}: wrote {:?}, read {:?}",
            path.display(),
            value,
            readback
        )));
    }
    Ok(readback)
}
