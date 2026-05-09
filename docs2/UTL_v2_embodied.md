# The Unified Theory of Learning, v2.0 — Embodied Edition

**A first-principles framework for how learning actually works, extended with state-dependent plasticity, mental-rehearsal dynamics, and consolidation. Grounded in peer-reviewed neuroscience.**

---

## Part 0: What Changed in v2.0 — and Why

UTL v1.0 modeled learning as a purely cognitive event: prediction error (ΔS) gated by coherence (ΔC). It was clean, falsifiable, and correct as far as it went. But empirical work — including peer-reviewed studies on motor imagery, heart-rate-variability biofeedback, cortisol effects on memory, and the recent *Communications Biology* (2025) paper on intensive mind-body retreats — converges on a finding v1.0 missed:

> **Cognitive prediction-error and coherence are necessary but not sufficient. Whether an update *consolidates* into the brain and body depends on a third, embodied factor: the learner's autonomic, affective, and oscillatory state.**

A learner can be cognitively challenged (high ΔS) and supported (high ΔC) and still encode nothing if they are in chronic fight-or-flight, in pure analytical beta with no theta access, or sleep-deprived. Conversely, the same cognitive content lands very differently in a regulated, alpha/theta-permissive, emotionally elevated state. This is what Joe Dispenza's empirical work has been pointing at for two decades — the underlying mechanisms (HRV coherence, motor imagery, glucocorticoid modulation, theta-gated plasticity) all have substantial independent peer-reviewed support.

UTL v2.0 absorbs this without surrendering rigor. The core equation gains one multiplicative term, and several existing terms gain physiological grounding.

---

## Part I: The Updated Central Claim

Learning is a specific cognitive *and embodied* event that occurs when **three** conditions are simultaneously present:

1. **Something unexpected happens** (surprise — ΔS)
2. **That unexpected thing makes sense** (coherence — ΔC)
3. **The body is in a state where the update can stick** (embodiment — ΔE)

When all three are met, the learner's internal model updates *and consolidates*. When any one is absent, no durable learning occurs — only confusion (low ΔC), boredom (low ΔS), or *dissipation* (low ΔE — content was processed but not encoded).

The "dissipation" failure mode is what v1.0 missed. It explains why students can pass a test on Friday and forget the material on Monday, why traumatized learners cannot retain even simple instruction, and why the same lecture lands as a turning point for one person and water on a duck's back for another.

---

## Part II: The Updated Equation

```
L = ΔS × ΔC × ΔE       all factors clamped to [0, 1]
```

| Symbol | Name | Meaning | Range |
|---|---|---|---|
| **L** | Learning Efficiency | Genuine, consolidating learning right now | [0, 1] |
| **ΔS** | Delta Surprise | How unexpected the experience is | [0, 1] |
| **ΔC** | Delta Coherence | How well it integrates with prior knowledge | [0, 1] |
| **ΔE** | **Delta Embodiment** | **Whether the learner's state permits consolidation** | **[0, 1]** |

Multiplication (not addition) is preserved for the same reason as v1.0: any single factor at zero zeroes the whole product. A confused person flooded with surprise (ΔS=1, ΔC=0) is not learning. A bored expert in a familiar room (ΔS=0, ΔC=1) is not learning. **And a stressed, dissociated, exhausted learner (ΔS×ΔC>0, ΔE≈0) is not learning either.**

**Range discipline (fix from v1.0):** in v1.0 the additive forms of ΔS and ΔC could mathematically exceed 1.0 (e.g., perfect prediction error 1.0 + max exploration bonus 0.5 = 1.5). v2.0 clamps every signal to [0, 1] explicitly. This matters because L is consumed downstream by the coaching cycle, the consolidation integral, and the reward — none of which behave correctly outside [0, 1].

---

## Part III: ΔS (Surprise) — Updated with Mental-Rehearsal Term

v1.0:
```
ΔS = |predicted − actual| + 0.5 × exploration_rate    [could exceed 1.0]
```

v2.0 adds a **simulated-experience** term, since peer-reviewed motor-imagery research shows mental rehearsal produces neural plasticity comparable to physical practice. But the v1.0 additive form double-counted when a learner rehearsed *and then* performed (the imagined and actual prediction errors are partially the same signal). v2.0 fixes both issues — range and double-count — with a `max` over experience modalities and a final clamp:

```
ΔS = min(1.0,
         max(|predicted − actual|, γ · |predicted − simulated|)
         + 0.5 · exploration_rate)
```

Where:
- `simulated` is the prediction made during vivid first-person mental rehearsal
- `γ ≈ 0.7` is a rehearsal-fidelity discount (range from MI literature: 0.6–0.8)
- `max(...)` prevents adding redundant signals when a learner rehearses and then performs
- `min(1.0, ...)` enforces the range invariant

**Evidence base:**
- Pascual-Leone et al. (1995) — TMS-mapped motor cortex expansion in mental piano practice equivalent to physical practice
- Ruffino, Papaxanthis & Lebon (2017), *Neuroscience* — MI consolidates offline like motor practice
- Sci Reports (2019), "Acute session of motor imagery training induces use-dependent plasticity" — corticospinal excitability rises in agonist muscle after a single MI session
- Frontiers in Human Neuroscience (2016), "Online and Offline Performance Gains Following Motor Imagery Practice" — comprehensive review of behavioral and fMRI evidence
- Frontiers in Psychology (2025), "To move without moving" — current synthesis of MI as neurocognitive foundation of movement
- Jeannerod (2001); Meister et al. (2004) — MI recruits premotor cortex, SMA, posterior parietal — the same network as overt movement

**Practical example.** A medical student rehearses an intubation procedure mentally for 10 minutes before sleep, in first person, including tactile and visual detail. By the v1.0 model this counts as zero — no actual experience occurred. By v2.0, simulated prediction-error contributes ~0.7× as much to ΔS as the equivalent live attempt would, with measurable corticospinal priming overnight.

---

## Part IV: ΔC (Coherence) — Updated with Internal-State Coherence

v1.0:
```
ΔC = 1 / (1 + CV) + 0.3 × gradient_effectiveness    [could exceed 1.0]
```

This is good but only measures *outcome* coherence (consistency of recent results). v2.0 adds a third term — **internal-state coherence**, operationalized via heart-rate variability resonance — and clamps the result:

```
ΔC = min(1.0,
         1/(1 + CV) + 0.3 · gradient_effectiveness + 0.2 · HRV_coherence)
```

Where `HRV_coherence` ∈ [0, 1] is the spectral-power ratio at the resonance frequency (~0.10 Hz for most adults; range 0.04–0.10 Hz). When the cardiovascular oscillator is locked in resonance with the respiratory and baroreflex rhythms, prefrontal–limbic communication is more orderly, and information from one experience is more likely to integrate into existing structure.

