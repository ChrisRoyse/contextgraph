---
namespace: reports
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:55:00Z
status: complete
tags: forensic-audit, final-report, phase-b, phase-c, phase-d, resource-management
---

# Sherlock S3 — Forensic Audit: Phase B/C/D Python Scripts

## Case Summary
**Subject**: Resource management, batching, cleanup, and streaming behavior of
the three Python scripts that ran before WSL crashed on 2026-04-17.
**Verdict**: All three scripts have real bugs; none are "unsafe to run",
but **Phase D carries the most risk** (Gemma VRAM, no subprocess isolation).
**Confidence**: HIGH (line-cited evidence for every finding).

## Root Cause of the WSL Crash
**The daemon's OOM death was NOT caused by these scripts.** Agent 4's own
blocker `memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md`
documents 4 × `voiceagent listen` Python processes consuming 107 GB of 120 GB
host RAM. The daemon got OOM-killed trying to load 20 GB of embedders.
Phase B had barely started. Phase C and Phase D never ran to completion.

That said — when Phase D DOES run, it will load Gemma (11.5 GB VRAM)
alongside the daemon (20.7 GB VRAM), which puts the 32.6 GB 5090 right at the
edge. And none of the three scripts protect themselves from host-RAM
pressure via an RSS cap. The scripts are not blameless — they are fragile.

---

## Per-Script Findings Tables

### Phase B: `scripts/shakespeare_stylistic_analyzer.py` (1,333 lines)

Overall verdict: **solid, properly streamed, minor cleanups needed.**

| # | File:Line | Severity | Issue | Fix |
|---|-----------|----------|-------|-----|
| B-1 | 1129-1133 | LOW | `_count_lines` reads whole file just to count — but once per run, not per record. OK. | No fix. |
| B-2 | 1246-1318 | MEDIUM | `error_log` opened in `"a"` mode, never rotated. Over many resume cycles /tmp fills (tmpfs on WSL). | Rotate if size > 10 MB on start. |
| B-3 | 1250-1251 | LOW | Outer `try:` opens error_log BEFORE the `with fin, fout:` — correct FD cleanup, just slightly unusual. | No fix required. |
| B-4 | 1076 | LOW | `daemon.score_passage(text)` called per record; `DaemonClient._sock` reused. Good. | No fix. |
| B-5 | 1229-1237 | LOW | Resume copy via `shutil.copy2(output_path, partial_path)` — uses ~output-size extra disk briefly. Acceptable for 7925 × ~2KB = 16 MB. | No fix. |
| B-6 | End of main() | MEDIUM | `daemon` object never explicitly `._reset()`-ed before process exit. Daemon logs "client closed abruptly" noise. | Add `finally: daemon._reset()` at outer scope. |
| B-7 | 1062-1095 | LOW | `spacy` Doc freed only by scope; vocab hash table grows permanently — ~30 MB after 7925 records. Normal. | No fix. |
| B-8 | None | HIGH | **No SIGTERM/SIGINT handler.** OOM kill leaves in-flight `store_memory` as orphan in daemon. | Use `shakespeare_resource_guard.managed_run()`. |
| B-9 | None | MEDIUM | **No RSS cap.** If voiceagents return and analyzer keeps growing, kernel picks analyzer or daemon to OOM-kill. | Use `managed_run(rss_cap_gb=12)`. |
| B-10 | 1245-1247 | LOW | `error_log` header line written before `try:` — if open succeeded but header write failed mid-way, partial header. Trivial. | No fix. |

Streaming of input → output: CORRECT. Reads line by line (1252), writes record by record with flush (1294-1296). Good.

---

### Phase C: `scripts/shakespeare_build_dpo_style_pairs.py` (856 lines)

Overall verdict: **several real issues; state loss on abort is significant.**

