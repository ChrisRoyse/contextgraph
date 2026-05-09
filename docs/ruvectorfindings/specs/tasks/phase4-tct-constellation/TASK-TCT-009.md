# TASK-TCT-009 — Constellation max-age policy (default 90 days; operator override audit-logged → MEJEPA_INFER_GTAU_STALE_CONSTELLATION)

```xml
<task_spec id="TASK-TCT-009" version="1.0">

<metadata>
  <title>Constellation freshness check in `crates/context-graph-mejepa-tct/src/freshness.rs` + `constellation.rs::check_freshness`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-TCT-11">Max-age 90 days default; MEJEPA_INFER_GTAU_STALE_CONSTELLATION on stale; operator override audit-logged</item>
    <item ref="AC-TCT-04-01">90+1 days stale → fail-closed</item>
    <item ref="EC-TCT-08">89 days fresh</item>
    <item ref="EC-TCT-09">91 days stale</item>
    <item ref="constitution:security_requirements">SEC-MEJEPA-04 + failure_taxonomy MEJEPA_INFER_GTAU_STALE_CONSTELLATION</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-008</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (freshness.rs)</estimated_files>
</metadata>

<context>
This task lands the freshness check that ConstellationStore.load (TASK-TCT-008) layers under at
inference time. Default max_age = 90 days; operator override `MEJEPA_TCT_ALLOW_STALE = true` (env
var) bypasses with audit log entry to ./memory/journal/ + (when wired) the witness chain.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (SEC-MEJEPA-04; failure_taxonomy MEJEPA_INFER_GTAU_STALE_CONSTELLATION)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-11; AC-TCT-04-01; ERR-TCT-02; EC-TCT-08, 09; BR-TCT-04 exception)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts TctConstellation.check_freshness)</file>
  <file purpose="upstream_store">crates/context-graph-mejepa-tct/src/store.rs (TASK-TCT-008 — load layered with freshness)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-008 complete</check>
  <check>`chrono = "0.4"` for ISO-8601 formatting</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/freshness.rs` with:
      - `pub const DEFAULT_MAX_AGE_DAYS: u32 = 90;`
      - `pub const ENV_ALLOW_STALE: &str = "MEJEPA_TCT_ALLOW_STALE";`
      - `pub const ENV_MAX_AGE_DAYS: &str = "MEJEPA_TCT_MAX_AGE_DAYS";`
      - `pub fn read_freshness_config() -> (u32, bool)` reading env vars (max_age_days from MEJEPA_TCT_MAX_AGE_DAYS or default; allow_stale from MEJEPA_TCT_ALLOW_STALE == "true")
      - `pub fn audit_log_override(version_id: [u8; 32], frozen_at: SystemTime, age_days: u32, max_age_days: u32) -> Result<(), TctError>` writes a frontmatter-ed `.md` file to `./memory/journal/agent-tct-allow-stale-{ulid}.md` with the override metadata
    - Add `pub fn check_freshness(&self, max_age_days: u32, allow_stale: bool) -> Result<(), TctError>` to `impl TctConstellation`:
      - Compute `age_days = (now - frozen_at) / 86400`
      - If `age_days > max_age_days && !allow_stale` → return `StaleConstellation { frozen_at_iso, age_days, max_age_days }`
      - If `age_days > max_age_days && allow_stale` → call audit_log_override + return Ok (with tracing::warn!)
      - Else Ok
    - Update `ConstellationStore::load` to call `check_freshness` after `check_provenance`
    - Add unit tests:
      - `check_freshness_89_days_passes`: synth constellation frozen_at=89 days ago → Ok
      - `check_freshness_91_days_fails_closed`: 91 days ago → StaleConstellation
      - `check_freshness_91_days_with_override_passes_with_audit`: allow_stale=true → Ok + audit log written
      - `check_freshness_exact_90_days_passes` (boundary at strict `>`)
      - `read_freshness_config_default_90_days`: no env vars set → (90, false)
      - `read_freshness_config_env_override`: MEJEPA_TCT_MAX_AGE_DAYS=180 → (180, *)
      - `audit_log_override_writes_smap_entry`: tempdir for ./memory/journal/ → file written with frontmatter
  </in_scope>
  <out_of_scope>
    - Witness-chain audit entry (Phase 7 SPEC-SHIFT-SUBSCRIBER + SPEC-WITNESS integration)
    - Phase 5 self-healing refit trigger (TASK-TCT-010 ViolationRateAggregator + Phase 5)
    - Constellation refit logic
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/freshness.rs">
use std::time::{Duration, SystemTime};
use crate::error::TctError;

