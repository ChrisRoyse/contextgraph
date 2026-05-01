# 09 — `/about` Page Specification

**Route:** `/about`
**File:** `src/app/about/page.tsx`
**Content:** `src/content/routes/about.ts` (REPOSITION existing file)
**Audience:** Anyone curious about the principal behind Teleox
**Single thing it must communicate:** "Chris Royse is a real person doing this work; here is the prior context."

---

## 1. Page meta

| Field | Value |
|---|---|
| `title` | `About — Teleox.ai` |
| `description` | `Teleox is an independent research operation founded by Chris Royse. Brief overview of the principal, the work, and adjacent projects.` |
| `og:title` | `About Teleox.ai` |
| `og:description` | `Independent research on verifiable AI training pipelines. Founded by Chris Royse.` |
| `og:image` | `/og-image.png` |
| `canonical` | `https://teleox.ai/about` |

---

## 2. Page structure

```
[ Header ]
[ Hero (eyebrow + h1 + subhead) ]
[ Section: Who runs Teleox ]
[ Section: What Teleox is ]
[ Section: What Teleox is not ]
[ Section: Adjacent projects ]
[ CTAs (Email Chris + See research) ]
[ Footer ]
```

Container width: `reading` (660px max).

---

## 3. Content (verbatim copy)

### 3.1 Eyebrow

```
About
```

### 3.2 H1

```
About Teleox
```

### 3.3 Subhead

```
Teleox is an independent research operation founded and run by Chris Royse. The work focuses on infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware.
```

### 3.4 Section: Who runs Teleox

**Heading:** `Who runs Teleox`

**Body:**

```
Teleox is run by Chris Royse, an independent researcher working on the
infrastructure side of AI training pipelines: provenance, verifiability,
reproducibility, and the structure of training-data artifacts. The current
research focuses on JEPA-style joint-embedding predictive architectures and
the question of what it takes to make a complete training and planning runtime
physically auditable end-to-end on commodity hardware.

Reach Chris directly at chrisroyseai@gmail.com.
```

### 3.5 Section: What Teleox is

**Heading:** `What Teleox is`

**Body:**

```
Teleox is an independent research operation. The current direction has three
working axes: pipeline verifiability (every step from source to weights is
auditable), training-data structure (bundles produced by audited runtimes form
a distinct class with cryptographic chain-of-custody), and single-GPU
reproducibility (external verification is feasible without lab-scale compute).

Current artifacts: a draft preprint (Dynamic / ME-JEPA) shared in private
preview with select reviewers, and an open-source companion implementation
(ClipCannon) that demonstrates the multi-embedder MCP architecture in a
different domain.
```

### 3.6 Section: What Teleox is not

**Heading:** `What Teleox is not`

**Body:**

```
Teleox is not a SaaS product, not a consulting firm, not an AI safety think
tank, not an open-source project (the runtime is private; the public
artifacts are released under specific licenses), and not a competitor to any
major AI lab. It is complementary infrastructure: the verifiability layer
that sits below the model.
```

### 3.7 Section: Adjacent projects

**Heading:** `Adjacent projects`

**Body:**

```
Chris also operates leapable.ai, a public AI memory product on the same
underlying infrastructure. The two projects are technically related but
strategically separate; this site is for the research direction.

The open-source companion to the current Teleox research is at
github.com/ChrisRoyse/clipcannon — an MCP server with five frozen embedders for
video understanding, demonstrating the multi-embedder pattern in a different
domain than the private JEPA runtime.
```

---

### 3.8 CTA pair

```
Primary:   Email Chris       →   mailto:chrisroyseai@gmail.com?subject=Teleox
Secondary: See research      →   /research
```

---

## 4. The full `about.ts` RouteCopy file

Replace `src/content/routes/about.ts` with:

