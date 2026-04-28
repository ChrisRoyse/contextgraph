---
namespace: patterns
agent_id: agent-41-abstract-writer
timestamp: 2026-04-16T23:45:00Z
status: active
locked: false
supersedes: none
tags: abstract, frontmatter, hedge-audit, banned-phrase-audit, arxiv, agent-41
depends_on:
  - memory/context/mission-brief.md
  - memory/context/constraints.md
  - memory/decisions/agent-04-construct-definer--dda-definition.md
  - memory/decisions/agent-04-construct-definer--meaning-compression-definition.md
  - memory/decisions/agent-04-construct-definer--tct-definition.md
  - memory/decisions/agent-04-construct-definer--identity-locked-definition.md
  - memory/patterns/agent-34-confidence-quantifier--confidence-language-for-writers.md
  - memory/handoff/agent-40-to-41--conclusion-ready.md
downstream_consumers:
  - agent-42-adversarial-reviewer
---

# Abstract Writing Decisions (Agent 41)

## 0. File produced

- `../sections/00_frontmatter.md` — title (H1), author line (Chris Royse),
  affiliation line (Teleox.ai · Leapable.ai), contact, arXiv category tags,
  keyword list, and the single-paragraph abstract.
- Abstract paragraph word count: **298 words** (target 200–300).
- Single paragraph, no subheadings inside. arXiv-style.
- No citations in the abstract (arXiv convention respected).
- No code blocks; inline LaTeX math only ($n \cdot (N + \binom{N}{2})$, $G_\tau$,
  $n_m$) at the level standard for arXiv cs.LG abstracts.

## 1. Structural decisions

1. **Open with the problem, not the solution.** Sentence 1 sets the three
   contextual anchors (data-wall ceiling, collapse theorems, licensing
   economics) in a single compound clause. Sentence 2 poses the research
   question and names the three constructs.
2. **Bold only the three locked construct names on first use** (DDA, meaning
   compression, TCT). This preserves the glossary hierarchy without
   visually overstating derivative phrases.
