use context_graph_core::memory::ast::{
    chunk_with_options, AstChunkOptions, Language as AstLanguage,
};
use context_graph_mejepa_corpus::{
    apply_mutation_for_language, Language, MutationCategory, MutationConfig, MutationOutcome,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
struct Fixture {
    language: Language,
    extension: &'static str,
    source: &'static str,
    alternate: &'static str,
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-multilang-fsv");
    reset_root(&root)?;
    let source_root = root.join("source_of_truth_sources");
    fs::create_dir_all(&source_root)?;
    fs::set_permissions(&source_root, fs::Permissions::from_mode(0o700))?;

    let mut rows = Vec::new();
    for fixture in fixtures() {
        let baseline_chunks = assert_parseable(fixture.language, fixture.source)?;
        for category in MutationCategory::all() {
            let before = json!({
                "language": fixture.language.slug(),
                "category": category.slug(),
                "source_sha256": sha256_hex(fixture.source.as_bytes()),
                "source_len": fixture.source.len(),
                "baseline_chunk_count": baseline_chunks,
            });
            let config = MutationConfig {
                seed: 17,
                alternate_source: (category == MutationCategory::WrongFile)
                    .then(|| fixture.alternate.to_string()),
            };
            let outcome =
                apply_mutation_for_language(fixture.language, category, fixture.source, config)?;
            verify_outcome(&fixture, category, &outcome)?;
            let file_path = source_root.join(format!(
                "{}-{}.{}",
                fixture.language.slug(),
                category.slug(),
                fixture.extension
            ));
            write_checked(&file_path, outcome.mutated_source.as_bytes())?;
            let readback = fs::read_to_string(&file_path)?;
            require(
                "mutated source file readback equality",
                readback == outcome.mutated_source,
            )?;
            let after = json!({
                "path": file_path,
                "readback_sha256": sha256_hex(readback.as_bytes()),
                "readback_len": readback.len(),
                "parseable_after_mutation": category != MutationCategory::CompileError,
                "mutation_site": outcome.mutation_site,
            });
            rows.push(json!({
                "case": "happy_path_multilang_operator",
                "before_state": before,
                "trigger": "apply_mutation_for_language",
                "after_state": after,
                "passes": true,
            }));
        }
    }

    let edge_cases = vec![
        edge_empty_input(&source_root)?,
        edge_wrong_file_missing_alternate(&source_root)?,
        edge_comment_and_string_masking(&source_root)?,
    ];
    let evidence = json!({
        "source_of_truth": {
            "generated_sources_dir": source_root,
            "evidence_json": root.join("multilang-mutation-evidence.json"),
            "operator_count": MutationCategory::all().len(),
            "language_count": fixtures().len(),
        },
        "happy_path_rows": rows,
        "edge_cases": edge_cases,
        "passes": true,
    });
    let evidence_path = root.join("multilang-mutation-evidence.json");
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require("evidence JSON readback equality", readback == evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn verify_outcome(
    fixture: &Fixture,
    category: MutationCategory,
    outcome: &MutationOutcome,
) -> FsvResult<()> {
    require(
        "outcome category matches trigger",
        outcome.category == category,
    )?;
    match category {
        MutationCategory::KnownGood => {
            require(
                "KnownGood byte-echoes source",
                outcome.mutated_source == fixture.source,
            )?;
            require(
                "KnownGood has no mutation site",
                outcome.mutation_site.is_none(),
            )?;
            assert_parseable(fixture.language, &outcome.mutated_source)?;
        }
        MutationCategory::WrongFile => {
            require(
                "WrongFile uses alternate source",
                outcome.mutated_source == fixture.alternate,
            )?;
            require(
                "WrongFile has no mutation site",
                outcome.mutation_site.is_none(),
            )?;
            assert_parseable(fixture.language, &outcome.mutated_source)?;
        }
        MutationCategory::CompileError => {
            require(
                "CompileError changes source",
                outcome.mutated_source != fixture.source,
            )?;
            require(
                "CompileError reports append site",
                outcome.mutation_site.is_some(),
            )?;
            require(
                "CompileError source is rejected by AST parser",
                assert_parseable(fixture.language, &outcome.mutated_source).is_err(),
            )?;
        }
        _ => {
            require(
                "mutation changes source",
                outcome.mutated_source != fixture.source,
            )?;
            require("mutation reports site", outcome.mutation_site.is_some())?;
            assert_parseable(fixture.language, &outcome.mutated_source)?;
        }
    }
    Ok(())
}

fn edge_empty_input(source_root: &Path) -> FsvResult<Value> {
    let before =
        json!({"language": "rust", "input_len": 0, "files_before": file_count(source_root)?});
    let err = apply_mutation_for_language(
        Language::Rust,
        MutationCategory::KnownGood,
        "",
        MutationConfig::default(),
    )
    .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(source_root)?});
    require(
        "empty input fails closed",
        err.code() == "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    require(
        "empty input writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_empty_input",
        "before_state": before,
        "trigger": "apply Rust KnownGood to empty source",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_wrong_file_missing_alternate(source_root: &Path) -> FsvResult<Value> {
    let fixture = fixtures()
        .into_iter()
        .find(|fixture| fixture.language == Language::JavaScript)
        .ok_or("JavaScript fixture missing")?;
    let before = json!({
        "language": fixture.language.slug(),
        "category": MutationCategory::WrongFile.slug(),
        "alternate_source": null,
        "files_before": file_count(source_root)?,
    });
    let err = apply_mutation_for_language(
        fixture.language,
        MutationCategory::WrongFile,
        fixture.source,
        MutationConfig::default(),
    )
    .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(source_root)?});
    require(
        "WrongFile without alternate fails closed",
        err.code() == "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    require(
        "WrongFile edge writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_wrong_file_missing_alternate",
        "before_state": before,
        "trigger": "apply JavaScript WrongFile without alternate_source",
        "after_state": after,
        "passes": true,
    }))
}

