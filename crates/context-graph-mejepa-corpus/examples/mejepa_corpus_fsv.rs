// Manual Full State Verification for the eight Phase-0 mutation operators and
// the train/calibration/holdout split helper.
//
// Sources of truth:
//   - /tmp/contextgraph-mejepa-corpus-fsv/generated_sources/*.py
//   - /tmp/contextgraph-mejepa-corpus-fsv/pyc/*.pyc
//   - /tmp/contextgraph-mejepa-corpus-fsv/split-assignments.csv
//   - /tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json
//
// The example fails closed. It asserts every invariant before writing
// evidence, fsyncs and read-checks each generated file, runs Python syntax
// checks through `py_compile`, and stores before/after state for boundary
// cases. CompileError is the only case expected to fail Python compilation.

use context_graph_mejepa_corpus::{
    apply_mutation,
    split::{
        cross_cut_weights, split_corpus, split_is_leakage_free, CorpusEntry, SplitConfig,
        SplitReport,
    },
    MutationCategory, MutationConfig, MutationOutcome, MutationSite,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
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
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root)?;
    }
    let generated_root = evidence_root.join("generated_sources");
    let pyc_root = evidence_root.join("pyc");
    fs::create_dir_all(&generated_root)?;
    fs::create_dir_all(&pyc_root)?;

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
            "split_assignments_csv": evidence_root.join("split-assignments.csv").display().to_string(),
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
    let corpus = build_split_synthetic_corpus();
    let total = corpus.len();
    let report = split_corpus(&corpus, SplitConfig::default()).map_err(|err| {
        format!("split_corpus(default 80/10/10 over {total} synthetic tasks) failed: {err:?}")
    })?;

    require_eq("split assignment count", &total, &report.assignments.len())?;
    let task_ids_in: HashSet<&str> = corpus.iter().map(|entry| entry.task_id.as_str()).collect();
    let task_ids_out: HashSet<&str> = report
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
        &report.bucket_sizes.iter().sum::<usize>(),
    )?;
    require(
        report.bucket_sizes.iter().all(|size| *size > 0),
        "split must produce non-empty train/calibration/holdout buckets",
    )?;
    require(report.leakage_check_passed, "split leakage_check_passed");
    require(
        split_is_leakage_free(&report),
        "split_is_leakage_free must confirm zero cross-bucket repo edges",
    )?;
    let cross_weights = cross_cut_weights(&report);
    require_eq("split cross-cut weights", &[0.0, 0.0, 0.0], &cross_weights)?;

    let csv_path = evidence_root.join("split-assignments.csv");
    let csv_text = split_assignments_csv(&report);
    let csv_sha256 = write_text_checked(&csv_path, &csv_text)?;
    let csv_readback = read_split_assignments_csv(&csv_path, &report)?;

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
    let single_repo_case = split_boundary_case(
        "single_repo_degenerate_split",
        &single_repo,
        SplitConfig::default(),
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

    Ok(json!({
        "source_of_truth": {
            "csv_path": csv_path.display().to_string(),
            "csv_sha256": csv_sha256,
            "csv_readback": csv_readback,
        },
        "corpus_size": total,
        "repo_count": report.repo_assignments.len(),
        "bucket_sizes": report.bucket_sizes,
        "bucket_fractions": report.bucket_fractions,
        "cross_cut_weights": cross_weights,
        "leakage_check_passed": report.leakage_check_passed,
        "edge_cases": {
            "empty_corpus": empty_case,
            "single_repo_degenerate_split": single_repo_case,
            "fraction_sum_not_one": bad_fraction_case,
            "duplicate_task_id": duplicate_task_case,
            "whitespace_repo_id": whitespace_repo_case,
        },
        "passes": true,
    }))
}

fn build_split_synthetic_corpus() -> Vec<CorpusEntry> {
    let plan: &[(&str, usize)] = &[
        ("astropy", 22),
        ("django", 40),
        ("flask", 4),
        ("matplotlib", 18),
        ("mwaskom__seaborn", 6),
        ("pallets__click", 3),
        ("psf__requests", 6),
        ("pvlib__pvlib-python", 4),
        ("pydata__xarray", 12),
        ("pydicom__pydicom", 5),
        ("pylint-dev__astroid", 5),
        ("pylint-dev__pylint", 13),
        ("pyvista__pyvista", 4),
        ("scikit-learn__scikit-learn", 28),
        ("sphinx-doc__sphinx", 19),
        ("sqlfluff__sqlfluff", 12),
        ("sympy__sympy", 75),
        ("hpcugent__easybuild-easyconfigs", 4),
        ("conan-io__conan", 5),
        ("scrapy", 5),
        ("requests-toolbelt", 3),
        ("pytest-dev__pytest", 12),
        ("celery", 4),
        ("paramiko", 3),
        ("redis-py", 8),
    ];
    let total: usize = plan.iter().map(|(_, count)| count).sum();
    assert_eq!(total, 300);
    let mut entries = Vec::with_capacity(total);
    for (repo, count) in plan {
        for idx in 0..*count {
            entries.push(CorpusEntry {
                task_id: format!("{repo}__task-{idx:04}"),
                repo: (*repo).to_string(),
            });
        }
    }
    entries
}

fn split_assignments_csv(report: &SplitReport) -> String {
    let mut out = String::from("task_id,repo,bucket\n");
    for assignment in &report.assignments {
        out.push_str(&format!(
            "{},{},{}\n",
            assignment.task_id,
            assignment.repo,
            assignment.bucket.slug()
        ));
    }
    out
}

fn read_split_assignments_csv(path: &Path, expected: &SplitReport) -> FsvResult<Value> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    require_eq(
        "split CSV header",
        &Some("task_id,repo,bucket"),
        &lines.next(),
    )?;
    let rows: Vec<&str> = lines.collect();
    require_eq(
        "split CSV row count",
        &expected.assignments.len(),
        &rows.len(),
    )?;
    let mut task_ids = HashSet::with_capacity(rows.len());
    for (row, expected_assignment) in rows.iter().zip(&expected.assignments) {
        let parts: Vec<&str> = row.split(',').collect();
        require_eq(
            &format!("split CSV row shape: {row}"),
            &3usize,
            &parts.len(),
        )?;
        require_eq(
            &format!("split CSV task_id: {row}"),
            &expected_assignment.task_id.as_str(),
            &parts[0],
        )?;
        require_eq(
            &format!("split CSV repo: {row}"),
            &expected_assignment.repo.as_str(),
            &parts[1],
        )?;
        require_eq(
            &format!("split CSV bucket: {row}"),
            &expected_assignment.bucket.slug(),
            &parts[2],
        )?;
        require(
            task_ids.insert(parts[0]),
            "split CSV task IDs must be unique",
        );
    }
    Ok(json!({
        "row_count": rows.len(),
        "unique_task_ids": task_ids.len(),
        "byte_length": text.len(),
    }))
}

fn split_boundary_case(
    name: &str,
    entries: &[CorpusEntry],
    config: SplitConfig,
    expected_code: &str,
) -> FsvResult<Value> {
    let repos: HashSet<&str> = entries.iter().map(|entry| entry.repo.as_str()).collect();
    let before = json!({
        "name": name,
        "entries": entries.len(),
        "repos": repos.len(),
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
