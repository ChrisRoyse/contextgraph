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
fn mutation_category_slug_round_trip() {
    for category in MutationCategory::all() {
        assert_eq!(MutationCategory::from_slug(category.slug()), Some(category));
    }
    assert_eq!(MutationCategory::from_slug("unknown"), None);
}

#[test]
fn error_codes_match_taxonomy() {
    assert_eq!(
        MutationError::invalid("field", "message", "remediation").code(),
        "MEJEPA_CORPUS_INVALID_INPUT"
    );
    assert_eq!(
        MutationError::no_site("field", "message", "remediation").code(),
        "MEJEPA_CORPUS_NO_MUTATION_SITE"
    );
    assert_eq!(
        MutationError::op_failed("field", "message", "remediation").code(),
        "MEJEPA_CORPUS_OPERATOR_FAILED"
    );
    assert_eq!(
        MutationError::leakage_detected(
            "sha256:duplicate",
            vec![("task-a".to_string(), "train".to_string())]
        )
        .code(),
        "MEJEPA_CORPUS_LEAKAGE_DETECTED"
    );
    let docker =
        crate::oracle::OracleError::docker_run_id_unsafe("bad+run", '+', 3, "use [A-Za-z0-9_.-]");
    assert_eq!(docker.code(), "MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE");
}

#[test]
fn language_enum_slug_round_trip() {
    let languages = Language::all();
    assert_eq!(languages.len(), 11);
    assert_eq!(languages[0], Language::Rust);
    assert_eq!(languages[1], Language::Python);
    for language in languages {
        assert_eq!(Language::from_slug(language.slug()), Some(language));
    }
    assert_eq!(Language::from_slug("lua"), None);
}

#[test]
fn language_parse_accepts_canonical_languages_and_rejects_unknown() {
    let accepted = parse_languages(&["python".to_string()]).unwrap();
    assert_eq!(accepted, vec![Language::Python]);
    let accepted_all =
        parse_languages(&["rust".to_string(), "python".to_string(), "rust".to_string()]).unwrap();
    assert_eq!(accepted_all, vec![Language::Rust, Language::Python]);

    let unknown = parse_languages(&["lua".to_string()]).unwrap_err();
    assert_eq!(unknown.code(), "MEJEPA_CORPUS_INVALID_INPUT");
    assert!(unknown.to_string().contains("unknown language slug"));
}

#[test]
fn oracle_verdict_reexport_round_trip() {
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_reexport.py::test_ok".to_string(),
            outcome: TestOutcome::Pass,
            runtime_ms: 1,
        }],
        exception: Some(ExceptionClass::AssertionError),
        evidence_unavailable: false,
    };
    let text = serde_json::to_string(&verdict).unwrap();
    let readback: OracleVerdict = serde_json::from_str(&text).unwrap();
    assert_eq!(readback, verdict);
}

#[test]
fn known_good_echoes_input_with_no_site() {
    let outcome = apply_mutation(
        MutationCategory::KnownGood,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig::default(),
    )
    .unwrap();
    assert_eq!(outcome.mutated_source, PY_SAMPLE_CMP_AND_OR);
    assert!(outcome.mutation_site.is_none());
}

