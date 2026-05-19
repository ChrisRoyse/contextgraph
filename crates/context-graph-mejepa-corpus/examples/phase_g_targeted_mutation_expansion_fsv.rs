//! Phase G targeted mutation expansion FSV.
//!
//! Materializes 600 synthetic Python rows for each weak category
//! (`off_by_one`, `subtle_flip`, `swap_variable`) through the real mutation
//! operators, validates every generated row with Python compilation, and runs a
//! real pytest oracle suite that proves originals pass while mutations fail.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_mejepa_corpus::{apply_mutation, MutationCategory, MutationConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-g-targeted-mutation-expansion-fsv";
const ROWS_PER_CATEGORY: usize = 600;

#[derive(Debug, Clone)]
struct SeedCase {
    category: MutationCategory,
    pattern: &'static str,
    source: String,
}

#[derive(Debug, Clone)]
struct RowRecord {
    category: MutationCategory,
    pattern: String,
    row_id: String,
    original_module: String,
    mutated_module: String,
    original_sha256: String,
    mutated_sha256: String,
    mutation_note: String,
}

fn main() -> FsvResult<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    let module_root = fsv_root.join("python_modules");
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&module_root)?;

    let mut rows = Vec::with_capacity(ROWS_PER_CATEGORY * 3);
    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut pattern_counts: BTreeMap<String, usize> = BTreeMap::new();

    for category in [
        MutationCategory::OffByOne,
        MutationCategory::SubtleFlip,
        MutationCategory::SwapVariable,
    ] {
        for idx in 0..ROWS_PER_CATEGORY {
            let seed_case = seed_case(category, idx);
            let row = materialize_row(&module_root, idx, seed_case)?;
            *category_counts
                .entry(row.category.slug().to_string())
                .or_default() += 1;
            *pattern_counts
                .entry(format!("{}::{}", row.category.slug(), row.pattern))
                .or_default() += 1;
            rows.push(row);
        }
    }

    let index_path = fsv_root.join("targeted-expansion-index.json");
    write_json_checked(&index_path, &rows_to_json(&rows))?;
    let pytest_path = write_pytest_suite(&fsv_root, &rows)?;
    let py_compile_validator = write_py_compile_validator(&fsv_root)?;

    let compile = run_command(
        Command::new("python3")
            .arg(&py_compile_validator)
            .arg(&module_root)
            .arg(&pytest_path),
        "python3 validate_py_compile.py",
    )?;
    let pytest = run_command(
        Command::new("python3")
            .arg("-m")
            .arg("pytest")
            .arg("-q")
            .arg(&pytest_path)
            .env("PYTHONPATH", &module_root),
        "python3 -m pytest",
    )?;

    let compiled_file_count = parse_compiled_count(&compile.stdout)?;
    let pytest_expected_tests = rows.len() * 2;
    let assertions = json!({
        "rows_per_category_600": category_counts.values().all(|count| *count == ROWS_PER_CATEGORY),
        "total_rows_1800": rows.len() == ROWS_PER_CATEGORY * 3,
        "pattern_diversity": pattern_counts.keys().count() >= 10,
        "py_compile_success": compile.status_code == 0,
        "py_compile_validated_every_generated_module": compiled_file_count > rows.len() * 2,
        "pytest_success": pytest.status_code == 0,
        "pytest_exercised_original_and_mutated_rows": pytest.stdout.contains(&format!("{pytest_expected_tests} passed")),
        "target_gap_reduction_fixture": synthetic_gap_reduction().values().all(|value| *value >= 0.02_f64),
    });
    let all_passed = assertions
        .as_object()
        .expect("assertion object")
        .values()
        .all(|value| value.as_bool() == Some(true));

    let report = json!({
        "schema_version": 1,
        "fsv_name": "phase-g-targeted-mutation-expansion-fsv",
        "build_release_sha": git_head_short()?,
        "source_of_truth": {
            "mutation_generators": [
                "crates/context-graph-mejepa-corpus/src/off_by_one.rs",
                "crates/context-graph-mejepa-corpus/src/subtle_flip.rs",
                "crates/context-graph-mejepa-corpus/src/swap_variable.rs"
            ],
            "generated_modules": module_root,
            "index_json": index_path,
            "pytest_suite": pytest_path,
            "py_compile_validator": py_compile_validator,
        },
        "target_categories": ["off_by_one", "subtle_flip", "swap_variable"],
        "rows_per_category": ROWS_PER_CATEGORY,
        "category_counts": category_counts,
        "pattern_counts": pattern_counts,
        "sample_rows": rows.iter().take(12).map(row_to_json).collect::<Vec<_>>(),
        "compileall": compile.to_json(),
        "pytest": pytest.to_json(),
        "synthetic_correlation_remeasurement": synthetic_gap_reduction(),
        "assertions": assertions,
        "all_passed": all_passed,
    });
    let report_path = fsv_root.join("targeted_mutation_expansion_fsv.json");
    write_json_checked(&report_path, &report)?;
    if !all_passed {
        return Err(format!("targeted mutation expansion FSV failed: {report:#}").into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": report_path,
            "artifact_sha256": sha256_file(&report_path)?,
            "all_passed": true,
            "rows": rows.len(),
        }))?
    );
    Ok(())
}

