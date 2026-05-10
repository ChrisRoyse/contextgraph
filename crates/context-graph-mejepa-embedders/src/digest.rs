use crate::config::EmbedderRegistration;
use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const DIGEST_CHUNK_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileDigest {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn digest_file_sha256(path: impl AsRef<Path>) -> EmbedResult<String> {
    let path = path.as_ref();
    let mut file = File::open(path).map_err(|err| EmbedError::ConfigRead {
        path: path.to_path_buf(),
        message: err.to_string(),
        remediation: "inspect the configured model path and file permissions",
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; DIGEST_CHUNK_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| EmbedError::ConfigRead {
                path: path.to_path_buf(),
                message: err.to_string(),
                remediation: "inspect disk health and retry the SHA-256 scan",
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_sha256_finalize(hasher))
}

pub fn verify_registration_digest(reg: &EmbedderRegistration) -> EmbedResult<Vec<FileDigest>> {
    if !reg
        .embedder
        .kind()
        .eq(&crate::embedder_id::EmbedderKind::ContentPretrained)
        && !reg
            .embedder
            .kind()
            .eq(&crate::embedder_id::EmbedderKind::LearnerState)
    {
        return Ok(Vec::new());
    }
    let base_dir = PathBuf::from(&reg.path);
    if !base_dir.exists() {
        return Err(EmbedError::WeightMissing {
            embedder: reg.embedder,
            path: base_dir,
            remediation:
                "download the exact model directory to D: or correct the path in models_config.toml",
        });
    }
    let (actual, files) = digest_manifest_for_embedder(reg.embedder, &base_dir, &reg.weight_files)?;
    if actual != reg.manifest_sha256 {
        return Err(EmbedError::DigestMismatch {
            embedder: reg.embedder,
            expected: reg.manifest_sha256.clone(),
            actual,
            remediation: "inspect the on-disk model files; if deliberately updated, update the SHA pins in git",
        });
    }
    Ok(files)
}

pub fn digest_manifest_for_embedder(
    embedder: EmbedderId,
    base_dir: &Path,
    files: &[String],
) -> EmbedResult<(String, Vec<FileDigest>)> {
    if files.is_empty() {
        return Err(EmbedError::invalid(
            "EmbedderRegistration.weight_files",
            format!("{embedder} has no weight files"),
            "pin at least one safetensors weight file in models_config.toml",
        ));
    }
    let mut sorted = files.to_vec();
    sorted.sort();
    let mut file_digests = Vec::with_capacity(sorted.len());
    for relative in sorted {
        let path = base_dir.join(&relative);
        if !path.exists() {
            return Err(EmbedError::WeightMissing {
                embedder,
                path,
                remediation:
                    "download the exact model artifact to D: and update models_config.toml",
            });
        }
        let metadata = path.metadata().map_err(|err| EmbedError::ConfigRead {
            path: path.clone(),
            message: err.to_string(),
            remediation: "inspect model file permissions and disk health",
        })?;
        if !metadata.is_file() {
            return Err(EmbedError::WeightMissing {
                embedder,
                path,
                remediation: "weight_files entries must point to files, not directories",
            });
        }
        file_digests.push(FileDigest {
            relative_path: relative,
            size_bytes: metadata.len(),
            sha256: digest_file_sha256(&path)?,
        });
    }

    let mut manifest = String::new();
    for file in &file_digests {
        manifest.push_str(&file.relative_path);
        manifest.push('\t');
        manifest.push_str(&file.sha256);
        manifest.push('\t');
        manifest.push_str(&file.size_bytes.to_string());
        manifest.push('\n');
    }
    Ok((sha256_hex(manifest.as_bytes()), file_digests))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_sha256_finalize(hasher)
}

fn hex_sha256_finalize(hasher: Sha256) -> String {
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn manifest_digest_uses_real_file_bytes_and_detects_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.safetensors"), b"alpha").unwrap();
        fs::write(temp.path().join("b.safetensors"), b"beta").unwrap();
        let files = vec!["b.safetensors".to_string(), "a.safetensors".to_string()];
        let (manifest, file_digests) =
            digest_manifest_for_embedder(EmbedderId::E1, temp.path(), &files).unwrap();
        assert_eq!(file_digests.len(), 2);

        let mut reg = EmbedderRegistration {
            embedder: EmbedderId::E1,
            name: "semantic".into(),
            kind: crate::embedder_id::EmbedderKind::ContentPretrained,
            path: temp.path().display().to_string(),
            repo: Some("real/local".into()),
            dimension: EmbedderId::E1.dimension(),
            weight_files: files,
            manifest_sha256: manifest,
        };
        verify_registration_digest(&reg).unwrap();
        reg.manifest_sha256 = "1".repeat(64);
        assert_eq!(
            verify_registration_digest(&reg).unwrap_err().code(),
            "MEJEPA_EMBED_DIGEST_MISMATCH"
        );
    }
}
