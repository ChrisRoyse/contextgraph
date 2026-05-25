# POLIS — Mathematical Specification (Single North-Star Document)

> **Single source of truth for the entire build.** The fresh repo's `POLIS.md`. The AI loops until `§10.BUILD_DONE` evaluates `TRUE`. Reload this file on every context compaction. This is the only doc that defines the goal; everything else the implementing AI may design, plan, and produce.

---

## 0. One-line goal

```
∃ deployment D over a city C of |C| ≥ 50,000 residents, such that
   BUILD_DONE(D) = TRUE  ∧  ∀ I ∈ Invariants(§8) : I(D) = TRUE
   ∧  D is live-in-production, can onboard ≥ 100 citizens end-to-end.
```

`BUILD_DONE` is fully mechanical (§10). When `TRUE`, the implementing AI halts and reports.

Post-launch civic-outcome targets (§9) are *not* the AI's build gate — they are the launched cooperative's responsibility, mathematically defined here for forward reference.

---

## 1. Notation

| Symbol | Meaning |
|---|---|
| `C` | set of citizens of a polis, `c ∈ C` |
| `V` | set of partner venues, `v ∈ V` |
| `F = {SYM, AGO, GYM, THE, FES, ALE}` | gathering formats (§5) |
| `G ⊂ C × F × V × Time` | set of gatherings |
| `Φ(c) ∈ Π_i ℝ^{d_i}` | Constellation of citizen `c` (§2.2): tuple of per-slot vectors |
| `S = {E_1, ..., E_n}` | embedder slate (§3) |
| `τ` | threshold (subscripted by context, e.g. `τ_distinct`, `τ_G`) |
| `U(c, g)` | utility of gathering `g` for citizen `c` (§4.1) |
| `G_τ(g) ∈ {0,1}` | Constellation guard predicate (§4.2) |
| `y_{c,g} ∈ {0,1}` | binary outcome: strong tie formed by `c` at `g` within 30d |
| `Π_{stratified}(·)` | stratified sortition operator (§6.2) |
| `ε`, `δ` | differential privacy parameters |
| `H(·)`, `I(·;·)` | Shannon entropy, mutual information |
| `corr(X,Y) ∈ [-1,1]` | Pearson correlation; `corr_+` = `|corr|` |
| `∧ ∨ ¬ ⇒ ⇔ ∀ ∃ ∈ ∉` | logic |
| `RFC2119: MUST / MUST NOT / SHOULD / MAY` | normative force |

---

## 2. Mathematical objects

### 2.1 Citizen

`c = (id_c, PoP_c, Φ_c, Σ_c, A_c)` where:
- `id_c ∈ Identifiers` — opaque per-polis identifier
- `PoP_c ∈ {Worldcoin, InPersonAttestation, GovID-KYC}` — exactly one Proof-of-Personhood credential, ZK-attested unique-per-human
- `Φ_c ∈ Π_i ℝ^{d_i}` — Constellation (§2.2), frozen at end-of-onboarding, refreshed ≤ 1×/quarter
- `Σ_c = (consent_flags, hard_constraints, lifecycle_stage)` — structured fields; `lifecycle_stage ∈ {Newcomer, Established, Elder, Inactive, Departed}`
- `A_c ⊂ G` — attendance history, append-only

Citizenship is per-polis. `∀c : |{polis(c)}| ≤ 1` at any time; move requires 60-day delay.

### 2.2 Constellation

```
Φ(c) := (E_1(input_c), E_2(input_c), ..., E_n(input_c))
```

Type: ordered tuple. **MUST NOT** be flattened into a single concatenated vector for similarity, distance, or any predictor input (Invariant I10).

Permitted ops:
- per-slot cosine `cos(E_i(c_1), E_i(c_2))` ✓
- slot-identity-preserving projection ✓
- scalar pairwise cross-slot relationship (MI, correlation) ✓

Forbidden ops:
- `flatten(Φ) → ℝ^{Σ d_i}` then cosine ✗
- cross-slot dim-by-dim compare `E_i[k]` vs `E_j[k]` ✗

### 2.3 Gathering

`g = (id_g, format_g ∈ F, venue_g ∈ V, time_g, attendees_g ⊂ C, convener_g ∈ C, leitmotif_g, outcomes_g)`

Cardinality by format (§5):

| Format | min attendees | max attendees | duration |
|---|---|---|---|
| SYM Symposium | 5 | 7 | 2.5h |
| AGO Agora | 12 | 25 | 1.5h |
| GYM Gymnasium | 8 | 15 | 1–2h |
| THE Theater | 10 | 25 | 2h |
| FES Festival | 100 | 500 | 1–3d |
| ALE Aleatory | 4 | 6 | 1.5h |

### 2.4 Polis

`P = (C, V, governance_state, fiscal_state, constellation_state, gathering_history)`

A polis is one city instance. The system is the union of polises plus central cooperative services (§6).

---

## 3. Embedder slate (rethought for high-signal differentiation)

### 3.1 Definitions

A slate `S = {E_1, ..., E_n}` is **differentiated** iff all three hold:

```
(D1) Distinctness:     ∀ i ≠ j : corr_+(E_i(X), E_j(X)) ≤ τ_distinct,  τ_distinct = 0.6
                       computed on calibration set X ⊂ C, |X| ≥ 500

(D2) Marginal info:    ∀ i : H(y | E_1,...,E_{i-1}) − H(y | E_1,...,E_i) ≥ τ_marginal
                       τ_marginal = 0.05 bits
                       y = binary tie-formation outcome at 30d

(D3) Frozen:           ∀ i, ∀ phase ∈ {match, train, eval, serve} :
                       ∂E_i/∂θ_i = 0
                       (TCT axiom; ContextGraph §6.12 inheritance)
```