fn edge_comment_and_string_masking(source_root: &Path) -> FsvResult<Value> {
    let source = "fn note() {\n    let text = \"a == b && true\"; // false != true\n}\n";
    let before = json!({
        "language": "rust",
        "source_sha256": sha256_hex(source.as_bytes()),
        "files_before": file_count(source_root)?,
    });
    let err = apply_mutation_for_language(
        Language::Rust,
        MutationCategory::SubtleFlip,
        source,
        MutationConfig::default(),
    )
    .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(source_root)?});
    require(
        "operators do not mutate comments or strings",
        err.code() == "MEJEPA_CORPUS_NO_MUTATION_SITE",
    )?;
    require(
        "masking edge writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_comment_and_string_masking",
        "before_state": before,
        "trigger": "apply Rust SubtleFlip where every operator-looking token is inside a string/comment",
        "after_state": after,
        "passes": true,
    }))
}

fn assert_parseable(language: Language, source: &str) -> FsvResult<usize> {
    let chunks = chunk_with_options(
        source.as_bytes(),
        ast_language(language),
        &AstChunkOptions::for_path(default_path(language)),
    )?;
    Ok(chunks.len())
}

fn ast_language(language: Language) -> AstLanguage {
    match language {
        Language::Rust => AstLanguage::Rust,
        Language::Python => AstLanguage::Python,
        Language::JavaScript => AstLanguage::JavaScript,
        Language::TypeScript => AstLanguage::TypeScript,
        Language::Go => AstLanguage::Go,
        Language::Java => AstLanguage::Java,
        Language::C => AstLanguage::C,
        Language::Cpp => AstLanguage::Cpp,
        Language::CSharp => AstLanguage::CSharp,
        Language::Ruby => AstLanguage::Ruby,
        Language::Php => AstLanguage::Php,
    }
}

