// Real-Python synthetic-input tests for the eight mutation operators.
// No mocks: every input is plausible Python that mypy + python3 -c can
// parse (with the exception of `CompileError`, whose output is INTENDED
// not to parse).

use super::*;

const PY_SAMPLE_CMP_AND_OR: &str = "def is_eligible(age: int, has_consent: bool) -> bool:\n    if age >= 18 and has_consent:\n        return True\n    return False\n";

const PY_SAMPLE_INTS: &str = "MAX_RETRIES = 3\n\ndef bounded_loop(start: int) -> list[int]:\n    result = []\n    for i in range(start, start + 10):\n        result.append(i + 1)\n    return result\n";

const PY_SAMPLE_VARS: &str = "def normalize(x: float, y: float) -> float:\n    a = x * 2.0\n    b = y * 3.0\n    z = a + b\n    return z\n";

const PY_SAMPLE_TESTS: &str = "import pytest\n\n\nclass TestSuite:\n    def test_one(self) -> None:\n        self.assertEqual(1 + 1, 2)\n        self.assertTrue(True)\n\n    def test_two(self) -> None:\n        assert 2 + 2 == 4\n        with pytest.raises(ValueError):\n            int(\"oops\")\n";

const PY_SAMPLE_NO_BOOL_OPS: &str = "VERSION = \"1.0\"\n\ndef noop() -> None:\n    return None\n";

const PY_ALTERNATE: &str = "def helper(value: str) -> int:\n    return len(value)\n";

#[test]
fn category_all_returns_eight_in_canonical_order() {
    let cats = MutationCategory::all();
    assert_eq!(cats.len(), 8);
    assert_eq!(cats[0], MutationCategory::KnownGood);
    assert_eq!(cats[1], MutationCategory::SubtleFlip);
    assert_eq!(cats[2], MutationCategory::OffByOne);
    assert_eq!(cats[3], MutationCategory::SwapVariable);
    assert_eq!(cats[4], MutationCategory::DeleteTestCall);
    assert_eq!(cats[5], MutationCategory::WrongFile);
    assert_eq!(cats[6], MutationCategory::OverEngineer);
    assert_eq!(cats[7], MutationCategory::CompileError);
}

#[test]
fn known_good_echoes_input_with_no_site() {
    let outcome =
        apply_mutation(MutationCategory::KnownGood, PY_SAMPLE_CMP_AND_OR, MutationConfig::default())
            .unwrap();
    assert_eq!(outcome.mutated_source, PY_SAMPLE_CMP_AND_OR);
    assert!(outcome.mutation_site.is_none());
}

