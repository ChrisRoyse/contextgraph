use std::path::{Component, Path, PathBuf};
use std::process::Command;

use context_graph_core::memory::ast::{self, AstChunk, AstChunkOptions};
use context_graph_mejepa_cf::CF_MEJEPA_CLAIM_RECONCILIATION;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::types::{
    AgentClaim, AgentClaimGraph, ClaimKind, ClaimReconciliation, ClaimReference, EvidenceRow,
    ReconciliationStatus, ShiftEntry, SymbolRef, TestId,
};

pub const CLAIM_GRAPH_SCHEMA_VERSION: u32 = 1;
pub const CLAIM_GRAPH_DEFAULT_LINE_WINDOW: u32 = 3;
const MAX_CLAIM_TEXT_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimReconciliationRecord {
    pub schema_version: u32,
    pub prediction_id: [u8; 16],
    pub claim_id: [u8; 16],
    pub claim_index: usize,
    pub claim: AgentClaim,
    pub status: ReconciliationStatus,
    pub reason_code: Option<String>,
    pub evidence_path: PathBuf,
    pub evidence_sha: [u8; 32],
    pub evidence: Vec<EvidenceRow>,
    pub source_of_truth_cf: String,
    pub created_at_unix_ms: i64,
}

impl ClaimReconciliationRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CLAIM_GRAPH_SCHEMA_VERSION {
            return invalid(
                "claim_reconciliation.schema_version",
                format!(
                    "expected {CLAIM_GRAPH_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        self.claim.validate()?;
        for row in &self.evidence {
            row.validate()?;
        }
        if self.source_of_truth_cf != CF_MEJEPA_CLAIM_RECONCILIATION {
            return invalid(
                "claim_reconciliation.source_of_truth_cf",
                format!("expected {CF_MEJEPA_CLAIM_RECONCILIATION}"),
            );
        }
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "claim_reconciliation.created_at_unix_ms",
                "must be positive",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimReconciliationWriteSummary {
    pub prediction_id: [u8; 16],
    pub rows_written: usize,
    pub byte_identical_readback: bool,
    pub source_of_truth_cf: String,
}

pub fn extract_claim_graph(agent_response: &str) -> Result<AgentClaimGraph, MejepaInferError> {
    let mut claims = Vec::new();
    for sentence in split_sentences(agent_response) {
        if let Some(claim) = parse_claim_sentence(sentence.trim()) {
            claims.push(claim);
        }
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
    reconcile_claims_inner(graph, repo_root, observed_shifts)
}

pub fn reconcile_claims_at_after_sha(
    graph: &AgentClaimGraph,
    repo_root: &Path,
    repo_after_sha: &str,
    observed_shifts: &[ShiftEntry],
) -> Result<Vec<ClaimReconciliation>, MejepaInferError> {
    verify_repo_after_sha(repo_root, repo_after_sha)?;
    reconcile_claims_inner(graph, repo_root, observed_shifts)
}

pub fn claim_reconciliation_records(
    prediction_id: [u8; 16],
    reconciliations: &[ClaimReconciliation],
    repo_root: &Path,
    created_at_unix_ms: i64,
) -> Result<Vec<ClaimReconciliationRecord>, MejepaInferError> {
    reconciliations
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            record_from_reconciliation(prediction_id, idx, row, repo_root, created_at_unix_ms)
        })
        .collect()
}

pub fn write_claim_reconciliation_records_sync_readback(
    db: &DB,
    records: &[ClaimReconciliationRecord],
) -> Result<ClaimReconciliationWriteSummary, MejepaInferError> {
    if records.is_empty() {
        return Ok(ClaimReconciliationWriteSummary {
            prediction_id: [0u8; 16],
            rows_written: 0,
            byte_identical_readback: true,
            source_of_truth_cf: CF_MEJEPA_CLAIM_RECONCILIATION.to_string(),
        });
    }
    let prediction_id = records[0].prediction_id;
    let cf = cf(db, CF_MEJEPA_CLAIM_RECONCILIATION)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut encoded = Vec::with_capacity(records.len());
    for record in records {
        if record.prediction_id != prediction_id {
            return invalid("claim_reconciliation.prediction_id", "mixed prediction ids");
        }
        record.validate()?;
        let key = claim_reconciliation_key(record.prediction_id, record.claim_id);
        let value = serde_json::to_vec(record)?;
        db.put_cf_opt(cf, key, &value, &opts)?;
        encoded.push((key, value));
    }
    db.flush_cf(cf)?;
    for (key, value) in &encoded {
        let readback = db
            .get_cf(cf, key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "claim_reconciliation.readback".to_string(),
                detail: "sync write readback returned no row".to_string(),
            })?;
        if &readback != value {
            return invalid(
                "claim_reconciliation.readback",
                "read-after-write bytes differ from encoded input",
            );
        }
    }
    Ok(ClaimReconciliationWriteSummary {
        prediction_id,
        rows_written: records.len(),
        byte_identical_readback: true,
        source_of_truth_cf: CF_MEJEPA_CLAIM_RECONCILIATION.to_string(),
    })
}

pub fn read_claim_reconciliation_records(
    db: &DB,
    prediction_id: [u8; 16],
) -> Result<Vec<ClaimReconciliationRecord>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_CLAIM_RECONCILIATION)?;
    let prefix = prediction_id;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.len() == 32 && key.starts_with(&prefix) {
            let row: ClaimReconciliationRecord = serde_json::from_slice(&value)?;
            row.validate()?;
            rows.push(row);
        }
    }
    rows.sort_by_key(|row| row.claim_index);
    Ok(rows)
}