fn default_path(language: Language) -> &'static str {
    match language {
        Language::Rust => "mutation.rs",
        Language::Python => "mutation.py",
        Language::JavaScript => "mutation.js",
        Language::TypeScript => "mutation.ts",
        Language::Go => "mutation.go",
        Language::Java => "Mutation.java",
        Language::C => "mutation.c",
        Language::Cpp => "mutation.cpp",
        Language::CSharp => "Mutation.cs",
        Language::Ruby => "mutation.rb",
        Language::Php => "mutation.php",
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            language: Language::Rust,
            extension: "rs",
            source: "fn check(alpha: i32, beta: i32) -> bool {\n    let total = alpha + beta + 3;\n    assert!(total >= 0);\n    total == 5 && true\n}\n",
            alternate: "fn alternate(alpha: i32) -> bool {\n    alpha == 1 && true\n}\n",
        },
        Fixture {
            language: Language::Python,
            extension: "py",
            source: "def check(alpha, beta):\n    total = alpha + beta + 3\n    expected = 5\n    assert total >= 0\n    return total == expected and True\n",
            alternate: "def alternate(alpha):\n    return alpha == 1 and True\n",
        },
        Fixture {
            language: Language::JavaScript,
            extension: "js",
            source: "function check(alpha, beta) {\n  const total = alpha + beta + 3;\n  console.assert(total >= 0);\n  return total == 5 && true;\n}\n",
            alternate: "function alternate(alpha) {\n  return alpha == 1 && true;\n}\n",
        },
        Fixture {
            language: Language::TypeScript,
            extension: "ts",
            source: "function check(alpha: number, beta: number): boolean {\n  const total = alpha + beta + 3;\n  console.assert(total >= 0);\n  return total == 5 && true;\n}\n",
            alternate: "function alternate(alpha: number): boolean {\n  return alpha == 1 && true;\n}\n",
        },
        Fixture {
            language: Language::Go,
            extension: "go",
            source: "package main\n\nfunc check(alpha int, beta int) bool {\n    total := alpha + beta + 3\n    if total >= 0 { t.Fatal(\"should stay\") }\n    return total == 5 && true\n}\n",
            alternate: "package main\n\nfunc alternate(alpha int) bool {\n    return alpha == 1 && true\n}\n",
        },
        Fixture {
            language: Language::Java,
            extension: "java",
            source: "class Mutation {\n  boolean check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    assert total >= 0;\n    return total == 5 && true;\n  }\n}\n",
            alternate: "class MutationAlt {\n  boolean alternate(int alpha) { return alpha == 1 && true; }\n}\n",
        },
        Fixture {
            language: Language::C,
            extension: "c",
            source: "int check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    assert(total >= 0);\n    return total == 5 && 1;\n}\n",
            alternate: "int alternate(int alpha) {\n    return alpha == 1 && 1;\n}\n",
        },
        Fixture {
            language: Language::Cpp,
            extension: "cpp",
            source: "bool check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    assert(total >= 0);\n    return total == 5 && true;\n}\n",
            alternate: "bool alternate(int alpha) {\n    return alpha == 1 && true;\n}\n",
        },
        Fixture {
            language: Language::CSharp,
            extension: "cs",
            source: "class Mutation {\n  bool Check(int alpha, int beta) {\n    int total = alpha + beta + 3;\n    assert(total >= 0);\n    return total == 5 && true;\n  }\n}\n",
            alternate: "class MutationAlt {\n  bool Alternate(int alpha) { return alpha == 1 && true; }\n}\n",
        },
        Fixture {
            language: Language::Ruby,
            extension: "rb",
            source: "def check(alpha, beta)\n  total = alpha + beta + 3\n  assert total >= 0\n  total == 5 && true\nend\n",
            alternate: "def alternate(alpha)\n  alpha == 1 && true\nend\n",
        },
        Fixture {
            language: Language::Php,
            extension: "php",
            source: "<?php\nfunction check($alpha, $beta) {\n    $total = $alpha + $beta + 3;\n    assert($total >= 0);\n    return $total == 5 && true;\n}\n",
            alternate: "<?php\nfunction alternate($alpha) {\n    return $alpha == 1 && true;\n}\n",
        },
    ]
}

fn reset_root(root: &Path) -> FsvResult<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    require("byte readback equality", fs::read(path)? == bytes)?;
    Ok(())
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    require("JSON byte readback equality", fs::read(path)? == bytes)?;
    Ok(())
}

fn file_count(path: &Path) -> FsvResult<usize> {
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::read_dir(path)?.count())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("multilang mutation FSV assertion failed: {label}").into())
    }
}
