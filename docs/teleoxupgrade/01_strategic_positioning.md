# 01 — Strategic Positioning

**Authority over:** every line of public copy, every CTA label, every meta description, every section heading.

---

## 1. Primary audience

**Frontier-lab research leadership.** Anthropic-leaning bias, but the targeting must be invisible.

### 1.1 Who counts as "frontier-lab research leadership"

| Lab | Specific people who would matter |
|---|---|
| **Anthropic** | Chris Olah, Catherine Olsson, Sam Bowman, Ethan Perez, Trenton Bricken, Jack Lindsey, Cem Anil, Adam Jermyn, Jan Leike, Jared Kaplan, Dario Amodei |
| **AMI Labs** | Yann LeCun (also NYU), Saining Xie (also NYU), Mike Rabbat, Pascale Fung, Min Lin, Alex LeBrun |
| **OpenAI** | Greg Brockman, Mira Murati, Aleksander Madry, Ilge Akkaya, Igor Babuschkin (xAI now) |
| **DeepMind** | Demis Hassabis, Shane Legg, Vedant Misra, Petar Veličković |
| **Microsoft AI** | Mustafa Suleyman, Karén Simonyan |
| **Perplexity** | Aravind Srinivas, Denis Yarats |
| **xAI** | Igor Babuschkin, Yuhuai (Tony) Wu |
| **NYU CILVR / Frontier Lab** | Yann LeCun, Kyunghyun Cho, Joan Bruna, Rob Fergus, Brenden Lake |
| **Stanford CRFM** | Christopher Ré, Percy Liang |
| **Mila / Vector / etc.** | Yoshua Bengio, Aaron Courville, Geoffrey Hinton |

### 1.2 What this audience reads on a website (in 2 seconds)

They make an instant judgment based on:
1. **Headline** — does this sound like a research operation or a startup pitch?
2. **First sentence** — is there a real intellectual thesis?
3. **Visual restraint** — is the page calm, or is it shouting?
4. **Citations or papers visible** — proof of seriousness
5. **Author identifiability** — is there a real person behind this?

They bounce in 2 seconds if they detect:
- Marketing vocabulary ("AI value," "leverage," "data moat," "transform")
- VC-pitch energy (logos of companies that "trust us," big bold gradients, animated counters)
- Vague safety / AGI gestures
- "Send X data problem"-style CTAs
- Signup walls, popups, cookie consent banners with dark patterns

---

## 2. Secondary audience

**Independent researchers, postdocs, and PhD students** in the JEPA / world-model / SSL / interpretability / training-data orbit. They:
- Spread word organically inside labs
- Are more reachable than principals (lower cost-per-attempt)
- Validate the work before it reaches principals

The site should read as legible to this audience too — they are the warm-introduction source.

---

## 3. The Anthropic-leaning bias (and how to make it invisible)

### 3.1 Why Anthropic specifically

Of the frontier labs, **Anthropic's research culture aligns most directly with the work Teleox publishes**:

| Anthropic theme | Maps to Teleox work |
|---|---|
| Mechanistic interpretability of models | → Mechanistic verifiability of training pipelines |
| Faithful reasoning chains | → Faithful provenance from source to output |
| Constitutional / specification-driven behavior | → Specification-driven verifier (FSV protocol) |
| Process supervision | → Per-step audited execution traces |
| Sparse features / structured representations | → Multi-instrument frozen latent panel |
| Training data attribution + influence | → Cryptographic chain-of-custody for bundles |

Other labs care about *some* of these (DeepMind cares about reproducibility, OpenAI cares about evals, Meta/AMI cares about world models) but Anthropic cares about **all of them simultaneously**. The work fits their identity better than any other lab's.

### 3.2 How the bias stays invisible

**Never** name Anthropic in copy. Never use the words:
- "Constitutional AI"
- "RLHF/RLAIF"
- "Sleeper agent"
- "Reward hacking"
- "Sycophancy"
- "Harmlessness"
- "Helpful, harmless, honest"

Use the **abstracted vocabulary** that Anthropic happens to also use, but which reads as universal:

| Use | Avoid (too Anthropic-tribal) | Avoid (too Meta/OpenAI-tribal) |
|---|---|---|
| verifiable | "constitutional" | — |
| faithful | "honest" | — |
| audit-grade | "interpretable" (alone) | — |
| process-supervised | "step-rewarded" | — |
| provenance | "lineage" (only) | "ancestry" |
| specification | "constitution" | — |
| empirical | "measured-first" | "scaling" |
| commodity hardware | "frontier compute" | "cluster-scale" |

If the homepage hits these words *naturally* in the course of explaining the actual work, an Anthropic-orbit reader pattern-matches positively without ever feeling targeted.

---

## 4. Message hierarchy

A visitor's attention budget is small. Order matters.

### 4.1 What the homepage must communicate, in order

| Position | Message | Example surface |
|---|---|---|
| 1 (above fold, ≤ 2 sec) | "This is a research operation, not a product company." | Hero headline + serif typography + restraint |
| 2 (subhead) | "Specifically, this is research on verifiable AI training pipelines." | One-sentence subhead |
| 3 (first body section) | "Here is the intellectual thesis: pipeline verifiability is achievable even when model interpretability isn't." | First content block |
| 4 (second body section) | "Here is the concrete artifact: bundles are a structured training-data class with cryptographic provenance." | Second content block |
| 5 (third body section) | "Here is the empirical floor: every number is reproducible from a single GPU." | Third content block |
| 6 (CTAs) | "Read the research" + "Email Chris" | Two CTAs, no email-list signup |
| 7 (footer) | Author identity, brief, dignified | Footer |

### 4.2 What each subpage must communicate

| Page | Single thing it must communicate |
|---|---|
| `/research` | "Here is the work, listed and downloadable." |
| `/paper` | "Here is the current paper, in plain language with the technical paper one click away." |
| `/about` | "Chris Royse is a real person doing this work, and here is his prior context." |
| `/contact` | "If you want to talk about the work, here is exactly how." |

---

## 5. Voice and tone

### 5.1 Voice (who is speaking)

The voice is **a single technically-rigorous independent researcher** who:
- Has done the work
- Is honest about what it does and does not solve
- Writes for a peer audience, not a marketing audience
- Knows when to stop talking

Reference voices for tone calibration:
- Chris Olah's blog (warm but rigorous, never marketing-y)
- Distill.pub article voice (precise, reader-respecting)
- Yann LeCun's longer Facebook posts (direct, opinionated, technical)
- Stripe Press / Stripe Engineering blog (measured, no hype, but elegant)

Anti-references:
- a16z founder posts (too declarative)
- VC-pitched startup landing pages (too transformational)
- Any page that uses "transform" as a verb in the hero

### 5.2 Tone (the emotional register)

| Register | Use? | Example |
|---|---|---|
| Calm | YES — primary | "The Dynamic / ME-JEPA Rust runtime is currently maintained as a private codebase." |
| Direct | YES | "The contribution lives in the bundle format, not the predictor weights." |
| Self-effacing | YES, sparingly | "This is not a representational-quality contribution to JEPA." |
| Dry humor | OK, very sparingly | "Bundles of failed runs are persisted as first-class records, not discarded." |
| Confident | YES, when warranted by evidence | "The action-removed ablation collapses to 0.790 — exactly as the design predicts." |
| Excited | NO | (do not use) |
| Promotional | NO | (do not use) |
| Apologetic | NO | (do not hedge) |
| Provocative | RARELY, in headlines only | "Models can be opaque. Pipelines shouldn't be." |

---

## 6. Vocabulary

### 6.1 Words to use freely (positive resonance with target audience)

```
verifiable, verifiability, verifier
audit-grade, auditable, audit log
provenance, chain-of-custody, lineage
faithful, honest about, calibrated
empirical, measured, reported
reproducible, deterministic, byte-equal
training-data class, training-data bundle, structured supervisory signal
process-supervised, per-step labels
mechanistic (when accurate)
specification (when there is a literal specification)
single-GPU, commodity hardware, RTX 5090
JEPA, latent prediction, action-conditioned predictor
multi-instrument, frozen instruments, frozen embedders
counterfactual, ablation, baseline
released as, published as, made available
```

### 6.2 Words to never use (negative resonance / hype-coded)