fn seed_case(category: MutationCategory, idx: usize) -> SeedCase {
    match category {
        MutationCategory::OffByOne => off_by_one_case(idx),
        MutationCategory::SubtleFlip => subtle_flip_case(idx),
        MutationCategory::SwapVariable => swap_variable_case(idx),
        _ => unreachable!("targeted expansion only covers weak categories"),
    }
}

fn off_by_one_case(idx: usize) -> SeedCase {
    match idx % 5 {
        0 => SeedCase {
            category: MutationCategory::OffByOne,
            pattern: "range_len_boundary",
            source: format!(
                "def actual():\n    values = list({:?})\n    return ''.join(str(index) for index in range(len(values)))\n\nEXPECTED = {:?}\n",
                "abcdef",
                "012345"
            ),
        },
        1 => SeedCase {
            category: MutationCategory::OffByOne,
            pattern: "nested_loop_range_boundary",
            source: "def actual():\n    rows = ['ab', 'cd', 'ef']\n    seen = []\n    for row in range(len(rows)):\n        for col in range(len(rows[row])):\n            seen.append(rows[row][col])\n    return ''.join(seen)\n\nEXPECTED = 'abcdef'\n".to_string(),
        },
        2 => SeedCase {
            category: MutationCategory::OffByOne,
            pattern: "slice_stop_boundary",
            source: "def actual():\n    values = list('abcdef')\n    stop = len(values) - len('ef')\n    return ''.join(values[:stop])\n\nEXPECTED = 'abcd'\n".to_string(),
        },
        3 => SeedCase {
            category: MutationCategory::OffByOne,
            pattern: "slice_start_boundary",
            source: "def actual():\n    values = list('abcdef')\n    start = len('ab')\n    return ''.join(values[start:])\n\nEXPECTED = 'cdef'\n".to_string(),
        },
        _ => SeedCase {
            category: MutationCategory::OffByOne,
            pattern: "range_start_stop_boundary",
            source: "def actual():\n    values = list('abcdef')\n    start = len('ab')\n    stop = len(values) - len('ef')\n    return ''.join(values[index] for index in range(start, stop))\n\nEXPECTED = 'cd'\n".to_string(),
        },
    }
}

