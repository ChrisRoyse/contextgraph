//! Code capture service for embedding and storing code entities.
//!
//! This module provides the CodeCaptureService which:
//! 1. Converts CodeChunks (from ASTChunker) to CodeEntities (for storage)
//! 2. Calls ALL 13 embedders to generate full SemanticFingerprint
//! 3. Stores entities and fingerprints in CodeStore (separate database)
//! 4. Provides search capabilities using all 13 embedding spaces
//!
//! # Architecture
//!
//! ```text
//! ASTChunker → CodeChunk → CodeCaptureService → CodeEntity + SemanticFingerprint → CodeStore
//! ```
//!
//! # Constitution Compliance
//! - ARCH-01: "TeleologicalArray is atomic - all 13 embeddings or nothing"
//! - ARCH-05: "All 13 embedders required - missing = fatal"
//! - Code embeddings are stored in SEPARATE database from teleological memories
//! - But each code entity gets the FULL 13-embedder treatment
//! - E7 (V_correctness) provides code-specific patterns, other embedders provide context

use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, info, instrument};
use uuid::Uuid;

use super::ast_chunker::{CodeChunk, EntityType as ChunkEntityType};
use crate::types::fingerprint::SemanticFingerprint;
use crate::types::{CodeEntity, CodeEntityType, CodeLanguage};

/// Errors from code embedding operations.
#[derive(Debug, Clone, Error)]
pub enum CodeEmbedderError {
    /// Embedding service is not available.
    #[error("Code embedding service unavailable")]
    Unavailable,

    /// Embedding computation failed.
    #[error("Code embedding computation failed: {message}")]
    ComputationFailed { message: String },

    /// Input is invalid for embedding.
    #[error("Invalid input for code embedding: {reason}")]
    InvalidInput { reason: String },

    /// Model not loaded.
    #[error("E7 model not loaded")]
    ModelNotLoaded,
}

/// Errors from code capture operations.
#[derive(Debug, Error)]
pub enum CodeCaptureError {
    /// Content is empty.
    #[error("Code content is empty")]
    EmptyContent,

