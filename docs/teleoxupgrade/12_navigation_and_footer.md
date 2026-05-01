# 12 — Navigation, Header, and Footer

**Authority over:** the chrome that wraps every page — header, primary nav, mobile drawer, footer.
**Renders on:** every page including 404 (which uses the same layout shell).

---

## 1. Header

### 1.1 Behavior

- **Sticky** at top of viewport
- Background: `var(--color-ink-1)` at `80%` opacity with `backdrop-blur-sm` (so content scrolling under is dimly visible)
- Bottom border: `1px solid rgba(36,48,40,0.6)` (i.e. `border-bordeaux/60`)
- z-index: `30` (above page content, below modals)
- Height: `auto` (determined by content + padding); approximately 64px desktop, 56px mobile

### 1.2 Layout

Single horizontal row, `flex justify-between items-center`:

- **Left:** wordmark
- **Right (desktop):** four nav links horizontally
- **Right (mobile):** hamburger toggle button

### 1.3 Wordmark spec

```tsx
<Link
  href="/"
  className="font-sans font-semibold tracking-tight text-cream text-lg hover:text-honey transition-colors"
  aria-label="Teleox.ai — home"
>
  Teleox<span className="text-amber">.</span>ai
</Link>
```

The amber period (`.`) is the only decorative typography moment in the entire site. See `03_brand_identity.md` §2.

### 1.4 Desktop nav links

Order, left to right:

```
Research   Paper   About   Contact
```

Spec:

```tsx
<nav className="hidden md:flex items-center gap-8 font-sans text-[14px] text-cream/82" aria-label="Primary navigation">
  <Link href="/research" className="hover:text-cream transition-colors">Research</Link>
  <Link href="/paper" className="hover:text-cream transition-colors">Paper</Link>
  <Link href="/about" className="hover:text-cream transition-colors">About</Link>
  <Link href="/contact" className="hover:text-cream transition-colors">Contact</Link>
</nav>
```

Notes:
- 14px text (small enough to be chrome, large enough to be tappable)
- `text-cream/82` default → `text-cream` on hover (subtle brightness change)
- 32px gap between items (`gap-8`)
- Hidden on mobile (`hidden md:flex`)

### 1.5 Active state

When the current page matches a nav item, that item gets:
- `text-cream` (full brightness)
- A small `text-amber` dot suffix: `Research·` (the dot is the wordmark accent extended to indicate active state)

Implementation:

```tsx
import { usePathname } from "next/navigation";

function NavLink({ href, label }: { href: string; label: string }) {
  const pathname = usePathname();
  const isActive = pathname === href;
  return (
    <Link
      href={href}
      className={`${isActive ? "text-cream" : "text-cream/82 hover:text-cream"} transition-colors relative`}
      aria-current={isActive ? "page" : undefined}
    >
      {label}
      {isActive ? <span className="text-amber ml-0.5" aria-hidden>·</span> : null}
    </Link>
  );
}
```

### 1.6 Full Header.tsx

