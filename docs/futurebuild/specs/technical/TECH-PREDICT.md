# TECH-PREDICT — Technical Specification

> **Level 3.** Defines HOW the live prediction agent API and multi-head extension are built. Implements [`SPEC-PREDICT.md`](../functional/SPEC-PREDICT.md).

**Status snapshot (2026-05-12):** Phase B code/FSV ships the rich `RealityPrediction` carrier, deterministic Phase B prediction surfaces, claim graph, reality-impact reconciler, what-if tool, and explain tool. Production DDA/neural feature consumption, trained per-head holdout gates, exemplar retrieval, and counterfactual-minimum-edit explanations remain open.

---

## Module layout

```
crates/context-graph-mejepa/
  src/
    compiler.rs                         # extended: rich RealityPrediction; multi-head verify()
    predictor/
      trunk.rs                          # existing: 6-layer transformer
      heads/
        panel.rs                        # existing
        oracle.rs                       # existing
        ...                             # Phase B typed auxiliary surfaces; full trained heads still hardening
    types.rs                            # extended: RealityPrediction with all Q1-Q5 fields
    prediction_surfaces.rs              # Phase B deterministic prediction surfaces
    verdict_assembly.rs                 # Phase B contradiction-aware verdict assembly
    claim_graph.rs                      # deterministic agent-claim extractor (REQ-PREDICT-17)
    reality_impact.rs                   # Q5 reconciler (REQ-PREDICT-19)
crates/context-graph-mcp/
  src/
    handlers/tools/
      mejepa_tools.rs                   # extended: predict_what_if, explain_prediction, record_agent_feedback
scripts/cc/
  hook_user_prompt_submit_inject_prediction.sh   # prediction injection (REQ-PREDICT-12)
  render_prediction_markdown.py                  # render RealityPrediction → markdown
```

---

## Multi-head architecture

The shared trunk produces a 1024-d hidden state per chunk; each head is a thin Linear projection on top, plus a head-specific feature projection consuming DDA signals:

```rust
pub struct MeJepaPredictor {
    pub trunk: TransformerTrunk,           // 6 layers, hidden 1024, 8 heads
    pub head_panel: PanelHead,
    pub head_oracle: OracleHead,
    pub head_failure_mode: FailureModeHead,    // Phase B surface; trained DDA-backed hardening remains
    pub head_edge_case: EdgeCaseHead,
    pub head_tech_debt: TechDebtHead,
    pub head_perf: PerfHead,
    pub head_security: SecurityHead,
    pub head_accuracy: AccuracyHead,
    pub head_cost: CostHead,
    pub head_reasoning: ReasoningHead,
}

impl MeJepaPredictor {
    pub fn forward(&self, panel_t01: &Tensor, action: &Tensor, dda: &DdaSignals) -> AllHeadOutputs {
        let trunk_h = self.trunk.forward(panel_t01, action)?;
        AllHeadOutputs {
            panel:        self.head_panel.forward(&trunk_h)?,
            oracle:       self.head_oracle.forward(&trunk_h)?,
            failure_mode: self.head_failure_mode.forward_with_dda(&trunk_h, dda)?,
            edge_case:    self.head_edge_case.forward_with_dda(&trunk_h, dda)?,
            tech_debt:    self.head_tech_debt.forward(&trunk_h)?,
            perf:         self.head_perf.forward(&trunk_h)?,
            security:     self.head_security.forward(&trunk_h)?,
            accuracy:     self.head_accuracy.forward(&trunk_h)?,
            cost:         self.head_cost.forward(&trunk_h)?,
            reasoning:    self.head_reasoning.forward(&trunk_h)?,
        }
    }
}
```

Each head is ~10-100k parameters (small Linear); the shared trunk is ~30M. Adding a head increases parameter count by < 1%.

---

## RealityPrediction full schema

See [`15_PREDICTION_TAXONOMY.md §1`](../../15_PREDICTION_TAXONOMY.md) for the canonical struct definition. The live implementation persists bincode-encoded `RealityPrediction` rows to `CF_MEJEPA_LIVE_PREDICTIONS` under `(session_id, created_at_unix_ms, prediction_id)` and performs read-after-write byte and decode validation.

---

## Conformal predictor extension

```rust
// crates/context-graph-mejepa/src/conformal.rs (extension)
pub struct PerHeadConformal {
    pub per_head: HashMap<HeadId, ConformalCalibration>,
}

impl PerHeadConformal {
    pub fn predict_with_interval(&self, head: HeadId, raw: &Tensor) -> ConformalInterval {
        let cal = self.per_head.get(&head).expect("head not calibrated");
        cal.predict_with_interval(raw)
    }
}
```

