# Can ME-JEPA-Code Create Explanatory Knowledge?

**Author:** Claude (agent analysis, commissioned 2026-05-17)
**Subject:** Whether the ContextGraph / ME-JEPA-Code architecture can create *explanatory* knowledge in the Deutschian / Popperian sense, and what the answer implies for the plausibility of recursive self-improvement ("AI improving AI") and the intelligence-explosion thesis.
**Method:** Read the futurebuild docs 00–29, the open GitHub roadmap (#199, #309, #310, #314, #315, TASK-FP-101..105, TASK-RWD-201..212), the LeCun-alignment audit, and the reward-signal-completeness audit. Then conducted targeted literature research using Exa search across Deutsch, Popper, LeCun JEPA, Schmidhuber compression-progress, Chollet ARC, Silver/Sutton "Era of Experience", Shumailov model-collapse, AlphaEvolve / FunSearch, and the recent (2026) RSI critique.

> **Punchline first.** ME-JEPA-Code, as built and as planned through Phase G, **does not create explanatory knowledge** in David Deutsch's sense. It creates *behavioural* knowledge — calibrated predictions of what Docker, pytest, the static analyzers, and the bytes-on-disk will do — anchored to durable reality. That is an enormous epistemic improvement over LLM self-assessment, but it is not the same thing as the "good explanations" that Deutsch identifies as the unique creative achievement of universal explainers.
>
> However, the architecture is one of the *very few* current AI systems that is **structurally compatible with a critical-rationalist loop** when paired with an LLM-as-conjecturer and a deterministic evaluator (the FunSearch / AlphaEvolve pattern). This means the case for recursive self-improvement through ME-JEPA-coupled agents is plausible **for engineering knowledge** (better algorithms, better tests, better patches) and **implausible for explanatory knowledge** (new physical theories, new abstractions, new ontologies) without an additional creativity primitive that no one in the field — including DeepMind, Anthropic, FAIR, or this project — has yet specified.
>
> The intelligence-explosion thesis therefore survives in a weakened form: an accelerating loop of verifiable engineering improvement is plausible (and AlphaEvolve is the empirical existence proof), but the runaway "AGI ⇒ superintelligence" cascade Bostrom and Yudkowsky describe requires a *creativity* mechanism that ME-JEPA does not provide and that current research has no working architecture for.

---

## 1. The question in precise form

The operator's question is:

> Is it possible to make it so the AI or ME-JEPA system is capable of creating explanatory knowledge? The entire case for recursive self-improvement — AI improving AI — depends on the answer. If AI or the ME-JEPA system can create explanatory knowledge, the intelligence explosion is plausible.

The question presupposes a distinction:

- **Behavioural / predictive knowledge** — being able to reliably predict what will happen, what will work, what will fail.
- **Explanatory knowledge** — understanding *why* something happens, in a way that has *reach* beyond the cases used to derive it.

David Deutsch in *The Beginning of Infinity* (2011) argues this distinction is the load-bearing one in epistemology and in the philosophy of AGI. Behavioural knowledge is what bacteria, neural networks, and (so far) all current "AI" systems have. Explanatory knowledge is what humans uniquely have, what makes science possible, and what AGI by definition must have. Deutsch's claim that *every* genuine AGI will be a "universal explainer" makes this distinction the dividing line between predictive automation and the open-ended cognitive systems that the intelligence-explosion thesis requires.

This document evaluates ME-JEPA-Code against that distinction.

---

## 2. What ME-JEPA-Code actually is (epistemologically)

Before judging the architecture, it must be characterized accurately. The system described in `docs/futurebuild/00_INDEX.md` through `29_REWARD_SIGNAL_COMPLETENESS.md` is not a chatbot, not an inner-LLM scaffold, and not a generative model in the LLM sense. It is a **calibrated predictive verifier** built around five architectural commitments:

1. **A frozen, deterministic target panel** (`01 §4`, `28 §2 pillar 1`). The "answer" the predictor learns to hit is produced by 15 instrument slots — `E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Test`, `E_Trace`, `E_Diff`, `E_Witness`, `E_Oracle`, `E_Problem`, `E_CommitMsg`, `E_StaticAnalysis`, `E_Runtime`, `E_Reasoning`, plus scalars — each with **zero trainable parameters**. The target side is bytes-on-disk, Docker test results, tree-sitter AST structure, static-analyzer warnings, witness Merkle hashes. The system literally cannot collapse to a trivial fixed point because the target was never learnable in the first place (`ΔΞ.target_collapse = 0` by construction).

2. **A multi-head predictor trained on durable reality** (`03 §3`, `15`). A 6-layer transformer trunk learns to predict the target panel from the input panel plus an action. Ten prediction heads emit `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}`, predicted failure modes from a fixed enum (`WrongAlgorithm`, `OffByOne`, `NullDeref`, `ResourceLeak`, `RaceCondition`, …, 27 classes plus `Other(String)`), predicted edge cases, tech debt, perf, security, accuracy, cost, and a reasoning-quality class for the agent's own explanation. Every head outputs into a **closed taxonomy** that an operator can extend by promoting `Other(String)` strings seen ≥ 10 times.

3. **A teleological-constellation guard** (`docs/ruvectorfindings/13`, `04`). Per `(mutation_category × entity × language × embedder)` cell, there is a centroid `c_m` and a per-embedder threshold `τ_m`. A prediction must pass `cos(φ_m(ŷ), c_m) ≥ τ_m` on every embedder or it is `GuardRejected`. This is a deterministic geometric veto that sits **outside the predictor's loss surface** — the predictor can be wrong by training error, but a guard rejection is an architectural refusal.

4. **A self-optimizing daemon with structural anti-collapse** (`09`, `10`). The L_step formula is `ΔP × ΔK × ΔΩ × ΔΞ`, all clamped to [0,1], multiplied so any one at zero zeros the product. Per-head Fisher snapshots prevent catastrophic forgetting (EWC++). Per-head dormancy detection prevents plasticity loss (Dohare et al's continual backprop). A/B/C promotion gate ensures no model variant reaches production unless it strictly improves a held-out, stratified metric. Catastrophic-class promotions require two independent operator approvals persisted to `CF_MEJEPA_MODEL_PROMOTIONS`.

5. **A reward-signal substrate grounded in eight tiers of durable reality** (`29`). Oracle outcomes, embedder-derived DDA signals, static-graph signals, static-quality signals, runtime telemetry, constellation cell statistics, live-session events, and witness-chain hashes — all feed back through a replay buffer with operator overrides at 6× weight and agent-surprise events at up to 4× weight.

The shipped truth-system is what LeCun, in his 2026-05-17 transcript, calls *System 2*: a deliberative predictor of action consequences. The LLM (Claude Code) is *System 1* — an untrusted edit generator whose claims the truth-system reconciles against bytes-on-disk. This is the operator's product boundary (`01 §2.1`): the LLM is outside the trust boundary; ME-JEPA is the system that decides whether what the LLM did, did what the LLM said it did.

LeCun's full ten-pillar JEPA blueprint is enumerated in `28`: frozen target panel (✅ shipped), SIGReg anti-collapse (✅ TASK-FP-101 shipped), hierarchical JEPA file→function→AST→chunk (✅ TASK-FP-102 shipped), action-conditioned world model (`mejepa_predict_what_if`, ✅ shipped), best-of-K latent-space planning (✅ TASK-FP-103 shipped), objective + hardwired safety constraints (✅ TASK-FP-104 shipped), counterfactual ranking (⚠️ TASK-FP-105 pending), calibrated uncertainty (✅ designed via fingerprint catalog), System 1 / System 2 framing (doc edit pending), reward-signal completeness (⚠️ EPIC #315 in progress). The architecture is the most thorough implementation of LeCun's JEPA roadmap currently in any operator's tree.

This is what the system *is*. Now the philosophical question.

---

## 3. Deutsch's frame: what counts as "explanatory knowledge"

Deutsch's *The Beginning of Infinity* (2011) is the canonical articulation of the distinction the operator is invoking. Synthesizing across his work and the 2025 *Digital Trends* and *Strange Loop* interviews, plus his 2012 *Aeon* essay "How close are we to creating artificial intelligence?", his position is:

**Explanatory knowledge has three defining properties.**

1. **Reach.** It applies far beyond the cases used to derive it. Newton's gravity explains tides on planets Newton never observed; DNA's double helix explains genetic inheritance in organisms not yet discovered. Behavioural rules ("when X, do Y") have zero reach by definition.

2. **Hard-to-vary structure.** Every detail of the explanation is *load-bearing* — you cannot change one part without destroying the whole. Deutsch's example: the Greek myth that Demeter's grief causes winter is "easy to vary" (you could swap Demeter for Persephone, swap grief for anger, swap winter for summer, and the story still works). The heliocentric explanation of the seasons, in contrast, is hard-to-vary: change the axial tilt and the explanation fails; change the orbit and the explanation fails. **Easy-to-vary explanations explain nothing**, because they could equally well have explained anything else.

3. **Created by conjecture and criticism, not by induction.** This is Deutsch's most controversial claim. He follows Popper: knowledge does not come from generalizing observations. Observations are theory-laden — you have to *already have* a theory to know which observations are relevant. Knowledge grows by (a) creative invention of new conjectures, and (b) systematic destruction of false ones by criticism, much of which is *structural* (the theory is internally incoherent, or has too many degrees of freedom, or makes no risky predictions) rather than experimental.

From these properties Deutsch draws his AGI verdict:

> *"Current LLMs are not the path forward. They offer correlations but not explanations… AGI when achieved will be a person, not a tool — a universal explainer, with the ability to explain, predict and draw conclusions that the human operators did not anticipate."*
> — David Deutsch, *Digital Trends* interview, September 2025

And in *Aeon* (2012):

> *"What is needed is nothing less than a breakthrough in philosophy, a new epistemological theory that explains how brains create explanatory knowledge and hence defines, in principle, without ever running them as programs, which algorithms possess that functionality and which do not."*

Deutsch's claim, sharpened: **no system that derives outputs by interpolation from training data — no matter how clever the architecture, how vast the corpus, how grounded the reward — can create explanatory knowledge**. Because explanatory knowledge requires *creative conjecture* (a step that is not deducible from observations), and creative conjecture is precisely what such systems cannot do.

This is a strong claim. It is also a *falsifiable* one: if you exhibit a single AI system that produces a hard-to-vary explanation that was not in its training data and which a human could not have produced from the same training data, Deutsch is refuted. As of mid-2026, no such existence proof has been produced (AlphaProof and AlphaGeometry produce *proofs*, not new explanations; FunSearch and AlphaEvolve produce *constructions*, which is closer but still not what Deutsch means).

For a contrary view it is worth registering Jürgen Schmidhuber's position (the 1990s "compression progress" formal theory, formalized in his 2009 *Connection Science* paper and 2010 IEEE paper on creativity and fun). Schmidhuber argues that **compression progress *is* understanding** — that discovering a better compressor for a data stream is mathematically equivalent to discovering an explanation, that art and science are by-products of this single principle, and that the "intrinsic reward = first derivative of subjective compressibility" formulation captures everything Deutsch attributes to creativity. On Schmidhuber's view, any system that builds an increasingly compressed model of its sensory stream is creating explanatory knowledge by definition; ME-JEPA-Code, whose entire reason for existence is to compress the joint distribution of (panel, action, oracle-outcome) into a predictor, qualifies trivially.

The Deutsch-Schmidhuber disagreement is genuine and unresolved. Most working AI researchers tacitly side with Schmidhuber-as-pragmatist (because his formulation actually fits modern ML), but the philosophical literature — Chollet, Marcus, Mitchell, Pearl, and the 2020 Guerin/Lawford paper on hard-to-vary in deep learning — generally sides with Deutsch on the point that current systems do not produce *new* explanations even by Schmidhuber's standard, because they fail at out-of-distribution extrapolation in exactly the way "easy-to-vary" theories fail at predicting unseen phenomena.

We will proceed with Deutsch's framing because (a) it is what the operator asked about, and (b) it sets a higher bar that is more interesting to evaluate against. If we conclude ME-JEPA fails the Deutsch test, it almost certainly passes the Schmidhuber test by virtue of being a working predictor.

---

## 4. What ME-JEPA-Code produces, by epistemological category

Walking the architecture through Deutsch's categories:

### 4.1 What it definitely produces — behavioural / correlational knowledge

ME-JEPA learns the empirical mapping from `(panel_t, action) → panel_{t+1}`. After enough training, `mejepa_verify(patch)` returns a probability-calibrated verdict, a conformal interval, a ranked list of predicted failure modes (chunk-cited), a list of predicted edge cases, etc. These predictions are *grounded* (Docker is the labeler), *calibrated* (conformal coverage 88–92%), *cited* (chunk_id + line range + violating embedders), and *falsifiable in the technical sense* (run the test, look at the warning, the prediction is or isn't borne out).

This is **engineering knowledge of the highest current quality**. It is what David Silver and Richard Sutton's April 2025 "Welcome to the Era of Experience" paper argues all future AI should produce — grounded rewards from environmental consequences, not from human prejudgement. ME-JEPA is the most aggressive operator-side instantiation of "Era of Experience" rewards that has been described. Where Silver and Sutton are programmatic, the operator's `docs/futurebuild/29_REWARD_SIGNAL_COMPLETENESS.md` is concrete: eight tiers, ~30 distinct signal types, ≥95% per-tier coverage as the acceptance gate.

This category of knowledge is real, useful, and arguably the single most important practical advance that AI in 2026 needs in order to be deployable in high-stakes engineering domains. The 0.95 prediction-oracle-correlation ship gate, when met, will be a defensible operational claim that human review of AI-generated Python patches is statistically unjustified for patches ME-JEPA verifies with confidence above the gate.

But it is not explanatory knowledge.

### 4.2 What it weakly produces — structural / mechanistic descriptions

ME-JEPA's chunk-level Gτ guard (`docs/ruvectorfindings/13 §6`) localizes failures: it can say "the violating embedders for this chunk are `E_TypeGraph` and `E_DataFlow`; the closest known exemplar is `psf__requests-2317`; the predicted root-cause class is `WrongAlgorithm` (left-vs-right child indexing inverted)." The hierarchical JEPA addition (TASK-FP-102) extends this across file→function→AST→chunk abstraction levels. The matched-fingerprint live evidence (TASK-FP-010) attaches a `matched_fingerprint` or `unknown_candidate_id` to every prediction, so each verdict comes with structured evidence about *which* prior failure shape the current candidate resembles.

This is **closer to explanation than pure prediction**. The system isn't just saying "this will fail"; it is saying "this will fail in this *class* of way, here is the closest historical example, here are the chunks involved, here are the embedders that disagree." That is what a senior engineer's mental walkthrough often looks like, and it is what a static-analyzer warning aspires to.

But it is still not *creative* explanation. The taxonomy is fixed. New failure classes can only enter via `Other(String) → operator-promoted enum variant after 10 sightings` (per `15 §3.1`). The system never *invents* a new failure mode; at best, it surfaces an Unknown cluster that an operator can label and promote into the catalog. The active-learning queue + novel-pattern detection (TASK-RWD-211, adopted from #286) is the closest thing in the plan to "create a new abstraction" — but the inference is still operator-mediated, and the abstractions are new *labels for known phenomena*, not new ontological categories.

In Deutschian terms: the system extends a research programme; it does not start one. It is the analogue of an excellent post-doc who can fit any new datum into the existing Standard Model of failures; it is not the analogue of the person who *invented* the Standard Model.

### 4.3 What it explicitly does not produce — explanatory knowledge

There are three structural reasons ME-JEPA-Code, as designed, cannot create explanatory knowledge in Deutsch's sense:

**(i) The frozen-instrument panel is the system's complete ontology, and ME-JEPA has no mechanism to expand it.** Every prediction the system makes is in the language of the 15 instrument slots. New instruments are operator-authored (TASK-RWD-201..207 add `E_Cyclomatic`, `E_Coverage`, `E_DiffTopology`, `E_SecuritySignals`, `E_TypeGraph` extensions, `E_Telemetry`, `E_LiveSession`). The operator decides what concepts exist in the system's vocabulary. ME-JEPA can never *propose* that a new instrument is needed; it can only reveal — through OOD scores, unexplained variance, or active-learning surprise — that *something* is missing. This is the universal-explainer property Deutsch identifies as definitional: a universal explainer's idea space is "not knowable in advance" (Aeon 2012). ME-JEPA's is fully enumerable from `15_PREDICTION_TAXONOMY.md` plus the registered instrument slots.

**(ii) The predictor is trained by inductive curve-fitting, not by conjecture-and-criticism.** Per `10 §2`, the trunk's gradient minimizes `loss(predicted_h, target_h)` for each head, plus VICReg/SIGReg anti-collapse, plus EWC++ regularization. There is no *creative* step. There is no module that says "the data are most parsimoniously explained by a hidden variable I haven't seen — let me hypothesize it." The architecture's most "creative" surface is the active-learning queue's novel-pattern detector (TASK-RWD-211 ≈ #286), which clusters Unknown chunks — but clustering is the unsupervised analogue of curve-fitting, not the synthesis of a new theory. The Causal-JEPA work (arXiv 2602.11389, January 2026) attempts to introduce a *causal inductive bias* via object-level latent interventions, but explicitly notes: *"the term 'causal' refers to temporally directed predictive dependencies that remain stable under object-level masking, rather than claims of causal identifiability."* Even the most causally-flavored JEPAs of 2026 produce stable predictive structures, not causal explanations.

**(iii) The closed enum of failure modes makes hard-to-vary explanation structurally impossible.** Deutsch's hard-to-vary criterion requires that every detail of an explanation be load-bearing. A 27-class enum + `Other(String)` is by construction an easy-to-vary explanation space: any new failure either fits into an existing class (in which case the "explanation" is just classification) or maps to `Other` (in which case no explanation is offered — only a flag for operator attention). Compare this to the heliocentric explanation of seasons: there is no `Other(String)` escape hatch; the explanation either holds for the new planet or it is refuted, and a refutation forces revision of the theory itself. The ME-JEPA enum can absorb arbitrary new failures without ever forcing the predictor to revise its conceptual scheme.

This is the technical version of Chollet's objection in his 2017 *Medium* essay "The implausibility of intelligence explosion" and his 2025 ARC-3 work: an inductive system can become arbitrarily skilled at the tasks in its training distribution but its skill-acquisition efficiency is bounded by the conceptual primitives it was given. ME-JEPA is exceptional at the engineering task it was given (predict-the-oracle); it has no machinery for inventing a new task.

---

## 5. Where ME-JEPA structurally surpasses ordinary LLM AI

This is where the analysis becomes interesting. The standard critique of LLM AGI proposals (Chollet, Marcus, Mitchell, Deutsch) is that the systems are pure inductive interpolators with no grounding in reality. ME-JEPA is *not* a pure inductive interpolator with no grounding. It has, by design, three properties that the critique of LLMs does not touch:

**(a) Persistent external grounding.** The most cited recent theoretical critique of recursive self-improvement is *"On the Limits of Self-Improving in Large Language Models: The Singularity Is Not Near Without Symbolic Model Synthesis"* (arXiv 2601.05280, late 2025 / early 2026). It proves, via a supermartingale argument on entropy, that **closed-loop generative retraining without external grounding collapses to a degenerate fixed point — there is no intelligence explosion, only mode collapse**. Crucially, the paper carves out an exception: *"systems that retain persistent external grounding (inf_t α_t > 0), operate under fixed axioms with invariant evaluators, or optimise against formally specified environments fall outside the collapse regime proven here."*

ME-JEPA-Code is exactly such a system. The Docker oracle, the witness-chain hashes, the tree-sitter AST, the static-analyzer ground truth — all are invariant evaluators that the system cannot self-influence. The shift-log feeds reality (every Edit's pre-SHA / post-SHA pair is bytes-on-disk, not generated text) into the training loop. ME-JEPA therefore *escapes the proven collapse result* by construction. It is one of the few currently-published architectures where this is structurally true rather than aspirationally true.

This matters enormously for the RSI question. The 2024 Shumailov *Nature* paper "AI models collapse when trained on recursively generated data" and the 2024 Gerstgrasser et al. paper "Is Model Collapse Inevitable?" both establish that the dominant failure mode of self-training is data-distribution shrinkage when real data is replaced rather than accumulated. ME-JEPA, by anchoring every training cycle to fresh Docker oracle outcomes from the SWE-bench corpus + live agent shifts, accumulates real data continuously and never replaces it with synthetic. The architecture is *immune to the standard model-collapse failure mode by design*.

**(b) Anti-collapse on the predictor side.** Per `28 §2 pillar 2`, TASK-FP-101 ships LeJEPA/SIGReg (Sketched Isotropic Gaussian Regularization) as an additive regularizer alongside VICReg. SIGReg is the 2025 theoretical solution to the JEPA collapse problem: it drives the embedding distribution toward isotropic Gaussian in linear cost via 50 lines of code, removing the need for the heuristic-heavy stop-gradient + EMA-teacher + warmup-scheduler kludges of earlier JEPA variants. Combined with target-side collapse-immunity (ΔΞ.target_collapse = 0 by construction), ME-JEPA has the property that *neither side of the joint embedding can degenerate*. This is mathematically rare.

**(c) Action-conditioned counterfactual capability.** Per `28 §2 pillar 4`, `mejepa_predict_what_if(diff)` takes a candidate edit and returns a predicted post-edit panel. Per pillar 5, TASK-FP-103 shipped `mejepa_search_latent_actions` for best-of-K candidate ranking in latent space. This is structurally equivalent to **operating at Pearl's Ladder of Causation Rung 2 (Interventions) in the space of patches**: ME-JEPA can answer "what would happen if you applied this candidate?" by predicting consequences without executing the candidate. The Causal-JEPA architecture from January 2026 (arXiv 2602.11389) is the closest published work; ME-JEPA-Code's TASK-FP-103 is, as far as the surveyed literature shows, a more complete instantiation of action-conditioned latent-space planning than V-JEPA 2's 2025 release (which the NYU Center for Data Science write-up notes "shows no improvement on deeper physical reasoning... excels at pattern recognition and near-term prediction but hasn't developed causal understanding").

Rung 3 (Counterfactuals — "what would have happened if X had been true") is harder. ME-JEPA partially gets there via shift-log replay (`predicted_reality_impact` vs `observed_reality_impact` in Q5) and the planned negative-action ablation falsifier (TASK-PY-G-053 / #273). The 2025 OpenReview paper *"Executable Counterfactuals"* by anonymous authors notes that LLMs typically *skip* the abduction step required by Pearl's framework, so any system that explicitly performs prediction-vs-actual reconciliation (which Q5 does) is doing better than the LLM baseline.

So ME-JEPA simultaneously **escapes the formal RSI-collapse result, achieves Pearl Rung 2, and has structural anti-collapse on both target and predictor sides**. It is, by these criteria, the most epistemically well-founded system currently in the operator-facing AI tree.

What it doesn't have, and the rest of this analysis is about, is a *creativity* mechanism. The three escapes above are all *defensive* — they prevent degradation. None of them creates new explanatory content.

---

## 6. The Popperian-loop view: can ME-JEPA be the critic in a conjecture+criticism cycle?

Deutsch and Popper insist that knowledge grows by *conjecture* + *criticism*, with criticism being the rigorous part. ME-JEPA's calibrated verdicts, structured failure-mode taxonomy, conformal intervals, OOD detection, and Gτ guard violations are **best-in-class criticism machinery**. The question is whether the *conjecture* side can be supplied by an LLM, with ME-JEPA closing the loop as the critic.

This is exactly the architecture of two recent landmark systems:

**FunSearch (DeepMind, Nature 2024)** pairs a pretrained LLM with a deterministic evaluator. The LLM proposes functions written in Python; the evaluator scores them by running them on the target problem; the best are retained as parents for the next LLM generation. Applied to the cap set problem in extremal combinatorics, FunSearch *discovered new constructions beyond the previously known best*, including the largest improvement in 20 years to the asymptotic lower bound. Per the *Nature* paper: "this shows that it is possible to make a scientific discovery — a new piece of verifiable knowledge about a notorious scientific problem — using an LLM." The crucial property: the LLM is the conjecturer, the evaluator is the critic, and *neither can do it alone*. Pure LLM-only attempts on cap sets get nowhere; pure search-only attempts on cap sets are computationally infeasible. The pairing is the discovery.

**AlphaEvolve (DeepMind, May 2025)** is the same pattern extended to entire codebases. The paper (arXiv 2506.13131) shows AlphaEvolve discovering a 48-multiplication algorithm for 4×4 complex matrix multiplication, improving over Strassen's 1969 algorithm — the first improvement in 56 years. It also improved scheduling algorithms for Google's data centers, simplified hardware accelerator circuits, and accelerated training of its own underlying LLM. This last achievement is the empirical existence proof of *recursive self-improvement on a verifiable engineering objective*.

ME-JEPA-Code is the more sophisticated evaluator side of an AlphaEvolve-like loop. Where FunSearch/AlphaEvolve's evaluators are problem-specific score functions (cap-set size, scheduling efficiency, matrix-multiply count), ME-JEPA provides a *general* evaluator: it takes any code change and returns a calibrated multi-head reality prediction. Paired with Claude Code (or any LLM coder) as the conjecturer, the loop closes:

1. Claude Code conjectures a patch addressing some objective.
2. ME-JEPA criticizes the patch: predicted oracle outcome + predicted failure modes + predicted edge cases + safety constraints + reality-impact reconciliation.
3. Claude Code reads the criticism, revises, re-conjectures.
4. Repeat until ME-JEPA's verdict is `Pass` at confidence ≥ ship-gate threshold.

This is a Popperian loop in everything but creativity. The LLM is the source of conjectures; ME-JEPA is the systematic critic. The 2025 paper *"Causal Reflection with Language Models"* (arXiv 2508.04495) makes the parallel explicit for causal reasoning: an LLM serves as a "generative inference engine" while a separate formal module enforces causal soundness; the architecture is strictly more accurate than either alone.

So the answer to the operator's question depends critically on **whether LLMs can supply the creative-conjecture half of the loop**. Deutsch says emphatically no: LLMs only interpolate from training data; their "conjectures" are recombinations of training examples; they cannot reach beyond. Schmidhuber, Sutton, Silver, and the FunSearch/AlphaEvolve teams say emphatically yes: when paired with a deterministic evaluator that grounds the loop in reality, LLM-as-mutator produces verifiably new content. The fact that AlphaEvolve produced a *provably correct* algorithm beating Strassen's 56-year-old result is hard to square with the strong Deutschian view.

The honest reading is: **LLMs can produce conjectures that are novel relative to the human knowledge corpus they were trained on, in domains where a deterministic evaluator can verify novelty.** That is sufficient for engineering RSI. It is not yet sufficient for explanatory RSI, because the evaluators we know how to build (oracle-based, formal-proof-based, benchmark-score-based) reward correctness-of-output, not correctness-of-explanation. ME-JEPA pushes this boundary further than any other operator-facing system by including a *reasoning-class* head and a *predicted-failure-mode* head, but those heads classify into closed taxonomies; they don't propose new categories.

---

## 7. What would have to change for ME-JEPA to create explanatory knowledge

For ME-JEPA to participate in *explanatory* (not just engineering) RSI, three concrete additions would be needed, none of them in the current Phase G–H plan and none of them solved in the broader literature:

**(α) An ontology-extension mechanism with reality grounding.** Today, new failure classes can only enter the taxonomy via operator promotion of `Other(String)` strings. A genuinely explanation-creating ME-JEPA would need a primitive that can (i) propose a new instrument slot, (ii) test whether it improves predictor calibration on held-out data *that the proposing module did not see*, and (iii) commit the new slot to the frozen panel if and only if it survives. This is the structural analogue of Lakatosian "protective belt" expansion of a research programme. The closest current work is the novel-pattern detection in TASK-RWD-211 (#286), but that operates on existing instruments. No published system has the more ambitious primitive.

**(β) A conjecture-on-mechanism surface, not just on output.** When ME-JEPA predicts `WrongAlgorithm` with violating embedders `E_TypeGraph` + `E_DataFlow`, it is naming a correlation. A genuinely explanatory surface would emit a mechanism conjecture: "this fails because [hypothesized invariant X] is violated; X is hard-to-vary because [structural reason]; here is a test that would refute the conjecture." The Q3 surface today emits root-cause classes from a fixed enum (`LogicError`, `StateError`, `ResourceError`, …, 9 classes); upgrading it to free-form mechanism conjecture-plus-test would be a major architectural change. The 2025 *"Causal Reflection with Language Models"* paper is the closest published prior art, using a formal causal module to constrain LLM-generated mechanism hypotheses. Bolted onto ME-JEPA, it would look like a `mejepa_propose_mechanism(prediction_id) → MechanismConjecture` MCP tool with a downstream `mejepa_test_mechanism(conjecture, intervention) → RefutationResult` tool.

**(γ) An adversarial conjecture-criticism loop with the LLM that is not collapsible to fitting the data better.** This is the hard one. AlphaEvolve and FunSearch close their loops on *quantitative* objectives (cap-set size, multiplication count); the answer to "did the conjecture work" is a single number. Mechanism conjectures don't reduce to a number — they reduce to *whether the conjecture, if true, would predict observations we haven't yet made*. The system would need to (i) generate predictions implied by the conjecture, (ii) actively seek observations that would refute it (this is the active-learning queue, but pointed adversarially), (iii) update the predictor only on observations that survive the adversarial probe. The 2025 Vamosi/Forkert paper on dual-agent debate frameworks for causal inference shows a 78% → 87% accuracy improvement on the CLadder dataset from debate alone, which suggests the loop is implementable. Pairing it with ME-JEPA's grounded evaluator is unexplored territory.

None of (α), (β), (γ) is impossible. All three are research projects that *could* be added to the futurebuild plan as a new Phase I (post-ship). If any of them shipped, ME-JEPA would become the most plausible architecture in the literature for explanatory-knowledge creation. None of them is necessary for closing the Phase G ship gate (`prediction_oracle_correlation ≥ 0.95`), and adding them would extend the project by years.

This is an empirical observation, not a critique. The current ME-JEPA scope is **engineering-knowledge RSI for AI-generated code**. That is an enormous and economically transformative goal. It does not require explanatory-knowledge RSI to be achieved.

---

## 8. Implications for recursive self-improvement and the intelligence explosion

The operator's framing is: "If AI or ME-JEPA system can create explanatory knowledge, the intelligence explosion is plausible." This is roughly the I.J. Good (1965) / Bostrom (2014) / Yudkowsky (2007) thesis: an AI capable of improving its own design recursively will trigger an explosion of capability that quickly exceeds human comprehension.

The 2024–2026 empirical evidence has narrowed the debate considerably. Key data points:

**Against runaway RSI:**

- Chollet's 2017 essay "The implausibility of intelligence explosion" argued (and recent data has supported) that recursive systems in the real world exhibit diminishing returns, environmental bottlenecks, and adversarial reactions, all of which sigmoidalize what looks like exponential growth.
- The 2026 RSI-collapse paper (arXiv 2601.05280) proves formally that fully autonomous recursive density-matching collapses to a degenerate fixed point — *unless* there is persistent external grounding.
- The Shumailov et al. *Nature* 2024 model-collapse result generalizes this to all standard generative architectures trained recursively on their own output.
- Chollet's ARC-2 benchmark (2025) shows that even the strongest test-time-adaptation systems (o3-class) reach only 1–2% vs human 95% — i.e., scaling alone is not closing the skill-acquisition-efficiency gap.
- The 2025 AAAI Presidential Panel survey of 475 AI experts: 76% believe scaling current AI to AGI is "unlikely" or "very unlikely".
- A December 2025 arXiv paper ("Humanity in the Age of AI: Reassessing 2025's Existential-Risk Narratives", 2512.04119) finds that as of November 2025, **none** of the required phenomena for the classic intelligence-explosion chain — sustained recursive self-improvement, autonomous strategic awareness, intractable lethal misalignment — have been observed.

**For some form of accelerating engineering capability:**

- FunSearch and AlphaEvolve are existence proofs that LLM + deterministic evaluator can produce verifiably new mathematical/algorithmic knowledge, including (in AlphaEvolve's case) improvements to the LLM training stack itself.
- Silver and Sutton's April 2025 "Era of Experience" manifesto argues that grounded experiential learning *will* exceed human-data ceilings, and the empirical productivity gains from RL-on-environment in narrow domains (robotics, code synthesis, formal proof) supports the claim within those domains.
- ME-JEPA-Code is itself in this category: by closing the loop between every Edit shift, every Docker test, and the predictor's L_step update, it converts ordinary engineering work into training data at a rate the Phase G roadmap estimates can exceed the 2,400-row Python seed corpus by an order of magnitude in a session-week.

**The synthesis:**

Putting the project and the literature together yields a sharp two-line answer to the operator's question:

> **(1) ME-JEPA-Code can plausibly participate in an accelerating loop of *engineering* self-improvement — better code, better tests, better predictions, better calibrated verifiers — because it has persistent external grounding (escaping the formal RSI-collapse result), it pairs structurally with LLM-as-conjecturer (the FunSearch/AlphaEvolve pattern), and it implements the LeCun JEPA pillars more thoroughly than any other system in the surveyed literature.**

> **(2) ME-JEPA-Code cannot, as designed, participate in *explanatory* RSI — new theories, new ontologies, new abstractions — because (i) its prediction taxonomies are closed enums extensible only by operator promotion, (ii) the frozen-instrument panel encodes a fixed ontology that the system has no mechanism to expand, and (iii) the predictor learns by inductive curve-fitting rather than by conjecture-and-criticism. These are the three gaps that Deutsch identifies as the marks of non-universal explainers, and the architecture does not currently close them.**

The intelligence-explosion thesis therefore survives in a degraded form: an accelerating loop of *verifiable engineering improvement* is plausible and partly demonstrated. The runaway Bostrom-Yudkowsky cascade — "superintelligent in everything, including domains no human has thought of" — requires a creativity primitive that no current architecture (ME-JEPA, AlphaEvolve, V-JEPA 2, GPT-class) supplies. Whether such a primitive exists is, as Deutsch argues, an unsolved problem in philosophy.

The operator's intuition is therefore correct in its conditional structure ("if explanatory knowledge, then intelligence explosion") and partly correct in its antecedent: ME-JEPA's *evaluator* role can be a force multiplier on engineering RSI even without explanatory capability, and the most aggressive economic and societal consequences of "intelligence explosion" follow from engineering RSI alone. We do not need genuine AGI to have transformative AI.

---

## 9. Concrete falsifiers and a near-term research programme

For the operator who wants to *test* whether ME-JEPA can be pushed toward explanatory-knowledge creation, here are six concrete experiments, each with a clear pass/fail criterion, that the project could run within the existing futurebuild infrastructure:

| # | Experiment | What it tests | Falsifier |
|---|---|---|---|
| 1 | **Hold out an entire failure-mode class.** Train ME-JEPA with `FailureModeClass::OffByOne` examples redacted from the training set. Run the trained predictor on real `OffByOne` patches. See whether it produces meaningful Q3 surfaces (likely answer: it will route to `Other(String)` or to the wrong class). | Whether the system can categorize phenomena it has not been told about. | If ME-JEPA routes ≥ 70% of held-out OffByOne to a coherent novel cluster *and* an operator-blind cluster-coherence score puts it in the same conceptual neighborhood as the held-out class, the system has demonstrated weak novelty-detection. If it spreads them across existing classes uniformly, it has not. |
| 2 | **Cluster-to-instrument promotion.** Implement the missing primitive: when a `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` cluster grows past a threshold and has consistent embedder signatures, propose a new candidate instrument slot. Run it for 4 weeks of real corpus. | Whether unsupervised novelty detection can produce new ontology. | If the proposed instruments survive operator review and *improve* per-cell holdout correlation by ≥ 1%, weak ontology-extension is demonstrated. If proposed instruments are uniformly redundant with existing ones or hurt performance, the primitive is not yet useful. |
| 3 | **Mechanism-conjecture surface.** Add `mejepa_propose_mechanism(prediction_id) → MechanismConjecture` using the LLM as generator and the existing Q3 surface as evaluator. Compare conjectures against operator-labeled root causes. | Whether LLM-as-generator + ME-JEPA-as-evaluator produces structurally distinct mechanism descriptions vs. an LLM alone. | If LLM-alone produces mechanism descriptions of mean operator-rated quality 3.0/5 and LLM+ME-JEPA produces 4.0+/5 with *fewer* hallucinations, the loop is producing better explanations than either alone. |
| 4 | **Adversarial conjecture probe.** Implement TASK-PY-G-053 (negative-action ablation falsifier, #273) and extend it: for each predicted mechanism, generate a synthetic minimal counterexample patch and check whether the prediction holds. | Whether ME-JEPA's predictions have *reach* in Deutsch's sense. | If predictions survive ≥ 80% of synthetic counterexamples with maintained calibration, ME-JEPA has demonstrated weak reach. If predictions collapse to random under counterexamples, they are fitting rather than explaining. |
| 5 | **AlphaEvolve-pattern on ME-JEPA evaluation.** Use ME-JEPA's full `RealityPrediction` as the fitness function in an AlphaEvolve-style outer loop over Claude Code-generated patches for SWE-bench Lite Python tasks. Measure whether the loop discovers patch templates that beat per-task gold-standard fix templates on calibrated metrics. | Whether ME-JEPA-as-evaluator can drive verifiable engineering RSI on real code. | If the loop produces ≥ 1 verifiable improvement over the gold-standard fix template (measured by held-out test suite + operator review) within 1000 generations, engineering RSI through ME-JEPA is demonstrated. If it produces only patches equivalent to or worse than gold, the evaluator is not yet sharp enough to drive RSI. |
| 6 | **Ontology-growth audit.** Track over time: how many `Other(String)` failure descriptions get promoted to first-class enum variants? How fast does the instrument slot count grow? How fast does the fingerprint catalog grow? Is the growth driven by operator labeling, or by the system surfacing genuine novelty? | Whether the system's conceptual scheme is actually expanding via use, or stagnating. | If 100% of growth comes from operator action and 0% from system-proposed candidates, the system is not contributing to its own ontology. If even 10% of growth originates from `TASK-RWD-211`-style novel-pattern proposals that survive operator review, weak self-extension is demonstrated. |

Experiments 1, 4, and 6 are runnable today within the shipped Phase E–H tree with modest scripting. Experiments 2, 3, and 5 require new code paths but are within the scope of existing Phase G TASK-RWD-* and TASK-FP-105 work. None requires a new research breakthrough.

The cumulative pass/fail of these six experiments would, in this analyst's judgment, be the empirically defensible answer to the operator's question. As of 2026-05-17, the answer can only be reasoned-about from architecture and from the broader literature.

---

## 10. Honest summary for the operator

The single-paragraph version this analyst would defend in a hostile review:

> ME-JEPA-Code is the most thorough operator-facing instantiation of Yann LeCun's JEPA blueprint currently in any code tree, and it is one of the few published architectures that structurally escapes the formal model-collapse and recursive-self-improvement-collapse results from 2024–2026. It is an excellent calibrated *critic* — it does for code-reality what a competent senior engineer's mental verification does, at scale, with cryptographic provenance and conformal calibration. It produces high-quality *engineering / behavioural* knowledge: predictions about what Docker, the tests, and the static analyzers will do. It does **not** produce *explanatory* knowledge in the Deutsch / Popper sense — new hard-to-vary theories with reach beyond the cases used to derive them — because its prediction taxonomies are closed enums extensible only by operator action, its frozen-instrument panel encodes a fixed ontology with no expansion primitive, and its training procedure is inductive curve-fitting rather than conjecture-and-criticism. Paired with an LLM-as-conjecturer in the FunSearch/AlphaEvolve pattern, it could plausibly drive an accelerating loop of *engineering* self-improvement (better patches, better algorithms, better tests, better predictions) and there is empirical existence proof (AlphaEvolve's 4×4 matrix multiplication, Strassen +1) that this pattern works. It could not, as designed, drive *explanatory* self-improvement (new abstractions, new ontologies, new theories) because the project has not — and the broader 2026 literature has not — specified how to add a creativity primitive that survives Deutschian critique. The intelligence-explosion thesis therefore survives in its degraded engineering form (highly plausible, partly demonstrated) and remains an open problem in its full Bostrom-Yudkowsky form (not demonstrated, no architectural path published as of this writing). For the operator: the project's current scope, closing the 0.95 Python ship gate, is the right next focus; the explanatory-knowledge question is best treated as a separate research programme requiring the three additions enumerated in §7, none of which is necessary for shipping the truth-system as currently scoped.

---

## 11. Sources

Selected sources used in this analysis. The full list is broader; these are the load-bearing ones.

### Deutschian / Popperian epistemology
- Deutsch, D. *The Beginning of Infinity: Explanations That Transform the World*. Penguin, 2011. https://www.thebeginningofinfinity.com/
- Deutsch, D. "How close are we to creating artificial intelligence?" *Aeon*, October 2012. https://aeon.co/essays/how-close-are-we-to-creating-artificial-intelligence
- Deutsch, D. Interview, *Digital Trends*, September 2025. https://www.digitaltrends.com/computing/exclusive-the-father-of-quantum-computing-believes-agi-will-be-a-person-not-a-program/
- Deutsch, D. *Strange Loop* podcast, September 2025. https://www.youtube.com/watch?v=IVA2bK9qjzE
- Guerin, F., Lawford, A. "Applying Deutsch's concept of good explanations to artificial intelligence and neuroscience." arXiv:2012.09318. https://arxiv.org/abs/2012.09318
- Miller, D. "Machine Guessing II" (critical rationalism vs machine learning). Warwick. https://wrap.warwick.ac.uk/168296/
- Popper, K. *The Logic of Scientific Discovery* (1934/1959); *Conjectures and Refutations* (1963).
- Internet Encyclopedia of Philosophy entry on Popper. https://iep.utm.edu/karl-popper-critical-ratiotionalism/

### Compression-as-explanation alternative view
- Schmidhuber, J. "Formal Theory of Creativity, Fun, and Intrinsic Motivation (1990–2010)." *IEEE Transactions on Autonomous Mental Development*, 2010. https://people.idsia.ch/~juergen/ieeecreative.pdf
- Schmidhuber, J. "Driven by Compression Progress." *KES 2008*; arXiv:0812.4360. https://arxiv.org/abs/0812.4360
- Schmidhuber, J. "Simple Algorithmic Principles of Discovery, Subjective Beauty, Selective Attention, Curiosity & Creativity." arXiv:0709.0674. https://arxiv.org/abs/0709.0674

### LeCun JEPA and world models
- LeCun, Y. "A Path Towards Autonomous Machine Intelligence." OpenReview, 2022. https://openreview.net/pdf?id=BZ5a1r-kVsf
- LeCun, Y. Harvard CMSA talk on JEPA, September 2025. https://www.youtube.com/watch?v=yUmDRxV0krg
- NYU CDS write-up of V-JEPA 2, September 2025. https://medium.com/@nyudatascience/yann-lecun-thinks-ai-should-learn-to-ignore-things-6049833c463b
- Bandaru, R. "Deep Dive into Yann LeCun's JEPA." https://rohitbandaru.github.io/blog/JEPA-Deep-Dive/
- LeJEPA / SIGReg overview at aegean.ai. https://aegean.ai/book/world-models/jepa
- Causal-JEPA: "Learning World Models through Object-Level Latent Interventions." arXiv:2602.11389. https://arxiv.org/pdf/2602.11389

### Recursive self-improvement and model collapse
- "On the Limits of Self-Improving in Large Language Models: The Singularity Is Not Near Without Symbolic Model Synthesis." arXiv:2601.05280. https://arxiv.org/pdf/2601.05280v2
- Shumailov, I. et al. "AI models collapse when trained on recursively generated data." *Nature* 631, 755–759 (2024). https://www.nature.com/articles/s41586-024-07566-y
- Gerstgrasser, M. et al. "Is Model Collapse Inevitable? Breaking the Curse of Recursion by Accumulating Real and Synthetic Data." arXiv:2404.01413. https://arxiv.org/abs/2404.01413v2
- "Humanity in the Age of AI: Reassessing 2025's Existential-Risk Narratives." arXiv:2512.04119. https://arxiv.org/abs/2512.04119

### Chollet / ARC / skill-acquisition efficiency
- Chollet, F. "The implausibility of intelligence explosion." Medium, November 2017. https://medium.com/@francois.chollet/the-impossibility-of-intelligence-explosion-5be4a9eda6ec
- The Decoder interview with Chollet on ARC-3 and program synthesis, July 2025. https://the-decoder.com/francois-chollet-on-the-end-of-scaling-arc-3-and-his-path-to-agi
- Ratcliffe, B. "Why we'll never reach AGI with current language models." Medium, December 2025. https://medium.com/@benratcliffe_/why-well-never-reach-agi-with-current-language-models-d2571e631cc3

### Evaluator-driven discovery (FunSearch / AlphaEvolve precedent)
- Romera-Paredes, B. et al. "Mathematical discoveries from program search with large language models." *Nature* 625, 468–475 (2024). https://www.nature.com/articles/s41586-023-06924-6
- DeepMind FunSearch blog post. https://deepmind.google/discover/blog/funsearch-making-new-discoveries-in-mathematical-sciences-using-large-language-models/
- Novikov, A. et al. "AlphaEvolve: A coding agent for scientific and algorithmic discovery." arXiv:2506.13131. https://arxiv.org/abs/2506.13131
- DeepMind AlphaEvolve blog post. https://deepmind.com/discover/blog/alphaevolve-a-gemini-powered-coding-agent-for-designing-advanced-algorithms/
- "DeepMind unveils 'spectacular' general-purpose science AI." *Nature* News, May 2025. https://www.nature.com/articles/d41586-025-01523-z

### Era of Experience (Silver & Sutton)
- Silver, D., Sutton, R. "Welcome to the Era of Experience." Preprint for *Designing an Intelligence* (MIT Press), April 2025. https://storage.googleapis.com/deepmind-media/Era-of-Experience%20/The%20Era%20of%20Experience%20Paper.pdf
- "The Era of Experience has an unsolved technical alignment problem." LessWrong, 2025. https://www.lesswrong.com/posts/TCGgiJAinGgcMEByt/the-era-of-experience-has-an-unsolved-technical-alignment
- Noumenal AI commentary on grounded rewards. https://www.noumenal.ai/post/grounded-rewards-in-the-era-of-experience-a-commentary
- "The Missing Reward: Active Inference in the Era of Experience." arXiv:2508.05619. https://arxiv.org/html/2508.05619v1

### Causal reasoning + LLMs
- "From Causal Parrots to Causal Prophets? Towards Sound Causal Reasoning with Large Language Models." ACL Anthology 2025. https://aclanthology.org/2025.nlp4dh-1.29/
- "Causal Reflection with Language Models." arXiv:2508.04495. https://arxiv.org/html/2508.04495v1
- "Causal Reasoning and Large Language Models: Opening a New Frontier for Causality." arXiv:2305.00050. https://arxiv.org/html/2305.00050v3
- "Executable Counterfactuals." OpenReview, 2025. https://openreview.net/pdf?id=…  (cited in research)
- Vamosi, Forkert. "Dual-Agent Debate for Causal Inference with Reasoning Language Models." arXiv:2511.22854. https://www.arxiv.org/pdf/2511.22854

### Project source documents (read for this analysis)
- `docs/futurebuild/00_INDEX.md` through `29_REWARD_SIGNAL_COMPLETENESS.md`
- `docs/futurebuild/specs/FSV-PROTOCOL.md`
- Git log: commits `07ec7be4`, `554ab5c7`, `3869eb89`, `8f86f879`, `ec67effd`, `62501579`, `86d58d8f`, `702b5d35` (TASK-FP-101..104 + TASK-FP-001..010 ship sequence)
- GitHub EPICs #199, #309, #310, #314, #315 (live phase-G + LeCun-alignment + reward-completeness roadmaps)
- `docs2/AICodingAgentSuperPrompt.md` and `CLAUDE.md` for operator doctrine

---

*This document is an analyst's interpretation produced 2026-05-17 in response to the operator's question. It is not a project deliverable, does not bind any TASK-* or FSV gate, and explicitly declines to predict timelines. It is intended as an intellectually honest mapping between the project as built and the philosophical question the operator posed; corrections that strengthen the argument in either direction are welcome.*
