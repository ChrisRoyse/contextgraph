from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .commands import (
    command_gold_smoke,
    command_image_status,
    command_preflight,
    command_prune_cache,
    command_pull_images,
)
from .core import (
    DEFAULT_DATASET,
    DEFAULT_EVIDENCE_ROOT,
    DEFAULT_SMOKE_TASK,
    DEFAULT_SPLIT,
    DEFAULT_SWEBENCH_ROOT,
    DEFAULT_SWEBENCH_VENV,
    DEFAULT_TASKS_DIR,
    SwebenchOpsError,
    utc_now,
    write_json_checked,
)
from .oracle_events import command_oracle_events
from .python_ast_chunk_command import command_ast_chunks
from .python_ast_parse import command_ast_parse
from .python_diff_chunk_map import command_diff_chunk_map
from .python_instrument_registry import command_instrument_registry
from .smoke_manifest import DEFAULT_SEED, DEFAULT_TARGET_COUNT, EXPECTED_MUTATION_CATEGORIES, command_smoke_manifest
from .smoke_intelligence_matrices import command_smoke_intelligence
from .smoke_storage_design import command_smoke_storage_design
from .full_run_relaunch_gate import command_full_run_relaunch_gate
from .python_full_ast_corpus import command_full_ast_corpus
from .python_static_graph import command_static_graph
from .python_test_intent import command_test_intent
from .code_constellation_schema import command_code_constellation_schema
from .python_dda_code_signals import command_python_dda_code_signals
from .python_pairwise_dda_mi import command_python_pairwise_dda_mi
from .python_tct_cells import command_python_tct_cells
from .python_reward_tensors import command_python_reward_tensors
from .python_reward_field_analysis import command_python_reward_field_analysis
from .python_data_readiness_gate import add_arguments as add_python_data_readiness_gate_arguments
from .python_data_readiness_gate import command_python_data_readiness_gate
from .python_embedder_contribution import add_arguments as add_python_embedder_contribution_arguments
from .python_embedder_contribution import command_python_embedder_contribution
from .python_reward_field_sample import add_arguments as add_python_reward_field_sample_arguments
from .python_reward_field_sample import command_python_reward_field_sample
from .python_tct_completeness import add_arguments as add_python_tct_completeness_arguments
from .python_tct_completeness import command_python_tct_completeness
from .python_proof_domain_isolation import add_arguments as add_python_proof_domain_isolation_arguments
from .python_proof_domain_isolation import command_python_proof_domain_isolation
from .python_secret_redaction import add_arguments as add_python_secret_redaction_arguments
from .python_secret_redaction import command_python_secret_redaction
from .python_failure_fingerprint_materializer import add_arguments as add_python_failure_fingerprint_arguments
from .python_failure_fingerprint_materializer import command_python_failure_fingerprint_materializer
from .python_failure_fingerprint_calibrator import add_arguments as add_python_failure_fingerprint_calibration_arguments
from .python_failure_fingerprint_calibrator import command_python_failure_fingerprint_calibrator
from .python_real_corpus_constellation_smoke import add_arguments as add_python_real_corpus_constellation_smoke_arguments
from .python_real_corpus_constellation_smoke import command_python_real_corpus_constellation_smoke
from .python_constellation_ablation_suite import add_arguments as add_python_constellation_ablation_suite_arguments
from .python_constellation_ablation_suite import command_python_constellation_ablation_suite
from .python_constellation_ablation_uncalibrated import (
    add_arguments as add_python_constellation_ablation_uncalibrated_arguments,
)
from .python_constellation_ablation_uncalibrated import command_python_constellation_ablation_uncalibrated
from .python_constellation_tensors import add_arguments as add_python_constellation_tensors_arguments
from .python_constellation_tensors import command_python_constellation_tensors
from .python_per_slot_calibration_ood import add_arguments as add_python_per_slot_calibration_ood_arguments
from .python_per_slot_calibration_ood import command_python_per_slot_calibration_ood
from .python_per_cell_smooth_ece import add_arguments as add_python_per_cell_smooth_ece_arguments
from .python_per_cell_smooth_ece import command_python_per_cell_smooth_ece
from .python_calibration_active_gate import add_arguments as add_python_calibration_active_gate_arguments
from .python_calibration_active_gate import command_python_calibration_active_gate
from .python_tracin_abstain_pool import add_arguments as add_python_tracin_abstain_pool_arguments
from .python_tracin_abstain_pool import command_python_tracin_abstain_pool
from .python_exact_tracin_gradient_rows import (
    add_arguments as add_python_exact_tracin_gradient_rows_arguments,
)
from .python_exact_tracin_gradient_rows import command_python_exact_tracin_gradient_rows
from .python_pair_feature_audit import add_arguments as add_python_pair_feature_audit_arguments
from .python_pair_feature_audit import command_python_pair_feature_audit
from .python_pair_feature_gate_probe import add_arguments as add_python_pair_feature_gate_probe_arguments
from .python_pair_feature_gate_probe import command_python_pair_feature_gate_probe
from .python_pair_surface_mining import add_arguments as add_python_pair_surface_mining_arguments
from .python_pair_surface_mining import command_python_pair_surface_mining
from .python_pair_conditional_admission_probe import (
    add_arguments as add_python_pair_conditional_admission_probe_arguments,
)
from .python_pair_conditional_admission_probe import command_python_pair_conditional_admission_probe
from .python_pair_interaction_mining import add_arguments as add_python_pair_interaction_mining_arguments
from .python_pair_interaction_mining import command_python_pair_interaction_mining
from .python_mendeleev_pair_table import add_arguments as add_python_mendeleev_pair_table_arguments
from .python_mendeleev_pair_table import command_python_mendeleev_pair_table
from .python_axis_rotation_diagnostic import add_arguments as add_python_axis_rotation_diagnostic_arguments
from .python_axis_rotation_diagnostic import command_python_axis_rotation_diagnostic
from .python_per_cell_calibration_substrate import (
    add_arguments as add_python_per_cell_calibration_substrate_arguments,
)
from .python_per_cell_calibration_substrate import command_python_per_cell_calibration_substrate
from .python_named_pair_selector_retest import add_arguments as add_python_named_pair_selector_retest_arguments
from .python_named_pair_selector_retest import command_python_named_pair_selector_retest
from .python_foundationality_pressure_probe import (
    add_arguments as add_python_foundationality_pressure_probe_arguments,
)
from .python_foundationality_pressure_probe import command_python_foundationality_pressure_probe
from .python_anti_compensation_fsv import add_arguments as add_python_anti_compensation_fsv_arguments
from .python_anti_compensation_fsv import command_python_anti_compensation_fsv
from .python_auto_label_discovery import add_arguments as add_python_auto_label_discovery_arguments
from .python_auto_label_discovery import command_python_auto_label_discovery
from .panel_b_artifacts import add_arguments as add_panel_b_artifacts_arguments
from .panel_b_artifacts import command_panel_b_artifacts
from .panel_b_model_scores import add_arguments as add_panel_b_model_scores_arguments
from .panel_b_model_scores import command_panel_b_model_scores
from .panel_b_score_rows import add_arguments as add_panel_b_score_rows_arguments
from .panel_b_score_rows import command_panel_b_score_rows
from .panel_b_parity_audit import add_arguments as add_panel_b_parity_audit_arguments
from .panel_b_parity_audit import command_panel_b_parity_audit
from .panel_b_parity_score import add_arguments as add_panel_b_parity_score_arguments
from .panel_b_parity_score import command_panel_b_parity_score


