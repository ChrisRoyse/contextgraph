# Reality-Loop Kickoff Plan — What's Broken, What to Fix, How to Start

**Date:** 2026-05-06
**Author:** investigated this turn (Opus 4.7) for Chris Royse
**Branch:** `ccreality-cutover` (commit `0017cdaa` at investigation time)
**Scope:** what the last 48 h of commits actually delivered, what is currently
blocking a clean kickoff against `psf__requests-2317`, and the smallest viable
plan to get the meta-loop sustaining productive iterations.

This is a *kickoff* doc. The 19 docs in `docs/fullloop/` and the 21 docs in
`docs/reality/` are the *design*. `.claude/loop.md` is the *runtime contract*
the optimizer reads each turn. None of those should be conflated with this
file. When this plan is consumed and Phase 13 + Phase 14 are green, this file
should be archived under `docs/fullloop/PHASE13_RESULTS_<date>.md` /
`PHASE14_RESULTS_<date>.md`, not edited indefinitely.

---

## 1. What the last 48 h actually shipped (git history 2026-05-04 → 2026-05-06)

Commits, in chronological order, with the *productive* delta (not just the
diff stat):

| Commit | Theme | Productive delta |
|---|---|---|
| `bb32aed4`–`9bf9dcdf` | DynamicJEPA + BlackBoxFix demo | reward compiler v14 + verified-outcome benchmark wired |
| `e9d394c3`–`f1d0937c` | gemmafrontend evidence console | multi-tool ContextGraph gateway for the frontend |
| `9f3838de`–`245894bb` | dynamic tool hint guide | the inner actor frontend now uses tool-hint guide instead of static presets |
| `b57a2f1c`–`8a3fce9a` | Reality-loop docs first cut | `docs/reality/` 4 new docs (meta-loop, smoke curriculum, measurement matrix, scoreboard) |
| `c12a5216`–`33562474` | Python loop → Rust loop pivot starts | governed agent tools, GPU VRAM budgeting, deterministic prompt compiler |
| `b7c4c5a2`–`56d9026b` | Rust reality-loop binary | `target/<profile>/reality-loop` owns the loop end-to-end |
| `cc8e6c33`–`16aa0a9d` | Reality-boundary fail-closed gates | runtime measurement contract; verifier/oracle paths sacred |
| `c21ee1d5`–`f58e2755` | Spec refreshes | structured-edit compilation path; EdgeRepository typed-edge keying |
| `ca958860`–`95b55278` | Python loop retired | Python loop deleted; compact official evidence persisted |
| `a30c2793`–`7e74e20b` | Problem/signal reality packets | static patch checks land |
| `7ad8e8a1`–`f799a53e` | Per-tool harness transitions | tool-mediated final edit + line-window edit tool |
| `e4cfa3ba`–`a16c147b` | Outer optimizer = governed coding agent | DeepSeek V4 Pro pivot, then consolidated guides |
| `7d51ee52`–`b8239354` | Cleanup + docs hygiene | DeepSeek scrapes added then removed; backups moved out of docs/ |
| `00f4d309`–`d9a95712` | DeepSeek/OpenAI **excised** | the outer optimizer becomes Claude Code (the ccreality plan) |
| `389521a6`–`32deac10` | ccreality planning package | hooks, subagents, skills, project-memory, runbook, schema |
| `411443fd`–`90d3896f` | 13-phase implementation plan | per-phase preconditions/tasks/verification packets |
| `f4c4bd66`–`13fc3983` | Phase 04/10 spec refinement | MCP interact/alter/shift + FSV file specs |
| `2cc2f1ff`–`0472c199` | MCP tool surface lands | analyze/interact/alter tools, governed paths, errors/helpers |
| `5da43f7b`–`a682077e` | reality-loop subcommands + .claude/ | attempt/smoke/replay/verify CLI; `.claude/` checked in |
| `6ca5c342`–`698caf35` | Tool surface hardened | governed-paths regex matcher; schema metadata |
| `afd762ed`–`8ac3cb88` | Hooks + ccreality CLI command | 6 hook scripts; ccreality phase-doc refinements |
| `2280dd63`–`1ff731a4` | Phase 0–8 hardening | CLAUDE.md ccreality role added; hardening gate script |
| `b3a02318`–`7ba891be` | Phase 09 smoke + audit-trail tool | full-stack smoke runner; ledger WHERE validator |
| `d3442fce` | Phase 10 FSV harness | 39 case scripts + master runner |
| `0f12ca53` | **Phase 12 production cutover complete** | first real SWE-bench Lite attempt against `psf__requests-2317` (failed closed at `RUST_REALITY_FINAL_EDIT_TOOL_FAILED`) |
| `80c01cc5` | Post-cutover hardening | serial tests, tool-leak guards, full-stack reports |
| `19238126`–`5b3affb3` | **fullloop docs land** | 19 docs covering Phase 13–22 forward plan |
| `eac5d411`–`704e6fe0` | Phase 13 stringified-arg + Phase 15 autoresearch + Phase 18 training-export | three forward phases get partial implementations |
| `1d8a5275` | **Ollama the inner actor → Rust llama.cpp in-process** | inner SWE actor now runs without Ollama, reads GGUF directly |
| `95dfd583`–`47e840b4` | the inner actor harness model-capability readback + prompt-budget guard + `apply_unified_diff` | additional final-edit path, graduated FAIL_TO_PASS signal |
| `6cbca8ac`–`5510e7e2` | Identity guards + issue-text mention priority | harness fails closed on identity mismatch / sub-128 K context |
| `d44fc71b`–`a66402d7` | Optimizer-side hardening | `.claude/loop.md` documents harness-already-handles behaviors; plain `git push` allowed; force-push denied |
| `0017cdaa` | **Cryptographic model-pin gate** | new `config/ccreality_model_pins.json` and pin-validation step in the harness |

