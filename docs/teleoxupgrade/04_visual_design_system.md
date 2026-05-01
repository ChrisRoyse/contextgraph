# 04 — Visual Design System

**Authority over:** color tokens, typography, spacing, layout grid, motion, breakpoints.
**Inherits from:** the existing **Ember** design system already shipped on the site (defined in `src/app/globals.css` `@theme` block + `tailwind.config.ts`).
**Modifies:** small token additions to fit research-grade positioning; deprecates none of the existing tokens.

---

## 1. Foundation: the existing Ember system

The site already has a complete design system (Ember). This upgrade does **not** replace it. It uses the existing tokens with stricter discipline.

### 1.1 Existing Ember tokens (verbatim from `src/app/globals.css`)

#### Colors (semantic)

```css
--color-ink-0:      #0E1210;   /* Deepest background */
--color-ink-1:      #111713;   /* Page background base */
--color-wine:       #161D18;   /* Section / surface base */
--color-bordeaux:   #243028;   /* Card body / mid surface */
--color-burgundy:   #1A211C;   /* Subtle surface, between ink-1 and wine */
--color-ember:      #66D19E;   /* Primary accent (mint/jade) */
--color-amber:      #F3B85B;   /* Secondary accent (warm gold) */
--color-honey:      #8AD18F;   /* Highlight / emphasis */
--color-peach:      #B9C6B8;   /* Soft text on darker surfaces */
--color-cream:      #F5F0E6;   /* Body text on dark surfaces */
--color-paper:      #141A16;   /* Off-black secondary surface */
--color-mauve-fog:  #26312A;   /* Hairline / divider tint */
```

#### Shadows + eases

```css
--shadow-tile-rest:  0 1px 0 rgba(245,240,230,0.06) inset, 0 24px 54px -38px rgba(0,0,0,0.92);
--shadow-tile-hover: 0 1px 0 rgba(245,240,230,0.10) inset, 0 30px 64px -40px rgba(0,0,0,0.96);

--ease-tap:    cubic-bezier(0.2,  0.8, 0.2, 1);
--ease-glide:  cubic-bezier(0.16, 1,   0.3, 1);
--ease-cinema: cubic-bezier(0.22, 1,   0.36, 1);
```

#### Type scale

```css
--text-display-xl: 120px;  /* line-height 0.95, weight 700 */
--text-display-l:   84px;  /* line-height 1.0,  weight 700 */
--text-display-m:   56px;  /* line-height 1.05, weight 600 */
--text-display-s:   40px;  /* line-height 1.08, weight 600 */
--text-body-l:      20px;  /* line-height 1.55              */
--text-body-m:      16px;  /* line-height 1.6               */
--text-body-s:      14px;  /* line-height 1.55              */
```

#### Default theme

`<meta name="theme-color" content="#111713">` — dark default, no light-mode toggle.

---

## 2. Color usage discipline (new rules layered over Ember)

Ember provides the palette. The upgrade adds **rules about how to use it** specific to research positioning.

### 2.1 Surface hierarchy

