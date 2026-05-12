# TECH-SECURITY — Technical Specification

> **Level 3.** Implements [`SPEC-SECURITY.md`](../functional/SPEC-SECURITY.md). Implementation reference: [`22_ADVERSARIAL_ROBUSTNESS.md`](../../22_ADVERSARIAL_ROBUSTNESS.md).

**Status snapshot (2026-05-12):** `verdict_assembly.rs` contradiction detection is implemented and tested as part of Phase B. Secret redaction, cross-language label gating, adversarial corpus, catastrophic promotion approval, and remaining hardening tasks are still tracked separately.

## Module layout

```
crates/context-graph-mejepa-instruments/
  src/
    redact.rs                          # REQ-SEC-01: secret redaction
crates/context-graph-mejepa/
  src/
    verdict_assembly.rs                # REQ-SEC-03: multi-head contradiction detection
    cross_language_label_gating.rs     # REQ-SEC-04
crates/context-graph-mcp/
  src/
    handlers/tools/
      mejepa_tools.rs                  # extended: two-person rule (REQ-SEC-05)
    render/
      markdown_secure.rs               # REQ-SEC-06
crates/context-graph-mejepa/
  src/
    daemon_validate.rs                 # extended: D-drive permissions (REQ-SEC-07)
ci/lints/
  hook_script_lint.sh                  # REQ-SEC-10
tests/fixtures/adversarial/
  README.md                            # REQ-SEC-02 corpus
  buggy_passing/*.diff                 # 10 cases
  knowngood_lookalike/*.diff           # 10 cases
  embedder_targeted/*.diff             # 10 cases
```

## Secret redaction

```rust
// crates/context-graph-mejepa-instruments/src/redact.rs
use regex::Regex;
use once_cell::sync::Lazy;

static SECRET_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| vec![
    Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),                   // AWS access key
    Regex::new(r"sk-[A-Za-z0-9]{32,}").unwrap(),                // OpenAI-style API key
    Regex::new(r"-----BEGIN [A-Z ]+-----[\s\S]+?-----END [A-Z ]+-----").unwrap(),  // PEM
    Regex::new(r"eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}").unwrap(),  // JWT
    Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(),                // GitHub PAT
    Regex::new(r"(?i)(api[_-]?key|password|secret|token)\s*[:=]\s*['\"][^'\"]{8,}['\"]").unwrap(),  // generic
    // ... extend per operator threat model
]);

pub fn redact_secrets(content: &str) -> String {
    let mut out = content.to_string();
    for pattern in SECRET_PATTERNS.iter() {
        out = pattern.replace_all(&out, "[REDACTED]").into_owned();
    }
    out
}
```

Called by the chunk pipeline before `EmbedderForward::forward`:

```rust
// in shift_subscriber materialize_shift
let chunks = memory::ast::chunk_with_options(content, language, opts)?;
let chunks = chunks.into_iter().map(|mut c| {
    c.text = redact_secrets(&c.text);
    c
}).collect();
```

## Multi-head contradiction detection

```rust
// crates/context-graph-mejepa/src/verdict_assembly.rs
pub fn assemble_verdict(
    oracle: &OracleHeadOutput,
    failure_mode: &FailureModeHeadOutput,
    guard_violations: &[GuardViolation],
    ood_score: f32,
    confidence_interval: &ConformalInterval,
) -> Verdict {
    // OOD takes precedence
    if ood_score >= OOD_THRESHOLD || confidence_interval.width() > WIDTH_THRESHOLD {
        return Verdict::OutOfDistribution;
    }
    
    // Guard violations
    if !guard_violations.is_empty() {
        return Verdict::GuardRejected;
    }
    
    // CONTRADICTION: oracle says Pass at high confidence BUT failure_mode says high-severity bug
    let high_severity_concerns = failure_mode.predictions.iter()
        .filter(|p| p.severity >= Severity::High && p.confidence > 0.6)
        .count();
    if oracle.pass_confidence > 0.85 && high_severity_concerns > 0 {
        return Verdict::Abstain;
    }
    
    if oracle.pass_confidence > confidence_interval.lower {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}
```

## Cross-language label gating

```rust
// crates/context-graph-mejepa/src/cross_language_label_gating.rs
pub fn label_can_transfer(
    src_label: &FailureModeLabel,
    dst_chunk: &Chunk,
    patch_similarity_graph: &PatchSimilarityGraph,
) -> Option<f32> {
    if src_label.language == dst_chunk.language {
        return Some(1.0);  // same-language; full weight
    }
    if src_label.root_cause_class != classify_root_cause(dst_chunk) {
        return None;
    }
    let sim = patch_similarity_graph.cosine(&src_label.patch_id, &dst_chunk.patch_id);
    if sim < CROSS_LANGUAGE_SIMILARITY_THRESHOLD { return None; }
    Some(0.5)  // half weight
}
```