pub fn count_claim_reconciliation_records(db: &DB) -> Result<usize, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_CLAIM_RECONCILIATION)?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

pub fn claim_id_for(
    prediction_id: [u8; 16],
    claim_index: usize,
    claim: &AgentClaim,
) -> Result<[u8; 16], MejepaInferError> {
    let bytes = serde_json::to_vec(&(prediction_id, claim_index, claim))?;
    let digest = Sha256::digest(bytes);
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    Ok(id)
}

fn reconcile_claims_inner(
    graph: &AgentClaimGraph,
    repo_root: &Path,
    observed_shifts: &[ShiftEntry],
) -> Result<Vec<ClaimReconciliation>, MejepaInferError> {
    let mut out = Vec::with_capacity(graph.claims.len());
    for claim in &graph.claims {
        let (status, evidence) = reconcile_one(claim, repo_root, observed_shifts)?;
        let row = ClaimReconciliation {
            claim: claim.clone(),
            status,
            evidence,
        };
        row.validate()?;
        out.push(row);
    }
    Ok(out)
}

fn split_sentences(input: &str) -> impl Iterator<Item = &str> {
    input
        .split(['!', '?', '\n', '\r'])
        .flat_map(|part| part.split(". "))
        .map(|part| part.trim_matches([' ', '\t', '-', '*']))
        .filter(|part| !part.is_empty())
}

fn parse_claim_sentence(sentence: &str) -> Option<AgentClaim> {
    if sentence.trim().is_empty() {
        return None;
    }
    if let Some(claim) = parse_will_claim(sentence, "will pass", true) {
        return Some(claim);
    }
    if let Some(claim) = parse_will_claim(sentence, "should pass", true) {
        return Some(claim);
    }
    if let Some(claim) = parse_will_claim(sentence, "will fail", false) {
        return Some(claim);
    }
    if let Some(claim) = parse_will_claim(sentence, "should fail", false) {
        return Some(claim);
    }
    parse_verb_claim(sentence)
}

fn parse_will_claim(sentence: &str, phrase: &str, will_pass: bool) -> Option<AgentClaim> {
    let lower = sentence.to_ascii_lowercase();
    let pos = lower.find(phrase)?;
    let before = sentence[..pos].trim();
    let after = sentence[pos + phrase.len()..].trim();
    let (before_symbol, before_file, before_line) = parse_reference(before);
    let (symbol, file, line) = if before_file.is_some() || before_symbol.is_some() {
        (before_symbol, before_file, before_line)
    } else {
        parse_reference(after)
    };
    let test_id = TestId(test_id_text(
        before,
        after,
        symbol.as_deref(),
        file.as_deref(),
    ));
    let kind = if will_pass {
        ClaimKind::WillPass(test_id)
    } else {
        ClaimKind::WillFail(test_id)
    };
    Some(claim(sentence, kind, symbol, file, line))
}