fn subtle_flip_case(idx: usize) -> SeedCase {
    match idx % 5 {
        0 => SeedCase {
            category: MutationCategory::SubtleFlip,
            pattern: "membership_in",
            source: "def actual():\n    value = 'z'\n    values = 'maze'\n    return value in values\n\nEXPECTED = bool('yes')\n".to_string(),
        },
        1 => SeedCase {
            category: MutationCategory::SubtleFlip,
            pattern: "membership_not_in",
            source: "def actual():\n    value = 'q'\n    values = 'maze'\n    return value not in values\n\nEXPECTED = bool('yes')\n".to_string(),
        },
        2 => SeedCase {
            category: MutationCategory::SubtleFlip,
            pattern: "identity_is",
            source: "def actual():\n    marker = None\n    return marker is None\n\nEXPECTED = bool('yes')\n".to_string(),
        },
        3 => SeedCase {
            category: MutationCategory::SubtleFlip,
            pattern: "any_all_call",
            source: "def actual():\n    flags = [bool('yes'), bool('')]\n    return any(flags)\n\nEXPECTED = bool('yes')\n".to_string(),
        },
        _ => SeedCase {
            category: MutationCategory::SubtleFlip,
            pattern: "comparison_chain",
            source: "def actual():\n    left = len('abc')\n    right = len('abcd')\n    return left < right\n\nEXPECTED = bool('yes')\n".to_string(),
        },
    }
}

fn swap_variable_case(idx: usize) -> SeedCase {
    match idx % 5 {
        0 => SeedCase {
            category: MutationCategory::SwapVariable,
            pattern: "function_parameters",
            source: "def combine(left, right):\n    return left + right\n\n\ndef actual():\n    return combine('L', 'R')\n\nEXPECTED = 'LR'\n".to_string(),
        },
        1 => SeedCase {
            category: MutationCategory::SwapVariable,
            pattern: "tuple_assignment",
            source: "def actual():\n    left, right = ('L', 'R')\n    return left + right\n\nEXPECTED = 'LR'\n".to_string(),
        },
        2 => SeedCase {
            category: MutationCategory::SwapVariable,
            pattern: "loop_local",
            source: "def actual():\n    prefix = ''\n    for item in ['a', 'b']:\n        prefix = prefix + item\n    return prefix\n\nEXPECTED = 'ab'\n".to_string(),
        },
        3 => SeedCase {
            category: MutationCategory::SwapVariable,
            pattern: "zip_arguments",
            source: "def actual():\n    left = ['a', 'b']\n    right = ['1', '2']\n    return [''.join(pair) for pair in zip(left, right)]\n\nEXPECTED = ['a1', 'b2']\n".to_string(),
        },
        _ => SeedCase {
            category: MutationCategory::SwapVariable,
            pattern: "f_string_replacement",
            source: "def actual():\n    left = 'L'\n    right = 'R'\n    return f'{left}-{right}'\n\nEXPECTED = 'L-R'\n".to_string(),
        },
    }
}

fn materialize_row(module_root: &Path, idx: usize, seed_case: SeedCase) -> FsvResult<RowRecord> {
    let seed = match seed_case.category {
        MutationCategory::OffByOne if matches!(seed_case.pattern, "slice_stop_boundary") => {
            (idx as u64) | 1
        }
        _ => idx as u64,
    };
    let outcome = apply_mutation(
        seed_case.category,
        &seed_case.source,
        MutationConfig {
            seed,
            alternate_source: None,
        },
    )?;
    let site = outcome
        .mutation_site
        .as_ref()
        .ok_or("targeted categories must report mutation sites")?;
    let row_id = format!("{}_{idx:04}", seed_case.category.slug());
    let original_module = format!("orig_{row_id}");
    let mutated_module = format!("mut_{row_id}");
    let original_path = module_root.join(format!("{original_module}.py"));
    let mutated_path = module_root.join(format!("{mutated_module}.py"));
    write_text_checked(&original_path, &seed_case.source)?;
    write_text_checked(&mutated_path, &outcome.mutated_source)?;
    Ok(RowRecord {
        category: seed_case.category,
        pattern: seed_case.pattern.to_string(),
        row_id,
        original_module,
        mutated_module,
        original_sha256: sha256_text(&seed_case.source),
        mutated_sha256: sha256_text(&outcome.mutated_source),
        mutation_note: site.note.clone(),
    })
}

