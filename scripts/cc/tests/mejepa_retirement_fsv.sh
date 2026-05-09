#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="${OUT:-/tmp/contextgraph-mejepa-retirement-fsv}"
mkdir -p "${OUT}"

log() {
  printf '%s\n' "$*" | tee -a "${OUT}/run.log"
}

capture() {
  local label="$1"
  shift
  log "=== ${label} ==="
  log "\$ $*"
  if "$@" >"${OUT}/${label}.stdout" 2>"${OUT}/${label}.stderr"; then
    log "exit_code=0"
    cat "${OUT}/${label}.stdout" | tee -a "${OUT}/run.log"
    return 0
  fi
  local code=$?
  log "exit_code=${code}"
  log "stderr_tail:"
  tail -c 4096 "${OUT}/${label}.stderr" | tee -a "${OUT}/run.log"
  return "${code}"
}

cd "${ROOT}"
: >"${OUT}/run.log"

log "=== ME-JEPA retirement FSV ==="
log "source_of_truth.repo=${ROOT}"
log "source_of_truth.evidence=${OUT}"

log "--- BEFORE state: retired local LLM files must be absent ---"
test ! -f "crates/context-graph-cli/src/bin/reality-loop.rs"
test ! -d "vendor/llama-cpp-sys-2-0.1.145"
log "PASS: reality-loop source and vendored llama.cpp patch are absent"

if rg -n "llama-cpp-2|llama_cpp_2|vendor/llama-cpp-sys-2" Cargo.toml crates/context-graph-cli crates/context-graph-mcp crates/context-graph-causal-agent crates/context-graph-graph-agent --glob '!target' >"${OUT}/llama-active-references.txt"; then
  log "FAIL: active build manifests still reference llama-cpp-2"
  cat "${OUT}/llama-active-references.txt" | tee -a "${OUT}/run.log"
  exit 1
fi
log "PASS: active CLI/MCP/agent build surface does not reference llama-cpp-2"

log "--- Trigger: build ME-JEPA/CLI library without default CUDA surface ---"
capture "cargo-check-cli-lib" cargo check -p context-graph-cli --no-default-features --lib --quiet

log "--- Edge case 1: retired binary must fail closed ---"
if cargo check -p context-graph-cli --bin reality-loop --quiet >"${OUT}/retired-bin.stdout" 2>"${OUT}/retired-bin.stderr"; then
  log "FAIL: retired reality-loop binary still builds"
  exit 1
fi
if ! rg -q "no bin target named `reality-loop`|no bin target named 'reality-loop'|no bin target named" "${OUT}/retired-bin.stderr"; then
  log "FAIL: retired binary failed for an unexpected reason"
  cat "${OUT}/retired-bin.stderr" | tee -a "${OUT}/run.log"
  exit 1
fi
log "PASS: cargo rejects retired reality-loop binary as absent"

log "--- Edge case 2: MCP retired resolver must fail closed, not spawn stale target binaries ---"
capture "cargo-check-mcp-no-default" cargo check -p context-graph-mcp --no-default-features --lib --quiet
if ! rg -n "CCREALITY_ENGINE_RETIRED|local reality-loop engine and inner LLM actor are retired" crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs >"${OUT}/retired-resolver-readback.txt"; then
  log "FAIL: retired resolver source-of-truth readback missing"
  exit 1
fi
cat "${OUT}/retired-resolver-readback.txt" | tee -a "${OUT}/run.log"

log "--- Edge case 3: hook verification command must target ME-JEPA library, not retired binary ---"
if ! rg -n "cargo check -p context-graph-cli --no-default-features --lib" scripts/cc/hook_post_tool_use_edit.sh crates/context-graph-cli/src/commands/ccreality/mod.rs >"${OUT}/hook-command-readback.txt"; then
  log "FAIL: hook command readback missing ME-JEPA library check"
  exit 1
fi
cat "${OUT}/hook-command-readback.txt" | tee -a "${OUT}/run.log"

log "=== FSV PASS ==="
log "after.source_of_truth.files_absent=reality-loop.rs,vendor/llama-cpp-sys-2-0.1.145"
log "after.source_of_truth.build=cargo-check-cli-lib,cargo-check-mcp-no-default"
log "after.source_of_truth.fail_closed=retired-bin.stderr,retired-resolver-readback.txt"