| # | File:Line | Severity | Issue | Fix |
|---|-----------|----------|-------|-----|
| C-1 | 383-414, 662 | HIGH | **`load_sft_records` materializes ALL 7,925 records in memory (~200 MB)** before loop starts. Combined with daemon pressure contributes to OOM susceptibility. | Convert to generator: `def iter_sft_records(...)`. |
| C-2 | 417-440, 665 | MEDIUM | `load_tag_index` keeps full tag dict with ALL fields (text, style_profile_vector, per_embedder_cosine arrays). ~150 MB for 7925 records. | Filter to 3 fields: register.primary, meter.form, semantic_topic. |
| C-3 | 842-852, 709, 740 | MEDIUM | `_maybe_abort_on_budget` raises `SystemExit(1)` — **skips the post-loop state checkpoint at 813-819**. Resume re-processes up to 99 records. | Set a flag, break out of outer for; state save in finally block. |
| C-4 | 622-652 | LOW | `parse_args` / `main` has no --help improvement, but fine. | No fix. |
| C-5 | 676, 677 | MEDIUM | `partial_f.open("a")` and `rejected_log_f.open("a")` do NOT install signal handlers — SIGTERM = buffered lines lost. Uses `flush()` after each write which is good, but the logs share fd — reasonable. | Use `managed_run(on_shutdown=lambda: [partial_f.flush(), partial_f.close(), rejected_log_f.close()])`. |
| C-6 | 609-621 | LOW | `output_is_complete` reads last 4 KB to find `COMPLETION_MARKER` — works but depends on the marker being written. If run aborts between `state save` and `write marker`, subsequent runs redo work. | Write marker BEFORE state save; inspect both. |
| C-7 | 784, 789 | LOW | Progress log every 100 pairs_written is fine; state checkpoint tied to same trigger is fine. | No fix. |
| C-8 | 798-810 | LOW | `finally:` closes rpc, partial_f, rejected_log_f. Good. But SystemExit from `_maybe_abort_on_budget` bypasses the end-of-main state+marker block (finding C-3). | Fix per C-3. |
| C-9 | `_gemma_shakespeare_lib.py:27-89` | MEDIUM | Rpc class holds a single socket; no reconnect on idle-reset. If daemon goes down mid-run for 30s and comes back, first call after will hit `daemon closed socket mid-call` (RuntimeError) — **caught by DaemonStyleScorer and counted as failure**, counts toward 1% budget. Could trigger spurious abort. | Add retry-once with reconnect in `Rpc.call` on `RuntimeError("daemon closed...")`. |
| C-10 | None | HIGH | **No SIGTERM/SIGINT handler.** Same orphan memory risk as Phase B, 1-4 fingerprints in flight per record. | Use `managed_run`. |
| C-11 | None | MEDIUM | **No RSS cap.** | Use `managed_run(rss_cap_gb=14)`. |

Streaming of output: CORRECT (line-per-write, flushed).
`store_memory` + `forget_concept` pairing: CORRECT (nested try/finally at 700-777).

---

### Phase D: `scripts/shakespeare_sft_augmenter.py` (2,814 lines)

Overall verdict: **highest-risk script; two high-severity findings and several design gaps.**