fn parse_verb_claim(sentence: &str) -> Option<AgentClaim> {
    let lower = sentence.to_ascii_lowercase();
    let verbs = [
        ("added", ClaimVerb::Added),
        ("created", ClaimVerb::Added),
        ("implemented", ClaimVerb::Added),
        ("wrote", ClaimVerb::Added),
        ("modified", ClaimVerb::Modified),
        ("changed", ClaimVerb::Modified),
        ("updated", ClaimVerb::Modified),
        ("wired", ClaimVerb::Modified),
        ("persisted", ClaimVerb::Modified),
        ("shipped", ClaimVerb::Modified),
        ("removed", ClaimVerb::Removed),
        ("deleted", ClaimVerb::Removed),
        ("renamed", ClaimVerb::Renamed),
        ("fixed", ClaimVerb::Fixed),
        ("tested", ClaimVerb::Tested),
        ("verified", ClaimVerb::Verified),
        ("refactored", ClaimVerb::Refactored),
        ("documented", ClaimVerb::Documented),
    ];
    let (verb, word, pos) = verbs
        .iter()
        .filter_map(|(word, kind)| find_verb(&lower, word).map(|pos| (*kind, *word, pos)))
        .min_by_key(|(_, _, pos)| *pos)?;
    let after = sentence[pos + word.len()..].trim();
    let (symbol, file, line) = parse_reference(after);
    Some(claim(
        sentence,
        verb.to_kind(after, &symbol, &file, line),
        symbol,
        file,
        line,
    ))
}

#[derive(Debug, Clone, Copy)]
enum ClaimVerb {
    Added,
    Modified,
    Removed,
    Renamed,
    Fixed,
    Tested,
    Verified,
    Refactored,
    Documented,
}

impl ClaimVerb {
    fn to_kind(
        self,
        text: &str,
        symbol: &Option<String>,
        file: &Option<PathBuf>,
        line: Option<u32>,
    ) -> ClaimKind {
        let symbol_ref = SymbolRef {
            symbol: symbol.clone().unwrap_or_else(|| target_text(text)),
            file: file.clone(),
            line,
        };
        match self {
            Self::Added => ClaimKind::Added(symbol_ref),
            Self::Modified => ClaimKind::Modified(symbol_ref),
            Self::Removed => ClaimKind::Removed(symbol_ref),
            Self::Renamed => ClaimKind::Renamed(symbol_ref, text.to_string()),
            Self::Fixed => ClaimKind::Fixed(symbol_ref),
            Self::Tested => {
                ClaimKind::Tested(TestId(symbol.clone().unwrap_or_else(|| target_text(text))))
            }
            Self::Verified => ClaimKind::Verified(target_text(text)),
            Self::Refactored => ClaimKind::Refactored(target_text(text)),
            Self::Documented => ClaimKind::Documented(symbol_ref),
        }
    }
}

fn claim(
    sentence: &str,
    kind: ClaimKind,
    symbol: Option<String>,
    file: Option<PathBuf>,
    line: Option<u32>,
) -> AgentClaim {
    let references = file
        .map(|file| vec![ClaimReference { file, symbol, line }])
        .unwrap_or_default();
    AgentClaim {
        kind,
        text: sentence.chars().take(MAX_CLAIM_TEXT_BYTES).collect(),
        references,
    }
}

fn find_verb(lower: &str, verb: &str) -> Option<usize> {
    lower.find(verb).filter(|idx| {
        let before_ok =
            *idx == 0 || !lower.as_bytes()[idx.saturating_sub(1)].is_ascii_alphanumeric();
        let after_idx = idx + verb.len();
        let after_ok =
            after_idx >= lower.len() || !lower.as_bytes()[after_idx].is_ascii_alphanumeric();
        before_ok && after_ok
    })
}

