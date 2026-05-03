# Phase 10 State And Invariants - Scanner Artifact Connector

Phase 10 ships `scanner_artifact_v1`, the local artifact connector for
vulnerability-remediation proof. It verifies scanner and dependency artifacts
from physical files and persists proof in the existing SQLite ledger evidence
path. There was no schema bump in Phase 10; `LEDGER_SCHEMA_VERSION` remained
`5` then. The current live schema after Phase 21 is `13`.

## Source Of Truth

Primary sources of truth:

- OSV JSON report files for vulnerability findings;
- Cargo lockfiles for resolved Rust package versions;
- CycloneDX JSON SBOM files for generated software bill of materials state;
- local SQLite ledger table `ledger_records` for expected deltas, readback
  snapshots, verifier results, evidence packets, and incident packets.

Manual/automated FSV must read artifact files independently from the CLI
result, then read the physical SQLite table directly. CLI stdout/stderr alone
is not proof.

## Shipped CLI Surface

```bash
context-graph blackbox connector test \
  --connector scanner_artifact_v1 \
  --source /path/to/osv-report.json

context-graph blackbox verify \
  --ledger /tmp/bbx.sqlite \
  --input spec.json \
  --tenant TENANT \
  --agent AGENT
```

`scanner_artifact_v1` does not accept `--command`. It is a file readback
connector, not a scanner process runner.

## Verifier Specs

`scanner_vulnerability_absent_v1` requires:

```json
{
  "source": {
    "report": "/tmp/osv-clean.json",
    "report_format": "osv-json"
  },
  "expected": {
    "vulnerability_id": "GHSA-m5pq-gvj9-9vr8",
    "package": "regex",
    "ecosystem": "crates.io",
    "version": "1.10.3"
  }
}
```

Only `vulnerability_id` is required under `expected`; package, ecosystem, and
version are optional filters. It passes only when the normalized OSV report
contains at least one package matching supplied filters and no matching
vulnerability id, alias, or grouped id.

`scanner_lockfile_pinned_to_version_v1` requires:

```json
{
  "source": {
    "lockfile": "/tmp/Cargo.lock",
    "lockfile_format": "cargo"
  },
  "expected": {
    "package": "regex",
    "version": "1.10.3"
  }
}
```

It parses the lockfile as TOML and passes only when the package exists and all
matching package entries have the expected version.

`scanner_sbom_artifact_present_v1` requires:

```json
{
  "source": {
    "sbom": "/tmp/bom.json",
    "sbom_format": "cyclonedx-json"
  },
  "expected": {
    "minimum_components": 1,
    "component_name": "regex",
    "component_version": "1.10.3"
  }
}
```

It passes only when the SBOM exists, is valid CycloneDX JSON, has at least the
minimum number of components, and contains the expected component/version when
those optional fields are supplied.

## Invariants

- Artifact paths must be present in the verifier spec. Missing source paths are
  blocked before a readback row is written.
- Artifact paths that exist but are not regular files are blocked before a
  readback row is written.
- Artifact format fields are closed sets: `osv-json`, `cargo`, and
  `cyclonedx-json`. Unsupported formats are blocked before source reads.
- Readback snapshots store path, existence, byte count, SHA-256, parse status,
  schema status, and normalized observed state when parsing succeeds.
- Missing artifact files are verifier failures, not successful empty states.
- Malformed JSON/TOML, invalid UTF-8, and schema drift are blocked with parse
  or schema detail in the readback when a physical file was available.
- A vulnerability id match checks OSV ids, aliases, and grouped ids.
- When package/ecosystem/version filters are supplied, the OSV report must
  contain at least one matching package. A clean report for the wrong artifact
  is failed, not accepted as proof.
- Lockfile matching is based on parsed package entries, never substring
  matching.
- SBOM verification requires `bomFormat="CycloneDX"`, a non-empty
  `specVersion`, and component names.
- `component_version` without `component_name` is blocked before source reads
  because version-only component proof is ambiguous.