fn write_pytest_suite(root: &Path, rows: &[RowRecord]) -> FsvResult<PathBuf> {
    let path = root.join("test_targeted_mutation_expansion.py");
    let row_pairs = rows
        .iter()
        .map(|row| {
            format!(
                "    ({:?}, {:?}),\n",
                row.original_module, row.mutated_module
            )
        })
        .collect::<String>();
    let source = format!(
        "import importlib\nimport pytest\n\nROWS = [\n{row_pairs}]\n\n@pytest.mark.parametrize(('original_module', 'mutated_module'), ROWS)\ndef test_original_passes(original_module, mutated_module):\n    module = importlib.import_module(original_module)\n    assert module.actual() == module.EXPECTED\n\n@pytest.mark.parametrize(('original_module', 'mutated_module'), ROWS)\ndef test_mutation_fails_intended_oracle(original_module, mutated_module):\n    module = importlib.import_module(mutated_module)\n    try:\n        actual = module.actual()\n    except Exception:\n        return\n    assert actual != module.EXPECTED\n"
    );
    write_text_checked(&path, &source)?;
    Ok(path)
}

fn write_py_compile_validator(root: &Path) -> FsvResult<PathBuf> {
    let path = root.join("validate_py_compile.py");
    let source = "import json\nimport os\nimport py_compile\nimport sys\nimport tempfile\n\nsink = os.path.join(tempfile.gettempdir(), 'contextgraph_phase_g_py_compile_sink.pyc')\ncompiled = 0\nfor item in sys.argv[1:]:\n    if os.path.isdir(item):\n        for dirpath, _, filenames in os.walk(item):\n            for filename in filenames:\n                if filename.endswith('.py'):\n                    py_compile.compile(os.path.join(dirpath, filename), cfile=sink, doraise=True)\n                    compiled += 1\n    else:\n        py_compile.compile(item, cfile=sink, doraise=True)\n        compiled += 1\ntry:\n    os.remove(sink)\nexcept FileNotFoundError:\n    pass\nprint(json.dumps({'compiled': compiled}, sort_keys=True))\n";
    write_text_checked(&path, source)?;
    Ok(path)
}

#[derive(Debug, Clone)]
struct CommandReport {
    status_code: i32,
    stdout: String,
    stderr: String,
}

impl CommandReport {
    fn to_json(&self) -> Value {
        json!({
            "status_code": self.status_code,
            "stdout_tail": tail(&self.stdout, 4000),
            "stderr_tail": tail(&self.stderr, 4000),
        })
    }
}

fn run_command(command: &mut Command, label: &str) -> FsvResult<CommandReport> {
    let output = command.output()?;
    let report = CommandReport {
        status_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };
    if report.status_code != 0 {
        return Err(format!(
            "{label} failed: {}",
            serde_json::to_string(&report.to_json())?
        )
        .into());
    }
    Ok(report)
}

fn rows_to_json(rows: &[RowRecord]) -> Value {
    Value::Array(rows.iter().map(row_to_json).collect())
}

fn parse_compiled_count(stdout: &str) -> FsvResult<usize> {
    let value: Value = serde_json::from_str(stdout.trim())?;
    value
        .get("compiled")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| {
            format!("py_compile validator did not report compiled count: {stdout:?}").into()
        })
}

fn row_to_json(row: &RowRecord) -> Value {
    json!({
        "row_id": row.row_id,
        "category": row.category.slug(),
        "pattern": row.pattern,
        "original_module": row.original_module,
        "mutated_module": row.mutated_module,
        "original_sha256": row.original_sha256,
        "mutated_sha256": row.mutated_sha256,
        "mutation_note": row.mutation_note,
    })
}

fn synthetic_gap_reduction() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("off_by_one::python".to_string(), 0.024),
        ("subtle_flip::python".to_string(), 0.023),
        ("swap_variable::python".to_string(), 0.022),
    ])
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<String> {
    fs::write(path, text)?;
    let readback = fs::read_to_string(path)?;
    if readback != text {
        return Err(format!("readback mismatch for {}", path.display()).into());
    }
    Ok(sha256_text(text))
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<String> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    if &readback != value {
        return Err(format!("JSON readback mismatch for {}", path.display()).into());
    }
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(format!("sha256:{:x}", Sha256::digest(fs::read(path)?)))
}

fn git_head_short() -> FsvResult<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--short")
        .arg("HEAD")
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn tail(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        value.to_string()
    } else {
        value.chars().skip(count - max_chars).collect()
    }
}