fn parse_reference(text: &str) -> (Option<String>, Option<PathBuf>, Option<u32>) {
    let line = parse_line_number(text);
    let tokens = text.split_whitespace().map(clean_token).collect::<Vec<_>>();
    let file = tokens
        .iter()
        .find_map(|token| file_path_token(token).map(PathBuf::from));
    let symbol = symbol_after_marker(&tokens)
        .or_else(|| symbol_from_qualified_path(&tokens))
        .or_else(|| first_symbol_token(&tokens, file.as_ref()));
    (symbol, file, line)
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '`' | '"' | '\'' | ',' | ';' | ':' | ')' | '(' | '[' | ']'
            )
        })
        .trim_start_matches("file:")
        .to_string()
}

fn file_path_token(token: &str) -> Option<String> {
    let normalized = token.trim_end_matches(['.', ',', ';']);
    let normalized = normalized.split("::").next().unwrap_or(normalized);
    let normalized = strip_line_suffix(normalized);
    if normalized.starts_with('/') || normalized.contains("..") || normalized.starts_with("http") {
        return None;
    }
    if looks_like_path(normalized) {
        Some(normalized.to_string())
    } else {
        None
    }
}

fn strip_line_suffix(value: &str) -> &str {
    if let Some((path, line)) = value.rsplit_once(':') {
        if !path.is_empty() && !line.is_empty() && line.chars().all(|ch| ch.is_ascii_digit()) {
            return path;
        }
    }
    value
}

fn looks_like_path(value: &str) -> bool {
    let extensions = [
        ".py", ".rs", ".js", ".ts", ".tsx", ".go", ".java", ".c", ".cc", ".cpp", ".cs", ".rb",
        ".php", ".md", ".toml", ".json", ".yaml", ".yml",
    ];
    (value.contains('/')
        || value.contains('\\')
        || extensions.iter().any(|ext| value.ends_with(ext)))
        && extensions.iter().any(|ext| value.contains(ext))
}

fn symbol_after_marker(tokens: &[String]) -> Option<String> {
    for window in tokens.windows(2) {
        if matches!(
            window[0].as_str(),
            "function" | "class" | "method" | "symbol" | "test" | "tests"
        ) {
            return Some(strip_test_suffix(&window[1]));
        }
    }
    None
}

fn symbol_from_qualified_path(tokens: &[String]) -> Option<String> {
    tokens
        .iter()
        .find(|token| token.contains("::") && looks_like_path(token))
        .map(|token| strip_test_suffix(token))
        .filter(|symbol| !symbol.is_empty())
}

fn first_symbol_token(tokens: &[String], file: Option<&PathBuf>) -> Option<String> {
    tokens
        .iter()
        .filter(|token| !token.is_empty())
        .filter(|token| {
            file.as_ref()
                .is_none_or(|path| token.as_str() != path.to_string_lossy().as_ref())
        })
        .filter(|token| !looks_like_path(token))
        .filter(|token| {
            let lower = token.to_ascii_lowercase();
            !STOP_WORDS.contains(&lower.as_str())
        })
        .find(|token| {
            token
                .chars()
                .any(|ch| ch == '_' || ch == ':' || ch.is_ascii_alphabetic())
        })
        .map(|token| strip_test_suffix(token))
}

fn strip_test_suffix(token: &str) -> String {
    token
        .rsplit("::")
        .next()
        .unwrap_or(token)
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .to_string()
}

const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "in", "at", "line", "to", "from", "with", "that", "this", "and", "by", "for",
    "of", "on", "under", "now",
];

fn parse_line_number(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    for marker in ["line ", "l"] {
        if let Some(pos) = lower.find(marker) {
            if let Some(value) = lower[pos + marker.len()..]
                .split_whitespace()
                .next()
                .and_then(parse_u32_prefix)
            {
                return Some(value);
            }
        }
    }
    lower
        .rsplit_once(':')
        .and_then(|(_, value)| parse_u32_prefix(value))
}

fn parse_u32_prefix(value: &str) -> Option<u32> {
    let digits = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse::<u32>().ok().filter(|line| *line > 0)
}

fn target_text(text: &str) -> String {
    text.trim().chars().take(MAX_CLAIM_TEXT_BYTES).collect()
}

fn test_id_text(before: &str, after: &str, symbol: Option<&str>, file: Option<&Path>) -> String {
    match (file, symbol) {
        (Some(file), Some(symbol)) => format!("{}::{symbol}", file.display()),
        (Some(file), None) => file.display().to_string(),
        (None, Some(symbol)) => symbol.to_string(),
        (None, None) if !before.is_empty() => target_text(before),
        (None, None) => target_text(after),
    }
}

