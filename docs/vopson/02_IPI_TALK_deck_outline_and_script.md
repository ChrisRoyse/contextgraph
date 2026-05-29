# 02 — IPI Talk: Slide-by-Slide Deck Outline + Speaker Script

**Target:** ~45 min talk + private member Q&A. ~22 slides. Register: rigorous engineer (see `00 §3`).
**Structure mirrors Vopson's own request:** (1) Leapable.ai business model → (2) ME-JEPA-Code / the predictor → (3) the science where infodynamics enters → (4) a testing protocol IPI can scrutinise.
**The whole arc:** *honest problem → honest system → honest current number → his law as the lens → a falsifiable joint experiment.*

Each slide below has a **[SLIDE]** title line, bullet content, and **[SAY]** speaker notes. Bold = on-slide text.

---

### [SLIDE 1] Title
- **The Second Law of Infodynamics in a Learning Machine**
- *Watching information entropy fall as an AI verifier trains itself against reality*
- Chris Royse — Leapable.ai · IPI Talk · [date]

**[SAY]** "Thank you for having me, and thank you Melvin for the invitation. I'm an engineer, not a physicist — so I'm going to show you a working system and the numbers it produces, and then make a falsifiable claim about how those numbers connect to your second law of infodynamics. I'll be explicit about what I've measured and what I haven't."

---

### [SLIDE 2] The honesty contract (put it up front)
- What I will claim: a measurement protocol + a hypothesis.
- What I will **not** claim: that the system is finished, or that I've proven anything about reality.
- Current headline number, stated now: **verifier–oracle correlation = 0.48**, target gate = **0.95**.

**[SAY]** "Melvin's one piece of feedback on my earlier write-up was that it had no data and no data processing in it. He's right, and this talk is my answer. So I'm putting my honesty contract on slide two. I'll give you the real number — which is not yet where it needs to be — and the exact protocol by which the interesting physics claim can be confirmed or killed. If you came for 'I built an oracle,' you'll be disappointed. If you came for a measurable question, stay."

*(This slide is the single most important credibility move in the whole talk. It disarms every skeptic in the room by beating them to the punch.)*

---

### [SLIDE 3] Part 1 — The bottleneck (business context, fast)
- AI agents generate code faster than humans can review it.
- Bottleneck = **verification, not generation.**
- Leapable.ai: an MCP marketplace; the product surface is a verifier agents call directly.

**[SAY]** "Quick business framing because Melvin asked for it. Coding agents now write hundreds of edits an hour; a human reviews maybe one a minute. The constraint is verification. Leapable.ai is a marketplace of tools that AI agents call over a protocol called MCP. The piece I'll talk about, ME-JEPA-Code, is the verifier in that marketplace — it doesn't write code, it predicts whether code will work."

---

### [SLIDE 4] Part 2 — What the system does (one sentence)
- For any AI code change → **Pass | Fail** against a deterministic oracle.
- Plus: a **named failure mode** and the **closest historical precedent**.
- It is a **predictor**, not a generator. No language model inside the prediction loop.

**[SAY]** "In one sentence: for any change an AI proposes, it returns Pass or Fail against ground truth, with a named reason and the nearest past case. Crucially it is *not* generative — there's no LLM inside making things up. It's a measurement device for code."

---

### [SLIDE 5] What is "ground truth" here? (the oracle)
- Oracle = a containerised test harness (SWE-bench Lite, 300 real GitHub issues × 8 controlled mutations = 2,400 cases).
- Deterministic: same code in → same Pass/Fail out.
- This is the analogue of your **measurement apparatus**: it returns a label without human judgement.

**[SAY]** "The oracle is a Docker test harness over 300 real software bugs, each perturbed eight controlled ways. It's deterministic — same input, same verdict, no human in the loop. In your language, it's the measurement apparatus that returns ground truth. The whole game is: can a learned predictor agree with this apparatus 95% of the time?"

---

### [SLIDE 6] The key architectural choice — the constellation
- A change is NOT encoded as one vector.
- It's an **array of ~15 frozen, independent channels** (AST, control-flow, data-flow, types, tests, traces, …).
- Each channel lives in **its own space**. We **never** flatten them into one vector. (Show the array-of-vectors diagram.)
- I call this a **teleological constellation** — like a star constellation, meaning is in the *pattern across independent observations*.

**[SAY]** "Here's the design choice that connects to your work. We don't squash a code change into one embedding. We measure it through ~15 independent frozen channels — its syntax tree, its control flow, its data flow, its types, its tests — and we keep each measurement in its own space. We never average them into one vector; comparing a syntax dimension to a test dimension is, to us, a category error, like comparing a star's brightness to its temperature as if they were the same axis. The object we reason over is the *array* — the constellation."

---