- Outcome mismatches are failed and produce `incident_packet` rows. Invalid
  specs are blocked and do not produce incidents.

## Error Codes

- `SCANNER_ARTIFACT_PATH_MISSING`
- `SCANNER_ARTIFACT_READ_FAILED`
- `SCANNER_ARTIFACT_NOT_FILE`
- `SCANNER_ARTIFACT_JSON_INVALID`
- `SCANNER_ARTIFACT_FORMAT_UNSUPPORTED`
- `SCANNER_REPORT_NOT_FOUND`
- `SCANNER_REPORT_PACKAGE_NOT_FOUND`
- `SCANNER_REPORT_SCHEMA_DRIFT`
- `SCANNER_VULNERABILITY_STILL_PRESENT`
- `SCANNER_LOCKFILE_NOT_FOUND`
- `SCANNER_LOCKFILE_UTF8_INVALID`
- `SCANNER_LOCKFILE_TOML_INVALID`
- `SCANNER_LOCKFILE_SCHEMA_DRIFT`
- `SCANNER_LOCKFILE_PACKAGE_NOT_FOUND`
- `SCANNER_LOCKFILE_VERSION_MISMATCH`
- `SCANNER_SBOM_MINIMUM_COMPONENTS_INVALID`
- `SCANNER_SBOM_COMPONENT_VERSION_WITHOUT_NAME`
- `SCANNER_SBOM_NOT_FOUND`
- `SCANNER_SBOM_SCHEMA_DRIFT`
- `SCANNER_SBOM_COMPONENTS_MISSING`
- `SCANNER_SBOM_COMPONENT_NOT_FOUND`
- `SCANNER_SBOM_COMPONENT_VERSION_MISMATCH`

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase10_scanner_fsv -- --nocapture
```

Current result: 1/1 passing.

The test writes real artifact files, drives the real `context-graph blackbox`
binary, then opens the physical SQLite ledger with `rusqlite` and checks row
counts and payloads. Coverage:

- connector list includes `scanner_artifact_v1`;
- connector test reads an OSV JSON report;
- happy vulnerability-absence verifier passes;
- happy Cargo lockfile pinned-version verifier passes;
- happy CycloneDX SBOM verifier passes;
- vulnerability still present fails;
- missing report fails;
- report package-filter miss fails;
- directory artifact path blocks;
- invalid OSV JSON blocks;
- wrong lockfile version fails;
- missing lockfile package fails;
- unsupported lockfile format blocks;
- missing SBOM fails;
- invalid SBOM schema blocks;
- `minimum_components=0` blocks.
- `component_version` without `component_name` blocks.

## Manual FSV Evidence

Manual run:

- log: `/tmp/bbx-phase10-manual-fsv.Ww4wd6/fsv.log`;
- SQLite source of truth:
  `sqlite:/tmp/bbx-phase10-manual-fsv.Ww4wd6/scanner.sqlite#ledger_records`;
- artifact source directory: `/tmp/bbx-phase10-manual-fsv.Ww4wd6`.

Physical artifact hashes read directly from disk:

```json
{
  "osv-clean.json": "sha256:0cf3d92ab61968b557143878af6bebb17d66c1b0927f38e3a197a669b1850a0b",
  "osv-vulnerable.json": "sha256:0c15bcc3437d8e19545e80aeee3595f56db157236047c7e74107f13b647b387e",
  "Cargo.lock": "sha256:2f8d405ca4fe677428530293e9be3420df45d436fe501eaf429e79e18c92b20e",
  "bom.json": "sha256:52bada329ad56a72d75c80ecb7698a96f21ee0a6c071c5b745fdb960b2cad8a8",
  "invalid-bom.json": "sha256:1bd630b5eb70aa6cfde40814c84fe3fe48212403802d2e82d02f7d3538408da2"
}
```

Happy-path proof from physical SQLite:

- `scanner_vulnerability_absent_v1` verifier row sequence `5`, status
  `passed`, matched vulnerability count `0`;
- `scanner_lockfile_pinned_to_version_v1` verifier row sequence `9`, status
  `passed`, matching package count `1`, observed `regex 1.10.3`;
