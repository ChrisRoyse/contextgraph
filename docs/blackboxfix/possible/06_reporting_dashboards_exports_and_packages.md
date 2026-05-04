# Reporting, Dashboards, Exports, And Packages

BlackBoxFix can turn verified ledger state into durable files for operators,
buyers, eval pipelines, incident response, and observability tools.

## Evidence Export

Command:

```bash
blackbox export evidence --ledger PATH --run-id RUN_ID --out FILE
```

Capabilities:

- export one run's evidence to a JSON report;
- include run timeline, verifier results, evidence packets, and incidents;
- read back the written file;
- include hashes/source references.

## Episode And Eval Export

Command:

```bash
blackbox export episodes --ledger PATH --out DIR [--force]
```

Output:

- `episodes.jsonl`
- `manifest.json`
- `evals/*.json`

Capabilities:

- convert verified agent activity into training/eval data;
- include verifier status;
- reparse JSONL after write;
- verify manifest hashes against physical file bytes.

## Dashboard

Commands:

```bash
blackbox dashboard build --ledger PATH --out DIR [--force]
blackbox dashboard api --ledger PATH --view VIEW [--run-id RUN_ID]
```

Output:

- `index.html`
- `manifest.json`
- `api/summary.json`
- `api/runs.json`
- `api/evidence.json`
- `api/verifier-status.json`
- `api/errors.json`
- `api/exports.json`
- `api/pilot-packages.json`
- `api/risk-assessments.json`
- per-run timeline JSON.

Capabilities:

- inspect run counts;
- inspect verifier status;
- inspect evidence and incidents;
- inspect exports;
- inspect pilot packages;
- inspect risk assessments;
- browse timelines per run.

## Enterprise Audit Export

Command:

```bash
blackbox enterprise export-audit --ledger PATH --tenant TENANT --principal-id PRINCIPAL --out DIR [--force] --agent AGENT
```

Output:

- `audit.jsonl`
- `siem-rfc5424.log`
- `controls.json`
- `manifest.json`

Capabilities:

- export audit/SIEM evidence for one tenant;
- enforce RBAC before export;
- manifest-hash every output file;
- preserve denied export attempts in the ledger.

## Pilot Package

Command:

```bash
blackbox package pilot-report --ledger PATH --tenant TENANT --principal-id PRINCIPAL --pilot-id PILOT --out DIR
```

Output:

- `pilot-report.json`
- `usage-meter-events.jsonl`
- `eval-candidates.jsonl`
- `manifest.json`

Capabilities:

- package buyer-facing proof;
- meter usage;
- generate eval candidates;
- verify minimum action thresholds;
- require access authorization;
- write `pilot_package` rows;
- expose package summaries through dashboard API files.

## First-Wedge Demo Package

Command:

```bash
blackbox demo first-wedge --ledger PATH --tenant TENANT --run-id RUN_ID --out DIR
```

Output:

- `demo-report.json`
- `evidence-index.json`
- `demo-script.jsonl`
- `manifest.json`

Capabilities:

- package a vulnerability-remediation buyer demo;
- require proof from Git, scanner, GitHub, and GitHub Actions evidence;
- include required failure demos;
- verify package manifest hashes;
- persist `first_wedge_demo` rows.

## Full-State Verification Package

Command:

```bash
blackbox fsv certify --ledger PATH --tenant TENANT --phase N --evidence EVIDENCE_JSON --out DIR
```

Output:

- `fsv-report.json`
- `provenance.json`
- `edge-cases.jsonl`
- `manifest.json`

Capabilities:

- validate completed FSV evidence;
- inspect physical SQLite state;
- run SQLite `PRAGMA integrity_check`;
- verify declared artifact hashes;
- persist `full_state_verification` rows;
- package FSV proof for review.

## OpenTelemetry Export

Command:

```bash
blackbox export otel --ledger PATH --out DIR [--force]
```

Output:

- `traces.jsonl`
- `logs.jsonl`
- `metrics.jsonl`
- `manifest.json`

Capabilities:

- export one span per physical ledger row;
- export logs for incidents, failed/blocked verifiers, denied policies,
  non-ok HTTP audits, and export completion;
- export low-cardinality metrics for record kinds, verifier status, policy
  decisions, and pass ratio;
- verify files and manifest after promotion;
- avoid mutating the ledger during export.

## Manifest Discipline

Generated file packages generally include:

- format id;
- schema/version;
- source ledger count;
- ledger tip hash;
- file paths;
- byte counts;
- SHA-256 hashes;
- item counts.

This makes exported artifacts independently inspectable.