### [SLIDE 7] Why "frozen channels" matters (Wheeler / your frame)
- Channels are **frozen** (no training) — they are *instruments*, not learners.
- The learner is a small head **on top** of the frozen instrument readings.
- This is "it from bit" operationally: structure → measured → predicts outcome.
- Bridge to your fly: behaviour from wiring alone; here, **outcome from structure alone.**

**[SAY]** "The channels are frozen — they don't learn; they just measure. Only a small predictor on top learns. This is close to your 'From Connectome to Cognition' point: the FlyWire fly walks and grooms with no training, purely from its wiring. My analogous claim is weaker and testable: the *structure* of a code change, measured through frozen instruments, should be enough to predict its outcome — without reward shaping. Whether that's true is an empirical question, and that's where your law comes in."

---

### [SLIDE 8] Part 3 — Where infodynamics enters
- The predictor learns **only from its mistakes** (closed loop against the oracle).
- **Hypothesis:** as it trains, the **per-channel information entropy of the constellation falls** and reaches a minimum — *your second law of infodynamics, in a learning machine.*
- Cite: Vopson & Lepadatu, *Second law of infodynamics*, AIP Advances 12, 075310 (2022).

**[SAY]** "Now the physics. The predictor improves only by being wrong against the oracle and correcting — a closed loop with reality. Your second law of infodynamics says: in a closed system of information states, information entropy stays flat or *falls*, bottoming out at equilibrium. My hypothesis is that this is exactly what happens to my constellation as it learns — the information entropy of those channels should fall, monotonically, toward a floor, and it should fall *as the predictor gets more correct.* If that's true, this system is a bench-top instrument for your law in a substrate that's neither a genome nor the cosmos."

---

### [SLIDE 9] The symmetry connection (his 2023 result)
- Your result: **high symmetry ⇔ low information entropy** (preferred states).
- Prediction here: trained constellations should become **more symmetric / lower-entropy** in their channel configuration.
- Testable: measure channel-configuration symmetry vs. training step.

**[SAY]** "You showed that high-symmetry states are preferred because high symmetry means low information entropy — you derived it across atomic orbitals and mathematical symmetries. The matching prediction in my system is that as the predictor learns, the geometry of the channel array should get *more symmetric and lower-entropy.* That's measurable per training window. If it goes the other way, the analogy is dead — and I'll show you that too."

---

### [SLIDE 10] The metric I already log: compression progress
- `CP_Φ(t)` = per-step reduction in the **conditional description length** of the frozen target state.
- This is **information entropy falling over time, in logged units** (`CF_MEJEPA_TRAIN_CERTS`).
- Schmidhuber's "compression progress" = your second law, in my training certificates.

**[SAY]** "I'm not starting from zero on instrumentation. The trainer already logs a quantity I call compression progress: how many fewer bits the predictor needs to describe the target state after each update. That is information entropy decreasing over time, in numbers I already have on disk. It's the dynamic dual of your law. The joint experiment is to add the *per-channel entropy* measurement next to it and check they move together with oracle agreement."

---

