use super::*;
use std::collections::BTreeSet;

#[test]
fn routing_covers_all_entity_types() {
    for entity_type in EntityType::all() {
        assert!(
            !route_to_embedders(entity_type).is_empty(),
            "{entity_type:?}"
        );
    }
}

#[test]
fn routing_ids_cover_all_21_embedders_with_dimensions() {
    let ids = EmbedderId::all();
    assert_eq!(ids.len(), 21);
    assert_eq!(ids[0], EmbedderId::E1);
    assert_eq!(ids[20], EmbedderId::E21);
    assert_eq!(EmbedderId::E7.projected_dimension(), 1536);
    assert_eq!(EmbedderId::E14.projected_dimension(), 1024);
    assert_eq!(EmbedderId::E17.projected_dimension(), 384);
    assert_eq!(EmbedderId::E19.projected_dimension(), 64);
    assert!(EmbedderId::E14.is_content());
    assert!(!EmbedderId::E5.is_content());
    assert!(!EmbedderId::E15.is_content());

    for id in ids {
        let text = serde_json::to_string(&id).unwrap();
        let readback: EmbedderId = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&readback).unwrap(), text);
    }
}

#[test]
fn routing_table_is_static_complete_and_unique() {
    let rows = routing_table_entries().collect::<Vec<_>>();
    assert_eq!(rows.len(), 19);
    let unique_keys = rows
        .iter()
        .map(|(key, language, _)| (*key, *language))
        .collect::<BTreeSet<_>>();
    assert_eq!(unique_keys.len(), rows.len());
    assert!(rows.iter().all(|(_, _, result)| match result {
        RoutingResult::Embedders(ids) => !ids.is_empty(),
        RoutingResult::HandledByInstrument(_) => true,
    }));
}

#[test]
fn routing_table_matches_embedder_spec() {
    assert_eq!(
        route_for_key(
            RoutingKey::Entity(EntityType::Function),
            Some(Language::Python)
        )
        .unwrap(),
        RoutingResult::Embedders(BTreeSet::from([EmbedderId::E7, EmbedderId::E14]))
    );
    assert_eq!(
        route_for_key(RoutingKey::FunctionSignature, Some(Language::Rust)).unwrap(),
        RoutingResult::Embedders(BTreeSet::from([EmbedderId::E1, EmbedderId::E10]))
    );
    assert_eq!(
        route_for_key(
            RoutingKey::Entity(EntityType::TestFunction),
            Some(Language::Go)
        )
        .unwrap(),
        RoutingResult::Embedders(BTreeSet::from([EmbedderId::E1, EmbedderId::E7]))
    );
    assert_eq!(
        route_for_key(RoutingKey::TestAssertion, Some(Language::Java)).unwrap(),
        RoutingResult::Embedders(BTreeSet::from([EmbedderId::E7, EmbedderId::E12]))
    );
    assert_eq!(
        route_for_key(RoutingKey::DiffHunk, None).unwrap(),
        RoutingResult::HandledByInstrument(DirectInstrument::EDiff)
    );
    assert_eq!(
        route_for_key(RoutingKey::AstNodeSequence, None).unwrap(),
        RoutingResult::HandledByInstrument(DirectInstrument::EAst)
    );
    assert_eq!(
        route_for_key(RoutingKey::CfgBasicBlock, None).unwrap(),
        RoutingResult::HandledByInstrument(DirectInstrument::ECfg)
    );
    assert_eq!(
        route_for_key(RoutingKey::DefUseEdge, None).unwrap(),
        RoutingResult::HandledByInstrument(DirectInstrument::EDataFlow)
    );
}

#[test]
fn routing_table_covers_every_language_entity_pair() {
    for language in Language::all() {
        for entity_type in EntityType::all() {
            match route_for_entity_type(entity_type, Some(language)).unwrap() {
                RoutingResult::Embedders(ids) => {
                    assert!(!ids.is_empty(), "{language:?} {entity_type:?}");
                    assert!(ids.iter().all(|id| id.projected_dimension() > 0));
                }
                RoutingResult::HandledByInstrument(_) => {}
            }
        }
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
    assert!(chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Import));
    assert!(chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Class));
    assert!(chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Method));
    assert!(chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::TestFunction));
}

#[test]
fn test_path_helpers_keep_production_entity_types() {
    let js = b"class Solver { compute(x) { return x + 1; } }\nfunction helper(x) { return x + 2; }\nfunction testCompute() { return helper(1) === 3; }\n";
    let js_chunks = chunk_with_options(
        js,
        Language::JavaScript,
        &AstChunkOptions::for_path("tests/sample.test.js"),
    )
    .unwrap();
    assert!(js_chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Method));
    assert!(js_chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Function));
    assert!(js_chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::TestFunction));

    let java = b"public class TestSample { int compute(int x) { return x + 1; } void testCompute() { if (compute(1) != 2) throw new RuntimeException(); } }\n";
    let java_chunks = chunk_with_options(
        java,
        Language::Java,
        &AstChunkOptions::for_path("src/test/java/TestSample.java"),
    )
    .unwrap();
    assert!(java_chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::Method));
    assert!(java_chunks
        .iter()
        .any(|chunk| chunk.entity_type == EntityType::TestFunction));

    let csharp = b"namespace Fsv; public class Solver { public int Compute(int x) { return x + 1; } public int TestimonyScore() { return 0; } public void TestCompute() { if (Compute(1) != 2) throw new System.Exception(\"bad\"); } }\n";
    let csharp_chunks = chunk_with_options(
        csharp,
        Language::CSharp,
        &AstChunkOptions::for_path("Tests/SolverTests.cs"),
    )
    .unwrap();
    assert!(csharp_chunks.iter().any(|chunk| {
        chunk.content.contains("TestimonyScore") && chunk.entity_type == EntityType::Method
    }));
    assert!(!csharp_chunks.iter().any(|chunk| {
        chunk.content.contains("TestimonyScore") && chunk.entity_type == EntityType::TestFunction
    }));
    assert!(csharp_chunks
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