A slot that violates D1/D2 at quarterly audit **MUST** be retired or rebuilt. The slate is **MUST NOT** ship without all three holding (Invariant I19).

### 3.2 Why differentiation matters (the user's insight, mathematized)

Information added by slot `i` is `ΔH_i = I(y; E_i | E_1,...,E_{i-1})`. If `corr_+(E_i, E_j) ≈ 1` for some `j < i`, then `ΔH_i ≈ 0` → the slot is noise per token. The slate's value is `Σ_i ΔH_i`, not `Σ_i H(E_i)`. Maximize differentiated information per slot, not raw dimensionality.

Operational corollary: embedders MUST measure orthogonal *dimensions* of the matching signal. Same-domain embedders trained on different corpora (per the user's example: causal embedders trained on news / medical / legal / psychology) are valid IFF D1 holds across them.

### 3.3 The 21-slot slate

Organized into 11 orthogonal axes. Each axis is a distinct dimension of human-matching signal. Slots within an axis share moderate correlation by design (they measure neighboring aspects); cross-axis pairs **MUST** satisfy D1.

| Axis | Slot | Source / training corpus | Dim | Purpose |
|---|---|---|---|---|
| **A. Semantic content** | S1 `sem-self` | e5-large-v2 / instructor-XL on intake answers | 1024 | What citizen explicitly cares about |
| | S2 `lex-vocab` | SPLADE-v3 sparse on all stated text | 30k sparse | Lexical fingerprint, vocab register |
| **B. Causal direction (want)** | S3 `want-cause` | asym Nomic-causal, "what would you like more of" | 768 cause / 768 effect | What they want to be caused; producer-axis |
| | S4 `give-cause` | asym Nomic-causal on past contribution statements | 768 / 768 | What they have caused for others; pattern of giving |
| **C. Narrative form** | S5 `story-shape` | T5/BART narrativeQA fine-tune → classifier-embedding | 256 | Story-structure they use to describe their own life |
| | S6 `hero-arc` | classifier (quest/redemption/becoming/exile/return) → embed | 64 | Mythic shape preference |
| **D. Cognitive mode** | S7 `reason-mode` | classifier (analytic/intuitive/narrative/mathematical/mythic) | 128 | HOW they reason |
| | S8 `abstract-concrete` | scalar classifier embedded | 32 | Abstraction-level preference |
| **E. Moral foundations** | S9 `mfq-vec` | classifier on Haidt's MFQ (care/fairness/loyalty/authority/sanctity/liberty) | 96 | Moral-vocabulary distribution |
| **F. Ideological bridging** | S10 `bridge-vec` | Ovadya bridging-rerank embedding | 256 | Capacity for cross-cluster reach |
| **G. Somatic / prosody** | S11 `voice-tempo` | speech-rate, pause-rate from intake audio | 32 | Conversational pace |
| | S12 `voice-affect` | emotional-prosody (Wav2Vec emotion head) | 64 | Affective baseline |
| | S13 `energy-band` | extraversion proxy: audio + behavioral signals | 32 | Energy-band for group-size matching |
| **H. Temporal rhythm** | S14 `day-rhythm` | Fourier on stated availability + attendance | 32 | Circadian compatibility |
| | S15 `week-rhythm` | Fourier on weekly pattern | 32 | Week-shape compatibility |
| **I. Aesthetic field** | S16 `visual-aest` | SigLIP-2 on 5 citizen-curated images | 768 | Visual aesthetic field |
| | S17 `prose-aest` | stylometry features (Burrows-Delta NN) | 128 | Prose-style preference |
| **J. Network position** | S18 `graph-pos` | asym TransE-class on attendance graph | 256 | Position in tie-network |
| | S19 `struct-hole` | Burt structural-hole centrality scalar | 16 | Bridging-potential metric |
| **K. Relational dynamics** | S20 `conflict-style` | classifier on Thomas-Kilmann modes | 80 | Disagreement pattern |
| | S21 `care-mode` | classifier on care-pattern taxonomy | 80 | How citizen shows care |
| **L. Hard constraint vector** | H (not an embedder) | structured fields | n/a | Accessibility, dietary, language, schedule, no-go list. Hard filter, never soft signal. |

**Slate cardinality:** 21 slot embeddings + 1 hard-constraint vector `H_c`.
**Total dimensionality:** `Σ d_i ≈ 35,800` per citizen (sparse slots counted as embedded density). Never flattened.

### 3.4 Slate audit cadence

- **Per-PR:** new/changed slot triggers D1 + D2 on calibration set.
- **Quarterly:** full slate audit; retire any slot failing D1/D2; rebuild on different corpus.
- **Per-model-upgrade:** any change to an underlying foundation model (e.g., Nomic v2 release) triggers full re-distinctness audit + downstream regression test.

### 3.5 Provenance per slot (uncompressible)

For every embedder forward-pass, write `(model_sha, source_hash, ts, operator, confidence, policy_version)` to `CF_CITIZEN_PROVENANCE` (audit-log column family per ContextGraph inheritance).

---

## 4. Matching algorithm (formal)

### 4.1 Utility function

For citizen `c`, candidate gathering `g`:

```
U(c, g) = α · P_tie(c, g)              tie-formation 30d  [regressor, conformal-calibrated]
        + β · ΔH_compress(c, g)         compression-progress (Schmidhuber surprise)
        + γ · P_joy(c, g)              positive-affect 24h  [regressor]
        + δ · CivicValue(g, c)         benefit-to-other-citizens
        − ε · Homog(g)                 anti-bubble penalty (bridging-aware)
        − ζ · Strain(c, week)          schedule-burnout super-linear
```

Default weights (Phase-0 calibration, SARB-versioned, published per Invariant I18):
```
α = 0.30,  β = 0.25,  γ = 0.20,  δ = 0.10,  ε = 0.10,  ζ = 0.05,    Σ = 1.0
```

`ΔH_compress(c, g) := d_cos(median_M1(g), Φ_M1(c))` mapped through a band-pass `[0.25, 0.45]` peak (too-near=redundant, too-far=alienating). Direct application of Schmidhuber's compression-progress operationalization.

`Homog(g)` = bridging-coverage penalty: raised when `M9 topic overlap > 0.75` or `M12 ideological cosine > 0.7` across attendees.

`Strain(c, week)` super-linear: `Strain(c, w) = max(0, n_attended(c, w) − 3)^2`.

### 4.2 Constellation guard `G_τ` (TCT-inherited)

```
G_τ(g) :=  ⋀_{c ∈ attendees(g)}  ⋀_{m ∈ M_required(format(g))}
              cos(Φ_m(c), centroid_m(g)) ≥ τ_m
        ∧  H_c satisfies hard constraints of g
```

where `M_required` per format:

| Format | Required slots `M_required` |
|---|---|
| SYM | M13 hard-constraints, S5 narrative-form, S17 prose-aest |
| AGO | M13, S10 bridge-vec, S9 mfq (diverse), S7 reason-mode (mixed) |
| GYM | M13, S14 day-rhythm, S13 energy-band, S11 voice-tempo (compatible) |
| THE | M13, S1 sem-self, S5 story-shape, S8 abstract-concrete |
| FES | M13 only (open) |
| ALE | M13 only (pure-random by design) |

Thresholds `τ_m` calibrated on disjoint per-format calibration set; **MUST NOT** be tuned on the metric used for evaluation (TCT discipline, `docs/PAPER.md §5.4`).

`G_τ(g) = 0 ⇒ g rejected` regardless of `U(c, g)`. The guard is the floor; the utility is the optimization above the floor.

### 4.3 Selection mechanism

Per matching window:

```
1. coarse_recall:   per-slot HNSW k-NN, k = 100, filtered by H_c
2. mmr_rerank:      MMR(λ = 0.6) for intra-format diversity
3. bridge_rerank:   Ovadya bridging multiplicative boost
4. sortition_shuffle: random replacement of fraction φ_F of slots
                     φ_SYM = 0.15, φ_AGO = 0.25, φ_GYM = 0.20,
                     φ_THE = 0.15, φ_FES = open, φ_ALE = 1.00
5. G_τ_filter:      §4.2
6. score & emit:    U(c, g) ranks survivors
7. capacity_alloc:  modified Gale-Shapley with Rawlsian floor
                    (every citizen ≥ 1 high-U match before any gets ≥ 2)
```

### 4.4 Mistake-loop update (online, EWC++ Fisher-protected)

Inputs (training signals):

| Signal | Source | Weight |
|---|---|---|
| 24h `reflection.was_worthwhile` | citizen self-report | 0.2 |
| 7d `would_re-attend` | opt-in | 0.3 |
| **30d `tie_strength_signal`** | self-attestation of out-of-platform contact | **0.5 (primary)** |
| 90d/180d/365d `still_in_contact` | quarterly survey | bonus weight |

Update rule (per-citizen weight profile head only; frozen embedders never updated):

```
θ_c^{t+1} = θ_c^t − η ∇_θ L(predicted, observed)
            − λ_EWC · F̂_c · (θ_c^t − θ_c^*)
            where F̂_c = empirical Fisher info matrix
                  θ_c^* = checkpoint at last quarterly Constellation refresh
                  λ_EWC ≥ 100 (calibrated to prevent catastrophic forgetting)
```

Replay buffer (mistake bias): events with `|predicted − observed| ≥ 0.5` are high-weight replay samples.

All weight changes write `(operator = "system + SARB-approval-id", reason, before, after)` to `CF_MATCHING_WEIGHT_HISTORY`. No silent reweights (Invariant I18).

---

## 5. Five formats (mathematical specification)

Each format is a constraint-bound matching subproblem. The mathematical contract:

```
∀ format f ∈ {SYM, AGO, GYM, THE, FES, ALE}:
    cardinality(g) ∈ [min_f, max_f]
    duration(g) = duration_f
    venue_type(g) ∈ venue_classes_f
    G_τ_f(g) ≡ TRUE     (per §4.2 with M_required_f)
    success_metric_f(g) ≥ target_f
```

| f | success metric | target |
|---|---|---|
| SYM | mean `would_re-meet ≥ 1 attendee` | ≥ 0.60 |
| AGO | aggregate `mind_change_rate` (% of attendees who shifted ≥ 1 Likert step) | ≥ 0.30 |
| GYM | mean `this_was_right_energy` Likert | ≥ 4.0/5 |
| THE | mean `would_re-read_with_them` Likert | ≥ 3.8/5 |
| FES | turnout fraction of polis-citizen invited base | ≥ 0.25 |
| ALE | downstream-Newcomer 90-day retention from attendees of this ALE | ≥ 0.55 |

Per-format physical/procedural rules (Symposium phone-basket, Agora wooden-token + `eknomōs`, Gymnasium no-headphones, Theater pre-read, Festival no-commercial-vendors, Aleatory pure-lot): the implementing AI MAY design any UX that preserves the rules. The rules themselves are non-negotiable.

---

## 6. Governance — sortition + cooperative + amendment

### 6.1 Mathematical structure

Five governance bodies:

| Body | Composition | Sortition rule | Mandate |
|---|---|---|---|
| **Citizen Assembly** (per polis) | 1000 citizens | `Π_stratified(C_polis, strata = {age, neighborhood, residency_yrs, gender, format_pref}, rotation = 12mo)` | Polis budget, Tier-4 vote |
| **SARB** (central) | 50 citizens cross-polis | `Π_stratified(∪ C_polis, weighting = 0.2 technical / 0.8 lay, rotation = 6mo, replace 25 every 3mo)` | Algorithm-weight changes, fairness audits |
| **Proposition Drafting Panel** (per polis) | 3 Elders | `Π(Elders_polis, rotation = 3mo)` | Agora propositions, steelman commissions |
| **Welcoming Cohort** (per polis) | 20–50 | `Π(Established+, rotation = 6mo)` | Newcomer intakes |
| **Trustee Council** (central) | founder + 4–7 named | named in articles; recall by 75% Assembly vote | Tier-4 veto-only |

### 6.2 Sortition operator (mathematical)

```
Π_stratified(pop, strata, n) :=
  1.  for each stratum s: target_count_s := round(n · |pop ∩ s| / |pop|)
  2.  draw via VRF (BLS12-381) within each stratum
  3.  reject draw if any stratum deviates from target by > 5pp; redraw
  4.  invite drawn citizens; replacement pool fills declines maintaining strata
  5.  publish (aggregate composition only) for audit
```

VRF receipts MUST be persisted in `CF_GOVERNANCE_SORTITION_LOG` and verifiable by any citizen.

### 6.3 Amendment process (Tier-4 invariant change)

```
amend(I_n) ⇒
   publish(I_n, reasoning, 90d_lead_time)
 ∧ SARB.vote(I_n) ≥ 0.60
 ∧ ∀ polis : Assembly_polis.vote(I_n) ≥ 0.60
 ∧ moratorium(I_n, 12mo) with Agora debate in every polis
 ∧ revote_after_moratorium with same thresholds
 ∧ (¬TrusteeCouncil.veto(I_n) ∨ Assembly_polis.override(veto) ≥ 0.75)
```

Practical lower bound on amendment time: ≥ 12 months. By design.

### 6.4 Cooperative articles + asset-lock

Multi-stakeholder cooperative. Membership classes (board allocation 5/3/2/2):

```
board_seats = 5 citizen-elected + 3 worker-elected + 2 venue-elected + 2 trustees
```

Asset-lock: assets not transferable to any entity not preserving Tier-4. Founder pre-commitment trust agreement signed day-1 binds founder to non-violation in perpetuity.

### 6.5 Business model (revenue streams)

```
revenue(polis) = subscription + venue_share + patronage + central_affiliation
   subscription per polis ≈ 0.60 · |C| · $96    (mixed tiers)
   venue_share per polis ≈ 0.50 · |C| · 1.5/wk · 52 · $30 · 0.08
   patronage ≈ 0.05–0.10 · operating_budget
   central_affiliation per polis ≈ |C| · $6

ad_revenue == $0  (Invariant I5, hard)
data_sale_revenue == $0 (Invariant I13, hard)
pay-to-play_revenue == $0 (Invariant I16, hard)
```

Subscription tiers `{Free, Solidarity $48, Standard $96, Patron $240}` — access-only, never matching-priority.

---

## 7. Privacy & security (formal properties)

### 7.1 Encryption

```
∀ c ∈ C : Φ_c, Σ_c stored as Enc_AEAD(plaintext, K_c)
   where K_c = HKDF(PoP_c, salt_polis)
   K_c held only ephemerally during match computation, in isolated process
   HSM-wrapped at rest
```

### 7.2 Differential privacy on aggregates

```
∀ public_aggregate A : DP(A; ε ≤ 1.0, δ ≤ 1e-6)
   ∧ cell_size(A) ≥ 30 citizens
∀ SARB-internal aggregate A : DP(A; ε ≤ 3.0, δ ≤ 1e-6)
   ∧ cell_size(A) ≥ 30
```

### 7.3 End-to-end DM channel

```
DM(c_1, c_2) defined ⇔ ∃ g ∈ G : c_1, c_2 ∈ attendees(g) ∧ both opt_in(g)
   payload encrypted via Signal-protocol; server holds only ciphertext
```

No DM without prior in-person meeting (architectural — no "swipe" path).

### 7.4 Right-to-be-forgotten

```
deletion_request(c) ⇒
   delete all Φ_c, Σ_c, DM ciphertext, attestations within 24h
   retain only anonymized event-outcome counts (no PII)
   issue signed deletion receipt to c
```

### 7.5 Audit log (append-only, ContextGraph inheritance)

```
∀ state_change s :
   write_audit(operator, ts, hash_before, hash_after, policy_version) → CF_AUDIT_LOG
   citizen-pullable via mcp tool: polis_citizen_audit_export
```

### 7.6 Breach response

```
breach_detected ⇒
   contain within 1h
   disclose to affected citizens + relevant DPA within 72h (GDPR Art 33)
   post-mortem publicly within 30d
```

---

## 8. Tier-4 invariants (boolean predicates, AI-checkable, immovable)

Each `I_n(D)` is a boolean predicate evaluable by an automated audit script. The AI **MUST** ensure every one returns `TRUE` in the deployment before `BUILD_DONE`.

| # | Predicate | Required value | AI-check method |
|---|---|---|---|
| I1 | `feed_exists(D)` | `false` | UI inventory + forbidden-symbol scan (`feed`, `timeline`, `for_you`) |
| I2 | `follower_count_visible(D)` | `false` | DB schema + UI scan |
| I3 | `like_count_visible(D)` | `false` | Same |
| I4 | `public_profile_browsable(D)` | `false` | Route inventory |
| I5 | `ad_revenue(D, year) > 0` | `false` | Financial audit + outbound API inventory |
| I6 | `corr(score, P_return_24h) − random_baseline ≠ 0` | `false` | SARB quarterly compute |
| I7 | `∃ visible_status_numeric` | `false` | Code scan for `reputation, karma, score, rank, level, badge` |
| I8 | `∃ infinite_scroll_component` | `false` | UI inventory; component-library scan |
| I9 | `time_on_app` in optimizer objective | `false` | Code review of objective functions |
| I10 | `Φ.flatten() exists in any code path` | `false` | Type system + lint scan |
| I11 | `∃ primary product = zero-IRL event` | `false` | Product surface inventory |
| I12 | `centralized_owner ∃ path to change Tier-4 alone` | `false` | Articles + state machine review |
| I13 | `∃ outbound third-party data sale endpoint` | `false` | Outbound API inventory |
| I14 | `∃ off-platform behavioral ad targeting pipeline` | `false` | Tracking pixel + SDK inventory |
| I15 | `LLM mediates message to-or-from citizen` | `false` | Workspace `feature = "llm"` compile-guard |
| I16 | `priority(c, matching) depends on $ paid or status` | `false` | Matcher contract: no priority parameter |
| I17 | `per-citizen match explanation in any public surface` | `false` | Output inventory |
| I18 | `∃ algorithm weight change without SARB log row` | `false` | All weight writes through `sarb.approve_weight_change` |
| I19 | `∃ slot with corr_+ > 0.6 or ΔH < 0.05 bits` | `false` | Distinctness gate CI |
| I20 | `release shipped without FSV evidence` | `false` | Release-tag CI gate |
| I21 | `release shipped without 14-axis hardening pass` | `false` | Release-tag CI gate |
| I22 | `breach disclosed > 72h after detection` | `false` | Incident-response runbook + drill |
| I23 | `release shipped without 22-invariant audit script returning all-pass` | `false` | `scripts/ci/invariant_check.sh` |
| I24 | `embedder slate change shipped without distinctness regression test` | `false` | CI |
| I25 | `governance state machine modified without sortitioned-review approval log` | `false` | Articles + audit |

AI-check ensemble:

```
INVARIANT_AUDIT(D) := ⋀_{n=1..25} I_n(D) == required_value(n)
```

`INVARIANT_AUDIT(D) = TRUE` is a prerequisite of `BUILD_DONE`.

---

## 9. Civic outcome metrics (post-launch, NOT codex's build gate)

For forward reference and for the post-launch operations team. The cooperative declares civic success iff (steady-state, ≥ 12 consecutive months, ≥ 4 polises):

```
serendipitous_events / citizen / week                  ≥ 1.5    (median)
qualified_new_strong_ties / citizen / year             ≥ 3
qualified_new_weak_ties / citizen / year               ≥ 25
UCLA-3 loneliness delta vs baseline                    ≤ −1.0
Steger MLQ-Search purpose delta                        ≥ +0.5
eudaimonia_index delta                                 ≥ +1.0   (5-Likert × 5 questions)
weekly debate-or-symposium attendance rate              ≥ 0.35
books_completed / citizen / year                       ≥ 6
serendipitous_romantic_match_rate (6mo sustained)      ≥ 0.01   (of single citizens / year)
cross-ideological-tie_rate                             ≥ 0.20
cross-generational-tie_rate                            ≥ 0.30
```

Anti-objectives (always):
```
mean_session_length_minutes                            ≤ 5
sessions / citizen / day                               ≤ 2
engagement_loop_addiction_score (Bergen BSMAS)         ≤ 1.0
status_score_dispersion                                = 0
ad_revenue_per_user                                    = 0
algorithmic_engagement_amplification                   = 0
```

Surveys (UCLA-3, Steger MLQ, BSMAS, eudaimonia) MUST be administered **in-person** by sortitioned peer citizens, on a stratified ~ 5–10% sample per polis per quarter, with ε-DP-bounded aggregation.

---

## 10. BUILD_DONE predicate (the AI's stopping condition — mechanical)

The implementing AI loops until this evaluates `TRUE`. Each line is independently auditable.

```
BUILD_DONE(D) ⇔

  // Code (T)
  T1.  ∀ crate ∈ workspace : cargo_build_release(crate).ok
  T2.  ∀ crate : cargo_test(crate).failed == 0 ∧ ignored == 0
  T3.  ∀ crate : cargo_clippy(crate, "--no-deps --tests --examples -- -D warnings").ok
  T4.  cargo_check("--workspace --tests --examples").ok
  T5.  ∀ file ∈ src : line_count(file) ≤ 500
  T6.  ∀ function ∈ src : line_count(fn) ≤ 30 ∧ cyclomatic(fn) ≤ 10
  T7.  no TODO / FIXME / "stub" / "placeholder" in main branch
  T8.  no forbidden symbol (`follower_count`, `engagement_score`, `like_count`, etc.) — see I1–I8

  // Embedders (E)
  E1.  |slate| == 21 (per §3.3) ∧ ∀ slot loadable
  E2.  D1 (distinctness) passes on calibration set X, |X| ≥ 500
  E3.  D2 (marginal info ΔH ≥ 0.05 bits) passes on calibration set
  E4.  D3 (frozen) compile-verified — no gradient path into any E_i

  // Constellation (C)
  C1.  ∀ test_citizen : Φ(c) constructible end-to-end (intake → all 21 slots populated)
  C2.  Provenance row written per slot per forward-pass to CF_CITIZEN_PROVENANCE
  C3.  Right-to-be-forgotten purges Φ_c < 24h, signed receipt issued
  C4.  Constellation type has no .flatten() / equivalent (I10)

  // Matching (M)
  M1.  U(c, g) computable per §4.1; weights signed by SARB-approval-id
  M2.  G_τ(g) per §4.2 implemented; trips on adversarial test set with > 95% recall
  M3.  Selection pipeline (§4.3) produces valid candidate gatherings for all 6 formats
  M4.  Mistake-loop (§4.4) consumes outcomes, updates per-citizen heads under EWC++ Fisher
  M5.  Capacity allocation = Gale-Shapley with Rawlsian floor; verified on synthetic 10k-citizen sim
  M6.  No code path accepts a "priority" / "promotion" parameter (I16)

  // Formats (F)
  F1.  ∀ f ∈ {SYM, AGO, GYM, THE, FES, ALE} : data_model, UI, venue-integration shipped
  F2.  ∀ f : per-format success-metric (§5) computable
  F3.  Agora: wooden-token protocol, eknomōs enforcement, mind-change-aggregate output
  F4.  Symposium: phone-basket UX, leitmotif card data model
  F5.  Festival: zero-commercial-vendor data model; no logo / sponsor field
  F6.  Aleatory: pure-lot composition (no algorithm involvement beyond H_c)

  // Lifecycle (L)
  L1.  Onboarding 5-step (PoP → intake interview → Citizenship Reading → first format → 30d signal) end-to-end works
  L2.  PoP credential supports all three modes (in-person attestation, Worldcoin, KYC fallback); KYC document deleted post-verification
  L3.  Citizen-anchored to ≤ 1 polis at any t; move requires 60d delay
  L4.  Exit ritual: 30min exit conversation offered; printed summary mailed; data deletion as L7

  // Governance (G)
  G1.  VRF-based stratified sortition operator (§6.2) implemented + verifiable via CF_GOVERNANCE_SORTITION_LOG
  G2.  Citizen Assembly state machine: convene, deliberate, tally, publish-aggregate
  G3.  SARB state machine: weight-change approval workflow; halt-by-supermajority
  G4.  Amendment process (§6.3) state machine end-to-end, time-locked
  G5.  Cooperative articles of incorporation drafted (legal artifact, generated by codex as JSON-LD + signed PDF) embedding all Tier-4 invariants
  G6.  Founder pre-commitment trust agreement template generated

  // Privacy + Security (P)
  P1.  All Φ_c, Σ_c stored AEAD-encrypted; key wrapped in HSM (or HSM-emulator for pilot)
  P2.  DP(ε=1.0, δ=1e-6) library integrated; min cell size 30 enforced
  P3.  Signal-protocol DM channel; server holds ciphertext only
  P4.  Audit log append-only; citizen-pullable
  P5.  Breach IR runbook; auto-page; drill completed

  // Business + Venues (B)
  B1.  Civic subscription billing (4 tiers); Solidarity self-attestable
  B2.  Partner Venue program: registry, group-rate booking, fee remittance (≤ 8% cap)
  B3.  Civic-Fund ledger (CF_CIVIC_FUND_LEDGER); catalyst grant workflow
  B4.  Financial audit: ad_revenue == 0, data_sale_revenue == 0 mechanically verifiable

  // Invariants (I)
  I0.  INVARIANT_AUDIT(D) per §8 returns ALL-PASS

  // Hardening (H)
  H1.  ∀ axis ∈ docs2/hardeningprompt.md (14 axes) : per-axis CI checklist clean
  H2.  Independent pen-test by external firm in last 12 months: no Critical / High open
  H3.  Reproducible-build verified for all release artifacts

  // Assets (A) — codex-generated visuals
  A1.  Logo SVG generated by codex, monochrome + color variants
  A2.  Brand palette (≤ 8 colors), typography spec, spacing scale (8pt grid)
  A3.  UI design-system (≤ 24 components, each ≤ 1 viewport tall, NO infinite-scroll component)
  A4.  Citizen-facing illustrations (≥ 20 SVG illustrations for onboarding, Five Formats, eudaimonia survey)
  A5.  Marketing landing page (single page, ≤ 5 sections, no growth-hack patterns)
  A6.  Festival poster template (parametric SVG; per-polis localization)
  A7.  Wooden-speaking-token SVG render (Agora ritual artifact)
  A8.  All assets pass accessibility audit: WCAG 2.2 AA, color-contrast ≥ 4.5:1

  // Deployment (D)
  D1.  Per-polis primary + 2-standby (Raft); RocksDB on durable NVMe; daily snapshots
  D2.  CI/CD pipeline; release-tag gate enforces I20, I21, I23
  D3.  Observability: structured logs, metrics, audit-log all live; aggregate dashboards (citizen-data-free)
  D4.  Disaster-recovery drill completed: RTO ≤ 4h, RPO ≤ 1h
  D5.  pilot_polis selected (population 50k–250k, walkable, diverse) and infra provisioned
  D6.  100-citizen invite list assembled and outreach scheduled (NOT yet invited — that's launch-day)

  // Documentation (X)
  X1.  Operator runbook: per-polis deployment, sortition operation, SARB workflow, IR
  X2.  Citizen-facing onboarding doc (≤ 8 pages, plain-language)
  X3.  Welcoming-citizen training guide (≤ 4 pages)
  X4.  Cooperative articles + bylaws (legal-reviewed)

BUILD_DONE(D) := T1..T8 ∧ E1..E4 ∧ C1..C4 ∧ M1..M6 ∧ F1..F6
              ∧ L1..L4 ∧ G1..G6 ∧ P1..P5 ∧ B1..B4 ∧ I0
              ∧ H1..H3 ∧ A1..A8 ∧ D1..D6 ∧ X1..X4
```

When `BUILD_DONE(D) = TRUE`, the AI halts and reports completion. No further iteration.

---

## 11. Falsification kill-switches (post-launch; AI must implement the *detectors*)

Each kill-switch is a measurable predicate that, if triggered, halts the project (post-launch):

```
F1.  After 24mo in ≥ 3 polises:  Tier-1 #1, #2, #4 fail ≥ 50% of target.
     ⇒ halt new-polis launches; commission RCA.

F2.  In any polis at any t:  Tier-2 #10, #11, or #12 drifts non-zero.
     ⇒ immediate rollback to last clean state.

F3.  In any polis at 12mo:  cross-ideological-tie rate < 0.05.
     ⇒ algorithm reweight; possible doctrinal review.

F4.  Independent academic study:  matched-cohort loneliness > non-platform cohort.
     ⇒ major review; may close.

F5.  Any Tier-4 invariant silently violated (audit catches drift).
     ⇒ immediate rollback + SARB + Trustee hearing.

F6.  Cooperative fails to constitute SARB by 24mo.
     ⇒ governance redesign.

F7.  Eudaimonia delta negative across ≥ 4 polises at 36mo.
     ⇒ product-theory review.
```

The AI's build job includes implementing the *detectors* for F1–F7 (the formulas exist; the data pipelines must be shipped). Whether the cooperative *acts* on a triggered detector is post-launch.

---

## 12. Codex / implementing-AI loop protocol

The AI runs in an infinite outer loop until `BUILD_DONE`. Each iteration is one Claude-Code outer-loop increment.

### 12.1 Per-iteration contract

```
iterate():
    state := read_source_of_truth()
    candidates := backlog ∪ regressions(state) ∪ failed_checks(BUILD_DONE)
    if candidates == ∅:
        if BUILD_DONE(state) == TRUE: HALT with success
        else: report blocker; sleep; retry
    next := select(candidates, criterion = max_progress_per_token)
    plan := decompose(next)
    for atom ∈ plan:
        before_sha := sha256(target_files)
        execute(atom)
        after_sha := sha256(target_files)
        evidence := FSV_readback(target_state)   // see §12.2
        if evidence.satisfies(atom.contract):
            commit + audit_log
        else:
            rollback; re-plan with evidence as context
```

### 12.2 FSV (Forensic Source-of-truth Verification) contract per atom

Universal per `docs/futurebuild/specs/FSV-PROTOCOL.md`. For every primitive shipped, the AI **MUST** answer mechanically:

1. What state changed? (one sentence, one CF / file / row)
2. Where does it live? (path + identifier)
3. How to read independently? (command / API call)
4. What value is expected? (literal)

Round-trip readback REQUIRED before claiming the atom complete. No mocks in gating tests. Fail-closed on unknowns with `SCREAMING_SNAKE_CASE` codes.

### 12.3 Asset generation responsibility

Codex **owns asset generation**. Codex MUST generate, via image-generation tools available to it:

- Brand: logo (≥ 3 variants), wordmark, monogram, color palette ≤ 8 colors, typography spec, spacing scale.
- Product UI: design-system components (≤ 24), all screens (≤ 8 distinct: home, next-event, pre-read, reflection, intake, exit, settings, audit-export).
- Illustrations: ≥ 20 SVG-class scenes covering onboarding, the Five Formats, Eudaimonia survey.
- Marketing: single landing page, ≤ 5 sections; festival poster template; Agora wooden-token 3D-SVG render.
- Accessibility: all assets WCAG 2.2 AA; contrast ≥ 4.5:1; alt-text generated.

Codex MUST NOT block on a human designer. The asset spec is part of `BUILD_DONE.A*`.

### 12.4 Anti-thrashing condition

```
monotone_progress(t) := count(BUILD_DONE clauses passing at t) − count(passing at t-N)
   where N = last 5 iterations
   monotone_progress(t) < 0 ⇒ AI MUST suspend feature-add, switch to RCA mode
```

I.e., if the AI's last 5 iterations have lowered the count of passing `BUILD_DONE` clauses, it has regressed; must investigate before continuing.

### 12.5 Memory persistence

Per `docs2/AICodingAgentSuperPrompt.md §3`: GitHub Issues are the memory substrate across sessions. CLAIM / PAUSE / RESOLVED / BLOCKED comment discipline. No `./memory/*.md` writes.

### 12.6 Inheritance from existing stack

The implementing AI is permitted (and encouraged) to inherit, reuse, or vendor:
- TCT / DDA / `G_τ` machinery from `docs/PAPER.md` (the Royse paper).
- ContextGraph workspace patterns: 51-CF RocksDB substrate, MCP server pattern, audit-log + provenance + lifecycle + retrieval-transparency agents, conformal predictor, HNSW, MMR, ed25519, witness Merkle, mistake-loop EWC++.
- ClipCannon multi-modal constellation pipeline (audio prosody, SigLIP visual, structural).

The implementing AI **MUST NOT** inherit:
- ContextGraph's 13-embedder code-prediction slate (wrong domain; rebuild slate per §3 for human-matching task).
- Any feature, surface, or substrate that violates Tier-4 (§8).

---

## 13. Forbidden operations (deny set)

```
FORBIDDEN := {
  add status surface (any I7-violating numeric),
  add engagement-amplification objective (any I6 violation),
  add ad-revenue path (any I5 violation),
  add inner-LLM message generation (any I15 violation),
  add cross-slot dim-compare on Φ (any I10 violation),
  add pay-to-play matching path (any I16 violation),
  use mock data in BUILD_DONE-gating tests,
  ship without FSV evidence per atom,
  destructive git: push --force, --force-with-lease, reset --hard, checkout --, clean -f, rebase shared branch,
  disable an invariant check to pass a test,
  paraphrase an invariant (compressionprompt.md §6.1, §6.9),
  add a TODO / placeholder where the invariant requires implementation,
  edit .env / *.pem / *.key / **/credentials.json,
  edit cooperative articles or any governance/articles/* without sortitioned approval,
  cargo check as substitute for cargo build --release,
  collapse Φ into a single concatenated vector for any purpose,
  introduce a synonym for an existing project term (compressionprompt.md §4.13),
  use an opaque acronym not in §1 notation table without defining,
  invent a project-specific glyph the foundation model has not seen (compressionprompt.md §6.5)
}
```

---

## 14. High-level hints (non-prescriptive)

The implementing AI may design freely within the spec. These hints are explicit because they would be expensive to rediscover:

1. **Start the embedder slate with the highest-ΔH slots** (S1 sem-self, S10 bridge-vec, S18 graph-pos, S11–S13 somatic). The somatic axis is where most matching products fail; the audio-derived signals carry disproportionate marginal info on Symposium-format compatibility.

2. **G_τ thresholds are calibration-set artifacts.** Build the calibration corpus *first* (intake-interview transcripts from ~ 500 friends-and-family pilot), then tune τ_m per-format on disjoint holdout. Don't let τ tuning happen at evaluation time.

3. **The mistake-loop is the only legitimate online-weight-update path** (Invariant-adjacent, inherited from ContextGraph #406). All other proposed online-update paths must be rejected as either Invariant I6 amplification or I18 unauthorized.

4. **Sortition is mathematically simple but politically subtle.** Test the VRF + stratification operator on synthetic 50k-citizen polises before live use; the rejection rate from the ± 5pp constraint can be high in extreme stratification.

5. **Asset generation is not a side-quest.** Visual identity binds the cooperative culturally as much as the algorithm binds it computationally. Generate the brand and design-system early — the design-system constrains every UI screen and so de-risks downstream work.

6. **The 5-question intake interview is data-richer than any form.** Resist the urge to "speed up" onboarding with a multiple-choice form. The Constellation depends on rich free-text from a curious human asking questions.

7. **Inherit deeply; rebuild the slate.** The ContextGraph 13 embedders solved code-patch reality prediction. This task is human-matching. Different signal. Different slate. The MACHINERY (TCT, DDA, G_τ, mistake-loop, audit-log, conformal) is the same; the EMBEDDERS are different.

8. **Compression doctrine applies to your own code and comments.** No multi-paragraph docstrings; one short line max; comment WHY not WHAT. Files ≤ 500 lines, functions ≤ 30 lines.

9. **The Agora `eknomōs` rule is sacrosanct.** Build the no-recording, no-transcription, no-quotation enforcement at the venue UX layer. Symposium phone-basket is the same instinct, easier to ship.

10. **Festival is the cheapest format to ship and the hardest to design.** Cheapest because the matcher is barely involved (open invitation); hardest because Festival creates the cultural cohesion that everything else depends on. Don't ship Festival as an afterthought.

11. **The Paideia layer is what makes Symposium/Theater compound.** Reading together turns weak ties into strong ties via the Chevruta mechanism. Build chevruta pairing as a first-class entity, not a special-case of Theater.

12. **The cooperative articles are a software artifact too.** Generate the cooperative legal documents as JSON-LD + signed PDF as part of `BUILD_DONE.G5`. Human lawyers will review; codex generates the draft.

13. **Loop on `BUILD_DONE` clauses in dependency order.** Embedders before matching. Matching before formats. Formats before lifecycle. Lifecycle before assets. Don't ship UI for a feature whose backend predicate isn't TRUE yet.

14. **Anti-thrashing detector is your safety net.** When `monotone_progress(t) < 0`, suspend feature work and run RCA. The instinct will be to push through; resist.

---

## 15. The minimal file structure of the fresh repo (suggested, not binding)

```
new-repo/
├── POLIS.md                    ← THIS FILE (the only required doc at root)
├── Cargo.toml                  ← workspace
├── crates/
│   ├── polis-core/             ← shared types (Citizen, Constellation, Gathering)
│   ├── polis-embedders/        ← 21-slot slate, forward passes, distinctness gate
│   ├── polis-storage/          ← RocksDB CFs, audit-log, provenance
│   ├── polis-matcher/          ← §4 algorithm, G_τ guard, mistake-loop
│   ├── polis-governance/       ← VRF sortition, SARB, Assembly, amendment
│   ├── polis-mcp/              ← MCP server, tool surface
│   ├── polis-client-api/       ← HTTP shim for browser auth + session
│   ├── polis-venue-ops/        ← Partner Venue lifecycle
│   ├── polis-witness/          ← ed25519 + Merkle (inherited from ContextGraph)
│   ├── polis-solver/           ← MMR + bridging-rerank + conformal (inherited)
│   └── polis-test-utils/       ← shared test fixtures
├── web/                        ← citizen-facing PWA (TypeScript)
├── assets/                     ← codex-generated brand, illustrations, design-system
├── scripts/
│   ├── ci/
│   │   ├── invariant_check.sh  ← evaluates I1–I25
│   │   ├── distinctness_gate.sh ← evaluates D1, D2
│   │   ├── build_done_audit.sh ← evaluates §10
│   │   └── hardening_check.sh  ← evaluates 14 axes
│   └── fsv/                    ← per-atom FSV harness templates
├── governance/
│   ├── articles/               ← cooperative articles + bylaws
│   └── runbooks/               ← operator runbooks
└── docs/                       ← anything the AI deems useful; not gating
```

The implementing AI MAY restructure freely; the structure above is one valid solution.

---

## 16. The build-done sentence, restated for clarity

```
The system has been built iff:
   ⋀ T1..T8 ∧ E1..E4 ∧ C1..C4 ∧ M1..M6 ∧ F1..F6
 ∧ L1..L4 ∧ G1..G6 ∧ P1..P5 ∧ B1..B4 ∧ I0
 ∧ H1..H3 ∧ A1..A8 ∧ D1..D6 ∧ X1..X4
 = TRUE.

Until that conjunction evaluates TRUE, the implementing AI iterates.
When it evaluates TRUE, the implementing AI halts.
```

That is the only stopping condition.

---

**Document terminus.** This is the entire mathematical specification of Polis. Reload on every context compaction. The implementing AI's loop terminates iff §10.BUILD_DONE = TRUE; until then, iterate.