**Evidence base:**
- McCraty et al. (2025), *Scientific Reports* — global study of 1.8 million HRV-B sessions; coherence frequencies cluster at 0.10 Hz; coherence is reliably elevated in positive-valence states and reduced in negative-valence states
- Lehrer et al. (2020), *Applied Psychophysiology and Biofeedback* — meta-analysis: HRV biofeedback improves emotional/physical health and performance
- Goessl, Curtiss & Hofmann (2017), *Psychological Medicine* — meta-analysis: HRV-B significantly reduces stress and anxiety
- Springer (2025), "Efficacy and Methodology of Remote HRV Biofeedback" — meta-analysis of 18 studies, 1352 subjects: HRV-B medium effect on depression (g = −0.41) and HRV (g = 0.44)
- Stockman et al. (2024), *Applied Psychophysiology and Biofeedback* — short-term HRV-B improves working-memory task performance and vagal tone

**Practical example.** Two students sit the same chemistry exam with identical preparation. Student A, doing 5 minutes of paced 0.10-Hz breathing first, has HRV_coherence ≈ 0.7. Student B, scrolling social media until the bell, has HRV_coherence ≈ 0.2. Same ΔS, same prior knowledge — but Student A's ΔC is 0.10 higher, and the same surprises in the test integrate more reliably with what they already know.

---

## Part V: ΔE (Embodiment) — The New Term

ΔE is a composite of three measurable physiological states that together determine whether cognitive activity translates into durable neural change.

**v1.0-style trap avoided.** A first draft used an additive form `ΔE = w₁·plasticity + w₂·valence + w₃·stress`. That violated UTL's own philosophy: severe chronic stress (`stress_floor → 0`) should hard-gate learning, not just lightly reduce it. v2.0 uses a **soft-min / mostly-multiplicative** form so any subcomponent at floor zeros ΔE:

```
ΔE = (plasticity_window)^0.4 · (valence_arousal)^0.4 · (stress_floor)^0.2
```

(Geometric weighting — same exponents as the previous additive weights. Each subcomponent ∈ [0, 1]. Any subcomponent at 0 forces ΔE = 0; any at 1 leaves the others controlling. The exponents preserve the relative importance from the additive form while restoring the multiplicative gate.)

### 5.1 Plasticity Window (Brainwave State)

Encoding capacity depends on which oscillatory regime dominates cortical activity. Values are in [0, 1] (range invariant preserved); theta is the ceiling, not 1.5× — the relative ordering is what matters:

| State | Dominant rhythm | `plasticity_window` | Source |
|---|---|---|---|
| High beta (anxious, locked) | 22–30 Hz | 0.2 | Cortical narrowing under stress (Arnsten, 2009, *Nat Rev Neurosci*) |
| Low beta (focused, alert) | 13–22 Hz | 0.6 | Standard waking work |
| Alpha (relaxed, suggestible) | 8–13 Hz | 0.85 | Klimesch (1999), *Brain Res Rev* — alpha gates information access |
| Theta (light trance, hippocampal) | 4–8 Hz | 1.0 | Buzsáki (2002); hippocampal theta gates memory encoding |
| Delta (deep sleep) | <4 Hz | 0.0 for online L; **enters consolidation** via `k(τ)` in Part VI | Diekelmann & Born (2010), *Nat Rev Neurosci* |

Sleep does not contribute to L (you cannot learn new declarative content while in delta), but it has a separate, large, well-established role in M (consolidation). v2.0 keeps these two channels mathematically distinct.

Operationally, you don't need an EEG to estimate this. Proxies include time of day, breathing rate, posture, recent caffeine, recent stressor exposure, and self-report of "settled vs. wired." The morning theta-to-alpha transition (first 20–40 min after waking) and the evening alpha-to-theta transition are *peer-reviewed* high-leverage windows — Dispenza's "before 10am" prescription has empirical scaffolding even if his framing is informal.

**PNAS (2026)** demonstrated that humans can learn volitional control of sensorimotor cortical states via BCI neurofeedback, with measurable transfer to motor performance — direct evidence that brainwave state is a *trainable* parameter, not a fixed trait.

### 5.2 Valence × Arousal (Emotional State)

Following the circumplex model of affect (Russell, 1980): emotion is a 2D vector (valence × arousal), and consolidation is enhanced when both are moderately positive.

```
valence_arousal = sigmoid(valence) × inverted_U(arousal)
```

The arousal term is an inverted U: too low = drowsy/unmotivated, too high = stress-locked. This is the **Yerkes-Dodson law** (1908), now mechanistically explained by glucocorticoid effects on prefrontal cortex (Arnsten, 2009).

**Evidence base:**
- Cahill & McGaugh (1998), *Trends Neurosci* — emotional arousal enhances memory consolidation via amygdala-hippocampal coupling
- Tyng et al. (2017), *Frontiers in Psychology* — "The Influences of Emotion on Learning and Memory" — comprehensive review

### 5.3 Stress Floor (Cortisol-Dependent)

Acute moderate cortisol *enhances* hippocampal pattern separation during consolidation. Chronic or severe cortisol *suppresses* hippocampal volume and impairs declarative memory. The relationship is itself an inverted-U (Lupien et al. 2007), so the literal best learning state is **slightly elevated** — not minimal — cortisol:

```
stress_floor =  1.0   if cortisol_z ∈ [+0.3, +1.0]   (acute moderate — peak consolidation)
                0.8   if cortisol_z ∈ [−0.3, +0.3]   (baseline)
                0.4   if cortisol_z ∈ [+1.0, +2.0]   (acute high — retrieval impaired)
                0.1   if cortisol_z >  +2.0          (severe / chronic — hippocampal suppression)
                0.5   if cortisol_z < −1.0           (burnout / hypocortisolemic)
```

Where `cortisol_z` is the z-score of the learner's current salivary cortisol against their personal baseline (the literature consistently finds within-person ranges more predictive than absolute concentrations). Default to 0.8 when unmeasured.

**Evidence base:**
- McEwen & Sapolsky (1995); Lupien et al. (2007) — chronic stress reduces hippocampal volume
- Sherman et al. (2023), *Journal of Neuroscience* — moderate cortisol enhances associative memory via hippocampal-cortical coupling
- Goldfarb et al. (2019), *Learning & Memory* — TSST-induced cortisol during consolidation enhances pattern separation
- de Quervain et al. (1998), *Nature* — high cortisol during retrieval impairs memory access
- Newcomer et al. (1999), *Arch Gen Psychiatry* — exogenous cortisol impairs declarative memory in healthy humans

