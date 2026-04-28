---
namespace: patterns
agent_id: agent-00-orchestrator
timestamp: 2026-04-19T15:15:00Z
status: active
tags: verification, dates, institutional-contacts, pre-send, playwright
---

# PATTERN — Date + Institutional-Contact Verification Before Send

## The rule

**Any outreach artefact that cites a conference date, application window, recipient email, or
institutional affiliation MUST be re-verified against the institution's public page within 72
hours of send.** Research-agent outputs are a starting point, not authority.

## Why this pattern exists

On Day-0 (2026-04-19) of the Teleox engagement, six gating items were re-verified via
Playwright. Three were wrong in the original research-agent outputs (Dohmatob email, ECIR 2026
workshop submission ask, FII Miami / LEAP Riyadh dates). All three would have produced visible
misses in live outreach:

- Wrong email → bounce or stale-mailbox no-reply
- Missed workshop deadline → "hasn't done homework on the community" signal to the community's
  keynote speaker
- Outdated conference dates → asking the recipient to book an event that already happened

See `memory/journal/agent-00-orchestrator--playwright-verification-day0.md` for full detail.

## What MUST be verified (in order)

1. **Recipient email address** against the institution's current faculty / staff / team page.
   Faculty pages use `mailto:` links with current verified addresses. LinkedIn-inferred emails
   are unreliable.
2. **Recipient current title + institution**. People move quarterly in AI; a 6-month-old
   research-agent snapshot is stale. Check Wikipedia, institutional page, ORCID, Google Scholar
   affiliation — whichever is most recent.
3. **Recipient's stated filters / preferences** — many faculty pages post explicit "don't email
   me about X" notices (Dohmatob's "no supervision requests" is an example). The outreach MUST
   explicitly acknowledge those filters to preserve credibility.
4. **Conference / workshop dates** — official event site, current year. Conferences move
   quarters; "annual in March" assumptions fail about 20% of the time.
5. **Submission deadlines** — official CFP page, not aggregator sites. CFPs often update
   deadlines without mirroring to aggregators.
6. **Recipient's recent publications (last 12 months)** — use as fresh conversation hooks.
   Referencing a paper from 2024 when they shipped a SIGIR 2025 Best Paper is a missed
   opportunity and a signal of inattention.

## What to do when verification contradicts the package

1. **Fix the draft first**, then the package source docs. Live outreach > retroactive
   consistency.
2. **Log the finding to** `memory/journal/` with URL + timestamp + what was wrong + what was
   corrected — preserves audit trail.
3. **If the correction changes the ask** (e.g., a missed deadline forces a different venue
   offer), revise the ask cleanly — don't paper over the miss.
4. **Never send the email with "I notice the workshop has already happened"-type
   acknowledgements** — that telegraphs lack of preparation. Just offer the correct forward-
   looking ask and skip the acknowledgement of what we almost sent.

## Playwright MCP is the verification substrate

- Always use `browser_navigate` + `browser_snapshot` for institutional pages.
- Always use `browser_evaluate` for fact extraction (filter body text by keyword).
- Save snapshots under `.playwright-mcp/` — these are the source-of-truth artefacts the journal
  entry points to.
- Google SERP scraping is acceptable for event-date disambiguation; Cloudflare Challenges on
  some event sites make direct access intermittent.

## When this pattern does NOT apply

- Pure technical claims from the paper itself (paper §8 scope guards, case-study metrics, paper
  sections) — these are internal to Chris's work and don't need external verification.
- Commercial TAM estimates — these are flagged to source to analyst reports, separate workflow.
- LinkedIn / X handles — these change less frequently than institutional emails, lower priority
  for verification.

## Resolution

This pattern stays OPEN for the lifetime of the engagement pipeline. Every outreach draft must
pass date-verification within 72 hours of send, regardless of what the research agents said at
wave-1 time.