### [SLIDE 11] The honest current data (DON'T skip)
- Predictor–oracle Pearson (raw, held-out): **0.4843**
- Brier **0.092**, ECE **0.024** (calibration is decent; correlation isn't there yet)
- Gap to deployment gate: **0.95 − 0.4843 = 0.4657**
- Source: logged evaluation run, sha-pinned.

**[SAY]** "Here's the real number. On held-out data, the predictor correlates with the oracle at 0.48. The gate I need is 0.95. So I'm less than halfway, honestly. The calibration is reasonable — when it says 90% it's roughly right 90% of the time — but the raw agreement isn't there yet. I want you to see this number because the *next* slide is the protocol that tells us whether the infodynamics story is even reachable on this system."

*(Note: do NOT show the 0.79 "rank-lift" number. It's a diagnostic exemplar-similarity transform your own issue log flags as a Goodhart magnifier. If asked, say exactly that — see `07`.)*

---

### [SLIDE 12] The two cheap probes that could falsify everything
- **Probe 1 — panel sufficiency:** measure `I(constellation ; oracle outcome)` in bits. If < ~1 bit, the channels physically can't determine the verdict — no training fixes it.
- **Probe 2 — oracle self-consistency:** run the oracle repeatedly on the same input. If it disagrees with itself, 0.95 is unreachable by definition.
- These are mutual-information / repeatability measurements — squarely in your wheelhouse.

**[SAY]** "Two cheap measurements decide whether any of this is reachable. One: the mutual information between the channel array and the oracle outcome — if the channels don't carry a bit of information about the verdict, no amount of training helps, and I should add channels, not epochs. Two: does the oracle even agree with itself on repeat runs? You can't correlate with an apparatus better than it correlates with itself. These are exactly the kinds of information-theoretic measurements your group does. I'd love a second pair of eyes on them."

---

### [SLIDE 13] Part 4 — The proposed joint experiment
- Measure, across training windows: (a) per-channel information entropy `H_i(t)`, (b) `CP_Φ(t)`, (c) channel-config symmetry, (d) predictor–oracle correlation.
- **Confirm** if `H_i(t)` falls monotonically AND tracks rising correlation.
- **Falsify** if entropy is flat/rising while correlation rises (→ the law doesn't describe this learner).
- Output: a joint IPI Letters / AIP Advances paper, with all logs released.

**[SAY]** "So here's the experiment I'd like to run with you. As the system trains, log four curves: per-channel information entropy, compression progress, channel symmetry, and oracle agreement. The clean confirmation is: entropy falls, symmetry rises, and both track accuracy. The clean falsification is: accuracy rises while entropy stays flat — in which case your law simply doesn't govern this kind of learner, which is itself a publishable result. Either way it's data, processed, with a falsification condition. That's the paper."

---

### [SLIDE 14] Why this is worth a physicist's time
- A learning machine is a **third substrate** for the second law — after digital storage and genomes.
- It's **fully instrumented and replayable** — unlike a genome or the cosmos, you can rerun it.
- If the law holds here, it's the first *engineered, controllable* demonstration; if it fails, it bounds the law's domain.

**[SAY]** "Your law has been shown on digital storage and on RNA. A self-optimising learner would be a third, very different substrate — and unlike a genome or the universe, I can rerun it a thousand times with controlled changes. That makes it an unusually good testbed: controllable, replayable, logged. Positive result extends your law's reach; negative result draws its boundary. Both are worth publishing."

---

### [SLIDE 15] Honest limitations (state them before Q&A)
- Not yet measured: the per-channel entropy trajectory itself (that's the joint work).
- 0.48 ≠ 0.95: the verifier isn't deployable yet.
- The infodynamics link is a **hypothesis**, not a result.
- Single domain (Python), single oracle — scope is narrow on purpose.

**[SAY]** "Limitations, up front. I have not yet measured the entropy trajectory — that's literally the experiment I'm proposing. The verifier is at 0.48, not deployable. The infodynamics connection is a hypothesis with a protocol, not a finding. And it's one language, one oracle — deliberately narrow. I'd rather you hear these from me than ask them as gotchas."

---

### [SLIDE 16] Leapable.ai — the business, briefly
- MCP marketplace: any AI agent can call any tool.
- ME-JEPA-Code is the flagship verifier tool.
- Revenue: usage of verification + marketplace. (Keep this short — he asked, but the science is the draw.)

**[SAY]** "Briefly on the business since Melvin asked: Leapable is a marketplace where AI agents plug into tools over a standard protocol. The verifier is the flagship. The model is straightforward — agents pay to verify and to access marketplace tools. Happy to go deeper in the private Q&A if useful."

---

### [SLIDE 17] What I'd want from a collaboration
- A physicist's scrutiny of the entropy/symmetry measurement design.
- Co-authorship on the joint paper (IPI Letters is a natural home).
- Optionally: connect this to the mass–energy–information program (does a falling-entropy learner have a measurable energy signature? — speculative, flagged as such).

**[SAY]** "What I'd want from working together: your eyes on the measurement design so it's done to a physicist's standard, co-authorship on the result, and — flagged as speculative — a conversation about whether a system that's measurably *reducing* information entropy has any energy signature your mass–energy–information work would predict. I don't know the answer; you would know how to even ask it."

---

### [SLIDE 18] Close
- The bottleneck is verification.
- The system measures code through frozen channels and learns from its mistakes.
- The hypothesis: that's your second law of infodynamics, observable and falsifiable in a machine.
- "Here is the system, here is the number, here is the protocol. Let's measure it together."

**[SAY]** "To close: verification is the real bottleneck in AI; this system attacks it by measuring code through independent frozen channels and learning only from reality. My claim to you is narrow and testable — that the second law of infodynamics is observable in how this thing learns. I've shown you the real number and the protocol. I'd like to run the measurement with you. Thank you — and I'm glad to take questions."

---

## Delivery notes
- **Time budget:** Parts 1–2 (slides 3–7) fast, ~10 min. Part 3 science (8–12) is the heart, ~18 min. Part 4 (13–18) ~12 min. Leave generous Q&A.
- **Tone:** calm, measured, "engineer reporting numbers." No superlatives. Let the *idea* be big; keep the *delivery* flat.
- **The recurring move:** every claim → immediately followed by the measurement that tests it. Do this ~6 times and the physicists relax.
- **If you only nail three slides:** Slide 2 (honesty contract), Slide 11 (the real 0.48), Slide 13 (the falsifiable joint experiment). Those three make you credible.
- Q&A prep + objection handling: `07_QA_prep_and_objections.md`.
