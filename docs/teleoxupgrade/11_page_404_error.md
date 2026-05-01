# 11 — 404 / Error / Not-Found Pages

**Routes:** `not-found.tsx`, `error.tsx`, `global-error.tsx`
**Files:** `src/app/not-found.tsx`, `src/app/error.tsx`, `src/app/global-error.tsx`
**Audience:** Anyone who hits a broken link or encounters a runtime error
**Single thing each must communicate:** "This is not what you wanted; here is the way back."

---

## 1. `not-found.tsx` (404 page)

### 1.1 When this renders

When Next.js cannot match the URL to any route. Triggered by:
- A typo in a URL
- An old / retired URL that doesn't have a redirect rule (rare; we have many)
- A user manually entering a non-existent path

### 1.2 Spec

```tsx
// src/app/not-found.tsx
import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Page not found — Teleox.ai",
  description: "This page does not exist on Teleox.ai. Try the research index, the paper, or contact.",
  robots: { index: false, follow: false },
};

export default function NotFound() {
  return (
    <main className="mx-auto max-w-[660px] px-7 pt-24 pb-32 md:px-10 lg:px-16">
      <p className="font-sans uppercase tracking-[0.12em] text-xs text-cream/60 mb-4">
        Page not found
      </p>

      <h1 className="font-body font-semibold text-cream tracking-tight text-[40px] md:text-[56px] leading-[1.05] mb-6">
        This page doesn't exist.
      </h1>

      <p className="font-body text-cream/82 text-[18px] leading-[1.6] mb-8">
        The URL you visited is not part of Teleox.ai. The site has a small,
        intentional surface — try one of these:
      </p>

      <ul className="list-disc list-outside pl-6 font-body text-[18px] leading-[1.6] text-cream [&>li]:mb-2">
        <li>
          <Link href="/research" className="underline decoration-cream/40 underline-offset-4 hover:text-honey hover:decoration-honey transition-colors">
            Research index
          </Link>{" "}
          — current papers and open-source artifacts
        </li>
        <li>
          <Link href="/paper" className="underline decoration-cream/40 underline-offset-4 hover:text-honey hover:decoration-honey transition-colors">
            Current paper
          </Link>{" "}
          — Dynamic / ME-JEPA, plain-language overview
        </li>
        <li>
          <Link href="/about" className="underline decoration-cream/40 underline-offset-4 hover:text-honey hover:decoration-honey transition-colors">
            About
          </Link>{" "}
          — who runs Teleox
        </li>
        <li>
          <Link href="/contact" className="underline decoration-cream/40 underline-offset-4 hover:text-honey hover:decoration-honey transition-colors">
            Contact
          </Link>{" "}
          — direct email
        </li>
        <li>
          <Link href="/" className="underline decoration-cream/40 underline-offset-4 hover:text-honey hover:decoration-honey transition-colors">
            Home
          </Link>
        </li>
      </ul>
    </main>
  );
}
```

### 1.3 Tone

Brief. Not cute. Not apologetic. Just states the fact and offers the way out.

### 1.4 What it does NOT have

- No "Oops! 😞" or any emoji
- No "404" giant typography (the heading does the work)
- No animated illustration
- No "Search the site" form (we have no search)
- No sad-face or 404-themed graphics

---

## 2. `error.tsx` (recoverable runtime error)

### 2.1 When this renders

When a runtime error occurs in a page or its children. Triggered by:
- Unhandled exception in a page component
- Network error in a server-side data fetch
- Any JS error that bubbles to a route

### 2.2 Spec