| Layer | Token | When to use |
|---|---|---|
| Page background | `--color-ink-1` (#111713) | Default body background |
| Section / panel | `--color-wine` (#161D18) | Slightly elevated section, used sparingly |
| Card / quote / call-out | `--color-bordeaux` (#243028) | A self-contained block of content |
| Hairline / divider | `--color-mauve-fog` (#26312A) at 1px | Section breaks, table borders |
| Inline `code` background | `--color-paper` (#141A16) | Inline code spans |

### 2.2 Text hierarchy

| Use | Token | Example |
|---|---|---|
| Body text on dark surfaces | `--color-cream` (#F5F0E6) | Default body color |
| Secondary / muted text | `rgba(245,240,230,0.72)` | Captions, metadata, timestamps |
| Tertiary / very muted | `rgba(245,240,230,0.48)` | Footer text, fine print |
| Headings | `--color-cream` (same as body, just larger) | h1, h2, h3 — color is identical to body |
| Inline accent emphasis | `--color-amber` (#F3B85B) | The `.` in the wordmark; rare emphasis |
| Code text | `--color-honey` (#8AD18F) | Inline `code` and code blocks |

### 2.3 Accent usage rules

The accent colors (`ember`, `amber`, `honey`) must appear **sparingly**. A research site that's all accents reads as desperate.

| Accent | Permitted uses | Forbidden |
|---|---|---|
| `--color-ember` (#66D19E mint) | Hover-state on primary CTA, hover-state on internal links, the small hairline-glow under the homepage hero, focus-visible ring | Body backgrounds, large blocks, gradients |
| `--color-amber` (#F3B85B gold) | The `.` in the wordmark, h2 prefix dot if used, "current research" badge | Body text, links, large headings |
| `--color-honey` (#8AD18F) | Inline code text, syntax highlighting (keywords) | Body text |

A given page should display the accent in **at most 4 places** (wordmark dot + 1 CTA + 2 inline emphases). More than 4 = over-decorated.

### 2.4 Anti-Anthropic-imitation guard

Anthropic's brand uses a warm amber/orange palette that visually overlaps with our `--color-amber` and `--color-honey`. To avoid looking like a tribute act:

- Never use orange-orange (#FF6A00 territory). The warmest hue we use is `--color-amber` #F3B85B (a softer gold).
- Never use cream-on-amber large surfaces (Anthropic's signature combo).
- Keep dark backgrounds the dominant surface (Anthropic uses light backgrounds primarily).

### 2.5 Forbidden colors

These do not appear anywhere:
- Pure white (#FFFFFF) — too harsh on Ember's warm palette
- Pure black (#000000) — `--color-ink-0` is the deepest we go
- Cool greys (#888, #aaa, etc.) — clashes with the warm-dark palette
- Cool blues (Twitter / Stripe / SaaS palette)
- Bright reds (error states already covered by Ember)
- Pure neon greens — `--color-ember` is jade-mint, not arcade-green

---

## 3. Typography

### 3.1 Inherited from existing `layout.tsx` — keep as-is

```ts
const inter = Inter({
  variable: "--font-inter",
  weight: "variable",
});

const sourceSerif = Source_Serif_4({
  variable: "--font-source-serif",
  weight: "variable",
  style: ["normal", "italic"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains",
  weight: "variable",
});
```

These three font families are the entire type system. No additional fonts.

### 3.2 Family assignments

| Element | Family | Reasoning |
|---|---|---|
| Body text | **Source Serif 4** (`var(--font-source-serif)`) | Serif body = research-grade default; Source Serif is open-source, multilingual, and legible at any size |
| Headings (h1, h2, h3) | **Source Serif 4** | Heading-body family unity is more academic than mixed-family sites |
| Display headings (homepage hero) | **Source Serif 4** at large display sizes | Same family, larger size — keeps the unity |
| Navigation, buttons, CTAs | **Inter** (`var(--font-inter)`) | Sans-serif for UI chrome — lets body serifs do the heavy lifting |
| Captions / metadata / timestamps | **Inter** | Smaller UI text reads better in sans |
| Code | **JetBrains Mono** (`var(--font-jetbrains)`) | Existing |

### 3.3 Type scale (matches Ember tokens)

| Class / token | Size | Line-height | Weight | Use |
|---|---|---|---|---|
| `text-display-xl` | 120px | 0.95 | 700 | (avoid in this upgrade — too large for restraint) |
| `text-display-l` | 84px | 1.0 | 700 | Homepage hero on desktop |
| `text-display-m` | 56px | 1.05 | 600 | Page h1 (research, paper, about, contact) |
| `text-display-s` | 40px | 1.08 | 600 | Section h2, mobile hero |
| `text-body-l` | 20px | 1.55 | 400 | Hero subhead, large lead paragraphs |
| `text-body-m` | 16px | 1.6 | 400 | Default body |
| `text-body-s` | 14px | 1.55 | 400 | Captions, metadata, footer |

### 3.4 Specific h1/h2/h3 spec

```css
h1 {
  font-family: var(--font-source-serif);
  font-size: var(--text-display-m);
  line-height: 1.05;
  font-weight: 600;
  letter-spacing: -0.015em;
  color: var(--color-cream);
}

h2 {
  font-family: var(--font-source-serif);
  font-size: var(--text-display-s);
  line-height: 1.08;
  font-weight: 600;
  letter-spacing: -0.01em;
  color: var(--color-cream);
  margin-top: 56px;
  padding-top: 24px;
  border-top: 1px solid var(--color-mauve-fog);
}

h3 {
  font-family: var(--font-source-serif);
  font-size: var(--text-body-l);   /* 20px */
  line-height: 1.4;
  font-weight: 600;
  color: var(--color-cream);
  margin-top: 32px;
}
```

### 3.5 Body paragraph spec

```css
p {
  font-family: var(--font-source-serif);
  font-size: var(--text-body-m);  /* 16px */
  line-height: 1.6;
  color: var(--color-cream);
  margin-bottom: 18px;
}

p:last-child {
  margin-bottom: 0;
}
```

### 3.6 Inline `code` spec

```css
code {
  font-family: var(--font-jetbrains);
  font-size: 0.92em;        /* slightly smaller than body */
  background: var(--color-paper);
  color: var(--color-honey);
  padding: 1px 6px;
  border-radius: 3px;
  font-feature-settings: "calt" 1;
}
```

### 3.7 Font loading

Already configured via `next/font/google` in `layout.tsx` with `display: "swap"` and `preload: false`. No changes needed.

### 3.8 What NOT to do typographically

- No more than 3 type families on a page (Source Serif + Inter + JetBrains Mono is the cap)
- No font sizes between scale steps (use the scale; don't invent 19px)
- No `text-align: justify` (creates rivers in serif body)
- No all-caps body text
- No letter-spacing > 0.05em on body
- No condensed or expanded variants
- No drop caps
- No fancy first-letter styling
- No animated text reveals on scroll

---

## 4. Spacing and layout

### 4.1 Spacing scale (Tailwind defaults; do not override)

```
0    px       (no space)
1    4px      (hairline gap)
2    8px      (intra-component)
3    12px
4    16px     (default paragraph margin)
5    20px
6    24px     (section internal padding)
7    28px
8    32px     (h3 margin-top)
10   40px
12   48px
14   56px     (h2 margin-top)
16   64px     (page section gap on desktop)
20   80px
24   96px     (page top/bottom padding on desktop)
32   128px    (hero vertical breathing room)
```

### 4.2 Container widths

| Container | Max-width | When |
|---|---|---|
| Reading-width container | `max-w-[660px]` (660px) | Body copy on most pages — proven readable line length |
| Standard page container | `max-w-[1024px]` | Pages with multi-column or wider content |
| Hero / display container | `max-w-[1200px]` | Homepage hero only |
| Full-bleed | `100%` | Header, footer, decorative dividers only |

A research site does not need 1440px wide layouts. Restraint matters.

### 4.3 Page padding (responsive)

| Breakpoint | Horizontal page padding | Vertical page padding |
|---|---|---|
| `< 640px` (mobile) | `28px` | `48px top, 64px bottom` |
| `640–1023px` | `40px` | `64px top, 80px bottom` |
| `≥ 1024px` | `64px` | `96px top, 128px bottom` |

These get applied at the `<main>` element on each page.

### 4.4 Vertical rhythm

Within a page, vertical spacing follows this rhythm:

```
Hero block               → mb-24 (96px) on desktop, mb-16 (64px) mobile
Section (h2 + content)   → mb-16 (64px) on desktop, mb-12 (48px) mobile
Subsection (h3 + content)→ mb-10 (40px) on desktop, mb-8 (32px) mobile
Paragraph                → mb-4 (16px), with last:mb-0
List item                → mb-2 (8px)
```

---

## 5. Layout grid

A formal grid is overkill for a 5-page research site. Use Tailwind's container + `flex`/`grid` primitives directly. The only fixed grids are:

- **Header nav** — `flex justify-between items-center` (wordmark left, nav right)
- **Footer** — `flex flex-col gap-2` on mobile, `flex justify-between` on desktop
- **Research page artifact list** — `grid grid-cols-1 md:grid-cols-2 gap-6` for the 2-up of paper + companion

No CSS Grid for body content layout. Body copy is single-column, max 660px.

---

## 6. Iconography

### 6.1 Icon library

The site uses **Lucide React** (already a dependency: `lucide-react@^1.11.0`). This is the only icon library.

### 6.2 Icon usage rules

- Maximum 1 icon per CTA. CTAs may have a single right-arrow (`ArrowRight`) only.
- Section headings do **not** have icons. Section headings are typographic only.
- Body text does **not** have inline icons.
- The footer does **not** have social-media icons (no Twitter / GitHub / LinkedIn icons in chrome).
- Icons in CTAs render at 16×16px (matching the body text size).
- Icons inherit the current color via `currentColor`.

### 6.3 Allowed icons (for the entire site)

| Icon | When | Component |
|---|---|---|
| `ArrowRight` | Primary CTA (e.g., "Read research →") | `<ArrowRight size={16} className="ml-1.5" />` |
| `ExternalLink` | Outbound link in body to external site (e.g., GitHub) | `<ExternalLink size={14} className="ml-1 inline-block" />` |
| `Download` | PDF download button | `<Download size={16} className="mr-1.5" />` |
| `Mail` | Email CTA, inline only when adjacent to address | `<Mail size={16} className="mr-1.5" />` |

That's it. Four icons. The discipline is the design.

### 6.4 What NOT to use

- No animated icons (no Lottie, no spinning loaders in non-async contexts)
- No emoji in body or headings
- No decorative icons (e.g., a "research" icon next to the Research nav item)

---

## 7. Motion

### 7.1 Existing motion eases (keep)

```css
--ease-tap:    cubic-bezier(0.2,  0.8, 0.2, 1);   /* button press */
--ease-glide:  cubic-bezier(0.16, 1,   0.3, 1);   /* smooth state changes */
--ease-cinema: cubic-bezier(0.22, 1,   0.36, 1);  /* hero / large reveals */
```

### 7.2 Motion philosophy for the upgrade

Less than the current site. Specifically:

| Element | Motion | Duration |
|---|---|---|
| Link hover | Color transition (cream → ember underline) | 120ms `--ease-tap` |
| Button hover | Background + border color transition | 150ms `--ease-tap` |
| CTA press | Subtle scale 1.0 → 0.99 then back | 100ms `--ease-tap` |
| Page load | None (no fade-in, no slide-up) | n/a |
| Scroll-triggered | None | n/a |
| Hero reveal | None — no animation on first paint | n/a |

### 7.3 No-motion preference

`prefers-reduced-motion: reduce` disables all transitions. Implement at the global CSS level:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0ms !important;
    scroll-behavior: auto !important;
  }
}
```

This already exists if `globals.css` was set up correctly — verify and add if missing.

### 7.4 What NOT to do

- No parallax scrolling
- No GSAP / Framer Motion animations on text reveals
- No animated number counters
- No animated background gradients
- No particles, blobs, or "AI-shimmering" effects
- No animated underline draws on link hover (just color change)
- No carousel rotation (no carousels at all)

---

## 8. Borders, dividers, hairlines

### 8.1 Standard hairline

```css
border: 1px solid var(--color-mauve-fog);  /* #26312A */
```

Used on:
- h2 padding-top divider (the line above each section heading)
- Card / quote borders
- Footer top border
- Table cell borders

### 8.2 Stronger separation (rare)

```css
border-top: 1px solid var(--color-bordeaux);  /* #243028 */
```

Used on:
- The line between header nav and page content
- Footer top divider on the homepage only

### 8.3 No border-radius rules

Border-radius defaults:
- Buttons / CTAs: `border-radius: 4px`
- Cards / surfaces: `border-radius: 8px`
- Code blocks: `border-radius: 6px`
- Inline `code`: `border-radius: 3px`
- Images / OG: `border-radius: 8px`

No more than 8px anywhere. Rounded blobs (16px+) read as marketing.

---

## 9. Buttons and CTAs

### 9.1 Primary CTA spec

```tsx
<Link
  href="/research"
  className="
    inline-flex items-center
    px-5 py-3
    bg-ember text-ink-1
    font-sans font-medium text-base
    rounded
    transition-colors duration-150 ease-[var(--ease-tap)]
    hover:bg-honey
    focus-visible:outline-none
    focus-visible:ring-2 focus-visible:ring-ember focus-visible:ring-offset-2 focus-visible:ring-offset-ink-1
  "
>
  Read current research
  <ArrowRight size={16} className="ml-1.5" />
</Link>
```

Styling rules:
- Background: `--color-ember`
- Text color: `--color-ink-1` (dark text on the bright button)
- Padding: `20px horizontal, 12px vertical`
- Font: Inter, 16px, weight 500
- Border-radius: 4px
- Hover: background → `--color-honey`
- No shadow on the button itself
- Right-arrow icon, 16px, with `ml-1.5` (6px gap)

### 9.2 Secondary CTA spec

```tsx
<Link
  href="mailto:chrisroyseai@gmail.com?subject=Teleox"
  className="
    inline-flex items-center
    px-5 py-3
    border border-cream/30
    text-cream
    font-sans font-medium text-base
    rounded
    transition-colors duration-150 ease-[var(--ease-tap)]
    hover:border-cream/60 hover:bg-cream/5
    focus-visible:outline-none
    focus-visible:ring-2 focus-visible:ring-cream/40 focus-visible:ring-offset-2 focus-visible:ring-offset-ink-1
  "
>
  Email Chris
</Link>
```

Styling rules:
- Background: transparent
- Text color: `--color-cream`
- Border: `1px solid rgba(245,240,230,0.3)`
- Hover: border opacity → 0.6, background tint → `cream/5`
- No icon (the secondary CTA is text-only)

### 9.3 Tertiary / ghost link

```tsx
<a
  href="https://github.com/ChrisRoyse/clipcannon"
  className="
    inline-flex items-center
    text-cream/72
    underline decoration-dotted underline-offset-4
    hover:text-cream hover:decoration-solid
    transition-colors duration-150
  "
>
  github.com/ChrisRoyse/clipcannon
  <ExternalLink size={14} className="ml-1 inline-block" />
</a>
```

For external research/code links in body copy. Dotted underline + ExternalLink icon.

### 9.4 CTA pairing rules

A page may have:
- 1 primary CTA
- 1 secondary CTA
- Any number of tertiary / inline links in body

A page may NOT have:
- Two primary CTAs side by side
- A "third" CTA option ("Read research" / "Email Chris" / "Subscribe to updates")
- Floating CTAs / fixed-bottom-bar CTAs
- A CTA in a sticky header

---

## 10. Tables

Used on `/research` for paper metadata and on the four-pillar mapping if it ports from the AMI landing page.

```css
table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-body-s); /* 14px */
  margin-bottom: 24px;
}

th {
  font-family: var(--font-inter);
  font-weight: 600;
  font-size: 12px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: rgba(245,240,230,0.72);
  padding: 10px 12px;
  text-align: left;
  border-bottom: 1px solid var(--color-bordeaux);
}

td {
  padding: 10px 12px;
  text-align: left;
  border-bottom: 1px solid var(--color-mauve-fog);
  vertical-align: top;
  font-family: var(--font-source-serif);
}
```

---

## 11. Code blocks (rare on this site)

Used on `/research` if any architecture is shown inline.

```css
pre {
  background: var(--color-paper);
  border: 1px solid var(--color-mauve-fog);
  border-radius: 6px;
  padding: 16px;
  overflow-x: auto;
  font-family: var(--font-jetbrains);
  font-size: 13px;
  line-height: 1.5;
  margin-bottom: 24px;
  color: var(--color-cream);
}

pre code {
  background: transparent;
  padding: 0;
  color: inherit;
}
```

If syntax highlighting is needed, use server-side rendering via Shiki (no client-side highlighter — keeps JS bundle small).

---

## 12. Forms

The site has no forms in the upgraded scope. The only "form" is `mailto:` links. If a future iteration adds a contact form, this section needs an addendum.

---

## 13. Image treatment

| Image type | Treatment |
|---|---|
| OG image | Static PNG, 1200×630, generated once |
| In-body screenshot (rare) | `border-radius: 8px`, `border: 1px solid var(--color-bordeaux)` |
| Author photo (if used on /about) | `border-radius: 50%`, max 120×120px, no border |
| Diagram (rare) | SVG inline with currentColor strokes; no PNG diagrams |

No background images. No hero images. No lifestyle photography. No stock illustration.

---

## 14. Light mode

**Not implemented.** Dark mode is the only mode. The existing decision (`memory/decisions/agent-04-design-system-lead--dark-mode-default.md`) makes this final.

If a future visitor system-prefers light mode, they get dark mode anyway. This is intentional — the design is calibrated for dark.

---

## 15. Tailwind v4 mapping (utility classes available)

Because the Ember tokens are in the `@theme` block, Tailwind v4 emits utility classes for all of them:

```
bg-ink-0      bg-ink-1      bg-wine        bg-bordeaux    bg-burgundy
bg-ember      bg-amber      bg-honey       bg-peach       bg-cream
bg-paper      bg-mauve-fog

text-ink-0    text-ink-1    text-wine      text-bordeaux  text-burgundy
text-ember    text-amber    text-honey     text-peach     text-cream

border-ink-0  border-ink-1  border-wine    border-bordeaux  border-mauve-fog

font-body          → Source Serif 4
font-sans          → Inter
font-mono          → JetBrains Mono

shadow-tile-rest   shadow-tile-hover
ease-tap           ease-glide          ease-cinema

text-display-xl    text-display-l      text-display-m   text-display-s
text-body-l        text-body-m         text-body-s
```

Use these utility classes directly in TSX. Do not write custom classes for color or type.

---

## 16. Verification

After implementation:

- [ ] No new color tokens added beyond Ember
- [ ] No new font families added beyond Inter / Source Serif / JetBrains
- [ ] No animation longer than 200ms
- [ ] No light-mode toggle
- [ ] No icons in body copy
- [ ] No emojis anywhere
- [ ] All buttons render with the spec'd radii (≤ 8px)
- [ ] All h2 elements have the spec'd `border-top` divider
- [ ] All body copy is in Source Serif, sized 16px, line-height 1.6
- [ ] All UI chrome (nav, buttons) is in Inter
- [ ] All code is in JetBrains Mono with `--color-honey` text
