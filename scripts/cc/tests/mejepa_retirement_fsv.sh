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

log "--- BEFORE state: retired local LLM files and artifacts must be absent ---"
test ! -f "crates/context-graph-cli/src/bin/reality-loop.rs"
test ! -d "vendor/llama-cpp-sys-2-0.1.145"
test ! -f "config/ccreality_model_pins.json"
test ! -e "models"
test ! -e "target/debug/reality-loop"
test ! -e "target/release/reality-loop"
log "PASS: reality-loop source, vendored llama.cpp, model pins, model directory, and stale target binaries are absent"

if rg -n "llama-cpp-2|llama_cpp_2|vendor/llama-cpp-sys-2|reality_run_attempt|reality_run_smoke|reality_run_official_eval|reality_run_targeted_test|reality_recompile_reward|validate_low_confidence_edges" Cargo.toml Cargo.lock crates/context-graph-cli crates/context-graph-mcp/src/tools crates/context-graph-mcp/src/handlers/tools/dispatch.rs crates/context-graph-mcp/src/handlers/tools/reality_loop crates/context-graph-benchmark scripts/cc config .claude/agents --glob '!target/**' --glob '!scripts/cc/tests/mejepa_retirement_fsv.sh' >"${OUT}/retired-active-references.txt"; then
  log "FAIL: active build/tooling surface still references retired local LLM or runner symbols"
  cat "${OUT}/retired-active-references.txt" | tee -a "${OUT}/run.log"
  exit 1
fi
if rg -n "CausalDiscoveryLLM|GraphDiscoveryService|Hermes|Qwen|qwen|GGUF|ollama" crates/context-graph-cli crates/context-graph-mcp/src/tools crates/context-graph-mcp/src/handlers/tools/reality_loop crates/context-graph-benchmark scripts/cc config --glob '!target/**' --glob '!scripts/cc/tests/mejepa_retirement_fsv.sh' >"${OUT}/retired-runtime-references.txt"; then
  log "FAIL: active runtime/tooling surface still references retired local LLM runtime symbols"
  cat "${OUT}/retired-runtime-references.txt" | tee -a "${OUT}/run.log"
  exit 1
fi
log "PASS: active CLI/MCP/benchmark/tooling surface does not reference retired local LLM or runner symbols"

log "--- Trigger: build ME-JEPA/CLI library without default CUDA surface ---"
capture "cargo-check-cli-lib" cargo check -p context-graph-cli --no-default-features --lib --quiet

log "--- Edge case 1: retired binary must fail closed ---"
if cargo check -p context-graph-cli --bin reality-loop --quiet >"${OUT}/retired-bin.stdout" 2>"${OUT}/retired-bin.stderr"; then
  log "FAIL: retired reality-loop binary still builds"
  exit 1
fi
if ! rg -q 'no bin target named `reality-loop`|no bin target named '\''reality-loop'\''|no bin target named' "${OUT}/retired-bin.stderr"; then
  log "FAIL: retired binary failed for an unexpected reason"
  cat "${OUT}/retired-bin.stderr" | tee -a "${OUT}/run.log"
  exit 1
fi
log "PASS: cargo rejects retired reality-loop binary as absent"

log "--- Edge case 2: MCP must not expose retired runner tools or stale binary spawns ---"
capture "cargo-check-mcp-no-default" cargo check -p context-graph-mcp --no-default-features --lib --quiet
for retired_tool in reality_run_attempt reality_run_smoke reality_run_official_eval reality_run_targeted_test reality_recompile_reward; do
  if rg -n "${retired_tool}" crates/context-graph-mcp/src/tools/names.rs crates/context-graph-mcp/src/tools/definitions/reality.rs crates/context-graph-mcp/src/handlers/tools/dispatch.rs crates/context-graph-mcp/src/main.rs >"${OUT}/retired-tool-${retired_tool}.txt"; then
    log "FAIL: retired MCP runner tool remains exposed: ${retired_tool}"
    cat "${OUT}/retired-tool-${retired_tool}.txt" | tee -a "${OUT}/run.log"
    exit 1
  fi
done
if rg -n "locate_engine_binary|spawn_harness_subcommand|CCREALITY_ENGINE_RETIRED" crates/context-graph-mcp/src/handlers/tools/reality_loop >"${OUT}/retired-spawn-readback.txt"; then
  log "FAIL: stale reality-loop binary spawn path remains"
  cat "${OUT}/retired-spawn-readback.txt" | tee -a "${OUT}/run.log"
  exit 1
fi
log "PASS: retired MCP runner tools and stale binary spawn path are absent"

log "--- Edge case 3: hooks and benchmark manifests must not target retired binary or local LLM bins ---"
if ! rg -n "cargo check -p context-graph-cli --no-default-features --lib" scripts/cc/hook_post_tool_use_edit.sh crates/context-graph-cli/src/commands/ccreality/mod.rs >"${OUT}/hook-command-readback.txt"; then
  log "FAIL: hook command readback missing ME-JEPA library check"
  exit 1
fi
cat "${OUT}/hook-command-readback.txt" | tee -a "${OUT}/run.log"
for retired_bin in causal-discovery-mcp-bench causal-perf-bench causal-prompt-experiment causal-provenance-e2e-bench causal-provenance-llm-bench causal-span-fix-bench graph-linking-gpu-bench mcp-bench model-injection-bench model-injection-rigorous-bench validation-bench causal-e2e-bench; do
  if rg -n "${retired_bin}" crates/context-graph-benchmark/Cargo.toml >"${OUT}/retired-bin-${retired_bin}.txt"; then
    log "FAIL: retired benchmark bin remains in Cargo manifest: ${retired_bin}"
    cat "${OUT}/retired-bin-${retired_bin}.txt" | tee -a "${OUT}/run.log"
    exit 1
  fi
done
log "PASS: hook and benchmark manifest readbacks exclude retired binary/local LLM targets"

log "=== FSV PASS ==="
log "after.source_of_truth.files_absent=reality-loop.rs,vendor/llama-cpp-sys-2-0.1.145,config/ccreality_model_pins.json,models,target reality-loop binaries"
log "after.source_of_truth.build=cargo-check-cli-lib,cargo-check-mcp-no-default"
log "after.source_of_truth.retired_symbols=retired-active-references.txt,retired-tool-*.txt,retired-spawn-readback.txt"