Per-head calibration is updated weekly by `heal::calibration::recalibrate_per_head` (extension of shipped recalibrate).

---

## TCT Gτ guard at inference (existing, restated)

```rust
// crates/context-graph-mejepa-tct/src/gtau.rs (existing)
pub fn gtau_check_chunks(
    predicted_panel: &Panel,
    constellation: &TctConstellation,
    chunks: &[Chunk],
) -> Vec<GuardViolation> {
    let mut out = Vec::new();
    for chunk in chunks {
        for embedder_id in active_embedders() {
            let cell = constellation.lookup(
                category=PredictedCategory::default(),
                entity_kind=chunk.kind,
                language=chunk.language,
                embedder=embedder_id,
            );
            let cosine = cosine(predicted_panel.per_chunk_emb(chunk, embedder_id), &cell.centroid);
            if cosine < cell.tau_m {
                out.push(GuardViolation {
                    embedder: embedder_id, chunk: chunk.id,
                    centroid_id: cell.id,
                    cosine, threshold_tau_m: cell.tau_m,
                    deficit: cell.tau_m - cosine,
                });
            }
        }
    }
    out
}
```

---

## Agent claim graph extractor (REQ-PREDICT-17)

```rust
// crates/context-graph-mejepa/src/claim_graph.rs
pub fn extract_claim_graph(agent_response: &str) -> AgentClaimGraph {
    // Deterministic rule-based parser. NO LLM.
    let mut claims = Vec::new();
    
    // Pattern: "I (added|modified|removed|renamed|tested|verified|refactored|documented) <symbol> [in <file>] [at line <N>]"
    for sentence in split_sentences(agent_response) {
        if let Some(claim) = parse_action_claim(sentence) {
            claims.push(claim);
        }
    }
    
    // Pattern: "the test <name> (passes|now passes|still passes)"
    for sentence in split_sentences(agent_response) {
        if let Some(claim) = parse_test_claim(sentence) {
            claims.push(claim);
        }
    }
    
    AgentClaimGraph { raw_response: agent_response.to_string(), claims }
}

pub fn reconcile_claim(claim: &AgentClaim, repo_root: &Path, after_sha: &Sha256) -> ClaimReconciliation {
    match &claim.kind {
        ClaimKind::Added(symbol_ref) => {
            // Check if the symbol now exists at the claimed path/line
            let exists = check_symbol_exists(repo_root, symbol_ref, after_sha)?;
            if exists {
                ClaimReconciliation { claim: claim.clone(), status: ReconciliationStatus::Confirmed, evidence: ... }
            } else {
                ClaimReconciliation { claim: claim.clone(), status: ReconciliationStatus::Missing, evidence: ... }
            }
        }
        // ... other kinds
    }
}
```

The rule-based parser is intentionally limited — only recognized claim shapes become structured `AgentClaim`s; everything else becomes `ClaimKind::Other` with `ReconciliationStatus::Unverifiable`.

---

## Prediction injection hook

```bash
#!/usr/bin/env bash
# scripts/cc/hook_user_prompt_submit_inject_prediction.sh
set -euo pipefail

SESSION_ID="${CLAUDE_SESSION_ID:-default}"

# Call MCP for latest prediction. Failure writes a diagnostic and exits non-zero.
RESPONSE=$(echo "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"mcp__cgreality__mejepa_predict_latest\",\"arguments\":{\"session_id\":\"$SESSION_ID\",\"max_age_seconds\":60,\"include_provenance\":false}}}" \
    | nc -U "/mnt/d/contextgraph/state/locks/mcp.sock")

# If no prediction, emit no markdown but do not claim a prediction was injected.
if [[ "$(echo "$RESPONSE" | jq -r '.result.prediction')" == "null" ]]; then
    exit 0
fi

# Render markdown
echo "$RESPONSE" | python3 scripts/cc/render_prediction_markdown.py --max-bytes 4096
```

The shell script is wired into `.claude/settings.json` under `hooks.UserPromptSubmit`. Output is appended to the agent's context as a `<user>` message addendum.

---

## Markdown rendering

