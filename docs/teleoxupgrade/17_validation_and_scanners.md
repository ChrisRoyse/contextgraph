# 17 — Validation Gates & Scanners

**Authority over:** the `scripts/scan-*` and `scripts/validate-*` scripts that gate the build.
**Goal:** every new content file the upgrade produces passes all gates before deploy.

---

## 1. Scanners that exist and run on every build

Per `next.config.ts` (the `runComplianceGate()` function), these scripts run during `PHASE_PRODUCTION_BUILD`:

| Script | What it scans | Exit code on violation |
|---|---|---|
| `validate:content` | TypeScript route copy files for required fields, signal IDs, route registration | 1 (build fails) |
| `validate:assets` | `public/` for orphaned or missing assets | 1 |
| `scan:claims` | All TSX, MD, JSON, TXT in `src/` and `public/` for unsupported claim phrases | 1 |
| `scan:public-posture` | Same files for "internal-only" vocabulary leaks | 1 |
| `scan:scope-guards` | TSX for scope-guard rule violations | 1 |
| `scan:event-names` | Analytics event names for naming convention | 1 |
| `scan:signals` | Signal IDs and bundle integrity | 1 |

Plus rendered-HTML scanners that run from `npm run postbuild` after `.next/server/app/*.html` exists.

---

## 2. Each scanner — what it enforces

### 2.1 `validate-content.mjs`

```bash
npm run validate:content
```

Checks every `RouteCopy` file:
- Has `path`, `purpose`, `audience`, `playerClasses`, `signals`, `heroHeadline`, `bodyBlocks`
- `path` matches a canonical route in `src/lib/canonical-routes.mjs`
- `signals` are valid `SignalId` values
- `bodyBlocks[*].paragraphs` is non-empty
- `mustNotSay` array, if present, is array of strings
- `playerClasses` are valid `PlayerClass` enum values

**For the upgrade:** as you reposition each route, the file remains a valid `RouteCopy` — none of these structural fields are removed. No changes to the validator needed.

### 2.2 `validate-assets.mjs`

Checks `public/` for:
- Required favicon files (`favicon.ico`, `icon.svg`, `apple-touch-icon.png`)
- Manifest exists and is valid JSON
- OG image exists and is correct dimensions

**For the upgrade:** ensure new `og-image.png` is in `public/` before the validator runs.

### 2.3 `scan-claims.ts`

Scans for **unsupported claim phrases**. Each `RouteCopy.mustNotSay` array contributes patterns. Plus a global block list in the script itself.

**For the upgrade:** the new `mustNotSay` list (per `01_strategic_positioning.md` §6.3) is more comprehensive than the existing list. When you author the new RouteCopy files, include the full mustNotSay list (per spec) for each page.

If a phrase you legitimately need to say (e.g., the title of a paper that contains a banned word) trips the scanner, **do not weaken the scanner**. Either:
- Rephrase
- Add an explicit allow-list entry in the scanner with documentation in `memory/decisions/`

### 2.4 `scan-public-posture.ts`

Scans for **internal-material leaks** — phrases that should never appear in public copy.

Banned vocabulary list (existing, with additions for the upgrade):

```
auction
mechanism design
internal-only
buyer
SaaS
sales pipeline
GTM
TAM / SAM / SOM
ARR
churn
funnel
upsell
landing page conversion
demo (as a noun referring to sales demo)
prospect
qualified lead
```

The new positioning intentionally removes "buyer" and "SaaS" framing, so this scanner should now find ZERO violations across the new pages. If it finds violations, the page's copy still has commercial residue — strip it.

**Specific allow-list for the upgrade:**
- `signal-bundle` is allowed in `src/lib/metadata.ts`, `src/types/signal.ts`, `src/content/signals.ts`, and `src/components/signals/SignalBundleMetaTag.tsx` (existing allow-list)
- No new allow-list entries needed

### 2.5 `scan-scope-guards.ts`

Enforces page-level scope guards. The upgrade's new pages have empty `scopeGuards: []`. If a future page needs scope guards, follow the existing pattern.

### 2.6 `scan-event-names.ts`

