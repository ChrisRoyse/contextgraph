# 08 — `/paper` Page Specification

**Route:** `/paper`
**File:** `src/app/paper/page.tsx`
**Content:** `src/content/routes/paper.ts` (REPOSITION existing file)
**Audience:** Researchers and technical leadership wanting the technical paper itself
**Single thing it must communicate:** "Here is the current paper. Read the plain-language summary, then the technical paper."

---

## 1. Page meta

| Field | Value |
|---|---|
| `title` | `Dynamic / ME-JEPA — Teleox.ai` |
| `description` | `The current Teleox research paper: an audited, domain-portable JEPA runtime on a single RTX 5090, with a new class of training data.` |
| `og:title` | `Dynamic / ME-JEPA` |
| `og:description` | `Verifiable JEPA pipeline on a single RTX 5090.` |
| `og:image` | `/og-image.png` |
| `canonical` | `https://teleox.ai/paper` |

---

## 2. Page structure (top to bottom)

```
[ Header ]
[ Hero (eyebrow + h1 + subhead) ]
[ Section: Abstract (plain language) ]
[ Section: Headline numbers ]
   [ ResultsBox ]
[ Section: What's actually new ]
[ Section: Reproducibility ]
[ Section: How to access ]
[ CTAs (Email Chris + See research index) ]
[ Footer ]
```

Container width: `reading` (660px max).

---

## 3. Content (verbatim copy)

### 3.1 Eyebrow

```
Current paper
```

### 3.2 H1

```
Dynamic / ME-JEPA
```

### 3.3 Subhead

```
An audited, domain-portable JEPA runtime on a single RTX 5090, with a new class of training data. Draft preprint, May 2026; not yet posted publicly.
```

### 3.4 Section: Abstract (plain language)

**Heading:** `What the paper does`

**Body:**

```
Most JEPA work emphasizes representational quality on natural images or video.
This paper takes a different angle: what does it take to make every step of a
JEPA runtime — data ingest, multi-instrument latent panel materialization,
action-conditioned training, planning, surprise recording, and artifact
provenance — physically auditable end-to-end on a single commodity GPU?

Dynamic / ME-JEPA is one Rust binary that does all of it. New domains ship as a
TOML manifest. Every operation writes typed rows into 25 RocksDB column families
plus an append-only audit log, and a structurally separate verifier reads
everything back. The same code path runs three domains: counter_world (480
trainable parameters), gridworld_5x5 (16,960), and a new career_taxonomy.v1
public-taxonomy benchmark (15,344) derived from O*NET 30.2 starting at Software
Developers (15-1252.00).
```

### 3.5 Section: Headline numbers

**Heading:** `Headline numbers`

**Body (1 sentence intro + ResultsBox):**

```
Five seeds of paper_small_1k (1,000 events each) on an RTX 5090:
```

**ResultsBox:**

```
Career-taxonomy validation cosine: 0.905 ± 0.044 across 5 seeds (BCa 95% CI [0.861, 0.931])
Test cosine: 0.868 ± 0.068 (BCa [0.806, 0.912])
Validation action-contrast accuracy: 0.960 ± 0.043
Action-removed (A3) ablation: collapses to validation cosine 0.790 — exactly as the action-conditioned predictor design requires.
```

### 3.6 Section: What's actually new

**Heading:** `What's actually new`

**Body (2 paragraphs):**

```
The paper is not a representational-quality contribution to the JEPA family. The
premise of latent prediction is taken as established (I-JEPA, V-JEPA, LeWorldModel,
LeCun's vision paper). The contribution is one level up: the infrastructure that
makes a complete JEPA runtime physically auditable, end-to-end, on a single GPU.

The second contribution is conceptual. Each immutable bundle the runtime produces
is itself a structured training-data artifact: most bytes are deterministic
functions of a public source ZIP; the small model-output slice is paired with the
observed next-state and with random-init / shuffled-target baselines in the same
record; counterfactual ablations and failed runs are persisted as first-class
records. The argument is that bundles of this shape form a new class of training
data — audited execution traces with cryptographic provenance — distinct from
web text, pure model-generated synthetic data, and rejection-sampled chain-of-
thought.
```

### 3.7 Section: Reproducibility

**Heading:** `Reproducibility`

**Body (2 paragraphs):**