    /// Embedding operation failed.
    #[error("Code embedding failed: {0}")]
    EmbeddingFailed(#[from] CodeEmbedderError),

    /// Storage operation failed.
    #[error("Code storage failed: {0}")]
    StorageFailed(String),

    /// AST chunking failed.
    #[error("AST chunking failed: {0}")]
    ChunkingFailed(String),

    /// File not found.
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    /// IO error.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for code capture operations.
pub type CodeCaptureResult<T> = Result<T, CodeCaptureError>;

/// Trait for code embedding providers.
///
/// Implementations MUST produce full 13-embedding SemanticFingerprint for code content.
/// Per constitution (ARCH-01, ARCH-05): All 13 embedders are required, including E7
/// (Qodo-Embed-1-1.5B) which is specifically designed for code.
///
/// # Constitution Compliance
///
/// - ARCH-01: "TeleologicalArray is atomic - all 13 embeddings or nothing"
/// - ARCH-05: "All 13 embedders required - missing = fatal"
/// - E7 (V_correctness) provides code-specific patterns
/// - Other embedders (E1 semantic, E5 causal, etc.) provide context
#[async_trait]
pub trait CodeEmbeddingProvider: Send + Sync {
    /// Embed code content into a full 13-embedding SemanticFingerprint.
    ///
    /// # Arguments
    /// * `code` - The code content to embed
    /// * `context` - Optional context (file path, scope chain, etc.)
    ///
    /// # Returns
    /// Complete SemanticFingerprint with all 13 embeddings on success.
    async fn embed_code(&self, code: &str, context: Option<&str>) -> Result<SemanticFingerprint, CodeEmbedderError>;

    /// Embed a batch of code snippets.
    ///
    /// More efficient than calling embed_code multiple times.
    async fn embed_batch(&self, codes: &[(&str, Option<&str>)]) -> Result<Vec<SemanticFingerprint>, CodeEmbedderError>;

    /// Check if all 13 embedders are initialized and ready.
    fn is_ready(&self) -> bool;
}

/// Trait for code storage backends.
///
/// Abstracts over the actual storage implementation (CodeStore).
/// Stores full SemanticFingerprint (all 13 embeddings) per code entity.
///
/// # Constitution Compliance
///
/// - ARCH-01: "TeleologicalArray is atomic - all 13 embeddings or nothing"
/// - Code entities stored in SEPARATE database from teleological memories
/// - But each entity gets the FULL 13-embedder treatment
#[async_trait]
pub trait CodeStorage: Send + Sync {
    /// Store a code entity with its full SemanticFingerprint.
    ///
    /// # Arguments
    /// * `entity` - The code entity to store
    /// * `fingerprint` - Complete 13-embedding fingerprint
    async fn store(&self, entity: &CodeEntity, fingerprint: &SemanticFingerprint) -> Result<(), String>;

    /// Get an entity by ID.
    async fn get(&self, id: Uuid) -> Result<Option<CodeEntity>, String>;

    /// Get entities by file path.
    async fn get_by_file(&self, file_path: &str) -> Result<Vec<CodeEntity>, String>;

    /// Delete all entities for a file.
    async fn delete_file(&self, file_path: &str) -> Result<usize, String>;

    /// Get the full SemanticFingerprint for an entity.
    async fn get_fingerprint(&self, id: Uuid) -> Result<Option<SemanticFingerprint>, String>;

    /// Search entities by embedding similarity in a specific space.
    ///
    /// # Arguments
    /// * `query_fingerprint` - Query fingerprint (uses E7 for code-specific, E1 for semantic)
    /// * `top_k` - Maximum number of results to return
    /// * `min_similarity` - Minimum similarity threshold (0.0 to 1.0)
    /// * `use_e7_primary` - If true, use E7 (code) as primary; else use E1 (semantic)
    ///
    /// # Returns
    /// Vector of (entity, similarity_score) pairs, sorted by decreasing similarity.
    async fn search_by_fingerprint(
        &self,
        query_fingerprint: &SemanticFingerprint,
        top_k: usize,
        min_similarity: f32,
        use_e7_primary: bool,
    ) -> Result<Vec<(CodeEntity, f32)>, String>;
}

/// Code capture service for embedding and storing code.
///
/// This is the main entry point for the code embedding pipeline.
/// It coordinates between the AST chunker, E7 embedder, and code storage.
pub struct CodeCaptureService<E: CodeEmbeddingProvider, S: CodeStorage> {
    /// Code embedding provider (E7).
    embedder: Arc<E>,
    /// Code storage backend.
    storage: Arc<S>,
    /// Session ID for tracking (used for logging and future session-scoped queries).
    #[allow(dead_code)]
    session_id: String,
}

impl<E: CodeEmbeddingProvider, S: CodeStorage> CodeCaptureService<E, S> {
    /// Create a new code capture service.
    pub fn new(embedder: Arc<E>, storage: Arc<S>, session_id: String) -> Self {
        Self {
            embedder,
            storage,
            session_id,
        }
    }

    /// Capture a code chunk and store it.
    ///
    /// Converts the CodeChunk to a CodeEntity, generates full 13-embedding
    /// SemanticFingerprint, and stores both in the code storage.
    ///
    /// # Returns
    /// The UUID of the stored entity.
    #[instrument(skip(self, chunk), fields(file = %chunk.metadata.file_path, lines = %format!("{}-{}", chunk.metadata.start_line, chunk.metadata.end_line)))]
    pub async fn capture_chunk(&self, chunk: CodeChunk) -> CodeCaptureResult<Uuid> {
        if chunk.code.trim().is_empty() {
            return Err(CodeCaptureError::EmptyContent);
        }

        // Convert chunk to entity
        let entity = self.chunk_to_entity(chunk.clone());
        let id = entity.id;

        // Generate full 13-embedding fingerprint from contextualized text
        let fingerprint = self
            .embedder
            .embed_code(&chunk.contextualized_text, None)
            .await?;

        // Store entity and fingerprint
        self.storage
            .store(&entity, &fingerprint)
            .await
            .map_err(CodeCaptureError::StorageFailed)?;

        debug!(
            id = %id,
            name = %entity.name,
            entity_type = %entity.entity_type,
            "Captured code entity with full 13-embedding fingerprint"
        );

        Ok(id)
    }

    /// Capture multiple code chunks in batch.
    ///
    /// More efficient than calling capture_chunk for each chunk.
    /// Each chunk receives a full 13-embedding fingerprint.
    #[instrument(skip(self, chunks), fields(count = chunks.len()))]
    pub async fn capture_batch(&self, chunks: Vec<CodeChunk>) -> CodeCaptureResult<Vec<Uuid>> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Convert chunks to entities
        let entities: Vec<CodeEntity> = chunks
            .iter()
            .filter(|c| !c.code.trim().is_empty())
            .map(|c| self.chunk_to_entity(c.clone()))
            .collect();

        if entities.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare batch for embedding
        let contexts: Vec<(&str, Option<&str>)> = chunks
            .iter()
            .filter(|c| !c.code.trim().is_empty())
            .map(|c| (c.contextualized_text.as_str(), None))
            .collect();

        // Generate full 13-embedding fingerprints in batch
        let fingerprints = self.embedder.embed_batch(&contexts).await?;

        // Store entities and fingerprints
        let mut ids = Vec::with_capacity(entities.len());
        for (entity, fingerprint) in entities.iter().zip(fingerprints.iter()) {
            self.storage
                .store(entity, fingerprint)
                .await
                .map_err(CodeCaptureError::StorageFailed)?;
            ids.push(entity.id);
        }

        info!(
            captured = ids.len(),
            file = %chunks.first().map(|c| c.metadata.file_path.as_str()).unwrap_or("unknown"),
            "Captured code entities batch with 13-embedding fingerprints"
        );

        Ok(ids)
    }

    /// Delete all entities for a file.
    ///
    /// Called when a file is deleted or before re-indexing.
    #[instrument(skip(self), fields(file = %file_path))]
    pub async fn delete_by_file(&self, file_path: &str) -> CodeCaptureResult<usize> {
        let deleted = self
            .storage
            .delete_file(file_path)
            .await
            .map_err(CodeCaptureError::StorageFailed)?;

        if deleted > 0 {
            info!(file = %file_path, deleted = deleted, "Deleted code entities for file");
        }

        Ok(deleted)
    }

    /// Get an entity by ID.
    pub async fn get(&self, id: Uuid) -> CodeCaptureResult<Option<CodeEntity>> {
        self.storage
            .get(id)
            .await
            .map_err(CodeCaptureError::StorageFailed)
    }

    /// Get all entities for a file.
    pub async fn get_by_file(&self, file_path: &str) -> CodeCaptureResult<Vec<CodeEntity>> {
        self.storage
            .get_by_file(file_path)
            .await
            .map_err(CodeCaptureError::StorageFailed)
    }

    /// Convert a CodeChunk to a CodeEntity.
    fn chunk_to_entity(&self, chunk: CodeChunk) -> CodeEntity {
        let entity_type = Self::convert_entity_type(chunk.metadata.entity_type);
        let language = Self::language_from_string(&chunk.metadata.language);

        // Extract name from scope chain or use default
        let name = chunk
            .metadata
            .scope_chain
            .last()
            .cloned()
            .unwrap_or_else(|| format!("anonymous_{}", chunk.metadata.start_line));

        let mut entity = CodeEntity::new(
            entity_type,
            name,
            chunk.code,
            language,
            chunk.metadata.file_path,
            chunk.metadata.start_line as usize,
            chunk.metadata.end_line as usize,
        );

        // Add optional metadata
        if let Some(sig) = chunk.metadata.entity_signature {
            entity = entity.with_signature(sig);
        }

        if let Some(parent) = chunk.metadata.parent_definition {
            entity = entity.with_parent_type(parent);
        }

        // Set module path from scope chain
        if chunk.metadata.scope_chain.len() > 1 {
            let module_path = chunk.metadata.scope_chain[..chunk.metadata.scope_chain.len() - 1].join("::");
            entity = entity.with_module_path(module_path);
        }

        entity
    }

    /// Convert AST chunker EntityType to CodeEntityType.
    fn convert_entity_type(chunk_type: ChunkEntityType) -> CodeEntityType {
        match chunk_type {
            ChunkEntityType::Function => CodeEntityType::Function,
            ChunkEntityType::Method => CodeEntityType::Method,
            ChunkEntityType::Struct => CodeEntityType::Struct,
            ChunkEntityType::Enum => CodeEntityType::Enum,
            ChunkEntityType::Trait => CodeEntityType::Trait,
            ChunkEntityType::Impl => CodeEntityType::Impl,
            ChunkEntityType::Module => CodeEntityType::Module,
            ChunkEntityType::Const => CodeEntityType::Const,
            ChunkEntityType::Static => CodeEntityType::Static,
            ChunkEntityType::TypeAlias => CodeEntityType::TypeAlias,
            ChunkEntityType::Macro => CodeEntityType::Macro,
            ChunkEntityType::Comment => CodeEntityType::Function, // Fallback for doc comments
            ChunkEntityType::Mixed => CodeEntityType::Function,   // Fallback for merged chunks
            ChunkEntityType::Unknown => CodeEntityType::Function, // Fallback
        }
    }

    /// Convert language string to CodeLanguage enum.
    fn language_from_string(lang: &str) -> CodeLanguage {
        match lang.to_lowercase().as_str() {
            "rust" => CodeLanguage::Rust,
            "python" => CodeLanguage::Python,
            "typescript" => CodeLanguage::TypeScript,
            "javascript" => CodeLanguage::JavaScript,
            "go" => CodeLanguage::Go,
            "java" => CodeLanguage::Java,
            "cpp" | "c++" => CodeLanguage::Cpp,
            "c" => CodeLanguage::C,
            "sql" => CodeLanguage::Sql,
            "toml" => CodeLanguage::Toml,
            "yaml" => CodeLanguage::Yaml,
            _ => CodeLanguage::Unknown,
        }
    }
}

/// Search result from code search.
#[derive(Debug, Clone)]
pub struct CodeSearchResult {
    /// The matched entity.
    pub entity: CodeEntity,
    /// Similarity score (0.0 to 1.0).
    pub score: f32,
    /// Full fingerprint (if requested).
    pub fingerprint: Option<SemanticFingerprint>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// Mock embedding provider for testing.
    /// Returns zeroed SemanticFingerprint for all code.
    struct MockCodeEmbedder;

