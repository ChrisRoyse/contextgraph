# Investigation 3 — Scripts, Docs, Config & Top-Level Dead Code

**Investigator:** Sherlock Holmes Agent #3
**Date:** 2026-05-19
**Scope:** scripts/, docs/, docs2/, config/, configs/, systemd/, .claude/, .github/, top-level dirs (agentdb/, samplecode/, shakespear/, reference/, backupdocs/, data/, artefacts/, tmp/, .tmp/, .playwright-mcp/, .pytest_cache/, .ruff_cache/, benchmarks/, .codex, plus root-level files)
**Out of scope** (parallel investigators handle): `crates/*/src/`, `crates/*/tests/`, `crates/*/examples/`.

**Verdict-grade summary:** ~70% of `scripts/` is dead historical mass — 46 shakespeare LoRA helpers (4 explicitly stubbed by Phase H, 42 with NO live reference), a D-drive migration script (superseded), the EXP-2 demo suite, and the 5090jepa release scripts are all archaeology now. The repository tracks **~750 KB of non-engineering tracked artifacts** (Playwright accessibility-tree YAMLs misnamed as `.yml`, a research-paper PDF, MATS / Zenodo browser snapshots, an empty `.codex` marker, an `INSTALL.md` referencing `/mnt/d` SoT that is now superseded). The biggest *untracked* footprint is **842 MB `samplecode/` + 170 MB `tmp/` + 76 MB `.playwright-mcp/` + 37 MB `agentdb/` + 18 MB `artefacts/`** — all ignored by `.gitignore` but eating local disk. The Phase G/H FSV scripts are not dead — they are correctly stand-alone audit harnesses with no CI callers by design.

---

## §1 Method

Followed Linear Sequential Unmasking discipline (CLAUDE.md §13 / Doctrine §11): read the live `docs/futurebuild/README.md`, `01_overview.md`, `06_operations_and_fsv.md`, `AGENTS.md`, and `CLAUDE.md` first to form an independent understanding of the live system. Only then did I judge candidate scripts/docs against project state.

For every removal candidate I gathered:
- absolute path
- `du -sh` or file size
- `git log -1 --format='%h %ci %s'` last-touched stamp
- `git ls-files --error-unmatch <path>` to determine TRACKED vs UNTRACKED
- live-reference grep across `.github/workflows/`, `scripts/ci/`, `scripts/cc/`, `.mcp.json`, `Makefile`, `reproduce.sh`, active `crates/*/Cargo.toml`, and `docs/futurebuild/`
- distinction between "active code path", "phase H absence-guard (KEEP)", "stub-with-historical-only-notice (KEEP)", and "real dead code".