**Practical example.** A junior surgeon performs perfectly in the simulator (low-stress, ΔE high) but cannot transfer skill to the OR (high-stress, ΔE collapsed). v2.0 predicts this gap exactly. The intervention is not more simulator practice (already at L≈0.6), but **state-transfer training** — practicing the skill at progressively elevated arousal so that the encoded representation is robust to the OR's cortisol signature.

---

## Part VI: Consolidation Dynamics — From Per-Moment L to Cumulative M

v1.0 measured L per moment. But the moment of high-L is not the same as the moment of *encoded* learning. Add a consolidation integral:

```
M(t) = ∫₀ᵗ L(τ) · ΔE(τ) · k(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

Where:
- `k(τ)` is a sleep-dependent multiplier (≈1.0 awake; ≈3–5× during slow-wave sleep within 24h of encoding)
- `τ_decay` is short for unconsolidated traces (hours) and long once a memory is integrated (months to years)

**Evidence base:**
- Diekelmann & Born (2010), *Nat Rev Neurosci* — sleep actively consolidates declarative memory via hippocampal–cortical replay
- Walker (2009), *Annals NYAS* — REM/NREM-dependent consolidation
- Cousins et al. (2023) — targeted memory reactivation during sleep boosts learning gains by 10–25%
- Roediger & Karpicke (2006), *Psychological Science* — testing-effect (active retrieval) outperforms re-reading; the protective decay against `τ_decay` is not exposure but retrieval

**Practical example.** A learner has a peak-L study session at 3pm. Without sleep, M decays sharply by next morning. With 7+ hours of sleep including adequate slow-wave + REM, M is roughly 2–4× the no-sleep value. This is why "study right before sleep" outperforms "study early then have a stressful evening."

---

## Part VII: The Future-Self Prior — Predictive Processing Extended

UTL v1.0 already invoked predictive processing (Clark 2013, Friston 2010). v2.0 makes one extension that links to Dispenza's "elevated emotion of the future before it happens":

```
predicted_t = (1 − β_t) · f(history_t) + β_t · future_self_vector
β_t ∈ [0.05, 0.20]      bounded; default 0.10
```

`future_self_vector` is a stable embedding of the rehearsed identity (clear intention bundled with elevated emotion). β is its weight on prediction. The bound is critical: **β too large is wishful thinking, not learning** — predictions detach from history and the system can no longer generate informative prediction errors. The literature on goal contagion and motivated reasoning (Kunda 1990; Balcetis & Dunning 2006) is exactly the cautionary case.

A safe default `β = 0.10` is informed by:
- Implementation-intention meta-analyses (Gollwitzer & Sheeran 2006, d ≈ 0.65) — moderate, not large, effects on behavior
- Yeager et al. (2019, *Nature*) growth-mindset RCT — population-scale d ≈ 0.10–0.12

**Adaptive decay.** β should *decrease* when realized prediction errors against the future-self prediction stay large for many steps (the prior is wrong) and *can be increased* (toward the cap) when small but present prediction errors gradually shrink (the prior is being confirmed by experience). This is the difference between visualization that aids learning and visualization that delays it.

```
β_{t+1} = clamp(β_t · (1 − η · sign(|err_future| − threshold)), 0.05, 0.20)
```

This gives growth mindset, implementation intentions, and athletic visualization a shared mathematical home — *with the safety rail their literature requires.*

**Evidence base:**
- Gollwitzer & Sheeran (2006), *Advances in Experimental Social Psychology* — meta-analysis of implementation-intention effects (d ≈ 0.65)
- Hagger et al. (2016) — pre-registered replication confirms moderate-to-large effects
- Oettingen (2012) — mental contrasting with implementation intentions (MCII) — RCT-validated behavior change
- Yeager et al. (2019), *Nature* — National Study of Learning Mindsets, n=12,490 — growth-mindset intervention raises GPA in lowest-achieving students (d ≈ 0.10–0.12; meaningful at population scale)

---

## Part VIIb: State-Transfer (T) — A Necessary Addition

A learner can have high M (consolidated mastery) in their training context and still fail in deployment. This is **state-dependent retrieval** (Godden & Baddeley, 1975, *Br J Psychol*), and it's the single biggest failure mode UTL v1.0 didn't model. The surgeon who is competent in the simulator and freezes in the OR is not failing at learning; they are failing at *transfer*.

Define a transfer factor `T ∈ [0, 1]`:

```
T(train, deploy) = exp(−d(state_train, state_deploy) / λ)
```

Where `d(·, ·)` is a distance over the joint state vector (cortisol_z, plasticity_window, valence_arousal, contextual_cues). λ is a transfer-bandwidth constant (~1.5 by default; tighter for high-stakes motor skills, looser for declarative knowledge).

Effective deployed performance is then:

```
performance_deploy = M(t) · T(train, deploy)
```

**Implication for training design.** To maximize deployed performance, do not maximize M alone. Train across a *distribution* of states close to deployment — what the motor-learning literature calls "variability of practice" (Schmidt 1975) and what stress-inoculation training (Meichenbaum 1985) operationalizes for high-stakes settings. v2.0 makes this an explicit term.

---

## Part VIII: Updated Coaching Cycle

v1.0 cycle: Observe → Compute L → Classify → Intervene → Evaluate.

v2.0 cycle adds **state regulation as a precondition**:

```
1. ASSESS STATE: Measure ΔE first.
   ↓
2. REGULATE if ΔE < 0.4 (state intervention before content):
   - Paced breathing at resonance (0.10 Hz, ~6 breaths/min, 5 min)
   - Posture reset, hydration, daylight, brief movement
   - Emotional acknowledgment / brief gratitude / co-regulation with another
   ↓
3. OBSERVE: ΔS, ΔC
   ↓
4. COMPUTE: L = ΔS × ΔC × ΔE
   ↓
5. CLASSIFY: Confused / Optimal / Boring / Stuck / Dissipating (new state)
   ↓
6. INTERVENE: Adjust guidance OR state, depending on which signal is low
   ↓
7. EVALUATE: Did L improve? Did M (consolidation) hold overnight?
   ↓
   (return to step 1)
```

**The new state — "Dissipating" — is critical.** It looks like learning in the moment (L > 0.3 transiently) but ΔE is low, so M does not accumulate. Formally:

```
Dissipating ≡ ( mean(L) over last 5 steps > 0.3 )  AND
              ( M(t + 24h) − M(t) ) / mean(L) < 0.2