```python
# scripts/cc/render_prediction_markdown.py
import sys
import json

def render(pred: dict, max_bytes: int) -> str:
    lines = []
    lines.append(f"### Reality prediction (chunks affected: {len(pred['covered_chunks'])})")
    lines.append("")
    ci = pred['confidence_interval']
    lines.append(f"**Verdict:** {pred['verdict']} (oracle pass confidence interval [{ci['lower']:.2f}, {ci['upper']:.2f}])")
    
    if pred['predicted_failure_modes']:
        lines.append("")
        lines.append("**Predicted failure modes (ranked):**")
        for i, fm in enumerate(pred['predicted_failure_modes'][:5], 1):
            lines.append(f"  {i}. `{fm['chunk']}:{fm['line_range'][0]}-{fm['line_range'][1]}` — **{fm['failure_class']}** (conf {fm['confidence']:.2f})")
            lines.append(f"     - explanation: {fm['explanation']}")
    
    # ... Q4 sections, all top-5 truncated ...
    
    out = "\n".join(lines)
    if len(out.encode()) > max_bytes:
        out = out[:max_bytes-30].rsplit('\n', 1)[0] + "\n...truncated; use mejepa_predict_latest for full"
    return out

if __name__ == "__main__":
    response = json.load(sys.stdin)
    pred = response['result']['prediction']
    print(render(pred, max_bytes=int(sys.argv[sys.argv.index('--max-bytes')+1])))
```

---

## `mejepa_record_agent_feedback` handler

```rust
// crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs (extension)
pub async fn handle_record_agent_feedback(
    storage: &Storage,
    args: RecordAgentFeedbackArgs,
) -> Result<RecordAgentFeedbackResponse> {
    // Rate limit
    if rate_limited(args.agent_id, max_per_hour=100) {
        return Err(FlywheelError::FeedbackRateLimited { agent_id: args.agent_id });
    }
    
    // Resolve prediction
    let pred = lookup_prediction(storage, &args.prediction_id)?
        .ok_or(FlywheelError::FeedbackPredictionNotFound { prediction_id: args.prediction_id })?;
    
    // Validate outcome shape
    if let Some(outcome) = &args.actual_outcome {
        validate_outcome_shape(&pred, outcome)?;
    }
    
    // Build SurpriseEvent
    let event = SurpriseEvent {
        feedback_id: FeedbackId::new(),
        prediction_id: args.prediction_id,
        agent_id: args.agent_id,
        ts: SystemTime::now(),
        feedback_kind: args.feedback_kind,
        agent_explanation: args.agent_explanation,
        actual_outcome: args.actual_outcome,
        severity: args.severity,
        extra_structured_data: args.extra_structured_data,
        witness_hash: derive_witness_hash(&pred.witness_hash, &args)?,
    };
    
    // Persist to CF_MEJEPA_AGENT_FEEDBACK
    storage.put_cf(CF_MEJEPA_AGENT_FEEDBACK, key_from(&event), serde_json::to_vec(&event)?)?;
    
    // Bump sampling weight on affected chunks
    let bump = 1.0 + 2.0 * args.severity.score();
    bump_sampling_weight_for_chunks(storage, &pred.covered_chunks, bump)?;
    
    // Add active-learning queue entry
    let queue_pos = enqueue_active_learning(storage, ActiveLearningEntry {
        kind: ActiveLearningKind::AgentSurprise,
        prediction_id: args.prediction_id,
        feedback_id: event.feedback_id,
        priority: args.severity.priority(),
    })?;
    
    // Catastrophic → alert
    let action = if args.severity == SurpriseSeverity::Catastrophic {
        write_alert(storage, AlertSeverity::Catastrophic, format!("Catastrophic agent surprise: {}", args.agent_explanation))?;
        "alert_catastrophic"
    } else { "fast_track_observe" };
    
    Ok(RecordAgentFeedbackResponse {
        feedback_id: event.feedback_id,
        witness_hash: event.witness_hash,
        sampling_weight_bump_applied: bump,
        active_learning_queue_position: queue_pos,
        drift_signal_contribution: args.severity.drift_class(),
        scheduler_action_queued: action.to_string(),
    })
}
```

---

## FSV strategy

| FSV | What it proves | Output path |
|---|---|---|
| `verify_full_q1_q5_fsv` | mejepa_verify returns RealityPrediction with all Q1-Q5 vecs populated | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/verify_full.json` |
| `predict_what_if_fsv` | hypothetical does not touch disk; comparison block reflects delta | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/what_if.json` |
| `explain_prediction_fsv` | per-prediction reasoning includes all required fields | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/explain.json` |
| `prediction_injection_fsv` | UserPromptSubmit hook renders markdown ≤ 4 KB | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/injection.md` |
| `claim_graph_extraction_fsv` | deterministic extractor parses 50 sample agent responses correctly | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/claim_graph.json` |
| `reality_impact_fsv` | Q5 reconciler matches predicted vs observed shifts | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/reality_impact.json` |
| `record_agent_feedback_fsv` | round-trip Surprise event persisted, weight bumped, queue length increments | `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/feedback.json` |