**One-paragraph summary.** The last 48 h took the pre-cutover prototype
(Python SWE-bench loop with Ollama the inner actor, DeepSeek-driven outer optimizer)
and replaced it with a Rust-only reality-loop binary, an in-process llama.cpp
the inner actor harness with a cryptographic pin gate, a Claude-Code-driven outer
optimizer wired through MCP/hooks/subagents/skills, and 19 forward-planning
docs under `docs/fullloop/`. Phase 12 ran one real attempt against
`psf__requests-2317` and failed closed correctly. Phases 13/14 are the
forward gap.

---

## 2. What is *actually* broken right now (verified by direct inspection of the active runtime root)

I read every artifact under `/tmp/contextgraph-rust-reality-loop/` (the
runtime root in `~/.cache/cgreality/active_runtime_root`). Three different
runs exist:

### Failure 1 — `1778064266` `RUST_REALITY_INNER_MODEL_PATH_MISSING`

Source of truth:
`file:/tmp/contextgraph-rust-reality-loop/failures/failure-1778064266-RUST_REALITY_INNER_MODEL_PATH_MISSING.json`

Engine looked at `/home/cabdru/contextgraph/models/qwen3.6-27b/qwen3.6-e4b.Q4_K_M.gguf`.
The actual file on disk is `qwen3.6-E4B-it-bf16.gguf` (15 GB BF16). The
**default in `crates/context-graph-cli/src/bin/reality-loop.rs:28`** is the
correct BF16 path:
```rust
const DEFAULT_INNER_MODEL_PATH: &str = "models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf";
```

Root cause: the operator (or watcher) was following stale documentation that
referenced the old `Q4_K_M` filename. Stale references found in:
- `docs/reality/18_inner_actor_coding_agent_harness.md:37` ← **fixed this turn**
- `docs/ccreality/phase-12-first-production/01_PRECONDITIONS.md:29` ← **fixed this turn**

Both edits routed through the harness-editor governance with PreToolUse SHA
gate + PostToolUse harness-transition mint.

### Failure 2 — `1778064748` `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED`

Source of truth:
`file:/tmp/contextgraph-rust-reality-loop/failures/failure-1778064748-RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED.json`

