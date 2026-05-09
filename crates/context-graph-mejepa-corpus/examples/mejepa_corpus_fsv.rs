// Manual Full State Verification for the eight Phase-0 mutation operators and
// the train/calibration/holdout split helper.
//
// Sources of truth:
//   - /tmp/contextgraph-mejepa-corpus-fsv/generated_sources/*.py
//   - /tmp/contextgraph-mejepa-corpus-fsv/pyc/*.pyc
//   - /tmp/contextgraph-mejepa-corpus-fsv/split-assignments.json
//   - /tmp/contextgraph-mejepa-corpus-fsv/split-repo-atomic-assignments.json
//   - /tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json
//
// The example fails closed. It asserts every invariant before writing
// evidence, fsyncs and read-checks each generated file, runs Python syntax
// checks through `py_compile`, and stores before/after state for boundary
// cases. CompileError is the only case expected to fail Python compilation.

use context_graph_mejepa_corpus::{
    apply_mutation,
    split::{
        assert_no_test_patch_leakage, cross_cut_weights, split_corpus, split_is_leakage_free,
        CorpusEntry, SplitConfig, SplitMode, TaskAssignment,
    },
    MutationCategory, MutationConfig, MutationOutcome, MutationSite,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn run(
    label: &str,
    category: MutationCategory,
    source: &str,
    config: MutationConfig,
) -> FsvResult<MutationOutcome> {
    apply_mutation(category, source, config)
        .map_err(|err| format!("{label} apply_mutation({category:?}) failed: {err}").into())
}

fn required_site(label: &str, outcome: &MutationOutcome) -> FsvResult<MutationSite> {
    outcome
        .mutation_site
        .clone()
        .ok_or_else(|| format!("{label} must produce a mutation_site").into())
}

fn main() -> FsvResult<()> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv");
    let generated_root = evidence_root.join("generated_sources");
    let pyc_root = evidence_root.join("pyc");
    if generated_root.exists() {
        fs::remove_dir_all(&generated_root)?;
    }
    if pyc_root.exists() {
        fs::remove_dir_all(&pyc_root)?;
    }
    fs::create_dir_all(&generated_root)?;
    fs::create_dir_all(&pyc_root)?;
    fs::set_permissions(&evidence_root, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&generated_root, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&pyc_root, fs::Permissions::from_mode(0o700))?;

    let py_source = "\
import math


def is_eligible(age: int, score: float, has_consent: bool) -> bool:
    threshold = 60
    bonus = 5
    weighted = score + bonus
    if age >= 18 and weighted > threshold:
        assert score >= 0.0
        return True
    return False
";

    let alternate = "\
def alt_helper(name: str) -> int:
    return len(name)
";

    let known_good = run(
        "KnownGood",
        MutationCategory::KnownGood,
        py_source,
        MutationConfig::default(),
    )?;
    let kg_passes = known_good.mutated_source == py_source && known_good.mutation_site.is_none();
    require(
        kg_passes,
        "KnownGood must byte-echo input and report no site",
    )?;

    let subtle = run(
        "SubtleFlip",
        MutationCategory::SubtleFlip,
        py_source,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    )?;
    let subtle_site = required_site("SubtleFlip", &subtle)?;
    let subtle_passes = subtle.mutated_source != py_source
        && subtle_site.replacement_text != subtle_site.original_text;
    require(
        subtle_passes,
        "SubtleFlip must change source at a reported site",
    )?;
    let subtle_excerpt = site_excerpt(&subtle.mutated_source, &subtle_site);

    let off = run(
        "OffByOne",
        MutationCategory::OffByOne,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )?;
    let off_site = required_site("OffByOne", &off)?;
    let off_orig: i64 = off_site.original_text.parse()?;
    let off_new: i64 = off_site.replacement_text.parse()?;
    let off_passes = (off_new - off_orig).abs() == 1;
    require(
        off_passes,
        "OffByOne replacement must differ by exactly one",
    )?;

    let swap = run(
        "SwapVariable",
        MutationCategory::SwapVariable,
        py_source,
        MutationConfig {
            seed: 11,
            alternate_source: None,
        },
    )?;
    let swap_site = required_site("SwapVariable", &swap)?;
    let swap_passes =
        swap_site.original_text != swap_site.replacement_text && swap.mutated_source != py_source;
    require(swap_passes, "SwapVariable must replace one variable use")?;

    let delete_call = run(
        "DeleteTestCall",
        MutationCategory::DeleteTestCall,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )?;
    let delete_site = required_site("DeleteTestCall", &delete_call)?;
    let delete_passes = delete_site.replacement_text.trim() == "pass"
        && delete_call.mutated_source.len()
            == py_source.len() - delete_site.byte_length + delete_site.replacement_text.len();
    require(
        delete_passes,
        "DeleteTestCall must remove assertion bytes and insert syntax-preserving pass",
    )?;

    let wrong = run(
        "WrongFile",
        MutationCategory::WrongFile,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: Some(alternate.to_string()),
        },
    )?;
    let wrong_passes = wrong.mutated_source == alternate && wrong.mutation_site.is_none();
    require(
        wrong_passes,
        "WrongFile must replace primary source with alternate source",
    )?;

    let over = run(
        "OverEngineer",
        MutationCategory::OverEngineer,
        py_source,
        MutationConfig {
            seed: 7,
            alternate_source: None,
        },
    )?;
    let over_site = required_site("OverEngineer", &over)?;
    let over_suffix = over_site
        .replacement_text
        .split("def _unused_helper_")
        .nth(1)
        .and_then(|tail| tail.split("()").next())
        .ok_or("OverEngineer helper suffix missing")?;
    let over_passes = over.mutated_source.contains("def _unused_helper_")
        && over
            .mutated_source
            .starts_with(py_source.trim_end_matches('\n'))
        && over_suffix.len() == 12
        && over_suffix.chars().all(|ch| ch.is_ascii_hexdigit());
    require(
        over_passes,
        "OverEngineer must preserve prefix and append a 48-bit helper id",
    )?;

    let cerr = run(
        "CompileError",
        MutationCategory::CompileError,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )?;
    let cerr_site = required_site("CompileError", &cerr)?;
    let cerr_passes = cerr_site.byte_length == 0
        && cerr.mutated_source.len() == py_source.len() + cerr_site.replacement_text.len()
        && (cerr_site.replacement_text.contains("unclosed paren")
            || cerr_site.replacement_text.contains("missing colon")
            || cerr_site.replacement_text.contains("invalid def signature"));
    require(
        cerr_passes,
        "CompileError must append one intentional syntax-error variant",
    )?;

    let empty_case = boundary_case(
        "empty_input",
        MutationCategory::KnownGood,
        "",
        MutationConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let no_alt_case = boundary_case(
        "wrong_file_without_alternate",
        MutationCategory::WrongFile,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let no_bool_case = boundary_case(
        "subtle_flip_no_bool_op",
        MutationCategory::SubtleFlip,
        "VERSION = \"1.0\"\n\ndef noop():\n    pass\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let no_int_case = boundary_case(
        "off_by_one_floats_only",
        MutationCategory::OffByOne,
        "PI = 3.14\nE = 2.718\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let no_assert_case = boundary_case(
        "delete_test_call_no_assertions",
        MutationCategory::DeleteTestCall,
        "def noop():\n    return None\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let docstring_bool_case = boundary_case(
        "subtle_flip_docstring_only",
        MutationCategory::SubtleFlip,
        "def noop():\n    \"\"\"x == y and y > z\"\"\"\n    return None\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let docstring_int_case = boundary_case(
        "off_by_one_docstring_only",
        MutationCategory::OffByOne,
        "def noop():\n    '''version 123'''\n    return None\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;

    let source_checks = json!({
        "known_good": persist_python_source(&generated_root, &pyc_root, "known_good", &known_good.mutated_source, true)?,
        "subtle_flip": persist_python_source(&generated_root, &pyc_root, "subtle_flip", &subtle.mutated_source, true)?,
        "off_by_one": persist_python_source(&generated_root, &pyc_root, "off_by_one", &off.mutated_source, true)?,
        "swap_variable": persist_python_source(&generated_root, &pyc_root, "swap_variable", &swap.mutated_source, true)?,
        "delete_test_call": persist_python_source(&generated_root, &pyc_root, "delete_test_call", &delete_call.mutated_source, true)?,
        "wrong_file": persist_python_source(&generated_root, &pyc_root, "wrong_file", &wrong.mutated_source, true)?,
        "over_engineer": persist_python_source(&generated_root, &pyc_root, "over_engineer", &over.mutated_source, true)?,
        "compile_error": persist_python_source(&generated_root, &pyc_root, "compile_error", &cerr.mutated_source, false)?,
    });
    let split_check = split_fsv(&evidence_root)?;

    let evidence = json!({
        "source_of_truth": {
            "evidence_root": evidence_root.display().to_string(),
            "evidence_json": evidence_root.join("mejepa-corpus-evidence.json").display().to_string(),
            "generated_python_sources": generated_root.display().to_string(),
            "generated_python_bytecode": pyc_root.display().to_string(),
            "split_assignments_json": evidence_root.join("split-assignments.json").display().to_string(),
            "generated_source_checks": source_checks,
        },
        "input_source_sha256": sha256_text(py_source),
        "input_byte_length": py_source.len(),
        "case_known_good": {
            "mutated_source": known_good.mutated_source,
            "site": known_good.mutation_site,
            "passes": kg_passes,
        },
        "case_subtle_flip": {
            "site": subtle_site,
            "mutated_excerpt_around_site": subtle_excerpt,
            "passes": subtle_passes,
        },
        "case_off_by_one": {
            "site": off_site,
            "original_int": off_orig,
            "replacement_int": off_new,
            "passes": off_passes,
        },
        "case_swap_variable": {
            "site": swap_site,
            "passes": swap_passes,
        },
        "case_delete_test_call": {
            "site": delete_site,
            "input_length": py_source.len(),
            "output_length": delete_call.mutated_source.len(),
            "passes": delete_passes,
        },
        "case_wrong_file": {
            "mutated_source": wrong.mutated_source,
            "primary_source_length": py_source.len(),
            "alternate_source_length": alternate.len(),
            "passes": wrong_passes,
        },
        "case_over_engineer": {
            "site": over_site,
            "appended_length": over.mutated_source.len() - py_source.len(),
            "passes": over_passes,
        },
        "case_compile_error": {
            "site": cerr_site,
            "passes": cerr_passes,
        },
        "edge_cases": {
            "boundary_empty_input_rejected": empty_case,
            "boundary_wrong_file_without_alternate_rejected": no_alt_case,
            "boundary_subtle_flip_no_bool_op_rejected": no_bool_case,
            "boundary_off_by_one_floats_only_rejected": no_int_case,
            "boundary_delete_test_call_no_assertions_rejected": no_assert_case,
            "boundary_subtle_flip_docstring_only_rejected": docstring_bool_case,
            "boundary_off_by_one_docstring_only_rejected": docstring_int_case,
        },
        "split_fsv": split_check,
    });

    let path = evidence_root.join("mejepa-corpus-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    let evidence_sha256 = write_text_checked(&path, &text)?;

    let readback = fs::read_to_string(&path)?;
    let parsed: Value = serde_json::from_str(&readback)?;
    require_eq("evidence_readback", &evidence, &parsed)?;
    println!("ME-JEPA mutation-corpus FSV evidence: {}", path.display());
    println!("Evidence SHA-256: {evidence_sha256}");
    println!("Read-back size: {} bytes", readback.len());
    println!("{}", text);
    Ok(())
}

fn split_fsv(evidence_root: &Path) -> FsvResult<Value> {
    let (corpus, tasks_dir, test_patch_sha_by_task) = build_split_real_corpus()?;
    let total = corpus.len();
    let split_config_repo_atomic = SplitConfig {
        mode: SplitMode::RepoAtomic,
        ..SplitConfig::default()
    };
    let repo_report = split_corpus(&corpus, split_config_repo_atomic).map_err(|err| {
        format!("split_corpus(default 80/10/10 over {total} real task manifests) failed: {err:?}")
    })?;

    require_eq(
        "repo-atomic split assignment count",
        &total,
        &repo_report.assignments.len(),
    )?;
    let task_ids_in: HashSet<&str> = corpus.iter().map(|entry| entry.task_id.as_str()).collect();
    let task_ids_out: HashSet<&str> = repo_report
        .assignments
        .iter()
        .map(|assignment| assignment.task_id.as_str())
        .collect();
    require_eq("split input task IDs unique", &total, &task_ids_in.len())?;
    require_eq(
        "split assignments cover exactly the input task IDs",
        &task_ids_in,
        &task_ids_out,
    )?;
    require_eq(
        "split bucket sizes sum",
        &total,
        &repo_report.bucket_sizes.iter().sum::<usize>(),
    )?;
    require(
        repo_report.bucket_sizes.iter().all(|size| *size > 0),
        "split must produce non-empty train/calibration/holdout buckets",
    )?;
    require(
        repo_report.leakage_check_passed,
        "repo-atomic split leakage_check_passed",
    )?;
    require(
        split_is_leakage_free(&repo_report),
        "split_is_leakage_free must confirm zero cross-bucket repo edges",
    )?;
    let cross_weights = cross_cut_weights(&repo_report);
    require_eq("split cross-cut weights", &[0.0, 0.0, 0.0], &cross_weights)?;

    let repo_assignments_path = evidence_root.join("split-repo-atomic-assignments.json");
    let repo_assignments_text = serde_json::to_string_pretty(&repo_report.assignments)?;
    let repo_assignments_sha256 =
        write_text_checked(&repo_assignments_path, &repo_assignments_text)?;
    let repo_assignments_readback_text = fs::read_to_string(&repo_assignments_path)?;
    let repo_assignments_readback: Vec<TaskAssignment> =
        serde_json::from_str(&repo_assignments_readback_text)?;
    require_eq(
        "repo-atomic assignments JSON readback",
        &repo_report.assignments,
        &repo_assignments_readback,
    )?;

    let instance_config = SplitConfig {
        mode: SplitMode::InstanceAtomic,
        ..SplitConfig::default()
    };
    let instance_report = split_corpus(&corpus, instance_config).map_err(|err| {
        format!("instance-atomic split over {total} real task manifests failed: {err:?}")
    })?;
    require_eq(
        "instance-atomic bucket sizes",
        &[240, 30, 30],
        &instance_report.bucket_sizes,
    )?;
    require(
        instance_report.leakage_check_passed,
        "instance-atomic split leakage_check_passed",
    )?;
    assert_no_test_patch_leakage(&instance_report.assignments, |task_id| {
        test_patch_sha_by_task.get(task_id).map(String::as_str)
    })
    .map_err(|err| -> Box<dyn std::error::Error> {
        format!("instance-atomic official_test_patch_sha256 leakage check failed: {err}").into()
    })?;

    let assignments_path = evidence_root.join("split-assignments.json");
    let assignments_text = serde_json::to_string_pretty(&instance_report.assignments)?;
    let assignments_sha256 = write_text_checked(&assignments_path, &assignments_text)?;
    let assignments_readback_text = fs::read_to_string(&assignments_path)?;
    let assignments_readback: Vec<TaskAssignment> =
        serde_json::from_str(&assignments_readback_text)?;
    require_eq(
        "instance-atomic assignments JSON readback",
        &instance_report.assignments,
        &assignments_readback,
    )?;
    let assignments_unique_task_ids: HashSet<&str> = assignments_readback
        .iter()
        .map(|assignment| assignment.task_id.as_str())
        .collect();

    let empty_case = split_boundary_case(
        "empty_corpus",
        &[],
        SplitConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let single_repo: Vec<CorpusEntry> = (0..50)
        .map(|idx| CorpusEntry {
            task_id: format!("solo__task-{idx:04}"),
            repo: "solo".to_string(),
        })
        .collect();
    // The "all 50 tasks share one repo" case is a degenerate-input failure
    // ONLY under repo-atomic, where assigning the whole repo to one bucket
    // leaves the other two empty. Under InstanceAtomic the same input
    // legitimately produces [40,5,5]. Pin RepoAtomic to keep this as a
    // failing boundary case.
    let single_repo_case = split_boundary_case(
        "single_repo_degenerate_split_repo_atomic",
        &single_repo,
        SplitConfig {
            mode: SplitMode::RepoAtomic,
            ..SplitConfig::default()
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let bad_fraction_case = split_boundary_case(
        "fraction_sum_not_one",
        &corpus,
        SplitConfig {
            train_fraction: 0.7,
            calibration_fraction: 0.2,
            holdout_fraction: 0.2,
            seed: 0,
            ..SplitConfig::default()
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let duplicate_task_case = split_boundary_case(
        "duplicate_task_id",
        &[
            CorpusEntry {
                task_id: "dup".to_string(),
                repo: "repo-a".to_string(),
            },
            CorpusEntry {
                task_id: "dup".to_string(),
                repo: "repo-b".to_string(),
            },
            CorpusEntry {
                task_id: "unique".to_string(),
                repo: "repo-c".to_string(),
            },
        ],
        SplitConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let whitespace_repo_case = split_boundary_case(
        "whitespace_repo_id",
        &[
            CorpusEntry {
                task_id: "task-a".to_string(),
                repo: " repo-a".to_string(),
            },
            CorpusEntry {
                task_id: "task-b".to_string(),
                repo: "repo-b".to_string(),
            },
            CorpusEntry {
                task_id: "task-c".to_string(),
                repo: "repo-c".to_string(),
            },
        ],
        SplitConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let missing_test_patch_sha_case = test_patch_leakage_boundary_case(
        "missing_test_patch_sha",
        &instance_report.assignments,
        &BTreeMap::new(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let duplicate_sha_by_task: BTreeMap<String, String> = instance_report
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.task_id.clone(),
                "sha256:duplicate-test-patch".to_string(),
            )
        })
        .collect();
    let duplicate_test_patch_sha_case = test_patch_leakage_boundary_case(
        "duplicate_test_patch_sha_cross_bucket",
        &instance_report.assignments,
        &duplicate_sha_by_task,
        "MEJEPA_CORPUS_OPERATOR_FAILED",
    )?;
    let unique_test_patch_sha_count = test_patch_sha_by_task
        .values()
        .collect::<HashSet<_>>()
        .len();

    Ok(json!({
        "source_of_truth": {
            "assignments_json": assignments_path.display().to_string(),
            "assignments_sha256": assignments_sha256,
            "repo_atomic_assignments_json": repo_assignments_path.display().to_string(),
            "repo_atomic_assignments_sha256": repo_assignments_sha256,
            "assignments_readback": {
                "byte_length": assignments_readback_text.len(),
                "row_count": assignments_readback.len(),
                "unique_task_ids": assignments_unique_task_ids.len(),
            },
            "task_manifest_dir": tasks_dir.display().to_string(),
            "official_test_patch_sha256_source": "tasks/swebench-lite/*.json:official_test_patch_sha256",
        },
        "corpus_size": total,
        "repo_atomic": {
            "mode": "repo_atomic",
            "repo_count": repo_report.repo_assignments.len(),
            "bucket_sizes": repo_report.bucket_sizes,
            "bucket_fractions": repo_report.bucket_fractions,
            "cross_cut_weights": cross_weights,
            "leakage_check_passed": repo_report.leakage_check_passed,
        },
        "instance_atomic": {
            "mode": "instance_atomic",
            "bucket_sizes": instance_report.bucket_sizes,
            "bucket_fractions": instance_report.bucket_fractions,
            "achieves_80_10_10": instance_report.bucket_sizes == [240, 30, 30],
            "leakage_check_passed": instance_report.leakage_check_passed,
            "test_patch_sha_leakage_check_passed": true,
            "official_test_patch_sha256_count": test_patch_sha_by_task.len(),
            "unique_official_test_patch_sha256_count": unique_test_patch_sha_count,
        },
        "edge_cases": {
            "empty_corpus": empty_case,
            "single_repo_degenerate_split_repo_atomic": single_repo_case,
            "fraction_sum_not_one": bad_fraction_case,
            "duplicate_task_id": duplicate_task_case,
            "whitespace_repo_id": whitespace_repo_case,
            "missing_test_patch_sha": missing_test_patch_sha_case,
            "duplicate_test_patch_sha_cross_bucket": duplicate_test_patch_sha_case,
        },
        "passes": true,
    }))
}

fn build_split_real_corpus() -> FsvResult<(Vec<CorpusEntry>, PathBuf, BTreeMap<String, String>)> {
    let tasks_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tasks/swebench-lite")
        .canonicalize()?;
    let mut entries = Vec::new();
    let mut test_patch_sha_by_task = BTreeMap::new();
    for dir_entry in fs::read_dir(&tasks_dir)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let manifest: Value = serde_json::from_str(&text)?;
        let instance_id = manifest
            .get("instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("task manifest missing instance_id: {}", path.display()))?;
        let repo = manifest
            .get("repo")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("task manifest missing repo: {}", path.display()))?;
        let test_patch_sha = manifest
            .get("official_test_patch_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "task manifest missing official_test_patch_sha256: {}",
                    path.display()
                )
            })?;
        require(
            test_patch_sha.starts_with("sha256:"),
            &format!(
                "official_test_patch_sha256 must be prefixed with sha256: in {}",
                path.display()
            ),
        )?;
        entries.push(CorpusEntry {
            task_id: instance_id.to_string(),
            repo: repo.to_string(),
        });
        test_patch_sha_by_task.insert(instance_id.to_string(), test_patch_sha.to_string());
    }
    entries.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    require(
        !entries.is_empty(),
        &format!("no task manifests found under {}", tasks_dir.display()),
    )?;
    require_eq(
        "test_patch_sha_by_task count",
        &entries.len(),
        &test_patch_sha_by_task.len(),
    )?;
    Ok((entries, tasks_dir, test_patch_sha_by_task))
}

fn test_patch_leakage_boundary_case(
    name: &str,
    assignments: &[TaskAssignment],
    sha_by_task: &BTreeMap<String, String>,
    expected_code: &str,
) -> FsvResult<Value> {
    let before_state = json!({
        "name": name,
        "assignment_count": assignments.len(),
        "sha_map_count": sha_by_task.len(),
        "unique_sha_count": sha_by_task.values().collect::<HashSet<_>>().len(),
    });
    let after_state = match assert_no_test_patch_leakage(assignments, |task_id| {
        sha_by_task.get(task_id).map(String::as_str)
    }) {
        Err(error) => {
            require_eq(name, &expected_code, &error.code())?;
            json!({
                "result": "error",
                "code": error.code(),
                "message": error.to_string(),
            })
        }
        Ok(()) => {
            return Err(format!(
                "boundary case {name} unexpectedly passed test-patch leakage check"
            )
            .into());
        }
    };
    Ok(json!({
        "before_state": before_state,
        "after_state": after_state,
        "expected_code": expected_code,
        "actual_code": after_state["code"],
        "passes": true,
    }))
}

fn split_boundary_case(
    name: &str,
    entries: &[CorpusEntry],
    config: SplitConfig,
    expected_code: &str,
) -> FsvResult<Value> {
    let repos: HashSet<&str> = entries.iter().map(|entry| entry.repo.as_str()).collect();
    let entries_json = serde_json::to_string(entries)?;
    let before = json!({
        "name": name,
        "entries": entries.len(),
        "repos": repos.len(),
        "entries_sha256": sha256_text(&entries_json),
        "config": config,
    });
    let after = match split_corpus(entries, config) {
        Err(err) => {
            require_eq(name, &expected_code, &err.code())?;
            json!({
                "result": "error",
                "code": err.code(),
                "message": err.to_string(),
            })
        }
        Ok(report) => {
            return Err(format!(
                "split boundary case {name} unexpectedly succeeded with bucket sizes {:?}",
                report.bucket_sizes
            )
            .into());
        }
    };
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": expected_code,
        "actual_code": after["code"],
        "passes": true,
    }))
}

fn boundary_case(
    name: &str,
    category: MutationCategory,
    source: &str,
    config: MutationConfig,
    expected_code: &str,
) -> FsvResult<Value> {
    let before = json!({
        "name": name,
        "category": category.slug(),
        "primary_source_len": source.len(),
        "primary_source_sha256": sha256_text(source),
        "alternate_source_present": config.alternate_source.is_some(),
    });
    let after = match apply_mutation(category, source, config) {
        Err(e) => {
            require_eq(name, &expected_code, &e.code())?;
            json!({
                "result": "error",
                "code": e.code(),
                "message": e.to_string(),
            })
        }
        Ok(outcome) => {
            return Err(format!(
                "boundary case {name} unexpectedly succeeded with category {:?}",
                outcome.category
            )
            .into());
        }
    };
    Ok(json!({
        "before_state": before,
        "after_state": after,
        "expected_code": expected_code,
        "actual_code": after["code"],
        "passes": true,
    }))
}

fn persist_python_source(
    generated_root: &Path,
    pyc_root: &Path,
    slug: &str,
    source: &str,
    expected_compile_ok: bool,
) -> FsvResult<Value> {
    let path = generated_root.join(format!("{slug}.py"));
    let pyc_path = pyc_root.join(format!("{slug}.pyc"));
    let file_sha256 = write_text_checked(&path, source)?;
    require_eq(
        &format!("{slug} source hash"),
        &sha256_text(source),
        &file_sha256,
    )?;
    let readback = fs::read_to_string(&path)?;
    require_eq(slug, &source, &readback.as_str())?;
    let compile = py_compile(&path, &pyc_path)?;
    let compile_ok = compile["ok"]
        .as_bool()
        .ok_or_else(|| format!("{slug} py_compile missing ok boolean"))?;
    require_eq(
        &format!("{slug} py_compile"),
        &expected_compile_ok,
        &compile_ok,
    )?;
    if expected_compile_ok {
        require(
            compile["pyc_exists"].as_bool() == Some(true),
            &format!("{slug} successful py_compile must produce a pyc file"),
        )?;
    } else {
        require(
            compile["pyc_exists"].as_bool() == Some(false),
            &format!("{slug} failed py_compile must not leave a pyc file"),
        )?;
    }
    Ok(json!({
        "path": path.display().to_string(),
        "byte_length": readback.len(),
        "sha256": file_sha256,
        "expected_compile_ok": expected_compile_ok,
        "py_compile": compile,
    }))
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    let readback = fs::read_to_string(path)?;
    require_eq(
        &format!("readback {}", path.display()),
        &text,
        &readback.as_str(),
    )?;
    Ok(sha256_text(&readback))
}

fn py_compile(path: &Path, pyc_path: &Path) -> FsvResult<Value> {
    if pyc_path.exists() {
        fs::remove_file(pyc_path)?;
    }
    if let Some(parent) = pyc_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let output = Command::new("python3")
        .arg("-c")
        .arg("import py_compile, sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)")
        .arg(path)
        .arg(pyc_path)
        .output()?;
    let pyc_exists = pyc_path.is_file();
    let pyc_sha256 = if pyc_exists {
        Value::String(sha256_file(pyc_path)?)
    } else {
        Value::Null
    };
    Ok(json!({
        "command": format!(
            "python3 -c 'import py_compile, sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)' {} {}",
            path.display(),
            pyc_path.display(),
        ),
        "ok": output.status.success(),
        "status_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "pyc_path": pyc_path.display().to_string(),
        "pyc_exists": pyc_exists,
        "pyc_sha256": pyc_sha256,
    }))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn site_excerpt(source: &str, site: &MutationSite) -> String {
    let mut start = site.byte_offset.saturating_sub(10);
    while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = site
        .byte_offset
        .saturating_add(site.replacement_text.len())
        .saturating_add(10)
        .min(source.len());
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    source[start..end].to_string()
}

fn require(condition: bool, label: &str) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}

fn require_eq<T>(label: &str, expected: &T, actual: &T) -> FsvResult<()>
where
    T: PartialEq + Debug + ?Sized,
{
    if expected == actual {
        Ok(())
    } else {
        Err(
            format!("FSV assertion failed for {label}: expected {expected:?}, actual {actual:?}")
                .into(),
        )
    }
}
