use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;
use crate::types::{FailedGate, PatchBundle};

pub fn check_source_sha_drift(
    patch: &PatchBundle,
    repo_root: &Path,
) -> Result<Option<FailedGate>, MejepaInferError> {
    for hunk in &patch.ast_diff.hunks {
        let canonical = match canonicalize_under_root(&hunk.path, repo_root) {
            Ok(path) => path,
            Err(MejepaInferError::SourceShaDrift { path, .. }) => {
                return Ok(Some(FailedGate::SourceShaDrift { path }));
            }
            Err(err) => return Err(err),
        };
        let observed = match sha256_of_file(&canonical) {
            Ok(hash) => hash,
            Err(MejepaInferError::SourceShaDrift { .. }) => {
                return Ok(Some(FailedGate::SourceShaDrift { path: canonical }));
            }
            Err(err) => return Err(err),
        };
        if observed != hunk.post_sha {
            return Ok(Some(FailedGate::SourceShaDrift { path: canonical }));
        }
    }
    Ok(None)
}

pub fn replay_witness_segment(segment: &[u8]) -> Result<Option<FailedGate>, MejepaInferError> {
    if segment.is_empty() {
        return Ok(Some(FailedGate::WitnessChainBroken {
            reason: "witness segment is empty".to_string(),
        }));
    }
    if !segment
        .len()
        .is_multiple_of(context_graph_witness::WITNESS_ENTRY_SIZE)
    {
        return Ok(Some(FailedGate::WitnessChainBroken {
            reason: format!(
                "witness segment len {} is not a multiple of {}",
                segment.len(),
                context_graph_witness::WITNESS_ENTRY_SIZE
            ),
        }));
    }
    match context_graph_witness::verify_chain_bytes(segment) {
        Ok(_) => Ok(None),
        Err(err) => Ok(Some(FailedGate::WitnessChainBroken {
            reason: err.to_string(),
        })),
    }
}

pub fn canonicalize_under_root(path: &Path, root: &Path) -> Result<PathBuf, MejepaInferError> {
    reject_control_chars(path)?;
    let root_canonical =
        root.canonicalize()
            .map_err(|source| MejepaInferError::SourceShaDrift {
                path: root.to_path_buf(),
                claimed: [0u8; 32],
                observed: observed_from_io(&source),
            })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root_canonical.join(path)
    };
    let canonical =
        candidate
            .canonicalize()
            .map_err(|source| MejepaInferError::SourceShaDrift {
                path: candidate.clone(),
                claimed: [0u8; 32],
                observed: observed_from_io(&source),
            })?;
    if !canonical.starts_with(&root_canonical) {
        return Err(MejepaInferError::SourceShaDrift {
            path: canonical,
            claimed: [0u8; 32],
            observed: [0u8; 32],
        });
    }
    Ok(canonical)
}

pub fn sha256_of_file(path: &Path) -> Result<[u8; 32], MejepaInferError> {
    let bytes = std::fs::read(path).map_err(|source| MejepaInferError::SourceShaDrift {
        path: path.to_path_buf(),
        claimed: [0u8; 32],
        observed: observed_from_io(&source),
    })?;
    Ok(sha256_bytes(&bytes))
}

pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

pub fn valid_witness_segment() -> Vec<u8> {
    let first = context_graph_witness::WitnessEntry::new(
        context_graph_witness::ZERO_HASH,
        [1u8; context_graph_witness::HASH_SIZE],
        10,
        1,
    );
    let second = context_graph_witness::WitnessEntry::new(
        first.chain_hash(),
        [2u8; context_graph_witness::HASH_SIZE],
        11,
        1,
    );
    let mut segment = Vec::with_capacity(context_graph_witness::WITNESS_ENTRY_SIZE * 2);
    segment.extend_from_slice(&first.to_bytes());
    segment.extend_from_slice(&second.to_bytes());
    segment
}

fn reject_control_chars(path: &Path) -> Result<(), MejepaInferError> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        if path
            .as_os_str()
            .as_bytes()
            .iter()
            .any(|b| *b < 0x20 || *b == 0x7f)
        {
            return Err(MejepaInferError::SourceShaDrift {
                path: path.to_path_buf(),
                claimed: [0u8; 32],
                observed: [0u8; 32],
            });
        }
    }
    #[cfg(not(unix))]
    {
        if path
            .to_string_lossy()
            .bytes()
            .any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(MejepaInferError::SourceShaDrift {
                path: path.to_path_buf(),
                claimed: [0u8; 32],
                observed: [0u8; 32],
            });
        }
    }
    Ok(())
}

fn observed_from_io(source: &std::io::Error) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[0] = source.raw_os_error().unwrap_or_default().to_be_bytes()[3];
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_sha_drift_valid_path_returns_none() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("x.py");
        std::fs::write(&file, b"print(4)\n").unwrap();
        let sha = sha256_of_file(&file).unwrap();
        let patch = PatchBundle::try_new(
            crate::types::AstDiff {
                hunks: vec![crate::types::DiffHunk {
                    path: PathBuf::from("x.py"),
                    pre_sha: sha,
                    post_sha: sha,
                    before: String::new(),
                    after: "print(4)\n".to_string(),
                }],
            },
            valid_witness_segment(),
            "test".to_string(),
            sha,
        )
        .unwrap();
        assert_eq!(check_source_sha_drift(&patch, temp.path()).unwrap(), None);
    }

    #[test]
    fn path_traversal_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let err = PatchBundle::try_new(
            crate::types::AstDiff {
                hunks: vec![crate::types::DiffHunk {
                    path: PathBuf::from("../../etc/passwd"),
                    pre_sha: [0; 32],
                    post_sha: [0; 32],
                    before: String::new(),
                    after: String::new(),
                }],
            },
            valid_witness_segment(),
            "test".to_string(),
            [0; 32],
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_INVALID_INPUT");
        assert_eq!(
            canonicalize_under_root(Path::new("../../etc/passwd"), temp.path())
                .unwrap_err()
                .code(),
            "MEJEPA_INFER_SOURCE_SHA_DRIFT"
        );
    }

    #[test]
    fn witness_corruption_detected() {
        let mut segment = valid_witness_segment();
        segment[context_graph_witness::WITNESS_ENTRY_SIZE] ^= 0x7f;
        assert!(matches!(
            replay_witness_segment(&segment).unwrap(),
            Some(FailedGate::WitnessChainBroken { .. })
        ));
    }
}
