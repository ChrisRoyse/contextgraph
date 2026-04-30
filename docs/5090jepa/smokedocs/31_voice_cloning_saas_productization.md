# 31 - Voice Cloning SaaS Productization

## Goal

Productize Case 3 from PAPER.md (**0.961 mean WavLM SECS, max 0.975, 9
of 10 sentences ≥ 0.95**) into a paid voice-cloning service whose
single defensible differentiator is **per-output Constellation Guard at
runtime**: every generated clip is geometrically validated against the
frozen identity centroid set before it streams.

This is the **first revenue product**. It pays the bills while the
infrastructure plays (docs 32 / 33 / 34 / 35) mature on longer cycles.

## Why Voice First

- Case 3 is the only **fully measured** case in PAPER.md (Cases 1 and
  2 are projected/partial).
- The base technology already beats published TTS frontier paradigm
  anchors (VALL-E 2 ~0.881, NaturalSpeech 3 ~0.86–0.89 band) on
  encoder-matched WavLM SECS.
- Constellation Guard is a category-creating feature; no competitor
  ships per-output runtime cosine rejection against a frozen centroid.
- Voice cloning has a real, growing market (ElevenLabs reported
  $11M+ ARR pre-2025; Cartesia, Resemble, Vapi all funded), so buyer
  awareness is a tailwind, not a barrier.

## Product Surface

### Three Tiers

| Tier | Audience | Pricing | Capabilities |
| --- | --- | --- | --- |
| **Creator** | Solo creators, podcasters, YouTubers | $19/mo + $0.10/min over 60 min | Web UI, web upload, 50-clip centroid build, Constellation Guard ON, watermark optional |
| **Developer** | App / agent builders | $0.05/min API + $99/mo platform fee | REST + streaming API, programmatic centroid build, license server, no watermark |
| **Enterprise** | Studios, contact centers, internal comms | $25K–$200K/yr ELA | Self-hosted Apptainer image, per-employee centroids (doc 34 overlap), audit dashboard, SLA |

The Constellation Guard is **always on** in all tiers. Customers can
adjust the per-modality threshold; they cannot remove the guard. This
is both an engineering simplification and a brand position
("**verified or rejected, never drifted**").

### Streaming Architecture

```text
client request
   │
   ▼
[ Auth / billing gate ]    license server (HMAC, already exists in ClipCannon)
   │
   ▼
[ Centroid lookup ]        per-tenant frozen centroid set in RocksDB
   │
   ▼
[ Generation worker ]      Qwen3-TTS + LoRA (or successor)
   │  ── chunk emitted
   ▼
[ Constellation Guard ]    encode chunk, cosine vs centroid, accept/reject
   │
   ▼
[ Stream relay ]           accepted chunks streamed to client
                            rejected chunks → typed error event
```

Latency budget: <150 ms first-chunk latency over websocket; <50 ms per
guard pass; total streaming overhead <8% relative to non-guarded TTS.

### Identity Provenance

Every emitted clip carries a per-output `provenance.json` (in the
response or downloadable separately):

```json
{
  "clip_id": "...",
  "centroid_set_hash": "...",
  "generation_artifact_hash": "...",
  "guard_decision": {
    "modality_cosines": [...],
    "modality_thresholds": [...],
    "final_verdict": true
  },
  "model_version": "qwen3-tts-v1+lora-v3",
  "sample_rate": 24000,
  "duration_ms": 12500
}
```

This is the same FSV-style bundle pattern as smokedoc 03 / doc 29,
miniaturized for per-output use. Customers can serialize provenance
into their own audit trails, which is required for any downstream EU
AI Act-compliant deployment that includes the customer.

## Critical Build Items (Pre-Launch)

| Item | Rationale | Effort |
| --- | --- | --- |
| Cross-encoder SECS verification (doc 38) | Paper claim moves from encoder-matched to cross-verified; product page can cite both | 4–6 weeks |
| Multi-speaker reference data | Single-speaker English is too narrow for a product; ≥10 speakers across 3 languages | 6–8 weeks |
| Constellation Guard for streaming | Existing guard is per-clip; needs per-chunk extension with hysteresis | 3–4 weeks |
| License server multi-tenant | ClipCannon's HMAC license server is single-tenant; needs tenant isolation | 4–6 weeks |
| Centroid build UX | 50-clip centroid construction needs a user-friendly upload + auto-validation flow | 3–4 weeks |
| Web UI | Next.js or SvelteKit; minimal: upload, build, generate, listen, download, provenance link | 4–6 weeks |
| Billing | Stripe + metering on minutes-generated; existing ClipCannon stub is one-shot | 3–4 weeks |
| Trust + Safety | Voice clone consent flow; deepfake disclosure; refusal on identity-disputed uploads | 3–4 weeks |