```
revolutionary, game-changing, paradigm-shifting
breakthrough (unless quoting someone else)
disruptive, disrupts, disruption
unleash, unlock, supercharge, accelerate
transform, transforms, transformation
solve (in the absolute sense — "we solve X")
end (as in "ending model collapse")
finally (as in "finally, AI that can…")
the future of, the next, the new
proudly, excited, thrilled
trusted by, used by [logos]
world-class, world's first, first ever, never before
AGI (almost never — only when explicitly distinguishing)
hallucination (use "factually unsupported output" or be specific)
synthetic data (loaded; use "model-generated data" if needed)
guarantee, guaranteed (legally + scientifically problematic)
"smart", "intelligent" (vague)
"powerful" (vague)
"seamless", "frictionless" (marketing tropes)
"end-to-end" (overused — only use when you literally mean ingest-to-output)
```

### 6.3 The mustNotSay enforcement list (machine-readable)

This list is enforced by `scripts/scan-claims.ts` and `scripts/scan-public-posture.ts`. Adding a phrase to `mustNotSay` in any `RouteCopy` does not just document intent — it fails the build if violated.

```ts
mustNotSay: [
  "guarantees AGI",
  "solves the data wall",
  "model collapse is over",
  "synthetic data replacement",
  "revolutionary",
  "revolutionizes",
  "game-changing",
  "first ever",
  "world's first",
  "world-first",
  "disrupt",
  "unleash",
  "transform AI",
  "the future of AI",
  "end of LLMs",
  "AGI is here",
  "trusted by",   // discouraged unless real attestation links exist
]
```

See `17_validation_and_scanners.md` for the full integration spec.

---

## 7. The single positioning sentence

If forced to summarize Teleox.ai in one sentence:

> *Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware, with cryptographic chain-of-custody from source data to trained weights.*

This sentence:
- Names what it is ("infrastructure")
- Names the technical scope ("JEPA-style and predictive-AI training pipelines")
- Names the property ("verifiable end-to-end")
- Names the constraint ("commodity hardware")
- Names the differentiator ("cryptographic chain-of-custody")
- Does **not** name an audience, a product, a buyer, or a market

It is the canonical reference for whether any other copy on the site is on-message.

---

## 8. The thesis

Beyond positioning, the site has a **thesis** — a defensible intellectual position about the field that anchors the work:

> *Mechanistic interpretability of trained models is a hard, open problem. The verifiability of the pipelines that produced them does not have to be. Most of the safety, reproducibility, and provenance properties researchers want from "interpretable AI" can be obtained at the pipeline layer instead, today, on commodity hardware — without waiting for representation-level interpretability to mature.*

This thesis is what an Anthropic-orbit reader will pattern-match to. It frames pipeline-level verifiability as **complementary** to model-level interpretability, not competing. That is critical for not appearing to dismiss interpretability work.

The thesis appears, slightly rephrased, in:
- The homepage second body block ("Verifiable pipelines, not interpretable models")
- The `/research` page introduction
- The about page mission paragraph

---

## 9. What Teleox is *not*

These framings are explicitly out of scope. The site must not accidentally drift into them:

| Not a | Why this matters |
|---|---|
| Not a SaaS product | We do not sell a hosted service to anyone; the site must not have a "Sign up" or "Start free trial" button |
| Not a consulting firm | We do not pitch hourly research help; we publish work |
| Not an AI safety think tank | We do not take a position on AI policy, x-risk, or governance |
| Not an open-source project | The runtime is private; the public artifacts (clipcannon, ME-JEPA paper) are released under specific licenses |
| Not a competitor to OpenAI / DeepMind / Meta / Anthropic | We are *complementary* infrastructure, not competing models |
| Not a job board | We do not list job openings (yet); if/when we do, that's a separate spec |
| Not a newsletter | No "subscribe to updates" CTAs |
| Not a community / Discord / forum | No social-feature surfaces |

---

## 10. Test the copy

Every paragraph of new copy must pass all four of these tests:

1. **Would Yann LeCun read past this sentence?** If no, rewrite.
2. **Would Chris Olah forward this to a colleague without embarrassment?** If no, rewrite.
3. **Does this contain a verifiable factual claim, or am I gesturing at a feeling?** If gesturing, rewrite.
4. **Could this same sentence appear, unchanged, on a SaaS startup landing page?** If yes, rewrite.

Pass all four → ship.
