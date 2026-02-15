//! MultiSpaceIndexManager trait implementation for HnswMultiSpaceIndex.
//!
//! This module implements the async trait for the multi-space index operations.

use async_trait::async_trait;
use std::fs::File;
use std::path::Path;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::multi_space::HnswMultiSpaceIndex;
use super::real_hnsw::RealHnswIndex;
use crate::index::config::EmbedderIndex;
use crate::index::error::{IndexError, IndexResult};
use crate::index::manager::MultiSpaceIndexManager;
use crate::index::status::IndexStatus;
use crate::types::fingerprint::SemanticFingerprint;

#[async_trait]
impl MultiSpaceIndexManager for HnswMultiSpaceIndex {
    async fn initialize(&mut self) -> IndexResult<()> {
        if self.is_initialized() {
            return Ok(());
        }

        info!("Initializing HnswMultiSpaceIndex with REAL hnsw_rs implementation");

        for embedder in EmbedderIndex::all_hnsw() {
            if let Some(config) = Self::config_for_embedder(embedder) {
                debug!(
                    "Creating RealHnswIndex for {:?}: dim={}, metric={:?}",
                    embedder, config.dimension, config.metric
                );

                let index = RealHnswIndex::new(config.clone()).map_err(|e| {
                    error!(
                        "FATAL: Failed to create RealHnswIndex for {:?}: {}",
                        embedder, e
                    );
                    e
                })?;

                self.insert_hnsw_index(embedder, index);
                self.insert_config(embedder, config);
            }
        }

        self.set_initialized(true);

        info!(
            "HnswMultiSpaceIndex initialized with {} real HNSW indexes",
            self.hnsw_count()
        );

        Ok(())
    }

    async fn add_vector(
        &mut self,
        embedder: EmbedderIndex,
        memory_id: Uuid,
        vector: &[f32],
    ) -> IndexResult<()> {
        if !embedder.uses_hnsw() {
            error!("FATAL: Invalid embedder {:?} for HNSW operation", embedder);
            return Err(IndexError::InvalidEmbedder { embedder });
        }

        if !self.is_initialized() {
            error!("FATAL: HnswMultiSpaceIndex not initialized");
            return Err(IndexError::NotInitialized { embedder });
        }

        if let Some(index) = self.get_hnsw_index_mut(&embedder) {
            let expected_dim = embedder.dimension().unwrap_or(0);
            if vector.len() != expected_dim {
                error!(
                    "FATAL: Dimension mismatch for {:?}: expected {}, got {}",
                    embedder,
                    expected_dim,
                    vector.len()
                );
                return Err(IndexError::DimensionMismatch {
                    embedder,
                    expected: expected_dim,
                    actual: vector.len(),
                });
            }

            index.add(memory_id, vector).map_err(|e| match e {
                IndexError::DimensionMismatch {
                    expected, actual, ..
                } => IndexError::DimensionMismatch {
                    embedder,
                    expected,
                    actual,
                },
                IndexError::ZeroNormVector { memory_id } => {
                    IndexError::ZeroNormVector { memory_id }
                }
                other => other,
            })?;

            return Ok(());
        }

        error!(
            "FATAL: No HNSW index found for {:?} - not initialized",
            embedder
        );
        Err(IndexError::NotInitialized { embedder })
    }

