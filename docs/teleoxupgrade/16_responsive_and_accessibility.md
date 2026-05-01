# 16 — Responsive Design & Accessibility

**Authority over:** breakpoints, mobile/tablet/desktop layout adjustments, ARIA, keyboard navigation, screen reader behavior, color contrast, motion preferences.

---

## 1. Breakpoints

The site uses Tailwind's default breakpoint scale:

| Name | Min width | Use case |
|---|---|---|
| (default) | 0px | Mobile-first base |
| `sm:` | 640px | Large phones / small tablets |
| `md:` | 768px | Tablets, desktop nav appears |
| `lg:` | 1024px | Desktop |
| `xl:` | 1280px | Large desktop |
| `2xl:` | 1536px | (rarely used) |

The site does NOT use `2xl:` for any layout decisions. Up through `xl:`, layouts adapt; beyond `xl:` content stays the same width with more whitespace.

---

## 2. Page-level responsive rules

### 2.1 Page padding

Defined in `PageContainer` and reproduced in `04_visual_design_system.md` §4.3:

| Breakpoint | Horizontal | Vertical (top / bottom) |
|---|---|---|
| `< 640px` | 28px (`px-7`) | 48px / 64px (`pt-12 pb-16`) |
| `640–1023px` | 40px (`md:px-10`) | 64px / 80px (`md:pt-16 md:pb-20`) |
| `≥ 1024px` | 64px (`lg:px-16`) | 96px / 128px (`lg:pt-24 lg:pb-32`) |

### 2.2 Container max-widths

| Container variant | Max width |
|---|---|
| `reading` (default) | 660px |
| `standard` | 1024px |
| `hero` | 1200px |

These do not change per breakpoint — the container is always the smaller of (viewport width minus padding) and (max-width).

### 2.3 Hero scaling

| Element | Mobile | Tablet | Desktop |
|---|---|---|---|
| h1 size | 40px | 56px | 84px (homepage), 56px (subpages) |
| h1 line-height | 1.08 | 1.05 | 1.0 |
| Subhead size | 18px | 18px | 20px |
| Subhead max-width | full | 640px | 640px |
| CTA pair direction | column | row | row |

### 2.4 Section heading scaling

| Element | Mobile | Tablet | Desktop |
|---|---|---|---|
| h2 size | 28px | 32px | 40px |
| h2 line-height | 1.1 | 1.1 | 1.08 |
| h2 top margin | 48px | 56px | 64px |

### 2.5 Body text

Single size across all breakpoints: 16px on mobile, 18px tablet+ for prose-heavy pages. Line-height 1.6.

### 2.6 Grid behavior

| Component | Mobile | Tablet | Desktop |
|---|---|---|---|
| ArtifactCard grid (research page) | 1 column | 1 column | 2 columns |
| Footer layout | stacked | stacked | flex row |
| Header nav | hamburger drawer | inline links | inline links |

---

## 3. Mobile drawer behavior

Specified in `12_navigation_and_footer.md` §1.7. Summary:

- Triggered by hamburger button on `< 768px`
- Full-screen `bg-ink-1` overlay, `z-40`
- Body scroll locked while open
- Closes on: X tap, link tap, Escape key

---

## 4. Touch targets

All interactive elements (buttons, links, hamburger, etc.) have a tap target ≥ 44×44px (per WCAG 2.5.5 AAA recommendation, also Apple HIG).

Examples:
- Nav links: visually 14px text, but the surrounding `py-4` on the header gives 24px+ vertical padding → tap target ≥ 56px tall
- Hamburger button: explicit `p-2 -mr-2` → 40px tap target with 8px padding on each side
- CTAs: `px-5 py-3` → 16px text + 24px vertical → 64px tall, well over 44px

---

## 5. Color contrast (WCAG AA + AAA where possible)