pub const DEFAULT_MAX_AGE_DAYS: u32 = 90;
pub const ENV_ALLOW_STALE: &amp;str = "MEJEPA_TCT_ALLOW_STALE";
pub const ENV_MAX_AGE_DAYS: &amp;str = "MEJEPA_TCT_MAX_AGE_DAYS";

pub fn read_freshness_config() -&gt; (u32, bool);

pub fn audit_log_override(
    version_id: [u8; 32],
    frozen_at: SystemTime,
    age_days: u32,
    max_age_days: u32,
) -&gt; Result&lt;(), TctError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/constellation.rs">
impl TctConstellation {
    pub fn check_freshness(&amp;self, max_age_days: u32, allow_stale: bool)
        -&gt; Result&lt;(), TctError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - DEFAULT_MAX_AGE_DAYS = 90 per SEC-MEJEPA-04
    - Strict `&gt;` comparison (90 days exactly is fresh, not stale, per EC-TCT-08/09 boundary)
    - Override env var `MEJEPA_TCT_ALLOW_STALE = "true"` (case-sensitive); any other value → false
    - Audit log file path `./memory/journal/agent-tct-allow-stale-{ulid}.md` with SMAP frontmatter (namespace=journal, agent_id=agent-tct-allow-stale, timestamp=ISO-8601, status=active, tags=tct,override)
    - When override fires, also `tracing::warn!` for runtime visibility
    - StaleConstellation error includes frozen_at_iso (RFC 3339), age_days, max_age_days
    - SystemTime arithmetic via `duration_since(UNIX_EPOCH)` for portability; reject future-dated frozen_at with structured error
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct freshness` shows ≥ 7 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - 89 days passes; 91 days fails closed
    - Override produces SMAP audit entry
    - Boundary 90 days exactly passes
    - Env var override read correctly
  </verification>
</definition_of_done>

<pseudo_code>
pub fn read_freshness_config() -> (u32, bool) {
    let max_age = std::env::var(ENV_MAX_AGE_DAYS).ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_AGE_DAYS);
    let allow_stale = std::env::var(ENV_ALLOW_STALE).ok().as_deref() == Some("true");
    (max_age, allow_stale)
}

impl TctConstellation {
    pub fn check_freshness(&self, max_age_days: u32, allow_stale: bool) -> Result<(), TctError> {
        let now = SystemTime::now();
        let age = now.duration_since(self.frozen_at)
            .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
        let age_days = (age.as_secs() / 86400) as u32;
        if age_days > max_age_days {
            if allow_stale {
                let _ = freshness::audit_log_override(self.version_id, self.frozen_at, age_days, max_age_days);
                tracing::warn!(?age_days, ?max_age_days, "stale constellation override active");
                return Ok(());
            }
            let frozen_at_iso = chrono::DateTime::<chrono::Utc>::from(self.frozen_at).to_rfc3339();
            return Err(TctError::StaleConstellation { frozen_at_iso, age_days, max_age_days });
        }
        Ok(())
    }
}

pub fn audit_log_override(version_id, frozen_at, age_days, max_age_days) -> Result<(), TctError> {
    use std::io::Write;
    let dir = std::path::Path::new("./memory/journal");
    std::fs::create_dir_all(dir).ok();
    let id = ulid::Ulid::new();
    let path = dir.join(format!("agent-tct-allow-stale-{}.md", id));
    let now = chrono::Utc::now().to_rfc3339();
    let body = format!(
        "---\nnamespace: journal\nagent_id: agent-tct-allow-stale\ntimestamp: {}\nstatus: active\ntags: tct,override\n---\n\n# TCT stale constellation override\n\n- version_id: {}\n- frozen_at: {}\n- age_days: {}\n- max_age_days: {}\n- override_via: MEJEPA_TCT_ALLOW_STALE=true\n",
        now, hex::encode(version_id),
        chrono::DateTime::<chrono::Utc>::from(frozen_at).to_rfc3339(),
        age_days, max_age_days,
    );
    let mut f = std::fs::File::create(&path)
        .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
    f.write_all(body.as_bytes())
        .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/freshness.rs">freshness config + audit_log_override + 7 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">add `check_freshness` method to `impl TctConstellation`</file>
  <file path="crates/context-graph-mejepa-tct/src/store.rs">ConstellationStore::load now calls check_freshness after check_provenance</file>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod freshness;`</file>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">ensure `chrono`, `ulid`, `hex` deps</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct freshness shows ≥ 7 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>89 days passes; 91 days fails closed</criterion>
  <criterion>Boundary exactly 90 days passes (strict &gt;)</criterion>
  <criterion>Override produces SMAP audit entry under ./memory/journal/</criterion>
  <criterion>StaleConstellation .code() == "MEJEPA_INFER_GTAU_STALE_CONSTELLATION"</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct freshness</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```
