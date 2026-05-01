# Teleox.ai Upgrade — Specification Suite

**Date authored:** 2026-05-01
**Author:** spec produced by Claude (Opus 4.7) for Chris Royse
**Target implementer:** another AI agent executing the upgrade against `/home/cabdru/teleoxwebsite/teleox-website`

---

## What this is

A complete, end-to-end implementation specification for repositioning Teleox.ai from its current B2B-SaaS-flavored framing to a **research-led independent operation** explicitly designed to attract attention from frontier AI lab research leadership (with an invisible Anthropic-leaning bias).

Every document in this directory is *self-contained at the level required to execute its scope*. An implementing AI should not need to ask clarifying questions; if a question arises, the answer is in here.

---

## Read order

| # | Document | What it covers | Read this if you are… |
|---|---|---|---|
| 00 | `00_README.md` | This file. Index + decisions log. | Anyone, first |
| 01 | `01_strategic_positioning.md` | Audience, message hierarchy, voice, vocabulary | Anyone — this is the mission brief |
| 02 | `02_information_architecture.md` | Sitemap, route map, navigation taxonomy, what to keep / retire / add | Anyone touching routing or nav |
| 03 | `03_brand_identity.md` | The Teleox name, positioning statement, brand voice, what we are / are not | Anyone touching copy |
| 04 | `04_visual_design_system.md` | Color tokens, typography, spacing, motion — inherits Ember + adjustments | Anyone touching styles |
| 05 | `05_component_library.md` | Existing reusable components + new ones to add | Anyone building UI |
| 06 | `06_page_homepage.md` | Full new homepage spec | Implementing the new homepage |
| 07 | `07_page_research.md` | New `/research` page | Implementing `/research` |
| 08 | `08_page_paper.md` | Repositioned `/paper` page | Implementing `/paper` |
| 09 | `09_page_about.md` | Repositioned `/about` page | Implementing `/about` |
| 10 | `10_page_contact.md` | Repositioned `/contact` page | Implementing `/contact` |
| 11 | `11_page_404_error.md` | 404, error.tsx, not-found.tsx | Implementing error states |
| 12 | `12_navigation_and_footer.md` | Header nav, footer, breadcrumbs | Implementing chrome |
| 13 | `13_content_schema.md` | TypeScript types for `RouteCopy` and content surfaces | Adding new content types |
| 14 | `14_metadata_seo_sitemap.md` | Meta tags, OG, JSON-LD, robots, sitemap | SEO + metadata work |
| 15 | `15_assets_inventory.md` | Every image, icon, favicon, OG image needed | Asset production |
| 16 | `16_responsive_and_accessibility.md` | Breakpoints, motion, a11y rules | Anyone styling responsively |
| 17 | `17_validation_and_scanners.md` | Content gates, `mustNotSay`, scan-* rules | Adding content that passes gates |
| 18 | `18_implementation_plan.md` | File-by-file work list, in order | Executing the upgrade |
| 19 | `19_deployment_rollout.md` | Vercel deploy, preview verification, production rollout, rollback | Shipping it |
| 20 | `20_post_launch_iteration.md` | What to measure, when to revisit | Post-launch |

---

## Strategic decisions encoded in this spec

The following decisions are **load-bearing** for everything below. Override them only by editing this README and the affected docs.