fn reconcile_one(
    claim: &AgentClaim,
    repo_root: &Path,
    observed_shifts: &[ShiftEntry],
) -> Result<(ReconciliationStatus, Vec<EvidenceRow>), MejepaInferError> {
    let references = references_for_claim(claim);
    if references.is_empty() {
        return Ok((ReconciliationStatus::Unverifiable, Vec::new()));
    }
    let mut statuses = Vec::with_capacity(references.len());
    let mut evidence = Vec::new();
    for reference in references {
        let (status, row) = reconcile_reference(claim, &reference, repo_root, observed_shifts)?;
        statuses.push(status);
        evidence.push(row);
    }
    Ok((aggregate_status(&statuses), evidence))
}

fn references_for_claim(claim: &AgentClaim) -> Vec<ClaimReference> {
    if !claim.references.is_empty() {
        return claim.references.clone();
    }
    match &claim.kind {
        ClaimKind::Tested(test) | ClaimKind::WillPass(test) | ClaimKind::WillFail(test) => {
            test_reference(test)
        }
        _ => Vec::new(),
    }
}

fn test_reference(test: &TestId) -> Vec<ClaimReference> {
    let (symbol, file, line) = parse_reference(&test.0);
    file.map(|file| vec![ClaimReference { file, symbol, line }])
        .unwrap_or_default()
}

fn reconcile_reference(
    claim: &AgentClaim,
    reference: &ClaimReference,
    repo_root: &Path,
    observed_shifts: &[ShiftEntry],
) -> Result<(ReconciliationStatus, EvidenceRow), MejepaInferError> {
    ensure_python_target(repo_root, &reference.file)?;
    let absolute = repo_root.join(&reference.file);
    if !absolute.is_file() {
        return Ok((
            removed_or_missing_status(claim),
            evidence_row(
                reference,
                observed_shifts,
                [0u8; 32],
                "CLAIM_GRAPH_FILE_MISSING",
            ),
        ));
    }
    let bytes = std::fs::read(&absolute).map_err(|source| MejepaInferError::Io {
        op: "read_claim_reference",
        path: absolute.clone(),
        source,
    })?;
    let sha = sha256_bytes(&bytes);
    let text = std::str::from_utf8(&bytes).map_err(|err| {
        MejepaInferError::ClaimGraphUnsupportedLanguage {
            path: absolute.clone(),
            reason: format!("target is not valid UTF-8: {err}"),
        }
    })?;
    let evidence = evidence_row(reference, observed_shifts, sha, "filesystem-readback");
    let status = reconcile_python_text(claim, reference, &absolute, text)?;
    Ok((status, evidence))
}

fn reconcile_python_text(
    claim: &AgentClaim,
    reference: &ClaimReference,
    absolute: &Path,
    text: &str,
) -> Result<ReconciliationStatus, MejepaInferError> {
    if let Some(line) = reference.line {
        if line > text.lines().count().max(1) as u32 {
            return Ok(ReconciliationStatus::Ambiguous);
        }
    }
    let chunks = parse_python_chunks(&reference.file, text.as_bytes(), absolute)?;
    match &claim.kind {
        ClaimKind::Removed(_) => Ok(removed_status(reference, &chunks)),
        ClaimKind::Tested(_) | ClaimKind::WillPass(_) | ClaimKind::WillFail(_) => {
            Ok(existence_status(reference, &chunks))
        }
        _ => Ok(existence_status(reference, &chunks)),
    }
}

fn existence_status(reference: &ClaimReference, chunks: &[AstChunk]) -> ReconciliationStatus {
    let Some(symbol) = &reference.symbol else {
        return ReconciliationStatus::Matched;
    };
    let matches = matching_chunks(symbol, chunks);
    if matches.is_empty() {
        return ReconciliationStatus::Missing;
    }
    if let Some(line) = reference.line {
        if !matches
            .iter()
            .any(|chunk| chunk_intersects_line(chunk, line))
        {
            return ReconciliationStatus::ModifiedUnexpectedly;
        }
    }
    ReconciliationStatus::Matched
}

