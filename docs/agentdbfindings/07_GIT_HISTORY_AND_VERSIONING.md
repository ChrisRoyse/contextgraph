# 07 — Git History & Versioning

You asked me to review git history "for the last 2 days" specifically. Here's what I found and what it tells us about the stability of any code you'd port.

---

## The full git history

**The agentdb repo at `https://github.com/ruvnet/agentdb` was init'd 2026-05-06.** Every commit in its history was made that day. There is no "history" prior to two days ago — the entire git log is:

```
a478ab3 2026-05-06 11:27:41 -0400  fix(plugins): repository must be string per Claude Code plugin schema (not npm-style object)
e8a3a80 2026-05-06 11:22:44 -0400  feat: Claude Code marketplace + 6 plugins + npm SEO bump
43d1cc3 2026-05-06 11:11:41 -0400  docs: rewrite README in ruflo style — concise, opinionated, scannable
65b79de 2026-05-06 11:05:12 -0400  chore: organize root — move loose scripts/dockerfiles into subfolders
a74a6b6 2026-05-06 11:02:44 -0400  chore: clean up repo root — archive stale release notes, drop tarballs
3e4a349 2026-05-06 10:59:07 -0400  docs: update inline GitHub link to new ruvnet/agentdb URL
8b3388b 2026-05-06 10:56:18 -0400  init: agentdb package source + marketing UI
```

Seven commits. All within 31 minutes. The "init" commit (`8b3388b`) is the one that actually populated the repo with the source tree.

---

## What that tells us

This is **not** a freshly written project. It's a *freshly published* project — the actual source has been developed somewhere else (probably a private monorepo at `ruvnet/`), and the public `agentdb` repo was carved out of it on 2026-05-06 for npm publishing and Claude Code plugin distribution.

Evidence:

- **CHANGELOG.md** documents versions back to 2025-10-10 (v1.0.7 with browser bundle), through v1.6.0 (2025-10-25), v2.0.0-alpha.2.16 (2025-12-03), and the README claims `3.0.0-alpha.14` (which `package.json` confirms).
- The CHANGELOG references migration guides for v1.2.2, v1.3.0, v1.6.0 — none of which exist in this git history because this git history doesn't go back that far.
- `docs/adrs/` has ADRs dated 2026-02-17 (ADR-006), 2026-02-20 (ADR-010) — written months before the public repo was created.
- The 5,921 LOC across `src/backends/rvf/*` and 13,899 LOC across `src/controllers/*` were not authored in 31 minutes.

So the *real* development history of agentdb is somewhere else and is not visible to us here. What we have is essentially a published npm package with a single squashed initial commit + 6 polish commits.

---

## What this means for adopting agentdb code

### 1. **You can't "git blame" any decision in this repo.**

If you want to know why `efSearch` defaults to 100 in `RvfBackend.ts`, or why `SolverBandit`'s `costWeight` is `0.01`, the answer is "because the source tree says so" — there's no commit history showing the iteration that landed on those numbers. You're getting the *outcome*, not the *reasoning*.

**Implication:** treat agentdb's defaults as "plausible starting points," not "battle-tested values." Re-tune for your own workload.

### 2. **You can't see what just changed before the fork.**

The interesting question for a downstream consumer is "what did they fix in the last 2 weeks?" Normally you'd `git log --since='2 weeks ago' -- src/backends/rvf/` to find recent stability fixes. Here, that returns the same 7 commits — the source-tree authors squashed everything before the publish.

**Implication:** the v3.0.0-alpha.14 publication is bleeding-edge. If a critical bug was fixed 3 weeks before publication, you can't see the fix; you only see the post-fix state. If you want stability, lean on the npm distribution itself — the published npm `agentdb@3.0.0-alpha.14` tarball is what they want consumers running.

### 3. **CHANGELOG-claimed versions don't match published versions.**

| What CHANGELOG says | What package.json says |
|---------------------|------------------------|
| Most recent version: `2.0.0-alpha.2.16` (2025-12-03) | `3.0.0-alpha.14` |
| v1.7.0 listed as "Planned" with QUIC | QUIC files exist in `src/controllers/QUICServer.ts` (498 LOC) and `QUICClient.ts` (667 LOC) |
| v2.0.0 listed as "Future" with HNSW + Hybrid + Quantization | All of those exist and ship today |

**Implication:** CHANGELOG.md is stale documentation. Don't use it to scope what's available; read the source tree. Specifically `src/index.ts` exports — that's the real surface.

