// Manual Full State Verification for the eight Phase-0 mutation operators.
//
// Source of truth: /tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json.
// Each per-category section records:
//   - input source bytes (original)
//   - mutation-site coordinates and replacement bytes
//   - output source bytes (after applying the site)
//   - a `passes` boolean asserting the category-specific invariant
// Boundary cases at the bottom: empty input, missing alternate for WrongFile,
// no-mutation-site for SubtleFlip on no-bool source.

use context_graph_mejepa_corpus::{
    apply_mutation, MutationCategory, MutationConfig, MutationOutcome,
};
use std::fs;
use std::path::PathBuf;

fn run(category: MutationCategory, source: &str, config: MutationConfig) -> MutationOutcome {
    apply_mutation(category, source, config)
        .unwrap_or_else(|e| panic!("apply_mutation({category:?}) failed: {e:?}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv");
    fs::create_dir_all(&evidence_root)?;

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

    let known_good = run(MutationCategory::KnownGood, py_source, MutationConfig::default());
    let kg_passes =
        known_good.mutated_source == py_source && known_good.mutation_site.is_none();

    let subtle = run(MutationCategory::SubtleFlip, py_source, MutationConfig { seed: 1, alternate_source: None });
    let subtle_site = subtle.mutation_site.clone().unwrap();
    let subtle_passes = subtle.mutated_source != py_source
        && subtle_site.replacement_text != subtle_site.original_text;

    let off = run(MutationCategory::OffByOne, py_source, MutationConfig { seed: 0, alternate_source: None });
    let off_site = off.mutation_site.clone().unwrap();
    let off_orig: i64 = off_site.original_text.parse()?;
    let off_new: i64 = off_site.replacement_text.parse()?;
    let off_passes = (off_new - off_orig).abs() == 1;

    let swap = run(MutationCategory::SwapVariable, py_source, MutationConfig { seed: 11, alternate_source: None });
    let swap_site = swap.mutation_site.clone().unwrap();
    let swap_passes = swap_site.original_text != swap_site.replacement_text
        && swap.mutated_source != py_source;

    let delete_call = run(MutationCategory::DeleteTestCall, py_source, MutationConfig { seed: 0, alternate_source: None });
    let delete_site = delete_call.mutation_site.clone().unwrap();
    let delete_passes = delete_site.replacement_text.is_empty()
        && delete_call.mutated_source.len() + delete_site.byte_length == py_source.len();

    let wrong = run(
        MutationCategory::WrongFile,
        py_source,
        MutationConfig { seed: 0, alternate_source: Some(alternate.to_string()) },
    );
    let wrong_passes = wrong.mutated_source == alternate && wrong.mutation_site.is_none();

    let over = run(MutationCategory::OverEngineer, py_source, MutationConfig { seed: 7, alternate_source: None });
    let over_passes = over.mutated_source.contains("def _unused_helper_")
        && over.mutated_source.starts_with(py_source.trim_end_matches('\n'));

    let cerr = run(MutationCategory::CompileError, py_source, MutationConfig { seed: 0, alternate_source: None });
    let cerr_site = cerr.mutation_site.clone().unwrap();
    let cerr_passes = cerr_site.byte_length == 0
        && cerr.mutated_source.len() == py_source.len() + cerr_site.replacement_text.len()
        && (cerr_site.replacement_text.contains("unclosed paren")
            || cerr_site.replacement_text.contains("missing colon")
            || cerr_site.replacement_text.contains("invalid def signature"));

    // === Boundary cases ===
    let empty_code = match apply_mutation(MutationCategory::KnownGood, "", MutationConfig::default()) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let no_alt_code = match apply_mutation(
        MutationCategory::WrongFile,
        py_source,
        MutationConfig { seed: 0, alternate_source: None },
    ) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let no_bool_code = match apply_mutation(
        MutationCategory::SubtleFlip,
        "VERSION = \"1.0\"\n\ndef noop():\n    pass\n",
        MutationConfig::default(),
    ) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let no_int_code = match apply_mutation(
        MutationCategory::OffByOne,
        "PI = 3.14\nE = 2.718\n",
        MutationConfig::default(),
    ) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let no_assert_code = match apply_mutation(
        MutationCategory::DeleteTestCall,
        "def noop():\n    return None\n",
        MutationConfig::default(),
    ) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    let evidence = serde_json::json!({
        "input_source_sha256_proxy": py_source.len(),
        "input_byte_length": py_source.len(),
        "case_known_good": {
            "mutated_source": known_good.mutated_source,
            "site": known_good.mutation_site,
            "passes": kg_passes,
        },
        "case_subtle_flip": {
            "site": subtle_site,
            "mutated_excerpt_around_site": &subtle.mutated_source
                [subtle.mutation_site.as_ref().unwrap().byte_offset.saturating_sub(10)
                    ..(subtle.mutation_site.as_ref().unwrap().byte_offset
                        + subtle.mutation_site.as_ref().unwrap().byte_length
                        + 10)
                    .min(subtle.mutated_source.len())],
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
            "site": over.mutation_site,
            "appended_length": over.mutated_source.len() - py_source.len(),
            "passes": over_passes,
        },
        "case_compile_error": {
            "site": cerr_site,
            "passes": cerr_passes,
        },
        "boundary_empty_input_rejected": {
            "expected_code": "MEJEPA_CORPUS_INVALID_INPUT",
            "actual_code": empty_code,
        },
        "boundary_wrong_file_without_alternate_rejected": {
            "expected_code": "MEJEPA_CORPUS_INVALID_INPUT",
            "actual_code": no_alt_code,
        },
        "boundary_subtle_flip_no_bool_op_rejected": {
            "expected_code": "MEJEPA_CORPUS_NO_MUTATION_SITE",
            "actual_code": no_bool_code,
        },
        "boundary_off_by_one_floats_only_rejected": {
            "expected_code": "MEJEPA_CORPUS_NO_MUTATION_SITE",
            "actual_code": no_int_code,
        },
        "boundary_delete_test_call_no_assertions_rejected": {
            "expected_code": "MEJEPA_CORPUS_NO_MUTATION_SITE",
            "actual_code": no_assert_code,
        },
    });

    let path = evidence_root.join("mejepa-corpus-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    fs::write(&path, &text)?;

    // Read-back verification: physically re-read the file and parse it.
    let readback = fs::read_to_string(&path)?;
    let parsed: serde_json::Value = serde_json::from_str(&readback)?;
    if parsed != evidence {
        return Err("read-back JSON did not match in-memory evidence".into());
    }
    println!("ME-JEPA mutation-corpus FSV evidence: {}", path.display());
    println!("Read-back size: {} bytes", readback.len());
    println!("{}", text);
    Ok(())
}
