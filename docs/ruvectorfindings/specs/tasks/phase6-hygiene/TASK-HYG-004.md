# TASK-HYG-004 — Tier enum + thresholds + decay_factor + tier_for_frequency

## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-10 STATE AUDIT — clean

No Tier enum or threshold table exists in `context-graph-mejepa-hygiene` (crate not yet created). Task body accurate.

```xml
<task_spec id="TASK-HYG-004" version="1.0">
<metadata>
  <title>Tier enum + threshold table + decay_factor (0.99 per 24h) + tier_for_frequency mapper</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-HYG-02</requirement_ref>
    <requirement_ref>REQ-HYG-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The 5-tier policy state machine: Hot (raw f32) → Warm (8-bit) → Cool (5-bit) → Cold (3-bit) →
Frozen (1-bit sign). Frequency thresholds {≥0.8 Hot, 0.6-0.8 Warm, 0.4-0.6 Cool, 0.2-0.4 Cold,
<0.2 Frozen}. Per-24h decay factor 0.99 per doc 09 §6.2. This task ships the pure-math layer:
no I/O, no encoder dispatch (TASK-HYG-005 wires that up). Promotion / demotion symmetry is
implemented in TASK-HYG-007.
</context>

<input_context_files>
  <file purpose="tier_policy">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="tier_diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-001 shipped</check>
  <check>TASK-HYG-002 shipped (OpsError exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/tier.rs` with:
      * `pub enum Tier { Hot, Warm, Cool, Cold, Frozen }` derives Debug, Clone, Copy,
        PartialEq, Eq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize.
      * `Tier::all() -> &'static [Tier; 5]`
      * `Tier::slug() -> &'static str` ("hot", "warm", "cool", "cold", "frozen")
      * `Tier::threshold_lower(self) -> f32` (Hot 0.8, Warm 0.6, Cool 0.4, Cold 0.2, Frozen f32::NEG_INFINITY)
      * `Tier::next_colder(self) -> Option<Tier>` (Hot→Warm, ..., Frozen→None)
      * `Tier::next_warmer(self) -> Option<Tier>` (Frozen→Cold, ..., Hot→None)
      * `Tier::bit_width(self) -> Option<u8>` (Hot=None raw f32; Warm=Some(8); Cool=Some(5); Cold=Some(3); Frozen=Some(1))
    - `pub fn decay_factor(elapsed_24h_units: f64) -> f64` returning `0.99_f64.powf(elapsed_24h_units)`
      bounded for finite f64 (refuse NaN/Inf with OpsError::Internal).
    - `pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -> f32`
      computing freq * 0.99^(elapsed_h / 24) clamped to [0.0, 1.0]. Refuse `now_ts_ns < last_read_ts_ns`
      with OpsError (no time travel).
    - `pub fn incremented_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -> f32`
      = `(decayed_frequency + 1.0).min(1.0)` per AC-HYG-05-01.
    - `pub fn tier_for_frequency(freq: f32) -> Tier` mapping per the threshold table.
      Refuse non-finite freq with panic? — no: clamp to 0.0 and log WARN per fail-honest.
    - Unit tests covering boundary cases (0.8 exact = Warm or Hot? → Hot per ≥0.8;
      0.0 = Frozen; 1.0 = Hot; round-trip Tier slug).
  </in_scope>
  <out_of_scope>
    - encode_for_tier / decode_for_tier (TASK-HYG-005).
    - tier_demote / tier_promote (TASK-HYG-007).
    - Frozen 1-bit encoding body (TASK-HYG-006).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/tier.rs">
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Tier { Frozen, Cold, Cool, Warm, Hot }
    // Note: ordered Frozen &lt; Cold &lt; ... &lt; Hot so derive(Ord) gives natural "colder &lt; hotter".

    impl Tier {
        pub const fn all() -&gt; &amp;'static [Tier; 5];
        pub const fn slug(self) -&gt; &amp;'static str;
        pub const fn threshold_lower(self) -&gt; f32;
        pub const fn next_colder(self) -&gt; Option&lt;Tier&gt;;
        pub const fn next_warmer(self) -&gt; Option&lt;Tier&gt;;
        pub const fn bit_width(self) -&gt; Option&lt;u8&gt;;
    }

    pub fn decay_factor(elapsed_24h_units: f64) -&gt; crate::error::OpsResult&lt;f64&gt;;
    pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; crate::error::OpsResult&lt;f32&gt;;
    pub fn incremented_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; crate::error::OpsResult&lt;f32&gt;;
    pub fn tier_for_frequency(freq: f32) -&gt; Tier;
    </signature>
  </signatures>

  <constraints>
    - threshold_lower returns NEG_INFINITY for Frozen so any finite freq ≥ that.
    - Comparisons use `freq >= 0.8` (≥, not strict >) per the table inclusive lower bound.
    - decay_factor with elapsed=0 returns exactly 1.0.
    - decayed_frequency(0.0, _, _) returns 0.0 (no spurious sub-zero from floating drift).
    - incremented_frequency clamps to 1.0 hard.
    - now_ts_ns < last_read_ts_ns returns OpsError::Internal { reason: "time_travel" } — no
      silent abs() / sentinel.
    - non-finite freq input is clamped to 0.0 with `tracing::warn!` (fail-honest, not silent).
    - File under 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_for_frequency` passes (boundary cases).
    - `cargo test -p context-graph-mejepa-hygiene -- decay_factor` passes (0 → 1.0; 1 day → 0.99; 30 days → 0.99^30 ≈ 0.7397).
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub const fn next_colder(self) -&gt; Option&lt;Tier&gt; {
    match self {
        Tier::Hot =&gt; Some(Tier::Warm),
        Tier::Warm =&gt; Some(Tier::Cool),
        Tier::Cool =&gt; Some(Tier::Cold),
        Tier::Cold =&gt; Some(Tier::Frozen),
        Tier::Frozen =&gt; None,
    }
}

pub fn tier_for_frequency(freq: f32) -&gt; Tier {
    let f = if freq.is_finite() { freq } else { tracing::warn!(target:"hygiene", "non-finite freq clamped to 0.0"); 0.0 };
    if f &gt;= 0.8 { Tier::Hot }
    else if f &gt;= 0.6 { Tier::Warm }
    else if f &gt;= 0.4 { Tier::Cool }
    else if f &gt;= 0.2 { Tier::Cold }
    else { Tier::Frozen }
}

pub fn decayed_frequency(prev_freq: f32, last_read_ts_ns: u64, now_ts_ns: u64) -&gt; OpsResult&lt;f32&gt; {
    if now_ts_ns &lt; last_read_ts_ns { return Err(OpsError::internal("time_travel")); }
    let elapsed_h = (now_ts_ns - last_read_ts_ns) as f64 / 3_600_000_000_000.0;
    let factor = decay_factor(elapsed_h / 24.0)?;
    Ok((prev_freq as f64 * factor).clamp(0.0, 1.0) as f32)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/tier.rs">Tier enum + threshold table + frequency math</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">tier module already declared in TASK-HYG-001; this populates body</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add tier boundary tests + decay tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>tier_for_frequency boundaries: 0.8→Hot, 0.79→Warm, 0.6→Warm, 0.59→Cool, 0.4→Cool, 0.39→Cold, 0.2→Cold, 0.19→Frozen, 0.0→Frozen, 1.0→Hot</criterion>
  <criterion>decay_factor(0.0) == 1.0; decay_factor(1.0) == 0.99; decay_factor(30.0) ≈ 0.7397</criterion>
  <criterion>decayed_frequency time-travel returns OpsError::Internal</criterion>
  <criterion>incremented_frequency clamps to 1.0</criterion>
  <criterion>Tier::all().len() == 5; slug round-trip works</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_for_frequency</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- decay_factor</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
