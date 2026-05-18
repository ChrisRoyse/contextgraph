---
namespace: discoveries
agent_id: claude-code
created: 2026-05-09T02:50:00Z
updated: 2026-05-09T02:50:00Z
status: active
tags: discovery, swebench, docker, run_id, fail-closed, validation
---

# SWE-bench harness run_id is embedded in Docker container names

## What

The official SWE-bench harness (`python -m swebench.harness.run_evaluation`)
uses the `--run_id` flag value DIRECTLY as a substring of the Docker
container name it creates per instance:

    sweb.eval.<instance_id>.<run_id>

Docker's container-name regex is `[a-zA-Z0-9][a-zA-Z0-9_.-]+`. Any
character outside `[a-zA-Z0-9_.-]` (e.g. `+`, `:`, `/`, ` `) causes a
silent harness-internal failure. The visible symptom is:

    requests.exceptions.HTTPError: 400 Client Error: Bad Request for url:
    http+docker://localhost/v1.54/containers/create?name=sweb.eval.<id>.<run_id>...
    docker.errors.APIError: 400 Client Error ... Bad Request
    ("Invalid container name (...), only [a-zA-Z0-9][a-zA-Z0-9_.-] are allowed")

The harness reports `error_instances=1` and the `report.json` is never
written, which is structurally indistinguishable from many other failure
modes. The Rust caller then sees `MEJEPA_ORACLE_IO: per-instance
report.json missing after harness run` with no obvious smoking gun.

## Why it matters

Any caller that invokes `swebench.harness.run_evaluation` programmatically
must validate the run_id against Docker's regex BEFORE invoking. Common
defaults that break it:

- `epoch+1234567s` (uses `+`)
- ISO 8601 timestamps with `:` (e.g. `2026-05-09T02:30:00Z`)
- Absolute paths (use `/`)
- Anything with whitespace

## How to apply

In `crates/context-graph-mejepa-corpus/src/swebench.rs::validate_config`:
the run_id alphabet is enforced as `[a-zA-Z0-9][a-zA-Z0-9_.-]+` up
front, returning `MEJEPA_ORACLE_INVALID_INPUT` with a remediation that
names the regex. Two unit tests lock this down:

- `validate_config_rejects_run_id_with_plus`
- `validate_config_rejects_run_id_starting_with_punctuation`

When constructing run_ids in scripts/CLIs, use a Docker-safe format:

    epoch.<unix_seconds>          (no +)
    mejepa-corpus-001             (slug)
    mejepa-corpus-2026-05-09e     (date with dashes only)

Avoid `:`, `+`, `/`, `\`, ` `, and any characters outside ASCII
alphanumerics + `_`, `.`, `-`.

## Related

- Code: `crates/context-graph-mejepa-corpus/src/swebench.rs`
- Tests: `swebench::tests::validate_config_rejects_run_id_*`
- FSV evidence: `/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`
- Original failure log: produced by the FSV's first run; the surfaced
  error included `Invalid container name (sweb.eval.sympy__sympy-20590.mejepa-swebench-oracle-gold-epoch+1778312175s)`.