## Two-person rule

```rust
// extension to handle_promote_approval
pub async fn handle_promote_approval(storage: &Storage, args: PromoteApprovalArgs) -> Result<PromoteApprovalResponse> {
    let promotion = read_pending_promotion(storage, &args.promotion_id)?
        .ok_or(HealError::PromotionNotFound)?;
    
    if !promotion.is_catastrophic_class() {
        return Err(HealError::PromotionNotPendingApproval);
    }
    
    // Two-person rule
    let config = read_security_config(storage)?;
    if config.require_two_approvers_for_catastrophic_promotion {
        let approvers = list_approvers_for(storage, &args.promotion_id)?;
        if approvers.len() == 0 {
            // First approver
            record_approver(storage, &args.promotion_id, &args.operator_id)?;
            return Ok(PromoteApprovalResponse {
                promoted: false,
                reason: Some("first approver recorded; awaiting second".to_string()),
                witness_hash: promotion.witness_hash,
            });
        }
        if approvers.contains(&args.operator_id) {
            return Err(HealError::SameApproverTwice);
        }
        // Second approver: proceed with promotion
    }
    
    if args.approve {
        promote_approved(&storage, &promotion).await?;
    }
    // ...
}
```

## Markdown render hardening

```python
# scripts/cc/render_prediction_markdown.py (extension)
DANGEROUS_PATTERNS = [
    re.compile(r'^(\s*)>>>\s', re.MULTILINE),       # prompt continuation
    re.compile(r'^(\s*)system:\s', re.IGNORECASE | re.MULTILINE),  # role override
    re.compile(r'\{\{[\s\S]+?\}\}'),                # template injection
    re.compile(r'^---\s*$', re.MULTILINE),          # frontmatter break
]

def escape_dangerous(text: str) -> str:
    for pattern in DANGEROUS_PATTERNS:
        text = pattern.sub(lambda m: '\\' + m.group(0), text)
    return text

def render(pred: dict, max_bytes: int) -> str:
    # ... existing render logic ...
    final = "\n".join(lines)
    final = escape_dangerous(final)
    if len(final.encode()) > max_bytes:
        final = final[:max_bytes-30].rsplit('\n', 1)[0] + "\n...truncated"
    return final
```

## D-drive permissions

```rust
// crates/context-graph-mcp/src/daemon_validate.rs (extension)
pub fn validate_d_drive_permissions(d_root: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(d_root)?;
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {  // any group or world bits set
        return Err(SecurityError::DDrivePermissionsTooOpen { path: d_root.to_path_buf(), mode });
    }
    Ok(())
}
```

## Hook script lint

```bash
#!/usr/bin/env bash
# ci/lints/hook_script_lint.sh
set -euo pipefail

# Look for shell-injection patterns in hook scripts
VIOLATIONS=$(grep -rn -E 'eval\s|`.*\$\{?[A-Za-z_]+|sh\s+-c\s+"\$' scripts/cc/ \
    | grep -v '^scripts/cc/.*\.bak:' \
    || true)

if [[ -n "$VIOLATIONS" ]]; then
    echo "FAIL: shell-injection patterns detected in hook scripts:"
    echo "$VIOLATIONS"
    exit 1
fi
echo "PASS: no shell-injection patterns."
```

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `redact_secrets_fsv` | each pattern correctly redacts | `/mnt/d/contextgraph/fsv/security-fsv/redact.json` |
| `verdict_contradiction_fsv` | oracle Pass + failure-mode High → Abstain | `/mnt/d/contextgraph/fsv/security-fsv/contradiction.json` |
| `cross_language_gate_fsv` | label transfers only when allowed | `/mnt/d/contextgraph/fsv/security-fsv/cross_lang.json` |
| `two_person_rule_fsv` | catastrophic promotion blocks until second approver | `/mnt/d/contextgraph/fsv/security-fsv/two_person.json` |
| `markdown_escape_fsv` | dangerous patterns escaped | `/mnt/d/contextgraph/fsv/security-fsv/markdown_escape.json` |
| `d_drive_perms_fsv` | world-readable D-root → daemon refuses | `/mnt/d/contextgraph/fsv/security-fsv/perms.json` |
| `adversarial_corpus_fsv` | full 30-case adversarial run | `/mnt/d/contextgraph/fsv/security-fsv/adversarial.json` |
