# 33 - EU AI Act Compliance Product

## Goal

Productize the FSV protocol + bundle schema + Constellation Guard as an
**enterprise compliance platform** that maps directly to EU AI Act
articles for high-risk AI systems. Customers deploy the platform
alongside their existing ML stack and emit regulator-ready
documentation, audit logs, and per-output provenance — by construction,
not as a retrofit.

## Why This Wins

EU AI Act is in force; high-risk system obligations enforce 2026–2027.
Every other vendor is bolting compliance on top of opaque pipelines
(MLflow + custom dashboards + manual data-card production). Our stack
**starts** with provenance and adds inference; we don't have to add
provenance later because it is the architectural premise.

The product positioning: "**compliance-by-construction, not
compliance-as-overlay**."

## Article-By-Article Mapping

The platform's marketing explicitly cites the regulation. Each row
below ties an EU AI Act obligation to a smokedocs primitive that
already exists or is on the build path.

| Article | Obligation | Our Primitive |
| --- | --- | --- |
| Art. 9 — Risk management | Continuous risk identification and mitigation | Doc 21 risk register; doc 23 CI drift detection; FSV fail-closed defaults |
| Art. 10 — Data governance | Data quality, relevance, representativeness, bias controls | DDA bundle (doc 32); `data_manifest.json` license + hash; embedder panel manifest |
| Art. 11 — Technical documentation | Detailed technical files | Smokedocs 00-41; bundle JSON; arXiv preprint cross-cite |
| Art. 12 — Logging | Automatic event recording | `command_log.jsonl`; `dj_audit_log` CF; tamper-evident chain |
| Art. 13 — Transparency | User-facing instructions, intended use, performance | Bundle `evaluation_report.json`; doc 21 intended-use statement |
| Art. 14 — Human oversight | Mechanisms for monitoring + intervention | Constellation Guard rejection (doc 28); Phase 12 deletion propagation (doc 25) |
| Art. 15 — Accuracy + cybersecurity | Performance levels, robustness, resilience | Multi-seed BCa CI (doc 15); collapse diagnostics (doc 22); Constellation Guard per-output verifiability |
| Art. 16-29 — Provider obligations | Quality mgmt, post-market monitoring | Doc 23 CI; doc 20 Zenodo + ACM packet |
| Art. 50 — Marking + transparency | Labeling AI-generated content | Voice clone watermarking (doc 31); per-output provenance link |
| Art. 60 — Real-world testing | Pre-deployment testing in real conditions | FSV gates (smokedoc 04) + smoke regression (doc 23) |

A compliance officer reads this table and sees that 80% of their work
is already done by adopting our stack.

## Product Surface

### Compliance Dashboard

A web UI that rolls up:

- per-deployed-system bundle inventory;
- FSV pass/fail timeline;
- artifact hash recompute status;
- Constellation Guard rejection rate by deployment;
- regulator-facing PDF report generator (per-Article evidence with
  bundle paths cited);
- alerts on drift (doc 23) or hash mismatch.

The PDF generator is the primary value to a risk officer: a one-click,
defensible, evidence-cited document they can hand to a regulator.

### Continuous Audit Log

Append-only log of every model-altering operation, with cryptographic
chaining (already a Context Graph primitive). Per-event fields:

```json
{
  "event_id": "...",
  "operator": "...",
  "operation": "model.deployed | data.ingested | guard.rejected | ...",
  "system_id": "...",
  "model_artifact_hash": "...",
  "bundle_path": "...",
  "prev_event_hash": "...",
  "this_event_hash": "...",
  "regulator_visible": true,
  "article_relevance": ["Art. 12", "Art. 14"]
}
```

Each event hashes to the next; the chain root is published to a
public timestamping service (e.g. OpenTimestamps). Tampering is
detectable from outside.

### Conformity Assessment Helper

For high-risk systems requiring third-party conformity assessment, the
platform exports a **conformity assessment package**:

```text
conformity_<deployment>_<date>/
  README_for_assessor.pdf             # one-page guide
  technical_documentation.pdf          # Art. 11 file
  risk_management_summary.pdf          # Art. 9
  data_governance_evidence/            # Art. 10
  logging_chain.parquet                # Art. 12
  transparency_documentation.pdf       # Art. 13
  human_oversight_evidence/            # Art. 14
  accuracy_robustness_report.pdf       # Art. 15
  bundles/                             # all referenced FSV bundles
  cryptographic_proofs/                # chain anchors, container digests
```

