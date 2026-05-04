# Workflows Now Possible

This file lists concrete workflows that BlackBoxFix can now execute or support
end to end.

## Verified Local Code Change

BlackBoxFix can:

1. record a run;
2. record a proposed Git patch;
3. apply the patch through the MCP gateway;
4. read the real Git diff;
5. verify the diff matches expected patch;
6. persist verifier/evidence rows;
7. create an incident if it does not match;
8. export evidence or include it in dashboard/package outputs.

## Command-Based Test Proof

BlackBoxFix can:

1. record a tool call;
2. declare a command and artifacts;
3. run the command;
4. capture stdout/stderr/exit code;
5. verify exit zero;
6. inspect declared artifact files;
7. persist evidence and incidents.

## Hosted CI Proof

BlackBoxFix can:

1. read GitHub Actions or GitLab CI workflow/pipeline/job state;
2. verify completed/successful CI state;
3. persist the expected delta, readback, verifier result, and evidence;
4. surface failures in dashboard and incidents.

## Vulnerability/Scanner Proof

BlackBoxFix can:

1. read OSV reports;
2. read lockfiles;
3. read CycloneDX SBOMs;
4. verify vulnerability absence, version pins, or package presence;
5. preserve physical artifact hashes;
6. attach scanner proof to buyer demo/pilot packages.

## Ticket Evidence Proof

BlackBoxFix can:

1. read GitHub issue body/comments or captured Jira/Linear/ServiceNow
   artifacts;
2. verify ticket identity and state;
3. verify evidence links when required;
4. persist proof or incidents.

## Kubernetes Deployment Verification

BlackBoxFix can:

1. ingest a physical Kubernetes Deployment JSON snapshot;
2. verify image digest;
3. verify replica count;
4. verify rollout completion;
5. persist verifier/evidence/incident rows.

## Terraform/Cloud Verification

BlackBoxFix can:

1. read Terraform plan JSON;
2. verify only approved changes are present;
3. read Terraform state JSON;
4. verify state attributes;
5. read cloud inventory JSON;
6. verify resource presence;
7. read cloud audit JSONL;
8. verify principal/action audit evidence.

## Enterprise Governance Workflow

BlackBoxFix can:

1. configure tenant SSO/RBAC/retention/key metadata;
2. sync directory/role state;
3. evaluate access decisions;
4. record audit events;
5. export SIEM-ready audit files;
6. use RBAC decisions in privacy, release, package, and risk workflows.

## Privacy Workflow

BlackBoxFix can:

1. evaluate retention for a run/data subject;
2. approve or deny erasure requests;
3. enforce retention class existence;
4. enforce legal holds;
5. enforce authorization;
6. persist append-only privacy evidence.

## Release Gate Workflow

BlackBoxFix can:

1. require a scorecard;
2. inspect incidents and failed verification;
3. enforce trust/risk thresholds;
4. require policy and privacy evidence;
5. require release authorization;
6. persist an approved or blocked release gate decision.

## Pilot Packaging Workflow

BlackBoxFix can:

1. inspect verified tenant evidence;
2. enforce minimum action thresholds;
3. package buyer proof;
4. generate usage-meter JSONL;
5. generate eval candidates;
6. write manifest-hashed package files;
7. persist `pilot_package` rows.

## Risk Closure Workflow

BlackBoxFix can:

1. inspect ledger evidence and runtime SQLite state;
2. inspect pilot package/readiness state;
3. require specific decision-register entries;
4. block incomplete risk posture;
5. persist accepted or blocked risk assessments;
6. expose risk state through dashboard/API output.

## First-Wedge Demo Workflow

BlackBoxFix can:

1. assemble proof for a vulnerability-remediation story;
2. require Git/scanner/GitHub/GHA evidence;
3. require failure demos;
4. generate a demo report, evidence index, script, and manifest;
5. persist `first_wedge_demo` rows.

## FSV Certification Workflow

BlackBoxFix can:

1. accept a completed FSV evidence file;
2. inspect physical SQLite state;
3. run SQLite integrity checks;
4. verify artifact hashes;
5. verify edge-case categories;
6. persist `full_state_verification` rows;
7. write FSV package files.

## SDK Runtime Workflow

BlackBoxFix can:

1. accept CloudEvents SDK event files;
2. validate source/id/type/spec/data;
3. validate trace context;
4. enforce mutating expected deltas;
5. prevent duplicate ingestion;
6. atomically append batch rows;
7. read back inserted rows from SQLite.

## Observability Export Workflow

BlackBoxFix can:

1. read a verified ledger;
2. export traces/logs/metrics JSONL;
3. write an OpenTelemetry manifest;
4. verify exported file hashes;
5. run connector readback over the export directory.