```tsx
// src/components/layout/Header.tsx
"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState, useEffect } from "react";
import { Menu, X } from "lucide-react";

const NAV_ITEMS = [
  { href: "/research", label: "Research" },
  { href: "/paper",    label: "Paper" },
  { href: "/about",    label: "About" },
  { href: "/contact",  label: "Contact" },
];

function NavLink({ href, label, onClick }: { href: string; label: string; onClick?: () => void }) {
  const pathname = usePathname();
  const isActive = pathname === href;
  return (
    <Link
      href={href}
      onClick={onClick}
      className={`${isActive ? "text-cream" : "text-cream/82 hover:text-cream"} transition-colors`}
      aria-current={isActive ? "page" : undefined}
    >
      {label}
      {isActive ? <span className="text-amber ml-0.5" aria-hidden>·</span> : null}
    </Link>
  );
}

export function Header() {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const pathname = usePathname();

  // Close drawer on route change
  useEffect(() => {
    setDrawerOpen(false);
  }, [pathname]);

  // Lock body scroll when drawer open
  useEffect(() => {
    if (drawerOpen) {
      document.body.style.overflow = "hidden";
    } else {
      document.body.style.overflow = "";
    }
    return () => { document.body.style.overflow = ""; };
  }, [drawerOpen]);

  return (
    <>
      <header className="border-b border-bordeaux/60 bg-ink-1/80 backdrop-blur-sm sticky top-0 z-30">
        <div className="max-w-[1200px] mx-auto px-7 md:px-10 lg:px-16 py-4 flex items-center justify-between">
          <Link
            href="/"
            className="font-sans font-semibold tracking-tight text-cream text-lg hover:text-honey transition-colors"
            aria-label="Teleox.ai — home"
          >
            Teleox<span className="text-amber">.</span>ai
          </Link>

          <nav className="hidden md:flex items-center gap-8 font-sans text-[14px] text-cream/82" aria-label="Primary navigation">
            {NAV_ITEMS.map((item) => (
              <NavLink key={item.href} {...item} />
            ))}
          </nav>

          <button
            type="button"
            className="md:hidden text-cream p-2 -mr-2"
            aria-label={drawerOpen ? "Close menu" : "Open menu"}
            aria-expanded={drawerOpen}
            aria-controls="mobile-nav-drawer"
            onClick={() => setDrawerOpen((v) => !v)}
          >
            {drawerOpen ? <X size={24} /> : <Menu size={24} />}
          </button>
        </div>
      </header>

      {drawerOpen ? (
        <div
          id="mobile-nav-drawer"
          className="fixed inset-0 z-40 bg-ink-1 md:hidden"
          role="dialog"
          aria-modal="true"
          aria-label="Mobile navigation"
        >
          <div className="flex items-center justify-between px-7 py-4 border-b border-bordeaux/60">
            <Link
              href="/"
              className="font-sans font-semibold tracking-tight text-cream text-lg"
              aria-label="Teleox.ai — home"
              onClick={() => setDrawerOpen(false)}
            >
              Teleox<span className="text-amber">.</span>ai
            </Link>
            <button
              type="button"
              className="text-cream p-2 -mr-2"
              aria-label="Close menu"
              onClick={() => setDrawerOpen(false)}
            >
              <X size={24} />
            </button>
          </div>

          <nav className="flex flex-col gap-6 p-7 font-sans text-[20px] text-cream" aria-label="Mobile navigation">
            {NAV_ITEMS.map((item) => (
              <NavLink key={item.href} {...item} onClick={() => setDrawerOpen(false)} />
            ))}
            <a
              href="mailto:chrisroyseai@gmail.com"
              className="mt-6 text-cream/72 text-[16px] hover:text-cream transition-colors"
              onClick={() => setDrawerOpen(false)}
            >
              chrisroyseai@gmail.com
            </a>
          </nav>
        </div>
      ) : null}
    </>
  );
}
```

### 1.7 Mobile drawer behavior

- Triggered by hamburger button on mobile (< 768px)
- Full-screen overlay (`fixed inset-0`)
- Background: solid `bg-ink-1` (no blur — the page behind is hidden)
- Renders the same 4 nav items vertically with larger text (20px)
- Includes the email address at the bottom for convenience
- Closes on:
  - Tapping the X button
  - Tapping a nav link (route change)
  - Pressing Escape (browser default with `role="dialog"` + `aria-modal="true"`)
- Locks body scroll while open

### 1.8 Accessibility

- `aria-label="Primary navigation"` on `<nav>`
- `aria-current="page"` on the active nav link
- `aria-label`, `aria-expanded`, `aria-controls` on the hamburger button
- `role="dialog"`, `aria-modal="true"`, `aria-label="Mobile navigation"` on drawer
- Drawer close button has `aria-label="Close menu"`
- Focus trap inside drawer (Next.js handles this with `aria-modal`, but verify with a screen reader test)

---

## 2. Footer

### 2.1 Behavior

- **Not sticky** — appears at the natural end of the page
- Top border: `1px solid var(--color-bordeaux)` (slightly stronger than header bottom border to signal "end of page")
- Background: same as page (transparent / `bg-ink-1`)
- Top margin: `96px` desktop (`mt-24`), `128px` extra-large desktop (`lg:mt-32`)

### 2.2 Layout (desktop)

Two-column flex on desktop:

```
[ Wordmark + email ]                    [ Nav links inline ]
[ © 2026 Teleox.ai. Independent research.                  ]
```

### 2.3 Layout (mobile)

Stacked:

```
[ Wordmark ]
[ email ]
[ Nav links flex-wrap ]
[ Copyright ]
```

### 2.4 Full Footer.tsx

