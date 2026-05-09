# TASK-EMBED-021 — E17 calibration certificate verifier + agent_state_score

```xml
<task_spec id="TASK-EMBED-021" version="1.0">
<metadata>
  <title>agent_state_score E17 valence/arousal_inv_U computation gated by ed25519-signed CalibrationCertificate in memory/decisions/</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>21</sequence>
  <implements>
    <item ref="REQ-EMBED-09">agent_state_score = valence · arousal_inv_U(arousal)</item>
    <item ref="BR-EMBED-05">refuse without certificate unless allow_uncalibrated</item>
    <item ref="EC-EMBED-07">JSON-only transcripts encode through E17 once calibrated</item>
    <item ref="TECH-EMBEDDERS::component_contracts::DiagnosticsModule (agent_state_score)">implementation</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-019</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Implements the ΔΩ_outer agent-affect signal via E17 AffectText. Until calibration on agent-transcript data has been certified (signed by operator-controlled ed25519 key per SEC-MEJEPA-03 and stored in `memory/decisions/`), the function returns `MEJEPA_EMBED_E17_UNCALIBRATED`. The Yerkes-Dodson inverted-U mapping `arousal_inv_u = 4 · a · (1 - a)` ensures the score peaks at moderate arousal.
</context>

<input_context_files>
  <file purpose="diagnostics_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#api_contracts (diagnostics::agent_state_score)</file>
  <file purpose="error_state_spec">docs/ruvectorfindings/specs/functional/SPEC-EMBEDDERS.md (ERR-EMBED-07)</file>
  <file purpose="calibration_cert">crates/context-graph-mejepa-embedders/src/registration.rs (CalibrationCertificate)</file>
  <file purpose="e17_wrapper">crates/context-graph-mejepa-embedders/src/embedders/learner_state.rs</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-019 complete (E17 wrapper produces valence + arousal scalars)</check>
  <check>TASK-EMBED-004 complete (CalibrationCertificate + ed25519 sign/verify)</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct CalibrationCertStore { dir: PathBuf, public_key: VerifyingKey }`
    - `pub fn from_memory_decisions(public_key: VerifyingKey) -> EmbedResult<Self>` — defaults to `memory/decisions/` directory
    - `pub fn lookup(&self, embedder_id: EmbedderId) -> EmbedResult<Option<CalibrationCertificate>>` — scans dir for `agent-*--<embedder_id>-calibration.json` files; verifies each ed25519 sig; returns first valid; returns Ok(None) if absent
    - `pub fn verify_calibrated(&self, embedder_id: EmbedderId) -> EmbedResult<CalibrationCertificate>` — wraps lookup; on None → MEJEPA_EMBED_E17_UNCALIBRATED
    - `pub fn agent_state_score(transcript_window: &str, e17: &E17AffectTextInstrument, cert_store: &CalibrationCertStore, allow_uncalibrated: bool) -> EmbedResult<f32>`:
      1. If `!allow_uncalibrated` → cert_store.verify_calibrated(E17AffectText)?
      2. encoded = e17.encode(EmbedderInput::Learner { input: LearnerEmbedderInput::Text { text: transcript_window.into() }, seed: 0 })?
      3. valence = encoded.scalars["valence"]; arousal = encoded.scalars["arousal"]
      4. valence_clamped = ((valence + 1.0) / 2.0).clamp(0.0, 1.0)
      5. arousal_clamped = arousal.clamp(0.0, 1.0)
      6. arousal_inv_u = 4.0 * arousal_clamped * (1.0 - arousal_clamped)
      7. return (valence_clamped * arousal_inv_u).clamp(0.0, 1.0)
    - Tests:
      - cert absent → MEJEPA_EMBED_E17_UNCALIBRATED
      - cert present + signed correctly → score returned
      - cert tampered (modified corpus_sha) → MEJEPA_EMBED_E17_UNCALIBRATED
      - allow_uncalibrated=true bypasses cert check; logs warning
      - synthetic E17 with valence=0.5, arousal=0.5 → score = clamp((0.5+1)/2 * 4*0.5*(1-0.5)) = 0.75 * 1.0 = 0.75
      - valence=-1, arousal=0 → score = 0.0
      - JSON-only transcripts (EC-EMBED-07) — pass through tokenizer cleanly
  </in_scope>
  <out_of_scope>
    - Producing calibration data (operator-driven offline process)
    - Calibration UI / CLI (out of EMBEDDERS scope)
    - E15/E16 calibration (not yet wired into ΔΩ_outer per SPEC out-of-scope)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/calibration.rs">
      use std::path::{Path, PathBuf};
      use std::fs;
      use ed25519_dalek::VerifyingKey;
      use tracing::{debug, warn};

      use crate::EmbedderId;
      use crate::embedders::learner_state::E17AffectTextInstrument;
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::CalibrationCertificate;
      use crate::types::EmbedderInput;
      use context_graph_embeddings::learner::LearnerEmbedderInput;

      pub struct CalibrationCertStore {
          dir: PathBuf,
          public_key: VerifyingKey,
      }

      impl CalibrationCertStore {
          pub fn new(dir: PathBuf, public_key: VerifyingKey) -&gt; Self;
          pub fn from_memory_decisions(public_key: VerifyingKey) -&gt; EmbedResult&lt;Self&gt;;
          pub fn lookup(&amp;self, embedder_id: EmbedderId) -&gt; EmbedResult&lt;Option&lt;CalibrationCertificate&gt;&gt;;
          pub fn verify_calibrated(&amp;self, embedder_id: EmbedderId) -&gt; EmbedResult&lt;CalibrationCertificate&gt;;
      }

      pub fn agent_state_score(
          transcript_window: &amp;str,
          e17: &amp;E17AffectTextInstrument,
          cert_store: &amp;CalibrationCertStore,
          allow_uncalibrated: bool,
      ) -&gt; EmbedResult&lt;f32&gt;;

      #[inline]
      pub fn arousal_inverted_u(a: f32) -&gt; f32 {
          let a = a.clamp(0.0, 1.0);
          (4.0 * a * (1.0 - a)).clamp(0.0, 1.0)
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - cert filename pattern: `agent-<NNN>-<role>--<embedder_slug>-calibration.json` (e.g. `agent-141-coordinator--e17-calibration.json`).
    - Each cert verified via `CalibrationCertificate::verify(&public_key)` per TASK-EMBED-004.
    - Failed verify → debug log + skip; return Ok(None) only if NO valid cert found.
    - allow_uncalibrated=true MUST log a warn with explicit "diagnostic only — NOT gated into ΔΩ_outer" message (per BR-EMBED-05 exception).
    - Score formula EXACTLY matches REQ-EMBED-09:
      - valence_clamped = ((valence + 1) / 2).clamp(0,1)
      - arousal_inv_u = 4 · arousal_clamped · (1 - arousal_clamped)
      - return (valence_clamped · arousal_inv_u).clamp(0,1)
    - sample_count ≥ 100 enforced by upstream CalibrationCertificate validator.
    - corpus_sha staleness: cert_store rejects certs older than 90 days (configurable via env CG_MEJEPA_CALIBRATION_MAX_AGE_DAYS); tracking stale certs returns the same MEJEPA_EMBED_E17_UNCALIBRATED with a `stale: true` discriminator hint in the error reason.
  </constraints>

  <verification>
    - cert absent → MEJEPA_EMBED_E17_UNCALIBRATED
    - signed cert present → score returned in [0,1]
    - tampered cert → MEJEPA_EMBED_E17_UNCALIBRATED
    - allow_uncalibrated=true bypass logs warn
    - score formula correct on canned synthetic E17 outputs
    - cargo test calibration passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
calibration.rs:
  - CalibrationCertStore reads dir on demand (no caching across calls — operator may add cert mid-session).
  - lookup(): for each *.json in dir matching pattern, parse + verify; return first valid.
  - verify_calibrated(): lookup → Some(c) | None → MEJEPA_EMBED_E17_UNCALIBRATED.
  - agent_state_score: per spec.
  - arousal_inverted_u: const-fn-ish helper.
  - Tests with tempfile dir + synthetic ed25519 keypair.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/calibration.rs">CalibrationCertStore + agent_state_score + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod calibration;` + `pub use calibration::{CalibrationCertStore, agent_state_score, arousal_inverted_u};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cert absent → MEJEPA_EMBED_E17_UNCALIBRATED</criterion>
  <criterion>signed cert present → score in [0,1]</criterion>
  <criterion>tampered cert → MEJEPA_EMBED_E17_UNCALIBRATED</criterion>
  <criterion>allow_uncalibrated=true bypass logs warn</criterion>
  <criterion>score formula matches REQ-EMBED-09 exactly</criterion>
  <criterion>stale cert (>90 days) returns MEJEPA_EMBED_E17_UNCALIBRATED</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders calibration</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