#[test]
fn empty_input_is_rejected() {
    let err =
        apply_mutation(MutationCategory::KnownGood, "", MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
}

#[test]
fn invalid_python_input_is_rejected_before_mutation() {
    let err = apply_mutation(
        MutationCategory::SubtleFlip,
        "def broken(:\n    return True\n",
        MutationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_INVALID_INPUT");
    let message = err.to_string();
    assert!(message.contains("line="), "missing line detail: {message}");
    assert!(
        message.contains("byte_range="),
        "missing byte detail: {message}"
    );
    assert!(message.contains("snippet="), "missing snippet: {message}");
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
    let site = outcome
        .mutation_site
        .expect("subtle_flip must report a site");
    let around =
        &outcome.mutated_source[site.byte_offset..site.byte_offset + site.replacement_text.len()];
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
    let err =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn subtle_flip_skips_triple_quoted_strings() {
    let src = "\"\"\"do not flip x > y or True here\"\"\"\nflag = left == right\n";
    let outcome =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "==");
}

#[test]
fn subtle_flip_skips_triple_quoted_docstrings() {
    let src = "def noop():\n    \"\"\"x == y and y > z\"\"\"\n    return None\n";
    let err =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn subtle_flip_skips_f_string_format_spec_alignment() {
    let src = "value = int(\"3\")\nprint(f\"{value:>10}\")\n";
    let err =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn subtle_flip_keeps_f_string_expression_code_visible() {
    let src = "left = int(\"1\")\nright = int(\"2\")\nprint(f\"{left == right:>10}\")\n";
    let outcome =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "==");
    assert!(!outcome.mutated_source.contains("{left == right:<10}"));
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
    let site = outcome
        .mutation_site
        .expect("off_by_one must report a site");
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
    let err =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn off_by_one_skips_numbers_in_triple_quoted_strings() {
    let src = "def noop():\n    '''version 123'''\n    return None\n";
    let err =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn off_by_one_skips_f_string_format_width_literals() {
    let src = "value = int(\"3\")\nprint(f\"{value:>10}\")\n";
    let err =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn off_by_one_rejects_digits_inside_identifiers() {
    let src = "def helper2(x): return x\n\nresult = helper2(7)\n";
    // helper2's `2` is an identifier suffix; only 7 is a true literal.
    let outcome =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "7");
}

#[test]
fn off_by_one_skips_triple_quoted_string_digits() {
    let src = "\"\"\"release 123 should stay literal text\"\"\"\nCOUNT = 7\n";
    let outcome =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "7");
    assert!(outcome.mutated_source.contains("release 123"));
}

#[test]
fn off_by_one_mutates_range_boundary_expression_without_numeric_literals() {
    let src = "def actual(values):\n    return list(range(len(values)))\n";
    let outcome = apply_mutation(
        MutationCategory::OffByOne,
        src,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "len(values)");
    assert!(site.note.contains("range(...) boundary"));
    assert!(site.replacement_text.contains("+ 1"));
}

#[test]
fn off_by_one_mutates_slice_boundary_expression_without_numeric_literals() {
    let src = "def actual(values, stop):\n    return values[:stop]\n";
    let outcome = apply_mutation(
        MutationCategory::OffByOne,
        src,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "stop");
    assert!(site.note.contains("slice boundary"));
    assert!(site.replacement_text.contains("- 1"));
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
fn subtle_flip_flips_membership_identity_and_any_all_patterns() {
    let membership = apply_mutation(
        MutationCategory::SubtleFlip,
        "def actual(value, values):\n    return value in values\n",
        MutationConfig::default(),
    )
    .unwrap();
    let membership_site = membership.mutation_site.unwrap();
    assert_eq!(membership_site.original_text, "in");
    assert_eq!(membership_site.replacement_text, "not in");

    let identity = apply_mutation(
        MutationCategory::SubtleFlip,
        "def actual(value):\n    return value is None\n",
        MutationConfig::default(),
    )
    .unwrap();
    let identity_site = identity.mutation_site.unwrap();
    assert_eq!(identity_site.original_text, "is");
    assert_eq!(identity_site.replacement_text, "is not");

    let any_all = apply_mutation(
        MutationCategory::SubtleFlip,
        "def actual(flags):\n    return any(flags)\n",
        MutationConfig::default(),
    )
    .unwrap();
    let any_all_site = any_all.mutation_site.unwrap();
    assert_eq!(any_all_site.original_text, "any");
    assert_eq!(any_all_site.replacement_text, "all");
}

#[test]
fn subtle_flip_does_not_mutate_for_loop_in_keyword() {
    let src =
        "def actual(values):\n    for value in values:\n        return value\n    return None\n";
    let err =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn swap_variable_uses_previously_defined_replacement() {
    let src = "def f():\n    a = 1\n    b = 2\n    c = 3\n    return b + c\n";
    let outcome = apply_mutation(
        MutationCategory::SwapVariable,
        src,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert!(matches!(site.replacement_text.as_str(), "a" | "b" | "c"));
    assert_ne!(site.original_text, site.replacement_text);
}

#[test]
fn swap_variable_collects_parameters_tuple_assignments_and_loop_locals() {
    let params = apply_mutation(
        MutationCategory::SwapVariable,
        "def actual(left, right):\n    return left + right\n",
        MutationConfig::default(),
    )
    .unwrap();
    let params_site = params.mutation_site.unwrap();
    assert!(matches!(
        params_site.original_text.as_str(),
        "left" | "right"
    ));
    assert_ne!(params_site.original_text, params_site.replacement_text);

    let tuple_assignment = apply_mutation(
        MutationCategory::SwapVariable,
        "def actual():\n    left, right = ('L', 'R')\n    return left + right\n",
        MutationConfig::default(),
    )
    .unwrap();
    let tuple_site = tuple_assignment.mutation_site.unwrap();
    assert!(matches!(
        tuple_site.original_text.as_str(),
        "left" | "right"
    ));
    assert_ne!(tuple_site.original_text, tuple_site.replacement_text);

    let loop_local = apply_mutation(
        MutationCategory::SwapVariable,
        "def actual(values):\n    prefix = ''\n    for item in values:\n        prefix = prefix + item\n    return prefix\n",
        MutationConfig::default(),
    )
    .unwrap();
    let loop_site = loop_local.mutation_site.unwrap();
    assert!(matches!(
        loop_site.original_text.as_str(),
        "prefix" | "item"
    ));
    assert_ne!(loop_site.original_text, loop_site.replacement_text);
}

#[test]
fn swap_variable_rejects_source_with_one_var() {
    let src = "def f():\n    x = 1\n    return x\n";
    let err = apply_mutation(
        MutationCategory::SwapVariable,
        src,
        MutationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn swap_variable_rejects_when_target_has_no_downstream_usage() {
    // a, b defined but neither referenced after definition (no return etc.).
    let src = "a = 1\nb = 2\n";
    let err = apply_mutation(
        MutationCategory::SwapVariable,
        src,
        MutationConfig::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_NO_MUTATION_SITE");
}

#[test]
fn swap_variable_sees_usage_inside_f_string_replacement_field() {
    let src = "left = \"a\"\nright = \"b\"\nprint(f\"{left}-{right}\")\n";
    let outcome = apply_mutation(
        MutationCategory::SwapVariable,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert!(site.original_text == "left" || site.original_text == "right");
    assert_ne!(site.original_text, site.replacement_text);
    let original_slice = &src[site.byte_offset..site.byte_offset + site.byte_length];
    assert_eq!(original_slice, site.original_text);
    assert!(
        site.byte_offset > src.find("f\"").unwrap(),
        "usage should be inside the f-string replacement field"
    );
}

#[test]
fn swap_variable_sees_nested_f_string_format_replacement_field() {
    let src = "width = 10\nvalue = 3\nprint(f\"{value:>{width}}\")\n";
    let outcome = apply_mutation(
        MutationCategory::SwapVariable,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert!(site.original_text == "width" || site.original_text == "value");
    assert_ne!(site.original_text, site.replacement_text);
    assert!(
        site.byte_offset > src.find("f\"").unwrap(),
        "usage should be inside the f-string"
    );
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
    assert_eq!(site.replacement_text.trim(), "pass");
    assert_eq!(
        outcome.mutated_source.len(),
        PY_SAMPLE_TESTS.len() - site.byte_length + site.replacement_text.len()
    );
    let trimmed = site.original_text.trim_start();
    let starts_with_assertion = trimmed.starts_with("self.assert")
        || trimmed.starts_with("assert ")
        || trimmed.starts_with("assert(")
        || trimmed.starts_with("pytest.raises")
        || trimmed.starts_with("with pytest.raises")
        || trimmed.starts_with("pytest.assume");
    assert!(
        starts_with_assertion,
        "deleted line did not begin with an assertion: {trimmed:?}"
    );
}

#[test]
fn delete_test_call_preserves_syntax_for_only_statement_body() {
    let src = "def test_one():\n    assert True\n";
    let outcome = apply_mutation(
        MutationCategory::DeleteTestCall,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    assert_eq!(outcome.mutated_source, "def test_one():\n    pass\n");
}

#[test]
fn delete_test_call_replaces_pytest_raises_block_with_pass() {
    let src = "def test_bad():\n    with pytest.raises(ValueError):\n        int('x')\n";
    let outcome = apply_mutation(
        MutationCategory::DeleteTestCall,
        src,
        MutationConfig {
            seed: 0,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert!(
        site.original_text.contains("with pytest.raises"),
        "site was {site:?}"
    );
    assert!(site.original_text.contains("int('x')"), "site was {site:?}");
    assert!(outcome.mutated_source.contains("    pass\n"));
    assert!(!outcome.mutated_source.contains("with pytest.raises"));
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
    assert!(outcome
        .mutated_source
        .contains("Auto-generated dead helper"));
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
fn over_engineer_helper_id_uses_48_bit_suffix() {
    let outcome = apply_mutation(
        MutationCategory::OverEngineer,
        PY_SAMPLE_CMP_AND_OR,
        MutationConfig {
            seed: 1,
            alternate_source: None,
        },
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    let marker = "def _unused_helper_";
    let start = site.replacement_text.find(marker).expect("helper marker") + marker.len();
    let suffix = &site.replacement_text[start..start + 12];
    assert!(suffix.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(&site.replacement_text[start + 12..start + 14], "()");
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
    assert!(
        v0 || v1 || v2,
        "CompileError appended unexpected variant: {appended:?}"
    );
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
    let outcome =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(site.original_text, "==");
    assert!(!outcome.mutated_source.contains("-< bool"));
}

#[test]
fn generic_subtle_flip_skips_rust_return_arrow() {
    // Regression: the generic non-Python operator must validate syntax after
    // replacement so Rust `->` return arrows are not mutated into `-<`.
    let src = "fn check(alpha: i32, beta: i32) -> bool {\n    let total = alpha + beta + 3;\n    let expected = 5;\n    total == expected && true\n}\n";
    let outcome = apply_mutation_for_language(
        Language::Rust,
        MutationCategory::SubtleFlip,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_ne!(site.original_text, ">");
    assert!(!outcome.mutated_source.contains("-< bool"));
}

#[test]
fn generic_swap_variable_skips_rust_binding_declarations() {
    // Regression: mutating `let expected = ...` made later uses unresolved.
    let src = "fn check(alpha: i32, beta: i32) -> bool {\n    let total = alpha + beta + 3;\n    let expected = 5;\n    total == expected && true\n}\n";
    for seed in 0..32 {
        let outcome = apply_mutation_for_language(
            Language::Rust,
            MutationCategory::SwapVariable,
            src,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        )
        .unwrap();
        assert!(outcome.mutated_source.contains("let total ="));
        assert!(outcome.mutated_source.contains("let expected ="));
    }
}

#[test]
fn generic_delete_test_call_does_not_match_expected_variable() {
    // Regression: substring matching for `expect` deleted the
    // `let expected = ...` binding instead of the actual assertion line.
    let src = "fn check(alpha: i32, beta: i32) -> bool {\n    let total = alpha + beta + 3;\n    let expected = 5;\n    assert!(total >= 0);\n    total == expected && true\n}\n";
    let outcome = apply_mutation_for_language(
        Language::Rust,
        MutationCategory::DeleteTestCall,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    assert!(outcome.mutated_source.contains("let expected ="));
    assert!(!outcome.mutated_source.contains("assert!(total >= 0);"));
}

#[test]
fn generic_swap_variable_skips_csharp_method_parameter_bindings() {
    // Regression: C# replacement names included `condition` from
    // `void assert(bool condition)`, creating an out-of-scope use.
    let src = "class MutationFixture0000 {\n  void assert(bool condition) {}\n  bool Check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    int expected = 5;\n    assert(total >= 0);\n    return total == expected && true;\n  }\n}\n";
    for seed in 0..32 {
        let outcome = apply_mutation_for_language(
            Language::CSharp,
            MutationCategory::SwapVariable,
            src,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        )
        .unwrap();
        assert!(!outcome.mutated_source.contains("condition >= 0"));
        assert!(outcome
            .mutated_source
            .contains("void assert(bool condition)"));
    }
}

#[test]
fn generic_swap_variable_skips_go_type_declarations() {
    // Regression: mutating `type tester0009 struct{}` left receiver and
    // variable declarations referring to an undefined type.
    let src = "package main\n\ntype tester0009 struct{}\nfunc (tester0009) Fatal(message string) {}\n\nfunc check0009(alpha int, beta int) bool {\n    var t tester0009\n    total := alpha + beta + 12\n    expected := 14\n    if total >= 0 { t.Fatal(\"fixture assertion\") }\n    _ = t\n    if expected < 0 { return false }\n    return total == expected && true\n}\n";
    for seed in 0..32 {
        let outcome = apply_mutation_for_language(
            Language::Go,
            MutationCategory::SwapVariable,
            src,
            MutationConfig {
                seed,
                alternate_source: None,
            },
        )
        .unwrap();
        assert!(outcome.mutated_source.contains("type tester0009 struct{}"));
        assert!(outcome.mutated_source.matches("expected").count() >= 2);
    }
}

#[test]
fn generic_delete_test_call_skips_csharp_assert_helper_definition() {
    // Regression: `void assert(...)` is a helper declaration, not the test
    // assertion call to delete.
    let src = "class MutationFixture0000 {\n  void assert(bool condition) {}\n  bool Check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    int expected = 5;\n    assert(total >= 0);\n    return total == expected && true;\n  }\n}\n";
    let outcome = apply_mutation_for_language(
        Language::CSharp,
        MutationCategory::DeleteTestCall,
        src,
        MutationConfig::default(),
    )
    .unwrap();
    assert!(outcome
        .mutated_source
        .contains("void assert(bool condition)"));
    assert!(!outcome.mutated_source.contains("assert(total >= 0);"));
}

#[test]
fn subtle_flip_skips_bitshift_operators() {
    // Regression: `<<` and `>>` must not surface as single-char `<`/`>`.
    let src = "MASK = 1 << 4\nSHIFT = 8 >> 2\nflag = MASK == SHIFT\n";
    let outcome =
        apply_mutation(MutationCategory::SubtleFlip, src, MutationConfig::default()).unwrap();
    let site = outcome.mutation_site.unwrap();
    assert_eq!(
        site.original_text, "==",
        "only `==` should be a candidate, not `<<`/`>>`"
    );
}

#[test]
fn off_by_one_skips_fractional_part_of_floats() {
    // Regression: `3.14` must NOT yield `14` as a candidate.
    let src = "PI = 3.14\nN = 7\n";
    let outcome =
        apply_mutation(MutationCategory::OffByOne, src, MutationConfig::default()).unwrap();
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
