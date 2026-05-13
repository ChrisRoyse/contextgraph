use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;
use crate::types::{AstDiff, DiffHunk, PatchBundle};

pub const SECRET_REDACTION_ATTESTATION_KEY: &str = "MEJEPA_SECRET_REDACTION_PRE_EMBED";

const AWS_ACCESS_KEY_LEN: usize = 20;
const HIGH_ENTROPY_MIN_LEN: usize = 32;
const HIGH_ENTROPY_MIN_BITS: f32 = 3.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRedactionSpan {
    pub secret_class: String,
    pub start: usize,
    pub end: usize,
    pub marker: String,
    pub original_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRedactionReport {
    pub original_patch_sha: [u8; 32],
    pub spans: Vec<SecretRedactionSpan>,
}

impl SecretRedactionReport {
    pub fn redacted_span_count(&self) -> usize {
        self.spans.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedPatchBundle {
    pub patch: PatchBundle,
    pub report: SecretRedactionReport,
}

pub fn redact_patch_bundle(patch: &PatchBundle) -> Result<RedactedPatchBundle, MejepaInferError> {
    let mut spans = Vec::new();
    let mut hunks = Vec::with_capacity(patch.ast_diff.hunks.len());
    for hunk in &patch.ast_diff.hunks {
        let before = redact_text(&hunk.before, &mut spans)?;
        let after = redact_text(&hunk.after, &mut spans)?;
        hunks.push(DiffHunk {
            path: hunk.path.clone(),
            pre_sha: hunk.pre_sha,
            post_sha: hunk.post_sha,
            before,
            after,
        });
    }
    let commit_message = redact_text(&patch.commit_message, &mut spans)?;
    let redacted = PatchBundle::try_new(
        AstDiff { hunks },
        patch.witness_chain_segment.clone(),
        commit_message,
        patch.patch_sha,
    )?;
    Ok(RedactedPatchBundle {
        patch: redacted,
        report: SecretRedactionReport {
            original_patch_sha: patch.patch_sha,
            spans,
        },
    })
}

pub fn redact_text(
    text: &str,
    aggregate_spans: &mut Vec<SecretRedactionSpan>,
) -> Result<String, MejepaInferError> {
    let mut spans = detect_secret_spans(text);
    if spans.is_empty() {
        return Ok(text.to_string());
    }
    spans.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for span in spans {
        if span.start < cursor {
            continue;
        }
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return Err(MejepaInferError::InvalidInput {
                field: "secret_redaction.span".to_string(),
                detail: "detected secret span did not align with UTF-8 boundaries".to_string(),
            });
        }
        redacted.push_str(&text[cursor..span.start]);
        redacted.push_str(&span.marker);
        cursor = span.end;
        aggregate_spans.push(span);
    }
    redacted.push_str(&text[cursor..]);
    Ok(redacted)
}

fn detect_secret_spans(text: &str) -> Vec<SecretRedactionSpan> {
    let mut spans = Vec::new();
    detect_pem_blocks(text, &mut spans);
    detect_env_assignments(text, &mut spans);
    detect_ascii_tokens(text, &mut spans);
    spans
}

fn detect_pem_blocks(text: &str, spans: &mut Vec<SecretRedactionSpan>) {
    let mut search_from = 0usize;
    while let Some(begin_rel) = text[search_from..].find("-----BEGIN ") {
        let begin = search_from + begin_rel;
        let Some(end_rel) = text[begin..].find("-----END ") else {
            break;
        };
        let end_line_start = begin + end_rel;
        let end = text[end_line_start..]
            .find("-----")
            .map(|rel| end_line_start + rel + "-----".len())
            .unwrap_or(text.len());
        push_span(text, spans, "pem-block", begin, end);
        search_from = end;
    }
}

fn detect_env_assignments(text: &str, spans: &mut Vec<SecretRedactionSpan>) {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if let Some(eq_idx) = line.find('=') {
            let name = line[..eq_idx].trim();
            if is_secret_name(name) {
                let mut value_start = eq_idx + 1;
                while value_start < line.len() && line.as_bytes()[value_start].is_ascii_whitespace()
                {
                    value_start += 1;
                }
                let mut value_end = line.trim_end_matches(['\r', '\n']).len();
                while value_end > value_start
                    && line.as_bytes()[value_end - 1].is_ascii_whitespace()
                {
                    value_end -= 1;
                }
                if value_end > value_start + 3 {
                    let value = line[value_start..value_end]
                        .trim_matches('"')
                        .trim_matches('\'');
                    if is_aws_access_key(value)
                        || is_jwt_like(value)
                        || is_high_entropy_secret(value)
                    {
                        continue;
                    }
                    push_span(
                        text,
                        spans,
                        "env-secret",
                        offset + value_start,
                        offset + value_end,
                    );
                }
            }
        }
        offset += line.len();
    }
}

fn detect_ascii_tokens(text: &str, spans: &mut Vec<SecretRedactionSpan>) {
    let bytes = text.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if !is_secret_token_byte(bytes[idx]) {
            idx += 1;
            continue;
        }
        let start = idx;
        while idx < bytes.len() && is_secret_token_byte(bytes[idx]) {
            idx += 1;
        }
        let token = &text[start..idx];
        if is_aws_access_key(token) {
            push_span(
                text,
                spans,
                "aws-access-key",
                start,
                start + AWS_ACCESS_KEY_LEN,
            );
        } else if is_jwt_like(token) {
            push_span(text, spans, "jwt", start, idx);
        } else if is_high_entropy_secret(token) {
            push_span(text, spans, "high-entropy", start, idx);
        }
    }
}

