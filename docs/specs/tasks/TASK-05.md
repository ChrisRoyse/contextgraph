# TASK-ARCH-005: Add CI gate for FFI consolidation

```xml
<task_spec id="TASK-ARCH-005" version="1.0">
<metadata>
  <title>Add CI gate for FFI consolidation</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements><requirement_ref>REQ-ARCH-005</requirement_ref></implements>
  <depends_on>TASK-04</depends_on>
  <estimated_hours>1</estimated_hours>
</metadata>

<context>
To prevent FFI scatter from recurring, a CI gate script must verify that all extern "C"
declarations for CUDA/FAISS are in context-graph-cuda crate only. This enforces the
architectural decision and enables focused security audits.
</context>

<input_context_files>
- /home/cabdru/contextgraph/.github/workflows/ (existing CI config)
- /home/cabdru/contextgraph/crates/context-graph-cuda/ (FFI crate from TASK-ARCH-004)
</input_context_files>

<scope>
<in_scope>
- Create scripts/check-ffi-consolidation.sh
- Script must find any extern "C" blocks outside context-graph-cuda
- Script must specifically check for cuda/faiss keywords
- Add script to CI workflow
- Script must exit 1 on violation, 0 on success
</in_scope>
<out_of_scope>
- Other CI checks
- FFI implementation (already done in TASK-ARCH-002, TASK-ARCH-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```bash
#!/usr/bin/env bash
# scripts/check-ffi-consolidation.sh
# Exit 0 if no FFI outside context-graph-cuda, exit 1 otherwise

set -euo pipefail

VIOLATIONS=$(find crates -name "*.rs" \
    -not -path "*/context-graph-cuda/*" \
    -exec grep -l 'extern "C"' {} \; 2>/dev/null | \
    xargs -I {} grep -l -E '(cuda|faiss|cuInit|cuDevice|faiss_)' {} 2>/dev/null || true)

if [ -n "$VIOLATIONS" ]; then
    echo "ERROR: Found CUDA/FAISS FFI outside context-graph-cuda:"
    echo "$VIOLATIONS"
    exit 1
fi

echo "OK: All CUDA/FAISS FFI consolidated in context-graph-cuda"
exit 0
```
</signatures>
<constraints>
- Script MUST be POSIX-compliant (works in sh, bash, dash)
- Script MUST handle crates with no extern "C" gracefully
- CI job MUST fail if script exits non-zero
- Script MUST provide clear error messages with file names
</constraints>
<verification>
```bash
# Test the script locally
chmod +x scripts/check-ffi-consolidation.sh
./scripts/check-ffi-consolidation.sh

# Verify CI integration
grep -q "check-ffi-consolidation" .github/workflows/*.yml
```
</verification>
</definition_of_done>

<files_to_create>
- scripts/check-ffi-consolidation.sh
</files_to_create>

<files_to_modify>
- .github/workflows/ci.yml (add FFI check job)
</files_to_modify>

<test_commands>
```bash
# Run script
./scripts/check-ffi-consolidation.sh

# Simulate violation (should fail)
echo 'extern "C" { fn cuInit(); }' > /tmp/test_violation.rs
# (manually verify script would catch this)
```
</test_commands>
</task_spec>
```

## Implementation Notes

### CI Workflow Addition

Add to `.github/workflows/ci.yml`:

```yaml
jobs:
  ffi-consolidation:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check FFI consolidation
        run: ./scripts/check-ffi-consolidation.sh
```

### False Positive Handling

The script specifically checks for CUDA/FAISS-related FFI. Other extern "C" blocks
(e.g., for SQLite, zstd) are allowed in their respective crates.

Keywords checked:
- `cuda` (case-insensitive)
- `faiss` (case-insensitive)
- `cuInit`, `cuDevice` (CUDA driver functions)
- `faiss_` (FAISS function prefix)

### Security Audit Benefit

With this gate in place, security audits of GPU code can focus on:
1. `crates/context-graph-cuda/src/ffi/` - All unsafe FFI
2. `crates/context-graph-cuda/src/safe/` - Safe wrappers

No other crates need GPU security review.