```ts
import type { RouteCopy } from "@/types/route-copy";

export const aboutCopy: RouteCopy = {
  path: "/about",
  purpose:
    "Identify Chris Royse as the principal behind Teleox, summarize the operation, and reference adjacent projects.",
  audience: "secondary",
  playerClasses: ["lab", "researcher", "public"],
  eyebrow: "About",

  primaryCta: {
    label: "Email Chris",
    href: "mailto:chrisroyseai@gmail.com?subject=Teleox",
  },
  secondaryCta: {
    label: "See research",
    href: "/research",
  },

  signals: ["S1", "S3"],
  scopeGuards: [],

  heroHeadline: "About Teleox",
  heroHeadlineLitWord: "Teleox",
  heroSubhead:
    "Teleox is an independent research operation founded and run by Chris Royse. The work focuses on infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware.",
  heroAlt: "About Teleox.ai — independent research, founded by Chris Royse.",

  bodyBlocks: [
    {
      heading: "Who runs Teleox",
      paragraphs: [
        "Teleox is run by Chris Royse, an independent researcher working on the infrastructure side of AI training pipelines: provenance, verifiability, reproducibility, and the structure of training-data artifacts. The current research focuses on JEPA-style joint-embedding predictive architectures and the question of what it takes to make a complete training and planning runtime physically auditable end-to-end on commodity hardware.",
        "Reach Chris directly at chrisroyseai@gmail.com.",
      ],
    },
    {
      heading: "What Teleox is",
      paragraphs: [
        "Teleox is an independent research operation. The current direction has three working axes: pipeline verifiability (every step from source to weights is auditable), training-data structure (bundles produced by audited runtimes form a distinct class with cryptographic chain-of-custody), and single-GPU reproducibility (external verification is feasible without lab-scale compute).",
        "Current artifacts: a draft preprint (Dynamic / ME-JEPA) shared in private preview with select reviewers, and an open-source companion implementation (ClipCannon) that demonstrates the multi-embedder MCP architecture in a different domain.",
      ],
    },
    {
      heading: "What Teleox is not",
      paragraphs: [
        "Teleox is not a SaaS product, not a consulting firm, not an AI safety think tank, not an open-source project (the runtime is private; the public artifacts are released under specific licenses), and not a competitor to any major AI lab. It is complementary infrastructure: the verifiability layer that sits below the model.",
      ],
    },
    {
      heading: "Adjacent projects",
      paragraphs: [
        "Chris also operates leapable.ai, a public AI memory product on the same underlying infrastructure. The two projects are technically related but strategically separate; this site is for the research direction.",
        "The open-source companion to the current Teleox research is at github.com/ChrisRoyse/clipcannon — an MCP server with five frozen embedders for video understanding, demonstrating the multi-embedder pattern in a different domain than the private JEPA runtime.",
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
    "founder-led startup",
    "co-founders",
    "team of",
  ],
};

export default aboutCopy;
```

---

## 5. The full `page.tsx` file

Replace `src/app/about/page.tsx` with:

```tsx
import type { Metadata } from "next";
import { aboutCopy } from "@/content/routes/about";
import { generateRouteMetadata } from "@/lib/metadata";
import { PageContainer } from "@/components/layout/PageContainer";
import { ResearchHero } from "@/components/hero/ResearchHero";
import { Section } from "@/components/content-blocks/Section";

export const metadata: Metadata = generateRouteMetadata(aboutCopy, {
  title: "About — Teleox.ai",
  description:
    "Teleox is an independent research operation founded by Chris Royse. Brief overview of the principal, the work, and adjacent projects.",
});

export default function AboutPage() {
  return (
    <PageContainer width="reading">
      <ResearchHero
        eyebrow={aboutCopy.eyebrow}
        headline={aboutCopy.heroHeadline}
        subhead={aboutCopy.heroSubhead}
      />

      {aboutCopy.bodyBlocks.map((block, i) => (
        <Section key={i} heading={block.heading}>
          {block.paragraphs.map((p, j) => (
            <p key={j}>{p}</p>
          ))}
        </Section>
      ))}

      <div className="mt-16 md:mt-20 pt-8 border-t border-mauve-fog">
        <ResearchHero
          headline=""
          primaryCta={aboutCopy.primaryCta}
          secondaryCta={aboutCopy.secondaryCta}
        />
      </div>
    </PageContainer>
  );
}
```

---

## 6. Steve Abbey question

The previous about page mentions Steve Abbey as a co-founder. The new about page omits Steve. **Decision required by site owner before this page ships.**

If Steve stays involved with Teleox publicly:
- Add a paragraph to the "Who runs Teleox" section identifying his role
- Use first-person plural ("we") in some places where current draft uses Chris singular

If Steve is moving on:
- Keep the page as drafted (Chris-only)
- Update `humans.txt` to remove Steve's credit

The default in this draft is "Chris-only" because:
1. The new positioning is research-led (Chris is the named author)
2. "Founder-led startup" reads commercial, not research
3. Single-principal pages read more academic

The `mustNotSay` list intentionally includes "co-founders" and "team of" to enforce the single-principal voice. Remove these from `mustNotSay` if Steve stays.

---

## 7. Author photo

Optional. The current site does not include a photo of Chris on /about. The upgrade leaves this decision to the site owner:

| Choice | Effect |
|---|---|
| **No photo** | Maximum minimalism; matches yann.lecun.com convention |
| **Small photo** (right-aligned, 96–120px circular, near the bio) | Adds personality; humanizes the brand |

If a photo is included, place it:
- After the H1, floated right on desktop, centered on mobile
- 120px diameter, `rounded-full`, no border
- `object-cover`, `object-center`
- alt text: "Chris Royse"

---

## 8. Test plan

- [ ] `https://teleox.ai/about` returns 200
- [ ] Hero renders title + subhead at correct sizes
- [ ] Four sections render with headings + body
- [ ] First-person reference is "Chris" / "Chris also operates" — no "we" if going single-principal route
- [ ] Both CTAs route correctly
- [ ] leapable.ai link opens to https://leapable.ai
- [ ] clipcannon link opens to GitHub
- [ ] No console errors
- [ ] Lighthouse Performance ≥ 95
- [ ] `npm run scan:all` passes
