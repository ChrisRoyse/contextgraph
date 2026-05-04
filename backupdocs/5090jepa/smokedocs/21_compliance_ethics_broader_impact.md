# 21 - Compliance, Ethics, And Broader Impact

## Goal

Map the smoke program to the documentation, governance, and ethics expectations
that apply to any modern ML release: NIST AI RMF (incl. the GenAI profile and
the April 2026 critical-infrastructure profile concept note), the NeurIPS
Ethics Guidelines, the O*NET license + non-endorsement requirements, and the
dual-use considerations that apply to anything that touches occupational
predictions. The paper does **not** make recruiting claims, but the broader
impact section must still address what the artifact could be misused for.

## Scope (What This Doc Is Not)

- Not a privacy gate for licensed real-data work (that is doc 25).
- Not legal advice; the operator is responsible for jurisdictional review.
- Not a hiring-decision impact assessment; the paper explicitly excludes that
  use case (smokedoc 01 §Claims We Must Not Make).

## NIST AI RMF Mapping (For Paper + Artifact)

Map our artifact to the four AI RMF Core functions. Each entry below names the
artifact location that satisfies the function.

### GOVERN

| Sub-category | Evidence |
| --- | --- |
| Policies, processes, procedures documented | smokedocs 00-25, CLAUDE.md repo policies |
| Accountability for risk | repo CODEOWNERS file + maintainer named in CITATION.cff |
| Workforce trained on ethics | n/a (single-author research artifact); state explicitly |
| Inclusion of diverse perspectives | external review during artifact-evaluation; state explicitly |

### MAP

| Sub-category | Evidence |
| --- | --- |
| Context, sector, intended/foreseeable uses | this doc §Intended Use + §Misuse Risks |
| Categorization of system | "research demonstrator, public-taxonomy benchmark" — not "production decision-making system" |
| Risks identified | this doc §Risk Register |
| Third-party data/software risk | doc 20 §License + Attribution; dependency lockfiles |

### MEASURE

| Sub-category | Evidence |
| --- | --- |
| Metrics for trustworthiness characteristics | smokedoc 07 (accuracy, calibration, collapse), doc 22 (HP sensitivity) |
| Documentation of performance | every bundle's `paper_tables/`; multi-seed aggregate (doc 15) |
| Independent verification | FSV protocol (smokedoc 08), artifact-evaluation badging (doc 20) |

### MANAGE

| Sub-category | Evidence |
| --- | --- |
| Risk responses prioritized | this doc §Risk Register order |
| Risk treatment | failure-mode tests, fail-closed defaults, explicit non-claims |
| Continuous monitoring | doc 23 CI smoke regression + drift detection |
| Incident disclosure | failed-bundle preservation policy (smokedoc 11) + worked example (seed-43) |

This mapping is condensed into a one-page table in the paper's appendix F per
doc 19.

## Intended Use

Permitted:

- Research and educational replication of the system, the FSV protocol, and
  the public-taxonomy benchmark.
- Use as a reference architecture for auditable latent world-model runtimes in
  other domains (counter_world / gridworld / new domain packs).
- Comparison baseline for future career-taxonomy benchmarks.

Not permitted (and not supported by the evidence):

- Real recruiting / hiring decisions.
- Career counseling tools that present predictions to end users without
  explicit "research demonstrator" disclosure.
- Compliance / labor-market regulatory reporting.
- Any use that relies on the public-taxonomy benchmark as a proxy for
  individual outcomes.

The README, paper, and DOI page all repeat this list verbatim.

## Risk Register

Order: highest residual risk first. "Residual" = after the listed mitigations.

| Risk | Severity | Likelihood | Mitigation | Residual |
| --- | --- | --- | --- | --- |
| Misuse for hiring decisions | High | Low (small + disclaimed) | Explicit non-claim; intended-use list; arXiv abstract qualifier | Low |
| Career counseling deployment | Medium | Medium | License terms + README; no decoder for individual-level predictions | Low-Medium |
| Cherry-picking seeds | Medium | Low | Multi-seed BCa + paired tests required for any "significantly" claim | Low |
| Silent metric drift in container | Medium | Low | CI weekly reference-hash check (doc 23) | Low |
| O*NET attribution gap | Low | Medium | Attribution in README, paper, manifest, container build log | Low |
| Dual-use as labor-market surveillance | Medium | Low | Scope is taxonomy walks, not real subjects; no individual-level path | Low |
| Reviewer cannot reproduce | Medium | Low | Apptainer + Docker + reference hash + diff report (doc 20) | Low |
| Reproducibility breaks on non-sm_120 | Low | Medium | Document tested archs; provide expected metric variance | Low |