Total pre-launch: ~18–24 weeks single-author; ~10–12 weeks with one
junior engineer on UI + billing.

## Trust + Safety (Non-Negotiable)

Voice cloning is a dual-use technology. The product must:

- Require **explicit consent attestation** for any uploaded reference
  audio, with a checked box and recorded IP/timestamp.
- Refuse cloning of **public figures** by name (matchlist + LLM
  judge); typed refusal, not silent.
- Offer **inaudible watermark** (optional Creator, default Developer)
  via Audio Watermarking Tools (AWT) or similar.
- Provide a **revocation channel**: subjects can submit a takedown
  with proof, model centroid is removed within 48h, all derived clips
  flagged, paying customer notified.
- Log every centroid build + every generation in a tamper-evident chain
  (already an FSV pattern).

The trust+safety policy is the difference between a defensible product
and a litigation magnet. Spend the engineering budget here.

## Competitive Position

| Competitor | Strength | Weakness vs Us |
| --- | --- | --- |
| ElevenLabs | UX, brand, voice library | No runtime guard; no per-output provenance |
| Cartesia | Real-time latency | Encoder-matched eval only; no centroid-aware identity locking |
| Resemble.ai | Enterprise sales | No public technical contribution; closed encoder eval |
| OpenAI TTS | Distribution, pricing | No identity locking; not designed for cloning per se |
| HeyGen / Synthesia (video-side) | Video distribution | Voice is a side feature; no SECS-level voice guarantees |

**Our wedge**: published 0.961 SECS + cross-encoder verified +
per-output guard + FSV provenance. None of the above ship those four
together.

## Pricing Discipline

- Resist freemium that includes API credits. Free tier only on UI demo
  with a fixed-voice (non-cloned) preview.
- Bill on minutes-generated, not characters or tokens. Consistent with
  customer's perceived value.
- Enterprise floor at $25K/yr to qualify customer (saves you sales
  cycles on tire-kickers).

## Launch Plan (4–6 Months)

| Week | Milestone |
| --- | --- |
| 1–4 | Cross-encoder triangulation (doc 38) live; paper revision drafted |
| 5–10 | Multi-speaker / multi-language eval; license server multi-tenant |
| 11–14 | Web UI + Stripe + centroid UX + Constellation Guard streaming |
| 15–18 | Closed beta, 20 invited users, 1-week feedback cycles |
| 19–22 | Public beta, pricing live, support docs, status page |
| 23–26 | Enterprise sales motion (cold + inbound) |

Success metrics (12-month post-launch):

- $1M+ ARR;
- 10+ enterprise logos at $25K+/yr;
- 1000+ Creator-tier subscribers;
- 0 verifiable identity-misuse incidents.

## Coordination With Other Docs

- Doc 28 (Constellation Guard for ME-JEPA): the same crate powers
  the voice product's runtime guard. **Build the shared crate first.**
- Doc 29 (unified bundle schema): per-output `provenance.json` is the
  miniature of the bundle.
- Doc 32 (DDA-aaS): centroid construction is a special case of DDA
  enrichment over a small reference set.
- Doc 38 (cross-encoder triangulation): hard dependency for credible
  product claims.
- Doc 36 (three-paper roadmap): paper revision with cross-encoder
  numbers is the trust-building artifact for enterprise sales.

## Acceptance

Voice product launch is **complete** when:

- public web app is live with Stripe billing;
- cross-encoder verification numbers are on the product page (PR-grade);
- a paying customer has built a centroid, generated audio, downloaded
  provenance, and successfully replayed verification end-to-end;
- trust+safety flow has refused at least one disallowed request in
  staging;
- doc 19 (paper drafting) cites a real production usage stat in the
  Discussion section.