| Pairing | Contrast ratio | WCAG level |
|---|---|---|
| `--color-cream` #F5F0E6 on `--color-ink-1` #111713 | ~14.5:1 | AAA (large + small text) |
| `cream/82` (#F5F0E6 at 82%) on `--color-ink-1` | ~11.9:1 | AAA |
| `cream/72` on `--color-ink-1` | ~10.4:1 | AAA |
| `cream/60` on `--color-ink-1` | ~8.7:1 | AAA |
| `cream/48` on `--color-ink-1` | ~6.9:1 | AAA |
| `cream/40` on `--color-ink-1` | ~5.8:1 | AA large + AAA large |
| `--color-ink-1` on `--color-ember` #66D19E (CTA primary) | ~14.5:1 | AAA |
| `--color-amber` #F3B85B on `--color-ink-1` (wordmark dot) | ~9.4:1 | AAA |
| `--color-honey` #8AD18F on `--color-ink-1` (code) | ~10.4:1 | AAA |

The cream-on-dark scheme is contrast-rich. Anything using `cream/40` or higher passes WCAG AA for body text. `cream/30` and below should only be used for decorative borders, not text.

### 5.1 Forbidden contrast pairings

- White on amber: not used in upgrade
- Cream on cream: never
- Ember on cream: too low contrast (~2:1)
- Cream on bordeaux: low contrast (~3.5:1) — only OK for large text or decorative

---

## 6. Keyboard navigation

### 6.1 Tab order

A user pressing Tab from page load should encounter elements in this order:

1. **Skip link** ("Skip to main content") — visible on focus
2. **Wordmark** (header) — links to /
3. **Nav links** in left-to-right order (Research, Paper, About, Contact)
4. **Hamburger button** (mobile only) OR
4'. **Page H1** (no, headings are not focusable; effectively skips)
5. **First interactive element in `<main>`** (typically the primary CTA)
6. **Subsequent interactive elements** in DOM order

### 6.2 Focus rings

Every interactive element has a visible focus ring:

```css
focus-visible:outline-none
focus-visible:ring-2
focus-visible:ring-ember        (or appropriate color)
focus-visible:ring-offset-2
focus-visible:ring-offset-ink-1
```

Use `:focus-visible` (not `:focus`) so the ring only appears on keyboard focus, not mouse click.

### 6.3 Escape behavior

| Context | Escape key action |
|---|---|
| Mobile drawer open | Close drawer |
| Modal open | Close modal (no modals currently) |
| Dropdown open | Close dropdown (no dropdowns currently) |
| Default | No action |

### 6.4 Enter / Space behavior

| Element | Enter | Space |
|---|---|---|
| Link (`<a>`) | Activate (navigate) | n/a |
| Button (`<button>`) | Activate | Activate |
| Hamburger | Toggle drawer | Toggle drawer |

Native HTML semantics handle this. Don't override with custom handlers unless necessary.

---

## 7. Screen reader behavior

### 7.1 Landmarks

Each page must have exactly:
- One `<header>` (the site header at top)
- One `<main>` (the page content) with `id="main"`
- One `<footer>` (the site footer at bottom)
- Optional `<nav>` elements with `aria-label`s if more than one

The site has only ONE `<nav>` typically (the one in the header). The footer's link group can also be `<nav aria-label="Footer navigation">`.

### 7.2 Heading hierarchy

Each page has exactly one `<h1>`. All section headings are `<h2>`. Sub-headings (rare) are `<h3>`. No skipping levels.

### 7.3 Image alt text

Every `<img>` and `<Image>` has meaningful `alt` text or `alt=""` (decorative).

| Image | Alt text |
|---|---|
| OG image (in metadata, not rendered as `<img>`) | "Teleox.ai — Models can be opaque. Pipelines shouldn't be." |
| Author photo | "Chris Royse" |
| Decorative dividers (none currently) | `alt=""` |

### 7.4 ARIA

- `aria-label` on the wordmark link: `"Teleox.ai — home"`
- `aria-label` on primary nav: `"Primary navigation"`
- `aria-label` on footer nav: `"Footer navigation"`
- `aria-current="page"` on the active nav item
- `aria-expanded`, `aria-controls` on the hamburger button
- `role="dialog"`, `aria-modal="true"`, `aria-label="Mobile navigation"` on the mobile drawer

### 7.5 Live regions

Not used. The site is static; no async updates require `aria-live`.

### 7.6 Forms

No forms in the upgrade. If added later, label every input explicitly with `<label for>` (not just `placeholder`).

---

## 8. Reduced motion

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

Add to `globals.css` if not already present. This is a single declaration that satisfies WCAG 2.3.3 (Animation from Interactions).

---

## 9. Color-blind friendliness

The site's color usage is heavily monochromatic (cream on dark) with sparse accent (amber, ember, honey). All meaningful information is conveyed via:

- Text content (read by screen readers and visually)
- Layout / position (not by color alone)
- Icons (Lucide icons have their own shape, not just color)

The amber dot in the wordmark is purely decorative and is also visually present as a punctuation period — color is not the only carrier.

The active nav indicator (the amber dot suffix) is supplemented by `aria-current="page"` and the brightness change from `cream/82` to `cream` — three signals (color + brightness + ARIA), not just color.

---

## 10. Print stylesheet (low priority)

A research site can reasonably expect occasional print attempts (someone printing the paper or about page). Add a minimal print stylesheet in `globals.css`:

```css
@media print {
  header, footer, nav, .no-print {
    display: none !important;
  }
  body {
    background: white !important;
    color: black !important;
    font-family: Georgia, serif !important;
  }
  a {
    text-decoration: underline !important;
  }
  a[href^="http"]::after {
    content: " (" attr(href) ")";
    font-size: 90%;
  }
  main {
    max-width: 6.5in;
    margin: 0;
    padding: 0;
  }
}
```

Optional. Skip if time is short.

---

## 11. Testing

### 11.1 Automated

```bash
npm run test:a11y       # Playwright + axe-core on every page
npm run test:lhci       # Lighthouse CI
```

Both should pass with zero violations.

### 11.2 Manual

- Tab through every page from top to bottom; verify no keyboard trap
- Operate the mobile drawer with keyboard alone (Tab to hamburger, Enter to open, Tab through items, Escape to close)
- Use VoiceOver (macOS) or NVDA (Windows) to navigate the homepage
- Resize the browser from 320px to 1920px width; verify no horizontal scroll, no overlapping text, no broken layout
- Enable `prefers-reduced-motion` in OS settings; verify all transitions disappear
- Use a color-blindness simulator (Stark, Sim Daltonism); verify nothing critical relies on color discrimination

### 11.3 Specific assertions

- [ ] Skip link is the first focusable element
- [ ] Skip link reveals on focus
- [ ] Skip link routes focus to `<main>` content
- [ ] Tab through mobile drawer cycles through all items, no trap
- [ ] Escape closes mobile drawer
- [ ] All text passes WCAG AA contrast
- [ ] All buttons / links have visible focus rings on keyboard focus
- [ ] No text below 14px in the layout
- [ ] No tap targets below 44×44px
- [ ] Page renders correctly at 320px width (smallest supported)
- [ ] Page renders correctly at 1920px width
- [ ] No horizontal scrolling at any breakpoint
- [ ] Reduced motion disables all transitions
- [ ] Screen reader announces page title, headings, landmarks correctly
