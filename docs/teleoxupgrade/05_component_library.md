# 05 — Component Library

**Authority over:** what UI primitives exist, what they look like, when to use them.
**Inherits from:** existing `src/components/` directory (large pre-existing library).
**Strategy:** the upgrade DOES NOT rebuild the component library. It identifies the small set of primitives that are needed for the new 5-page site and prescribes which existing components to use. Unused legacy components stay in the codebase but are not invoked.

---

## 1. The five components every new page needs

| Component | Existing? | Source path | Notes |
|---|---|---|---|
| **PageContainer** | reuse / build small | `src/components/layout/PageContainer.tsx` (likely exists; create if not) | Wraps `<main>` with the responsive page padding from §4.3 of design system |
| **PageHero** | new (small variant) | `src/components/hero/ResearchHero.tsx` (NEW) | Simpler than the existing Hero; just a headline + subhead + CTA pair |
| **Section** | reuse | `src/components/content-blocks/Section.tsx` | h2 + body block container |
| **PrimaryCta** | reuse | `src/components/ui/Button.tsx` (likely exists) | Per §9.1 of design system |
| **SecondaryCta** | reuse | `src/components/ui/Button.tsx` (variant) | Per §9.2 of design system |

Plus chrome (header / footer / nav) covered in `12_navigation_and_footer.md`.

---

## 2. PageContainer

### 2.1 Purpose

A single component that all pages render inside. Provides the responsive padding, max-width, and main-element semantics.

### 2.2 Spec

```tsx
// src/components/layout/PageContainer.tsx
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

type PageContainerProps = {
  children: ReactNode;
  width?: "reading" | "standard" | "hero";
  className?: string;
};

const WIDTH_CLASSES: Record<NonNullable<PageContainerProps["width"]>, string> = {
  reading: "max-w-[660px]",
  standard: "max-w-[1024px]",
  hero: "max-w-[1200px]",
};

export function PageContainer({
  children,
  width = "reading",
  className,
}: PageContainerProps) {
  return (
    <main
      className={cn(
        "mx-auto px-7 pt-12 pb-16 md:px-10 md:pt-16 md:pb-20 lg:px-16 lg:pt-24 lg:pb-32",
        WIDTH_CLASSES[width],
        className,
      )}
    >
      {children}
    </main>
  );
}
```

### 2.3 Usage

```tsx
// Most pages
<PageContainer>{...content}</PageContainer>

// Homepage hero
<PageContainer width="hero">{...hero}</PageContainer>

// Wider pages (research with multi-column)
<PageContainer width="standard">{...content}</PageContainer>
```

### 2.4 Accessibility notes

- `<main>` element provides a single landmark per page.
- Each page should have exactly one `PageContainer`. If a page needs separate "wider hero / narrower body" sections, use two `<section>` blocks inside one `PageContainer` — not two `PageContainer`s.

---

## 3. ResearchHero

### 3.1 Purpose

A simpler hero than the existing `InstitutionalRoutePage` hero. Used on every page to render the eyebrow, headline, subhead, and CTA pair consistently.

### 3.2 Spec

