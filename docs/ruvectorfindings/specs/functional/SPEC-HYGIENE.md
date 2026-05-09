# SPEC-HYGIENE — ME-JEPA-Code Disk and Memory Hygiene (Phase 6)

> **Level 2 — Functional Specification.** Defines WHAT the hygiene domain (Phase 6) must do, not HOW. Implementation lives in `TECH-HYGIENE.md`. Conflicts with the constitution resolve to the constitution.

```xml
<functional_spec id="SPEC-HYGIENE" version="1.0">

<metadata>
  <title>ME-JEPA-Code Disk and Memory Hygiene (`me_jepa::ops::*` — tier compression, mincut splits, witness-chain Merkle compression, nightly GC, hard quota enforcement)</title>
  <status>draft</status>
  <owner>Phase 6 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-CORPUS</spec_ref>           <!-- Phase 0 mutation corpus + RocksDB CFs (CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS) consumes the same hard quota -->
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>      <!-- Phase 1 panels persisted to CF_MEJEPA_PANELS / CF_MEJEPA_PANEL_META; tiered storage applies to panel cache + disk overflow -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>        <!-- Phase 2 weight blobs in CF_MEJEPA_WEIGHT_BLOBS subject to "retain last 10 + tagged releases" -->
    <spec_ref>SPEC-TRAINING</spec_ref>         <!-- Phase 3 per-step training certs in CF_MEJEPA_TRAIN_CERTS feed witness-chain Merkle compression candidates -->
    <spec_ref>SPEC-INFERENCE</spec_ref>        <!-- Phase 4a CF_MEJEPA_LIVE_PREDICTIONS subject to LRU + tier policy; calibration snapshots retain last 30 -->
    <spec_ref>SPEC-TCT</spec_ref>              <!-- Phase 4b constellation centroids retained indefinitely (gold-tier) under "Aux signals & operator overrides" 2 GB budget -->
    <spec_ref>SPEC-SELF-HEALING</spec_ref>     <!-- Phase 5 owns drift detection + retrain semantics; this spec only exposes the MCP shim for heal_detect_drift / heal_retrain_if_needed -->
    <spec_ref>SPEC-WITNESS</spec_ref>          <!-- 73-byte SHAKE-256 chain (shipped); witness_compress_old_segments + verify_chain_integrity operate on its on-disk format -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §6.1 (storage budget table, ~115 GB total, CG_MEJEPA_DISK_QUOTA_GB env override), §6.2 (tiered 8/7/5/3-bit policy keyed on access-frequency decayed at 0.99/24h), §6.3 (8-step nightly GC pass at 03:00 local), §6.4 (Merkle compression of segments older than 90 days; 1024-entry segments → ~750× saving), §6.5 (hard quota enforcement + emergency eviction + alert), §6.6 (`me_jepa::ops::*` primitives), §7 Phase 6 (DoD + verification command + MCP tools)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §F (operator priorities — bounded storage is non-negotiable for self-healing)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (performance_budgets: rocksdb_disk_total_quota &lt;= 115 GB; failure_taxonomy: MEJEPA_RETRAIN_QUOTA_EXCEEDED, MEJEPA_OBSERVE_INTEGRITY_VIOLATION; anti_patterns: no unbounded growth, every persistent surface has a budget, every CF has a tier policy)</source>
    <source type="shipped">crates/context-graph-tier-compression/ (✅ 8/7/5/3-bit groupwise linear scalar quantization; magic+version-headed serialization; 22-byte header; 21 tests; FSV at /tmp/contextgraph-tier-compression-fsv/{blob_8,blob_7,blob_5,blob_3}.bin with cosines 0.99999/0.99997/0.99960/0.99192 on 768-dim Gaussian; ratios 3.89×/4.43×/6.12×/9.91×)</source>
    <source type="shipped">crates/context-graph-mincut/ (✅ Stoer-Wagner global mincut; dense O(V³) variant; fail-closed on negative weights / NaN / disconnected graphs; reused for repo-atomic split leakage minimization in this spec's GC eviction scoring)</source>
    <source type="shipped">crates/context-graph-witness/ (✅ 73-byte SHAKE-256 chain entries; verify_chain_integrity + Stage 0/F/Merkle certs in cert.rs; cert_merkle.rs supplies the Merkle tree primitive reused for §6.4 segment compression)</source>
  </source_documents>
</metadata>

<overview>
The hygiene domain is the bounded-storage substrate for ME-JEPA-Code. Without it the system grows
without limit — panels, training certs, witness entries, calibration snapshots, model checkpoints,
and live predictions accumulate forever, and within months the RTX 5090 host's disk fills, RocksDB
compactions stall, and the predictor's serving latency degrades. Phase 6 ships the operations
layer that keeps the entire ME-JEPA-Code system under a 115 GB hard quota indefinitely, while
preserving the auditability contract (every prediction traces to a witness-chain entry; every
training step carries a certificate; every old segment is verifiable by re-fetching the original
1024-entry pack from cold archive and recomputing its Merkle root).

Three machineries cooperate. (1) **Tier compression** (per doc 09 §6.2) demotes durable artifacts
from raw f32 (Hot, 4× saving) through 8/7/5/3/1-bit widths (Frozen, 32× saving) keyed on per-entry
access frequency that decays at 0.99 per 24h. Reads promote entries back to a hotter tier. The
shipped `context-graph-tier-compression` crate (8/7/5/3-bit groupwise linear scalar quantization
with magic+version-headed serialization, ratios 3.89× / 4.43× / 6.12× / 9.91× including the 22-byte
header, cosines 0.99999 / 0.99997 / 0.99960 / 0.99192 on 768-d Gaussian) is the substrate; this
spec wires it into RocksDB CF `tier_demote` / `tier_promote` ops keyed by `EntryId`. (2) **Nightly
GC** (per doc 09 §6.3) runs an 8-step pass at 03:00 local: demote → compact_range_cf each CF →
evict bottom-1 % entries by `(uplift × access_frequency × recency)` when corpus exceeds budget →
prune model checkpoints (keep last 10 + tagged releases) → prune calibration snapshots (keep last
30) → compress witness chain (per §6.4) → verify chain integrity (abort if broken; fail-closed
with `MEJEPA_HYG_GC_INTEGRITY_FAIL`) → write report to `&lt;runtime_root&gt;/mejepa/gc-report-&lt;date&gt;.json`.
(3) **Witness-chain Merkle compression** (per doc 09 §6.4) groups consecutive 1024-entry segments
older than 90 days into a single "compressed-segment" entry containing only the Merkle root +
segment metadata (start/end timestamps, entry count). The compressed-segment chains to the previous
entry's hash and is followed by the next entry, preserving the overall chain structure. Storage
saving: 1024 × 73 bytes = 74,752 B → ~100 B = ~750× per old segment; per the Phase 6 DoD a
100,000-entry chain compresses from ~7 MB to ~10 KB (~700× overall).

Hard quota enforcement (per doc 09 §6.5) is the structural fail-closed guarantee. If any category
exceeds budget at any time (not just at 03:00), `quota_check_and_evict` triggers immediately. If
one full GC pass cannot bring the category under budget, the system raises an alert (logged +
surfaced to operator) and **stops accepting new data into the over-budget category** while
continuing to serve predictions; this is the `MEJEPA_HYG_QUOTA_UNRECOVERABLE` failure mode. Tagged
releases of model checkpoints can NEVER be pruned (`MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED`); operator
overrides on aux signals are retained indefinitely (gold-tier within the 2 GB budget). The MCP
surface is three new tools — `mcp__cgreality__mejepa_gc_run` (returns `GcReport`),
`mejepa_quota_status` (returns per-category `QuotaStatus`), `mejepa_witness_compress(threshold_days:
u32)` (returns `CompressReport`). Two cross-cutting ops (`heal_detect_drift`,
`heal_retrain_if_needed`) appear on the `me_jepa::ops::*` interface for surface symmetry but their
**bodies are owned by SPEC-SELF-HEALING (Phase 5)**; this spec specifies only their MCP shim and
fail-closed contracts.

Beneficiaries: (1) the RTX 5090 operator host stays under 115 GB indefinitely, so the predictor
keeps serving without manual cleanup; (2) the audit chain stays verifiable for years (any old
entry can be checked against its archived segment) without paying linear storage cost; (3) the
self-healing loop (Phase 5) gets a stable substrate — it can write training certs and observe()
panels without worrying about disk pressure; (4) external integrations get an honest signal
(`mejepa_quota_status` reports per-category usage in real time, so a calling agent can detect that
the system is in graceful-degradation mode before submitting more data).
</overview>

<user_types>

  <user_type id="UT-01" name="ME-JEPA system processes (panel writers, training cert emitters, witness appenders)">
    <description>Internal Rust modules that write to the bounded RocksDB CFs (CF_MEJEPA_PANELS, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_LIVE_PREDICTIONS, witness chain). They consume the quota; they call `quota_check_and_evict` before bulk writes; they read tier-promoted entries through `tier_promote(entry_id)` on access.</description>
    <permissions>READ all per-CF quota; CALL `quota_check_and_evict` before bulk writes; CALL `tier_promote` on read; CANNOT bypass the 115 GB ceiling; CANNOT write to over-budget categories that have entered `MEJEPA_HYG_QUOTA_UNRECOVERABLE`.</permissions>
    <goals>Continue serving predictions / writing observations without hitting disk-full; receive deterministic structured errors when the system is in graceful-degradation mode.</goals>
  </user_type>

  <user_type id="UT-02" name="Nightly GC scheduler (cron / systemd timer)">
    <description>Process invoked at 03:00 local that executes the 8-step nightly GC pass. May run on the same host as the predictor or on a dedicated maintenance window.</description>
    <permissions>CALL `gc_run_nightly`; READ all CFs; WRITE to all tiered CFs; PRUNE model checkpoints / calibration snapshots subject to retention rules; INVOKE `witness_compress_old_segments(90 days)`; WRITE GC report to `&lt;runtime_root&gt;/mejepa/gc-report-&lt;date&gt;.json`; CANNOT run while a Phase 5 retrain is mid-promotion (must wait or skip with structured deferral).</permissions>
    <goals>Keep total disk under budget; emit a complete GC report; halt fail-closed if witness-chain integrity breaks at end of pass.</goals>
  </user_type>

  <user_type id="UT-03" name="Operator (human) — invokes MCP hygiene tools">
    <description>The human running the system. Inspects quota state, manually triggers GC when needed, configures `CG_MEJEPA_DISK_QUOTA_GB`, approves manual eviction during `MEJEPA_HYG_QUOTA_UNRECOVERABLE`, decides retention policy for tagged releases.</description>
    <permissions>READ all FSV evidence at /tmp/contextgraph-mejepa-hygiene-fsv/; CALL `mcp__cgreality__mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`; SET `CG_MEJEPA_DISK_QUOTA_GB` env override; TAG model checkpoints as gold-tier (immune to pruning); APPROVE archive-and-evict for over-budget aux signals; CANNOT silently disable quota enforcement (would require a constitution amendment).</permissions>
    <goals>Verify the system is bounded; intervene during graceful-degradation alerts; preserve chosen checkpoints / overrides indefinitely.</goals>
  </user_type>

  <user_type id="UT-04" name="External MCP caller (CI bot, audit reviewer)">
    <description>Any agent that calls the Phase 6 MCP tools to inspect storage state — typically a CI gate that refuses to submit a new task batch when quota is in red, or an auditor verifying that old witness segments were Merkle-compressed correctly.</description>
    <permissions>READ-only over MCP: `mcp__cgreality__mejepa_quota_status`, `mejepa_witness_compress(threshold_days)` returns `CompressReport` with the list of compressed segments + their Merkle roots; CANNOT mutate quota; CANNOT trigger eviction directly.</permissions>
    <goals>Receive an honest snapshot of storage budgets and a verifiable transcript of compression operations.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-HYG-01" priority="must-have">
    <narrative>
      <as_a>ME-JEPA system process (UT-01)</as_a>
      <i_want_to>append a new panel to CF_MEJEPA_PANELS without exceeding the 4 GB live-panel-cache RAM budget or the 20 GB live-panel-disk-overflow budget</i_want_to>
      <so_that>the predictor keeps serving without OOM and without disk-full crashes</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-01-01">
        <given>The live panel cache (RAM, 4 GB budget) is at 95% full AND the live panel disk overflow (20 GB budget) is at 50% full</given>
        <when>A new panel of size 5,120 × 4 bytes (20,480 B = 20 KB f32) is appended via the panel writer</when>
        <then>
          - The LRU evicts the oldest panel from RAM into the disk overflow CF (tiered by access frequency at the time of eviction)
          - The new panel lands in RAM at the head of the LRU
          - `CF_MEJEPA_PANEL_META` records the demotion event with `from_tier=Hot`, `to_tier` per the access-frequency table (≥0.8 → Hot 8-bit; 0.6-0.8 → Warm 7-bit; etc.)
          - No write returns success until the demoted entry is durably persisted (no silent dropping)
        </then>
      </criterion>
      <criterion id="AC-HYG-01-02">
        <given>The live panel disk overflow is at 100 % of its 20 GB budget AND the GC has not yet run today</given>
        <when>A new panel is appended</when>
        <then>
          - `quota_check_and_evict` triggers immediately (emergency eviction, not nightly)
          - Bottom-1 % entries by `(uplift × access_frequency × recency)` score are demoted to Frozen (1-bit) or evicted entirely if already Frozen
          - The new panel write succeeds if eviction freed sufficient space; otherwise the writer receives `MEJEPA_HYG_QUOTA_UNRECOVERABLE` and the panel write is rejected
          - An alert is logged + surfaced to operator
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-02" priority="must-have">
    <narrative>
      <as_a>nightly GC scheduler (UT-02)</as_a>
      <i_want_to>execute the 8-step nightly GC pass at 03:00 local and write a verifiable report</i_want_to>
      <so_that>the system stays under 115 GB total disk and the audit chain remains intact</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-02-01">
        <given>It is 03:00 local AND no Phase 5 retrain is mid-promotion AND total disk is at 100 GB</given>
        <when>`gc_run_nightly()` is invoked</when>
        <then>
          - Step 1 (demote): every entry whose access frequency dropped below the next-tier threshold in the last 24 h moves down one tier
          - Step 2 (compact): `compact_range_cf` runs on every ME-JEPA CF
          - Step 3 (evict): if total corpus exceeds budget, bottom-1 % entries by uplift × frequency × recency are evicted
          - Step 4 (prune checkpoints): keep last 10 + tagged releases; older non-tagged checkpoints deleted
          - Step 5 (prune calibration snapshots): keep last 30; older deleted
          - Step 6 (compress witness chain): segments older than 90 days are Merkle-compressed (calls `witness_compress_old_segments(Duration::days(90))`)
          - Step 7 (verify integrity): `verify_chain_integrity` runs end-to-end; on break, GC aborts with `MEJEPA_HYG_GC_INTEGRITY_FAIL` (no checkpoint deletes are committed; the integrity-broken state is the bug to fix)
          - Step 8 (report): writes `&lt;runtime_root&gt;/mejepa/gc-report-&lt;YYYY-MM-DD&gt;.json` with per-step counts, durations, total_disk_used_gb_before/after, integrity_check_passed: bool
        </then>
      </criterion>
      <criterion id="AC-HYG-02-02">
        <given>A Phase 5 retrain is mid-promotion when 03:00 arrives</given>
        <when>The scheduler attempts `gc_run_nightly()`</when>
        <then>
          - GC defers with structured `GcReport.deferred=true` + `deferred_reason="retrain_mid_promotion"`
          - GC reschedules for the next 03:00 OR the operator-configured retry interval (default: 1 h)
          - No CF mutation occurs
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-03" priority="must-have">
    <narrative>
      <as_a>operator or external auditor (UT-03 or UT-04)</as_a>
      <i_want_to>verify any old witness-chain entry by fetching its archived 1024-entry segment and recomputing the Merkle root</i_want_to>
      <so_that>the auditability contract survives storage compression — old facts remain verifiable forever</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-03-01">
        <given>A witness chain has segments older than 90 days that have been Merkle-compressed AND the original 1024-entry segments are archived to cold storage with `(corpus_sha, segment_id, start_ts, end_ts)` provenance</given>
        <when>An auditor requests verification of entry at `(segment_id=42, offset=137)` of the original chain</when>
        <then>
          - The auditor fetches the original 1024-entry segment from cold archive
          - Recomputes the Merkle tree using the canonical leaf/node tags from `context-graph-witness::cert::{MERKLE_LEAF_TAG, MERKLE_NODE_TAG}`
          - The recomputed Merkle root MUST byte-equal the root stored in the compressed-segment entry's `action_hash` field
          - The compressed-segment entry's `prev_hash` MUST link correctly to the previous (compressed or live) entry; the next live entry's `prev_hash` MUST link to the compressed-segment's hash
          - On any mismatch: the verifier returns `MEJEPA_HYG_MERKLE_VERIFY_FAIL` with the (expected_root, actual_root, segment_id) tuple
        </then>
      </criterion>
      <criterion id="AC-HYG-03-02">
        <given>A 100,000-entry witness chain (~7 MB raw at 73 B/entry) has been Merkle-compressed for all segments older than 90 days</given>
        <when>The compressed chain is measured on disk</when>
        <then>
          - Total on-disk size is approximately ~10 KB (~700× compression for the 100,000-entry case per Phase 6 DoD)
          - End-to-end `verify_chain_integrity()` passes
          - Sampling 5% of compressed segments and re-verifying via cold-archive fetch: 100 % match Merkle roots
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-04" priority="must-have">
    <narrative>
      <as_a>operator (UT-03)</as_a>
      <i_want_to>inspect per-category quota status in real time over MCP</i_want_to>
      <so_that>I can detect graceful-degradation conditions and intervene before the system rejects writes</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-04-01">
        <given>The system is steady-state with per-category usage at 60 % of budget</given>
        <when>The operator calls `mcp__cgreality__mejepa_quota_status`</when>
        <then>
          - Returns a `QuotaStatus` with one entry per category from doc 09 §6.1 (mutation_corpus, live_attempt_corpus, model_checkpoints, witness_chains, live_panel_cache_ram, live_panel_disk_overflow, calibration_snapshots, aux_signals_overrides, shift_log_subscriber_state)
          - Each entry contains: `budget_bytes`, `used_bytes`, `tier_distribution: HashMap&lt;Tier, usize&gt;` (count per tier), `oldest_entry_age_seconds`, `over_budget: bool`, `degradation_status: Healthy | Approaching | OverBudget | Unrecoverable`
          - `total_disk_used_gb` = sum of all categories
          - `effective_quota_gb` = `CG_MEJEPA_DISK_QUOTA_GB` env value or 115 default
          - Latency &lt; 50 ms p50 for the read-only snapshot
        </then>
      </criterion>
      <criterion id="AC-HYG-04-02">
        <given>The mutation corpus category is in `Unrecoverable` state (one full GC pass failed to bring it under budget)</given>
        <when>The operator calls `mejepa_quota_status`</when>
        <then>
          - The mutation_corpus entry's `degradation_status = Unrecoverable`
          - `last_gc_attempt_ts` populated; `last_gc_outcome = "no_progress"` populated
          - Top-level `system_accepts_new_data_in_categories: HashSet&lt;Category&gt;` excludes `mutation_corpus`
          - The status reflects that the system continues serving predictions but rejects new mutation_corpus writes
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-05" priority="must-have">
    <narrative>
      <as_a>ME-JEPA panel reader (UT-01)</as_a>
      <i_want_to>read a tier-compressed panel and have the system promote it back to a hotter tier on access</i_want_to>
      <so_that>frequently-accessed panels stay hot (low-latency, high-fidelity) while cold ones stay compressed</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-05-01">
        <given>An entry at Cold tier (3-bit, ~10.7× compression, cosine ~0.99192 vs original) AND access frequency 0.25</given>
        <when>The reader calls a method that internally invokes `tier_promote(entry_id)`</when>
        <then>
          - The entry is decompressed via `context-graph-tier-compression::decode(&blob)`
          - Reconstruction error is bounded by `(max - min) / (2 * (2^bits - 1))` per the half-bin-width invariant
          - Access frequency is incremented by the documented decay schedule (per doc 09 §6.2: per-read increment per the frequency-update rule)
          - If new frequency crosses the next-tier threshold (e.g., into 0.4-0.6 → Cool 5-bit), the entry is re-encoded at the hotter tier and re-persisted
          - `CF_MEJEPA_PANEL_META` records the promotion event
        </then>
      </criterion>
      <criterion id="AC-HYG-05-02">
        <given>A blob whose magic bytes are NOT `CGTC` (the `context-graph-tier-compression` magic)</given>
        <when>`tier_promote` attempts to decompress</when>
        <then>
          - Decompression fails with `MEJEPA_HYG_TIER_DECOMPRESS_FAIL` carrying the offending bytes (first 4) + `expected_magic="CGTC"` + entry_id
          - The reader receives an error; no silent fallback; no zero-fill
          - The event is logged with severity ≥ warn
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-06" priority="must-have">
    <narrative>
      <as_a>nightly GC scheduler (UT-02)</as_a>
      <i_want_to>refuse to prune any model checkpoint that carries a `tagged_release: true` provenance flag</i_want_to>
      <so_that>operator-tagged releases are immune to retention-policy garbage collection forever</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-06-01">
        <given>20 model checkpoints exist; oldest 5 are tagged_release=true; newest 10 are not</given>
        <when>GC step 4 (prune model checkpoints) runs with retention "keep last 10 + tagged releases"</when>
        <then>
          - The 10 newest checkpoints are kept (regardless of tag)
          - All 5 tagged-release checkpoints are kept (regardless of age)
          - The remaining 5 (older, untagged) are pruned
          - Final count: 10 + 5 = 15 checkpoints; if any prune attempt targets a tagged release, it fails with `MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED` and the GC continues without aborting
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-07" priority="must-have">
    <narrative>
      <as_a>ME-JEPA stress harness</as_a>
      <i_want_to>run a 10,000-entry stress corpus over 7 simulated days and confirm the system stays under 110 GB</i_want_to>
      <so_that>the Phase 6 DoD is verifiable</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-07-01">
        <given>A fresh ME-JEPA RocksDB instance and the stress harness `cargo run --release -p context-graph-mejepa -- stress --entries 10000 --duration 7d`</given>
        <when>The 7-day stress run completes</when>
        <then>
          - `total_disk_used_gb` ≤ 110
          - `gc_passes_completed` == 7
          - `witness_chain_compressed: bool` == true (witness compression triggered for old segments)
          - `quota_violations` == 0 after the final GC pass
          - FSV evidence at /tmp/contextgraph-mejepa-hygiene-fsv/gc-stress-evidence.json contains all 7 GC reports with per-step counts
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HYG-08" priority="should-have">
    <narrative>
      <as_a>external MCP caller (UT-04)</as_a>
      <i_want_to>trigger witness-chain Merkle compression with a custom threshold (e.g., 30 days for testing)</i_want_to>
      <so_that>I can verify compression behavior in non-production environments without waiting for the 90-day default</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HYG-08-01">
        <given>A test witness chain with entries spanning 60 days</given>
        <when>The caller invokes `mcp__cgreality__mejepa_witness_compress(threshold_days=30)`</when>
        <then>
          - Returns a `CompressReport` with: `segments_compressed: u64`, `entries_compressed: u64`, `bytes_before: u64`, `bytes_after: u64`, `compression_ratio: f64`, `merkle_roots: Vec&lt;[u8;32]&gt;`, `cold_archive_paths: Vec&lt;String&gt;` (where the original 1024-entry segments were archived)
          - Each `merkle_roots[i]` is verifiable by re-fetching `cold_archive_paths[i]` and recomputing
          - `verify_chain_integrity()` passes after the compression
          - Latency proportional to chain length (linear in number of entries)
        </then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-HYG-01" story_ref="US-HYG-01" priority="must">
    <description>The system SHALL enforce a hard total-disk quota defaulting to 115 GB and overridable via the `CG_MEJEPA_DISK_QUOTA_GB` environment variable, summed across the 9 storage categories enumerated in doc 09 §6.1.</description>
    <rationale>Without a hard ceiling the predictor host fills its disk and ME-JEPA stops serving — non-negotiable per doc 10 §F operator priority on bounded storage.</rationale>
    <constraints>The 9 categories and budgets are: mutation_corpus (5 GB, static), live_attempt_corpus (50 GB, tiered), model_checkpoints (10 GB, last 10 + tagged), witness_chains (20 GB, append-only with Merkle compression > 90d), live_panel_cache_ram (4 GB, LRU), live_panel_disk_overflow (20 GB, LRU spill), calibration_snapshots (5 GB, last 30), aux_signals_overrides (2 GB, gold-tier overrides retained indefinitely), shift_log_subscriber_state (0.5 GB, watermark + per-file cache). Total = 116.5 GB upper bound; effective default 115 GB allows ~1.5 GB buffer.</constraints>
    <verification_method>Unit test: configure each category to its budget, attempt a write that would exceed; verify rejection with `MEJEPA_HYG_QUOTA_EXCEEDED`. Integration test: stress run of 10,000 entries × 7 days reports `total_disk_used_gb &lt;= 110` (Phase 6 DoD).</verification_method>
  </requirement>

  <requirement id="REQ-HYG-02" story_ref="US-HYG-01" priority="must">
    <description>Tier compression for `live_attempt_corpus` and `live_panel_disk_overflow` SHALL use the shipped `context-graph-tier-compression` crate with bit widths 8 / 7 / 5 / 3 keyed on access frequency thresholds {≥0.8 Hot, 0.6-0.8 Warm, 0.4-0.6 Cool, 0.2-0.4 Cold, &lt;0.2 Frozen 1-bit}, and access frequency SHALL decay at 0.99 per 24 hours.</description>
    <rationale>Reuses the shipped, FSV-covered, magic+version-headed quantizer (cosines 0.99999/0.99997/0.99960/0.99192 on 768-d Gaussian); avoids reinventing the encoder.</rationale>
    <constraints>1-bit Frozen tier is NOT supplied by `context-graph-tier-compression` (which supports only 8/7/5/3); Frozen tier is implemented as a separate sign-bit-only encoding (1 bit per element + 22-byte header analogous to `CompressedBlob` but with magic `CGTF` for fail-closed dispatch). Access-frequency decay: `freq_new = freq_old * 0.99^(hours_since_last_read / 24)`; on read, `freq_new += 1.0` clamped to [0, 1].</constraints>
    <verification_method>Unit test per tier: encode → decode round-trip on 100 random 768-d vectors; verify `(max - min) / (2 * (2^bits - 1))` element-wise upper bound. FSV: /tmp/contextgraph-mejepa-hygiene-fsv/gc-stress-evidence.json reports per-tier entry counts after a 7-day stress run.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-03" story_ref="US-HYG-02" priority="must">
    <description>The nightly GC pass SHALL execute the 8 steps in order at 03:00 local: (1) demote, (2) `compact_range_cf` per CF, (3) evict bottom-1 % by `(uplift × access_frequency × recency)`, (4) prune model checkpoints (keep last 10 + tagged releases), (5) prune calibration snapshots (keep last 30), (6) compress witness chain segments older than 90 days, (7) `verify_chain_integrity` (abort on break with `MEJEPA_HYG_GC_INTEGRITY_FAIL`), (8) write report to `&lt;runtime_root&gt;/mejepa/gc-report-&lt;YYYY-MM-DD&gt;.json`.</description>
    <rationale>The 8-step ordering is structural: compaction must follow demotion (so RocksDB can reclaim freed bytes); eviction must follow compaction (so metrics reflect post-compaction sizes); chain compression must precede integrity verification (so the verifier sees the final state); report must be last (so it captures all step outcomes).</rationale>
    <constraints>Steps 4 and 5 MUST refuse to delete tagged releases (`tagged_release: true` in checkpoint metadata) — failed prune attempts surface `MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED` but do not abort the GC. Step 7 abort is hard: no commit of step 4/5 deletes occurs if step 7 detects integrity break (atomic — implemented via RocksDB write-batch staging or transactional CF). Step 8 report SHALL include per-step `{count, duration_ms, bytes_freed, errors_encountered}`.</constraints>
    <verification_method>Unit test: synthetic 10-CF instance; inject a deliberate chain integrity break before step 7; verify abort + no checkpoint deletes committed. Integration test: stress run reports `gc_passes_completed: 7` over 7 days. FSV: gc-stress-evidence.json contains 7 per-day reports.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-04" story_ref="US-HYG-03" priority="must">
    <description>Witness-chain Merkle compression SHALL group consecutive 1024-entry segments older than the configurable threshold (default 90 days) into a single "compressed-segment" entry containing only the Merkle root + segment metadata (start_timestamp_ns, end_timestamp_ns, entry_count, original_segment_archive_path), and SHALL preserve the chain's `prev_hash` linking such that `verify_chain_integrity` continues to pass.</description>
    <rationale>Storage saving: 1024 × 73 B = 74,752 B → ~100 B per compressed segment (~750× per segment); 100,000-entry chain compresses from ~7 MB to ~10 KB (~700× overall per Phase 6 DoD). Auditability preserved: any old entry verifiable by re-fetching its 1024-entry archive and recomputing the Merkle root via the canonical `MERKLE_LEAF_TAG` / `MERKLE_NODE_TAG` from `context-graph-witness::cert`.</rationale>
    <constraints>The original 1024-entry segments MUST be archived to `cold_archive_paths` (operator-configurable; default `&lt;runtime_root&gt;/mejepa/witness-archive/&lt;segment_id&gt;.bin`) BEFORE the compressed-segment entry replaces them in the live chain — atomic copy-then-replace. Compressed-segment entries reuse the 73-byte witness format with `witness_type` discriminator (e.g., `WITNESS_TYPE_COMPRESSED_SEGMENT = 0x80` to keep low-byte-range for live entries). Threshold age MUST be configurable but NEVER less than 1 day (would compress live activity).</constraints>
    <verification_method>Unit test: 2,048-entry synthetic chain (2 segments) compressed at threshold 0; verify resulting chain has 2 compressed-segment entries; verify Merkle roots by recomputing from archived segments. FSV: /tmp/contextgraph-mejepa-hygiene-fsv/merkle-compression-evidence.json contains the 100,000-entry case showing ~7 MB → ~10 KB compression. Phase 6 DoD verification command: `cargo run --release -p context-graph-mejepa -- stress --entries 10000 --duration 7d` reports `witness_chain_compressed: yes`.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-05" story_ref="US-HYG-01,US-HYG-04" priority="must">
    <description>`quota_check_and_evict` SHALL trigger emergency eviction immediately (not nightly) when any category exceeds its budget; if one full eviction pass cannot bring the category under budget, the system SHALL transition that category to `Unrecoverable` state, raise an alert (logged + surfaced via `mejepa_quota_status`), and stop accepting new data into that category until manual intervention.</description>
    <rationale>Hard quota means "the disk does not fill"; graceful degradation means "predictions still serve when one category is full" — together they guarantee uptime + bounded storage simultaneously.</rationale>
    <constraints>Emergency eviction reuses the same `(uplift × access_frequency × recency)` scoring as nightly step 3, but evicts until under budget rather than the bottom-1 %. `Unrecoverable` state is sticky: only operator action (calling `mejepa_quota_status` with explicit override flag, or manually deleting entries via maintenance script) clears it. Predictions continue serving because read paths do not write to over-budget categories.</constraints>
    <verification_method>Unit test: configure tiny synthetic budget (e.g., 1 MB); fill to 110 %; call `quota_check_and_evict`; verify category reaches healthy state. Negative test: configure budget such that all entries are gold-tier (cannot be evicted); verify transition to `Unrecoverable` + `MEJEPA_HYG_QUOTA_UNRECOVERABLE` returned to subsequent writers. FSV: /tmp/contextgraph-mejepa-hygiene-fsv/quota-overflow-evidence.json captures both cases.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-06" story_ref="US-HYG-05" priority="must">
    <description>Tier promotion on read SHALL increment access frequency per the §6.2 schedule and SHALL re-encode + re-persist the entry at a hotter tier when the new frequency crosses the next threshold; tier demotion on the nightly pass SHALL re-encode + re-persist at a colder tier when the frequency dropped below the current tier's lower bound.</description>
    <rationale>Bidirectional tiering is the §6.2 design — read = promote = restore fidelity for hot entries; idle = demote = save storage for cold entries.</rationale>
    <constraints>Re-encode failures (e.g., source blob corrupt) MUST surface `MEJEPA_HYG_TIER_DECOMPRESS_FAIL` with offending bytes + entry_id and MUST NOT silently degrade to a default value. Frozen-tier 1-bit demotion uses sign bit only (`CGTF` magic, separate from `CGTC`); promotion from Frozen reads only the sign bit and reconstructs at the requested target tier with explicit "fidelity_lost: true" flag in the panel meta (the operator can choose to re-materialize from upstream source if available).</constraints>
    <verification_method>Unit test: place an entry at Cold; simulate 5 reads in 1 hour; verify promotion to Cool then Warm; verify CF_MEJEPA_PANEL_META records each transition. FSV: gc-stress-evidence.json reports per-day promotion + demotion counts.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-07" story_ref="US-HYG-08" priority="must">
    <description>Three new MCP tools SHALL be exposed: `mcp__cgreality__mejepa_gc_run` (returns `GcReport`), `mcp__cgreality__mejepa_quota_status` (returns `QuotaStatus`), `mcp__cgreality__mejepa_witness_compress(threshold_days: u32)` (returns `CompressReport`). All three SHALL be MCP-callable for operator inspection per doc 09 §6.6 + Phase 6 DoD.</description>
    <rationale>External operator + audit inspection requires structured, schema-validated MCP responses; bypassing the MCP boundary would break the constitution's `mcp_tool_namespace_lock`.</rationale>
    <constraints>Tool names use the `mcp__cgreality__mejepa_*` prefix per the constitution's `mcp_tool_namespace_lock`. Schemas reject `additionalProperties` (per the project pattern in MEMORY.md "additionalProperties:false blocks unlisted params"). `mejepa_gc_run` is destructive (mutates CFs) — its MCP schema MUST require explicit operator confirmation flag (`confirm_run: true`) to prevent accidental triggers. `mejepa_witness_compress` accepts `threshold_days` (u32, min 1, max 36500); rejects 0 fail-closed.</constraints>
    <verification_method>Integration test: spawn the MCP server in test mode; call each tool; verify schema-validated response. Negative test: call `mejepa_gc_run` without `confirm_run`; verify rejection. Audit log: every tool invocation appends to the witness chain with `witness_type` for the operation.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-08" story_ref="US-HYG-02" priority="must">
    <description>The `me_jepa::ops::*` module SHALL expose the 8 primitive functions per doc 09 §6.6: `gc_run_nightly`, `quota_check_and_evict`, `heal_detect_drift`, `heal_retrain_if_needed`, `witness_compress_old_segments`, `witness_verify_integrity`, `tier_demote`, `tier_promote`. The two cross-cutting ops (`heal_detect_drift`, `heal_retrain_if_needed`) SHALL forward to SPEC-SELF-HEALING (Phase 5) implementations; this spec SHALL specify only their fail-closed contracts.</description>
    <rationale>The 8-primitive interface is the doc 09 §6.6 contract; cross-cutting boundary with Phase 5 must be explicit so this spec doesn't accidentally re-specify drift detection semantics.</rationale>
    <constraints>All 8 ops return `Result&lt;_, OpsError&gt;`. `heal_detect_drift` and `heal_retrain_if_needed` defer to `crates/context-graph-mejepa/src/self_healing/` (Phase 5); if Phase 5 is not yet shipped, they return `OpsError::CrossCuttingDeferred { spec: "SPEC-SELF-HEALING" }` fail-closed (no silent no-op).</constraints>
    <verification_method>Unit test per op; deferred-Phase-5 test confirms structured error variant. Surface test: every op is reachable via the MCP shim or the binary CLI.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-09" story_ref="US-HYG-06" priority="must">
    <description>Tagged-release model checkpoints SHALL be immune to retention-policy pruning. Any prune attempt against a tagged release SHALL fail with `MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED` and SHALL NOT abort the GC pass.</description>
    <rationale>Operator-tagged releases are gold provenance — losing them silently would destroy reproducibility of evaluations done against those weights.</rationale>
    <constraints>Tag flag stored as `tagged_release: bool` in checkpoint metadata (CF_MEJEPA_WEIGHT_BLOBS sibling key). Tags can only be set/cleared by operator via dedicated tool (out of this spec's scope; lives in SPEC-SELF-HEALING or operator runbook). Failed prune is logged at WARN with `(checkpoint_id, theta_sha, tagged_at_ts)`.</constraints>
    <verification_method>Unit test US-HYG-06 AC-HYG-06-01. FSV: gc-stress-evidence.json includes a tagged-release injection scenario.</verification_method>
  </requirement>

  <requirement id="REQ-HYG-10" story_ref="US-HYG-04" priority="should">
    <description>`mejepa_quota_status` SHALL report each category's `degradation_status: { Healthy | Approaching | OverBudget | Unrecoverable }` with thresholds: Healthy &lt; 80 %, Approaching 80-95 %, OverBudget 95-100 %, Unrecoverable post-failed-GC.</description>
    <rationale>Honest signal to operator + CI; "approaching" gives early warning before emergency eviction.</rationale>
    <constraints>Thresholds are hard-coded constants; not operator-configurable in v1.0 (constitution amendment to change).</constraints>
    <verification_method>Unit test: each threshold boundary; verify reported status.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-HYG-01" req_ref="REQ-HYG-01">
    <condition>Any single category usage exceeds its budget AND total disk usage is below the 115 GB ceiling</condition>
    <action>Trigger `quota_check_and_evict` for that category immediately; do not wait for nightly GC</action>
    <exception>If the category is `aux_signals_overrides` AND the entry is operator-tagged gold-tier, do not evict; surface `MEJEPA_HYG_QUOTA_EXCEEDED` to the writer instead</exception>
  </rule>

  <rule id="BR-HYG-02" req_ref="REQ-HYG-03">
    <condition>Nightly GC step 7 (`verify_chain_integrity`) detects any break</condition>
    <action>Abort the GC pass; do NOT commit step 4/5 deletes; surface `MEJEPA_HYG_GC_INTEGRITY_FAIL`; halt scheduler until operator inspection</action>
    <exception>None — integrity break is a structural bug that requires human intervention; auto-recovery would mask the underlying issue</exception>
  </rule>

  <rule id="BR-HYG-03" req_ref="REQ-HYG-04">
    <condition>A witness segment older than `threshold_age` (default 90 days) is encountered during compression</condition>
    <action>Atomically: (1) archive original 1024-entry segment to cold storage with provenance, (2) compute Merkle root, (3) write compressed-segment entry, (4) replace segment in live chain</action>
    <exception>If atomic operation fails at any step (e.g., disk full during archive write), the original segment remains in the live chain unchanged; `MEJEPA_HYG_QUOTA_EXCEEDED` propagates to the caller</exception>
  </rule>

  <rule id="BR-HYG-04" req_ref="REQ-HYG-02">
    <condition>An access-frequency increment causes the value to cross a tier threshold (e.g., 0.59 → 0.61, crossing Cool→Warm boundary at 0.6)</condition>
    <action>Re-encode the entry at the new tier's bit width and re-persist; record promotion event in `CF_MEJEPA_PANEL_META`</action>
    <exception>If decompression of the source blob fails (`MEJEPA_HYG_TIER_DECOMPRESS_FAIL`), surface the error to the reader; do NOT promote a corrupt entry; do NOT silently zero-fill</exception>
  </rule>

  <rule id="BR-HYG-05" req_ref="REQ-HYG-09">
    <condition>GC step 4 attempts to prune a checkpoint with `tagged_release: true`</condition>
    <action>Skip the prune; log at WARN level with `(checkpoint_id, theta_sha, tagged_at_ts)`; emit `MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED` event to the GC report; continue with next candidate</action>
    <exception>None — tagged releases are immutable provenance</exception>
  </rule>

  <rule id="BR-HYG-06" req_ref="REQ-HYG-08">
    <condition>`heal_detect_drift` or `heal_retrain_if_needed` is called AND SPEC-SELF-HEALING (Phase 5) implementation is not yet shipped</condition>
    <action>Return `OpsError::CrossCuttingDeferred { spec: "SPEC-SELF-HEALING" }` fail-closed</action>
    <exception>None — no silent no-op; the caller must observe the deferral</exception>
  </rule>

  <rule id="BR-HYG-07" req_ref="REQ-HYG-03">
    <condition>Nightly GC scheduler invocation arrives during a Phase 5 retrain mid-promotion</condition>
    <action>Defer GC; emit `GcReport { deferred: true, deferred_reason: "retrain_mid_promotion" }`; reschedule for next 03:00 OR operator-configured retry interval</action>
    <exception>If deferrals exceed 7 consecutive nights, raise alert (graceful-degradation territory; operator must intervene)</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-HYG-01" req_ref="REQ-HYG-04">
    <scenario>The witness chain has fewer than 1024 entries older than the threshold age (e.g., 500 old entries) — cannot form a complete 1024-entry segment</scenario>
    <expected_behavior>Compression skips the partial segment; old entries remain uncompressed until the segment fills. The threshold age applies to the OLDEST entry in the candidate segment; partial segments wait until they fill to 1024 entries OR the operator manually invokes a "compress partial segments" override (out of scope for v1.0)</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HYG-02" req_ref="REQ-HYG-02">
    <scenario>An entry's access frequency drops to exactly 0.0 (no reads for very long time) — would map to Frozen 1-bit, but the reconstruction error at 1-bit is significant</scenario>
    <expected_behavior>The entry is demoted to Frozen 1-bit (sign-bit-only encoding); subsequent reads return reconstructed values with the documented "fidelity_lost: true" flag in the panel meta. The operator can re-materialize from upstream source if available; the system does not silently zero-fill</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HYG-03" req_ref="REQ-HYG-05">
    <scenario>All entries in a category are gold-tier (operator overrides retained indefinitely); category exceeds budget; GC has no eviction candidates</scenario>
    <expected_behavior>Category transitions to `Unrecoverable` after one full GC pass; alert raised; new writes to the category are rejected with `MEJEPA_HYG_QUOTA_UNRECOVERABLE`; predictions continue serving from existing entries</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HYG-04" req_ref="REQ-HYG-04">
    <scenario>Cold archive write fails during witness segment compression (e.g., archive disk separately full)</scenario>
    <expected_behavior>The atomic archive-then-replace operation rolls back; the original segment remains in the live chain unchanged; `MEJEPA_HYG_QUOTA_EXCEEDED` propagates with archive_path field; GC step 6 logs the failure and continues with step 7 (integrity verification will pass since the live chain is unchanged)</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HYG-05" req_ref="REQ-HYG-06">
    <scenario>A tier-compressed blob's bytes are corrupted (e.g., bit flip on disk)</scenario>
    <expected_behavior>`tier_promote` decompression fails magic-bytes check OR header parsing OR sub-byte unpacking; returns `MEJEPA_HYG_TIER_DECOMPRESS_FAIL` with offending bytes; the entry is marked `corrupt: true` in `CF_MEJEPA_PANEL_META`; subsequent reads of this entry surface the same error; GC step 3 (eviction) prioritizes corrupt entries for removal in the next pass</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-HYG-06" req_ref="REQ-HYG-03">
    <scenario>The system is started for the first time (empty CFs); GC runs at 03:00 the first night</scenario>
    <expected_behavior>All 8 steps execute; counts are zero; report shows `bytes_freed: 0` per step; `verify_chain_integrity` passes vacuously on empty chain; GC report still written to disk</expected_behavior>
    <priority>low</priority>
  </edge_case>

  <edge_case id="EC-HYG-07" req_ref="REQ-HYG-04">
    <scenario>The cold archive is unavailable when an auditor requests verification of an old entry (network mount down, archive disk offline)</scenario>
    <expected_behavior>Verification returns `MEJEPA_HYG_MERKLE_VERIFY_FAIL` with sub-error `cold_archive_unavailable`; the live chain itself remains verifiable (`verify_chain_integrity` passes because compressed-segment entries chain correctly); the operator must restore the archive and retry</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HYG-08" req_ref="REQ-HYG-01">
    <scenario>Operator sets `CG_MEJEPA_DISK_QUOTA_GB=50` (below the sum of category budgets, 115)</scenario>
    <expected_behavior>System honors the lower override; per-category budgets are scaled proportionally OR the system surfaces a startup warning that the override is below the recommended floor + clamps to the override; logged at WARN. Out-of-the-box behavior: scale all categories proportionally (e.g., 50/115 = 43.5 % of each default budget)</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HYG-09" req_ref="REQ-HYG-04">
    <scenario>Witness chain Merkle compression produces a Merkle root that, when re-verified, mismatches due to non-deterministic ordering of entries within a segment</scenario>
    <expected_behavior>This is a structural bug — Merkle tree construction MUST be deterministic given the same entry sequence. Re-verification returns `MEJEPA_HYG_MERKLE_VERIFY_FAIL`; the operator inspects the failure and the bug is fixed at the construction site. No silent recovery</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-HYG-10" req_ref="REQ-HYG-09">
    <scenario>Operator tags a checkpoint as gold-tier AFTER it has been pruned by GC</scenario>
    <expected_behavior>Tag operation fails fast: `CHECKPOINT_NOT_FOUND` (the bytes are gone); operator must restore from external backup. Mitigation: tagging is an explicit operator action and SHOULD be done before retention pressure builds; not a retention-policy concern</expected_behavior>
    <priority>low</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-HYG-01" http_code="507" req_ref="REQ-HYG-01,REQ-HYG-05">
    <condition>A category exceeds its budget; emergency eviction is triggered</condition>
    <user_message>"Storage quota exceeded for category {category}: used {used_gb} GB, budget {budget_gb} GB. Emergency eviction in progress."</user_message>
    <internal_message>"MEJEPA_HYG_QUOTA_EXCEEDED category={category} used_bytes={used} budget_bytes={budget} action=emergency_evict_started timestamp={ts}"</internal_message>
    <recovery_action>Wait for emergency eviction to complete; retry the write. If category transitions to `Unrecoverable`, operator intervention required (see ERR-HYG-02)</recovery_action>
  </error>

  <error id="ERR-HYG-02" http_code="507" req_ref="REQ-HYG-05">
    <condition>One full GC eviction pass cannot bring a category under budget; category enters `Unrecoverable` state</condition>
    <user_message>"Storage quota for category {category} is unrecoverable: GC pass freed {bytes_freed} bytes but category remains at {used_gb} GB > {budget_gb} GB. New writes to this category are rejected. Operator intervention required."</user_message>
    <internal_message>"MEJEPA_HYG_QUOTA_UNRECOVERABLE category={category} pass_freed_bytes={freed} pass_duration_ms={dur} unrecoverable_since={ts} blocking_writes=true"</internal_message>
    <recovery_action>Operator manually evicts entries via maintenance script OR raises `CG_MEJEPA_DISK_QUOTA_GB` (if total disk has headroom) OR archives the category to external storage and clears CFs</recovery_action>
  </error>

  <error id="ERR-HYG-03" http_code="500" req_ref="REQ-HYG-06">
    <condition>Tier-compressed blob fails decompression (wrong magic / truncated / bit-flip)</condition>
    <user_message>"Failed to decompress entry {entry_id}: {reason}. The entry is marked corrupt and will be evicted on next GC pass."</user_message>
    <internal_message>"MEJEPA_HYG_TIER_DECOMPRESS_FAIL entry_id={id} expected_magic=CGTC actual_magic={hex} bytes_len={len} sub_error={detail}"</internal_message>
    <recovery_action>Mark entry corrupt; re-materialize from upstream source if available; otherwise wait for GC eviction</recovery_action>
  </error>

  <error id="ERR-HYG-04" http_code="500" req_ref="REQ-HYG-03">
    <condition>Witness chain integrity check fails at end of GC step 7</condition>
    <user_message>"Garbage collection aborted: witness chain integrity check failed at offset {offset}. No checkpoint deletes were committed. Operator inspection required."</user_message>
    <internal_message>"MEJEPA_HYG_GC_INTEGRITY_FAIL offset={offset} expected_prev_hash={hex} actual_prev_hash={hex} gc_run_id={uuid} aborted_at_step=7 staged_deletes_rolled_back={count}"</internal_message>
    <recovery_action>Operator inspects witness chain; runs `witness_chain_repair` (see SPEC-WITNESS); re-runs GC after repair</recovery_action>
  </error>

  <error id="ERR-HYG-05" http_code="500" req_ref="REQ-HYG-04">
    <condition>Compressed-segment Merkle root mismatch on verification (re-fetched 1024-entry archive does not produce the stored root)</condition>
    <user_message>"Merkle verification failed for segment {segment_id}: stored root {hex} does not match recomputed root {hex}. The segment archive may be corrupt or tampered."</user_message>
    <internal_message>"MEJEPA_HYG_MERKLE_VERIFY_FAIL segment_id={id} stored_root={hex} recomputed_root={hex} archive_path={path} entry_count={n}"</internal_message>
    <recovery_action>Operator inspects archive bytes; restores from external backup; structural bug investigation if multiple segments fail</recovery_action>
  </error>

  <error id="ERR-HYG-06" http_code="409" req_ref="REQ-HYG-09">
    <condition>GC step 4 attempts to prune a tagged-release checkpoint</condition>
    <user_message>"Refused to prune tagged-release checkpoint {checkpoint_id} (tagged at {tagged_at_ts}). Tagged releases are retained indefinitely."</user_message>
    <internal_message>"MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED checkpoint_id={id} theta_sha={hex} tagged_at_ts={ts} retention_policy=keep_last_10_plus_tagged_releases"</internal_message>
    <recovery_action>None required — this is the intended fail-closed behavior; operator can untag the release explicitly if pruning is desired</recovery_action>
  </error>

  <error id="ERR-HYG-07" http_code="503" req_ref="REQ-HYG-08">
    <condition>`heal_detect_drift` or `heal_retrain_if_needed` invoked while SPEC-SELF-HEALING (Phase 5) is not yet shipped</condition>
    <user_message>"Self-healing operation deferred: SPEC-SELF-HEALING (Phase 5) is not yet implemented."</user_message>
    <internal_message>"OpsError::CrossCuttingDeferred spec=SPEC-SELF-HEALING op={op_name} requested_at={ts}"</internal_message>
    <recovery_action>Wait for Phase 5 implementation; the MCP shim is the only Phase 6 deliverable for these ops</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="GcReport">
    <field name="gc_run_id" type="Uuid" required="true">
      <description>Unique ID for this GC pass; appears in witness chain entry for the run</description>
      <constraints>Generated at start of pass</constraints>
    </field>
    <field name="started_at" type="DateTime&lt;Utc&gt;" required="true">
      <description>Pass start timestamp</description>
      <constraints>ISO 8601</constraints>
    </field>
    <field name="completed_at" type="Option&lt;DateTime&lt;Utc&gt;&gt;" required="false">
      <description>Pass end timestamp; None if aborted</description>
      <constraints>None when aborted</constraints>
    </field>
    <field name="deferred" type="bool" required="true">
      <description>True if pass was deferred (e.g., retrain mid-promotion)</description>
      <constraints>Default false</constraints>
    </field>
    <field name="deferred_reason" type="Option&lt;String&gt;" required="false">
      <description>Human-readable deferral reason</description>
      <constraints>Required when deferred=true</constraints>
    </field>
    <field name="aborted" type="bool" required="true">
      <description>True if pass aborted (e.g., integrity check failed at step 7)</description>
      <constraints>Default false</constraints>
    </field>
    <field name="abort_reason" type="Option&lt;String&gt;" required="false">
      <description>Structured abort reason; e.g., "MEJEPA_HYG_GC_INTEGRITY_FAIL"</description>
      <constraints>Required when aborted=true</constraints>
    </field>
    <field name="step_reports" type="Vec&lt;GcStepReport&gt;" required="true">
      <description>Per-step report; one entry per executed step (some may be empty if pass aborted early)</description>
      <constraints>Length 1-8; ordered</constraints>
    </field>
    <field name="total_disk_used_gb_before" type="f64" required="true">
      <description>Total disk usage before pass</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="total_disk_used_gb_after" type="f64" required="true">
      <description>Total disk usage after pass</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="integrity_check_passed" type="bool" required="true">
      <description>Whether step 7 succeeded</description>
      <constraints>If false, pass is aborted</constraints>
    </field>
  </entity>

  <entity name="GcStepReport">
    <field name="step_index" type="u8" required="true">
      <description>1-8 per the §6.3 ordering</description>
      <constraints>1 ≤ x ≤ 8</constraints>
    </field>
    <field name="step_name" type="String" required="true">
      <description>"demote" | "compact" | "evict" | "prune_checkpoints" | "prune_calibration" | "compress_witness" | "verify_integrity" | "report"</description>
      <constraints>Snake_case enum</constraints>
    </field>
    <field name="duration_ms" type="u64" required="true">
      <description>Wall-clock duration</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="entries_processed" type="u64" required="true">
      <description>Count of entries the step touched</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="bytes_freed" type="i64" required="true">
      <description>Bytes freed by this step (can be negative for compress_witness if archive write was larger than freed bytes during in-flight)</description>
      <constraints>None</constraints>
    </field>
    <field name="errors_encountered" type="Vec&lt;String&gt;" required="true">
      <description>Structured error codes encountered (e.g., MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED) that did NOT abort the step</description>
      <constraints>Empty list if clean</constraints>
    </field>
  </entity>

  <entity name="QuotaStatus">
    <field name="effective_quota_gb" type="f64" required="true">
      <description>The active total quota; CG_MEJEPA_DISK_QUOTA_GB env value or 115 default</description>
      <constraints>Positive</constraints>
    </field>
    <field name="total_disk_used_gb" type="f64" required="true">
      <description>Sum of all category usages</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="categories" type="Vec&lt;CategoryStatus&gt;" required="true">
      <description>One per category from doc 09 §6.1 (9 entries)</description>
      <constraints>Length == 9</constraints>
    </field>
    <field name="system_accepts_new_data_in_categories" type="HashSet&lt;Category&gt;" required="true">
      <description>Categories that are not in Unrecoverable state and accept new writes</description>
      <constraints>Subset of all 9 categories</constraints>
    </field>
    <field name="snapshot_at" type="DateTime&lt;Utc&gt;" required="true">
      <description>When this snapshot was taken</description>
      <constraints>ISO 8601</constraints>
    </field>
  </entity>

  <entity name="CategoryStatus">
    <field name="category" type="Category" required="true">
      <description>Enum of 9 categories</description>
      <constraints>One of: MutationCorpus, LiveAttemptCorpus, ModelCheckpoints, WitnessChains, LivePanelCacheRam, LivePanelDiskOverflow, CalibrationSnapshots, AuxSignalsOverrides, ShiftLogSubscriberState</constraints>
    </field>
    <field name="budget_bytes" type="u64" required="true">
      <description>Budget per doc 09 §6.1</description>
      <constraints>Positive</constraints>
    </field>
    <field name="used_bytes" type="u64" required="true">
      <description>Current usage</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="degradation_status" type="DegradationStatus" required="true">
      <description>Healthy &lt; 80%, Approaching 80-95%, OverBudget 95-100%, Unrecoverable post-failed-GC</description>
      <constraints>Enum</constraints>
    </field>
    <field name="tier_distribution" type="HashMap&lt;Tier, u64&gt;" required="false">
      <description>For tiered categories: count per tier {Hot, Warm, Cool, Cold, Frozen}</description>
      <constraints>Empty for non-tiered categories (e.g., MutationCorpus)</constraints>
    </field>
    <field name="oldest_entry_age_seconds" type="Option&lt;u64&gt;" required="false">
      <description>Age of oldest entry in seconds; None if category empty</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="last_gc_attempt_ts" type="Option&lt;DateTime&lt;Utc&gt;&gt;" required="false">
      <description>Last GC pass that touched this category</description>
      <constraints>None if no GC has run</constraints>
    </field>
    <field name="last_gc_outcome" type="Option&lt;String&gt;" required="false">
      <description>"freed_N_bytes" | "no_progress" | "deferred"</description>
      <constraints>None if no GC has run</constraints>
    </field>
  </entity>

  <entity name="CompressReport">
    <field name="threshold_days" type="u32" required="true">
      <description>The threshold used for this run</description>
      <constraints>1 ≤ x ≤ 36500</constraints>
    </field>
    <field name="segments_compressed" type="u64" required="true">
      <description>Count of 1024-entry segments compressed</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="entries_compressed" type="u64" required="true">
      <description>= segments_compressed * 1024</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="bytes_before" type="u64" required="true">
      <description>Live-chain bytes before compression</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="bytes_after" type="u64" required="true">
      <description>Live-chain bytes after compression (excludes archived bytes)</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="compression_ratio" type="f64" required="true">
      <description>bytes_before / bytes_after</description>
      <constraints>≥ 1.0</constraints>
    </field>
    <field name="merkle_roots" type="Vec&lt;[u8; 32]&gt;" required="true">
      <description>One per compressed segment; verifiable via cold_archive_paths</description>
      <constraints>Length == segments_compressed</constraints>
    </field>
    <field name="cold_archive_paths" type="Vec&lt;String&gt;" required="true">
      <description>Filesystem paths where original 1024-entry segments are archived</description>
      <constraints>Length == segments_compressed; each path resolvable on local filesystem</constraints>
    </field>
    <field name="integrity_check_passed" type="bool" required="true">
      <description>Whether `verify_chain_integrity` passes after compression</description>
      <constraints>True (else CompressReport is itself corrupt)</constraints>
    </field>
  </entity>

  <entity name="EvictReport">
    <field name="trigger" type="EvictTrigger" required="true">
      <description>EmergencyOverBudget | NightlyBottom1Pct | OperatorManual</description>
      <constraints>Enum</constraints>
    </field>
    <field name="category" type="Category" required="true">
      <description>The category that was evicted from</description>
      <constraints>One of 9</constraints>
    </field>
    <field name="entries_evicted" type="u64" required="true">
      <description>Count of entries removed</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="bytes_freed" type="u64" required="true">
      <description>Bytes freed</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="reached_under_budget" type="bool" required="true">
      <description>Whether the category is now under budget</description>
      <constraints>If false, category transitions to Unrecoverable on emergency triggers</constraints>
    </field>
    <field name="duration_ms" type="u64" required="true">
      <description>Wall-clock duration of eviction</description>
      <constraints>Non-negative</constraints>
    </field>
  </entity>

  <entity name="ChainIntegrityReport">
    <field name="entries_verified" type="u64" required="true">
      <description>Count of live + compressed-segment entries verified</description>
      <constraints>Non-negative</constraints>
    </field>
    <field name="passed" type="bool" required="true">
      <description>True if every prev_hash links and every signature (where present) verifies</description>
      <constraints>None</constraints>
    </field>
    <field name="break_offset" type="Option&lt;usize&gt;" required="false">
      <description>Byte offset of first break; None if passed=true</description>
      <constraints>Required when passed=false</constraints>
    </field>
    <field name="duration_ms" type="u64" required="true">
      <description>Wall-clock duration</description>
      <constraints>Non-negative</constraints>
    </field>
  </entity>

  <entity name="EntryId">
    <field name="cf_name" type="String" required="true">
      <description>RocksDB CF the entry lives in (e.g., "CF_MEJEPA_PANELS")</description>
      <constraints>Must be one of the registered ME-JEPA CFs</constraints>
    </field>
    <field name="key" type="Vec&lt;u8&gt;" required="true">
      <description>RocksDB key (binary)</description>
      <constraints>Non-empty</constraints>
    </field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-HYG-01" category="performance">
    <description>Nightly GC pass total wall-clock SHALL complete within 60 minutes for the steady-state corpus (~115 GB across 9 categories) on RTX 5090 host with NVMe SSD</description>
    <metric>p95 GC pass duration ≤ 60 min over 30-night rolling window</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HYG-02" category="performance">
    <description>`tier_promote` on a single Cool→Warm transition SHALL complete in &lt; 5 ms p95 (decompress + re-encode + write)</description>
    <metric>p95 latency &lt; 5 ms over 1,000-promotion benchmark</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-HYG-03" category="performance">
    <description>`mejepa_quota_status` SHALL return in &lt; 50 ms p50 (read-only snapshot from RocksDB metadata)</description>
    <metric>p50 latency &lt; 50 ms; p99 &lt; 200 ms</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-HYG-04" category="reliability">
    <description>The 8-step nightly GC pass SHALL be atomic per step: a step's mutations either fully commit or fully roll back; no partial state on crash mid-step</description>
    <metric>Power-cycle test mid-step recovers to last-committed-step state</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HYG-05" category="reliability">
    <description>Witness-chain Merkle compression SHALL be deterministic: the same input segment produces the same Merkle root byte-equal across hosts and runs</description>
    <metric>Cross-host reproduction test: 1,000 segments compressed on host A and host B produce identical Merkle roots</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HYG-06" category="security">
    <description>FSV evidence files at /tmp/contextgraph-mejepa-hygiene-fsv/ SHALL be created with mode 0o600 per SEC-MEJEPA-07</description>
    <metric>Filesystem audit: every file in the FSV dir has mode 0o600</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HYG-07" category="usability">
    <description>GC reports at `&lt;runtime_root&gt;/mejepa/gc-report-&lt;date&gt;.json` SHALL be human-readable JSON (pretty-printed, 2-space indent) so operators can `cat` them without tooling</description>
    <metric>Manual operator inspection: each report parseable with `jq` and human-readable</metric>
    <priority>could</priority>
  </nfr>

  <nfr id="NFR-HYG-08" category="reliability">
    <description>Tier compression round-trip SHALL preserve cosine similarity ≥ 0.99 for Hot/Warm/Cool tiers and ≥ 0.99 for Cold tier on 768-d Gaussian inputs (matching the shipped FSV evidence: 0.99999/0.99997/0.99960/0.99192)</description>
    <metric>1,000-vector round-trip benchmark per tier; cosine similarity distribution; min ≥ 0.99 for tiers 8/7/5; min ≥ 0.99 for tier 3</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>context-graph-tier-compression (✅ shipped)</name>
    <description>8/7/5/3-bit groupwise linear scalar quantization with magic+version-headed serialization; 22-byte header; 21 tests; FSV at /tmp/contextgraph-tier-compression-fsv/{blob_8,blob_7,blob_5,blob_3}.bin with cosines 0.99999/0.99997/0.99960/0.99192 on 768-d Gaussian; ratios 3.89× / 4.43× / 6.12× / 9.91×</description>
    <impact>Phase 6 hygiene cannot ship without the encoder. Crate is shipped + FSV-covered as of 2026-05-09c</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mincut (✅ shipped)</name>
    <description>Stoer-Wagner global mincut; dense O(V³) variant; fail-closed on negative weights / NaN / disconnected graphs</description>
    <impact>Reused in Phase 6 GC step 3 for repo-atomic eviction scoring (when evicting bottom-1% of corpus, prefer cuts that minimize cross-task leakage to keep training/holdout splits stable). Also reused upstream by SPEC-CORPUS Phase 0 split. Shipped + FSV-covered as of 2026-05-09</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-witness (✅ shipped — cert.rs Merkle primitive)</name>
    <description>73-byte SHAKE-256 witness chain entries + chain hashing + verify_chain_integrity + Stage 0/F/Merkle certs in cert.rs (MerkleTree, MerkleProof, MERKLE_LEAF_TAG, MERKLE_NODE_TAG)</description>
    <impact>Phase 6 §6.4 reuses cert.rs MerkleTree directly for compressed-segment construction; no separate Merkle implementation needed. The 73-byte format is reused for compressed-segment entries with new `witness_type` discriminator</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-SELF-HEALING (Phase 5, deferred)</name>
    <description>Owns drift detection + retrain semantics; provides bodies for `heal_detect_drift` and `heal_retrain_if_needed`</description>
    <impact>Cross-cutting boundary. This spec exposes only the MCP shim + fail-closed `OpsError::CrossCuttingDeferred` until Phase 5 ships. Phase 6 can ship its own deliverables (gc, tier, witness compression, quota) without Phase 5</impact>
  </dependency>

  <dependency type="external">
    <name>RocksDB (per CLAUDE.md tech stack)</name>
    <description>`compact_range_cf` for nightly compaction; CF-level deletes for eviction; metadata reads for `mejepa_quota_status`</description>
    <impact>Hygiene depends on RocksDB CF semantics; no fallback. Pinned to constitution's `rocksdb 0.21.x`</impact>
  </dependency>

  <dependency type="external">
    <name>cron / systemd timer (operator host)</name>
    <description>Triggers `gc_run_nightly()` at 03:00 local</description>
    <impact>If unavailable, operator manually invokes `mcp__cgreality__mejepa_gc_run` daily; degraded but functional</impact>
  </dependency>

  <dependency type="external">
    <name>cold archive storage (operator-configured)</name>
    <description>Filesystem path (or S3/GCS in future) where original 1024-entry witness segments are written before being replaced by compressed-segment entries</description>
    <impact>If unavailable, witness compression refuses (ERR-HYG-04 / atomic rollback); old entries remain in live chain unchanged. Operator must configure before enabling §6.4</impact>
  </dependency>

</dependencies>

<out_of_scope>

  <item>Drift detection algorithm + retrain semantics — handled by SPEC-SELF-HEALING (Phase 5). This spec only exposes MCP shim for `heal_detect_drift` / `heal_retrain_if_needed`</item>
  <item>EWC++ Fisher matrix accumulation — handled by SPEC-SELF-HEALING (Phase 5)</item>
  <item>A/B/C retrain promotion gating — handled by SPEC-SELF-HEALING (Phase 5)</item>
  <item>Conformal calibration threshold updates — handled by SPEC-INFERENCE (Phase 4a) + SPEC-SELF-HEALING (Phase 5)</item>
  <item>Phase 0 mutation corpus generation — handled by SPEC-CORPUS; this spec only consumes its CFs as a quota category</item>
  <item>Panel materialization — handled by SPEC-INSTRUMENTS; this spec only consumes its CFs as a quota category</item>
  <item>Cold-archive durability + cross-region replication — operator-policy; this spec only writes to the configured archive_root and reports paths</item>
  <item>Operator override commands for tagging checkpoints / setting CG_MEJEPA_DISK_QUOTA_GB — operator runbook (out of spec scope)</item>
  <item>Distributed multi-host hygiene coordination — single-host scope per the constitution's RTX 5090 deployment target</item>

</out_of_scope>

<test_plan>

  <test_case id="TC-HYG-01" type="unit" req_ref="REQ-HYG-01">
    <description>Verify total disk quota enforcement: configure each category to its budget, attempt overflow write, verify rejection</description>
    <preconditions>Fresh RocksDB instance; `CG_MEJEPA_DISK_QUOTA_GB` unset (defaults to 115)</preconditions>
    <inputs>9 categories filled to 100 % of budget; one additional write attempt per category</inputs>
    <expected_result>Each write returns `MEJEPA_HYG_QUOTA_EXCEEDED` with category + used_bytes + budget_bytes</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HYG-02" type="unit" req_ref="REQ-HYG-02">
    <description>Verify tier-compression round-trip cosines per shipped FSV evidence</description>
    <preconditions>1,000 random 768-d Gaussian vectors</preconditions>
    <inputs>1,000 vectors × 4 tiers (8/7/5/3-bit)</inputs>
    <expected_result>Min cosine per tier: 8-bit ≥ 0.99999 - ε, 7-bit ≥ 0.99997 - ε, 5-bit ≥ 0.99960 - ε, 3-bit ≥ 0.99192 - ε (matching shipped FSV)</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HYG-03" type="integration" req_ref="REQ-HYG-03">
    <description>Verify nightly GC pass executes 8 steps in order; aborts on integrity break at step 7</description>
    <preconditions>Synthetic 10-CF instance with 1,000 entries; deliberate witness chain integrity break injected</preconditions>
    <inputs>Invoke `gc_run_nightly()`</inputs>
    <expected_result>Steps 1-6 execute; step 7 detects break; step 8 writes report with `aborted=true, abort_reason="MEJEPA_HYG_GC_INTEGRITY_FAIL"`; step 4/5 deletes rolled back; FSV at /tmp/contextgraph-mejepa-hygiene-fsv/gc-stress-evidence.json captures the abort case</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HYG-04" type="integration" req_ref="REQ-HYG-04">
    <description>Verify witness-chain Merkle compression on 100,000-entry chain produces ~700× compression and remains verifiable</description>
    <preconditions>Synthetic 100,000-entry witness chain spanning 365 days</preconditions>
    <inputs>Invoke `witness_compress_old_segments(Duration::days(90))`</inputs>
    <expected_result>Resulting live-chain bytes ~10 KB (down from ~7 MB); ~98 compressed-segment entries (100,000 / 1024 ≈ 97.66 segments older than 90 days assuming uniform distribution); `verify_chain_integrity()` passes; sampling 5 segments and re-fetching archives + recomputing Merkle roots: 100 % match</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HYG-05" type="integration" req_ref="REQ-HYG-05">
    <description>Verify hard quota enforcement triggers emergency eviction; transitions to Unrecoverable when no eviction candidates exist</description>
    <preconditions>Synthetic category at 110 % of a 1 MB budget; mix of evictable + gold-tier entries</preconditions>
    <inputs>Invoke `quota_check_and_evict`; then refill with gold-tier-only entries; invoke again</inputs>
    <expected_result>First call frees space, category returns to Healthy. Second call cannot evict gold-tier; category transitions to Unrecoverable; alert raised; subsequent writes return `MEJEPA_HYG_QUOTA_UNRECOVERABLE`. FSV at /tmp/contextgraph-mejepa-hygiene-fsv/quota-overflow-evidence.json</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HYG-06" type="integration" req_ref="REQ-HYG-09">
    <description>Verify tagged-release pruning refusal does not abort GC</description>
    <preconditions>20 model checkpoints (5 tagged, 15 untagged); GC step 4 retention "keep last 10 + tagged releases"</preconditions>
    <inputs>Run GC step 4</inputs>
    <expected_result>10 newest + 5 tagged kept (15 total); 5 oldest untagged pruned; `MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED` events captured per attempted tagged-release prune (zero in this case since only untagged are pruned); GC continues to step 5</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HYG-07" type="e2e" req_ref="REQ-HYG-01,REQ-HYG-03,REQ-HYG-04,REQ-HYG-05">
    <description>Phase 6 DoD stress run: 10,000-entry corpus over 7 simulated days</description>
    <preconditions>Fresh ME-JEPA RocksDB instance</preconditions>
    <inputs>`cargo run --release -p context-graph-mejepa -- stress --entries 10000 --duration 7d`</inputs>
    <expected_result>`total_disk_used_gb ≤ 110`, `gc_passes_completed == 7`, `witness_chain_compressed: yes`, `quota_violations == 0` after final GC; FSV at /tmp/contextgraph-mejepa-hygiene-fsv/gc-stress-evidence.json</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HYG-08" type="integration" req_ref="REQ-HYG-07">
    <description>Verify three new MCP tools return schema-validated responses</description>
    <preconditions>MCP server running in test mode with fresh ME-JEPA instance</preconditions>
    <inputs>Three MCP calls: `mejepa_gc_run(confirm_run=true)`, `mejepa_quota_status()`, `mejepa_witness_compress(threshold_days=90)`</inputs>
    <expected_result>Each call returns `GcReport` / `QuotaStatus` / `CompressReport` matching their JSON schemas; `additionalProperties:false` rejects unknown params</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HYG-09" type="unit" req_ref="REQ-HYG-08">
    <description>Verify cross-cutting ops (`heal_detect_drift`, `heal_retrain_if_needed`) return `OpsError::CrossCuttingDeferred` when SPEC-SELF-HEALING is not yet shipped</description>
    <preconditions>Phase 5 implementation absent</preconditions>
    <inputs>Direct calls to both ops</inputs>
    <expected_result>Both return `OpsError::CrossCuttingDeferred { spec: "SPEC-SELF-HEALING" }`</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HYG-10" type="unit" req_ref="REQ-HYG-06">
    <description>Verify tier promotion + demotion symmetry; corruption surfaces fail-closed</description>
    <preconditions>Entry at Cold tier; access frequency 0.25</preconditions>
    <inputs>Read entry 5 times rapidly (forcing access frequency increment); then idle 30 days; then trigger nightly demotion</inputs>
    <expected_result>Promotion: Cold → Cool → Warm → Hot recorded in CF_MEJEPA_PANEL_META. Demotion (after 30 days idle): Hot → Cold recorded. Inject corrupt magic byte; subsequent read returns `MEJEPA_HYG_TIER_DECOMPRESS_FAIL` with offending bytes</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HYG-11" type="integration" req_ref="REQ-HYG-04,NFR-HYG-05">
    <description>Cross-host Merkle determinism: 1,000 segments compressed on host A and host B produce identical roots</description>
    <preconditions>Two hosts with identical input segments</preconditions>
    <inputs>1,000 deterministic 1,024-entry segments</inputs>
    <expected_result>1,000 Merkle roots from host A byte-equal 1,000 Merkle roots from host B</expected_result>
    <priority>must</priority>
  </test_case>

  <test_case id="TC-HYG-12" type="manual" req_ref="REQ-HYG-04">
    <description>Auditor verifies a specific old entry by re-fetching its archived segment</description>
    <preconditions>Compressed chain with 100 segments; auditor selects segment_id=42, offset=137</preconditions>
    <inputs>Auditor fetches `cold_archive_paths[42]`, recomputes Merkle root</inputs>
    <expected_result>Recomputed root byte-equals stored compressed-segment entry's action_hash; on synthetic tampering of the archive bytes, re-verification returns `MEJEPA_HYG_MERKLE_VERIFY_FAIL`</expected_result>
    <priority>high</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-HYG-01" status="open" assignee="operator">
    <text>Should `CG_MEJEPA_DISK_QUOTA_GB` overrides BELOW 115 (the sum of category budgets) scale categories proportionally, or hard-fail at startup?</text>
    <context>EC-HYG-08 currently specifies proportional scaling with a WARN log. Operator may prefer fail-fast to surface mis-configuration. Decision affects EC-HYG-08 implementation.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HYG-02" status="open" assignee="Phase 5 team">
    <text>What's the exact `OpsError::CrossCuttingDeferred` payload shape that SPEC-SELF-HEALING expects to consume on its arrival?</text>
    <context>Phase 6 ships the MCP shim now; Phase 5 may want richer context (e.g., last-known-good drift state) at deferral time. Decision affects REQ-HYG-08 implementation.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HYG-03" status="open" assignee="operator">
    <text>What is the cold-archive root path's default? `&lt;runtime_root&gt;/mejepa/witness-archive/` or a separate operator-configured drive?</text>
    <context>Default drives the operator runbook + EC-HYG-04 archive-write-failure scenarios. Default of `&lt;runtime_root&gt;/mejepa/witness-archive/` keeps the archive on the same disk as the live chain (fails together on disk-full); a separate drive provides independence at operator configuration cost.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HYG-04" status="open" assignee="Phase 5 team">
    <text>Should GC step 7 (verify_chain_integrity) be incremental (only verify segments touched by step 6) or full-chain on every nightly pass?</text>
    <context>Full-chain verification is O(N) in chain length; for 1M+ entry chains this could exceed the 60-min NFR-HYG-01 budget. Incremental is cheaper but may miss latent corruption in untouched segments. Doc 09 §6.3 says "verify integrity at the end" without specifying incremental vs full.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HYG-05" status="open" assignee="operator">
    <text>Frozen 1-bit tier — does it use a separate magic (`CGTF`) or extend `context-graph-tier-compression` to add a `BitWidth::One` variant?</text>
    <context>REQ-HYG-02 currently specifies separate `CGTF` magic + sign-bit-only encoding. Extending the existing crate with `BitWidth::One` would unify the encoder/decoder dispatch but require a constitution-amendment-equivalent revision to the shipped tier-compression crate.</context>
    <resolution></resolution>
  </question>

</open_questions>

<glossary>
  <term name="Tier">One of {Hot, Warm, Cool, Cold, Frozen} corresponding to bit widths {raw f32, 8-bit, 5-bit (Cool), 3-bit (Cold), 1-bit (Frozen)}. Per doc 09 §6.2 the table maps frequency thresholds → tier; "Warm 7" sits between Hot 8 and Cool 5</term>
  <term name="Access frequency">Per-entry score in [0, 1] decayed at 0.99 per 24h; incremented on read; drives tier promotion / demotion</term>
  <term name="Gold-tier">Operator-tagged immune-from-eviction entries (tagged_release on checkpoints; explicit operator overrides on aux signals). Retained indefinitely</term>
  <term name="Compressed-segment entry">A 73-byte witness entry replacing 1024 consecutive original entries; contains only the Merkle root + segment metadata; chains to prev_hash like any other witness entry</term>
  <term name="Cold archive">Filesystem path (or future cloud storage) where original 1024-entry segments are persisted before being replaced in the live chain by compressed-segment entries; required for after-the-fact verification of any old entry</term>
  <term name="Emergency eviction">Quota-overflow-triggered eviction outside the nightly GC window; runs `quota_check_and_evict` on the over-budget category</term>
  <term name="Unrecoverable">Sticky degradation state for a category that one full GC eviction pass could not bring under budget. New writes to the category are rejected; predictions continue serving from existing entries</term>
  <term name="Hard quota">The 115 GB total-disk ceiling (default; CG_MEJEPA_DISK_QUOTA_GB overrides); summed across the 9 storage categories. The system never exceeds this without operator override</term>
  <term name="EntryId">A `(cf_name, key)` pair uniquely identifying a tier-compressible entry in any of the ME-JEPA RocksDB CFs</term>
  <term name="GcStepReport">Per-step structured report inside a GcReport; one per executed step with `{count, duration_ms, bytes_freed, errors_encountered}`</term>
  <term name="Cross-cutting boundary">The interface surface where Phase 6 hygiene exposes `heal_detect_drift` / `heal_retrain_if_needed` MCP shims whose bodies live in SPEC-SELF-HEALING (Phase 5). This spec specifies only the signatures + fail-closed contracts</term>
</glossary>

</functional_spec>
```