    async fn add_fingerprint(
        &mut self,
        memory_id: Uuid,
        fingerprint: &SemanticFingerprint,
    ) -> IndexResult<()> {
        if !self.is_initialized() {
            return Err(IndexError::NotInitialized {
                embedder: EmbedderIndex::E1Semantic,
            });
        }

        // E1 Semantic
        self.add_vector(
            EmbedderIndex::E1Semantic,
            memory_id,
            &fingerprint.e1_semantic,
        )
        .await?;

        // E1 Matryoshka 128D
        let matryoshka: Vec<f32> = fingerprint.e1_semantic.iter().take(128).copied().collect();
        self.add_vector(EmbedderIndex::E1Matryoshka128, memory_id, &matryoshka)
            .await?;

        // E2-E5 Temporal embeddings
        self.add_vector(
            EmbedderIndex::E2TemporalRecent,
            memory_id,
            &fingerprint.e2_temporal_recent,
        )
        .await?;
        self.add_vector(
            EmbedderIndex::E3TemporalPeriodic,
            memory_id,
            &fingerprint.e3_temporal_periodic,
        )
        .await?;
        self.add_vector(
            EmbedderIndex::E4TemporalPositional,
            memory_id,
            &fingerprint.e4_temporal_positional,
        )
        .await?;
        self.add_vector(EmbedderIndex::E5Causal, memory_id, fingerprint.e5_active_vector())
            .await?;

        // E7-E11
        self.add_vector(EmbedderIndex::E7Code, memory_id, &fingerprint.e7_code)
            .await?;
        self.add_vector(EmbedderIndex::E8Graph, memory_id, fingerprint.e8_active_vector())
            .await?;
        self.add_vector(EmbedderIndex::E9HDC, memory_id, &fingerprint.e9_hdc)
            .await?;
        self.add_vector(
            EmbedderIndex::E10Multimodal,
            memory_id,
            fingerprint.e10_active_vector(),
        )
        .await?;
        self.add_vector(EmbedderIndex::E11Entity, memory_id, &fingerprint.e11_entity)
            .await?;

        // E13 SPLADE -> inverted index
        let splade_pairs: Vec<(usize, f32)> = fingerprint
            .e13_splade
            .indices
            .iter()
            .zip(fingerprint.e13_splade.values.iter())
            .map(|(&idx, &val)| (idx as usize, val))
            .collect();
        self.add_splade_internal(memory_id, &splade_pairs)?;

        Ok(())
    }


    async fn add_splade(&mut self, memory_id: Uuid, sparse: &[(usize, f32)]) -> IndexResult<()> {
        self.add_splade_internal(memory_id, sparse)
    }

    async fn search(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        if !embedder.uses_hnsw() {
            error!("FATAL: Invalid embedder {:?} for HNSW search", embedder);
            return Err(IndexError::InvalidEmbedder { embedder });
        }

        if !self.is_initialized() {
            error!("FATAL: HnswMultiSpaceIndex not initialized for search");
            return Err(IndexError::NotInitialized { embedder });
        }

        let expected_dim = embedder.dimension().unwrap_or(0);
        if query.len() != expected_dim {
            error!(
                "FATAL: Query dimension mismatch for {:?}: expected {}, got {}",
                embedder,
                expected_dim,
                query.len()
            );
            return Err(IndexError::DimensionMismatch {
                embedder,
                expected: expected_dim,
                actual: query.len(),
            });
        }

        if let Some(index) = self.get_hnsw_index(&embedder) {
            return index.search(query, k).map_err(|e| match e {
                IndexError::DimensionMismatch {
                    expected, actual, ..
                } => IndexError::DimensionMismatch {
                    embedder,
                    expected,
                    actual,
                },
                other => other,
            });
        }

        error!(
            "FATAL: No HNSW index found for {:?} during search",
            embedder
        );
        Err(IndexError::NotInitialized { embedder })
    }

    async fn search_splade(
        &self,
        sparse_query: &[(usize, f32)],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        if !self.is_initialized() {
            return Err(IndexError::NotInitialized {
                embedder: EmbedderIndex::E13Splade,
            });
        }

        Ok(self.search_splade_internal(sparse_query, k))
    }

    async fn search_matryoshka(
        &self,
        query_128d: &[f32],
        k: usize,
    ) -> IndexResult<Vec<(Uuid, f32)>> {
        self.search(EmbedderIndex::E1Matryoshka128, query_128d, k)
            .await
    }


    async fn remove(&mut self, memory_id: Uuid) -> IndexResult<()> {
        let mut found = false;

        for index in self.hnsw_indexes_mut().values_mut() {
            if index.remove(memory_id) {
                found = true;
            }
        }

        if self.remove_splade(memory_id) {
            found = true;
        }

        if !found {
            debug!(
                "Memory {} not found in any index during remove - may have been partially indexed",
                memory_id
            );
        }

        Ok(())
    }

    fn status(&self) -> Vec<IndexStatus> {
        let mut statuses = Vec::with_capacity(14);

        for embedder in EmbedderIndex::all_hnsw() {
            statuses.push(self.get_embedder_status(embedder));
        }

        let mut splade_status = IndexStatus::new_empty(EmbedderIndex::E13Splade);
        splade_status.update_count(self.splade_len(), 40);
        statuses.push(splade_status);

        statuses
    }