```tsx
// src/app/error.tsx
"use client";

import { useEffect } from "react";
import Link from "next/link";

type ErrorProps = {
  error: Error & { digest?: string };
  reset: () => void;
};

export default function Error({ error, reset }: ErrorProps) {
  useEffect(() => {
    console.error("Page error:", error);
  }, [error]);

  return (
    <main className="mx-auto max-w-[660px] px-7 pt-24 pb-32 md:px-10 lg:px-16">
      <p className="font-sans uppercase tracking-[0.12em] text-xs text-cream/60 mb-4">
        Something broke
      </p>

      <h1 className="font-body font-semibold text-cream tracking-tight text-[40px] md:text-[56px] leading-[1.05] mb-6">
        Something on this page broke.
      </h1>

      <p className="font-body text-cream/82 text-[18px] leading-[1.6] mb-6">
        A runtime error occurred while rendering this page. The site is intentionally
        small; this should not happen often. The error has been logged to the browser
        console.
      </p>

      {error.digest ? (
        <p className="font-mono text-[13px] text-cream/60 mb-8">
          Error reference: {error.digest}
        </p>
      ) : null}

      <div className="flex flex-col sm:flex-row gap-3 sm:gap-4">
        <button
          type="button"
          onClick={reset}
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
          Try again
        </button>

        <Link
          href="/"
          className="
            inline-flex items-center justify-center
            px-5 py-3
            border border-cream/30
            text-cream
            font-sans font-medium text-base
            rounded
            transition-colors duration-150
            hover:border-cream/60 hover:bg-cream/5
          "
        >
          Go home
        </Link>
      </div>
    </main>
  );
}
```

### 2.3 Tone

Acknowledge the error briefly. Offer two recovery actions: retry, go home. Provide an error reference if Next has assigned one.

---

## 3. `global-error.tsx` (root error fallback)

### 3.1 When this renders

When `error.tsx` itself fails, or an error occurs in the root layout.

### 3.2 Spec

```tsx
// src/app/global-error.tsx
"use client";

type GlobalErrorProps = {
  error: Error & { digest?: string };
  reset: () => void;
};

export default function GlobalError({ error, reset }: GlobalErrorProps) {
  return (
    <html lang="en">
      <body
        style={{
          margin: 0,
          padding: "64px 28px",
          background: "#111713",
          color: "#F5F0E6",
          fontFamily: "Georgia, serif",
          fontSize: "16px",
          lineHeight: 1.6,
        }}
      >
        <main style={{ maxWidth: 660, margin: "0 auto" }}>
          <h1 style={{ fontSize: 40, fontWeight: 600, marginBottom: 24 }}>
            Something broke at the root.
          </h1>

          <p style={{ marginBottom: 16 }}>
            A fatal rendering error occurred. The page could not load.
          </p>

          {error.digest ? (
            <p style={{ fontFamily: "monospace", fontSize: 13, opacity: 0.6, marginBottom: 24 }}>
              Error reference: {error.digest}
            </p>
          ) : null}

          <button
            type="button"
            onClick={reset}
            style={{
              padding: "12px 20px",
              background: "#66D19E",
              color: "#111713",
              border: 0,
              borderRadius: 4,
              cursor: "pointer",
              fontFamily: "Inter, system-ui, sans-serif",
              fontSize: 16,
              fontWeight: 500,
            }}
          >
            Reload
          </button>
        </main>
      </body>
    </html>
  );
}
```

### 3.3 Why inline styles

`global-error.tsx` renders **outside** the root `layout.tsx`. The Tailwind classes are not available because `globals.css` may not be loaded if the error occurred during layout setup. Inline styles guarantee the page is at least readable.

### 3.4 Tone

Brutally minimal. The CSS classes don't exist; the fonts may not have loaded; assume the worst-case rendering environment.

---

## 4. Test plan

- [ ] Visit `https://teleox.ai/this-does-not-exist` → 404 page renders correctly
- [ ] 404 page contains links to all 5 main routes (research, paper, about, contact, home)
- [ ] 404 page has `<meta name="robots" content="noindex, nofollow">`
- [ ] Force a runtime error in dev (e.g., `throw new Error("test")` in a page component) → `error.tsx` renders with "Try again" + "Go home" buttons
- [ ] Click "Try again" → page resets and re-renders
- [ ] Click "Go home" → navigates to /
- [ ] `error.tsx` logs the error to the browser console
- [ ] Force a layout-level error (rare) → `global-error.tsx` renders with inline styles, even without external CSS

---

## 5. What these pages deliberately do NOT have

- No 404 graphic (no broken-robot, no sad-emoji, no abstract illustration)
- No "Did you mean?" search/redirect suggestions (no fuzzy matching)
- No newsletter signup (we have no newsletter)
- No "Report this issue" form (the email is enough)
- No analytics tracking specific to error pages (Vercel Analytics catches them organically)