def add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--swebench-root", type=Path, default=DEFAULT_SWEBENCH_ROOT)
    parser.add_argument("--venv", type=Path, default=DEFAULT_SWEBENCH_VENV)
    parser.add_argument("--tasks-dir", type=Path, default=DEFAULT_TASKS_DIR)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_EVIDENCE_ROOT)
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET)
    parser.add_argument("--split", default=DEFAULT_SPLIT)
    parser.add_argument("--namespace", default="swebench")
    parser.add_argument("--arch", default="x86_64")
    parser.add_argument("--tag", default="latest")
    parser.add_argument("--timeout", type=int, default=600)


def add_expected(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--expected-instance-ids",
        nargs="*",
        default=None,
        help="SWE-bench images allowed in local cache. Defaults to the command's sample task.",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Fail-closed operational checks for ContextGraph's SWE-bench Lite Phase 0 gate.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    preflight = sub.add_parser("preflight", help="Verify official harness, Docker, dataset, manifests, and cache hygiene.")
    add_common(preflight)
    add_expected(preflight)
    preflight.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    preflight.add_argument(
        "--allow-dirty-cache",
        dest="require_clean_cache",
        action="store_false",
        default=True,
        help="Record dirty cache state instead of failing. Do not use for readiness gates.",
    )
    preflight.set_defaults(func=command_preflight, evidence_name="preflight-evidence.json")

    status = sub.add_parser("image-status", help="Inspect local SWE-bench Docker image cache.")
    add_common(status)
    add_expected(status)
    status.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    status.add_argument("--require-clean", action="store_true")
    status.set_defaults(func=command_image_status, evidence_name="image-status-evidence.json")

    pull = sub.add_parser("pull-images", help="Pull only the explicitly needed official SWE-bench Lite images.")
    add_common(pull)
    pull.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    pull.add_argument("--instance-ids", nargs="*")
    pull.add_argument(
        "--allow-dirty-cache",
        dest="require_clean",
        action="store_false",
        default=True,
        help="Do not fail if unrelated SWE-bench images are present after pull.",
    )
    pull.set_defaults(func=command_pull_images, evidence_name="pull-images-evidence.json")

    prune = sub.add_parser("prune-cache", help="Remove stale SWE-bench images not in the expected set.")
    add_common(prune)
    add_expected(prune)
    prune.add_argument("--sample-task", default=DEFAULT_SMOKE_TASK)
    prune.add_argument("--yes", action="store_true", help="Actually remove images. Without this, only writes a dry-run plan.")
    prune.set_defaults(func=command_prune_cache, evidence_name="prune-cache-evidence.json")

    smoke = sub.add_parser("gold-smoke", help="Run one official SWE-bench Lite gold evaluation through Docker.")
    add_common(smoke)
    smoke.add_argument("--instance-id", default=DEFAULT_SMOKE_TASK)
    smoke.add_argument("--run-id")
    smoke.add_argument("--instance-timeout", type=int, default=1800)
    smoke.set_defaults(func=command_gold_smoke, evidence_name="gold-smoke-evidence.json")

    events = sub.add_parser("oracle-events", help="Materialize strict typed oracle events from Docker oracle outcomes.")
    events.add_argument("--outcomes-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-oracle-baselines-v1/oracle_outcomes.jsonl"))
    events.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    events.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    events.add_argument("--event-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-oracle-events-v1"))
    events.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-008-oracle-events-fsv"))
    events.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-008-oracle-events-fsv"))
    events.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-008-oracle-events"))
    events.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    events.set_defaults(func=command_oracle_events, evidence_name="oracle-events-command-evidence.json")

    ast_parse = sub.add_parser("ast-parse", help="Materialize fail-closed Python AST parse records.")
    ast_parse.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    ast_parse.add_argument("--parse-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-parse-v1"))
    ast_parse.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-009-ast-parse-fsv"))
    ast_parse.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-009-ast-parse-fsv"))
    ast_parse.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-009-ast-parse"))
    ast_parse.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    ast_parse.set_defaults(func=command_ast_parse, evidence_name="ast-parse-command-evidence.json")

    ast_chunks = sub.add_parser("ast-chunks", help="Materialize stable AST-aware Python code chunks.")
    ast_chunks.add_argument("--parse-records", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-parse-v1/python_ast_parse_records.jsonl"))
    ast_chunks.add_argument("--chunk-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1"))
    ast_chunks.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-010-ast-chunks-fsv"))
    ast_chunks.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-010-ast-chunks-fsv"))
    ast_chunks.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-010-ast-chunks"))
    ast_chunks.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    ast_chunks.set_defaults(func=command_ast_chunks, evidence_name="ast-chunks-command-evidence.json")

    smoke_manifest = sub.add_parser(
        "smoke-manifest",
        help="Select a deterministic real Python AST chunk smoke manifest for embedder intelligence runs.",
    )
    smoke_manifest.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    smoke_manifest.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    smoke_manifest.add_argument("--manifest-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest"))
    smoke_manifest.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv"))
    smoke_manifest.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv"))
    smoke_manifest.add_argument("--target-count", type=int, default=DEFAULT_TARGET_COUNT)
    smoke_manifest.add_argument("--seed", default=DEFAULT_SEED)
    smoke_manifest.add_argument("--expected-task-count", type=int, default=300)
    smoke_manifest.add_argument("--expected-mutation-categories", default=",".join(EXPECTED_MUTATION_CATEGORIES))
    smoke_manifest.add_argument("--full-ast-corpus-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json"))
    smoke_manifest.set_defaults(func=command_smoke_manifest, evidence_name="smoke-manifest-command-evidence.json")

    smoke_intelligence = sub.add_parser(
        "smoke-intelligence-matrices",
        help="Compute durable representation-alignment matrices for Python embedder smoke or completed-shard runs.",
    )
    smoke_intelligence.add_argument("--forward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache"))
    smoke_intelligence.add_argument("--input-mode", choices=["smoke-manifest", "completed-shards"], default="smoke-manifest")
    smoke_intelligence.add_argument("--smoke-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    smoke_intelligence.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    smoke_intelligence.add_argument("--checkpoint", type=Path, default=None)
    smoke_intelligence.add_argument("--shard-plan", type=Path, default=None)
    smoke_intelligence.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices"))
    smoke_intelligence.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv"))
    smoke_intelligence.add_argument("--task-id", default="TASK-PY-DATA-062")
    smoke_intelligence.add_argument("--expected-chunk-count", type=int, default=1000)
    smoke_intelligence.add_argument("--expected-embedder-count", type=int, default=12)
    smoke_intelligence.add_argument("--min-overlap", type=int, default=20)
    smoke_intelligence.add_argument("--neighbor-k", type=int, default=10)
    smoke_intelligence.add_argument("--min-completed-shards", type=int, default=1)
    smoke_intelligence.add_argument("--max-completed-shards", type=int, default=None)
    smoke_intelligence.add_argument("--clean-output", action="store_true")
    smoke_intelligence.set_defaults(func=command_smoke_intelligence, evidence_name="smoke-intelligence-matrices-command-evidence.json")

    smoke_storage_design = sub.add_parser(
        "smoke-storage-design",
        help="Derive bounded full-run storage and metric policy from the 1000-chunk smoke outputs.",
    )
    smoke_storage_design.add_argument("--forward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache"))
    smoke_storage_design.add_argument("--matrix-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices"))
    smoke_storage_design.add_argument("--design-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design"))
    smoke_storage_design.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-063-post249-smoke-storage-design-fsv"))
    smoke_storage_design.add_argument("--forward-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json"))
    smoke_storage_design.add_argument("--matrix-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"))
    smoke_storage_design.add_argument("--full-chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    smoke_storage_design.add_argument("--full-chunk-count", type=int, default=None)
    smoke_storage_design.add_argument("--smoke-chunk-count", type=int, default=1000)
    smoke_storage_design.add_argument("--window-chunks", type=int, default=1000)
    smoke_storage_design.add_argument("--max-durable-forward-bytes", type=int, default=536_870_912_000)
    smoke_storage_design.add_argument("--clean-output", action="store_true")
    smoke_storage_design.set_defaults(func=command_smoke_storage_design, evidence_name="smoke-storage-design-command-evidence.json")

    full_run_relaunch_gate = sub.add_parser(
        "full-run-relaunch-gate",
        help="Apply accepted post-#249 smoke findings and emit the fail-closed #255 full-run relaunch gate.",
    )
    full_run_relaunch_gate.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-064-post249-full-run-relaunch-gate-fsv"))
    full_run_relaunch_gate.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    full_run_relaunch_gate.add_argument("--cache-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515"))
    full_run_relaunch_gate.add_argument("--run-fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-016-full-c8117f15-20260515-fsv"))
    full_run_relaunch_gate.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-016-full-c8117f15-20260515"))
    full_run_relaunch_gate.add_argument("--stale-cache-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-20260514"))
    full_run_relaunch_gate.add_argument("--stale-run-fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-016-full-20260514-fsv"))
    full_run_relaunch_gate.add_argument("--stale-hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-016-full-20260514"))
    full_run_relaunch_gate.add_argument("--smoke-manifest-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json"))
    full_run_relaunch_gate.add_argument("--smoke-forward-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json"))
    full_run_relaunch_gate.add_argument("--smoke-intelligence-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"))
    full_run_relaunch_gate.add_argument("--smoke-storage-design-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-063-post249-smoke-storage-design-fsv/smoke_storage_design_fsv.json"))
    full_run_relaunch_gate.add_argument("--full-ast-corpus-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json"))
    full_run_relaunch_gate.add_argument("--storage-design-go-no-go", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/go_no_go.json"))
    full_run_relaunch_gate.add_argument("--storage-policy", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/storage_policy.json"))
    full_run_relaunch_gate.add_argument("--storage-estimates", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/full_scale_storage_estimates.json"))
    full_run_relaunch_gate.add_argument("--shard-size", type=int, default=128)
    full_run_relaunch_gate.add_argument("--shard-count", type=int, default=3124)
    full_run_relaunch_gate.add_argument("--active-embedder-count", type=int, default=12)
    full_run_relaunch_gate.add_argument("--true-batch-size", type=int, default=8)
    full_run_relaunch_gate.add_argument("--io-workers", type=int, default=2)
    full_run_relaunch_gate.add_argument("--cpu-workers", type=int, default=30)
    full_run_relaunch_gate.add_argument("--gpu-workers", type=int, default=1)
    full_run_relaunch_gate.add_argument("--jsonl-sync-every-rows", type=int, default=512)
    full_run_relaunch_gate.add_argument("--checkpoint-every-shards", type=int, default=1)
    full_run_relaunch_gate.add_argument("--cargo-runner", default="target/release/examples/python_forward_cache_fsv")
    full_run_relaunch_gate.add_argument("--expected-next-shard", default="auto")
    full_run_relaunch_gate.add_argument("--clean-output", action="store_true")
    full_run_relaunch_gate.set_defaults(func=command_full_run_relaunch_gate, evidence_name="full-run-relaunch-gate-command-evidence.json")

    full_ast_corpus = sub.add_parser(
        "full-ast-corpus",
        help="Regenerate accepted Python AST parse/chunk SoTs with fail-closed 300x8 cleaned-manifest coverage.",
    )
    full_ast_corpus.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    full_ast_corpus.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    full_ast_corpus.add_argument("--repo-object-root", type=Path, default=Path("/zfs/archive/contextgraph/cache/mejepa-corpus/repos"))
    full_ast_corpus.add_argument("--parse-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-parse-v1"))
    full_ast_corpus.add_argument("--chunk-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1"))
    full_ast_corpus.add_argument("--source-snapshot-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files"))
    full_ast_corpus.add_argument("--task-checkpoint-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/task-checkpoints-v1"))
    full_ast_corpus.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv"))
    full_ast_corpus.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv"))
    full_ast_corpus.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-058-full-ast-corpus"))
    full_ast_corpus.add_argument("--hot-budget-bytes", type=int, default=85_899_345_920)
    full_ast_corpus.add_argument("--git-timeout-seconds", type=int, default=900)
    full_ast_corpus.add_argument("--recommended-shard-size", type=int, default=128)
    full_ast_corpus.add_argument("--max-tasks", type=int, default=None)
    full_ast_corpus.add_argument("--allow-partial-coverage", dest="require_full_coverage", action="store_false", default=True)
    full_ast_corpus.add_argument("--clean-output", action="store_true")
    full_ast_corpus.add_argument("--resume-from-task-checkpoints", action="store_true")
    full_ast_corpus.set_defaults(func=command_full_ast_corpus, evidence_name="full-ast-corpus-command-evidence.json")

    diff_map = sub.add_parser("diff-chunk-map", help="Map Python patch/test hunks to stable AST chunks.")
    diff_map.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    diff_map.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    diff_map.add_argument("--map-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-diff-chunk-map-v1"))
    diff_map.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-011-diff-chunk-map-fsv"))
    diff_map.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-011-diff-chunk-map-fsv"))
    diff_map.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-011-diff-chunk-map"))
    diff_map.set_defaults(func=command_diff_chunk_map, evidence_name="diff-chunk-map-command-evidence.json")

    static_graph = sub.add_parser("static-graph", help="Extract deterministic Python static reality graph features.")
    static_graph.add_argument("--parse-records", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-parse-v1/python_ast_parse_records.jsonl"))
    static_graph.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    static_graph.add_argument("--graph-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-static-reality-graph-v1"))
    static_graph.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-012-static-graph-fsv"))
    static_graph.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-012-static-graph-fsv"))
    static_graph.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-012-static-graph"))
    static_graph.set_defaults(func=command_static_graph, evidence_name="static-graph-command-evidence.json")

    test_intent = sub.add_parser("test-intent", help="Extract Python test intent and assertion reward signals.")
    test_intent.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    test_intent.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    test_intent.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    test_intent.add_argument("--diff-map-records", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-diff-chunk-map-v1/diff_chunk_map_records.jsonl"))
    test_intent.add_argument("--intent-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-test-intent-v1"))
    test_intent.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-013-test-intent-fsv"))
    test_intent.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-013-test-intent-fsv"))
    test_intent.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-013-test-intent"))
    test_intent.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    test_intent.set_defaults(func=command_test_intent, evidence_name="test-intent-command-evidence.json")

    code_constellation = sub.add_parser(
        "code-constellation-schema",
        help="Define and FSV-verify the Python code teleological constellation schema.",
    )
    code_constellation.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    code_constellation.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    code_constellation.add_argument("--schema-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-code-constellation-schema-v1"))
    code_constellation.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-019-code-constellation-schema-fsv"))
    code_constellation.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-019-code-constellation-schema-fsv"))
    code_constellation.set_defaults(func=command_code_constellation_schema, evidence_name="code-constellation-schema-command-evidence.json")

    dda_signals = sub.add_parser(
        "python-dda-code-signals",
        help="Materialize first-order DDA reward/supervision signals from real Python embedder outputs.",
    )
    dda_signals.add_argument("--forward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache"))
    dda_signals.add_argument("--input-mode", choices=["smoke-manifest", "completed-shards"], default="smoke-manifest")
    dda_signals.add_argument("--smoke-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    dda_signals.add_argument("--chunks-jsonl", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
    dda_signals.add_argument("--checkpoint", type=Path, default=None)
    dda_signals.add_argument("--shard-plan", type=Path, default=None)
    dda_signals.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    dda_signals.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv"))
    dda_signals.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv"))
    dda_signals.add_argument("--constellation-schema", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json"))
    dda_signals.add_argument("--expected-embedders", default="e1,e6,e7,e8,e10,e12,e13,e14,e2,e3,e4,e9")
    dda_signals.add_argument("--min-completed-shards", type=int, default=1)
    dda_signals.add_argument("--max-completed-shards", type=int, default=None)
    dda_signals.add_argument("--clean-output", action="store_true")
    dda_signals.set_defaults(func=command_python_dda_code_signals, evidence_name="python-dda-code-signals-command-evidence.json")

    pairwise_dda = sub.add_parser(
        "python-pairwise-dda-mi",
        help="Compute pairwise DDA interaction and mutual-information audit rows from Python DDA signals.",
    )
    pairwise_dda.add_argument("--input-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    pairwise_dda.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise"))
    pairwise_dda.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-018-pairwise-dda-mi-fsv"))
    pairwise_dda.add_argument("--expected-embedders", default="e1,e6,e7,e8,e10,e12,e13,e14,e2,e3,e4,e9")
    pairwise_dda.add_argument("--min-overlap", type=int, default=20)
    pairwise_dda.add_argument("--min-support", type=int, default=20)
    pairwise_dda.add_argument("--mi-bins", type=int, default=8)
    pairwise_dda.add_argument("--top-k", type=int, default=20)
    pairwise_dda.add_argument("--clean-output", action="store_true")
    pairwise_dda.set_defaults(func=command_python_pairwise_dda_mi, evidence_name="python-pairwise-dda-mi-command-evidence.json")

    tct_cells = sub.add_parser(
        "python-tct-cells",
        help="Build Python teleological constellation cell memberships/support from DDA-enriched smoke outputs.",
    )
    tct_cells.add_argument("--constellation-schema", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json"))
    tct_cells.add_argument("--smoke-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    tct_cells.add_argument("--forward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache"))
    tct_cells.add_argument("--dda-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    tct_cells.add_argument("--pairwise-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise"))
    tct_cells.add_argument("--matrix-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices"))
    tct_cells.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1"))
    tct_cells.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-020-tct-cells-fsv"))
    tct_cells.add_argument("--full-ast-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json"))
    tct_cells.add_argument("--smoke-manifest-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json"))
    tct_cells.add_argument("--forward-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json"))
    tct_cells.add_argument("--dda-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json"))
    tct_cells.add_argument("--pairwise-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-018-pairwise-dda-mi-fsv/python_pairwise_dda_mi_fsv.json"))
    tct_cells.add_argument("--matrix-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"))
    tct_cells.add_argument("--expected-chunk-count", type=int, default=1000)
    tct_cells.add_argument("--expected-embedders", default="e1,e6,e7,e8,e10,e12,e13,e14,e2,e3,e4,e9")
    tct_cells.add_argument("--clean-output", action="store_true")
    tct_cells.set_defaults(func=command_python_tct_cells, evidence_name="python-tct-cells-command-evidence.json")

    reward_tensors = sub.add_parser(
        "python-reward-tensors",
        help="Assemble strict Q1-Q5 RealityPrediction reward tensors from Python DDA/TCT artifacts.",
    )
    reward_tensors.add_argument("--smoke-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    reward_tensors.add_argument("--dda-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    reward_tensors.add_argument("--tct-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1"))
    reward_tensors.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1"))
    reward_tensors.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-021-reward-tensors-fsv"))
    reward_tensors.add_argument("--constellation-schema", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-code-constellation-schema-v1/code_constellation_schema.json"))
    reward_tensors.add_argument("--full-ast-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json"))
    reward_tensors.add_argument("--smoke-manifest-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json"))
    reward_tensors.add_argument("--dda-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json"))
    reward_tensors.add_argument("--tct-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-020-tct-cells-fsv/python_tct_cells_fsv.json"))
    reward_tensors.add_argument("--expected-chunk-count", type=int, default=1000)
    reward_tensors.add_argument("--clean-output", action="store_true")
    reward_tensors.set_defaults(func=command_python_reward_tensors, evidence_name="python-reward-tensors-command-evidence.json")

    reward_analysis = sub.add_parser(
        "python-reward-field-analysis",
        help="Analyze Python reward-field density, gaps, risks, and training readiness.",
    )
    reward_analysis.add_argument("--reward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1"))
    reward_analysis.add_argument("--dda-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    reward_analysis.add_argument("--pairwise-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise"))
    reward_analysis.add_argument("--tct-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1"))
    reward_analysis.add_argument("--matrix-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices"))
    reward_analysis.add_argument("--output-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-reward-field-analysis-v1"))
    reward_analysis.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-022-reward-field-analysis-fsv"))
    reward_analysis.add_argument("--reward-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-021-reward-tensors-fsv/python_reward_tensors_fsv.json"))
    reward_analysis.add_argument("--dda-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json"))
    reward_analysis.add_argument("--tct-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-020-tct-cells-fsv/python_tct_cells_fsv.json"))
    reward_analysis.add_argument("--matrix-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"))
    reward_analysis.add_argument("--expected-chunk-count", type=int, default=1000)
    reward_analysis.add_argument("--clean-output", action="store_true")
    reward_analysis.set_defaults(func=command_python_reward_field_analysis, evidence_name="python-reward-field-analysis-command-evidence.json")

    embedder_contribution = sub.add_parser(
        "python-embedder-contribution",
        help="Quantify smoke-corpus embedder contribution to Q1-Q5 RealityPrediction targets.",
    )
    add_python_embedder_contribution_arguments(embedder_contribution)
    embedder_contribution.set_defaults(func=command_python_embedder_contribution, evidence_name="python-embedder-contribution-command-evidence.json")

    data_readiness = sub.add_parser(
        "python-data-readiness-gate",
        help="Fail closed unless Python ME-JEPA reward data is ready for the requested training preflight.",
    )
    add_python_data_readiness_gate_arguments(data_readiness)
    data_readiness.set_defaults(func=command_python_data_readiness_gate, evidence_name="python-data-readiness-gate-command-evidence.json")

    reward_sample = sub.add_parser(
        "python-reward-field-sample",
        help="Persist one end-to-end physical Python ME-JEPA reward-field sample with FSV.",
    )
    add_python_reward_field_sample_arguments(reward_sample)
    reward_sample.set_defaults(func=command_python_reward_field_sample, evidence_name="python-reward-field-sample-command-evidence.json")

    tct_completeness = sub.add_parser(
        "python-tct-completeness",
        help="Audit Python teleological-constellation reward-signal completeness.",
    )
    add_python_tct_completeness_arguments(tct_completeness)
    tct_completeness.set_defaults(func=command_python_tct_completeness, evidence_name="python-tct-completeness-command-evidence.json")

    proof_domain_isolation = sub.add_parser(
        "python-proof-domain-isolation",
        help="Fail closed if proof-domain harnesses become active Python ship-gate prerequisites.",
    )
    add_python_proof_domain_isolation_arguments(proof_domain_isolation)
    proof_domain_isolation.set_defaults(func=command_python_proof_domain_isolation, evidence_name="python-proof-domain-isolation-command-evidence.json")

    secret_redaction = sub.add_parser(
        "python-secret-redaction",
        help="TASK-PY-G-059 PHASE 1: redaction library FSV (patterns + invariance + perf + fail-closed).",
    )
    add_python_secret_redaction_arguments(secret_redaction)
    secret_redaction.set_defaults(func=command_python_secret_redaction, evidence_name="python-secret-redaction-command-evidence.json")

    failure_fingerprints = sub.add_parser(
        "python-failure-fingerprint-materialize",
        help="TASK-FP-002: materialize seed failure-shape fingerprint centroids and variance from packed forwards.",
    )
    add_python_failure_fingerprint_arguments(failure_fingerprints)
    failure_fingerprints.set_defaults(
        func=command_python_failure_fingerprint_materializer,
        evidence_name="python-failure-fingerprint-materialize-command-evidence.json",
    )

    calibrate_fingerprints = sub.add_parser(
        "python-failure-fingerprint-calibrate",
        help="TASK-FP-003: calibrate per-fingerprint G-tau thresholds from a seed fingerprint catalog.",
    )
    add_python_failure_fingerprint_calibration_arguments(calibrate_fingerprints)
    calibrate_fingerprints.set_defaults(
        func=command_python_failure_fingerprint_calibrator,
        evidence_name="python-failure-fingerprint-calibrate-command-evidence.json",
    )

    real_corpus_constellation = sub.add_parser(
        "python-real-corpus-constellation-smoke",
        help="TASK-PY-G-095: smoke real aiwonder packed forwards as slot-preserving teleological constellations.",
    )
    add_python_real_corpus_constellation_smoke_arguments(real_corpus_constellation)
    real_corpus_constellation.set_defaults(
        func=command_python_real_corpus_constellation_smoke,
        evidence_name="python-real-corpus-constellation-smoke-command-evidence.json",
    )

    constellation_ablation_suite = sub.add_parser(
        "python-constellation-ablation-suite",
        help="TASK-PY-G-097: rerun constellation ablations with #388 tensors and #389 per-slot calibration/OOD state.",
    )
    add_python_constellation_ablation_suite_arguments(constellation_ablation_suite)
    constellation_ablation_suite.set_defaults(
        func=command_python_constellation_ablation_suite,
        evidence_name="python-constellation-ablation-suite-command-evidence.json",
    )

    constellation_ablation_uncalibrated = sub.add_parser(
        "python-constellation-ablation-uncalibrated",
        help="TASK-PY-G-093: rerun uncalibrated #396 constellation ablations against real aiwonder arrays.",
    )
    add_python_constellation_ablation_uncalibrated_arguments(constellation_ablation_uncalibrated)
    constellation_ablation_uncalibrated.set_defaults(
        func=command_python_constellation_ablation_uncalibrated,
        evidence_name="python-constellation-ablation-uncalibrated-command-evidence.json",
    )

    constellation_tensors = sub.add_parser(
        "python-constellation-tensors",
        help="TASK-PY-G-088: materialize slot-preserving cross-embedder correlation tensors from real aiwonder forwards.",
    )
    add_python_constellation_tensors_arguments(constellation_tensors)
    constellation_tensors.set_defaults(
        func=command_python_constellation_tensors,
        evidence_name="python-constellation-tensors-command-evidence.json",
    )

    per_slot_calibration_ood = sub.add_parser(
        "python-per-slot-calibration-ood",
        help="TASK-PY-G-089: calibrate per-slot uncertainty and fail-closed OOD guard from #388/#396 artifacts.",
    )
    add_python_per_slot_calibration_ood_arguments(per_slot_calibration_ood)
    per_slot_calibration_ood.set_defaults(
        func=command_python_per_slot_calibration_ood,
        evidence_name="python-per-slot-calibration-ood-command-evidence.json",
    )

    per_cell_smooth_ece = sub.add_parser(
        "python-per-cell-smooth-ece",
        help="TASK-PY-G-121: report per-cell SmoothECE and active/abstain coverage for the current ship-gate surface.",
    )
    add_python_per_cell_smooth_ece_arguments(per_cell_smooth_ece)
    per_cell_smooth_ece.set_defaults(
        func=command_python_per_cell_smooth_ece,
        evidence_name="python-per-cell-smooth-ece-command-evidence.json",
    )

    calibration_active_gate = sub.add_parser(
        "python-calibration-active-gate",
        help="TASK-PY-G-122: fit calibration-only active/abstain policy and report holdout gate impact.",
    )
    add_python_calibration_active_gate_arguments(calibration_active_gate)
    calibration_active_gate.set_defaults(
        func=command_python_calibration_active_gate,
        evidence_name="python-calibration-active-gate-command-evidence.json",
    )

    tracin_abstain_pool = sub.add_parser(
        "python-tracin-abstain-pool",
        help="TASK-PY-G-123: retained projected-gradient influence audit for abstain and weak-cell rows.",
    )
    add_python_tracin_abstain_pool_arguments(tracin_abstain_pool)
    tracin_abstain_pool.set_defaults(
        func=command_python_tracin_abstain_pool,
        evidence_name="python-tracin-abstain-pool-command-evidence.json",
    )

    exact_tracin_gradient_rows = sub.add_parser(
        "python-exact-tracin-gradient-rows",
        help="TASK-PY-G-126: materialize exact projected row-gradient artifacts for abstain-pool falsification.",
    )
    add_python_exact_tracin_gradient_rows_arguments(exact_tracin_gradient_rows)
    exact_tracin_gradient_rows.set_defaults(
        func=command_python_exact_tracin_gradient_rows,
        evidence_name="python-exact-tracin-gradient-rows-command-evidence.json",
    )

    pair_feature_audit = sub.add_parser(
        "python-pair-feature-audit",
        help="TASK-PY-G-124: audit existing cross-embedder pair evidence and calibration-selected top-K candidates.",
    )
    add_python_pair_feature_audit_arguments(pair_feature_audit)
    pair_feature_audit.set_defaults(
        func=command_python_pair_feature_audit,
        evidence_name="python-pair-feature-audit-command-evidence.json",
    )

    pair_feature_gate_probe = sub.add_parser(
        "python-pair-feature-gate-probe",
        help="TASK-PY-G-124: calibration-only gate probe for selected cross-embedder pair features.",
    )
    add_python_pair_feature_gate_probe_arguments(pair_feature_gate_probe)
    pair_feature_gate_probe.set_defaults(
        func=command_python_pair_feature_gate_probe,
        evidence_name="python-pair-feature-gate-probe-command-evidence.json",
    )

    pair_surface_mining = sub.add_parser(
        "python-pair-surface-mining",
        help="TASK-PY-G-124B: mine the declared cross-embedder pair surface for conditional consequence labels.",
    )
    add_python_pair_surface_mining_arguments(pair_surface_mining)
    pair_surface_mining.set_defaults(
        func=command_python_pair_surface_mining,
        evidence_name="python-pair-surface-mining-command-evidence.json",
    )

    pair_conditional_admission_probe = sub.add_parser(
        "python-pair-conditional-admission-probe",
        help="TASK-PY-G-124B: calibration-only active/abstain routing probe for selected pair motifs.",
    )
    add_python_pair_conditional_admission_probe_arguments(pair_conditional_admission_probe)
    pair_conditional_admission_probe.set_defaults(
        func=command_python_pair_conditional_admission_probe,
        evidence_name="python-pair-conditional-admission-probe-command-evidence.json",
    )

    pair_interaction_mining = sub.add_parser(
        "python-pair-interaction-mining",
        help="TASK-PY-G-124B: mine pair-pair interaction motifs without widening the predictor.",
    )
    add_python_pair_interaction_mining_arguments(pair_interaction_mining)
    pair_interaction_mining.set_defaults(
        func=command_python_pair_interaction_mining,
        evidence_name="python-pair-interaction-mining-command-evidence.json",
    )

    mendeleev_pair_table = sub.add_parser(
        "python-mendeleev-pair-table",
        help="TASK-PY-G-124D: build the head x pair structural-hole consumption table before pair wiring.",
    )
    add_python_mendeleev_pair_table_arguments(mendeleev_pair_table)
    mendeleev_pair_table.set_defaults(
        func=command_python_mendeleev_pair_table,
        evidence_name="python-mendeleev-pair-table-command-evidence.json",
    )

    axis_rotation_diagnostic = sub.add_parser(
        "python-axis-rotation-diagnostic",
        help="TASK-PY-G-126: per-cell feature-axis rotation diagnostic for calibration mismatch.",
    )
    add_python_axis_rotation_diagnostic_arguments(axis_rotation_diagnostic)
    axis_rotation_diagnostic.set_defaults(
        func=command_python_axis_rotation_diagnostic,
        evidence_name="python-axis-rotation-diagnostic-command-evidence.json",
    )

    per_cell_calibration_substrate = sub.add_parser(
        "python-per-cell-calibration-substrate",
        help="TASK-PY-G-124E: per-cell calibration/shrinkage substrate for named-pair retests.",
    )
    add_python_per_cell_calibration_substrate_arguments(per_cell_calibration_substrate)
    per_cell_calibration_substrate.set_defaults(
        func=command_python_per_cell_calibration_substrate,
        evidence_name="python-per-cell-calibration-substrate-command-evidence.json",
    )

    named_pair_selector_retest = sub.add_parser(
        "python-named-pair-selector-retest",
        help="TASK-PY-G-124C: diagnostic retest for #586-eligible named-pair structural holes.",
    )
    add_python_named_pair_selector_retest_arguments(named_pair_selector_retest)
    named_pair_selector_retest.set_defaults(
        func=command_python_named_pair_selector_retest,
        evidence_name="python-named-pair-selector-retest-command-evidence.json",
    )

    foundationality_pressure_probe = sub.add_parser(
        "python-foundationality-pressure-probe",
        help="TASK-PY-G-125: calibration-only probe for foundationality scalar pressure against #392 rows.",
    )
    add_python_foundationality_pressure_probe_arguments(foundationality_pressure_probe)
    foundationality_pressure_probe.set_defaults(
        func=command_python_foundationality_pressure_probe,
        evidence_name="python-foundationality-pressure-probe-command-evidence.json",
    )

    anti_compensation = sub.add_parser(
        "python-anti-compensation-fsv",
        help="TASK-PY-G-094: prove slot/pair/temporal failures cannot be offset by aggregate flat-vector evidence.",
    )
    add_python_anti_compensation_fsv_arguments(anti_compensation)
    anti_compensation.set_defaults(
        func=command_python_anti_compensation_fsv,
        evidence_name="python-anti-compensation-fsv-command-evidence.json",
    )

    auto_label_discovery = sub.add_parser(
        "python-auto-label-discovery",
        help="TASK-PY-G-117: deterministic candidate labels, code-state constellations, and usefulness scoring.",
    )
    add_python_auto_label_discovery_arguments(auto_label_discovery)
    auto_label_discovery.set_defaults(
        func=command_python_auto_label_discovery,
        evidence_name="python-auto-label-discovery-command-evidence.json",
    )

    panel_b_artifacts = sub.add_parser(
        "panel-b-artifacts",
        help="TASK-PY-G-111D: materialize pinned model-backed Panel B artifacts on aiwonder.",
    )
    add_panel_b_artifacts_arguments(panel_b_artifacts)
    panel_b_artifacts.set_defaults(
        func=command_panel_b_artifacts,
        evidence_name="panel-b-artifacts-command-evidence.json",
    )

    panel_b_model_scores = sub.add_parser(
        "panel-b-model-scores",
        help="TASK-PY-G-111D: score current holdout rows with local-only model-backed Panel B artifacts.",
    )
    add_panel_b_model_scores_arguments(panel_b_model_scores)
    panel_b_model_scores.set_defaults(
        func=command_panel_b_model_scores,
        evidence_name="panel-b-model-scores-command-evidence.json",
    )

    panel_b_score_rows = sub.add_parser(
        "panel-b-score-rows",
        help="TASK-PY-G-111D: build strict CG_PANEL_B_SCORE_ROWS_JSONL from real Panel B scores.",
    )
    add_panel_b_score_rows_arguments(panel_b_score_rows)
    panel_b_score_rows.set_defaults(
        func=command_panel_b_score_rows,
        evidence_name="panel-b-score-rows-command-evidence.json",
    )

    panel_b_parity_audit = sub.add_parser(
        "panel-b-parity-audit",
        help="TASK-PY-G-111F: audit Panel B scorer parity gaps against the #392 consequence stack.",
    )
    add_panel_b_parity_audit_arguments(panel_b_parity_audit)
    panel_b_parity_audit.set_defaults(
        func=command_panel_b_parity_audit,
        evidence_name="panel-b-parity-audit-command-evidence.json",
    )

    panel_b_parity_score = sub.add_parser(
        "panel-b-parity-score",
        help="TASK-PY-G-111F: fit split-legal Panel B parity scorer over live-safe consequence features.",
    )
    add_panel_b_parity_score_arguments(panel_b_parity_score)
    panel_b_parity_score.set_defaults(
        func=command_panel_b_parity_score,
        evidence_name="panel-b-parity-score-command-evidence.json",
    )

    registry = sub.add_parser("instrument-registry", help="Register the frozen Python ME-JEPA-Code target-side instrument panel.")
    registry.add_argument("--models-config", type=Path, default=Path("/zfs/hot/contextgraph/models/mejepa_models_config.toml"))
    registry.add_argument("--embedders-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json"))
    registry.add_argument("--forward-fsv", type=Path, default=Path("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-forward-fsv/forward-wrapper-evidence.json"))
    registry.add_argument("--oracle-event-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-oracle-events-v1/oracle_event_manifest.json"))
    registry.add_argument("--ast-parse-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-parse-v1/python_ast_parse_manifest.json"))
    registry.add_argument("--ast-chunk-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunk_manifest.json"))
    registry.add_argument("--diff-chunk-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-diff-chunk-map-v1/diff_chunk_map_manifest.json"))
    registry.add_argument("--static-graph-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-static-reality-graph-v1/python_static_graph_manifest.json"))
    registry.add_argument("--test-intent-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-test-intent-v1/python_test_intent_manifest.json"))
    registry.add_argument("--registry-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-frozen-instrument-registry-v1"))
    registry.add_argument("--fsv-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-014-instrument-registry-fsv"))
    registry.add_argument("--evidence-root", type=Path, default=Path("/zfs/archive/contextgraph/fsv/task-py-data-014-instrument-registry-fsv"))
    registry.add_argument("--hot-root", type=Path, default=Path("/home/cabdru/.cache/contextgraph/task-py-data-014-instrument-registry"))
    registry.add_argument("--hot-budget-bytes", type=int, default=1_073_741_824)
    registry.set_defaults(func=command_instrument_registry, evidence_name="instrument-registry-command-evidence.json")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    evidence_root = getattr(args, "evidence_root", None)
    evidence_name = getattr(args, "evidence_name", None)
    try:
        payload = args.func(args)
        if isinstance(payload, int):
            return payload
        if evidence_root is None or evidence_name is None:
            raise SwebenchOpsError(
                "SWEBENCH_OPS_EVIDENCE_TARGET_MISSING",
                f"command {args.command} returned payload without evidence_root/evidence_name",
                "Add evidence arguments for this command or return a process status integer.",
            )
        evidence_path = evidence_root / evidence_name
        payload["evidence_path"] = str(evidence_path)
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    except SwebenchOpsError as exc:
        if evidence_root is None or evidence_name is None:
            print(json.dumps({"checked_at": utc_now(), "command": args.command, **exc.to_json()}, indent=2, sort_keys=True), file=sys.stderr)
            return 2
        evidence_path = evidence_root / evidence_name
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path),
            **exc.to_json(),
        }
        write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 2
    except Exception as exc:
        evidence_path = (evidence_root / evidence_name) if evidence_root is not None and evidence_name is not None else None
        payload = {
            "checked_at": utc_now(),
            "command": args.command,
            "evidence_path": str(evidence_path) if evidence_path is not None else None,
            "ok": False,
            "error_code": "SWEBENCH_UNEXPECTED_ERROR",
            "message": str(exc),
            "remediation": "Fix the script defect or unexpected environment issue before continuing.",
            "details": {"type": type(exc).__name__},
        }
        if evidence_path is not None:
            write_json_checked(evidence_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 3