    async fn persist(&self, path: &Path) -> IndexResult<()> {
        std::fs::create_dir_all(path).map_err(|e| IndexError::io("creating index directory", e))?;

        info!("Persisting HnswMultiSpaceIndex to {:?}", path);

        for (embedder, index) in self.hnsw_indexes() {
            let file_name = format!("{:?}.real_hnsw.bin", embedder);
            let file_path = path.join(&file_name);
            index.persist(&file_path)?;
            debug!(
                "Persisted RealHnswIndex for {:?} with {} vectors",
                embedder,
                index.len()
            );
        }

        let splade_path = path.join("splade.bin");
        self.persist_splade(&splade_path)?;

        let meta_path = path.join("index_meta.json");
        let meta = serde_json::json!({
            "version": "3.0.0",
            "hnsw_count": self.hnsw_count(),
            "splade_count": self.splade_len(),
            "initialized": self.is_initialized(),
            "index_type": "RealHnswIndex",
            "note": "RealHnswIndex only - legacy SimpleHnswIndex support removed"
        });
        let meta_file =
            File::create(&meta_path).map_err(|e| IndexError::io("creating metadata file", e))?;
        serde_json::to_writer_pretty(meta_file, &meta)
            .map_err(|e| IndexError::serialization("serializing metadata", e))?;

        info!(
            "Persisted {} HNSW indexes, {} SPLADE entries",
            self.hnsw_count(),
            self.splade_len()
        );

        Ok(())
    }

    async fn load(&mut self, path: &Path) -> IndexResult<()> {
        let meta_path = path.join("index_meta.json");
        if !meta_path.exists() {
            error!("FATAL: Index metadata not found at {:?}", meta_path);
            return Err(IndexError::CorruptedIndex {
                path: meta_path.display().to_string(),
            });
        }

        info!("Loading HnswMultiSpaceIndex from {:?}", path);

        let meta_file =
            File::open(&meta_path).map_err(|e| IndexError::io("opening metadata file", e))?;
        let meta: serde_json::Value = serde_json::from_reader(meta_file)
            .map_err(|e| IndexError::serialization("parsing metadata", e))?;

        let version = meta
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0");
        debug!("Index version: {}", version);

        for embedder in EmbedderIndex::all_hnsw() {
            let file_name = format!("{:?}.real_hnsw.bin", embedder);
            let file_path = path.join(&file_name);

            if file_path.exists() {
                match RealHnswIndex::load(&file_path) {
                    Ok(index) => {
                        info!(
                            "Loaded RealHnswIndex for {:?} with {} vectors",
                            embedder,
                            index.len()
                        );
                        self.insert_hnsw_index(embedder, index);
                        if let Some(config) = Self::config_for_embedder(embedder) {
                            self.insert_config(embedder, config);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Could not load RealHnswIndex for {:?}: {} - creating empty index",
                            embedder, e
                        );
                        if let Some(config) = Self::config_for_embedder(embedder) {
                            let index = RealHnswIndex::new(config.clone()).map_err(|e| {
                                error!(
                                    "FATAL: Failed to create fallback RealHnswIndex for {:?}: {}",
                                    embedder, e
                                );
                                e
                            })?;
                            self.insert_hnsw_index(embedder, index);
                            self.insert_config(embedder, config);
                        }
                    }
                }
            } else if let Some(config) = Self::config_for_embedder(embedder) {
                let index = RealHnswIndex::new(config.clone()).map_err(|e| {
                    error!(
                        "FATAL: Failed to create RealHnswIndex for {:?}: {}",
                        embedder, e
                    );
                    e
                })?;
                self.insert_hnsw_index(embedder, index);
                self.insert_config(embedder, config);
            }
        }

        // AP-007: FAIL FAST on legacy formats
        for embedder in EmbedderIndex::all_hnsw() {
            let legacy_file_name = format!("{:?}.hnsw.bin", embedder);
            let legacy_file_path = path.join(&legacy_file_name);

            if legacy_file_path.exists() {
                error!(
                    "FATAL: Legacy SimpleHnswIndex file found for {:?} at {:?}. \
                     Legacy formats are no longer supported. Data must be reindexed.",
                    embedder, legacy_file_path
                );
                return Err(IndexError::legacy_format(
                    legacy_file_path.display().to_string(),
                    format!(
                        "Legacy SimpleHnswIndex file found for {:?}. \
                         This format was deprecated and is no longer supported.",
                        embedder
                    ),
                ));
            }
        }

        let splade_path = path.join("splade.bin");
        if splade_path.exists() {
            self.load_splade(&splade_path)?;
            info!("Loaded SPLADE index with {} entries", self.splade_len());
        }

        self.set_initialized(true);

        info!(
            "Loaded HnswMultiSpaceIndex: {} HNSW, {} SPLADE",
            self.hnsw_count(),
            self.splade_len()
        );

        Ok(())
    }
}