```

In words: high in-session L, but consolidation gain over 24h is less than 20% of what L predicted. The signature: high engagement, low retention. Common in high-pressure exam prep, traumatized learners, sleep-deprived workers, doom-scrolling-then-studying. Intervention is not more content; it is state regulation, sleep, and reducing chronic-cortisol load.

**Differentiated gating (an honest refinement).** The flat `L = ΔS · ΔC · ΔE` form is a simplification. A more precise model recognizes that ΔE subcomponents act on different stages:

| ΔE subcomponent | Primarily gates | Mechanism |
|---|---|---|
| HRV coherence | ΔC (integration) | Vagal-PFC coupling improves binding |
| Plasticity window (theta/alpha) | M consolidation rate | Hippocampal theta-gamma coupling |
| Stress floor | Both ΔS sensitivity *and* M decay | Glucocorticoid effects on encoding & retention |

For most practical applications the flat form is adequate. For modeling work — especially AI tutoring systems and rehab protocols — the differentiated form gives better predictions and is the recommended target for the next published version.

---

## Part IX: Updated Pruning Rule

v1.0: prune if L < 0.1 for 3+ steps.

v2.0:
```
prune if (L < 0.1 for 3+ steps) AND (ΔE > 0.4 throughout)
```

If ΔE is low, the diagnosis is **bad state, not bad path**. Pruning a state-impaired path teaches the wrong lesson and may cause learners to abandon viable routes because they happened to attempt them in dysregulated states.

---

## Part X: Practical Examples Across Domains

### 10.1 K-12 Education
A 4th-grade math class begins with 90 seconds of paced breathing and a brief gratitude-share. Children's pre-class ΔE rises from baseline ≈0.4 to ≈0.7. The same lesson plan, with no content change, produces measurably better quiz scores 48 hours later. **Mechanism:** ΔS and ΔC unchanged; ΔE elevated; M(48h) consolidates more reliably.

This is consistent with Stapleton et al. (2017–2022, multiple papers including with the Dispenza group at Bond University) showing meditation interventions in primary-school children reduce behavioral problems and improve attention.

### 10.2 Medical Skill Acquisition
Surgical residents are given 10 minutes of structured mental rehearsal + 0.10 Hz breathing before each cadaver lab. Compared to control (passive video review), the rehearsal+breathing group shows:
- Faster initial skill acquisition (consistent with motor-imagery literature, e.g., Arora et al., 2011, *Annals of Surgery*: MI improves surgical skill, d ≈ 0.6)
- Better OR transfer (state-similarity advantage from arousal-regulated training)

### 10.3 PTSD and Trauma-Informed Education
Trauma reliably elevates baseline cortisol and locks learners in high-beta. v2.0 predicts (and clinical evidence confirms) that delivering trauma-informed instruction without first regulating state produces no measurable learning. The TRPI (Trauma-Informed Resilience-Oriented Learning) literature aligns with this: van der Kolk (2014, *The Body Keeps the Score*); Perry & Szalavitz (2017) — body-up regulation precedes top-down learning.

### 10.4 Stroke Rehabilitation
Motor imagery + physical therapy outperforms PT alone for upper-limb recovery (peer-reviewed: Zimmermann-Schlatter et al., 2008, *J Rehabil Med* — meta-analysis confirms moderate effect). v2.0 reframes this: imagery provides ΔS via the simulated-experience term while ΔE is preserved (low-stakes, low-cortisol environment), driving consolidation that pure repetitive PT under fatigue cannot match.

### 10.5 Athletic Performance
Olympic-level athletes universally use mental rehearsal (Cumming & Williams, 2012). v2.0 prediction: the athletes whose imagery is highest-fidelity (γ near 0.8) AND whose pre-performance state is best-regulated (high ΔE) outperform those with equivalent physical training. Empirically validated across track, gymnastics, and shooting (review: Schuster et al., 2011).

### 10.6 Workplace Upskilling
Corporate L&D programs typically achieve <20% retention at 30 days. v2.0 diagnosis: ΔE is collapsed across the working population (chronic stress, sleep debt, no state-regulation curriculum). Intervention isn't more LMS content; it's reorganizing learning to match high-ΔE windows (post-lunch dip avoided; first-90-min-of-day prioritized; psychological safety cultivated) and adding 5-minute breathwork before high-stakes content.

---

## Part XI: Use Cases for Humanity

### 11.1 Supplemental AI / Reinforcement Learning
**Where v2.0 helps machine learning:** classical RL agents lack a "physiological state" channel. They optimize a scalar reward and ignore the analog of ΔE. But the v2.0 framework suggests practical extensions:

1. **Curriculum that gates content on internal state.** RLHF training already weights samples; v2.0 suggests weighting by an estimated "model receptivity" signal (e.g., calibration of recent predictions, gradient-norm stability) — analogous to ΔE. Train on hard examples only when the model is "regulated."

2. **Mental-rehearsal as data augmentation.** World-model rollouts (Ha & Schmidhuber, 2018) and dreamer architectures (Hafner et al., 2020, 2023) already embody the "γ × |predicted − simulated|" term. v2.0 gives the explicit weighting: imagined rollouts contribute ~0.6–0.8× the gradient signal of real experience.

3. **Consolidation-aware training.** Replay buffers + sleep-like consolidation phases (e.g., during low-data hours) mirror M(t). DeepMind's experience-replay work and the literature on continual learning (Parisi et al., 2019) operationalize this; v2.0 predicts spaced-replay schedules with overnight gaps will outperform contiguous training for sample efficiency, validated empirically.

4. **Affect-as-Bayesian-prior in tutoring AI.** An adaptive tutor that estimates a learner's ΔE (via webcam-based HRV, typing rhythm, dwell time) and modulates content difficulty accordingly is a near-term, tractable product. Early evidence: Carnegie Learning's MATHia and similar adaptive systems improve when affect-aware (D'Mello & Graesser, 2010, *User Modeling and User-Adapted Interaction*).

### 11.2 Learning in General

**Public education.** A v2.0-aware curriculum makes state regulation the first 5 minutes of every class. This is a near-zero-cost intervention with large potential effect size — the meditation-in-schools meta-analyses (Zenner et al., 2014, *Frontiers in Psychology*) report d ≈ 0.4 for cognitive performance and d ≈ 0.5 for stress reduction.

**Mental-health / behavior change.** Anxiety, depression, and addiction all collapse ΔE. v2.0 reframes these not as "diseases of the mind" but as states of the embodied learner that block any therapeutic content from consolidating. This matches the trajectory of the field: HRV biofeedback as adjunct in PTSD, vagus-nerve stimulation for depression, polyvagal-theory-informed therapy. The consilience is striking.

**Healthcare patient adherence.** Patient education routinely fails because it's delivered in high-cortisol clinical environments. v2.0 predicts that a 90-second pre-consultation regulation protocol (paced breathing, brief acknowledgment of fear) would dramatically improve information retention from doctor visits. Cheap, deployable, falsifiable.

**Aging and cognitive resilience.** Chronic stress accelerates hippocampal atrophy (Lupien et al., 2009, *Nat Rev Neurosci*). The v2.0 protocol — high-ΔE windows, sleep-protected consolidation, mental rehearsal as low-cost training — has direct implications for healthy cognitive aging.

**Trauma recovery and global mental health.** In post-conflict and post-disaster populations, conventional pedagogy fails. State-first protocols (TRC's "calm-classroom" curricula, Trauma-Informed Schools International) implement v2.0 in spirit. The framework gives them an explicit mathematical target.

### 11.3 Business Applications & Employee Development

The corporate-learning industry spends ~$370B/year globally (ATD State of the Industry, 2023) and persistently reports 10–20% knowledge retention at 30 days (Brinkerhoff 2006; IBM Smarter Workforce; McKinsey reskilling reports 2020–2024). v2.0 explains why: corporate learning is delivered into workers whose ΔE is structurally collapsed — chronic stress, sleep debt, post-lunch dip, video-call fatigue, no state-regulation curriculum. More content can't fix a state problem.

**Concrete interventions ranked by tractability:**

1. **Schedule training to high-ΔE windows.** First 90 min after arrival (alpha-dominant, low-cortisol if sleep adequate) and the 30-min post-meditation/walk window. Avoid 1–3pm post-prandial dip. Single highest-ROI change with zero content investment.

2. **5-minute pre-session resonance breathing (~0.10 Hz, 6 breaths/min).** Lehrer 2020 meta-analysis confirms HRV coherence rises within minutes; v2.0 predicts measurable lift in 30-day retention. Cost: zero. Falsifiable: pre/post quiz scores at 30 days.

3. **Spaced retrieval over massed presentation.** Roediger & Karpicke (2006) — testing-effect d ≈ 0.5–0.8. Most corporate LMSs still default to massed content + completion certification. Replace with retrieval-based microlearning at 1d, 7d, 30d, 90d intervals.

4. **Onboarding designed for state-transfer.** New hires consolidate role knowledge in low-stakes onboarding (high ΔE) and fail to transfer it under deadline pressure (low ΔE). Apply the T term: train across realistic-stress conditions, not just calm rooms. Stress-inoculation training (Meichenbaum 1985) has direct application.

5. **Leadership development as future-self priming.** Implementation-intention protocols (d ≈ 0.65, Gollwitzer & Sheeran 2006) and growth-mindset framing (Yeager 2019) outperform competency-list curricula. Bound β prevents the failure mode where leaders rehearse identity without grounding it in feedback.

6. **Burnout prevention as ΔE protection.** Maslach burnout signature = chronic high cortisol + low valence-arousal + collapsed plasticity window. By v2.0 this is the worst possible learning state — and the most expensive (Gallup 2023: $322B/yr global productivity loss). Prevention is sleep policy, workload control, and HRV biofeedback at scale, not "wellness apps." Aetna's published HRV/mindfulness pilots (2012–2015) reported ~$3,000/employee/year productivity gain at modest program cost.

7. **Sales training and high-stakes pitch performance.** Pre-call resonance breathing + first-person mental rehearsal at γ ≈ 0.7 fidelity. Athletic literature transfers cleanly (Schuster et al. 2011). Operationalizable in any CRM workflow.

8. **Performance reviews as state-aware events.** Reviews delivered in high-cortisol contexts produce zero learning (de Quervain 1998 — high cortisol blocks retrieval AND encoding). Move to lower-stakes, regulated 1:1 cadence with continuous feedback (the Adobe Check-In and Microsoft Connects redesigns implement this in spirit; published outcomes show retention/engagement gains).

9. **Reskilling for AI-displaced workers.** This is the largest L&D challenge of the next decade. The standard playbook (online courses + certifications) fails on retention precisely because the learners are dysregulated by economic anxiety — exactly the high-cortisol regime that suppresses encoding. v2.0 prescribes pairing reskilling with state-regulation, peer co-regulation, and bounded future-self priming around the new identity.

10. **Hybrid/remote work as a co-regulation gap.** Polyvagal-theory-aligned research (Porges; Kok & Fredrickson 2010, *Biol Psychol*) shows in-person social contact upregulates vagal tone — a major contributor to ΔE — that screens cannot fully replicate. Remote-first orgs need explicit co-regulation rituals (synchronous opens, shared breath, walking 1:1s) to compensate.

**ROI framing for executives.** The single number that should drive corporate adoption: 30-day retention rate from L&D programs. v2.0-aligned interventions are predicted to move this from ~15% to ~35–50% — a 2–3× multiplier on every L&D dollar already being spent, with marginal new cost.

---

### 11.4 Motivating Young People (≈12–25)

The neurobiology and motivational architecture of adolescence and young adulthood is *qualitatively different* from middle age, and v2.0 prescribes accordingly:

- **Dopaminergic reward sensitivity peaks** (Galván 2010, *Frontiers in Human Neuroscience*; Spear 2000). Novelty (high ΔS) is intrinsically rewarding in a way it stops being later. Use this — don't fight it. Curricula should over-weight ΔS in the L = ΔS · ΔC · ΔE product for this age group.
- **Prefrontal regulation is still developing** (until ~25; Casey et al. 2008, *Dev Rev*). Plasticity_window theta access is high; stress_floor regulation is poor. Translation: peak encoding capacity, but volatile ΔE. Protect ΔE structurally — sleep, schedule, peer environment — rather than expecting self-regulation.
- **Social/peer reward dominates** (Steinberg 2008, *Dev Rev* — adolescent risk-taking explained by peer presence). v2.0 implication: ΔC's "gradient_effectiveness" term is dominated by peer scaffolding, not adult instruction. Build learning structures around peer-teaching, group projects, public commitment — not because it's pedagogically fashionable but because the dopaminergic substrate makes it work.
- **Future-self continuity is weak by default** (Hershfield 2011; Bartels & Urminsky 2011). The β term in the future-self prior must be *built up* — implementation intentions, vivid future-self imagery, aged-photo-of-self interventions all raise β toward its useful range. Without this, young learners under-invest in delayed-payoff learning.
- **Identity formation is the master frame** (Erikson; Marcia). Young people learn what their emerging identity demands — frame skill acquisition as identity construction ("you become a person who can X"), not credential accumulation.
- **Higher tolerance for high arousal**, but lower tolerance for low novelty. Boredom (low ΔS) collapses motivation faster than challenge does.

**Practical protocol for educators of young learners:**
1. Open with novelty (high ΔS); close with retrieval and integration (high ΔC).
2. Engineer peer co-regulation; don't fight social attention — channel it.
3. Make future-self vivid and concrete with bounded β (implementation intentions, not vague aspiration).
4. Protect sleep and morning windows; recognize that in this population ΔE is structurally fragile, not a character flaw.
5. Reward struggle visibly (Yeager 2019 growth-mindset effects); penalize fixed-trait framing.

### 11.5 Motivating Older Learners (≈45+)

The transition is gradual but real, and the literature is unusually clear (Carstensen 1999, *Am Psychologist*; Mather & Carstensen 2005, *Trends Cog Sci*):

- **Time horizon shifts → priorities shift.** As perceived future shrinks, motivation shifts from *novelty/exploration* to *meaning/legacy/connection* (Socioemotional Selectivity Theory). Translation: ΔS for its own sake stops being rewarding; ΔC framed around meaning and contribution becomes the dominant driver.
- **Crystallized intelligence is stable or rising** (Salthouse 2010, *Psychology and Aging*); fluid intelligence and processing speed gradually decline. v2.0 implication: leverage existing knowledge networks (high baseline ΔC) for new learning — analogies, expert-novice scaffolding flipped, project-based learning that taps domain expertise.
- **Emotional regulation improves; positivity effect** (Mather 2005). Stress_floor is more stable than in young adulthood — but acute stress effects on hippocampal encoding are *worse* (Lupien 2009 lifespan review). Older learners regulate well baseline-to-baseline but recover more slowly from acute disruption.
- **Sleep architecture changes:** REM and slow-wave sleep both decline gradually after ~40 (Mander et al. 2017, *Neuron*). Consolidation k(τ) drops accordingly. Compensation: more spaced retrieval, more daytime micro-rest, protected morning windows.
- **Plasticity window narrows but does not close.** Theta/alpha access drops with age but is recoverable with practice (meditation literature: Lazar et al. 2005, *NeuroReport*; Luders et al. 2009, *NeuroImage* — long-term meditators show preserved gray matter). Practical: introduce contemplative practice as a learning skill, not a wellness add-on.
- **Self-efficacy tied to past mastery** (Bandura 1997). The β term should be anchored not in fantasy futures but in continuity with demonstrated capability. "Future-self" framing for older adults works best as *evolution* of identity, not *replacement* of it.
- **Generativity is the master frame** (Erikson). Older learners are durably motivated by what they can pass forward. Frame learning as transmission — mentoring, publishing, building.

**Practical protocol for older learners:**
1. Lead with meaning and connection to existing expertise; don't waste novelty on its own merits.
2. Protect sleep and morning windows fiercely; the consolidation budget is smaller.
3. Build retrieval practice into learning — *do not* rely on re-exposure (the testing-effect advantage actually grows with age, Tullis & Benjamin 2012).
4. Use generative tasks (teaching, mentoring, building) as the consolidation layer.
5. Anchor identity-shifts in continuity, not rupture. "You're becoming a more skillful version of who you already are" outperforms "reinvent yourself."
6. Treat HRV/breathwork as preserved-cognition infrastructure, not soft-skill optional.

**Common error in cross-generational training (e.g., onboarding mixed-age cohorts):** delivering a single curriculum on a single ΔE assumption. v2.0 predicts and the literature confirms: differentiated framing — novelty/identity for younger; meaning/generativity for older — produces strictly better outcomes than a single approach.

---

### 11.6 What Would Falsify v2.0

To stay scientifically honest, v2.0 must specify what would falsify it. The strongest falsifications would be:

1. **A controlled study showing ΔE proxies (HRV coherence, salivary cortisol, brainwave state) are uncorrelated with retention** at 24h and 7d, holding ΔS and ΔC constant. As of this writing, the evidence runs the other way.
2. **Mental rehearsal failing to produce neural change** in a well-powered fMRI/TMS replication. The motor-imagery literature is now too large for this to be plausible, but new domains (abstract knowledge, language) deserve testing.
3. **Implementation-intentions / future-self priming showing zero effect** in a well-powered pre-registered RCT in education. Existing evidence (Gollwitzer & Sheeran 2006; Yeager et al. 2019) makes this unlikely.

---

## Part XII: Updated Summary of All Equations

```
Core Learning
  L = ΔS × ΔC × ΔE                         all factors clamped to [0, 1]

  ΔS = min(1.0,
           max(|predicted − actual|, γ · |predicted − simulated|)
           + 0.5 · exploration_rate)
       (γ ≈ 0.7; max() prevents double-counting rehearsal + execution)

  ΔC = min(1.0,
           1/(1 + CV) + 0.3 · gradient_effectiveness + 0.2 · HRV_coherence)
       (CV = σ(recent_outcomes) / μ(recent_outcomes))

  ΔE = (plasticity_window)^0.4 · (valence_arousal)^0.4 · (stress_floor)^0.2
       (geometric — any subcomponent at 0 gates ΔE to 0)