    #[async_trait]
    impl CodeEmbeddingProvider for MockCodeEmbedder {
        async fn embed_code(&self, _code: &str, _context: Option<&str>) -> Result<SemanticFingerprint, CodeEmbedderError> {
            // Return a zeroed fingerprint for testing
            Ok(SemanticFingerprint::zeroed())
        }

        async fn embed_batch(&self, codes: &[(&str, Option<&str>)]) -> Result<Vec<SemanticFingerprint>, CodeEmbedderError> {
            Ok(codes.iter().map(|_| SemanticFingerprint::zeroed()).collect())
        }

        fn is_ready(&self) -> bool {
            true
        }
    }

    /// Mock storage for testing.
    /// Stores full SemanticFingerprint per entity.
    struct MockCodeStorage {
        entities: RwLock<HashMap<Uuid, (CodeEntity, SemanticFingerprint)>>,
        file_index: RwLock<HashMap<String, Vec<Uuid>>>,
    }

    impl MockCodeStorage {
        fn new() -> Self {
            Self {
                entities: RwLock::new(HashMap::new()),
                file_index: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl CodeStorage for MockCodeStorage {
        async fn store(&self, entity: &CodeEntity, fingerprint: &SemanticFingerprint) -> Result<(), String> {
            let mut entities = self.entities.write().await;
            let mut file_index = self.file_index.write().await;

            entities.insert(entity.id, (entity.clone(), fingerprint.clone()));

            file_index
                .entry(entity.file_path.clone())
                .or_default()
                .push(entity.id);

            Ok(())
        }

        async fn get(&self, id: Uuid) -> Result<Option<CodeEntity>, String> {
            let entities = self.entities.read().await;
            Ok(entities.get(&id).map(|(e, _)| e.clone()))
        }

        async fn get_by_file(&self, file_path: &str) -> Result<Vec<CodeEntity>, String> {
            let file_index = self.file_index.read().await;
            let entities = self.entities.read().await;

            let ids = file_index.get(file_path).cloned().unwrap_or_default();
            Ok(ids
                .iter()
                .filter_map(|id| entities.get(id).map(|(e, _)| e.clone()))
                .collect())
        }

        async fn delete_file(&self, file_path: &str) -> Result<usize, String> {
            let mut file_index = self.file_index.write().await;
            let mut entities = self.entities.write().await;

            let ids = file_index.remove(file_path).unwrap_or_default();
            let count = ids.len();

            for id in ids {
                entities.remove(&id);
            }

            Ok(count)
        }

        async fn get_fingerprint(&self, id: Uuid) -> Result<Option<SemanticFingerprint>, String> {
            let entities = self.entities.read().await;
            Ok(entities.get(&id).map(|(_, fp)| fp.clone()))
        }

        async fn search_by_fingerprint(
            &self,
            _query_fingerprint: &SemanticFingerprint,
            _top_k: usize,
            _min_similarity: f32,
            _use_e7_primary: bool,
        ) -> Result<Vec<(CodeEntity, f32)>, String> {
            // Mock implementation - returns empty for tests
            Ok(Vec::new())
        }
    }

    fn create_test_chunk(name: &str, code: &str) -> CodeChunk {
        use super::super::ast_chunker::CodeChunkMetadata;

        CodeChunk {
            code: code.to_string(),
            contextualized_text: format!("File: test.rs\n---\n{}", code),
            description: None,
            metadata: CodeChunkMetadata {
                file_path: "/test/file.rs".to_string(),
                language: "rust".to_string(),
                scope_chain: vec![name.to_string()],
                entity_type: ChunkEntityType::Function,
                entity_signature: Some(format!("fn {}()", name)),
                start_line: 1,
                end_line: 3,
                start_byte: 0,
                end_byte: code.len(),
                non_whitespace_chars: code.chars().filter(|c| !c.is_whitespace()).count(),
                imports: vec![],
                parent_definition: None,
            },
        }
    }

    #[tokio::test]
    async fn test_capture_chunk() {
        let embedder = Arc::new(MockCodeEmbedder);
        let storage = Arc::new(MockCodeStorage::new());
        let service = CodeCaptureService::new(embedder, storage.clone(), "test-session".to_string());

        let chunk = create_test_chunk("test_func", "fn test_func() { println!(\"hello\"); }");

        let id = service.capture_chunk(chunk).await.unwrap();

        // Verify entity was stored
        let entity = storage.get(id).await.unwrap().unwrap();
        assert_eq!(entity.name, "test_func");
        assert_eq!(entity.entity_type, CodeEntityType::Function);

        // Verify fingerprint was stored with all 13 embeddings
        let fingerprint = storage.get_fingerprint(id).await.unwrap().unwrap();
        assert!(fingerprint.is_complete(), "Fingerprint should have all 13 embeddings");
        assert_eq!(fingerprint.e7_code.len(), 1536, "E7 should be 1536D");
        assert_eq!(fingerprint.e1_semantic.len(), 1024, "E1 should be 1024D");
    }

    #[tokio::test]
    async fn test_capture_batch() {
        let embedder = Arc::new(MockCodeEmbedder);
        let storage = Arc::new(MockCodeStorage::new());
        let service = CodeCaptureService::new(embedder, storage.clone(), "test-session".to_string());

        let chunks = vec![
            create_test_chunk("func1", "fn func1() {}"),
            create_test_chunk("func2", "fn func2() {}"),
            create_test_chunk("func3", "fn func3() {}"),
        ];

        let ids = service.capture_batch(chunks).await.unwrap();
        assert_eq!(ids.len(), 3);

        // Verify all entities were stored
        for id in &ids {
            assert!(storage.get(*id).await.unwrap().is_some());
        }
    }

    #[tokio::test]
    async fn test_delete_by_file() {
        let embedder = Arc::new(MockCodeEmbedder);
        let storage = Arc::new(MockCodeStorage::new());
        let service = CodeCaptureService::new(embedder, storage.clone(), "test-session".to_string());

        let chunks = vec![
            create_test_chunk("func1", "fn func1() {}"),
            create_test_chunk("func2", "fn func2() {}"),
        ];

        let ids = service.capture_batch(chunks).await.unwrap();
        assert_eq!(ids.len(), 2);

        // Delete by file
        let deleted = service.delete_by_file("/test/file.rs").await.unwrap();
        assert_eq!(deleted, 2);

        // Verify entities are gone
        for id in &ids {
            assert!(storage.get(*id).await.unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn test_empty_content_error() {
        let embedder = Arc::new(MockCodeEmbedder);
        let storage = Arc::new(MockCodeStorage::new());
        let service = CodeCaptureService::new(embedder, storage, "test-session".to_string());

        let chunk = create_test_chunk("empty", "   ");

        let result = service.capture_chunk(chunk).await;
        assert!(matches!(result, Err(CodeCaptureError::EmptyContent)));
    }

    #[test]
    fn test_entity_type_conversion() {
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::convert_entity_type(ChunkEntityType::Function),
            CodeEntityType::Function
        );
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::convert_entity_type(ChunkEntityType::Struct),
            CodeEntityType::Struct
        );
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::convert_entity_type(ChunkEntityType::Impl),
            CodeEntityType::Impl
        );
    }

    #[test]
    fn test_language_from_string() {
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::language_from_string("rust"),
            CodeLanguage::Rust
        );
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::language_from_string("Python"),
            CodeLanguage::Python
        );
        assert_eq!(
            CodeCaptureService::<MockCodeEmbedder, MockCodeStorage>::language_from_string("unknown_lang"),
            CodeLanguage::Unknown
        );
    }
}