fn is_secret_name(name: &str) -> bool {
    let candidate = name.trim();
    if candidate.is_empty()
        || !candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return false;
    }
    let upper = name
        .trim_start_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "CREDENTIAL"]
        .iter()
        .any(|needle| upper.contains(needle))
}

fn is_secret_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/' | b'+' | b'=' | b'.')
}

fn is_aws_access_key(token: &str) -> bool {
    token.len() >= AWS_ACCESS_KEY_LEN
        && (token.starts_with("AKIA") || token.starts_with("ASIA"))
        && token
            .as_bytes()
            .iter()
            .take(AWS_ACCESS_KEY_LEN)
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn is_jwt_like(token: &str) -> bool {
    let parts = token.split('.').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.iter().take(3).all(|part| {
            part.len() >= 8
                && part
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

fn is_high_entropy_secret(token: &str) -> bool {
    if token.len() < HIGH_ENTROPY_MIN_LEN {
        return false;
    }
    let has_lower = token.bytes().any(|byte| byte.is_ascii_lowercase());
    let has_upper = token.bytes().any(|byte| byte.is_ascii_uppercase());
    let has_digit = token.bytes().any(|byte| byte.is_ascii_digit());
    let has_symbol = token
        .bytes()
        .any(|byte| matches!(byte, b'_' | b'-' | b'/' | b'+' | b'='));
    let class_count = [has_lower, has_upper, has_digit, has_symbol]
        .into_iter()
        .filter(|present| *present)
        .count();
    class_count >= 3 && shannon_entropy_bits_per_byte(token.as_bytes()) >= HIGH_ENTROPY_MIN_BITS
}

fn shannon_entropy_bits_per_byte(bytes: &[u8]) -> f32 {
    let mut counts = [0usize; 256];
    for byte in bytes {
        counts[*byte as usize] += 1;
    }
    let len = bytes.len() as f32;
    counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let p = *count as f32 / len;
            -p * p.log2()
        })
        .sum()
}

fn push_span(
    text: &str,
    spans: &mut Vec<SecretRedactionSpan>,
    secret_class: &str,
    start: usize,
    end: usize,
) {
    if start >= end || end > text.len() {
        return;
    }
    let mut digest = Sha256::new();
    digest.update(&text.as_bytes()[start..end]);
    spans.push(SecretRedactionSpan {
        secret_class: secret_class.to_string(),
        start,
        end,
        marker: format!("[REDACTED:{secret_class}]"),
        original_sha256: digest.finalize().into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_access_key_is_redacted() {
        let mut spans = Vec::new();
        let redacted = redact_text("let key = \"AKIAIOSFODNN7EXAMPLE\";", &mut spans).unwrap();
        assert!(redacted.contains("[REDACTED:aws-access-key]"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn ordinary_source_code_is_not_over_redacted() {
        let source = "fn add(left: i32, right: i32) -> i32 { left + right }\n";
        let mut spans = Vec::new();
        let redacted = redact_text(source, &mut spans).unwrap();
        assert_eq!(redacted, source);
        assert!(spans.is_empty());
    }

    #[test]
    fn env_secret_value_is_redacted() {
        let mut spans = Vec::new();
        let redacted = redact_text("API_TOKEN = \"abc1234567890\"\n", &mut spans).unwrap();
        assert!(redacted.contains("API_TOKEN = [REDACTED:env-secret]"));
        assert_eq!(spans[0].secret_class, "env-secret");
    }

    #[test]
    fn high_entropy_token_is_redacted() {
        let mut spans = Vec::new();
        let redacted = redact_text("token aB3dE5gH7jK9mN2pQ4rS6tU8vW0xY1z_+", &mut spans).unwrap();
        assert!(redacted.contains("[REDACTED:high-entropy]"));
        assert_eq!(spans.len(), 1);
    }
}