```
The release reference is host-specific to RTX 5090 sm_120 / CUDA 13.2 / Rust
1.95. Cross-host portability is future work. The CI gate is local and self-
hosted to keep evidence on disk; a reviewer without RTX 5090 access cannot run
the full release-C gate.

The reproducibility artifact is a deterministic Docker tar (SHA-256
099f0535…d80e), an Apptainer SIF (SHA-256 30b531c7…2825), and a release archive
mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz (SHA-256 8cccf498…3ecd).
The career-taxonomy data substrate uses O*NET 30.2 under CC BY 4.0; the source
ZIP SHA-256 and per-edge walk hash are recorded in every generated bundle.
```

### 3.8 Section: How to access

**Heading:** `How to access`

**Body (2 paragraphs):**

```
The paper has not been posted to arXiv yet. It is being shared privately with a
small set of researchers whose work it is in conversation with, before any
public post. If you are in that orbit and would like a copy, write to
chrisroyseai@gmail.com.

The Dynamic / ME-JEPA Rust runtime itself is a private codebase. Reviewers
requiring source-level access for verification can request a time-bounded
private grant via the same email. The reproducibility artifact described in the
paper is the canonical reproducible release; reviewers with an RTX 5090 host
can reproduce every headline number from the artifact alone, without source
access.
```

---

### 3.9 CTA pair

```
Primary:   Email Chris            →   mailto:chrisroyseai@gmail.com?subject=ME-JEPA paper
Secondary: See research index     →   /research
```

---

## 4. The full `paper.ts` RouteCopy file

Replace `src/content/routes/paper.ts` with:

```ts
import type { RouteCopy } from "@/types/route-copy";

export const paperCopy: RouteCopy = {
  path: "/paper",
  purpose:
    "Translate the current Teleox research paper into a plain-language overview alongside the technical headline numbers and a path to access.",
  audience: "primary",
  playerClasses: ["lab", "researcher"],
  eyebrow: "Current paper",

  primaryCta: {
    label: "Email Chris",
    href: "mailto:chrisroyseai@gmail.com?subject=ME-JEPA paper",
  },
  secondaryCta: {
    label: "See research index",
    href: "/research",
  },

  signals: ["S1", "S3", "S5"],
  scopeGuards: [],

  heroHeadline: "Dynamic / ME-JEPA",
  heroHeadlineLitWord: "ME-JEPA",
  heroSubhead:
    "An audited, domain-portable JEPA runtime on a single RTX 5090, with a new class of training data. Draft preprint, May 2026; not yet posted publicly.",
  heroAlt: "Dynamic / ME-JEPA — current Teleox research paper.",

  bodyBlocks: [
    {
      heading: "What the paper does",
      paragraphs: [
        "Most JEPA work emphasizes representational quality on natural images or video. This paper takes a different angle: what does it take to make every step of a JEPA runtime — data ingest, multi-instrument latent panel materialization, action-conditioned training, planning, surprise recording, and artifact provenance — physically auditable end-to-end on a single commodity GPU?",
        "Dynamic / ME-JEPA is one Rust binary that does all of it. New domains ship as a TOML manifest. Every operation writes typed rows into 25 RocksDB column families plus an append-only audit log, and a structurally separate verifier reads everything back. The same code path runs three domains: counter_world (480 trainable parameters), gridworld_5x5 (16,960), and a new career_taxonomy.v1 public-taxonomy benchmark (15,344) derived from O*NET 30.2 starting at Software Developers (15-1252.00).",
      ],
    },
    {
      heading: "Headline numbers",
      paragraphs: [
        "Five seeds of paper_small_1k (1,000 events each) on an RTX 5090:",
        // ResultsBox is rendered separately in page.tsx
      ],
    },
    {
      heading: "What's actually new",
      paragraphs: [
        "The paper is not a representational-quality contribution to the JEPA family. The premise of latent prediction is taken as established (I-JEPA, V-JEPA, LeWorldModel, LeCun's vision paper). The contribution is one level up: the infrastructure that makes a complete JEPA runtime physically auditable, end-to-end, on a single GPU.",
        "The second contribution is conceptual. Each immutable bundle the runtime produces is itself a structured training-data artifact: most bytes are deterministic functions of a public source ZIP; the small model-output slice is paired with the observed next-state and with random-init / shuffled-target baselines in the same record; counterfactual ablations and failed runs are persisted as first-class records. The argument is that bundles of this shape form a new class of training data — audited execution traces with cryptographic provenance — distinct from web text, pure model-generated synthetic data, and rejection-sampled chain-of-thought.",
      ],
    },
    {
      heading: "Reproducibility",
      paragraphs: [
        "The release reference is host-specific to RTX 5090 sm_120 / CUDA 13.2 / Rust 1.95. Cross-host portability is future work. The CI gate is local and self-hosted to keep evidence on disk; a reviewer without RTX 5090 access cannot run the full release-C gate.",
        "The reproducibility artifact is a deterministic Docker tar (SHA-256 099f0535…d80e), an Apptainer SIF (SHA-256 30b531c7…2825), and a release archive mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz (SHA-256 8cccf498…3ecd). The career-taxonomy data substrate uses O*NET 30.2 under CC BY 4.0; the source ZIP SHA-256 and per-edge walk hash are recorded in every generated bundle.",
      ],
    },
    {
      heading: "How to access",
      paragraphs: [
        "The paper has not been posted to arXiv yet. It is being shared privately with a small set of researchers whose work it is in conversation with, before any public post. If you are in that orbit and would like a copy, write to chrisroyseai@gmail.com.",
        "The Dynamic / ME-JEPA Rust runtime itself is a private codebase. Reviewers requiring source-level access for verification can request a time-bounded private grant via the same email. The reproducibility artifact described in the paper is the canonical reproducible release; reviewers with an RTX 5090 host can reproduce every headline number from the artifact alone, without source access.",
      ],
    },
  ],

  mustNotSay: [
    "guarantees AGI",
    "solves the data wall",
    "model collapse is over",
    "synthetic data replacement",
    "revolutionary",
    "first ever",
    "world's first",
  ],
};

export default paperCopy;
```