## Broader Impact (For Paper §Discussion / NeurIPS Checklist)

Three paragraphs:

### Paragraph 1 — Positive

Auditable world-model runtimes lower the bar for verifying ML systems claims.
The smoke program demonstrates that domain-pack abstraction + source-of-truth
verification is feasible on commodity high-end hardware. Other research groups
can reuse the runtime to add domains they care about without porting auditing
infrastructure.

### Paragraph 2 — Risks

Career-domain models are sensitive. Even a public-taxonomy demonstrator could
be repurposed to imply predictive validity it does not have. This is mitigated
by (a) disclaiming top-k accuracy until the decoder head is present and
calibrated, (b) requiring multi-seed paired tests for any "significantly"
claim, (c) refusing to release a model trained on real subject-level data
under this artifact (deferred to Phase 12, doc 25), and (d) maintaining
explicit non-claims in the abstract.

### Paragraph 3 — Mitigation Path

The artifact has a fail-closed default. The intended-use list is enforced by
documentation, license terms, and the readiness of the FSV protocol to expose
quality regressions immediately. We commit to maintaining the CI regression
(doc 23) and to publishing any Phase 12 privacy gate (doc 25) on the same
public DOI before any real-data extension.

## Dual-Use Disclosure (NeurIPS Ethics Guidelines)

Required because the system touches occupational predictions. The disclosure
covers:

- **Surveillance**: career-trajectory inference at population scale could be
  used for labor-market surveillance. Our artifact uses public taxonomies, not
  subject-level data; the public-taxonomy benchmark cannot be retrained on
  subject data without violating the artifact license + Phase 12 gate.
- **Deceptive use**: a re-skinned tool could present model outputs as career
  recommendations without the disclaimers. The disclaimers are required in
  the README and in any paper or downstream that cites the work.
- **Bias amplification**: O*NET / ESCO are constructed taxonomies and have
  their own gaps (e.g. emerging occupations underrepresented). The paper
  reports per-cluster top-k accuracy so under-represented clusters are
  visible, not hidden behind macro accuracy.

## O*NET License + Non-Endorsement Compliance

Concrete requirements (already partially in place):

| Requirement | Where Enforced |
| --- | --- |
| O*NET attribution text in artifact | README, paper, `data_manifest.json` |
| Non-endorsement clause verbatim | README, paper, container build log |
| Source ZIP SHA-256 recorded | `data_manifest.json`, doc 13 |
| Allowed-use field set | `data_manifest.json`, currently `research-demo` |
| No suggestion that O*NET endorses the work | review of README + paper before submission |

Add a `LICENSE_ATTRIBUTION.md` at repo root that contains the verbatim O*NET
language. Doc 18 adds equivalent ESCO and BLS-crosswalk language.

## Bias And Fairness Surface

The paper does **not** claim a fairness contribution. It does report:

- per-cluster top-k accuracy in the supplementary (doc 16);
- per-cluster sample size in `table_walk_coverage.csv`;
- explicit disclosure that O*NET cluster representation reflects U.S.
  occupation taxonomies and is not globally representative.

This is sufficient for paper-quality ethical disclosure. A real fairness
contribution requires a different study design and is out of scope.

## What The Paper Does Not Promise

Restate verbatim in the manuscript and the README:

- No recruiting claim.
- No prediction validity for individual humans.
- No production deployment guarantees.
- No compliance claim with any specific labor regulation.
- No fairness claim across protected classes.
- No Phase 12 privacy gate evidence (deferred).

## Acceptance

This work is done when:

- the AI RMF mapping table is filled in with bundle paths;
- the risk register has owners and dates;
- the broader impact paragraphs are drafted to fit the paper §Discussion;
- the dual-use disclosure is in the NeurIPS checklist;
- `LICENSE_ATTRIBUTION.md` exists at repo root and contains all third-party
  attributions verbatim;
- doc 11 §Risks is updated with cross-references to this doc.