3. **Systems sentence uses semicolons, not commas**, between system facts to
   avoid list ambiguity with the numeric figures ("13 embedders", "59
   RocksDB column families").
4. **One MEASURED result, bold, scope-disclosed inline**: "0.961 mean WavLM
   SECS (max 0.975) on 10 held-out English sentences for one speaker under
   an encoder-matched protocol." Same-sentence scope gate eliminates C5
   ambiguity without a separate disclosure clause.
5. **Projected cases flagged in one sentence** with "camera-ready
   re-verification flagged" — the exact clause C11 (Case 1) and the Case 2
   T3 projection specification (agent-34 §C9) both require.
6. **Collapse scope claim as parenthetical** — "(scope, not refutation)" —
   so that reviewers cannot read the abstract as a Shumailov-immunity
   theorem (C4 forbidden-verb protection).
7. **Open problems named as a three-item list**, each a short noun phrase
   (cross-encoder SECS triangulation, pairwise MI audit, $n_m$ sweep) — no
   verb commits the paper to outcomes.

## 2. Per-claim hedge audit (agent-34 compliance)

| Claim | Template | Hedge applied in abstract |
|---|---|---|
| C1 — counting identity | "up to $n \cdot (N + \binom{N}{2})$" and "up to 91" / "up to 28" | Both "up to" prefixes present; "constructive upper bound"; "tightness is pending a pairwise mutual-information audit" |
| C2 — fourth taxonomic entry | "proposed as a fourth taxonomic entry"; "semantically distinct in unit" | Present; uses "semantically distinct" (not "dimensionally distinct"); "rather than superior to them" softens the orthogonality claim |
| C3 — per-output verifiability | "per-output verifiability relative to the chosen centroid panel" | Present, exact phrasing from C3 template; "deterministic geometric test" |
| C4 — DDA collapse-silence scope | "DDA pipelines are structurally outside the Shumailov-form recursion regime (scope, not refutation)" | Present; "structurally outside" = template-compliant; "(scope, not refutation)" added to block overreading |
| C5 — 0.961 SECS encoder-matched | "0.961 mean WavLM SECS (max 0.975) on 10 held-out English sentences for one speaker under an encoder-matched protocol" | Present; encoder-matched disclosure is in-sentence, not forward-referenced. The max value 0.975 is included for completeness. |
| C6 — DNSMOS parity | "DNSMOS P.808 parity (3.93 / 3.93) and UTMOS parity (3.012 / 2.997) on a 14-sentence set" | Present; no "statistically equivalent" / "indistinguishable from real speech" language; scope ($N=14$) included inline |
| C11 — Case 1 projected | "Case 1 (Shakespeare stylistic LoRA) … projected, with camera-ready re-verification flagged" | Present; does not claim 15/15 FSV on cleaned retrain |
| C9 — Case 2 projected | "Case 2 (identity-locked talking-head clone) … projected, with camera-ready re-verification flagged" | Present |
| C14 — no marketing | No product pitches; no revenue/pricing; no "revolutionary" / "breakthrough" / "best"; Teleox.ai and Leapable.ai only in the author affiliation line | Verified clean |

## 3. Banned-phrase audit (agent-34 §4)

Ran a literal grep against the abstract paragraph for every entry in
agent-34 §4's banned-phrase table. Matches: **zero**.

Specifically searched-and-confirmed-absent:

- "91 signals per input" (bare) — present only as "up to 91 signals per
  input".
- "dimensionally distinct" — absent; replaced by "semantically distinct in
  unit".
- "0.961 mean SECS" (bare) — present only with the encoder-matched
  scope gate in the same sentence.
- "indistinguishable from real speech" — absent; DNSMOS/UTMOS rendered as
  "parity" with $N=14$ inline.
- "solves the data wall" — absent; the word "third path" from the title is
  a reframing claim, not a solution claim.
- "outperforms RLHF / DPO / CAI" — absent; scalar-reward alignment is not
  named in the abstract at all, by design (saves words and sidesteps
  benchmark-competition framing).
- "breakthrough" / "revolutionary" / "game-changing" — absent.
- "solves AGI" / AGI claim — absent.
- "the best" — absent.
- "T=0.3 is optimal" — absent; $T=0.3$ is not mentioned in the abstract at
  all (details live in §6.3).
- "proven collapse-free" — absent; "structurally outside" used instead.
- "we demonstrate the sample-complexity collapse" — absent; the abstract
  does not even state Theorem 1 to avoid needing the C12a/C12b split in
  abstract space.
- "is necessary" / "must" / "cannot be achieved without" — absent.

## 4. Word-economy decisions (300-word cap)

Four drafting passes were required to land inside 300 words:

- Pass 1: 324 words. Over by 24.
- Pass 2: 318. Replaced "hundreds of millions of dollars" → "hundreds of
  millions"; removed "both remain inside the collapse recursion" duplicate.
- Pass 3: 308. Replaced "Two production systems instantiate the substrate —
  Context Graph (a 370,000-line Rust workspace with 13 embedders, …)" with
  semicolon-separated figures; replaced "roughly" with "≈".
- Pass 4: 302. Replaced "approached the practical ceiling of high-quality
  public text" with "approached the ceiling of high-quality public text";
  replaced "we name three interlocking constructs" with "and name three
  constructs".
- Pass 5: 298. Tightened closing sentences: "— a scope argument, not a
  refutation." → "(scope, not refutation)."; "with camera-ready
  re-verification is flagged" → "with camera-ready re-verification
  flagged"; "Three open problems remain: …" → "Open problems: …".

Load-bearing phrases that were NOT cut under word pressure:

- "approximately independently trained" (C1 / V11 axiom 2)
- "constructive upper bound"
- "pending a pairwise mutual-information audit"
- "semantically distinct in unit" (C2 MUST, not "dimensionally distinct")
- "relative to the chosen centroid panel" (C3 MUST)
- "under an encoder-matched protocol" (C5 MUST)
- "on 10 held-out English sentences for one speaker" (C5 scope)
- "scope, not refutation" (C4)
- "camera-ready re-verification flagged" (C9, C11)

## 5. What the abstract deliberately does NOT include

- No explicit statement of Theorem 1 or Theorem 2. The C12a/C12b
  constructive-vs-empirical split cannot be rendered at abstract word
  density without losing the hedge discipline; the theorems are referenced
  implicitly via "constructive upper bound" and "per-output verifiability".
- No citations. arXiv convention; references live in §2 and the
  bibliography.
- No acronym definitions beyond "(DDA)", "(TCT)" on first use. "LoRA",
  "MCP", "DAG", "WavLM", "SECS", "DNSMOS", "UTMOS", "ECAPA-TDNN" are left
  undefined in the abstract per cs.LG convention; §6 defines them.
- No $T=0.3$, no best-of-12, no 50-clip centroid. These live in §6.3.
- No mention of Leapable.ai's 161-tool marketplace or Stripe economics.
  Leapable is named only in the author affiliation line per C14.
- No Acknowledgements / Competing-Interests sentence — that will be a
  separate section per the mission brief.

## 6. Title decision

The title was provided verbatim by the mission brief and reproduced
without modification. No changes proposed.

## 7. Keyword decision

Keywords selected to maximise arXiv cs.LG retrieval coverage across the
paper's three construct domains:

- **Multi-embedder retrieval** — DDA substrate vocabulary.
- **Identity-locked generation** — TCT outcome vocabulary.
- **Synthetic data / model collapse / scaling laws** — §2 cluster-A
  related-work hooks.
- **Voice cloning / speaker verification / WavLM** — Case 3 measurement
  hooks.
- **Frozen centroids / LoRA** — method-layer retrieval hooks.

Ten keywords total; arXiv allows a free-form keyword line.

## 8. Confidence

**HIGH** that the abstract stays inside the 200–300-word target (298).

**HIGH** that the banned-phrase audit against agent-34 §4 returns zero
matches.

**HIGH** that every per-claim hedge from agent-34 (C1, C2, C3, C4, C5, C6,
C9, C11, C14) is present in the required form.

**HIGH** that the abstract reports one MEASURED number with full scope
disclosure (encoder-matched, $N=10$, one speaker, one language) and two
projected cases flagged as such.

**MEDIUM** on adversarial-review exposure. Likely review vectors are: (i)
whether "ceiling of high-quality public text" reads too strong without
"practical" (decision: kept out for word count; Villalobos et al.
hedging lives in §1); (ii) whether the "third path" framing in the title
can be misread as a solve claim (protected by "(scope, not refutation)" in
the abstract body and by §9's "reframes the data wall as three
complementary paths"); (iii) whether the projected/measured split is
adequately clear in abstract space (protected by the explicit "measured
at … encoder-matched" for Case 3 and "projected, with camera-ready
re-verification flagged" for Cases 1 and 2).

## Changelog

- 2026-04-16 — Initial issue by agent-41-abstract-writer.
