// Manual Full State Verification for the eight Phase-0 mutation operators.
//
// Sources of truth:
//   - /tmp/contextgraph-mejepa-corpus-fsv/generated_sources/*.py
//   - /tmp/contextgraph-mejepa-corpus-fsv/pyc/*.pyc
//   - /tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json
//
// This example fails closed. It does not merely record `passes: false`; every
// invariant is asserted before the evidence file is written, every generated
// source is read back from disk, and Python syntax reality is checked with
// `python3` + `py_compile`.

use context_graph_mejepa_corpus::{
    apply_mutation, MutationCategory, MutationConfig, MutationOutcome,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn run(category: MutationCategory, source: &str, config: MutationConfig) -> MutationOutcome {
    apply_mutation(category, source, config)
        .unwrap_or_else(|e| panic!("apply_mutation({category:?}) failed: {e:?}"))
}

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("ME-JEPA corpus FSV invariant failed: {label}").into())
    }
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);

    let readback = fs::read_to_string(path)?;
    ensure(
        &format!("durable readback must match write for {}", path.display()),
        readback == text,
    )?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    sha256_file(path)
}

fn compile_python_source(py_path: &Path, pyc_path: &Path) -> FsvResult<Value> {
    if pyc_path.exists() {
        fs::remove_file(pyc_path)?;
    }
    if let Some(parent) = pyc_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let output = Command::new("python3")
        .arg("-c")
        .arg("import py_compile, sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)")
        .arg(py_path)
        .arg(pyc_path)
        .output()?;
    let pyc_exists = pyc_path.is_file();
    let pyc_sha256 = if pyc_exists {
        Value::String(sha256_file(pyc_path)?)
    } else {
        Value::Null
    };
    Ok(json!({
        "source_of_truth": py_path.display().to_string(),
        "pyc_source_of_truth": pyc_path.display().to_string(),
        "exit_code": output.status.code().unwrap_or(-1),
        "success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
        "pyc_exists": pyc_exists,
        "pyc_sha256": pyc_sha256,
    }))
}

fn persist_source_and_compile(
    generated_root: &Path,
    pyc_root: &Path,
    slug: &str,
    source: &str,
    expect_parse_success: bool,
) -> FsvResult<Value> {
    let py_path = generated_root.join(format!("{slug}.py"));
    let pyc_path = pyc_root.join(format!("{slug}.pyc"));
    let file_sha256 = write_text_checked(&py_path, source)?;
    ensure(
        &format!("{} source hash must match text hash", slug),
        file_sha256 == sha256_text(source),
    )?;
    let compile = compile_python_source(&py_path, &pyc_path)?;
    let success = compile["success"].as_bool() == Some(true);
    ensure(
        &format!("{slug} py_compile success must be {expect_parse_success}"),
        success == expect_parse_success,
    )?;
    if expect_parse_success {
        ensure(
            &format!("{slug} successful py_compile must produce pyc"),
            compile["pyc_exists"].as_bool() == Some(true),
        )?;
    } else {
        ensure(
            &format!("{slug} failed py_compile must not leave pyc"),
            compile["pyc_exists"].as_bool() == Some(false),
        )?;
    }
    Ok(json!({
        "source_of_truth": py_path.display().to_string(),
        "sha256": file_sha256,
        "byte_length": source.len(),
        "expected_parse_success": expect_parse_success,
        "py_compile": compile,
    }))
}