```tsx
// src/components/hero/ResearchHero.tsx
import type { ReactNode } from "react";
import Link from "next/link";
import { ArrowRight } from "lucide-react";

type CtaProps = {
  label: string;
  href: string;
  external?: boolean;
};

type ResearchHeroProps = {
  eyebrow?: string;
  headline: string;
  subhead?: string;
  primaryCta?: CtaProps;
  secondaryCta?: CtaProps;
};

export function ResearchHero({
  eyebrow,
  headline,
  subhead,
  primaryCta,
  secondaryCta,
}: ResearchHeroProps) {
  return (
    <header className="mb-16 md:mb-24">
      {eyebrow ? (
        <p className="font-sans uppercase tracking-[0.12em] text-xs text-cream/60 mb-4">
          {eyebrow}
        </p>
      ) : null}

      <h1 className="font-body font-semibold text-cream tracking-tight text-[40px] leading-[1.08] md:text-[56px] md:leading-[1.05] lg:text-[84px] lg:leading-[1.0] mb-6 max-w-[18ch]">
        {headline}
      </h1>

      {subhead ? (
        <p className="font-body text-cream/82 text-[18px] md:text-[20px] leading-[1.55] max-w-[640px] mb-8">
          {subhead}
        </p>
      ) : null}

      {(primaryCta || secondaryCta) ? (
        <div className="flex flex-col sm:flex-row gap-3 sm:gap-4 sm:items-center">
          {primaryCta ? (
            <Link
              href={primaryCta.href}
              {...(primaryCta.external ? { target: "_blank", rel: "noopener noreferrer" } : {})}
              className="
                inline-flex items-center justify-center
                px-5 py-3
                bg-ember text-ink-1
                font-sans font-medium text-base
                rounded
                transition-colors duration-150
                hover:bg-honey
                focus-visible:outline-none
                focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2 focus-visible:ring-offset-ink-1
              "
            >
              {primaryCta.label}
              <ArrowRight size={16} className="ml-1.5" />
            </Link>
          ) : null}

          {secondaryCta ? (
            <Link
              href={secondaryCta.href}
              {...(secondaryCta.external ? { target: "_blank", rel: "noopener noreferrer" } : {})}
              className="
                inline-flex items-center justify-center
                px-5 py-3
                border border-cream/30
                text-cream
                font-sans font-medium text-base
                rounded
                transition-colors duration-150
                hover:border-cream/60 hover:bg-cream/5
                focus-visible:outline-none
                focus-visible:ring-2 focus-visible:ring-cream/40 focus-visible:ring-offset-2 focus-visible:ring-offset-ink-1
              "
            >
              {secondaryCta.label}
            </Link>
          ) : null}
        </div>
      ) : null}
    </header>
  );
}
```

### 3.3 Usage

```tsx
<ResearchHero
  eyebrow="Independent research"
  headline="Models can be opaque. Pipelines shouldn't be."
  subhead="Teleox publishes infrastructure that makes JEPA-style and predictive-AI training pipelines verifiable end-to-end on commodity hardware."
  primaryCta={{ label: "Read current research", href: "/research" }}
  secondaryCta={{ label: "Email Chris", href: "mailto:chrisroyseai@gmail.com?subject=Teleox" }}
/>
```

### 3.4 Variants

- `eyebrow` is optional — used on subpages, not the homepage
- `subhead` is optional — short pages may omit
- Either CTA can be omitted

### 3.5 Accessibility notes

- The `<h1>` is unique per page. The `<header>` element wraps the whole hero (one `<header>` per page).
- CTAs use `<Link>` from Next, which renders `<a>` with proper `href`.
- External CTAs include `rel="noopener noreferrer"` automatically.

---

## 4. Section

### 4.1 Purpose

Renders a `<section>` with an `<h2>` and body content. Used for body content blocks below the hero.

### 4.2 Spec

```tsx
// src/components/content-blocks/Section.tsx
import type { ReactNode } from "react";

type SectionProps = {
  heading: string;
  children: ReactNode;
};

export function Section({ heading, children }: SectionProps) {
  return (
    <section className="mt-12 md:mt-14 pt-6 border-t border-mauve-fog">
      <h2 className="font-body font-semibold text-cream tracking-tight text-[28px] leading-[1.1] md:text-[40px] md:leading-[1.08] mb-6">
        {heading}
      </h2>
      <div className="font-body text-cream text-[16px] md:text-[18px] leading-[1.6] [&>p]:mb-4 [&>p:last-child]:mb-0">
        {children}
      </div>
    </section>
  );
}
```

### 4.3 Usage

```tsx
<Section heading="Verifiable pipelines, not interpretable models">
  <p>Mechanistic interpretability of trained models is hard…</p>
  <p>Every operation in a Teleox runtime…</p>
</Section>
```

### 4.4 Accessibility notes

- Each `Section` is a `<section>` landmark with an `<h2>`.
- Sections cannot nest meaningfully without proper heading hierarchy. If you need a sub-heading inside a section, use `<h3>` with appropriate styling (see design system §3.4).

---

## 5. ResultsBox

### 5.1 Purpose

Displays the headline empirical numbers. Used on the homepage and `/research` and `/paper`.

### 5.2 Spec

```tsx
// src/components/content-blocks/ResultsBox.tsx
import type { ReactNode } from "react";

type ResultsBoxProps = {
  children: ReactNode;
};

export function ResultsBox({ children }: ResultsBoxProps) {
  return (
    <div
      className="
        bg-paper
        border border-mauve-fog
        rounded-md
        px-5 py-4
        font-sans text-[14px] leading-[1.65] text-cream/90
        [&_strong]:text-honey [&_strong]:font-medium
        my-6
      "
    >
      {children}
    </div>
  );
}
```

