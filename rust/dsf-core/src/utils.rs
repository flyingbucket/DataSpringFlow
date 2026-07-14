use crate::core::MetaDataError;
use crate::merkle::HashRes;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use directories::ProjectDirs;
use whoami;

pub fn hashres_to_hex(bytes: HashRes) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);

    for &b in &bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }

    out
}

pub(crate) fn validate_name_tag(name: &str, tag: &str) -> Result<()> {
    if name.is_empty() || tag.is_empty() {
        bail!("name/tag should not be empty");
    }
    if name.contains('@') || tag.contains('@') {
        bail!("name/tag should not contain '@'");
    }
    Ok(())
}

pub(crate) fn build_default_merkle_path(name: &str, tag: &str) -> Result<PathBuf> {
    let merkle_dir = ProjectDirs::from("io", "flyingbucket", "dataspringflow")
        .map(|proj| proj.data_dir().join("merkle"))
        .unwrap_or_else(||{
            log::warn!("Failed to find OS standard project dir. Using current working dir as a backup. Check your environment varible $HOME.");
            PathBuf::from("./data/merkle")
        });
    fs::create_dir_all(&merkle_dir)?;
    Ok(merkle_dir.join(format!("{}@{}.merkle.bin", name, tag)))
}

pub fn validate_dataset_id(id: &str) -> Result<()> {
    let parts: Vec<&str> = id.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!("Illegal id: {}，must be in form name@tag", id);
    }
    Ok(())
}

pub fn ensure_exists(p: &Path, arg_name: &str) -> Result<()> {
    if !p.exists() {
        bail!("{} path doesn't exist: {}", arg_name, p.display());
    }
    Ok(())
}

#[cfg(unix)]
pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn get_username() -> Result<String, MetaDataError> {
    whoami::username()
        .map_err(|e| MetaDataError::OwnerResolveFailed(format!("OS username unavailable: {e}")))
}

pub(crate) fn to_io_invalid_input(e: anyhow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
}
