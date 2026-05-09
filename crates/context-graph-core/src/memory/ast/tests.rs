use super::*;

#[test]
fn routing_covers_all_entity_types() {
    for entity_type in EntityType::all() {
        assert!(!route_to_embedders(entity_type).is_empty(), "{entity_type:?}");
    }
}

#[test]
fn python_chunks_test_method_import_and_class() {
    let source = b"import os\n\nclass Solver:\n    def compute(self):\n        return 1\n\ndef test_compute():\n    assert Solver().compute() == 1\n";
    let chunks = chunk_with_options(
        source,
        Language::Python,
        &AstChunkOptions::for_path("tests/test_solver.py"),
    )
    .unwrap();
    assert!(chunks.iter().any(|chunk| chunk.entity_type == EntityType::Import));
    assert!(chunks.iter().any(|chunk| chunk.entity_type == EntityType::Class));
    assert!(chunks.iter().any(|chunk| chunk.entity_type == EntityType::Method));
    assert!(chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::TestFunction));
}

#[test]
fn python_hash_ignores_whitespace_formatting() {
    let a = b"def add(a, b):\n    return a + b\n";
    let b = b"def add(a,b):\n  return a+b\n";
    let opts = AstChunkOptions::for_path("pkg/math.py");
    let ca = chunk_with_options(a, Language::Python, &opts).unwrap();
    let cb = chunk_with_options(b, Language::Python, &opts).unwrap();
    let fa = ca
        .iter()
        .find(|chunk| chunk.entity_type == EntityType::Function)
        .unwrap();
    let fb = cb
        .iter()
        .find(|chunk| chunk.entity_type == EntityType::Function)
        .unwrap();
    assert_eq!(fa.sha256, fb.sha256);
}

#[test]
fn invalid_python_fails_closed() {
    let err = chunk_with_options(
        b"def broken(:\n    return 1\n",
        Language::Python,
        &AstChunkOptions::for_path("pkg/broken.py"),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CHUNKER_PARSE_FAILED");
}

#[test]
fn php_inline_html_fails_closed() {
    let err = chunk_with_options(
        b"<?php function ok() { return 1; ?> <html></html>",
        Language::Php,
        &AstChunkOptions::for_path("index.php"),
    )
    .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CHUNKER_MIXED_LANGUAGE");
}