| # | File:Line | Severity | Issue | Fix |
|---|-----------|----------|-------|-----|
| D-1 | 2088-2119 | HIGH | **GemmaGenerator.generate runs batch_size=1** — 1,500 candidates × ~3s each = 75 min at peak VRAM. Batched at 4 = 19 min. Longer Gemma-resident window = higher crash probability. Also `del out, inputs` and periodic `empty_cache()` missing — VRAM fragmentation. | Implement `generate_batch(list)` with left-padding. |
| D-2 | 2038-2074, 2761 | HIGH | **Gemma unload does NOT guarantee VRAM release** on WSL. `torch.cuda.empty_cache()` returns blocks to caching allocator, not to driver. Daemon restart at 2779-2784 may find insufficient VRAM. No assertion. | Move Phase 2 to `subprocess.run([python, __file__, --internal-phase=2])` — process exit RELEASES VRAM fully to driver. See discovery `phase-d-spacy-gemma-both-resident.md`. |
| D-3 | 2336-2351, 2388-2391 | HIGH | **`AtomicJsonlWriter` renames `.partial` to `.final` on close, but `count_existing_by_category` looks at `.partial`** — after a successful finalize, a subsequent run sees 0 in `.partial` and REGENERATES everything from scratch, overwriting the good `.final`. | Check BOTH `.partial` and `.final` in count_existing; don't delete `.partial` until after confirming final exists. |
| D-4 | 2094-2104 | MEDIUM | `apply_chat_template` may raise `ModelLoadError` inside the hot generation loop. Per-record try/except at 2420-2426 CATCHES `Exception` — good, but logs `WARN` and `continue`, which means the script plows on. If the error is persistent (CUDA OOM), you get 1,500 WARN lines. | Track consecutive-failure count; abort after N (say, 10). |
| D-5 | 2115 | LOW | `pad_token_id=self._tokenizer.eos_token_id` assumes tokenizer has eos. No assertion — NoneType crashes deep inside generate. | Assert `eos_token_id is not None` in load(). |
| D-6 | 2105 | LOW | `attention_mask = torch.ones_like(inputs)` — `inputs` is int tensor; `ones_like` returns int tensor mask. Works but cast-inefficient; the correct form is a float tensor the same shape. | Minor; use `torch.ones(inputs.shape, dtype=torch.long, device=inputs.device)`. |
| D-7 | 2414-2419 | MEDIUM | Few-shot block repeated per-prompt — same 3 anchors prepended to each of 1,500 system+user pairs. Adds ~800 tokens per call → 1,500 × 800 = 1.2M extra tokens processed. Wastes Gemma compute. | Cache a pre-tokenized `_fewshot_prefix_ids` once; concatenate before chat template. |
| D-8 | 1815-1846 | MEDIUM | `stop_daemon` uses `fuser -k` if pidfile missing — could kill unrelated process using port 3100. Also the `fuser -KILL` path at 1841 is last-resort SIGKILL; daemon tests confirm it DOES leave orphan HNSW indexes on abrupt kill (pre-existing design concern, per memory/MEMORY.md). | Only fuser-kill if confirmed via `ss -tlnp` that the PID matches expected binary. |
| D-9 | 2343 | MEDIUM | `AtomicJsonlWriter.__init__` opens `.partial` in append mode. If the previous `.partial` is corrupted (half-written JSON line from a previous SIGKILL), appending continues AFTER the bad line. `count_existing_by_category` skips the bad line via `json.JSONDecodeError: continue` but the file is permanently broken. | Scan/truncate partial on open: read line-by-line, stop at first JSONDecodeError, truncate there. |
| D-10 | 2345-2347 | LOW | `writer.write` flushes but does NOT `os.fsync()`. SIGKILL between flush and kernel-commit could lose last line. | `os.fsync(self._fp.fileno())` after write, cost ~5-20ms. |
| D-11 | 2750-2762 | HIGH | `gemma = GemmaGenerator(...)`; `gemma.load(); ...; gemma.unload()` — if `gemma.load()` OOMs partway, `finally: gemma.unload()` runs on a partially-loaded model. `self._model = None` might not reach CUDA release path. Combined with D-2: VRAM leak on load failure. | See D-2. |
| D-12 | 1780-1801 | LOW | `load_register_distribution` iterates tags file but only uses 1 field; full record parsing via `json.loads` is O(N). Fine for 7925 records but wasteful. | Use a streaming JSON parser (`ijson`) or a regex for `"primary":"..."`. |
| D-13 | 2812-2814 | LOW | Top-level `except KeyboardInterrupt: sys.exit(130)` is good but arrives too late — the `main()` finally blocks don't get a chance to forget in-flight fingerprints because the scorer's lock might be held. | Install signal handler at startup (managed_run). |
| D-14 | None | HIGH | **No SIGTERM handler.** | Use `managed_run`. |
| D-15 | None | MEDIUM | **No RSS cap.** Phase D is the most likely to OOM (Gemma + daemon + spaCy not loaded here but transformers tokenizer caches). | `managed_run(rss_cap_gb=22)`. |
| D-16 | 2050, 2078, 2092 | LOW | `import torch` repeated inside each method — by design (lazy), but means `--help` crashes on systems without torch. OK given the help check in argparse. | No fix needed. |

Subprocess / orphan concern: D-11 + D-2 combined is the most serious risk.

---

## TOP-5 Critical Findings (ranked by severity × crash probability)