- `scanner_sbom_artifact_present_v1` verifier row sequence `13`, status
  `passed`, component count `1`, matching component count `1`.

Edge-case proof from physical SQLite:

| Case | Expected result | Verifier row |
|---|---|---|
| vulnerable OSV report includes `GHSA-m5pq-gvj9-9vr8` | `failed`, `SCANNER_VULNERABILITY_STILL_PRESENT` | sequence `17` |
| missing OSV report path | `failed`, `SCANNER_REPORT_NOT_FOUND` | sequence `22` |
| OSV report lacks requested package filters | `failed`, `SCANNER_REPORT_PACKAGE_NOT_FOUND` | hardened FSV |
| artifact path is a directory | `blocked`, `SCANNER_ARTIFACT_NOT_FILE` | hardened FSV |
| invalid OSV JSON | `blocked`, `SCANNER_ARTIFACT_JSON_INVALID` | sequence `27` |
| expected lockfile version `9.9.9` against observed `1.10.3` | `failed`, `SCANNER_LOCKFILE_VERSION_MISMATCH` | sequence `31` |
| missing SBOM path | `failed`, `SCANNER_SBOM_NOT_FOUND` | sequence `36` |
| invalid CycloneDX schema | `blocked`, `SCANNER_SBOM_SCHEMA_DRIFT` | sequence `41` |
| `minimum_components=0` | `blocked`, `SCANNER_SBOM_MINIMUM_COMPONENTS_INVALID` | sequence `44` |
| missing lockfile package | `failed`, `SCANNER_LOCKFILE_PACKAGE_NOT_FOUND` | sequence `48` |
| unsupported lockfile format | `blocked`, `SCANNER_ARTIFACT_FORMAT_UNSUPPORTED` | sequence `52` |
| `component_version` without `component_name` | `blocked`, `SCANNER_SBOM_COMPONENT_VERSION_WITHOUT_NAME` | hardened FSV |

Final physical SQLite counts from the original full manual FSV:

```json
{
  "agent_run": 1,
  "tool_call": 1,
  "expected_delta": 12,
  "readback_snapshot": 10,
  "verifier_result": 12,
  "evidence_packet": 12,
  "incident_packet": 5
}
```

Hardening addendum:

- log: `/tmp/bbx-hardening-fsv.ch1qc2hp/fsv.log`;
- SQLite source of truth:
  `sqlite:/tmp/bbx-hardening-fsv.ch1qc2hp/phase10/scanner.sqlite#ledger_records`;
- clean OSV report SHA:
  `sha256:5965cad3e2112453e57e9a69f20884235d84edc98805cfa189a140da2d111a45`;
- CycloneDX SBOM SHA:
  `sha256:7aca74d88d03ddb9a0091407265a51e25969b47a65d960188a97bcd1642ecab0`;
- final focused counts: `agent_run=1`, `tool_call=1`,
  `expected_delta=4`, `readback_snapshot=2`, `verifier_result=4`,
  `evidence_packet=4`, `incident_packet=1`;
- verifier rows: passed OSV filter match at sequence `5`, failed
  `SCANNER_REPORT_PACKAGE_NOT_FOUND` at sequence `9`, blocked
  `SCANNER_ARTIFACT_NOT_FILE` at sequence `13`, and blocked
  `SCANNER_SBOM_COMPONENT_VERSION_WITHOUT_NAME` at sequence `16`.

The automated hardened FSV asserts the complete Phase 10 suite final counts:
`expected_delta=15`, `readback_snapshot=11`, `verifier_result=15`,
`evidence_packet=15`, `incident_packet=6`.

Final metadata:

```json
{
  "schema_version": "5",
  "ledger_record_count": "53",
  "ledger_tip_sha256": "sha256:834c04a349061753903ec64d070476ab3cacde32639a2bc6b21fdab4232d8901"
}
```

The manual run printed before/after SQLite counts for every case and listed
all final verifier/readback/evidence rows, including artifact hashes and typed
error codes.