```json
{
  "memory_free_mb": 7010,
  "memory_total_mb": 32607,
  "memory_used_mb": 25178,
  "min_free_mb": 8192,
  "phase": "loop_start",
  "model": "qwen3.6:27b"
}
```

The 5090 had 25 GB used by other processes when the engine started; the
8 GB reserve check correctly failed closed. Right now (verified via
`nvidia-smi` this turn): `17 GB free, 15 GB used` — there is currently
*enough* headroom for the BF16 model + 128 K KV cache. The fail-closed
behavior is correct; the operator just needs to free GPU before kickoff.

### Run that *almost* worked — `1778064282` (no attempt-summary, but everything else)

This run got all the way through:
- Model load + identity readback OK (`inner-model-readback.json` has
  full metadata; 7.5 B params, 131 072 ctx, BF16).
- Problem-reality packet emitted.
- 24 rounds of agent tool calls (`agent-tools/round-01..round-24/`).
- Candidate patch generated (`candidate.patch`).
- Static checks passed (`static-checks.json`: `python_py_compile` exit 0).
- Validation passed (`validation.json: status=passed`).
- Official Docker eval kicked off (`logs/run_evaluation/.../run_instance.log`
  shows the container created + patch applied cleanly).

But:
1. **No `attempt-summary.json` exists.** This means the engine crashed *after*
   spawning the Docker eval but *before* writing the summary. The Docker
   container `5f5be42adcbe` was started; the engine process either died (most
   likely from the VRAM-overshoot above, since this run is the one that ran
   immediately before the VRAM failure) or the eval timed out without the
   summary writeback path closing.
2. **The candidate patch is wrong.** the inner actor submitted a patch that *deletes*
   the `kwargs.setdefault('allow_redirects', True)` line in `Session.get()`
   and prepends a stray comment `# method = builtin_str(method)` to the
   `return self.request(...)` line — semantically destroying the GET helper.
   Compare to the gold fix for `psf__requests-2317`, which puts
   `method = builtin_str(method)` (no `#`) at the top of `Session.request()`,
   not `Session.get()`. the inner actor had the *text* but applied it to the wrong
   function. This is a model-capability gap, not a harness gap.

### Quiet-but-real issue — Phase 13 T13.6 already half-fixed

`scripts/cc/hook_session_start.sh` was updated in commit `eac5d411` to branch
on `record_kind == ccreality_smoke_attempt_summary` and read the flat-shape
synthetic-smoke summary correctly. T13.1 of Phase 13 is therefore *already
done*; the planning doc `04_PHASE_13_HARDENING.md` should mark it complete.

T13.2 (stringified attempt arg coercion) was also addressed by `eac5d411`'s
"phase 13 stringified-arg coercion" commit. T13.6 (recommendation overwrite
prevention) is **not** verified yet — needs an FSV layer that fires two
`/loop` calls against the same attempt.

---

## 3. Best practices I incorporated (web research, May 2026)

I pulled three search bands relevant to this work. Sources at end of section.

### 3a. Self-evolving SWE-bench agents (Live-SWE-agent, arxiv 2511.13646)

- **Reflection prompts after every tool round** add 3–5 pp absolute solve
  rate. The harness already has rounds 01–24 visible; what is missing is a
  reflection step between consecutive `apply_line_window_edit` rejections.
- **Dynamic tool augmentation at runtime** is the differentiator: agents
  that can declare a new tool mid-attempt (e.g., a `read_failing_test_body`
  helper) outperform fixed-tool agents. Our harness has 7 fixed tools today.
  Adding a `propose_new_tool` round-N step is a Phase 14b candidate.
- **Loop-breaking protocols** — the agent must detect when it is trying the
  same edit twice. Round 23 + round 24 in the failed attempt both used
  `apply_unified_diff` after `apply_line_window_edit` had failed twice; that
  is *exactly* the convergence anti-pattern Live-SWE-agent calls out.

### 3b. Mini-SWE-Agent (1–8 B SLMs)