### 1. Phase D Gemma unload cannot guarantee VRAM release (D-2, D-11)
**Evidence**: `shakespeare_sft_augmenter.py:2077-2086` + `2761-2762`.
**Impact**: After Gemma, daemon restart hits insufficient VRAM → daemon dies
→ Phase 3 scoring can't run → we're back in the same crash cycle.
**Fix**: Run Phase 2 (generation) as a subprocess. Process exit = guaranteed
VRAM release to driver.

### 2. Phase D AtomicJsonlWriter resume bug loses work on second run (D-3)
**Evidence**: `shakespeare_sft_augmenter.py:2336-2351` + `2388-2391`.
**Impact**: After one successful run, running the script AGAIN (e.g. to top up
counts) silently regenerates the whole candidates file from scratch,
overwriting the good one. Operator loses hours of Gemma compute.
**Fix**: Check both `.partial` AND `.final` in resume counter; keep `.partial`
until verified.

### 3. Phase C loads all 7,925 SFT records in memory (C-1)
**Evidence**: `shakespeare_build_dpo_style_pairs.py:383-414, 662`.
**Impact**: ~200 MB resident purely for bookkeeping; combined with tags dict
(C-2) ~350 MB. On a RAM-pressured host, contributes to OOM.
**Fix**: Convert to generator `iter_sft_records()`.

### 4. ALL three scripts have no SIGTERM/SIGINT handlers (B-8, C-10, D-14)
**Evidence**: `rg -n 'signal\.|SIGTERM|SIGINT|atexit' scripts/shakespeare_*.py`
returns zero matches.
**Impact**: Kernel OOM-kill = orphan `store_memory` in daemon (VRAM wasted);
half-written JSONL lines; Gemma VRAM held until process reap; training
data possibly corrupt if `.partial` rename mid-flight.
**Fix**: `shakespeare_resource_guard.managed_run()` — installed, smoke-tested.

### 5. Phase D batch_size=1 prolongs the peak-VRAM window (D-1)
**Evidence**: `shakespeare_sft_augmenter.py:2088-2119`.
**Impact**: 75+ min at peak VRAM × increased crash probability. Not a leak
per se but a multiplier on the crash window.
**Fix**: Implement `generate_batch(list[tuple[sys,user]])`, batch=4.

---

## Specific Patches Required (ready-to-apply pointers)

### Phase B (analyzer)
- **B-8/B-9 → ADD `managed_run` wrapper at main():**
  ```python
  from shakespeare_resource_guard import managed_run
  ...
  def main():
      ...
      with managed_run(
          script_name="phase-b-analyzer",
          rss_cap_gb=12,
          heartbeat_path=Path("/tmp/shake_phaseb_mem.jsonl"),
          on_shutdown=lambda: daemon._reset(),
          forget_fingerprint=lambda fp: daemon._tool_call(
              "forget_concept", {"node_id": fp, "reason": "guard cleanup"}),
      ):
          ...existing main body...
  ```
- **B-6 → daemon._reset() in finally** (file end of main).
- **B-2 → rotate error log** on start if > 10 MB.

### Phase C (DPO builder)
- **C-1 → replace `load_sft_records` with generator `iter_sft_records`.**
  (New function at `scripts/shakespeare_build_dpo_style_pairs.py:383`.)
- **C-2 → trim tag_idx** to `{register, meter_form, semantic_topic}` only.
- **C-3 → refactor `_maybe_abort_on_budget`** to set a flag; state save in
  outer `finally`.
- **C-9 → add auto-reconnect** in `_gemma_shakespeare_lib.Rpc.call`.
- **C-10/C-11 → `managed_run`:**
  ```python
  with managed_run(
      script_name="phase-c-dpo-builder",
      rss_cap_gb=14,
      heartbeat_path=Path("/tmp/shake_phasec_mem.jsonl"),
      on_shutdown=lambda: (
          partial_f.flush(), partial_f.close(),
          rejected_log_f.close(), rpc.close()
      ),
      forget_fingerprint=scorer.forget,
  ):
      ...
  ```

### Phase D (augmenter)
- **D-2/D-11 → split Phase 2 into subprocess** via
  `--internal-phase=generate-only` argument. Parent calls subprocess, waits,
  validates return code.