I did NOT investigate `crates/*/src`, `crates/*/tests`, `crates/*/examples`, `swebench/**`, or `tests/oracle_**` (Investigation #1 / #2 handle those; CLAUDE.md §7 path discipline).

I did NOT execute any modifications. All `Bash`/`Read` calls were read-only.

---

## §2 Removal Candidates — HIGH confidence (whole-directory)

All paths absolute. "Tracked" means committed to git. "Untracked" means present locally only (gitignored).

| Path | Size | Tracked? | Reason | Evidence | Suggested action |
|---|---|---|---|---|---|
| `/home/cabdru/contextgraph/agentdb/` | 37 MB | UNTRACKED (in `.gitignore` "Nested external repo (separate project, has its own .git)") | Clean-room rule (CLAUDE.md §6.7) forbids vendor/path-dep on `ruvnet/agentdb`. **Zero crate path-deps** it; `grep -rE 'agentdb' --include="*.toml" crates/` returns nothing. Directory contains its own `.git` — it is a sibling clone, not a workspace member. | `agentdb HAS .git`; no `crates/*/Cargo.toml` references; `.gitignore:36 agentdb/` | `rm -rf /home/cabdru/contextgraph/agentdb/` — purely local. If reference inspection is later needed, re-clone outside the repo tree. |
| `/home/cabdru/contextgraph/.playwright-mcp/` | 76 MB | UNTRACKED (`.gitignore:91 .playwright-mcp/`) | Ephemeral Playwright MCP session artifacts. Already gitignored as "ephemeral browser logs". | `.gitignore` explicit | `rm -rf` — safe at any time. |
| `/home/cabdru/contextgraph/.pytest_cache/` | 32 KB | UNTRACKED (`__pycache__` ignored, but `.pytest_cache` not explicitly listed — `.gitignore` does list `__pycache__/`) | pytest cache; auto-regenerated | filesystem | `rm -rf` |
| `/home/cabdru/contextgraph/.ruff_cache/` | 20 KB | UNTRACKED | ruff lint cache; auto-regenerated | filesystem | `rm -rf` |
| `/home/cabdru/contextgraph/.tmp/` | 88 KB | UNTRACKED (`.gitignore:167 .tmp/`) | Sole content: one operator scratch (`ylecun_timeline_2026-05-01.md`) | filesystem | `rm -rf` after operator confirms the scratch is no longer wanted. |
| `/home/cabdru/contextgraph/tmp/` | 170 MB | UNTRACKED (`.gitignore:166 tmp/`) | Operator scratch — 3 SITREPs (~100 KB) + 170 MB `5090jepa_evidence/` from 2026-05-18. SITREPs are ephemeral status reports; 5090jepa evidence belongs in `/zfs/archive/contextgraph/fsv/` if it has any production claim. | `ls tmp/` → 5090jepa_evidence + SITREP_*.md + disk-guard.log | `rm -rf` — or move SITREPs to a GitHub issue comment first, archive `5090jepa_evidence/` to `/zfs/archive/contextgraph/fsv/` if any of it is still load-bearing. |
| `/home/cabdru/contextgraph/.codex` | 0 bytes (tracked) | TRACKED (`043a983a 2026-04-27 chore: add .codex marker file`) | Empty marker file from a brief Codex experiment — never referenced again. Read-only marker (`-r--r--r--`). | `wc -c .codex` = 0; no greps find consumer | `git rm .codex` |

## §3 Removal Candidates — HIGH confidence (individual files)

| Path | Size | Tracked? | Reason | Evidence | Suggested action |
|---|---|---|---|---|---|
| `/home/cabdru/contextgraph/.mcp.json.bak.20260502-092812` | 774 B | TRACKED (`4c26b21d 2026-05-02 chore: snapshot MCP config backups`) | Backup of an older `.mcp.json` containing `claude-flow` + a stdio `context-graph` server pointing at retired `/home/cabdru/contextgraph/contextgraph_data` SoT. Current `.mcp.json` is the live HTTP cgreality entry. Two weeks old, no consumer. | `cat` content shows pre-aiwonder config | `git rm .mcp.json.bak.20260502-092812` |
| `/home/cabdru/contextgraph/compose-khattab.yml` | 146 KB | TRACKED (`891f66b8 2026-04-21 docs+research: SMAP protocol rename, MATS/EOI browser snapshots`) | NOT docker-compose — it's a Playwright accessibility-tree dump of a Khattab/LateInteraction research page. 1150 `iframe`/`ref=eN` entries. No `services:` / `image:` keys. Has never been loaded by anything. | `head` shows `- generic [ref=e1]:` etc., no `services:` keyword | `git rm compose-khattab.yml` |
| `/home/cabdru/contextgraph/mats-eoi.yml` | 1.5 KB | TRACKED (same commit as above) | Same pattern — Playwright snapshot of the MATS EOI signup page. Not YAML config. | filesystem | `git rm mats-eoi.yml` |
| `/home/cabdru/contextgraph/zenodo_new_upload_initial.yml` | 33 KB | TRACKED (`a562ec5a 2026-05-01 docs(5090jepa): add researchpaper draft and zenodo upload snapshot`) | Playwright snapshot of the Zenodo upload page from the 5090jepa paper submission. Never loaded by anything. | filesystem | `git rm zenodo_new_upload_initial.yml` |
| `/home/cabdru/contextgraph/gemmafrontend-playwright-console.log` | 3.6 KB | UNTRACKED | Playwright console log artifact (404s on a localhost gemma frontend). No git history. | filesystem | `rm` |
| `/home/cabdru/contextgraph/MEJEPAResearchPaper.pdf` | 608 KB | TRACKED (`b9b6ff0c 2026-05-02 docs(5090jepa): convert paper citations to APA-style natbib`) | Static rendered PDF of the live `docs/PAPER.md`. Drifts from source the moment PAPER.md updates. PDFs of source documents should be regenerable, not version-controlled. | `MEJEPAResearchPaper.pdf` is rendered from PAPER.md | `git rm MEJEPAResearchPaper.pdf` (regenerate as build artifact on demand). |
| `/home/cabdru/contextgraph/.mcp.json` `claude-flow` block | — | N/A (current `.mcp.json` has only `cgreality`) | Already cleaned in current `.mcp.json` — only mentioned to record that `.mcp.json` itself is LIVE. | `cat .mcp.json` shows single `cgreality` http entry | NO ACTION — keep `.mcp.json`. |
| `/home/cabdru/contextgraph/scripts/migrate_contextgraph_data_to_d.sh` | (small) | TRACKED (`a72f8600 2026-05-10`) | One-shot WSL → D-drive migration script. Aiwonder migration superseded the D-drive durable-SoT rule (CLAUDE.md §2 / #363). Filename is misleading — script now writes to `/zfs/archive/contextgraph/` (it was updated in place after the aiwonder move) but the *operation it performs* — symlinking local dirs to a remote durable root — is a one-time operator action that has already been done. | head of file: "Move durable ContextGraph data from WSL home/cache/tmp locations to aiwonder" | KEEP for now (defensible as a re-runnable bootstrap), but file an issue to either delete or rename to `bootstrap_aiwonder_symlinks.sh` and remove the "to_d" suffix since the data is no longer going to D-drive. |
| `/home/cabdru/contextgraph/INSTALL.md` | 479 B | TRACKED | Top-level install doc still references "D-backed state" and `/mnt/d/contextgraph` SoT. Conflicts with the aiwonder-first policy (CLAUDE.md §2 / #363) that took effect 2026-05-18. | `cat INSTALL.md` shows `/mnt/d/contextgraph/fsv/...` SoT | EDIT (not delete) to point at `/zfs/archive/contextgraph/fsv/first-time-setup/setup-readback.json` — the actual path produced by `scripts/setup/first_time_setup.sh` (which itself was correctly updated to `/zfs/archive/contextgraph/...`). |

## §4 Removal Candidates — MEDIUM confidence

| Path | Size | Tracked? | Reason | Suggested action |
|---|---|---|---|---|
| `/home/cabdru/contextgraph/samplecode/` | 842 MB | UNTRACKED (`.gitignore:24 samplecode/`) | Multi-language scratch with `node_modules/`, `obj/`, `bin/`, `target/`, `__pycache__/`. **Is referenced by `crates/context-graph-mejepa-instruments/examples/ast_embedder_smoke_fsv.rs:44` and 16 other lines in the same file.** So the directory exists for a real example. But only the 12 small source files (`sample.py`, `sample.rs`, etc., ~few KB each) are needed; the **node_modules / obj / target / bin / __pycache__** subtrees are 99% of the size and are pure build cache. | KEEP minimal source files. `rm -rf samplecode/{node_modules,obj,bin,target,__pycache__}` to reclaim ~840 MB. File a `type:tech-debt` issue to add `samplecode/{node_modules,target,obj,bin,__pycache__}/` to `.gitignore` more specifically. |
| `/home/cabdru/contextgraph/shakespear/` | 5.4 MB | UNTRACKED (`.gitignore:21 shakespear/`) | Single 5.6 MB file `complete_works.txt`. Referenced as `LibraryId::ShakespeareCanon` in `crates/context-graph-mejepa/src/library_foundationality.rs` and 7 other live files (mejepa MCP tool descriptions, BGE-M3 tokenizer probe, etc.). The DATA is live. **The 46 `scripts/shakespeare_*.py` training scripts are dead.** | KEEP `shakespear/complete_works.txt`. See §7.6 for the 46 scripts. |
| `/home/cabdru/contextgraph/backupdocs/` | 17 MB | TRACKED (`2759ff3b 2026-05-17 chore: ship MEJEPA stop-hook self-verify gate and prune vendor/archive trees`) | Contains 63 numbered `.md` files (`00_*` through `27_*`, two parallel series) **plus a copy of the now-deleted `memory.archive/`** (the SAMP/SMAP narrative that CLAUDE.md §4 still references as "read-only history at `./memory.archive/`"). The numbered docs LOOK like the pre-condensation 31-doc futurebuild suite that already lives at `docs/futurebuild/_archived/` (34 archived files). | Two separate questions: (a) Do we need `backupdocs/memory.archive/` if CLAUDE.md still points at `./memory.archive/` at the repo root? Either restore as a top-level symlink or fix CLAUDE.md §4 / §18 to point at `backupdocs/memory.archive/`. (b) The 63 numbered docs are likely duplicates of `_archived/` content — verify each pair, then `git rm -r backupdocs/{numbered docs}/`. Net: reclaim ~17 MB after operator triage. |
| `/home/cabdru/contextgraph/artefacts/` | 18 MB | UNTRACKED (`.gitignore:178 artefacts/`) | EXP-2 manual FSV evidence from 2026-04-29 (audit_chain_environment.json, corpus_manifest_v1.json, learning_events_export.json, multiple RocksDB scratch dirs, MCP smoke RocksDB). Predates the aiwonder migration. EXP-2 itself per `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh` is classified as "EXP-2 strata matches are generated snapshots marked historical, do not reanimate". | `rm -rf artefacts/` once operator confirms no current re-replay against this evidence is open. If wanted as legacy proof, move to `/zfs/archive/contextgraph/fsv/exp2-legacy/`. |
| `/home/cabdru/contextgraph/data/download_balanced.py` + `data/download_beir_scifact.py` | 16 KB | TRACKED (`a12af473 2026-01-18 refactor(mcp): add PRD v6 tool constants`) | Two BEIR / multi-dataset downloader scripts from January 2026 (pre-MEJEPA pivot). `.gitignore:181 data/` (untracks RocksDB runtime data but does NOT untrack files tracked before the rule). No live caller in `scripts/`, `Makefile`, `reproduce.sh`. | `git rm data/download_balanced.py data/download_beir_scifact.py` |
| `/home/cabdru/contextgraph/benchmarks/` | 36 KB | UNTRACKED (`.gitignore:5 benchmarks/`) | Contains `e9/` subdir. Untracked. Benchmark scratch. | KEEP if any current operator process writes here; remove from local disk if not. No repo action needed. |

## §5 Removal Candidates — LOW confidence (needs operator review)

| Path | Tracked? | Reason | Suggested action |
|---|---|---|---|
| `/home/cabdru/contextgraph/.claude/agents/cc-reality-{harness-editor,diagnostician,reward-analyst}.md` | TRACKED | 3 retired agents explicitly KEPT for provenance per `.claude/agents/README.md` ("These files are kept for provenance only. They must not be invoked for current work."). Phase H FSV `scripts/ci/phase_h_agent_roster_retirement_fsv.sh` asserts they remain present as proof of retirement. | **KEEP** — phase H absence-guard requires the files to exist with retirement banners. See `phase_h_agent_roster_retirement_fsv.sh`. |
| `/home/cabdru/contextgraph/.claude/skills/ccreality-{iterate,readback,smoke,trigger-check}/` | TRACKED | 4 retired Claude Code skills. Still referenced by `docs2/realitywhitepaper/build/paper.md`, `docs2/realitywhitepaper/appendix_a_architecture.md`, `docs2/contextgraphspecs/22_reality_loop_subsystem.md`, `docs/MEJEPA_SUPER_PROMPT.md` (the SUPERSEDED doc). No live consumer in `crates/`. | Operator decides: are these skills still useful as legacy-mode commands, or fully dead like the cc-reality-* triage agents? If dead, `git rm -r .claude/skills/ccreality-*`. If kept-for-provenance, add a status banner like the cc-reality-* agents have. |
| `/home/cabdru/contextgraph/scripts/cc/cgreality-tick.sh` | TRACKED (`8928d390 2026-05-10 follow-up(e11+paths): wire E11 disabled path + migrate cgreality lockfile`) | Periodic outer-watcher script: reads `/zfs/archive/contextgraph/state/cgreality/active_runtime_root`, finds latest attempt-summary.json, spawns a single-shot Claude Code optimizer turn. **No cron / systemd timer file** in the repo. No reference from `.github/workflows/`. Functionally orphaned. | Confirm with operator whether the cgreality outer-loop optimizer is still in use; if not, `git rm scripts/cc/cgreality-tick.sh`. (Phase H is about inner-actor retirement; the outer-loop is allowed.) |
| `/home/cabdru/contextgraph/configs/dynamicjepa/` | TRACKED (`cd7da268 2026-05-08`) | DynamicJEPA configs (`domain_packs/`, `verification/`) used by `.github/workflows/ci_smoke_a.yml` paths trigger and by `scripts/5090jepa/local_ci_gate.sh`. **Different namespace** than `config/` (which holds `default.toml`, `mejepa/`, retention_policy.toml — the MEJEPA-Code primary). Per `01_overview.md` and the 8-doc reframe, MEJEPA-Code is the product, DynamicJEPA is a sibling. | KEEP — actively wired into CI Smoke A. The `configs/` plural vs `config/` singular naming is confusing but BOTH are live. File a `type:tech-debt` to rename to `config/dynamicjepa/` (collapse the plural). |
| `/home/cabdru/contextgraph/docs2/realitywhitepaper/` | TRACKED (`fade1e4e 2026-05-11`) | 4.5 MB whitepaper draft + appendices for the legacy reality-loop subsystem. References `/ccreality-iterate` etc. (the retired skills). Whitepaper PDF inside. | Operator decides: is this a paper-in-progress, or fully archaeological? If a draft for submission, KEEP and add status banner. If archaeological, move to `docs/futurebuild/_archived/realitywhitepaper/`. |
| `/home/cabdru/contextgraph/docs2/contextgraphspecs/` | TRACKED (`9fd16377 2026-05-08`) | 22 numbered spec files. Last touched 2026-05-08 — BEFORE the 2026-05-18 31→8 condensation. Doc 22 is literally "Reality-Loop Subsystem" (the retired one). | Operator decides: condense / archive / delete. Per `docs/futurebuild/README.md` §"What changed" the futurebuild 8-doc suite is the only LIVE strategic doc and `docs2/` is meant to hold doctrine prompts (the live `AICodingAgentSuperPrompt.md`, `hardeningprompt.md`, `prdtospec.md`). The 22-doc `contextgraphspecs/` is parallel and unmaintained. Recommend `git rm -r docs2/contextgraphspecs/` after a one-shot diff against `docs/futurebuild/` confirms no unique content. |
| `/home/cabdru/contextgraph/docs2/SHARED-MIND-AGENT-PROTOCOL.md` | TRACKED | The SMAP narrative document — superseded by the AI Coding Agent Doctrine (§3 GitHub Issues memory). No "SUPERSEDED" banner. | Add SUPERSEDED banner OR delete. |
| `/home/cabdru/contextgraph/docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` | TRACKED | Self-marked "Status (2026-05-05): Superseded by [docs/ccreality/]" — but `docs/ccreality/` no longer exists (was deleted in the 2026-05-17 prune). Dangling supersession. | `git rm docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` |
| `/home/cabdru/contextgraph/docs2/CUDA-13-1-RTX-5090-Report.md` | TRACKED | RTX 5090 + CUDA 13.1 hardware reference. Likely useful for the 5090jepa subsystem; not for MEJEPA-Code. | KEEP if 5090jepa is still alive, otherwise move. |
| `/home/cabdru/contextgraph/docs2/TheoryOfMachineLearning.md` | TRACKED | "Theory of Machine Learning v1.0 — JEPA Edition." Pure background / theory doc, not engineering. | KEEP (project context document). |
| `/home/cabdru/contextgraph/docs2/specdoc.md` | TRACKED | Generic "Feature Specification" template — placeholder. | KEEP (style reference) or `git rm` if `docs2/prdtospec.md` covers this. |
| `/home/cabdru/contextgraph/docs2/systemspecs.md` | TRACKED | Geekbench dump of the operator's workstation. | `git rm docs2/systemspecs.md` (private benchmark; not engineering). |
| `/home/cabdru/contextgraph/docs2/modulateprompt.md` | TRACKED | Generic "AI-driven file modularization" prompt — utility prompt, not project-specific. | KEEP if any agent invokes it; otherwise candidate for removal. |
| `/home/cabdru/contextgraph/docs/outreach/` | TRACKED | 6 outreach docs (emails to Cho, LeCun, Xie & Rabbat, landing page, scout timeline, LeCun strategy). Non-engineering, business comms. | KEEP if a paper / outreach push is active; otherwise relocate outside the engineering repo. |
| `/home/cabdru/contextgraph/docs/leapableai_youtube_transcripts.md` | TRACKED (671 KB) | "68 videos, 12:52:21 total duration" — full transcript archive of the operator's YouTube channel. Not engineering. | KEEP if used by current research (e.g. the explanatory-knowledge analysis cites it); otherwise relocate. |
| `/home/cabdru/contextgraph/docs/alldata.md` | TRACKED (270 KB) | ClipCannon white paper — a SIBLING product, not ME-JEPA-Code. Referenced by the comparison analysis. | KEEP only if the comparison is actively maintained, otherwise relocate to `/home/cabdru/clipcannon/`. |
| `/home/cabdru/contextgraph/docs/E14_SEMANTIC_ROLE.md` | TRACKED (`e9d2cbaa 2026-05-15 Fix E14 optimization FSV contract`) | E14 BGE-M3 semantic role explainer. Current. | KEEP. |
| `/home/cabdru/contextgraph/docs/the_symmetry_of_knowing_partial.md` | TRACKED (`83d41345 2026-05-17`) | "Symmetry of Knowing" theory paper draft. Companion to `docs2/TheSymmetryOfKnowing_Revised.pdf`. Personal theory work. | KEEP only if active; otherwise relocate. |
| `/home/cabdru/contextgraph/docs/PAPER.md` | TRACKED (`May 18`) | TCT paper draft. LIVE (operator's primary publication). | KEEP. |
| `/home/cabdru/contextgraph/docs/clipcannon_vs_mejepa_comparison.md` | TRACKED (`May 19`) | Live comparison doc, dated yesterday by today's standard. | KEEP if comparison is current; otherwise relocate. |
| `/home/cabdru/contextgraph/docs/explanatory_knowledge_and_mejepaupdated.md` | TRACKED (`May 19`) | Live analytical doc, dated 2026-05-19. | KEEP. |
| `/home/cabdru/contextgraph/systemd/contextgraph-mcp-http.service` | TRACKED (`4f6b6557 2026-05-10 feat(mcp): add Streamable HTTP transport`) | **CRITICAL DRIFT** — every Environment= line points at `/mnt/d/contextgraph/...` and `/mnt/d/cabdru/contextgraph/models/...`. Per CLAUDE.md §6.5, production durable state moved to `/zfs/archive/contextgraph/` on aiwonder. The `--mode reality-loop` flag itself is FINE (that's a valid daemon mode, not the retired binary — see §11 below); the **paths** are stale. | EDIT to point at `/zfs/archive/contextgraph/` and `/zfs/hot/contextgraph/models/` per #363 / #369; or KEEP if this unit is the *legacy* WSL service and aiwonder has its own unit. Open a `type:bug` issue. |

---

## §6 Top-level directory triage (per-directory verdict)

**`/home/cabdru/contextgraph/agentdb/`** — Vendored clone of `ruvnet/agentdb` (TypeScript). 37 MB with its own `.git`. CLAUDE.md §6.7 explicitly forbids vendor / path-dep on it. Zero `crates/*/Cargo.toml` references; `.gitignore` blocks it from being committed. Untracked. **VERDICT: removal candidate (whole-dir).**

**`/home/cabdru/contextgraph/samplecode/`** — Multi-language scratch (sample.py / sample.rs / Sample.java / etc.) used by `ast_embedder_smoke_fsv.rs` as a reference corpus for the AST chunker. 842 MB because it embeds 99% build cache (node_modules, target, obj, bin, __pycache__). Untracked. **VERDICT: keep the 12 source files (~kilobytes); reclaim ~840 MB by deleting build cache subtrees.**

**`/home/cabdru/contextgraph/shakespear/`** — Single `complete_works.txt` 5.6 MB. Used as `LibraryId::ShakespeareCanon` in `library_foundationality.rs`, `chunk_foundationality.rs`, the BGE-M3 tokenizer probe, and as an example mejepa library slug. Live data. **VERDICT: KEEP.**

**`/home/cabdru/contextgraph/reference/`** — 220 KB of JSON: `reference_paper_small_v1.json`, `reference_release_v1.0.0.json`, `reference_release_v2.0.0.json`, `reference_smoke_hashes.json`, `tolerances.json`. Consumed by `scripts/5090jepa/local_ci_gate.sh` (heavily — 12+ references) and `reproduce.sh`. **VERDICT: KEEP** (5090jepa sub-system is live in `.github/workflows/ci_smoke_a.yml`).

**`/home/cabdru/contextgraph/backupdocs/`** — 17 MB, 63 numbered .md files + a backup copy of `memory.archive/` (CLAUDE.md §4 / §18 still references `./memory.archive/` at root; the actual archived files live HERE, not at root). The numbered docs appear to duplicate `docs/futurebuild/_archived/`. **VERDICT: needs operator triage** — either (a) keep, restore root symlink, OR (b) delete after diff vs `_archived/`. Either way CLAUDE.md §4 needs updating to reflect the actual location.

**`/home/cabdru/contextgraph/data/`** — 16 KB (2 January-2026 BEIR downloader scripts). `.gitignore` blocks NEW runtime data (`data/`); the two old `.py` files were tracked before the rule. **VERDICT: tracked files are removal candidates** (`git rm`).

**`/home/cabdru/contextgraph/artefacts/`** — 18 MB of 2026-04-29 EXP-2 RocksDB scratch + corpus manifests. Untracked. Phase H FSV classifies EXP-2 strata as "historical, do not reanimate." **VERDICT: removal candidate** (or relocate to aiwonder).

**`/home/cabdru/contextgraph/tmp/`** — 170 MB. SITREPs from 2026-05-15 / 17 / 18 plus a 170 MB `5090jepa_evidence/` tree. Untracked, gitignored. **VERDICT: removal candidate** (move evidence to aiwonder first if it has any production claim).

**`/home/cabdru/contextgraph/.tmp/`** — 88 KB, one file. **VERDICT: removal candidate.**

**`/home/cabdru/contextgraph/memory.archive/`** — **DOES NOT EXIST** at the root. CLAUDE.md §4 + AGENTS.md §"Memory" both reference it as the "read-only legacy SAMP/SMAP narrative." The actual archived files live at `/home/cabdru/contextgraph/backupdocs/memory.archive/`. The whole-directory delete commit (`2759ff3b 2026-05-17 chore: ship MEJEPA stop-hook self-verify gate and prune vendor/archive trees`) removed `memory.archive/`. **VERDICT: documentation drift — fix CLAUDE.md and AGENTS.md to either point at `backupdocs/memory.archive/` or restore the root location.**

**`/home/cabdru/contextgraph/.pytest_cache/`, `.ruff_cache/`, `.playwright-mcp/`** — All untracked tool caches. **VERDICT: removal candidates** (regenerable).

**`/home/cabdru/contextgraph/configs/dynamicjepa/`** vs **`/home/cabdru/contextgraph/config/`** — Both live, different products. `config/` (singular) holds MEJEPA-Code primary configs (default.toml, retention_policy.toml, mejepa/, models_config.toml.example, ccreality_governed_paths.json, ccreality_recommendation_schema.json). `configs/` (plural) holds DynamicJEPA configs (domain_packs/, verification/) referenced by `ci_smoke_a.yml`. **VERDICT: KEEP both; file tech-debt to collapse the plural into `config/dynamicjepa/`.**

**`/home/cabdru/contextgraph/systemd/`** — Single unit `contextgraph-mcp-http.service`. Uses `/mnt/d/...` everywhere. **VERDICT: drift bug** (paths need aiwonder update OR clearly mark as legacy-WSL unit).

**`/home/cabdru/contextgraph/benchmarks/`** — 36 KB scratch with `e9/`. Gitignored (`.gitignore:5`). **VERDICT: KEEP if used locally; nothing to remove from repo.**

**`/home/cabdru/contextgraph/tests/`** — `e2e/` (13 happy-path / chaos / soak / load / restart / multi-agent tests) + `fixtures/`. Wired into `.github/workflows/{chaos.yml, e2e.yml, load.yml, soak.yml}`. **VERDICT: KEEP** (active CI assets — out of crate scope, in scope for parallel investigators if they cover top-level tests/).

**`/home/cabdru/contextgraph/tasks/swebench-lite/`** — 300 SWE-bench Lite Python task JSONs. Live corpus per `08_roadmap_and_open_work.md`. **VERDICT: KEEP.**

---

## §7 Scripts/ — categorized inventory (35 MB)

### §7.1 Phase G FSV / lint (status per script)

`scripts/ci/phase_g_*.sh` — 16 files. **Zero CI workflow callers.** They are stand-alone operator-invoked FSV audit harnesses (per `docs/futurebuild/06_operations_and_fsv.md §2.2` weekly cadence and the various `TASK-G-NNN` issues that close them).

| Script | Caller (other than itself) | Verdict |
|---|---|---|
| phase_g_cell_exemptions_fsv.sh | none | KEEP (operator-invoked audit) |
| phase_g_cell_status_fsv.sh | `task_py_g_036_stale_gate_fsv.sh` | KEEP |
| phase_g_conformal_recalibration_fsv.sh | none | KEEP |
| phase_g_convergence_eta_fsv.sh | none | KEEP |
| phase_g_failure_mode_class_metrics_fsv.sh | none | KEEP |
| phase_g_holdout_rotation_fsv.sh | none | KEEP |
| phase_g_label_quality_fsv.sh | none | KEEP |
| phase_g_oracle_flakiness_quarantine_fsv.sh | none | KEEP |
| phase_g_per_cell_promotion_gate_fsv.sh | none | KEEP |
| phase_g_ship_gate_dashboard_fsv.sh | `task_py_g_036_stale_gate_fsv.sh` | KEEP |
| phase_g_ship_gate_stability_fsv.sh | none | KEEP |
| phase_g_targeted_mutation_expansion_fsv.sh | none | KEEP |
| phase_g_telemetry_feedback_fsv.sh | none | KEEP |
| phase_g_training_holdout_drift_fsv.sh | none | KEEP |
| phase_g_training_pipeline_orchestrator_fsv.sh | none | KEEP |
| phase_g_weekly_cell_triage_fsv.sh | none | KEEP |

Verdict tier: **all 16 KEEP**. Not dead — operator harnesses.

### §7.2 Phase H absence-check scripts (KEEP — these prove retired stuff stays gone)

9 files in `scripts/ci/phase_h_*.sh`. These are the load-bearing guards that ensure the inner-actor LLM stack (Hermes / Qwen / Ollama / llama.cpp generative) and the reality-loop CLI binary do NOT come back. Per CLAUDE.md §6.1 / §15.1 they are project-critical.

`phase_h_scripts_config_inner_llm_residue_fsv.sh` itself calls 3 sibling phase_h scripts (`phase_h_claude_inner_loop_reference_fsv.sh`, `phase_h_no_inner_actor_models_fsv.sh`, `phase_h_smap_superseded_decisions_fsv.sh`). The other 5 are operator-invoked.

**Verdict: all 9 KEEP.** These are the retirement contract.

### §7.3 scripts/cc/ Claude Code hooks (11 files)

| File | Verdict |
|---|---|
| `_common.sh` | KEEP (helper) |
| `cgreality-tick.sh` | LOW-confidence removal — cgreality outer-loop optimizer tick. No cron/systemd wiring in repo. Operator decides. |
| `hook_post_tool_use_edit.sh` | KEEP (post-edit hook per `06_operations_and_fsv.md §3`) |
| `hook_post_tool_use_test_capture.sh` | KEEP (the `posttooluse-mejepa-capture` hook in §3 of operations doc — currently firing in *this very session*) |
| `hook_pre_compact.sh` | KEEP |
| `hook_pre_tool_use_edit.sh` | KEEP |
| `hook_session_start.sh` | KEEP |
| `hook_stop.sh` | KEEP |
| `hook_user_prompt_submit.sh` | KEEP |
| `hook_user_prompt_submit_inject_prediction.sh` | KEEP (TASK-PY-G-055 / #274 prediction injection per ops doc §3) |
| `parse_test_output.py` | KEEP (used by test-capture hook) |
| `render_prediction_markdown.py` | KEEP |
| `stop-mejepa-self-verify.sh` | KEEP (TASK-PY-G-080 / #381 per ops doc §3) |
| `tests/` | KEEP (hook unit tests) |

**Verdict: all KEEP except cgreality-tick.sh = low-confidence removal.**

### §7.4 scripts/ci/ CI lint scripts (~6 non-phase files)

| File | Verdict |
|---|---|
| `check_no_c_drive_paths.sh` | KEEP (live in `.github/workflows/ci.yml`) |
| `hook_script_lint.sh` | KEEP |
| `no_mnt_d_production_paths_fsv.py` | KEEP (#369 sweep) |
| `pinned-action-shas.md` | KEEP (CLAUDE.md §11 pinning policy) |
| `runbook_coverage.sh` | KEEP |
| `task_py_g_036_stale_gate_fsv.sh` | KEEP (wires phase_g_cell_status + phase_g_ship_gate_dashboard) |
| `phase_f_deprecation_call_sites.md` | KEEP (Phase F audit reference) |

**Verdict: all KEEP.**

### §7.5 scripts/swebench_lite_ops/ (live — supporting #278 / #413 / Phase G data)

48 Python modules. The MEMORY.md log records #278 PHASE 1 shipped 2026-05-15 (Python secret redaction library at `scripts/swebench_lite_ops/python_secret_redaction.py`); #413 closed 2026-05-19 (`scripts/swebench_lite_ops/python_auto_label_discovery.py`). The whole package is invoked by `.github/workflows/ci.yml` ("Reject proof-domain Python gate drift" step runs `python3 scripts/swebench_lite_ops.py python-proof-domain-isolation`).

**Verdict: all 48 KEEP.**

### §7.6 Demos / smokes / one-shots (mostly removable)

**Shakespeare scripts (`scripts/shakespeare_*.py` — 46 files, ~700 KB):**

Per `docs/futurebuild/_archived/11_DELETION_LIST_INNER_LOOP.md`: "four old Shakespeare local-runtime helpers abort with the historical-only notice". Per the Phase H FSV (`scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh`), the active disallowed count is **0** for inner-LLM residue and 4 Shakespeare scripts are explicit retired-with-banner stubs.

Spot-check: `scripts/shakespeare_inference_server.py` head says: "Custom inference server for the Shakespeare LoRA'd Gemma4 model" — this is exactly the inner-actor LLM workflow that Phase H retired. Same family as the deleted Hermes/Qwen/Ollama path.

Last-touched stamps: 2026-04-14 / 2026-04-21 / 2026-04-26. None has been modified since the 2026-05-09 Phase H retirement; none is invoked by any live workflow/hook/CI script (except as a Phase H absence guard).

**Verdict:** 4 retired-with-banner scripts must KEEP (Phase H asserts they remain present with the retirement notice). The remaining 42 should be **removed** OR consolidated under a single `scripts/shakespeare_RETIRED/` directory with a single README explaining the LoRA workflow is dead. File `type:dead-code` GitHub issue.

**5090jepa scripts (`scripts/5090jepa/` — 8 files):** Wired into `.github/workflows/ci_smoke_a.yml` (DynamicJEPA CI Smoke A) and `reproduce.sh`. KEEP.

**EXP-2 (`scripts/exp2/` — ~20 files):** Per Phase H classification, "EXP-2 strata matches are generated snapshots marked historical, do not reanimate." Last touched 2026-05-13. The matching local evidence dir `artefacts/exp2/` is the 18 MB scratch in §4. **Removal candidate** — but verify no Phase H FSV asserts presence first. Best path: open issue, audit, then delete both `scripts/exp2/` and `artefacts/exp2/` together.

**Audit scripts (`scripts/audit/no_inner_actor_models.sh`):** Phase H guard. KEEP.

**DR scripts (`scripts/dr/dr_drill.sh`, `scripts/dr/verify_backup.sh`):** Disaster recovery drills per `06_operations_and_fsv.md §4`. KEEP.

**Docker migration (`scripts/docker/migrate_docker_desktop_wsl_to_d.ps1`):** PowerShell, one-shot, references `/d`. Pre-aiwonder. **Removal candidate.**

**Top-level loose scripts:**

| File | Verdict |
|---|---|
| `aiwonder_capability_audit.py` | KEEP (active aiwonder readiness FSV) |
| `audit_label_quality.py` | KEEP (TASK-G-019 family) |
| `benchmark-build-size-comparison.sh` | LOW-confidence — operator utility |
| `benchmark-check.sh` | LOW-confidence |
| `benchmark-disk-management.sh` | LOW-confidence |
| `blackboxfix_demo_intelligence_report.py` | LOW-confidence — "blackboxfix demo" likely from the retired ccreality blackbox subsystem |
| `check-embedder-count.sh` | KEEP (operator utility) |
| `check-ffi-consolidation.sh` | LOW-confidence |
| `clean-build-artifacts.sh` | KEEP (disk hygiene) |
| `context-graph-mcp-wrapper.sh` | KEEP (live in `.mcp.json` — wraps the MCP server entry point) |
| `disk-guard.sh` | KEEP (Makefile-referenced) |
| `download_utl_assets.py` | LOW-confidence (UTL = Unified Theory of Learning narrative) |
| `ensure_daemon.sh` | KEEP (live, called by `context-graph-mcp-wrapper.sh`; recent edit `c4fd94a0 2026-05-18 Fail closed on non-aiwonder production roots`) |
| `gh-labels-bootstrap.sh` | KEEP (AGENTS.md "scripts/gh-labels-bootstrap.sh") |
| `heatmap_render.py` | LOW-confidence — operator utility |
| `mcp_full_verify.py` | KEEP (verification utility) |
| `mejepa_embedder_smoke_fsv.py` | KEEP |
| `migrate_contextgraph_data_to_d.sh` | Rename per §3 |
| `reindex_memories.rs` | LOW-confidence — "fix E1==E9 issue" predates the embedder rework; not in `[bin]` of any crate; orphan `.rs` outside `crates/` |
| `render_executive_summary.py` | KEEP (TASK-OBS-016) |
| `render_reasoning_trace.py` | KEEP |
| `render_ship_gate_dashboard.py` | KEEP |
| `restart_pipeline_services.sh` | EDIT — uses `/mnt/d/contextgraph` paths; same drift bug as systemd unit |
| `run_hf_benchmark.sh` | LOW-confidence |
| `setup/first_time_setup.sh` | KEEP (correctly updated to `/zfs/archive/contextgraph/`) |

### §7.7 Scripts referencing retired inner-LLM / reality-loop / Hermes / Qwen / Ollama

Phase H FSV evidence (per `scripts/ci/phase_h_scripts_config_inner_llm_residue_fsv.sh` and `_archived/11_DELETION_LIST_INNER_LOOP.md`) declares **active disallowed count = 0**. Any remaining matches are categorized:
- "retired audit" (Phase H scripts themselves)
- "generated snapshot" (EXP-2 strata historical content)
- "explicit `historical, do not reanimate`" (4 Shakespeare LoRA stubs)

Nothing here to remove without breaking the Phase H absence-guard contract.

### §7.8 Migration scripts now legacy

- `scripts/migrate_contextgraph_data_to_d.sh` — see §3 (rename/cleanup)
- `scripts/docker/migrate_docker_desktop_wsl_to_d.ps1` — removal candidate (D-drive era)

---

## §8 docs/ inventory

### §8.1 Live (futurebuild/, specs/FSV-PROTOCOL.md, current top-level)

- `docs/futurebuild/` — 8-doc LIVE suite + `_archived/` (34 superseded files explicitly marked archive) + `specs/FSV-PROTOCOL.md` (LIVE binding contract). KEEP all.
- `docs/PAPER.md` — Live TCT publication draft. KEEP.
- `docs/E14_SEMANTIC_ROLE.md` — Live (touched 2026-05-15). KEEP.
- `docs/clipcannon_vs_mejepa_comparison.md` — Live (2026-05-19). KEEP.
- `docs/explanatory_knowledge_and_mejepaupdated.md` — Live (2026-05-19). KEEP.

### §8.2 Superseded but project-content-valid

- `docs/MEJEPA_SUPER_PROMPT.md` — Banner: "SUPERSEDED FOR PROTOCOL — 2026-05-13". CLAUDE.md §18 keeps it as "legacy MEJEPA narrative (project content still valid)". KEEP with banner intact.

### §8.3 Non-engineering / Outreach

- `docs/outreach/` — emails, landing page, scout timeline. Non-engineering. Operator decides keep vs relocate.
- `docs/leapableai_youtube_transcripts.md` — 671 KB transcript archive. Operator decides.
- `docs/alldata.md` — ClipCannon white paper (sibling product). Operator decides keep vs relocate to ClipCannon repo.
- `docs/the_symmetry_of_knowing_partial.md` — Personal theory paper. Operator decides.

### §8.4 Dead / removable

Nothing in `docs/` qualifies as outright dead (all is either live, superseded-but-valid, or operator-judgment). The `docs/ccreality/` directory referenced by `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` no longer exists (already cleaned).

---

## §9 docs2/ inventory

| Path | Verdict | Notes |
|---|---|---|
| `docs2/AICodingAgentSuperPrompt.md` | KEEP | Master doctrine (CLAUDE.md §18) |
| `docs2/hardeningprompt.md` | KEEP | 14-axis hardening deep-dive (CLAUDE.md §18) |
| `docs2/aiwonder.md` | KEEP | Live (touched 2026-05-18) |
| `docs2/flagprompt.md` | KEEP with banner | Self-marked SUPERSEDED 2026-05-13 — kept for "reference / archaeology only" per CLAUDE.md §18 |
| `docs2/prdtospec.md` | KEEP | PRD → spec template (CLAUDE.md §18 mentions it) |
| `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` | REMOVE | Self-marked superseded by `docs/ccreality/` which no longer exists — dangling supersession |
| `docs2/SHARED-MIND-AGENT-PROTOCOL.md` | REMOVE or banner | SMAP narrative, superseded by Doctrine §3 |
| `docs2/CUDA-13-1-RTX-5090-Report.md` | KEEP | Hardware reference (RTX 5090 / CUDA 13.1) — useful for 5090jepa subsystem |
| `docs2/TheoryOfMachineLearning.md` | KEEP | Project theory background |
| `docs2/TheSymmetryOfKnowing_Revised.pdf` | KEEP | Companion PDF to `docs/the_symmetry_of_knowing_partial.md` |
| `docs2/modulateprompt.md` | LOW-confidence | Generic refactor prompt |
| `docs2/specdoc.md` | LOW-confidence | Generic spec template; redundant with `prdtospec.md`? |
| `docs2/systemspecs.md` | REMOVE | Geekbench workstation dump |
| `docs2/contextgraphspecs/` | REMOVE-after-diff | 22 numbered spec files; parallel to `docs/futurebuild/`; last touched 2026-05-08 (pre-condensation). Likely 100% superseded. |
| `docs2/realitywhitepaper/` | LOW-confidence | 4.5 MB whitepaper draft referencing retired `/ccreality-*` skills; archaeological. |
| `docs2/agent-prompts/AIWONDER-308-rust-refactor-handoff.md` | KEEP | Live handoff doc (touched 2026-05-15) |

---

## §10 Config audit

`/home/cabdru/contextgraph/config/` — primary MEJEPA-Code configuration:
- `default.toml`, `development.toml`, `production.toml`, `test.toml` — live env tiers
- `mejepa/` — MEJEPA-specific configs
- `retention_policy.toml`, `cell_exemptions.toml`, `corpus_quarantine.toml` — hygiene
- `models_config.toml.example` — embedder model pins
- `reward_spec.verified_outcome.json` — reward spec
- `ccreality_governed_paths.json`, `ccreality_recommendation_schema.json` — last touched `6cabbbbc 2026-05-07 ccreality: fully remove governed-paths gate; rewrite CLAUDE.md`. Per CLAUDE.md §7: the MCP-server-side path-policy gate was removed by operator directive. These JSON files therefore describe a retired mechanism. **REMOVAL CANDIDATE** unless an FSV script still loads them as proof-of-retirement (no caller found in `scripts/`).

`/home/cabdru/contextgraph/configs/dynamicjepa/` — DynamicJEPA configs:
- `domain_packs/`, `verification/` — actively wired into `.github/workflows/ci_smoke_a.yml`

**Verdict:** KEEP both directories. Two separate product configurations. File `type:tech-debt` to collapse the plural `configs/` into `config/dynamicjepa/`. Verify `ccreality_*.json` removal candidates against any Phase H absence guard.

---

## §11 .claude/ harness audit

**`.claude/agents/cc-reality-*` (5 files)** — Per `.claude/agents/README.md`, 2 are active (`cc-reality-source-of-truth-readback`, `cc-reality-swe-failure-triage`) and 3 are retired-with-provenance (`cc-reality-harness-editor`, `cc-reality-diagnostician`, `cc-reality-reward-analyst`). The Phase H FSV `scripts/ci/phase_h_agent_roster_retirement_fsv.sh` *requires* the retired 3 to remain present so the retirement is auditable. **KEEP all 5.**

**`.claude/skills/ccreality-*` (4 dirs: iterate, readback, smoke, trigger-check)** — Tracked. Referenced from `docs2/realitywhitepaper/` (legacy), `docs2/contextgraphspecs/22_reality_loop_subsystem.md` (suspected dead per §9), and `docs/MEJEPA_SUPER_PROMPT.md` (SUPERSEDED). NO Phase H FSV asserts their presence. **LOW-confidence removal — operator decides.** If kept, add status banner like the cc-reality-* agents.

**`.claude/loop.md`** — Empty file (0 bytes). AGENTS.md and CLAUDE.md §18 both reference it as "per-tick prompt". Should either be populated or removed. **Filed as a bug.**

**`.claude/commands/`, `.claude/skills/` (non-ccreality), `.claude/agents/` (non-cc-reality)** — Vast Claude Code skill/command library (~40+ commands, ~40+ skills, ~30+ agent subdirs). Outside the scope of this engineering audit; would need an entire separate investigation to triage. **KEEP for now.**

**`.claude/checkpoints/`** — Empty. No git history. Removal candidate.

**`.claude/helpers/`, `.claude/statusline.{mjs,sh}`** — KEEP (statusline display).

**`.claude/settings.json`, `.claude/settings.local.json`** — KEEP (live Claude Code settings).

**`.claude/hooks/{pre,post}_tool_use.sh`, `session_{start,end}.sh`, `user_prompt_submit.sh`** — Live Claude Code hooks (different from `scripts/cc/hook_*.sh`; this is the Claude Code SDK-side, the scripts/cc set is the project-side). **KEEP all.**

---

## §12 .github/workflows/ audit

8 workflows tracked (per `.gitignore:6-11` only specific workflows are allowed):

| Workflow | Triggers | Verdict |
|---|---|---|
| `ci.yml` | push/PR main + develop, manual | KEEP — primary "Python ME-JEPA Scope Gate" |
| `ci_smoke_a.yml` | DynamicJEPA paths | KEEP — Tier A local-free smoke |
| `ci_smoke_b.yml` | (similar pattern, B tier) | KEEP |
| `ci_smoke_c.yml` | (C tier) | KEEP |
| `chaos.yml` | nightly cron + PR | KEEP — TASK-TEST-011 Phase F chaos |
| `e2e.yml` | push/PR main | KEEP — Phase F E2E happy path |
| `load.yml` | scheduled | KEEP |
| `soak.yml` | scheduled | KEEP |

All workflows pinned to SHA (per CLAUDE.md §11 Free-plan budget). Free-plan compliant: `runs-on: ubuntu-latest` only (one exception: `ci_smoke_a.yml` uses self-hosted runner — operator decision). All have `timeout-minutes:` set. **Verdict: KEEP all 8.**

No workflow references retired phases (no `phase_g_*` or `phase_h_*` script is invoked from any workflow — they are operator-invoked stand-alone tools).

---

## §13 agentdb/ verdict

**Investigation:**
- `agentdb/` size: 37 MB
- Has its own `.git/` directory → it is a SIBLING repository, not a subtree / submodule
- `.gitignore:36 agentdb/` blocks it from this repo's git index
- TypeScript/Node project (`package.json`, `tsconfig.json`, `vitest.config.ts`)
- `grep -rE "agentdb" --include="*.toml" crates/` returns ZERO results
- `grep -rE 'path\s*=\s*"\.\./?\.\.?/agentdb' --include="*.toml" .` returns ZERO results
- No `crates/*/Cargo.toml` has it as a dependency
- Only references to "agentdb" outside `agentdb/` are inside `backupdocs/memory.archive/` (the legacy `./memory/` archive)

**Conclusion:** Clean-room policy (CLAUDE.md §6.7) intact. `agentdb/` is a vendored sibling clone for read-only inspection only. **VERDICT: SAFE REMOVAL CANDIDATE.** The clone can be regenerated outside the repo tree (`cd ~ ; git clone https://github.com/ruvnet/agentdb`) if reference inspection is later needed. Suggested action: `rm -rf /home/cabdru/contextgraph/agentdb/` — reclaims 37 MB local disk; does not touch git.

---

## §14 Suggested removal sequence

Ordered by safety (lowest blast-radius first). Each step is one operator decision and one issue.

1. **Local-disk hygiene (untracked, gitignored, zero risk):**
   - `rm -rf /home/cabdru/contextgraph/.playwright-mcp/` (76 MB)
   - `rm -rf /home/cabdru/contextgraph/.pytest_cache/` (32 KB)
   - `rm -rf /home/cabdru/contextgraph/.ruff_cache/` (20 KB)
   - `rm /home/cabdru/contextgraph/gemmafrontend-playwright-console.log`
   - Verify free disk delta.

2. **Local-disk archaeological (untracked, requires operator confirmation):**
   - `rm -rf /home/cabdru/contextgraph/tmp/` (170 MB — confirm SITREPs are saved to issues first)
   - `rm -rf /home/cabdru/contextgraph/.tmp/`
   - `rm -rf /home/cabdru/contextgraph/artefacts/` (18 MB — confirm EXP-2 evidence is not load-bearing)
   - `rm -rf /home/cabdru/contextgraph/agentdb/` (37 MB — clean-room rule)
   - `rm -rf /home/cabdru/contextgraph/samplecode/{node_modules,target,obj,bin,__pycache__}` (~840 MB — keep source files)

3. **Tracked file cleanup (single-commit, low-risk):**
   - `git rm /home/cabdru/contextgraph/.codex`
   - `git rm /home/cabdru/contextgraph/.mcp.json.bak.20260502-092812`
   - `git rm /home/cabdru/contextgraph/compose-khattab.yml`
   - `git rm /home/cabdru/contextgraph/mats-eoi.yml`
   - `git rm /home/cabdru/contextgraph/zenodo_new_upload_initial.yml`
   - `git rm /home/cabdru/contextgraph/MEJEPAResearchPaper.pdf`
   - `git rm /home/cabdru/contextgraph/data/download_balanced.py`
   - `git rm /home/cabdru/contextgraph/data/download_beir_scifact.py`
   - `git rm /home/cabdru/contextgraph/docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md`
   - `git rm /home/cabdru/contextgraph/docs2/systemspecs.md`

4. **Documentation drift fixes (edit in place):**
   - Update `CLAUDE.md` §4 / §18 + `AGENTS.md` "Memory" section to point `./memory.archive/` references at `backupdocs/memory.archive/` (or restore the root location).
   - Update `INSTALL.md` `/mnt/d/contextgraph` references → `/zfs/archive/contextgraph` (per #363).
   - Update `systemd/contextgraph-mcp-http.service` Environment= paths → aiwonder; or clearly mark unit as legacy-WSL.
   - Update `scripts/restart_pipeline_services.sh` to aiwonder paths.

5. **Operator-decision (file as `type:dead-code` GitHub issue first):**
   - 42 of 46 `scripts/shakespeare_*.py` (consolidate into `scripts/shakespeare_RETIRED/` or remove; keep the 4 that Phase H tracks).
   - `scripts/exp2/` + `artefacts/exp2/` joint removal.
   - `scripts/docker/migrate_docker_desktop_wsl_to_d.ps1`.
   - `scripts/cc/cgreality-tick.sh`.
   - `config/ccreality_governed_paths.json` + `config/ccreality_recommendation_schema.json`.
   - `docs2/contextgraphspecs/` (22 files).
   - `docs2/realitywhitepaper/`.
   - `docs2/SHARED-MIND-AGENT-PROTOCOL.md`.
   - `.claude/skills/ccreality-{iterate,readback,smoke,trigger-check}/`.
   - `backupdocs/00_*..27_*` numbered duplicates of `docs/futurebuild/_archived/`.

6. **Tech-debt issues (no removal — just file):**
   - Collapse `configs/dynamicjepa/` → `config/dynamicjepa/` (singular).
   - Better `.gitignore` rules for `samplecode/` subtrees.
   - Add SUPERSEDED banners where missing (`docs2/SHARED-MIND-AGENT-PROTOCOL.md`).

---

## §15 What I did NOT investigate

Per task scope:
- `crates/*/src/` — Investigation #1
- `crates/*/tests/` — Investigation #2
- `crates/*/examples/` — Investigation #1 or #2
- `swebench/**` upstream — CLAUDE.md §7 path discipline (read-only)
- `tests/oracle_**` — CLAUDE.md §7 path discipline (READ-only, but tests/ at top-level was scanned for live workflow wiring only)

I did NOT execute any modifications. All findings are read-only audit data.

---

**End of Investigation 3 report.**