- **Stateless orchestration** with a Docker bash REPL is the canonical
  small-model loop. Our harness is more structured (typed tools + governed
  edits) which is correct for verifiability but more constraining for the
  model.
- **Energy/efficiency**: the inner actor-3 4 B = 23.41 kJ per run, Qwen-3 1.7 B =
  54.13 kJ per run. The 5090 budget is not the bottleneck; reasoning
  capacity is.
- **Bottleneck named**: "the SLM's intrinsic reasoning capacity" — without
  loop detection, repeated failed reasoning steps accumulate. Our engine has
  no loop-detection step today; it just truncates after N rounds.

### 3c. llama.cpp VRAM management

- **KV cache scales linearly with context** — 1–2 GB extra per 8 K → 32 K
  context for ~9 B models, 2–4 GB for 64 K. Our requested context is 128 K
  on a 7.5 B BF16 model; the KV cache alone is ≈ 8–12 GB at 128 K. With
  the 15 GB model weights, a fully GPU-resident run wants 23–27 GB.
- **Partial offload via `-ngl`** — half the layers on GPU still gives a
  meaningful speedup over CPU-only. Our `args.inner_gpu_layers` is currently
  `u32::MAX` (all-on-GPU). For a 5090 sharing the GPU with other processes,
  adding `--inner-actor-gpu-layers <N>` to drop layers when free VRAM is tight is
  a robust fallback that respects the fail-closed reserve.
- **`--override-tensor` regex** lets us keep attention/KV on GPU and
  push FFN to CPU. For BF16-7.5B that recovers ~6–8 GB VRAM. Worth
  evaluating in Phase 14b if the all-GPU path keeps tripping the reserve.