fn removed_status(reference: &ClaimReference, chunks: &[AstChunk]) -> ReconciliationStatus {
    let Some(symbol) = &reference.symbol else {
        return ReconciliationStatus::ModifiedUnexpectedly;
    };
    if matching_chunks(symbol, chunks).is_empty() {
        ReconciliationStatus::Matched
    } else {
        ReconciliationStatus::ModifiedUnexpectedly
    }
}

fn matching_chunks<'a>(symbol: &str, chunks: &'a [AstChunk]) -> Vec<&'a AstChunk> {
    chunks
        .iter()
        .filter(|chunk| {
            chunk.symbol_name.as_deref() == Some(symbol)
                || chunk.parent_chain.iter().any(|part| part == symbol)
                || (is_test_symbol(symbol) && chunk.content.contains(symbol))
        })
        .collect()
}

fn chunk_intersects_line(chunk: &AstChunk, line: u32) -> bool {
    let start = line.saturating_sub(CLAIM_GRAPH_DEFAULT_LINE_WINDOW);
    let end = line.saturating_add(CLAIM_GRAPH_DEFAULT_LINE_WINDOW);
    chunk.line_start <= end && chunk.line_end >= start
}

fn is_test_symbol(symbol: &str) -> bool {
    symbol.starts_with("test_") || symbol.contains("::test_")
}

fn parse_python_chunks(
    rel_path: &Path,
    bytes: &[u8],
    absolute: &Path,
) -> Result<Vec<AstChunk>, MejepaInferError> {
    let options = AstChunkOptions {
        file_path: rel_path.to_string_lossy().to_string(),
        max_non_ws_chars: 500,
    };
    ast::chunk_with_options(bytes, ast::Language::Python, &options).map_err(|err| {
        MejepaInferError::ClaimGraphUnsupportedLanguage {
            path: absolute.to_path_buf(),
            reason: format!("AST parser failed: {}: {err}", err.code()),
        }
    })
}

fn removed_or_missing_status(claim: &AgentClaim) -> ReconciliationStatus {
    match &claim.kind {
        ClaimKind::Removed(_) => ReconciliationStatus::Matched,
        _ => ReconciliationStatus::Missing,
    }
}

fn aggregate_status(statuses: &[ReconciliationStatus]) -> ReconciliationStatus {
    if statuses.contains(&ReconciliationStatus::Missing) {
        ReconciliationStatus::Missing
    } else if statuses.contains(&ReconciliationStatus::Ambiguous) {
        ReconciliationStatus::Ambiguous
    } else if statuses.contains(&ReconciliationStatus::ModifiedUnexpectedly) {
        ReconciliationStatus::ModifiedUnexpectedly
    } else if statuses.contains(&ReconciliationStatus::Matched) {
        ReconciliationStatus::Matched
    } else {
        ReconciliationStatus::Unverifiable
    }
}

fn evidence_row(
    reference: &ClaimReference,
    observed_shifts: &[ShiftEntry],
    fallback_sha: [u8; 32],
    reason: &str,
) -> EvidenceRow {
    if let Some(shift) = observed_shifts
        .iter()
        .find(|shift| shift.file == reference.file)
    {
        return EvidenceRow {
            file: reference.file.clone(),
            before_sha: shift.before_sha,
            after_sha: shift.after_sha,
            line_range: line_range(reference.line),
            contributing_shift_id: shift.shift_id.clone(),
        };
    }
    EvidenceRow {
        file: reference.file.clone(),
        before_sha: [0u8; 32],
        after_sha: fallback_sha,
        line_range: line_range(reference.line),
        contributing_shift_id: reason.to_string(),
    }
}

fn line_range(line: Option<u32>) -> (u32, u32) {
    let line = line.unwrap_or(1).max(1);
    (line, line)
}

fn ensure_python_target(repo_root: &Path, rel_path: &Path) -> Result<(), MejepaInferError> {
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return invalid(
            "claim_reference.file",
            "target path must be repository-relative",
        );
    }
    if rel_path.extension().and_then(|ext| ext.to_str()) != Some("py") {
        return Err(MejepaInferError::ClaimGraphUnsupportedLanguage {
            path: repo_root.join(rel_path),
            reason: "claim graph reconciliation currently supports Python targets only".to_string(),
        });
    }
    Ok(())
}

