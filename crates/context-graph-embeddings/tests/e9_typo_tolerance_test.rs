//! E9 typo tolerance verification test

use context_graph_embeddings::models::custom::HdcModel;
use context_graph_embeddings::traits::EmbeddingModel;
use context_graph_embeddings::types::ModelInput;

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }
    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[test]
fn test_e9_typo_tolerance_single_word() {
    let model = HdcModel::default_model();

    // Test: authentication vs authetication (1 char missing)
    let correct = "authentication";
    let typo = "authetication";

    let hv1 = model.encode_text(correct);
    let hv2 = model.encode_text(typo);

    // Native HDC similarity
    let native_sim = HdcModel::similarity(&hv1, &hv2);

    // Projected vectors
    let proj1 = model.project_to_float(&hv1);
    let proj2 = model.project_to_float(&hv2);
    let cosine_sim = cosine_similarity(&proj1, &proj2);

    println!("\n=== E9 Single Word Typo Tolerance ===");
    println!("Correct: '{}' | Typo: '{}'", correct, typo);
    println!("Native Hamming Similarity: {:.4}", native_sim);
    println!("Projected Cosine Similarity: {:.4}", cosine_sim);

    // Typo tolerance: both should be reasonably high (> 0.7)
    assert!(
        native_sim > 0.7,
        "Native similarity {} should be > 0.7",
        native_sim
    );
    // Cosine may be lower but should still show similarity (> 0.5)
    assert!(
        cosine_sim > 0.5,
        "Cosine similarity {} should be > 0.5",
        cosine_sim
    );
}

#[test]
fn test_e9_typo_tolerance_full_query() {
    let model = HdcModel::default_model();

    let correct = "authentication postgresql database";
    let typo = "authetication postgresq databse";

    let hv1 = model.encode_text(correct);
    let hv2 = model.encode_text(typo);

    let native_sim = HdcModel::similarity(&hv1, &hv2);

    let proj1 = model.project_to_float(&hv1);
    let proj2 = model.project_to_float(&hv2);
    let cosine_sim = cosine_similarity(&proj1, &proj2);

    println!("\n=== E9 Full Query Typo Tolerance ===");
    println!("Correct: '{}'", correct);
    println!("Typo:    '{}'", typo);
    println!("Native Hamming Similarity: {:.4}", native_sim);
    println!("Projected Cosine Similarity: {:.4}", cosine_sim);

    // With multiple typos, we expect lower but still reasonable similarity
    assert!(
        native_sim > 0.6,
        "Native similarity {} should be > 0.6",
        native_sim
    );
    assert!(
        cosine_sim > 0.4,
        "Cosine similarity {} should be > 0.4",
        cosine_sim
    );
}

#[tokio::test]
async fn test_e9_embedding_residual_preserves_typo_tolerance() {
    let model = HdcModel::default_model();

    let correct = model
        .embed(&ModelInput::text("authentication").unwrap())
        .await
        .unwrap();
    let typo = model
        .embed(&ModelInput::text("authetication").unwrap())
        .await
        .unwrap();

    let cosine_sim = cosine_similarity(&correct.vector, &typo.vector);
    assert!(
        cosine_sim > 0.5,
        "residual embedding cosine similarity {} should remain > 0.5",
        cosine_sim
    );
}

#[test]
fn test_e9_query_vs_document() {
    // Test how query with typos matches stored document
    let model = HdcModel::default_model();

    let document = "The authentication system uses PostgreSQL database for secure credential storage. User passwords are hashed using bcrypt algorithm with proper salting techniques.";
    let query = "authetication postgresq databse"; // typos

    let hv_doc = model.encode_text(document);
    let hv_query = model.encode_text(query);

    let native_sim = HdcModel::similarity(&hv_doc, &hv_query);

    let proj_doc = model.project_to_float(&hv_doc);
    let proj_query = model.project_to_float(&hv_query);
    let cosine_sim = cosine_similarity(&proj_doc, &proj_query);

    println!("\n=== E9 Query vs Document Typo Match ===");
    println!(
        "Document: '{}'...",
        document.chars().take(50).collect::<String>()
    );
    println!("Query:    '{}'", query);
    println!("Native Hamming Similarity: {:.4}", native_sim);
    println!("Projected Cosine Similarity: {:.4}", cosine_sim);

    // Query vs document will have lower similarity due to length difference
    // but should still be non-trivial if the key terms match
    println!("(Note: Lower similarity expected due to document length difference)");
}