### 5.3 Usage

```tsx
<ResultsBox>
  Career-taxonomy validation cosine: <strong>0.905 ± 0.044</strong> across 5 seeds (BCa 95% CI <strong>[0.861, 0.931]</strong>)<br />
  Test cosine: <strong>0.868 ± 0.068</strong> (BCa [0.806, 0.912])<br />
  Action-removed ablation: collapses to validation cosine <strong>0.790</strong> — exactly as the action-conditioned predictor design requires.
</ResultsBox>
```

### 5.4 Accessibility notes

- `ResultsBox` is a presentational `<div>`, not a `<table>`. If results are tabular and benefit from a screen reader rendering them as a table, use a `<table>` directly per design system §10.

---

## 6. ArtifactCard

### 6.1 Purpose

A card displayed on `/research` for each artifact (paper, companion code, future projects). Two-up grid on desktop, stacked on mobile.

### 6.2 Spec

```tsx
// src/components/content-blocks/ArtifactCard.tsx
import Link from "next/link";
import { ArrowRight, ExternalLink } from "lucide-react";

type ArtifactCardProps = {
  eyebrow: string;          // e.g., "Draft preprint" / "Open source"
  title: string;            // e.g., "Dynamic / ME-JEPA"
  oneLine: string;          // 1-sentence framing
  cta: {
    label: string;
    href: string;
    external?: boolean;
  };
  meta?: string;            // e.g., "18 pages · 594 KB · May 2026"
  status?: "private-preview" | "open-source" | "in-progress";
};

const STATUS_COPY: Record<NonNullable<ArtifactCardProps["status"]>, string> = {
  "private-preview": "Private preview",
  "open-source": "Public",
  "in-progress": "In progress",
};

const STATUS_COLOR: Record<NonNullable<ArtifactCardProps["status"]>, string> = {
  "private-preview": "text-amber",
  "open-source": "text-ember",
  "in-progress": "text-cream/60",
};

export function ArtifactCard({
  eyebrow,
  title,
  oneLine,
  cta,
  meta,
  status,
}: ArtifactCardProps) {
  return (
    <article className="
      bg-bordeaux
      border border-mauve-fog
      rounded-lg
      p-6 md:p-7
      flex flex-col h-full
      transition-colors duration-150
      hover:border-cream/30
    ">
      <header className="mb-4">
        <p className="font-sans uppercase tracking-[0.1em] text-[11px] text-cream/60 mb-2">
          {eyebrow}
          {status ? (
            <>
              <span className="text-cream/30 mx-2">·</span>
              <span className={STATUS_COLOR[status]}>{STATUS_COPY[status]}</span>
            </>
          ) : null}
        </p>
        <h3 className="font-body font-semibold text-cream text-[22px] leading-[1.2]">
          {title}
        </h3>
      </header>

      <p className="font-body text-cream/82 text-[16px] leading-[1.55] mb-6 flex-grow">
        {oneLine}
      </p>

      {meta ? (
        <p className="font-sans text-[12px] text-cream/48 mb-3">
          {meta}
        </p>
      ) : null}

      <Link
        href={cta.href}
        {...(cta.external ? { target: "_blank", rel: "noopener noreferrer" } : {})}
        className="
          inline-flex items-center
          font-sans font-medium text-[14px]
          text-ember
          hover:text-honey
          transition-colors
          mt-auto
          self-start
        "
      >
        {cta.label}
        {cta.external
          ? <ExternalLink size={14} className="ml-1" />
          : <ArrowRight size={14} className="ml-1" />}
      </Link>
    </article>
  );
}
```

### 6.3 Usage

```tsx
<div className="grid grid-cols-1 md:grid-cols-2 gap-6">
  <ArtifactCard
    eyebrow="Draft preprint"
    title="Dynamic / ME-JEPA"
    oneLine="An audited, domain-portable JEPA runtime on a single RTX 5090."
    meta="18 pages · 594 KB · May 2026"
    status="private-preview"
    cta={{ label: "Request preview access", href: "mailto:chrisroyseai@gmail.com?subject=ME-JEPA preview access" }}
  />

  <ArtifactCard
    eyebrow="Open source"
    title="ClipCannon"
    oneLine="MCP server for video understanding with 5 frozen embedders. Same multi-embedder pattern as the private runtime."
    meta="MCP server · Rust + Python · BSL 1.1"
    status="open-source"
    cta={{ label: "github.com/ChrisRoyse/clipcannon", href: "https://github.com/ChrisRoyse/clipcannon", external: true }}
  />
</div>
```