fn boundary_case(
    label: &str,
    category: MutationCategory,
    source: &str,
    config: MutationConfig,
    expected_code: &'static str,
) -> FsvResult<Value> {
    let before = json!({
        "category": category.slug(),
        "input_byte_length": source.len(),
        "input_sha256": sha256_text(source),
        "alternate_source_present": config.alternate_source.is_some(),
    });
    let (actual_code, after) = match apply_mutation(category, source, config) {
        Err(err) => (
            err.code().to_string(),
            json!({
                "result": "error",
                "error_code": err.code(),
                "error_message": err.to_string(),
            }),
        ),
        Ok(outcome) => (
            "UNEXPECTED_OK".to_string(),
            json!({
                "result": "ok",
                "output_byte_length": outcome.mutated_source.len(),
                "output_sha256": sha256_text(&outcome.mutated_source),
                "mutation_site": outcome.mutation_site,
            }),
        ),
    };
    ensure(
        &format!("{label} must return {expected_code}, got {actual_code}"),
        actual_code == expected_code,
    )?;
    Ok(json!({
        "before": before,
        "after": after,
        "expected_code": expected_code,
        "actual_code": actual_code,
        "passes": true,
    }))
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

    // Real Python sample with comparisons + boolean operators + ints +
    // assertion-style lines + variable definitions.
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
        MutationCategory::KnownGood,
        py_source,
        MutationConfig::default(),
    );
    ensure(
        "KnownGood must echo input with no mutation site",
        known_good.mutated_source == py_source && known_good.mutation_site.is_none(),
    )?;
    let known_good_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "known_good",
        &known_good.mutated_source,
        true,
    )?;

    let subtle = run(
        MutationCategory::SubtleFlip,
        py_source,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    );
    let subtle_site = subtle.mutation_site.clone().unwrap();
    ensure(
        "SubtleFlip must change source and replace with different text",
        subtle.mutated_source != py_source
            && subtle_site.replacement_text != subtle_site.original_text,
    )?;
    let subtle_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "subtle_flip",
        &subtle.mutated_source,
        true,
    )?;

    let off = run(
        MutationCategory::OffByOne,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    );
    let off_site = off.mutation_site.clone().unwrap();
    let off_orig: i64 = off_site.original_text.parse()?;
    let off_new: i64 = off_site.replacement_text.parse()?;
    ensure(
        "OffByOne must move selected int by exactly one",
        (off_new - off_orig).abs() == 1,
    )?;
    let off_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "off_by_one",
        &off.mutated_source,
        true,
    )?;

    let swap = run(
        MutationCategory::SwapVariable,
        py_source,
        MutationConfig {
            seed: 11,
            alternate_source: None,
        },
    );
    let swap_site = swap.mutation_site.clone().unwrap();
    ensure(
        "SwapVariable must rename a usage to a different name",
        swap_site.original_text != swap_site.replacement_text && swap.mutated_source != py_source,
    )?;
    let swap_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "swap_variable",
        &swap.mutated_source,
        true,
    )?;

    let delete_call = run(
        MutationCategory::DeleteTestCall,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    );
    let delete_site = delete_call.mutation_site.clone().unwrap();
    ensure(
        "DeleteTestCall must replace its reported assertion bytes with pass",
        delete_site.replacement_text.trim() == "pass"
            && delete_call.mutated_source.len()
                == py_source.len() - delete_site.byte_length + delete_site.replacement_text.len(),
    )?;
    let delete_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "delete_test_call",
        &delete_call.mutated_source,
        true,
    )?;

    let wrong = run(
        MutationCategory::WrongFile,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: Some(alternate.to_string()),
        },
    );
    ensure(
        "WrongFile must replace primary with alternate and no mutation site",
        wrong.mutated_source == alternate && wrong.mutation_site.is_none(),
    )?;
    let wrong_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "wrong_file",
        &wrong.mutated_source,
        true,
    )?;

    let over = run(
        MutationCategory::OverEngineer,
        py_source,
        MutationConfig {
            seed: 7,
            alternate_source: None,
        },
    );
    let over_site = over.mutation_site.clone().unwrap();
    ensure(
        "OverEngineer must append a 48-bit unused helper and preserve original prefix",
        over.mutated_source.contains("def _unused_helper_")
            && over
                .mutated_source
                .starts_with(py_source.trim_end_matches('\n'))
            && over_site.replacement_text.contains("def _unused_helper_")
            && over_site
                .replacement_text
                .split("def _unused_helper_")
                .nth(1)
                .and_then(|tail| tail.split("()").next())
                .map(|suffix| {
                    suffix.len() == 12 && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
                })
                == Some(true),
    )?;
    let over_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "over_engineer",
        &over.mutated_source,
        true,
    )?;

    let cerr = run(
        MutationCategory::CompileError,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    );
    let cerr_site = cerr.mutation_site.clone().unwrap();
    ensure(
        "CompileError must append one expected syntax-error variant",
        cerr_site.byte_length == 0
            && cerr.mutated_source.len() == py_source.len() + cerr_site.replacement_text.len()
            && (cerr_site.replacement_text.contains("unclosed paren")
                || cerr_site.replacement_text.contains("missing colon")
                || cerr_site.replacement_text.contains("invalid def signature")),
    )?;
    let cerr_disk = persist_source_and_compile(
        &generated_root,
        &pyc_root,
        "compile_error",
        &cerr.mutated_source,
        false,
    )?;

    // Boundary cases with explicit before/after state.
    let empty_boundary = boundary_case(
        "empty input",
        MutationCategory::KnownGood,
        "",
        MutationConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let no_alt_boundary = boundary_case(
        "WrongFile without alternate",
        MutationCategory::WrongFile,
        py_source,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let no_bool_boundary = boundary_case(
        "SubtleFlip no bool op",
        MutationCategory::SubtleFlip,
        "VERSION = \"1.0\"\n\ndef noop():\n    pass\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let no_int_boundary = boundary_case(
        "OffByOne floats only",
        MutationCategory::OffByOne,
        "PI = 3.14\nE = 2.718\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    let no_assert_boundary = boundary_case(
        "DeleteTestCall no assertions",
        MutationCategory::DeleteTestCall,
        "def noop():\n    return None\n",
        MutationConfig::default(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;

    let evidence = json!({
        "input_source_sha256": sha256_text(py_source),
        "input_byte_length": py_source.len(),
        "generated_sources_root": generated_root.display().to_string(),
        "pyc_root": pyc_root.display().to_string(),
        "case_known_good": {
            "disk": known_good_disk,
            "site": known_good.mutation_site,
            "passes": true,
        },
        "case_subtle_flip": {
            "disk": subtle_disk,
            "site": subtle_site,
            "passes": true,
        },
        "case_off_by_one": {
            "disk": off_disk,
            "site": off_site,
            "original_int": off_orig,
            "replacement_int": off_new,
            "passes": true,
        },
        "case_swap_variable": {
            "disk": swap_disk,
            "site": swap_site,
            "passes": true,
        },
        "case_delete_test_call": {
            "disk": delete_disk,
            "site": delete_site,
            "passes": true,
        },
        "case_wrong_file": {
            "disk": wrong_disk,
            "primary_source_length": py_source.len(),
            "alternate_source_length": alternate.len(),
            "passes": true,
        },
        "case_over_engineer": {
            "disk": over_disk,
            "site": over_site,
            "passes": true,
        },
        "case_compile_error": {
            "disk": cerr_disk,
            "site": cerr_site,
            "passes": true,
        },
        "boundary_empty_input_rejected": empty_boundary,
        "boundary_wrong_file_without_alternate_rejected": no_alt_boundary,
        "boundary_subtle_flip_no_bool_op_rejected": no_bool_boundary,
        "boundary_off_by_one_floats_only_rejected": no_int_boundary,
        "boundary_delete_test_call_no_assertions_rejected": no_assert_boundary,
    });

    let path = evidence_root.join("mejepa-corpus-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    let evidence_sha = write_text_checked(&path, &text)?;

    let readback = fs::read_to_string(&path)?;
    let parsed: Value = serde_json::from_str(&readback)?;
    ensure(
        "read-back JSON must match in-memory evidence",
        parsed == evidence,
    )?;
    println!("ME-JEPA mutation-corpus FSV evidence: {}", path.display());
    println!("Evidence sha256: {evidence_sha}");
    println!("Read-back size: {} bytes", readback.len());
    println!("{}", text);
    Ok(())
}