### 4. **The "six plugins" e8a3a80 commit added Claude Code marketplace integration.**

The commit `e8a3a80 feat: Claude Code marketplace + 6 plugins + npm SEO bump` is interesting because *that's marketing positioning, not a code feature*. They want agentdb to be discoverable as a Claude Code plugin. The `.claude-plugin/` dir was added then.

**Implication:** there's an active commercial-positioning push. Expect more "MCP/Claude/plugin" pivots; the technical core is stable but the packaging will keep moving.

### 5. **The `a478ab3` fix (latest commit) is a one-liner schema fix.**

Plugin manifest's `repository` field needed to be a string per Claude Code plugin schema, not an npm-style object. Tells us: the plugin distribution is brand new (broke on first try), and the maintainer is actively responsive (fixed within 5 minutes).

---

## Versioning context

The published version is `3.0.0-alpha.14` per `package.json:3`. CHANGELOG never reaches `3.x`, so the alpha numbering is opaque. From the dependency tree:

```
@ruvector/sona       0.1.4   (Dec 2025 federated-learning release per CHANGELOG)
@ruvector/gnn        0.1.23  (recent)
@ruvector/rvf-solver 0.1.7   (ADR-010 covers v0.1.6 → v0.1.7 upgrade rationale)
@ruvector/attention  0.1.2
ruvector             0.1.30
```

These are the source-of-truth packages. agentdb is the consumer; it tracks `@ruvector/*` major-version-zero releases. **All of agentdb's "AGI"-flavored capabilities are functions of the `@ruvector/*` versions it pins.** Pinning is loose (`^0.1.x`), so a fresh `npm install` could land on different patch versions than the maintainer tested.

**Implication:** if you do depend on `@ruvector/*` packages from a sibling Node project (which is rare in the ContextGraph workspace anyway), pin exact versions and snapshot the lockfile. Don't trust `^0.1.x` to be reproducible.

---

## What the 31-minute commit burst suggests about code quality

I read enough source to form an opinion: the code is **competent but not production-hardened**.

Pros:
- TypeScript strict mode is on; types are mostly clean.
- Bounds checking is everywhere (`MAX_DIMENSION`, `MAX_HIDDEN_DIM`, `MAX_INTENTS`, etc.) — they took the security review seriously.
- Lazy-loading optional deps with try/catch fallback is consistent throughout.
- ADR docs are thoughtful (ADR-006 in particular is the right level of detail).

Cons:
- Several controllers are very large (1391 LOC `ReflexionMemory`, 1287 LOC `LearningSystem`, 1020 LOC `AttentionService`) — past the "split this file" threshold most teams use. Your CLAUDE.md §4 says "Files under 500 lines."
- "Graceful fallback when native dep missing" is the default everywhere. That makes for nice DX, but it hides bugs (a missing-dep failure becomes a silent perf regression).
- A bunch of "feature flag default false" research-grade features (HyperbolicAttention, GraphRoPE, FlashConsolidation) ship in the production path. Untested in the typical config.
- Test coverage exists but isn't measurable from here without running it. The CHANGELOG says things like "38/38 landing page features verified (100%)" which is a slightly weird metric (landing-page features?).

**Implication for porting:** rewrite, don't transliterate. Take the algorithm + the schema; leave the structure. Your Rust ports should be 1/3 to 1/2 the LOC of the TS originals.

---

## Should we re-pull periodically?

**Yes, but selectively.** The interesting questions to track:

| Track | Why | How |
|-------|-----|-----|
| `src/backends/rvf/` LOC growth | Self-learning is the active workstream | `wc -l src/backends/rvf/*.ts` over time |
| New ADRs | The ADR cadence shows where the architects are spending effort | `ls docs/adrs/` |
| `@ruvector/*` version bumps | Their core algorithmic changes | `package.json` diff |
| README capability table | Their pitch | `git log -- README.md` |

Don't re-clone. Either pull updates with `git -C agentdb pull`, or for clean state, replace the directory and re-clone — tracking via `agentdb/.git` against the upstream is fine.

---

## Final note on the 2-day-history question

The literal interpretation of "review git history for last 2 days" is "show me the diff." The actual answer — "this repo only has 2 days of history because it's a fresh re-publication of a private monorepo" — is the more interesting finding. There's nothing newer-than-old in this tree to compare; everything in the source tree is "as of 2026-05-06 11:27 EDT."

If you want to track ongoing development, watch `https://github.com/ruvnet/agentdb/commits/main` going forward. Anything between today and now is genuine new work.