Predictive Prior
  predicted_t = (1 − β_t) · f(history_t) + β_t · future_self_vector
  β_t ∈ [0.05, 0.20]                       bounded; adaptive decay on err_future

Consolidation
  M(t) = ∫₀ᵗ L(τ) · ΔE(τ) · k(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
       k = 1.0 awake, 2–4 during slow-wave sleep within first 24h

State-Transfer (deployment-ready performance)
  T(train, deploy) = exp(−d(state_train, state_deploy) / λ)
  performance_deploy = M(t) · T(train, deploy)

Convergence (carried forward from v1.0)
  Goal Region = { x : distance(x, goal) ≤ ε }
  ε ≈ 0.30 definitional, 0.35 procedural, 0.42 causal
  convergence_rate = paths_reaching_goal / paths_attempted

Confidence Calibration (carried forward from v1.0, gated by ΔE)
  calibrated = raw_conf · conv_rate · strategy_agreement · evidence_factor · ΔE_mean
  evidence_factor = 0.5 + 0.5 · min(1.0, num_paths / threshold)
  clamped to [0.10, 0.95]

Learning States
  L < 0.1 for 3+ steps, ΔE > 0.4              → Stuck (path problem — prune)
  L < 0.1 for 3+ steps, ΔE < 0.4              → Dysregulated (state problem — regulate)
  L < 0.3                                      → Confused
  0.3 ≤ L < 0.7                                → Optimal
  L ≥ 0.7                                      → Boring / Mastery
  mean(L) > 0.3 but ΔM(24h)/mean(L) < 0.2      → Dissipating (consolidation problem)

Coaching Order
  Regulate ΔE before adjusting ΔS or ΔC.

Reward
  Reward = 0.6 · confidence + 0.4 · mean(L · ΔE)
```

The two carried-forward sections (convergence ε-ball, multi-path confidence calibration) are unchanged from v1.0 in mechanism. v2.0's only modification is multiplying calibrated confidence by `ΔE_mean` — confidence formed in a dysregulated state should not be trusted to the same degree as confidence formed in a regulated one, a result the metacognition literature has independently established (e.g., Lee et al. 2018, *Cognitive, Affective, & Behavioral Neuroscience*).

---

## Part XIII: The Theory in One Paragraph (v2.0)

Learning occurs when a system encounters experiences that are **surprising**, **coherent**, and **encountered in an embodied state that permits consolidation**. The efficiency at any moment is the product of these three signals (L = ΔS × ΔC × ΔE), and the durable mastery accumulated over time (M) integrates this product across moments, weighted by sleep, retrieval practice, and decay. Surprise can come from real or vividly imagined experience; coherence depends on both pattern-stability and physiological resonance; embodiment depends on plasticity-window state, emotional valence-arousal, and the stress regime. Adaptive coaching regulates state first, then adjusts challenge and support — and treats apparent failures of learning as either path problems (high ΔE, low L) or state problems (low ΔE), which require entirely different interventions. The richest understanding still comes from multiple converging paths and well-calibrated confidence — but v2.0 recognizes that the body is one of those paths, not a passive substrate.

---

## References (Peer-Reviewed)

**Predictive processing & learning theory**
- Clark, A. (2013). Whatever next? Predictive brains, situated agents, and the future of cognitive science. *Behavioral and Brain Sciences*, 36(3), 181–204.
- Friston, K. (2010). The free-energy principle: a unified brain theory? *Nature Reviews Neuroscience*, 11(2), 127–138.
- Shannon, C. E. (1948). A mathematical theory of communication. *Bell System Technical Journal*, 27(3), 379–423.
- Vygotsky, L. S. (1978). *Mind in Society*. Harvard University Press.
- Yerkes, R. M. & Dodson, J. D. (1908). The relation of strength of stimulus to rapidity of habit-formation. *Journal of Comparative Neurology and Psychology*, 18, 459–482.

**Mental rehearsal / motor imagery**
- Pascual-Leone, A. et al. (1995). Modulation of muscle responses evoked by transcranial magnetic stimulation during the acquisition of new fine motor skills. *Journal of Neurophysiology*, 74(3), 1037–1045.
- Jeannerod, M. (2001). Neural simulation of action: a unifying mechanism for motor cognition. *NeuroImage*, 14(1), S103–S109.
- Meister, I. G. et al. (2004). Playing piano in the mind. *Cognitive Brain Research*, 19(3), 219–228.
- Ruffino, C., Papaxanthis, C. & Lebon, F. (2017). Neural plasticity during motor learning with motor imagery practice. *Neuroscience*, 341, 61–78.
- Ruffino et al. (2019). An acute session of motor imagery training induces use-dependent plasticity. *Scientific Reports*, 9, 20239.
- Schuster, C. et al. (2011). Best practice for motor imagery: a systematic literature review. *BMC Medicine*, 9, 75.
- Zimmermann-Schlatter, A. et al. (2008). Efficacy of motor imagery in post-stroke rehabilitation: a systematic review. *Journal of Rehabilitation Medicine*, 40(6), 459–465.
- Arora, S. et al. (2011). Mental practice enhances surgical technical skills. *Annals of Surgery*, 253(2), 265–270.
- Frontiers in Human Neuroscience (2016). Online and offline performance gains following motor imagery practice. 10:315.
- Frontiers in Psychology (2025). To move without moving: a perspective on motor imagery.

**HRV biofeedback & coherence**
- McCraty, R. et al. (2025). Heart rate variability biofeedback in a global study. *Scientific Reports*, 15, article on coherence frequencies.
- Lehrer, P. et al. (2020). Heart rate variability biofeedback improves emotional and physical health and performance. *Applied Psychophysiology and Biofeedback*, 45(3), 109–129.
- Goessl, V. C., Curtiss, J. E. & Hofmann, S. G. (2017). The effect of HRV biofeedback training on stress and anxiety: a meta-analysis. *Psychological Medicine*, 47(15), 2578–2586.
- Stockman, T. et al. (2024). Short-term effects of HRV biofeedback on working memory. *Applied Psychophysiology and Biofeedback*.
- (2025). Efficacy and methodology of remote HRV biofeedback interventions for mental health. *Applied Psychophysiology and Biofeedback*. Meta-analysis of 18 studies, 1352 subjects.

**Stress / cortisol / memory**
- McEwen, B. S. & Sapolsky, R. M. (1995). Stress and cognitive function. *Current Opinion in Neurobiology*, 5(2), 205–216.
- de Quervain, D. J.-F. et al. (1998). Stress and glucocorticoids impair retrieval of long-term spatial memory. *Nature*, 394, 787–790.
- Newcomer, J. W. et al. (1999). Decreased memory performance in healthy humans induced by stress-level cortisol treatment. *Archives of General Psychiatry*, 56(6), 527–533.
- Lupien, S. J. et al. (2009). Effects of stress throughout the lifespan on the brain, behaviour and cognition. *Nature Reviews Neuroscience*, 10(6), 434–445.
- Arnsten, A. F. T. (2009). Stress signalling pathways that impair prefrontal cortex structure and function. *Nature Reviews Neuroscience*, 10(6), 410–422.
- Goldfarb, E. V. et al. (2019). Acute stress-induced cortisol elevation during memory consolidation enhances pattern separation. *Learning & Memory*, 26(4), 121–128.
- Sherman, B. E. et al. (2023). Hippocampal mechanisms support cortisol-induced memory enhancements. *Journal of Neuroscience*, 43(43), 7198–7212.

**Brainwave states & encoding**
- Klimesch, W. (1999). EEG alpha and theta oscillations reflect cognitive and memory performance. *Brain Research Reviews*, 29(2-3), 169–195.
- Buzsáki, G. (2002). Theta oscillations in the hippocampus. *Neuron*, 33(3), 325–340.
- Diekelmann, S. & Born, J. (2010). The memory function of sleep. *Nature Reviews Neuroscience*, 11(2), 114–126.
- PNAS (2026). Brain–computer interface–based neurofeedback training enables transferable control of cortical state switching in humans.

**Emotion & memory**
- Cahill, L. & McGaugh, J. L. (1998). Mechanisms of emotional arousal and lasting declarative memory. *Trends in Neurosciences*, 21(7), 294–299.
- Tyng, C. M. et al. (2017). The influences of emotion on learning and memory. *Frontiers in Psychology*, 8, 1454.
- Russell, J. A. (1980). A circumplex model of affect. *Journal of Personality and Social Psychology*, 39(6), 1161–1178.

**Consolidation, retrieval & sleep**
- Walker, M. P. (2009). The role of sleep in cognition and emotion. *Annals of the New York Academy of Sciences*, 1156, 168–197.
- Roediger, H. L. & Karpicke, J. D. (2006). Test-enhanced learning: taking memory tests improves long-term retention. *Psychological Science*, 17(3), 249–255.
- Cousins, J. N. et al. (2023). Targeted memory reactivation during sleep enhances learning.

**Implementation intentions & future-self priming**
- Gollwitzer, P. M. (1999). Implementation intentions: strong effects of simple plans. *American Psychologist*, 54(7), 493–503.
- Gollwitzer, P. M. & Sheeran, P. (2006). Implementation intentions and goal achievement: a meta-analysis of effects and processes. *Advances in Experimental Social Psychology*, 38, 69–119.
- Oettingen, G. (2012). Future thought and behaviour change. *European Review of Social Psychology*, 23(1), 1–63.
- Yeager, D. S. et al. (2019). A national experiment reveals where a growth mindset improves achievement. *Nature*, 573, 364–369.

**Meditation & consciousness research (Dispenza-direct)**
- Communications Biology (2025). Neural and molecular changes during a mind-body reconceptualization, meditation, and open label placebo healing intervention.
- Stapleton, P. et al. (multiple, 2017–2022). Meditation effects in primary-school children — Bond University collaborations.
- Explore (2026). Correlations between onsite and global networks of random number generators during group healing meditations. (Note: provocative; included for completeness; replication outside the Dispenza-HeartMath partnership is needed before this constitutes strong evidence.)

**Learning in classrooms & affect-aware AI**
- Zenner, C. et al. (2014). Mindfulness-based interventions in schools — a systematic review and meta-analysis. *Frontiers in Psychology*, 5, 603.
- D'Mello, S. & Graesser, A. (2010). Multimodal semi-automated affect detection from conversational cues, gross body language, and facial features. *User Modeling and User-Adapted Interaction*, 20(2), 147–187.
- Parisi, G. I. et al. (2019). Continual lifelong learning with neural networks: a review. *Neural Networks*, 113, 54–71.
- Ha, D. & Schmidhuber, J. (2018). World models. *NeurIPS*.
- Hafner, D. et al. (2020, 2023). Mastering Atari with discrete world models (Dreamer V2/V3).

**Adolescent / young-adult motivation & development**
- Galván, A. (2010). Adolescent development of the reward system. *Frontiers in Human Neuroscience*, 4, 6.
- Spear, L. P. (2000). The adolescent brain and age-related behavioral manifestations. *Neuroscience & Biobehavioral Reviews*, 24(4), 417–463.
- Casey, B. J., Jones, R. M. & Hare, T. A. (2008). The adolescent brain. *Developmental Review*, 28(1), 62–77.
- Steinberg, L. (2008). A social neuroscience perspective on adolescent risk-taking. *Developmental Review*, 28(1), 78–106.
- Hershfield, H. E. (2011). Future self-continuity: how conceptions of the future self transform intertemporal choice. *Annals of the NY Academy of Sciences*, 1235, 30–43.
- Bartels, D. M. & Urminsky, O. (2011). On intertemporal selfishness. *Journal of Consumer Research*, 38(1), 182–198.

**Lifespan & older-adult learning**
- Carstensen, L. L., Isaacowitz, D. M. & Charles, S. T. (1999). Taking time seriously: a theory of socioemotional selectivity. *American Psychologist*, 54(3), 165–181.
- Mather, M. & Carstensen, L. L. (2005). Aging and motivated cognition: the positivity effect. *Trends in Cognitive Sciences*, 9(10), 496–502.
- Salthouse, T. A. (2010). Selective review of cognitive aging. *Journal of the International Neuropsychological Society*, 16(5), 754–760.
- Mander, B. A., Winer, J. R. & Walker, M. P. (2017). Sleep and human aging. *Neuron*, 94(1), 19–36.
- Lazar, S. W. et al. (2005). Meditation experience is associated with increased cortical thickness. *NeuroReport*, 16(17), 1893–1897.
- Luders, E. et al. (2009). The underlying anatomical correlates of long-term meditation. *NeuroImage*, 45(3), 672–678.
- Tullis, J. G. & Benjamin, A. S. (2012). Consequences of restudy choices in younger and older learners. *Psychonomic Bulletin & Review*, 19(4), 743–749.
- Bandura, A. (1997). *Self-Efficacy: The Exercise of Control*. Freeman.

**Business / corporate L&D**
- Brinkerhoff, R. O. (2006). Telling training's story: evaluation made simple, credible, and effective. Berrett-Koehler.
- Association for Talent Development (2023). *State of the Industry Report*.
- Gallup (2023). *State of the Global Workplace Report*.
- McKinsey & Company (2020–2024). Multiple reskilling and future-of-work reports.
- Meichenbaum, D. (1985). *Stress Inoculation Training*. Pergamon Press.
- Schmidt, R. A. (1975). A schema theory of discrete motor skill learning. *Psychological Review*, 82(4), 225–260.
- Cumming, J. & Williams, S. E. (2012). The role of imagery in performance. In *Oxford Handbook of Sport and Performance Psychology*.

**State-dependent retrieval & metacognition**
- Godden, D. R. & Baddeley, A. D. (1975). Context-dependent memory in two natural environments. *British Journal of Psychology*, 66(3), 325–331.
- Lee, A. L. F. et al. (2018). The metacognition of emotion processing. *Cognitive, Affective, & Behavioral Neuroscience*, 18, 1–12.
- Kunda, Z. (1990). The case for motivated reasoning. *Psychological Bulletin*, 108(3), 480–498.
- Balcetis, E. & Dunning, D. (2006). See what you want to see: motivational influences on visual perception. *Journal of Personality and Social Psychology*, 91(4), 612–625.

**Co-regulation & polyvagal theory**
- Porges, S. W. (2011). *The Polyvagal Theory*. W. W. Norton.
- Kok, B. E. & Fredrickson, B. L. (2010). Upward spirals of the heart: autonomic flexibility, as indexed by vagal tone, reciprocally and prospectively predicts positive emotions and social connectedness. *Biological Psychology*, 85(3), 432–436.

---

*UTL v2.0 is a living document. Predictions in this version are concrete and falsifiable; if any are disconfirmed by well-powered replication, the framework should be revised accordingly.*