Validates analytics event names. Not relevant for the upgrade unless analytics events are added (they aren't).

---

## 3. Pre-build vs post-build

`next.config.ts` runs `validate:content` and `validate:assets` during `PHASE_PRODUCTION_BUILD` (before the build proper).

`npm run postbuild` (in `package.json`) runs `scan:all` after the build, against rendered HTML in `.next/server/app/`.

Both must pass for a successful deploy.

---

## 4. Updated `mustNotSay` list (canonical for new pages)

Every new RouteCopy uses this list:

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
  "Send data problem",
  "Talk to sales",
  "Schedule a call",
  "Book a demo",
  "buyer win",
  "data moat",
  "founder-led startup",
  "co-founders",
  "team of",
],
```

Page-specific additions:

| Page | Adds |
|---|---|
| `/about` | `"founder-led startup"`, `"co-founders"`, `"team of"` (already in canonical) |
| `/contact` | (use canonical) |
| `/research` | (use canonical) |
| `/paper` | (use canonical) |
| `/` | (use canonical) |

---

## 5. New `mustNotSay` enforcement: how the scan-claims script handles it

The existing `scan-claims.ts` reads each `RouteCopy.mustNotSay` array, then scans the rendered HTML output of that route for those strings (case-insensitive substring match by default).

**Checking the scanner's actual behavior:** read `scripts/scan-claims.ts` to confirm:
- Does it do substring or word-boundary matching?
- Does it check the bodyBlocks paragraphs only, or all rendered HTML?
- Does it match across element boundaries (e.g., word split across `<a>` tags)?

If the existing behavior is permissive (case-sensitive substring without word boundaries), it may miss "Revolutionary" while catching "revolutionary". Tighten to case-insensitive word-boundary matching:

```ts
const pattern = new RegExp(`\\b${phrase.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`, "i");
```

If updating the scanner, document in `memory/decisions/`.

---

## 6. CI gate

The deploy command (Vercel build) runs `npm run build`. That triggers:

1. `next.config.ts` `runComplianceGate()` → `validate:content`, `validate:assets`
2. `next build` itself
3. `npm run postbuild` → `scan:all`

If any step exits non-zero, the build fails and the deploy is blocked. This is the safety net.

---

## 7. Bypass mechanism (emergency only)

`next.config.ts` honors the `TELEOX_SKIP_SCAN=1` environment variable to bypass the gate:

```ts
if (process.env.TELEOX_SKIP_SCAN === "1") {
  process.stderr.write(
    "[next.config] TELEOX_SKIP_SCAN=1 — scanners bypassed. This is logged.\n",
  );
  process.env.TELEOX_SCAN_GATE_RAN = "1";
  return;
}
```

**Use this only in genuine emergencies.** Document every use in `memory/decisions/agent-XX--scan-bypass-justification-YYYYMMDD.md` per the existing convention.

---

## 8. Pre-deploy checklist

Before triggering the production deploy:

```bash
cd /home/cabdru/teleoxwebsite/teleox-website
npm run typecheck         # 0 errors
npm run lint              # 0 errors
npm run validate:content  # 0 failures
npm run validate:assets   # 0 failures
npm run test              # all unit tests pass
npm run scan:all          # 0 violations
npm run build             # successful
```

If any step fails, fix before deploy.

---

## 9. Adding a new scanner

If the upgrade needs a new gate (e.g., enforcing canonical URL casing), add it to `scripts/scan-*.ts`:

1. Create the script with the `Rule[]` pattern from existing scanners
2. Add to the `scan:all` orchestrator script
3. Document in `memory/decisions/agent-XX--new-scanner-X.md`
4. Wire into `next.config.ts` if it should run pre-build

For the upgrade itself, **no new scanners are required**. The existing infrastructure is sufficient.

---

## 10. Scanner verification post-upgrade

- [ ] `npm run validate:content` passes
- [ ] `npm run validate:assets` passes
- [ ] `npm run scan:claims` passes
- [ ] `npm run scan:public-posture` passes — specifically, "buyer" appears 0 times across `/`, `/research`, `/paper`, `/about`, `/contact`
- [ ] `npm run scan:scope-guards` passes
- [ ] `npm run scan:signals` passes
- [ ] `npm run scan:event-names` passes
- [ ] `npm run scan:all` passes end-to-end
- [ ] Production Vercel build completes successfully (which means all gates ran and passed)