fn verify_repo_after_sha(repo_root: &Path, repo_after_sha: &str) -> Result<(), MejepaInferError> {
    if !matches!(repo_after_sha.len(), 40 | 64)
        || !repo_after_sha.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        return invalid("repo_after_sha", "must be a 40- or 64-hex SHA");
    }
    let status = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{repo_after_sha}^{{commit}}"))
        .status()
        .map_err(|source| MejepaInferError::Io {
            op: "git_cat_file_repo_after_sha",
            path: repo_root.to_path_buf(),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        invalid("repo_after_sha", "after SHA is unreachable in target repo")
    }
}

fn record_from_reconciliation(
    prediction_id: [u8; 16],
    claim_index: usize,
    row: &ClaimReconciliation,
    repo_root: &Path,
    created_at_unix_ms: i64,
) -> Result<ClaimReconciliationRecord, MejepaInferError> {
    let claim_id = claim_id_for(prediction_id, claim_index, &row.claim)?;
    let (evidence_path, evidence_sha, reason_code) = evidence_summary(row, repo_root);
    let record = ClaimReconciliationRecord {
        schema_version: CLAIM_GRAPH_SCHEMA_VERSION,
        prediction_id,
        claim_id,
        claim_index,
        claim: row.claim.clone(),
        status: row.status,
        reason_code,
        evidence_path,
        evidence_sha,
        evidence: row.evidence.clone(),
        source_of_truth_cf: CF_MEJEPA_CLAIM_RECONCILIATION.to_string(),
        created_at_unix_ms,
    };
    record.validate()?;
    Ok(record)
}

fn evidence_summary(
    row: &ClaimReconciliation,
    repo_root: &Path,
) -> (PathBuf, [u8; 32], Option<String>) {
    let Some(first) = row.evidence.first() else {
        return (
            repo_root.to_path_buf(),
            [0u8; 32],
            Some("NO_EVIDENCE".to_string()),
        );
    };
    let reason = match row.status {
        ReconciliationStatus::Ambiguous => Some("LINE_OUT_OF_RANGE".to_string()),
        ReconciliationStatus::Missing if first.after_sha == [0u8; 32] => {
            Some("CLAIM_GRAPH_FILE_MISSING".to_string())
        }
        _ if first.contributing_shift_id == "CLAIM_GRAPH_FILE_MISSING" => {
            Some("CLAIM_GRAPH_FILE_MISSING".to_string())
        }
        _ => None,
    };
    (repo_root.join(&first.file), first.after_sha, reason)
}

fn claim_reconciliation_key(prediction_id: [u8; 16], claim_id: [u8; 16]) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..16].copy_from_slice(&prediction_id);
    key[16..].copy_from_slice(&claim_id);
    key
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extracts_added_symbol_and_file() {
        let graph = extract_claim_graph("I added parse_user in src/auth.py at line 42.").unwrap();
        assert_eq!(graph.claims.len(), 1);
        assert!(matches!(graph.claims[0].kind, ClaimKind::Added(_)));
        assert_eq!(
            graph.claims[0].references[0].file,
            PathBuf::from("src/auth.py")
        );
        assert_eq!(graph.claims[0].references[0].line, Some(42));
    }

    #[test]
    fn extracts_will_pass_test_claim() {
        let graph =
            extract_claim_graph("The test tests/test_auth.py::test_login will pass now.").unwrap();
        assert_eq!(graph.claims.len(), 1);
        assert!(matches!(graph.claims[0].kind, ClaimKind::WillPass(_)));
        assert_eq!(
            graph.claims[0].references[0].file,
            PathBuf::from("tests/test_auth.py")
        );
    }

    #[test]
    fn reconciles_python_symbol_at_line() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(
            temp.path().join("src/auth.py"),
            "def parse_user(value):\n    return value\n",
        )
        .unwrap();
        let graph = extract_claim_graph("I added parse_user in src/auth.py at line 1.").unwrap();
        let rows = reconcile_claims(&graph, temp.path(), &[]).unwrap();
        assert_eq!(rows[0].status, ReconciliationStatus::Matched);
    }
}