#[test]
fn empty_input_is_rejected() {
    let err = apply_mutation(MutationCategory::KnownGood, "", MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
}

#[test]
fn subtle_flip_inverts_a_boolean_operator() {
    let outcome = apply_mutation(
        MutationCategory::SubtleFlip,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    assert_ne!(outcome.mutated_source, PY_SAMPLE_CMP_AND_OR);
    let site = outcome.mutation_site.expect("subtle_flip must report a site");
    let around = &outcome.mutated_source[site.byte_offset..site.byte_offset + site.replacement_text.len()];
    assert_eq!(around, site.replacement_text);
}

#[test]
fn subtle_flip_rejects_source_with_no_boolean_ops() {
    let err = apply_mutation(
        MutationCategory::SubtleFlip,
        PY_SAMPLE_NO_BOOL_OPS,
        MutationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn subtle_flip_is_deterministic_per_seed() {
    let a = apply_mutation(
        MutationCategory::SubtleFlip,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 42,
            alternate_source: None,
        },
    )
    .unwrap();
    let b = apply_mutation(
        MutationCategory::SubtleFlip,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 42,
            alternate_source: None,
        },
    )
    .unwrap();
    assert_eq!(a.mutated_source, b.mutated_source);
    assert_eq!(a.mutation_site, b.mutation_site);
}

#[test]
fn subtle_flip_skips_operators_in_strings_and_comments() {
    let src = "# >= is just a comment here\ndef noop():\n    msg = \"if x == y\"\n    return msg\n";
    let err = apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default())
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn off_by_one_perturbs_an_integer_literal() {
    let outcome = apply_mutation(
        MutationCategory::OffByOne,
        PY_SAMPLE_INTS,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    assert_ne!(outcome.mutated_source, PY_SAMPLE_INTS);
    let site = outcome.mutation_site.expect("off_by_one must report a site");
    let original_int: i64 = site.original_text.parse().unwrap();
    let replacement_int: i64 = site.replacement_text.parse().unwrap();
    assert_eq!(replacement_int - original_int, 1);
}

#[test]
fn off_by_one_with_odd_seed_subtracts_one() {
    let outcome = apply_mutation(
        MutationCategory::OffByOne,
        PY_SAMPLE_INTS,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    let original_int: i64 = site.original_text.parse().unwrap();
    let replacement_int: i64 = site.replacement_text.parse().unwrap();
    assert_eq!(replacement_int - original_int, -1);
}

#[test]
fn off_by_one_rejects_source_with_only_floats() {
    let src = "PI = 3.14\nE = 2.718\n";
    let err = apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default())
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn off_by_one_rejects_digits_inside_identifiers() {
    let src = "def helper2(x): return x\n\nresult = helper2(7)\n";
    // helper2's `2` is an identifier suffix; only 7 is a true literal.
    let outcome = apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "7");
}

#[test]
fn swap_variable_renames_one_usage() {
    let outcome = apply_mutation(
        MutationCategory::SwapVariable,
        PY_SAMPLE_VARS,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    assert_ne!(outcome.mutated_source, PY_SAMPLE_VARS);
    let site = outcome.mutation_site.unwrap();
    assert_ne!(site.original_text, site.replacement_text);
}

#[test]
fn swap_variable_rejects_source_with_one_var() {
    let src = "def f():\n    x = 1\n    return x\n";
    let err = apply_mutation(MutationCategory::SwapVariable, src, MutationConfig::default())
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn swap_variable_rejects_when_target_has_no_downstream_usage() {
    // a, b defined but neither referenced after definition (no return etc.).
    let src = "a = 1\nb = 2\n";
    let err = apply_mutation(MutationCategory::SwapVariable, src, MutationConfig::default())
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn delete_test_call_removes_one_assertion_line() {
    let outcome = apply_mutation(
        MutationCategory::DeleteTestCall,
        PY_SAMPLE_TESTS,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert!(!site.original_text.is_empty());
    assert!(site.replacement_text.is_empty());
    assert_eq!(outcome.mutated_source.len(), PY_SAMPLE_TESTS.len() - site.byte_length);
    let trimmed = site.original_text.trim_start();
    let starts_with_assertion = trimmed.starts_with("self.assert")
        || trimmed.starts_with("assert ")
        || trimmed.starts_with("assert(")
        || trimmed.starts_with("pytest.raises")
        || trimmed.starts_with("pytest.assume");
    assert!(starts_with_assertion, "deleted line did not begin with an assertion: {trimmed:?}");
}

#[test]
fn delete_test_call_rejects_source_with_no_assertions() {
    let err = apply_mutation(
        MutationCategory::DeleteTestCall,
        PY_SAMPLE_NO_BOOL_OPS,
        MutationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn wrong_file_replaces_primary_with_alternate() {
    let outcome = apply_mutation(
        MutationCategory::WrongFile,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: Some(PY_ALTERNATE.to_string()),
        },
    )
    .unwrap();
    assert_eq!(outcome.mutated_source, PY_ALTERNATE);
    assert!(outcome.mutation_site.is_none());
}

#[test]
fn wrong_file_rejects_missing_alternate() {
    let err = apply_mutation(
        MutationCategory::WrongFile,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
}

#[test]
fn wrong_file_rejects_alternate_equal_to_primary() {
    let err = apply_mutation(
        MutationCategory::WrongFile,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: Some(PY_SAMPLE_CMP_AND_OR.to_string()),
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
}

#[test]
fn over_engineer_appends_helper_and_keeps_original_intact() {
    let outcome = apply_mutation(
        MutationCategory::OverEngineer,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 7,
            alternate_source: None,
        },
    )
    .unwrap();
    let original_trim = PY_SAMPLE_CMP_AND_OR.trim_end_matches('\n');
    assert!(outcome.mutated_source.starts_with(original_trim));
    assert!(outcome.mutated_source.contains("def _unused_helper_"));
    assert!(outcome.mutated_source.contains("Auto-generated dead helper"));
}

#[test]
fn over_engineer_helper_id_is_seed_dependent() {
    let a = apply_mutation(
        MutationCategory::OverEngineer,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    )
    .unwrap();
    let b = apply_mutation(
        MutationCategory::OverEngineer,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 2,
            alternate_source: None,
        },
    )
    .unwrap();
    assert_ne!(a.mutated_source, b.mutated_source);
}

#[test]
fn compile_error_appends_one_of_three_variants() {
    let outcome = apply_mutation(
        MutationCategory::CompileError,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let appended = outcome
        .mutation_site
        .as_ref()
        .map(|s| s.replacement_text.clone())
        .unwrap();
    let v0 = appended.contains("unclosed paren");
    let v1 = appended.contains("missing colon");
    let v2 = appended.contains("invalid def signature");
    assert!(v0 || v1 || v2, "CompileError appended unexpected variant: {appended:?}");
}

#[test]
fn compile_error_variant_distribution_covers_three_bins() {
    let mut variants = std::collections::HashSet::new();
    for seed in 0..30u64 {
        let outcome = apply_mutation(
            MutationCategory::CompileError,
            PY_SAMPLE_CMP_AND_OR,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        )
        .unwrap();
        let s = outcome.mutation_site.unwrap().replacement_text;
        if s.contains("unclosed paren") {
            variants.insert(0);
        } else if s.contains("missing colon") {
            variants.insert(1);
        } else if s.contains("invalid def signature") {
            variants.insert(2);
        }
    }
    assert_eq!(variants.len(), 3);
}

#[test]
fn slug_round_trips() {
    for cat in MutationCategory::all() {
        let slug = cat.slug();
        assert!(!slug.is_empty());
        assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }
}

#[test]
fn json_round_trips_outcome() {
    let outcome = apply_mutation(
        MutationCategory::SubtleFlip,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let json = serde_json::to_string(&outcome).unwrap();
    let back: MutationOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(back, outcome);
}

#[test]
fn subtle_flip_skips_python_type_annotation_arrow() {
    // Regression: `->` must NOT be picked up as a `>` candidate. The only
    // remaining candidate in this snippet is the `==` inside the body.
    let src = "def predict(x: int) -> bool:\n    return x == 1\n";
    let outcome = apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "==");
    assert!(!outcome.mutated_source.contains("-< bool"));
}

#[test]
fn subtle_flip_skips_bitshift_operators() {
    // Regression: `<<` and `>>` must not surface as single-char `<`/`>`.
    let src = "MASK = 1 << 4\nSHIFT = 8 >> 2\nflag = MASK == SHIFT\n";
    let outcome = apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "==", "only `==` should be a candidate, not `<<`/`>>`");
}

#[test]
fn off_by_one_skips_fractional_part_of_floats() {
    // Regression: `3.14` must NOT yield `14` as a candidate.
    let src = "PI = 3.14\nN = 7\n";
    let outcome = apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "7");
    assert!(!outcome.mutated_source.contains("3.15"));
    assert!(!outcome.mutated_source.contains("3.13"));
}

#[test]
fn applying_mutation_site_yields_byte_exact_replacement() {
    let outcome = apply_mutation(
        MutationCategory::OffByOne,
        PY_SAMPLE_INTS,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.as_ref().unwrap();
    let mut manual = String::new();
    manual.push_str(&PY_SAMPLE_INTS[..site.byte_offset]);
    manual.push_str(&site.replacement_text);
    manual.push_str(&PY_SAMPLE_INTS[site.byte_offset + site.byte_length..]);
    assert_eq!(manual, outcome.mutated_source);
}