The package is generated from existing bundles, not authored by hand.

## Pricing

| Tier | Pricing | Features |
| --- | --- | --- |
| **Standard** | $250K/yr ELA | Up to 10 systems, dashboard, audit log, 4 conformity packages/yr |
| **Plus** | $750K/yr ELA | Up to 50 systems, custom domain packs, dedicated CSM, SOC 2 |
| **Sovereign** | $2M+/yr | On-prem, air-gapped, 24/7 support, regulator-facing co-presentation |

12–18 month sales cycles. Pipeline math: 30 enterprise conversations →
8 paid POCs → 3–5 closed at $250K–$750K → ~$1.5M–$3M ARR by month 18
on this product alone.

## Buyer Personas

- **Chief Risk Officer (financial services)**: bank, insurer, asset
  manager. Top concern: SR 11-7 + EU AI Act overlap. Our pitch: a
  single platform satisfies both.
- **Head of AI Governance (large enterprise)**: typically reports to
  CIO or Chief Privacy. Top concern: post-deployment monitoring + audit
  scalability. Our pitch: continuous chain + per-output Guard
  rejections are observability they don't currently have.
- **CISO (regulated enterprise)**: cybersecurity overlap (Art. 15). Our
  pitch: the chain + container provenance are CISO-grade evidence.
- **CTO of an AI startup selling into the EU**: needs minimum-viable
  compliance to pass procurement. Our pitch: Standard tier as the
  cheapest path to an "AI Act-compliant" claim with substance behind
  it.

## Hard Engineering Items (Pre-Launch)

- Multi-deployment dashboard (per-system bundle inventory + FSV
  rollups);
- Audit chain → public timestamping integration;
- Conformity package generator (PDF templating + bundle aggregation);
- Customer-facing per-deployment domain packs (doc 37 dependency);
- SOC 2 Type II + ISO 27001 (parallel track; ~12 months);
- Regulator-engagement playbook (in advance of customer asks).

## Sales Risks

- **Slow government interpretation**: EU AI Act articles are real, but
  national implementing acts and conformity assessment bodies are
  still finalizing. Counterweight: position as **forward-compatible
  best practice**, not just regulatory minimum.
- **Compliance buyers are budget-constrained mid-cycle**: most large
  enterprises plan annual security/compliance spend in Q4–Q1.
  Counterweight: align launch and big-customer pushes with that cycle.
- **Customer wants partial compliance**: tempting to offer just the
  audit log without the rest. Refuse: the differentiation is the
  whole stack. Sell it whole or not at all.
- **Open-source compliance overlays improve**: MLflow, OpenLineage,
  Marquez are getting better. Counterweight: their data model is
  observability; ours is **verifiable replay from raw corpus to
  output**. Different category.

## Coordination With Other Docs

- Doc 21 (compliance + ethics): the NIST AI RMF mapping is reused as
  the US-side counterpart story.
- Doc 23 (CI + drift): drift alerts feed the audit chain.
- Doc 25 (Phase 12 privacy gate): activates for customers with
  subject-level data; sold as Sovereign-tier add-on.
- Doc 32 (DDA-aaS): satisfies Article 10 data-governance evidence as a
  bundle.
- Doc 28 (Constellation Guard): satisfies Article 14 (human oversight
  → automated rejection) and Article 15 (accuracy floor).
- Doc 34 (identity-locked agents): an upsell on top of compliance for
  internal AI deployments.

## Acceptance

EU AI Act compliance product is **launch-ready** when:

- the dashboard renders 5+ deployed systems with FSV rollups;
- a generated conformity package has been reviewed by an external EU
  AI Act-qualified counsel and rated at minimum "substantively
  responsive" to Article 11–15 obligations;
- audit chain is anchored to OpenTimestamps with public proof of
  tamper-evidence;
- one paying Standard-tier customer has issued an internal compliance
  audit using the platform output;
- pricing book and sales playbook (doc 40) are live.