---

## 7. Footer

### 7.1 Purpose

Site footer. Minimal: brand wordmark, email, nav repeat, RSS / llms.txt.

### 7.2 Spec

```tsx
// src/components/layout/Footer.tsx
import Link from "next/link";

export function Footer() {
  return (
    <footer className="border-t border-bordeaux mt-24 lg:mt-32 px-7 md:px-10 lg:px-16 py-12">
      <div className="max-w-[1024px] mx-auto flex flex-col md:flex-row md:justify-between md:items-end gap-6">
        <div>
          <Link
            href="/"
            className="font-sans font-semibold tracking-tight text-cream text-base hover:text-honey transition-colors"
          >
            Teleox<span className="text-amber">.</span>ai
          </Link>
          <p className="font-sans text-[13px] text-cream/60 mt-2">
            <a
              href="mailto:chrisroyseai@gmail.com"
              className="hover:text-cream transition-colors"
            >
              chrisroyseai@gmail.com
            </a>
          </p>
        </div>

        <nav className="flex flex-wrap gap-x-6 gap-y-2 font-sans text-[13px] text-cream/72">
          <Link href="/research" className="hover:text-cream transition-colors">Research</Link>
          <Link href="/paper" className="hover:text-cream transition-colors">Paper</Link>
          <Link href="/about" className="hover:text-cream transition-colors">About</Link>
          <Link href="/contact" className="hover:text-cream transition-colors">Contact</Link>
          <Link href="/blog" className="hover:text-cream transition-colors">Blog</Link>
          <a href="/blog/rss.xml" className="hover:text-cream transition-colors">RSS</a>
          <a href="/llms.txt" className="hover:text-cream transition-colors">llms.txt</a>
        </nav>
      </div>

      <p className="font-sans text-[12px] text-cream/40 mt-8 max-w-[1024px] mx-auto">
        © 2026 Teleox.ai. Independent research.
      </p>
    </footer>
  );
}
```

---

## 8. Header

### 8.1 Purpose

Sticky-ish header with wordmark and nav.

### 8.2 Spec

```tsx
// src/components/layout/Header.tsx
import Link from "next/link";
import { MobileNavToggle } from "./MobileNavToggle";

export function Header() {
  return (
    <header className="border-b border-bordeaux/60 bg-ink-1/80 backdrop-blur-sm sticky top-0 z-30">
      <div className="max-w-[1200px] mx-auto px-7 md:px-10 lg:px-16 py-4 flex items-center justify-between">
        <Link
          href="/"
          className="font-sans font-semibold tracking-tight text-cream text-lg hover:text-honey transition-colors"
        >
          Teleox<span className="text-amber">.</span>ai
        </Link>

        <nav className="hidden md:flex items-center gap-8 font-sans text-[14px] text-cream/82">
          <Link href="/research" className="hover:text-cream transition-colors">Research</Link>
          <Link href="/paper" className="hover:text-cream transition-colors">Paper</Link>
          <Link href="/about" className="hover:text-cream transition-colors">About</Link>
          <Link href="/contact" className="hover:text-cream transition-colors">Contact</Link>
        </nav>

        <MobileNavToggle />
      </div>
    </header>
  );
}
```

(See `12_navigation_and_footer.md` for `MobileNavToggle` spec.)

---

## 9. Existing components — keep / use

These components in `src/components/` are still useful for the upgraded site:

| Component dir | Use? | Notes |
|---|---|---|
| `analytics/` | KEEP | `FrontendInstrumentation` already wired in `layout.tsx` |
| `brand/` | EVALUATE | Replace usages with simpler wordmark spec from §03 |
| `hero/` | PARTIALLY KEEP | Existing `InstitutionalRoutePage` may be too heavy; `ResearchHero` (new) is simpler |
| `icons/` | KEEP if generic | Use Lucide directly; deprecate any custom Ember-specific icon set |
| `layout/` | KEEP `RootLayoutShell`; build new `PageContainer`, `Header`, `Footer` | Some existing layout components may be tied to old IA — verify |
| `signals/` | LIKELY DELETE | The "signal-bundle" concept is from the old commercial framing; not used in upgraded site |
| `structured-data/` | KEEP | `JsonLd` component still serves the new IA |
| `ui/` | KEEP `Heading`, possibly `Button` | Verify Button matches new CTA spec; otherwise build per §3.1 of design system |

