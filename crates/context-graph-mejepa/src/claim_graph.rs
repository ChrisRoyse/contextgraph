use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;
use crate::types::{
    AgentClaim, AgentClaimGraph, ClaimKind, ClaimReconciliation, ClaimReference, EvidenceRow,
    ReconciliationStatus, ShiftEntry, SymbolRef, TestId,
};

pub fn extract_claim_graph(agent_response: &str) -> Result<AgentClaimGraph, MejepaInferError> {
    let mut claims = Vec::new();
    for sentence in split_sentences(agent_response) {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            continue;
        }
        claims.push(parse_claim_sentence(trimmed));
    }
    let graph = AgentClaimGraph {
        raw_response: agent_response.to_string(),
        claims,
    };
    graph.validate()?;
    Ok(graph)
}

pub fn reconcile_claims(
    graph: &AgentClaimGraph,
    repo_root: &Path,
    observed_shifts: &[ShiftEntry],
) -> Result<Vec<ClaimReconciliation>, MejepaInferError> {
    let mut out = Vec::with_capacity(graph.claims.len());
    for claim in &graph.claims {
        let mut evidence = Vec::new();
        let status = reconcile_one(claim, repo_root, observed_shifts, &mut evidence)?;
        out.push(ClaimReconciliation {
            claim: claim.clone(),
            status,
            evidence,
        });
    }
    for row in &out {
        row.validate()?;
    }
    Ok(out)
}

fn split_sentences(input: &str) -> impl Iterator<Item = &str> {
    input
        .split(['!', '?', '\n'])
        .flat_map(|part| part.split(". "))
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn parse_claim_sentence(sentence: &str) -> AgentClaim {
    let lower = sentence.to_ascii_lowercase();
    for verb in [
        "added",
        "modified",
        "changed",
        "removed",
        "deleted",
        "renamed",
        "tested",
        "verified",
        "refactored",
        "documented",
    ] {
        if let Some(pos) = find_verb(&lower, verb) {
            let after = sentence[pos + verb.len()..].trim();
            let (symbol, file, line) = parse_reference(after);
            let symbol_ref = SymbolRef {
                symbol: symbol.clone().unwrap_or_else(|| after.to_string()),
                file: file.clone(),
                line,
            };
            let kind = match verb {
                "added" => ClaimKind::Added(symbol_ref),
                "modified" | "changed" => ClaimKind::Modified(symbol_ref),
                "removed" | "deleted" => ClaimKind::Removed(symbol_ref),
                "renamed" => ClaimKind::Renamed(symbol_ref, after.to_string()),
                "tested" => {
                    ClaimKind::Tested(TestId(symbol.clone().unwrap_or_else(|| after.to_string())))
                }
                "verified" => ClaimKind::Verified(after.to_string()),
                "refactored" => ClaimKind::Refactored(after.to_string()),
                "documented" => ClaimKind::Documented(symbol_ref),
                _ => ClaimKind::Other(sentence.to_string()),
            };
            let references = file
                .map(|file| vec![ClaimReference { file, symbol, line }])
                .unwrap_or_default();
            return AgentClaim {
                kind,
                text: sentence.to_string(),
                references,
            };
        }
    }
    AgentClaim {
        kind: ClaimKind::Other(sentence.to_string()),
        text: sentence.to_string(),
        references: Vec::new(),
    }
}

fn find_verb(lower: &str, verb: &str) -> Option<usize> {
    lower.find(verb).filter(|idx| {
        let before_ok = *idx == 0 || lower.as_bytes()[idx.saturating_sub(1)].is_ascii_whitespace();
        let after_idx = idx + verb.len();
        let after_ok =
            after_idx >= lower.len() || lower.as_bytes()[after_idx].is_ascii_whitespace();
        before_ok && after_ok
    })
}

fn parse_reference(text: &str) -> (Option<String>, Option<PathBuf>, Option<u32>) {
    let line = parse_line_number(text);
    let words = text
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| ch == '`' || ch == ',' || ch == ';'))
        .collect::<Vec<_>>();
    let file = words
        .iter()
        .find(|word| looks_like_path(word))
        .map(|word| PathBuf::from(*word));
    let symbol = words
        .iter()
        .find(|word| !looks_like_path(word) && !matches!(**word, "in" | "at" | "line" | "to"))
        .map(|word| word.trim_matches(['`', '"', '\'']).to_string())
        .filter(|value| !value.is_empty());
    (symbol, file, line)
}

fn parse_line_number(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    let marker = lower.find("line ")?;
    lower[marker + 5..]
        .split_whitespace()
        .next()
        .and_then(|raw| {
            raw.trim_matches(|ch: char| !ch.is_ascii_digit())
                .parse()
                .ok()
        })
}

fn looks_like_path(value: &str) -> bool {
    (value.contains('/') || value.contains('\\') || value.contains('.'))
        && !value.starts_with("http")
        && !value.contains("..")
}

fn reconcile_one(
    claim: &AgentClaim,
    repo_root: &Path,
    observed_shifts: &[ShiftEntry],
    evidence: &mut Vec<EvidenceRow>,
) -> Result<ReconciliationStatus, MejepaInferError> {
    if claim.references.is_empty() {
        return Ok(ReconciliationStatus::Unverifiable);
    }
    let mut any_confirmed = false;
    for reference in &claim.references {
        let file = repo_root.join(&reference.file);
        let exists = file.is_file();
        let shift = observed_shifts
            .iter()
            .find(|shift| shift.file == reference.file);
        if let Some(shift) = shift {
            evidence.push(EvidenceRow {
                file: reference.file.clone(),
                before_sha: shift.before_sha,
                after_sha: shift.after_sha,
                line_range: (
                    reference.line.unwrap_or(1),
                    reference.line.unwrap_or(1).max(1),
                ),
                contributing_shift_id: shift.shift_id.clone(),
            });
        } else if exists {
            let bytes = std::fs::read(&file).map_err(|source| MejepaInferError::Io {
                op: "read_claim_reference",
                path: file.clone(),
                source,
            })?;
            let digest = Sha256::digest(&bytes);
            let mut after_sha = [0u8; 32];
            after_sha.copy_from_slice(&digest);
            evidence.push(EvidenceRow {
                file: reference.file.clone(),
                before_sha: [0u8; 32],
                after_sha,
                line_range: (
                    reference.line.unwrap_or(1),
                    reference.line.unwrap_or(1).max(1),
                ),
                contributing_shift_id: "filesystem-readback".to_string(),
            });
        }
        any_confirmed |= exists;
    }
    Ok(if any_confirmed {
        ReconciliationStatus::Confirmed
    } else {
        ReconciliationStatus::Missing
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_added_symbol_and_file() {
        let graph = extract_claim_graph("I added parse_user in src/auth.rs at line 42.").unwrap();
        assert_eq!(graph.claims.len(), 1);
        assert!(matches!(graph.claims[0].kind, ClaimKind::Added(_)));
        assert_eq!(
            graph.claims[0].references[0].file,
            PathBuf::from("src/auth.rs")
        );
        assert_eq!(graph.claims[0].references[0].line, Some(42));
    }
}