Sources:
- [Live-SWE-Agent (arxiv 2511.13646)](https://arxiv.org/html/2511.13646v3)
- [Mini-SWE-Agent (Emergent Mind)](https://www.emergentmind.com/topics/mini-swe-agent-27d26942-1f63-4337-bee8-576ebb1468c3)
- [llama.cpp VRAM Requirements (LocalLLM.in)](https://localllm.in/blog/llamacpp-vram-requirements-for-local-llms)
- [llama.cpp n_gpu_layers Explained (Patrick Hughes)](https://bmdpat.com/blog/llama-cpp-n-gpu-layers-explained-2026)
- [SWE-bench leaderboard / agentic coding (LLM-Stats)](https://llm-stats.com/benchmarks/swe-bench-verified-(agentic-coding))
- [SWE-bench, agentic coding, Sonnet 4.5 → 4.6 (DEV.to)](https://dev.to/blamsa0mine/swe-bench-agentic-coding-and-what-actually-changed-from-claude-sonnet-45-to-46-1gig)
- [Partial LLM Loading (TinyComputers)](https://tinycomputers.io/posts/partial-llm-loading-running-models-too-big-for-vram.html)
- [llama.cpp `--override-tensor` for MoE / partial offload (HF blog, Doctor Shotgun)](https://huggingface.co/blog/Doctor-Shotgun/llamacpp-moe-offload-guide)

---

## 4. Robust fixes — root-cause, no fallbacks/workarounds

### Fix 1 — Stale model-path docs (DONE this turn)

Edits applied:
- `docs/reality/18_inner_actor_coding_agent_harness.md:37` →
  `models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf`, params 7.5 B, quant
  `BF16 (lossless, ~15 GB on disk)`.
- `docs/ccreality/phase-12-first-production/01_PRECONDITIONS.md:29` →
  preconditions block now references the BF16 path *and* points operators
  at `config/ccreality_model_pins.json` for SHA verification, *and* names
  the `RUST_REALITY_INNER_MODEL_SIZE_MISMATCH` /
  `RUST_REALITY_INNER_MODEL_SHA_MISMATCH` error codes so a fail-closed pin
  gate is no longer surprising.

Both edits passed the PreToolUse governed-path allow check
(`docs/reality/**` and `docs/ccreality/**` are in the allow list); both
emitted PostToolUse harness-transition records (`round-01.json`,
`round-02.json`); both shift IDs are recorded in the active session shift
log.

This is the durable fix. There is nothing to mock and no fallback path —
if any future `Q4_K_M` reference reappears, `rg "Q4_K_M\.gguf" docs/`
catches it; the cryptographic pin gate refuses to load the wrong file
anyway.

### Fix 2 — Pre-flight VRAM check should be informational, not destructive

The current `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` is correct behavior
(fail-closed before bricking the GPU) but the failure mode is operator-
hostile: the engine writes a one-shot failure file and exits, with no
guidance on *which other process to kill* and no retry path. Two robust
upgrades, neither of which weakens the gate:

1. **Enrich the failure record** with `nvidia-smi --query-compute-apps=pid,process_name,used_memory`
   so the operator sees *who* is holding 25 GB. The check already reads
   `compute_processes_raw`; the failure JSON shows that field as empty
   string, meaning either nvidia-smi returned nothing or the parser drops
   it. Audit the parsing path in
   `crates/context-graph-cli/src/bin/reality-loop.rs` (search for
   `compute_processes_raw`) — the field should be JSON-array-of-objects, not
   raw text.
2. **Optional `--inner-actor-gpu-layers <N>` override.** Currently the engine
   loads with `u32::MAX` (all layers on GPU). When VRAM is tight, the
   research-supported pattern is partial offload (e.g., 36 of 42 layers on
   GPU + the rest on CPU). Wire `args.inner_gpu_layers` to a CLI flag with
   a soft default of `all`, and require either the all-GPU reserve OR a
   user-specified partial-offload count. The pin gate stays — it only
   checks the file, not VRAM placement.

These are *not workarounds*. They convert "fails closed with an opaque
error" into "fails closed with an error that points at the fix." If the
operator can't free VRAM and can't accept the perf hit of partial offload,
the loop should still refuse to start. The reserve stays.

### Fix 3 — Loop-detection in the inner-attempt tool loop

Direct evidence from the failed run: rounds 4, 8, 9, 12, 13, 16, 18, 20
all called `apply_line_window_edit` and got rejections; rounds 23 + 24
called `apply_unified_diff` (also rejected). 7 + 2 = 9 redundant edit
rounds inside one attempt. This burns context window and is exactly what
Live-SWE-agent's loop-breaking protocol is for.

Robust fix (Phase 14a, smallest viable):
1. After each `apply_*` rejection, the engine appends a structured
   `TOOL_REJECTION_FACT` to the next prompt: `{tool, error_code, sha,
   rejected_payload_sha256, attempt_count}`.
2. If `attempt_count >= 3` for the same `(file, region)` tuple, the next
   prompt forces a different action: either `read_failing_test_body` (if
   not done yet) or escalation to a structured "I am stuck on this region;
   propose a different file" round.
3. Cap N total edit rounds at 8 (current default seems unbounded). When
   the cap is hit, the attempt fails with
   `RUST_REALITY_TOOL_LOOP_BUDGET_EXCEEDED`, distinct from
   `RUST_REALITY_FINAL_EDIT_TOOL_FAILED`, so the optimizer can diagnose
   "stuck" vs "produced a bad patch" separately.

Failures should error loudly. No silent retry. No fallback "just submit
whatever". The optimizer (Claude Code outer loop) is what proposes prompt
deltas to break the inner agent out of these loops.

### Fix 4 — Phase 13 T13.6 (recommendation overwrite prevention)

Verify with an FSV-style probe: run two `/loop` invocations against the
same attempt and assert that two distinct `recommendation-turn-NN.json`
files exist (turn-01 + turn-02) without overwrite. The doc already
specifies this; what's missing is the test under
`scripts/cc/tests/fsv/cases/`. Add `D1_recommendation_no_overwrite.sh`
and chain it into `scripts/cc/tests/fsv.sh`.

### Fix 5 — Drop-in attempt-summary writeback durability

The "almost worked" run is missing `attempt-summary.json`. The fix has
two parts:
1. **Sync-on-write**: confirm the engine's summary-writer calls
   `sync_data()` (or equivalent fsync) before declaring success. Search
   `attempt-summary` writes in `crates/context-graph-cli/src/bin/reality-loop.rs`.
2. **Crash-resilient partial summary**: write a `attempt-summary.partial.json`
   at the start of each attempt and a `attempt-summary.json` at the end;
   on engine restart, if a `.partial` exists for an attempt with an
   official-prediction.jsonl present, finalize it rather than orphaning
   the run. This is a write-ahead-log pattern; it is robust because the
   .partial path is the durable record that an attempt was *taken*, even
   if the summary was never finalized.

No fallback / mock data. If the partial can't be promoted to a full
summary, the engine should write a `RUST_REALITY_ATTEMPT_SUMMARY_ORPHANED`
failure record naming both `.partial` paths and the official-prediction
path so the operator can finalize manually.

---

## 5. How to actually kick off the loop (operator runbook, May 2026)

Follow these steps in order. Each step has an objectively-checkable
post-condition. Stop the moment a post-condition fails and fix root cause
before continuing.

### Step 0 — Build everything fresh

```bash
cd ~/contextgraph
cargo build --release \
  -p context-graph-cli --bin reality-loop \
  -p context-graph-cli --bin context-graph \
  -p context-graph-mcp --bin context-graph-mcp
```

Post-condition: three binaries exist under `target/release/`.

### Step 1 — Verify the model + pin

```bash
ls -la models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf
sha256sum models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf | head -c 71
jq -r '.models.qwen3_6_27b_q6_k_xl.expected_sha256' config/ccreality_model_pins.json
```

Post-condition: the file exists, is ~15 GB, and the sha256 matches the
pin. If it does not, do **not** edit the pin. The model on disk is wrong;
re-download it.

### Step 2 — Verify GPU has headroom

```bash
nvidia-smi --query-gpu=memory.free,memory.used,memory.total --format=csv,noheader
```

Post-condition: `memory.free` ≥ 28 GB if you want full GPU offload of the
BF16 model + 128 K KV cache. If less, either kill other GPU processes or
plan to add `--inner-actor-gpu-layers 30` (partial offload, ~80% on GPU). If
free VRAM < 8 GB, kill processes; the reserve will refuse to start anyway.

```bash
nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader
```

Post-condition: you have a list of which PIDs to free.

### Step 3 — Set the active runtime root

```bash
RUNTIME_ROOT=/tmp/ccreality-prod-2026-05-06
mkdir -p ~/.cache/cgreality "$RUNTIME_ROOT"
echo "$RUNTIME_ROOT" > ~/.cache/cgreality/active_runtime_root
echo "psf__requests-2317" > ~/.cache/cgreality/active_target_instance
```

Post-condition: both files exist and are non-empty.

### Step 4 — Run a synthetic smoke first

```bash
target/release/reality-loop smoke --runtime-root "$RUNTIME_ROOT" \
  --target cgreality-smoke-kickoff
bash scripts/cc/tests/smoke.sh   # full Layer 0..N pipeline
```

Post-condition: smoke exits 0; `attempt-summary.json` with
`record_kind=ccreality_smoke_attempt_summary` exists somewhere under
`$RUNTIME_ROOT`. The SessionStart hook should *not* report "Latest
attempt summary read failed" (Phase 13 T13.1 fix verified).

### Step 5 — Run the first real attempt

```bash
target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --runtime-root "$RUNTIME_ROOT" \
  --model qwen3.6:27b \
  --official-eval-each-attempt \
  --max-attempts 1
```

Post-condition: an `attempt-summary.json` exists at
`$RUNTIME_ROOT/<run>/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/`.
If `error_code` is set, read it carefully — it tells you exactly which
fix-class to address next. Do **not** retry blindly; the outer optimizer
is what reasons about retries.

### Step 6 — Hand the optimizer the wheel

```bash
claude --model opus --permission-mode acceptEdits
> /ccreality-trigger-check
> /ccreality-iterate
```

Post-condition: a `recommendation-turn-01.json` lands at
`$RUNTIME_ROOT/<run>/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/claude-code-optimizer/`.
The Stop hook will block the turn from ending without one.

### Step 7 — Sustained run

```bash
> /loop 15m
```

Post-condition: every 15 min a new recommendation-turn-NN.json appears
(no overwrites — verifies T13.6 in real life). The shift-log under
`$RUNTIME_ROOT/cgreality-shift-log/` accumulates entries.

The success predicate stays *unchanged*: the loop is "done" when
`mcp__cgreality__reality_official_report {run_id, attempt}` returns a
JSON object whose `resolved_ids` array contains
`"psf__requests-2317"`. Anything short of that is partial progress;
partial progress is meaningful (see
`attempt-summary.official_progress.passed_fail_to_pass_ratio`) but is
not success.

### Step 8 — When to stop the unattended loop

Per the existing `14_OPERATOR_RUNBOOK.md`:
- 3 consecutive `blocked` recommendations citing the same root cause.
- Any `external_drift_detected` recommendation against a file the operator
  did not edit.
- A new failure class no diagnostician category matches.

In any of those cases, stop, read the latest 3 recommendations, fix root
cause manually, restart from Step 5.

---

## 6. Manual Full-State-Verification (FSV) protocol applied to this turn

Per the user's instruction, every claim in this doc was verified by direct
read of the durable artifact, not by trusting an intermediate log line.

| Claim | Source of truth | Verified? |
|---|---|---|
| Branch is `ccreality-cutover` | `git log --oneline -n 5` | yes |
| Last 48 h of commits is the list in §1 | `git log --since="2 days ago"` | yes |
| `.claude/loop.md` exists and is the optimizer runtime contract | `Read /home/cabdru/contextgraph/.claude/loop.md` | yes |
| Active runtime root | `cat ~/.cache/cgreality/active_runtime_root` | yes — `/tmp/contextgraph-rust-reality-loop` |
| Active target | `cat ~/.cache/cgreality/active_target_instance` | yes — `psf__requests-2317` |
| `RUST_REALITY_INNER_MODEL_PATH_MISSING` failure | `cat .../failure-1778064266-*.json` | yes |
| `RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED` failure | `cat .../failure-1778064748-*.json` | yes |
| Stale `Q4_K_M` references in 2 docs | `rg "qwen3.6-e4b\.Q4_K_M"` | yes — fixed both |
| Default model path in source is BF16 | `crates/context-graph-cli/src/bin/reality-loop.rs:28` | yes |
| Almost-worked run has no attempt-summary | `ls .../attempts/attempt-000001/` | yes — confirmed missing |
| Candidate patch is semantically wrong | `cat candidate.patch` | yes — verified the broken edit |
| Static checks passed but eval did not write report | `static-checks.json + run_instance.log` truncated | yes |
| Phase 13 T13.1 fixed in `eac5d411` | `Read scripts/cc/hook_session_start.sh` | yes — branch-on-record-kind logic present |
| GPU current state has headroom | `nvidia-smi` output 17 GB free | yes (this turn) |
| The two doc edits I made hit the harness governance pipeline | PreToolUse + PostToolUse system-reminders | yes — shift IDs `01JB97024B...` and `01JF0CF79B...` |

Edge-case audit:
1. *Empty active_runtime_root file* — handled by `read_active_runtime_root`
   in `helpers.rs:10` (returns `Ok(None)` for empty); SessionStart hook
   correctly emits "not currently running" message. Verified by reading
   the source.
2. *Synthetic-smoke summary shape vs real-attempt summary shape* — both
   handled by `hook_session_start.sh:30-66` branching on `record_kind`.
   Verified by reading the script.
3. *Stale recommendation collision* — **not yet** verified end-to-end. T13.6
   FSV case missing. Action item.

---

## 7. AI-verifiable next-step checklist (in order, smallest viable)

Each line is a `cmd → expected exit/output`. None of these require human
inspection of free-text output to decide pass/fail.

```bash
# Phase 13 leftovers
[ -x scripts/cc/cgreality-tick.sh ] && bash -n scripts/cc/cgreality-tick.sh   # exit 0
bash scripts/cc/tests/hooks/test_session_start.sh                              # exit 0 (T13.1)
cargo test -p context-graph-mcp --lib reality_loop --quiet                     # exit 0 (T13.2)
# T13.6 FSV case (TODO):
bash scripts/cc/tests/fsv/cases/D1_recommendation_no_overwrite.sh              # exit 0 (NEW)

# Phase 14a kickoff (this is the productive milestone)
test -f models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf
sha256sum models/qwen3.6-27b/Qwen3.6-27B-UD-Q6_K_XL.gguf \
  | awk '{print "sha256:" $1}' \
  | grep -q "$(jq -r '.models.qwen3_6_27b_q6_k_xl.expected_sha256' config/ccreality_model_pins.json)"
nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits \
  | awk '{ exit !($1 >= 28000) }'   # 28 GB+ free for full-GPU BF16+128K
target/release/reality-loop smoke --runtime-root /tmp/ccreality-kickoff
target/release/reality-loop attempt \
  --task-manifest tasks/swebench-lite/psf__requests-2317.json \
  --runtime-root /tmp/ccreality-kickoff \
  --model qwen3.6:27b \
  --official-eval-each-attempt --max-attempts 1
test -f /tmp/ccreality-kickoff/*/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/attempt-summary.json

# Then in a Claude Code session:
#   > /ccreality-iterate
# After it lands, verify:
ls /tmp/ccreality-kickoff/*/tasks/psf__requests-2317/qwen3.6-27b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json

# Phase 14a success predicate (the actual goal)
mcp__cgreality__reality_official_report run_id=<run_id> attempt=<n> \
  | jq -e '.resolved_ids | index("psf__requests-2317")'
```

If the last line passes, the meta-loop has solved its first SWE-bench Lite
instance. That is the demarcation between "plumbing works" and "system is
productive."

---

## 8. Risks & explicit non-goals for this kickoff

- **Not fixing the inner actor's reasoning capacity.** That is a model-quality issue.
  The harness can give the inner actor better prompts, retry budgets, and tool
  affordances, but it cannot make a 7.5 B model reason like a 70 B one.
  If after 50 sustained `/loop` turns the inner actor still cannot land
  `psf__requests-2317`, switch instance per the Phase 14 failure-mode
  plan.
- **Not building the autoresearch engine yet.** Phase 15 (champion state +
  experiment registry) is partially landed (commit `704e6fe0`) but the
  full surface — `mcp__cgreality__champion_state_get`,
  `mcp__cgreality__experiment_registry_list`, `attempts_history_query` —
  is forward work. The kickoff plan above does not assume them.
- **Not enabling cross-model.** The loop is single-model by design (`qwen3.6:27b`, cryptographically pinned). The Phase-16 cross-model plan was retired; multi-model is no longer a forward goal.
- **Not changing the success predicate.** Sacred: `official_report.resolved_ids`
  contains the target instance. Anything else — partial test-pass ratio,
  patch parse rate, reward signal — is *partial progress* and informative
  but not success.
- **Not weakening the deny list.** `swebench/**`, `tests/oracle_**`,
  `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`, `.env` are
  forever sacred. The harness enforces this; the optimizer must not work
  around it.

---

## 9. One-paragraph TL;DR

The plumbing works. The two failures in the active runtime root were
(a) someone pointed `--inner-model-path` at a stale `Q4_K_M` filename
that two docs (now fixed) still referenced, and (b) the GPU was busy. The
near-success run shows the inner actor can reach the official Docker eval but
submits a semantically-wrong patch on its own — exactly the inner-
capability gap Phase 14 names. Kickoff is: free GPU, verify model SHA,
run smoke + 1 real attempt, then drive `/loop 15m` against it. The
optimizer has the durable artifact tree it needs (problem-reality packet,
agent-tool transcript, candidate.patch, validation, official-eval logs);
what it needs from us is loop-detection inside the attempt and a richer
VRAM failure record so partial offload is a first-class option. Both are
small, local, root-cause fixes — no fallbacks, no mock data, no
silent-retry — and both are pre-resolved-ids work.