| Decision | Choice | Rationale | Doc that depends on this |
|---|---|---|---|
| **Brand name** | Stays `Teleox.ai` (Chris Royse as named principal) | Existing equity + domain authority + Vercel project linked | 03 |
| **Primary audience** | Frontier-lab research leadership; Anthropic-leaning (invisible) | Pre-revenue site, no commercial conversion to preserve, target maximum-leverage attention | 01 |
| **Hero positioning** | "Models can be opaque. Pipelines shouldn't be." | Frames pipeline-verifiability as separable from model-interpretability — Anthropic-aligned, universally legible | 06 |
| **Visual identity** | Inherit existing Ember palette + typography | Already shipped, distinctive, works for research positioning, no time/budget to replace | 04 |
| **Dark mode default** | Yes (existing) | Confirmed in `memory/decisions/agent-04-design-system-lead--dark-mode-default.md` | 04 |
| **Type system** | Source Serif 4 + Inter + JetBrains Mono (Google Fonts, already wired) | Already in `layout.tsx`; matches research-grade tone | 04 |
| **Tailwind version** | v4 with `@theme` tokens (already in use) | No change | 04 |
| **CTA simplification** | Replace all "Send X data problem" CTAs with research-orbit equivalents | "Send data problem" is B2B sales funnel; doesn't fit new positioning | 06–10 |
| **Pages to RETIRE** | `/pricing`, `/infrastructure`, `/sign`, `/standard`, `/verify`, `/governance`, `/mark`, `/centroids`, `/citations`, `/feed`, `/artifacts`, `/measurements`, `/methodology`, `/priority-date`, `/measured`, `/method`, `/alignment-index` | All read as B2B-SaaS / regulatory / patent-defense framing inappropriate for research positioning | 02 |
| **Pages to KEEP + REPOSITION** | `/`, `/research` (new), `/paper`, `/about`, `/contact`, `/blog` | Core surface for the new positioning | 02, 06–10 |
| **Pages to RETAIN AS-IS** | `/api/*`, JSON-LD, robots, sitemap (regenerated to drop retired routes) | Infrastructure | — |
| **Validation gate behavior** | Keep all scanners; update `mustNotSay` lists; whitelist new vocabulary | The gates are good infrastructure, just need new content rules | 17 |
| **License of public artifacts** | Site copy CC BY 4.0; companion code (clipcannon) BSL 1.1; ME-JEPA paper PolyForm-NC | Status quo; documented for the implementer | 03 |
| **Companion artifact** | `github.com/ChrisRoyse/clipcannon` is the public engineering proof | Already public, real code, real engineering | 07 |
| **Private artifact** | ME-JEPA paper is a private preview shared via `teleox.ai/{slug}/` URLs | Preserves privileged-access framing | 07, 08 |

---

## Out of scope for this upgrade

- Re-architecting Next.js / changing framework
- Redesigning the Ember visual identity (palette, type scale, motion)
- Replacing the Tailwind v4 build chain
- Translating to other languages
- Building a CMS for non-technical editing (content stays in TypeScript)
- Changing analytics provider (stays on Vercel Analytics + Speed Insights)
- Modifying any `/api/*` route
- Touching `leapable.ai` (separate property, separate roadmap)
- Building the actual ME-JEPA paper or clipcannon — those exist already

---

## What "done" looks like

The upgrade is done when, in production:

1. A frontier-lab researcher landing on `https://teleox.ai/` sees a headline that signals serious research within 2 seconds.
2. The first three navigation choices visible are research-oriented (research / paper / about), not commercial (no "pricing" / "send data problem" in primary nav).
3. Clicking through to `/research` surfaces the ME-JEPA paper, the clipcannon companion, and a clear path to email Chris.
4. Every retired route returns either a clean 404 or a 308 redirect to the closest live route per the table in `02_information_architecture.md`.
5. All `mustNotSay` violations are zero across the site (validated by `scan-*` scripts).
6. Sitemap, robots, JSON-LD reflect the new IA.
7. Lighthouse scores remain ≥ existing baseline (perf 90+, a11y 95+, best-practices 95+, SEO 95+).
8. Vercel preview build passes; production deploy succeeds; analytics record visits without errors.

---

## Non-goals (things this spec deliberately leaves to the implementer's judgment)

- Exact pixel values where Ember tokens already exist — use the tokens, do not invent new spacing scales.
- Specific copy on the `/blog` index — leave blog as-is; it's not in the conversion path.
- Whether to ship in one PR or split — pick whatever is most reviewable, but the deploy must be atomic from the user's POV (no half-state visible to visitors).

---

## How to use these docs

1. Read `01_strategic_positioning.md` first. Internalize the audience.
2. Read `02_information_architecture.md` second. Understand the sitemap delta.
3. Read `18_implementation_plan.md` third. That's your work order.
4. Reference docs 03–17 as you implement each piece.
5. Use `19_deployment_rollout.md` for the actual deploy chain.
6. Use `20_post_launch_iteration.md` after launch.

If you encounter ambiguity, default to: **what would Yann LeCun, Chris Olah, or Saining Xie think if they landed on this page right now?** That is the audience test.