```tsx
// src/components/layout/Footer.tsx
import Link from "next/link";

const FOOTER_NAV = [
  { href: "/research", label: "Research" },
  { href: "/paper",    label: "Paper" },
  { href: "/about",    label: "About" },
  { href: "/contact",  label: "Contact" },
  { href: "/blog",     label: "Blog" },
];

const FOOTER_META = [
  { href: "/blog/rss.xml", label: "RSS" },
  { href: "/llms.txt",     label: "llms.txt" },
];

export function Footer() {
  return (
    <footer className="border-t border-bordeaux mt-24 lg:mt-32 px-7 md:px-10 lg:px-16 py-12">
      <div className="max-w-[1024px] mx-auto flex flex-col md:flex-row md:justify-between md:items-end gap-6 mb-8">
        <div>
          <Link
            href="/"
            className="font-sans font-semibold tracking-tight text-cream text-base hover:text-honey transition-colors"
            aria-label="Teleox.ai — home"
          >
            Teleox<span className="text-amber">.</span>ai
          </Link>
          <p className="font-sans text-[13px] text-cream/60 mt-2">
            <a
              href="mailto:chrisroyseai@gmail.com?subject=Teleox"
              className="hover:text-cream transition-colors"
            >
              chrisroyseai@gmail.com
            </a>
          </p>
        </div>

        <nav className="flex flex-wrap gap-x-6 gap-y-2 font-sans text-[13px] text-cream/72" aria-label="Footer navigation">
          {FOOTER_NAV.map((item) => (
            <Link key={item.href} href={item.href} className="hover:text-cream transition-colors">
              {item.label}
            </Link>
          ))}
          {FOOTER_META.map((item) => (
            <a key={item.href} href={item.href} className="hover:text-cream transition-colors">
              {item.label}
            </a>
          ))}
        </nav>
      </div>

      <p className="font-sans text-[12px] text-cream/40 max-w-[1024px] mx-auto">
        © 2026 Teleox.ai. Independent research.
      </p>
    </footer>
  );
}
```

### 2.5 What is NOT in the footer

- No social-media icons (no Twitter / GitHub / LinkedIn icons)
- No "Made with ❤️ in [city]"
- No "Powered by Vercel" badge (Vercel doesn't require this)
- No language switcher
- No accessibility-toggle widget
- No site-search input
- No newsletter signup form
- No legal/privacy/terms links (these don't exist for this site; if added later, they go in footer)
- No multiple columns of "Resources / Company / Legal"
- No backlink to leapable.ai (mentioned in `/about` only)

---

## 3. Layout shell wiring

The `Header` and `Footer` are wrapped around every page via `RootLayoutShell`. Update or replace `src/components/layout/RootLayoutShell.tsx`:

```tsx
// src/components/layout/RootLayoutShell.tsx
import type { ReactNode } from "react";
import { Header } from "./Header";
import { Footer } from "./Footer";

export function RootLayoutShell({ children }: { children: ReactNode }) {
  return (
    <>
      <Header />
      {children}
      <Footer />
    </>
  );
}
```

Then `layout.tsx` continues to use `<RootLayoutShell>{children}</RootLayoutShell>` as it does today.

---

## 4. Skip-link (accessibility)

Add a "skip to main content" link as the first focusable element in `<body>`:

```tsx
// In src/app/layout.tsx, immediately inside <body>
<a
  href="#main"
  className="
    sr-only focus:not-sr-only
    focus:absolute focus:top-4 focus:left-4 focus:z-50
    bg-ember text-ink-1 px-4 py-2 rounded
    font-sans font-medium text-sm
    focus:outline-none focus:ring-2 focus:ring-cream
  "
>
  Skip to main content
</a>
```

This requires every page's `PageContainer` to render `<main id="main">`. Update `PageContainer` to add `id="main"`:

```tsx
return (
  <main id="main" className={...}>
    {children}
  </main>
);
```

---

## 5. Header rendering during scroll

The header has `backdrop-blur-sm` so content scrolling under it is visible-but-dimmed. Test that this:
- Works on Safari (which has historically had backdrop-filter quirks)
- Works on Firefox (Firefox supports backdrop-filter as of ~v103+)
- Falls back gracefully on browsers without support (the `bg-ink-1/80` semi-transparent background is the fallback)

---

## 6. Header height contribution to layout

Because the header is sticky, the page content needs to account for it. The `PageContainer` `pt-12 md:pt-16 lg:pt-24` already provides enough top spacing that there is visible whitespace below the sticky header.

If a page's hero starts immediately at the top with no breathing room, increase its top padding.

---

## 7. Test plan

- [ ] Header renders on all pages
- [ ] Wordmark links to `/`
- [ ] Active nav link has the amber-dot indicator
- [ ] Hover state on nav links increases brightness from `cream/82` → `cream`
- [ ] On mobile, hamburger toggle opens drawer
- [ ] Drawer renders all 4 nav items + email
- [ ] Drawer closes on link tap, X tap, and Escape key
- [ ] Body scroll is locked while drawer open
- [ ] Footer renders on all pages
- [ ] Footer email link opens system mail client
- [ ] Footer nav links route correctly
- [ ] Skip-link is the first focusable element on every page
- [ ] Skip-link visible only when focused (sr-only otherwise)
- [ ] Tab through the page: skip-link → wordmark → nav items → main content
- [ ] No console errors related to nav components
- [ ] All Vitest tests for Header / Footer pass
- [ ] Playwright e2e for nav passes (open drawer, navigate via drawer, verify route change)