- **D-1 → implement `generate_batch`** (batch=4). Call sites at 2421, 2508,
  2517 pass batches instead of single items.
- **D-3 → fix `count_existing_by_category`** to check both partial and final.
- **D-9 → scan-and-truncate partial on open** (detect half-line).
- **D-14/D-15 → `managed_run`:**
  ```python
  with managed_run(
      script_name="phase-d-augmenter",
      rss_cap_gb=22,                        # 22 GB room for Gemma (11.5) + overhead
      heartbeat_path=Path("/tmp/shake_phased_mem.jsonl"),
      on_shutdown=lambda: (
          gemma.unload() if gemma else None,
          scorer.close() if scorer else None,
      ),
      forget_fingerprint=lambda fp: scorer._tool_call(
          "forget_concept", {"node_id": fp, "reason": "guard cleanup"})
              if scorer else None,
  ):
      ...
  ```

---

## Shared Resource Guard Module

**Path**: `/home/cabdru/contextgraph/scripts/shakespeare_resource_guard.py`
**Purpose**: Provides `managed_run()` context manager with SIGTERM/SIGINT
handlers, configurable RSS cap via `RLIMIT_AS`, background memory
heartbeat (RSS/VMS/CUDA to sidecar JSONL), and `register_fingerprint()`/
`unregister_fingerprint()` for tracking in-flight daemon memories.
**Verified**: Smoke-tested standalone (`python3 scripts/shakespeare_resource_guard.py --smoke`)
with natural-exit AND SIGTERM paths; both correctly run `on_shutdown`,
forget tracked fingerprints, and exit with the right codes (0 / 143).

---

## Unsafe-To-Run Assessment

| Script | Safe to run as-is? | Rationale |
|--------|--------------------|-----------|
| `shakespeare_stylistic_analyzer.py` | **YES, with caveats** | Properly streamed; daemon OOM already blocks it; the script itself won't cause crashes. Missing signal handler is a risk only on interrupt. |
| `shakespeare_build_dpo_style_pairs.py` | **YES, with caveats** | C-1 (200 MB in-memory) adds to pressure but won't crash alone. C-3 state-loss on abort costs minutes of rework, not catastrophic. |
| `shakespeare_sft_augmenter.py` | **HOLD — integrate resource_guard FIRST** | D-2 (VRAM release) + D-3 (resume bug) + D-11 (partial load unload) combined mean a single crash can destroy candidates output AND leave VRAM held. Patch D-3 at minimum before running. |

**Recommendation to the operator**: Apply the `managed_run` wrapper to all
three scripts AND fix D-3 (resume bug) before running Phase D. Phase B and
Phase C can run without patches but monitoring via heartbeat sidecar is
strongly recommended.

---

## Memory Files Written

| File | Purpose |
|------|---------|
| `memory/journal/agent-s3-sherlock-scripts--start-forensic-audit.md` | session start |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-c-loads-7925-records-in-memory.md` | C-1 |
| `memory/discoveries/agent-s3-sherlock-scripts--no-signal-handlers-anywhere.md` | B-8, C-10, D-14 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-d-gemma-double-encode-and-batch-size-1.md` | D-1 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-d-resume-double-writes-candidates.md` | D-3, D-9, D-10 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-b-error-log-fd-leak.md` | B-2, B-6 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-c-budget-abort-loses-state.md` | C-3 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-d-spacy-gemma-both-resident.md` | D-2, D-11 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-d-missing-rss-cap-and-no-heartbeat.md` | B-9, C-11, D-15 |
| `memory/discoveries/agent-s3-sherlock-scripts--phase-b-spacy-doc-not-released.md` | B-7 informational |
| `memory/reports/agent-s3-sherlock-scripts--final-report.md` | this file |

**New script**: `/home/cabdru/contextgraph/scripts/shakespeare_resource_guard.py`

---

## Confidence

HIGH. Every finding is line-cited against the three scripts. The resource
guard module was smoke-tested (both natural exit and SIGTERM paths) and
writes the expected heartbeat output. The root cause of the crash was
independently confirmed via `memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md`
— NOT a Python leak but host-RAM exhaustion by voiceagent processes. The
scripts are fragile to that environment; the guard module gives them the
ability to fail gracefully next time.