---

## 10. Existing components — retire (with retired pages)

These are tied to retired pages and can be removed (or left in place — they just won't be referenced):

- `artifacts/` — used by `/artifacts`
- `centroids/` — used by `/centroids`
- `citations/` — used by `/citations`
- `governance/` — used by `/governance`
- `mark/` — used by `/mark`
- `pricing/` — used by `/pricing`
- `priority/` — used by `/priority-date`
- `sign/` — used by `/sign`
- `verify/` — used by `/verify`
- `scope-guards/` — likely tied to old IA; verify before removing
- `ember/` — possibly Ember design-system-specific decorations; KEEP if generic, prune if specifically scoped to retired pages
- `institutional/` — `InstitutionalRoutePage` may still serve; verify
- `content-blocks/` — KEEP; reusable

The cleanup of retired components is **not a hard requirement** for this upgrade. The implementer may leave them in place to ship faster. A future cleanup PR can prune them.

---

## 11. Components that DO NOT exist yet (to build)

| Component | Spec | Doc reference |
|---|---|---|
| `PageContainer` | This doc §2 | — |
| `ResearchHero` | This doc §3 | — |
| `Section` | This doc §4 (verify if exists; if so, reuse) | — |
| `ResultsBox` | This doc §5 | — |
| `ArtifactCard` | This doc §6 | — |
| `Footer` | This doc §7 (replaces existing footer) | — |
| `Header` | This doc §8 (replaces existing header) | — |
| `MobileNavToggle` | `12_navigation_and_footer.md` | 12 |

---

## 12. Component naming convention

- One component per file
- File name = component name in PascalCase
- Co-locate types in the same file
- Co-locate small subcomponents in the same file
- Pull out into separate file if reused 2+ times across pages
- Use the `cn()` utility (from `src/lib/cn.ts`) for conditional class joining

---

## 13. Component testing

Each new component should have:

- A Vitest unit test verifying the rendered HTML structure (heading, link href, CTA presence)
- A Playwright e2e test that verifies the component renders and is interactive on the page that uses it

Existing `vitest.config.ts` and `playwright.config.ts` are already wired. Add new test files in `tests/` following the repo's convention.

Example unit test:

```ts
// tests/components/ArtifactCard.test.tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { ArtifactCard } from "@/components/content-blocks/ArtifactCard";

describe("ArtifactCard", () => {
  it("renders title, oneLine, and CTA link", () => {
    render(
      <ArtifactCard
        eyebrow="Draft preprint"
        title="Dynamic / ME-JEPA"
        oneLine="An audited JEPA runtime."
        cta={{ label: "Read more", href: "/paper" }}
      />,
    );

    expect(screen.getByRole("heading", { name: /Dynamic \/ ME-JEPA/i })).toBeInTheDocument();
    expect(screen.getByText(/audited JEPA runtime/i)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Read more/i })).toHaveAttribute("href", "/paper");
  });

  it("renders ExternalLink icon when cta.external is true", () => {
    render(
      <ArtifactCard
        eyebrow="Code"
        title="ClipCannon"
        oneLine="Open-source companion."
        cta={{ label: "GitHub", href: "https://github.com/ChrisRoyse/clipcannon", external: true }}
      />,
    );

    const link = screen.getByRole("link", { name: /GitHub/i });
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });
});
```

---

## 14. Verification

After component build:

- [ ] `PageContainer` renders responsive padding correctly at all breakpoints
- [ ] `ResearchHero` renders headline, subhead, and both CTAs with correct styling
- [ ] `Section` renders h2 with the spec'd top divider
- [ ] `ResultsBox` highlights `<strong>` text in honey color
- [ ] `ArtifactCard` shows status badge in correct color
- [ ] `Header` is sticky, shows nav on desktop, toggle on mobile
- [ ] `Footer` renders all 7 nav items in spec'd grid
- [ ] All components pass Vitest unit tests
- [ ] All components pass axe-core accessibility check