---

## 5. The full `page.tsx` file

Replace `src/app/paper/page.tsx` with:

```tsx
import type { Metadata } from "next";
import { paperCopy } from "@/content/routes/paper";
import { generateRouteMetadata } from "@/lib/metadata";
import { PageContainer } from "@/components/layout/PageContainer";
import { ResearchHero } from "@/components/hero/ResearchHero";
import { Section } from "@/components/content-blocks/Section";
import { ResultsBox } from "@/components/content-blocks/ResultsBox";

export const metadata: Metadata = generateRouteMetadata(paperCopy, {
  title: "Dynamic / ME-JEPA — Teleox.ai",
  description:
    "The current Teleox research paper: an audited, domain-portable JEPA runtime on a single RTX 5090, with a new class of training data.",
});

export default function PaperPage() {
  return (
    <PageContainer width="reading">
      <ResearchHero
        eyebrow={paperCopy.eyebrow}
        headline={paperCopy.heroHeadline}
        subhead={paperCopy.heroSubhead}
      />

      <Section heading={paperCopy.bodyBlocks[0].heading}>
        {paperCopy.bodyBlocks[0].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <Section heading={paperCopy.bodyBlocks[1].heading}>
        <p>{paperCopy.bodyBlocks[1].paragraphs[0]}</p>
        <ResultsBox>
          Career-taxonomy validation cosine: <strong>0.905 ± 0.044</strong> across 5 seeds (BCa 95% CI <strong>[0.861, 0.931]</strong>)
          <br />
          Test cosine: <strong>0.868 ± 0.068</strong> (BCa [0.806, 0.912])
          <br />
          Validation action-contrast accuracy: <strong>0.960 ± 0.043</strong>
          <br />
          Action-removed (A3) ablation: collapses to validation cosine <strong>0.790</strong> — exactly as the action-conditioned predictor design requires.
        </ResultsBox>
      </Section>

      <Section heading={paperCopy.bodyBlocks[2].heading}>
        {paperCopy.bodyBlocks[2].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <Section heading={paperCopy.bodyBlocks[3].heading}>
        {paperCopy.bodyBlocks[3].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <Section heading={paperCopy.bodyBlocks[4].heading}>
        {paperCopy.bodyBlocks[4].paragraphs.map((p, i) => (
          <p key={i}>{p}</p>
        ))}
      </Section>

      <div className="mt-16 md:mt-20 pt-8 border-t border-mauve-fog">
        <ResearchHero
          headline=""
          primaryCta={paperCopy.primaryCta}
          secondaryCta={paperCopy.secondaryCta}
        />
      </div>
    </PageContainer>
  );
}
```

---

## 6. Test plan

- [ ] `https://teleox.ai/paper` returns 200
- [ ] Hero renders title + subhead at correct sizes
- [ ] All 5 sections render with correct headings + body
- [ ] ResultsBox displays 4 metric lines with bolded numbers
- [ ] Both CTAs route correctly
- [ ] No console errors
- [ ] Lighthouse Performance ≥ 95
- [ ] `npm run scan:all` passes

---

## 7. What this page deliberately does NOT have

- No PDF embed (PDFs are reached via per-recipient slug or email request)
- No bibliography on page (paper has its own references)
- No "cite as" snippet (premature; no DOI yet)
- No "share this paper" buttons
- No comments / discussion
- No related-paper sidebar
