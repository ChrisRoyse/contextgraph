``

### 11.5 Goal Hierarchy Index (Layer 2C)

Navigate the North Star → Mid → Local goal hierarchy:

```rust
/// Hierarchical index for goal-aligned retrieval
pub struct GoalHierarchyIndex {
    /// North Star goals (system-wide)
    north_stars: HashMap<GoalId, NorthStarGoal>,

    /// Mid-level goals (subsystems)
    mid_goals: HashMap<GoalId, MidGoal>,

    /// Local goals (specific actions)
    local_goals: HashMap<GoalId, LocalGoal>,

    /// Memory-to-goal alignment cache
    /// Key: (memory_id, goal_id), Value: alignment_score
    alignment_cache: BTreeMap<(MemoryId, GoalId), AlignmentEntry>,

    /// Inverted index: goal -> aligned memories
    goal_to_memories: HashMap<GoalId, BTreeSet<(f32, MemoryId)>>,
}

#[derive(Clone)]
pub struct AlignmentEntry {
    /// Overall alignment to this goal
    pub aggregate: f32,

    /// Per-embedder alignment breakdown (13 embedders with E13 SPLADE)
    pub per_space: [f32; 13],

    /// Per-stage alignment (5-stage pipeline)
    pub per_stage: [f32; 5],

    /// When this alignment was computed
    pub computed_at: DateTime<Utc>,

    /// Alignment delta from previous computation
    pub delta: f32,
}

/// Stage-specific alignment scores (5-stage pipeline)
pub struct StageAlignments {
    /// Stage 1: SPLADE sparse (E13)
    pub stage1_splade: f32,
    /// Stage 2: Matryoshka 128D (E1 truncated)
    pub stage2_matryoshka: f32,
    /// Stage 3: RRF multi-space (E1-E11, E13)
    pub stage3_rrf: f32,
    /// Stage 4: Teleological filter
    pub stage4_teleological: f32,
    /// Stage 5: MaxSim late interaction (E12)
    pub stage5_maxsim: f32,
}

impl GoalHierarchyIndex {
    /// Find memories aligned with a specific goal
    pub fn find_aligned_memories(
        &self,
        goal_id: GoalId,
        min_alignment: f32,
        top_k: usize,
    ) -> Vec<(MemoryId, AlignmentEntry)> {
        self.goal_to_memories
            .get(&goal_id)
            .map(|set| {
                set.iter()
                    .rev() // Highest alignment first
                    .filter(|(score, _)| *score >= min_alignment)
                    .take(top_k)
                    .map(|(score, id)| (*id, self.alignment_cache.get(&(*id, goal_id)).cloned().unwrap()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find memories aligned with goal hierarchy
    pub fn find_hierarchically_aligned(
        &self,
        north_star_id: GoalId,
        min_transitive_alignment: f32,
    ) -> Vec<(MemoryId, f32, Vec<GoalId>)> {
        let mut results = vec![];

        // Get mid-level goals under this North Star
        let mid_goals: Vec<_> = self.mid_goals.values()
            .filter(|g| g.parent_id == Some(north_star_id))
            .collect();

        for mid in mid_goals {
            let mid_to_north = self.get_goal_alignment(mid.id, north_star_id);

            // Get local goals under this mid-level
            let local_goals: Vec<_> = self.local_goals.values()
                .filter(|g| g.parent_id == Some(mid.id))
                .collect();

            for local in local_goals {
                let local_to_mid = self.get_goal_alignment(local.id, mid.id);

                // Get memories aligned with local goal
                for (memory_id, entry) in self.find_aligned_memories(local.id, 0.5, 1000) {
                    // Compute transitive alignment bound
                    let transitive = 2.0 * entry.aggregate * local_to_mid * mid_to_north - 1.0;

                    if transitive >= min_transitive_alignment {
                        results.push((
                            memory_id,
                            transitive,
                            vec![local.id, mid.id, north_star_id],
                        ));
                    }
                }
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results
    }
}
```

### 11.6 Johari Quadrant Storage

Store and query by Johari Window classification per embedder:

```rust
/// Johari Window quadrant for each embedding space
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum JohariQuadrant {
    /// Known to self and others - high alignment, high confidence
    Open,

    /// Known to self, not others - high confidence, low external visibility
    Hidden,

    /// Not known to self, known to others - low confidence, high external signal
    Blind,

    /// Unknown to both - low alignment, low confidence
    Unknown,
}

/// Per-embedder Johari classification (13 embedders including E13 SPLADE)
pub struct JohariFingerprint {
    /// Quadrant per embedding space (E1-E13)
    pub quadrants: [JohariQuadrant; 13],

    /// Confidence of classification per space
    pub confidence: [f32; 13],

    /// Transition probability matrix (for evolution prediction)
    pub transition_probs: [[f32; 4]; 13],

    /// 5-stage pipeline relevance per embedder
    pub stage_relevance: [StageRole; 13],
}

/// Role of each embedder in the 5-stage pipeline
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StageRole {
    /// Stage 1: SPLADE sparse pre-filter (E13)
    Stage1PreFilter,
    /// Stage 2: Matryoshka ANN (E1 truncated)
    Stage2ANN,
    /// Stage 3: RRF multi-space rerank (E1-E11, E13)
    Stage3RRF,
    /// Stage 4: Teleological alignment filter
    Stage4Teleological,
    /// Stage 5: Late interaction MaxSim (E12)
    Stage5MaxSim,
}

/// Index for Johari-based queries
pub struct JohariIndex {
    /// Bitmap index: quadrant -> embedder -> memory_ids
    quadrant_index: HashMap<(JohariQuadrant, EmbedderType), RoaringBitmap>,

    /// Pattern index: full 13-quadrant signature (E1-E13)
    pattern_index: HashMap<[JohariQuadrant; 13], Vec<MemoryId>>,

    /// Stage-based index: memories with high confidence at each stage
    stage_index: HashMap<StageRole, BTreeSet<(f32, MemoryId)>>,
}

impl JohariIndex {
    /// Find memories in a specific quadrant for a specific embedder
    pub fn find_by_quadrant(
        &self,
        quadrant: JohariQuadrant,
        embedder: EmbedderType,
    ) -> Vec<MemoryId> {
        self.quadrant_index
            .get(&(quadrant, embedder))
            .map(|bitmap| bitmap.iter().map(|id| MemoryId(id as u64)).collect())
            .unwrap_or_default()
    }

    /// Find "blind spot" memories: high causal importance but low awareness
    pub fn find_blind_spots(&self) -> Vec<MemoryId> {
        // Memories in Blind quadrant for E5 (causal) but Open for E1 (semantic)
        let causal_blind = self.find_by_quadrant(JohariQuadrant::Blind, EmbedderType::Causal);
        let semantic_open = self.find_by_quadrant(JohariQuadrant::Open, EmbedderType::Semantic);

        // Intersection: know what it means but blind to causal implications
        causal_blind.into_iter()
            .filter(|id| semantic_open.contains(id))
            .collect()
    }

    /// Find memories with potential for growth (Unknown → Open)
    pub fn find_growth_opportunities(&self) -> Vec<(MemoryId, EmbedderType)> {
        let mut opportunities = vec![];

        for embedder in EmbedderType::all() {
            let unknown = self.find_by_quadrant(JohariQuadrant::Unknown, embedder);
            for memory_id in unknown {
                // These could become known with more exploration
                opportunities.push((memory_id, embedder));
            }
        }

        opportunities
    }
}
```

### 11.7 Temporal Purpose Evolution Storage

Track how teleological alignment changes over time:

```rust
/// Snapshot of purpose at a point in time
pub struct PurposeSnapshot {
    /// Timestamp of snapshot
    pub timestamp: DateTime<Utc>,

    /// Purpose vector at this time
    pub purpose: PurposeVector,

    /// Johari quadrants at this time
    pub johari: JohariFingerprint,

    /// Event that triggered the snapshot
    pub trigger: EvolutionTrigger,
}

#[derive(Clone)]
pub enum EvolutionTrigger {
    /// Initial creation
    Created,

    /// Accessed and alignment recomputed
    Accessed { query_context: String },

    /// Goal hierarchy changed
    GoalChanged { old_goal: GoalId, new_goal: GoalId },

    /// Periodic recalibration
    Recalibration,

    /// Misalignment detected
    MisalignmentDetected { delta_a: f32 },
}

/// Time-series storage for purpose evolution
pub struct PurposeEvolutionStore {
    /// Time-series database connection (InfluxDB/TimescaleDB)
    timeseries: TimeSeriesDb,

    /// Evolution patterns (for prediction)
    evolution_patterns: HashMap<PatternId, EvolutionPattern>,
}

impl PurposeEvolutionStore {
    /// Record a purpose snapshot
    pub async fn record_snapshot(
        &self,
        memory_id: MemoryId,
        snapshot: PurposeSnapshot,
    ) -> Result<()> {
        self.timeseries.write(
            "purpose_evolution",
            memory_id,
            snapshot.timestamp,
            &snapshot,
        ).await
    }

    /// Get purpose evolution over time
    pub async fn get_evolution(
        &self,
        memory_id: MemoryId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<PurposeSnapshot>> {
        self.timeseries.query(
            "purpose_evolution",
            memory_id,
            start,
            end,
        ).await
    }

    /// Detect purpose drift
    pub async fn detect_drift(
        &self,
        memory_id: MemoryId,
        window: Duration,
    ) -> Result<Option<DriftAlert>> {
        let snapshots = self.get_evolution(
            memory_id,
            Utc::now() - window,
            Utc::now(),
        ).await?;

        if snapshots.len() < 2 {
            return Ok(None);
        }

        let first = &snapshots[0];
        let last = &snapshots[snapshots.len() - 1];

        // Compute drift per embedding space (13 embedders including E13 SPLADE)
        let mut drifts = [0.0f32; 13];
        for i in 0..13 {
            drifts[i] = last.purpose.alignments[i] - first.purpose.alignments[i];
        }

        // Alert if significant drift in any space
        for (i, drift) in drifts.iter().enumerate() {
            if *drift < -0.15 {
                // Determine severity based on pipeline stage
                let stage = match i {
                    12 => StageRole::Stage1PreFilter,  // E13 SPLADE
                    0 => StageRole::Stage2ANN,         // E1 Semantic (Matryoshka)
                    4 => StageRole::Stage3RRF,         // E5 Causal (critical)
                    11 => StageRole::Stage5MaxSim,     // E12 Late Interaction
                    _ => StageRole::Stage3RRF,
                };

                return Ok(Some(DriftAlert {
                    memory_id,
                    embedder: EmbedderType::from_index(i),
                    drift: *drift,
                    stage,
                    severity: if *drift < -0.30 { Severity::Critical } else { Severity::Warning },
                }));
            }
        }

        Ok(None)
    }
}
```

### 11.8 Complete Storage Schema (SQL + Vector + Time-Series)

```sql
-- ============================================================
-- PRIMARY STORAGE: Full Teleological Fingerprint
-- ============================================================

CREATE TABLE teleological_memories (
    id UUID PRIMARY KEY,
    content_hash BYTEA NOT NULL UNIQUE,

    -- The 13-embedding array (stored with PQ-8 quantization, ~17KB total)
    -- Storage reduced 63% from ~46KB via Product Quantization
    e1_semantic BYTEA NOT NULL,           -- PQ-8: 128B (was 4KB)
    e1_matryoshka_128d BYTEA,             -- Optional: 128D truncation for Stage 2
    e2_temporal_recent BYTEA NOT NULL,    -- Float8: 512B (was 2KB)
    e3_temporal_periodic BYTEA NOT NULL,  -- Float8: 512B (was 2KB)
    e4_temporal_positional BYTEA NOT NULL,-- Float8: 512B (was 2KB)
    e5_causal BYTEA NOT NULL,             -- PQ-8: 96B (was 3KB)
    e5_causal_direction SMALLINT NOT NULL,-- 1=cause, 2=effect, 3=bidirectional
    e5_asymmetric_weight REAL NOT NULL DEFAULT 1.0, -- 1.2 for cause→effect, 0.8 for effect→cause
    e6_sparse_indices INT[] NOT NULL,     -- Sparse: Active indices
    e6_sparse_values REAL[] NOT NULL,     -- Sparse: Activation values
    e7_code BYTEA NOT NULL,               -- PQ-8: 192B (was 6KB)
    e8_graph BYTEA NOT NULL,              -- Float8: 384B (was 1.5KB)
    e9_hdc BYTEA NOT NULL,                -- Binary: 128B (native)
    e10_multimodal BYTEA NOT NULL,        -- PQ-8: 96B (was 3KB)
    e11_entity BYTEA NOT NULL,            -- Float8: 384B (was 1.5KB)
    e12_late_interaction BYTEA NOT NULL,  -- PQ-8: ~1KB (variable)
    e12_token_count SMALLINT NOT NULL,
    e13_splade_indices INT[] NOT NULL,    -- SPLADE sparse: vocabulary indices
    e13_splade_values REAL[] NOT NULL,    -- SPLADE sparse: BM25-weighted values
    e13_splade_norm REAL NOT NULL,        -- Precomputed L2 norm for scoring

    -- The 13D Purpose Vector (alignment to current North Star)
    -- Includes E13 SPLADE alignment
    purpose_vector REAL[13] NOT NULL,
    purpose_coherence REAL NOT NULL,      -- How aligned are all spaces?
    purpose_dominant_space SMALLINT NOT NULL, -- Which space dominates?

    -- 5-Stage Pipeline scores
    stage_scores REAL[5] NOT NULL,        -- [splade, matryoshka, rrf, teleological, maxsim]

    -- Johari quadrants per embedder (encoded as 2 bits each)
    johari_quadrants BYTEA NOT NULL,      -- 13 * 2 bits = 26 bits = 4 bytes
    johari_confidence REAL[13] NOT NULL,

    -- Metadata
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    last_accessed_at TIMESTAMPTZ,
    access_count INT DEFAULT 0,
    source_type VARCHAR(50),
    source_id VARCHAR(255)
);

-- Purpose vector index (13D HNSW)
CREATE INDEX idx_purpose_vector ON teleological_memories
    USING hnsw (purpose_vector vector_cosine_ops) WITH (m = 16, ef_construction = 200);

-- 5-Stage Pipeline indexes
CREATE INDEX idx_stage_scores ON teleological_memories(stage_scores);

-- Stage 1: SPLADE inverted index for sparse pre-filter
CREATE TABLE idx_e13_splade_inverted (
    term_id INTEGER NOT NULL,
    memory_id UUID REFERENCES teleological_memories(id),
    weight REAL NOT NULL,
    PRIMARY KEY (term_id, memory_id)
);
CREATE INDEX idx_splade_term ON idx_e13_splade_inverted(term_id);

-- Stage 2: Matryoshka 128D index for fast ANN
CREATE TABLE idx_e1_matryoshka_128 (
    memory_id UUID REFERENCES teleological_memories(id),
    embedding vector(128),
    PRIMARY KEY (memory_id)
);
CREATE INDEX idx_e1_matryoshka_hnsw ON idx_e1_matryoshka_128
    USING hnsw (embedding vector_cosine_ops) WITH (m = 32, ef_construction = 400);

-- ============================================================
-- PER-EMBEDDER VECTOR INDEXES (Layer 2A)
-- ============================================================

-- E1: Semantic (1024D)
CREATE TABLE idx_e1_semantic (
    memory_id UUID REFERENCES teleological_memories(id),
    embedding vector(1024),
    PRIMARY KEY (memory_id)
);
CREATE INDEX idx_e1_hnsw ON idx_e1_semantic
    USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 200);

-- E5: Causal (768D) - separate for asymmetric search
CREATE TABLE idx_e5_causal (
    memory_id UUID REFERENCES teleological_memories(id),
    embedding vector(768),
    direction SMALLINT NOT NULL,
    PRIMARY KEY (memory_id)
);
CREATE INDEX idx_e5_hnsw ON idx_e5_causal
    USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 200);
CREATE INDEX idx_e5_direction ON idx_e5_causal(direction);

-- E7: Code (1536D)
CREATE TABLE idx_e7_code (
    memory_id UUID REFERENCES teleological_memories(id),
    embedding vector(1536),
    PRIMARY KEY (memory_id)
);
CREATE INDEX idx_e7_hnsw ON idx_e7_code
    USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 200);

-- (Similar indexes for E2, E3, E4, E8, E9, E10, E11)

-- ============================================================
-- GOAL HIERARCHY INDEX (Layer 2C)
-- ============================================================

CREATE TABLE goal_hierarchy (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    level SMALLINT NOT NULL,  -- 0=north_star, 1=mid, 2=local
    parent_id UUID REFERENCES goal_hierarchy(id),

    -- Goal embedding (can be searched)
    goal_embedding BYTEA NOT NULL,  -- Full 13-array goal representation

    -- Or simplified 1536D for fast search
    goal_embedding_1536 vector(1536),

    created_at TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_goal_level ON goal_hierarchy(level);
CREATE INDEX idx_goal_parent ON goal_hierarchy(parent_id);
CREATE INDEX idx_goal_embedding ON goal_hierarchy
    USING hnsw (goal_embedding_1536 vector_cosine_ops);

-- Memory-to-Goal alignment cache
CREATE TABLE memory_goal_alignment (
    memory_id UUID REFERENCES teleological_memories(id),
    goal_id UUID REFERENCES goal_hierarchy(id),

    -- Overall alignment
    aggregate_alignment REAL NOT NULL,

    -- Per-embedder breakdown (13 embedders including E13 SPLADE)
    per_space_alignment REAL[13] NOT NULL,

    -- 5-Stage Pipeline alignment breakdown
    per_stage_alignment REAL[5] NOT NULL,

    -- Alignment metadata
    computed_at TIMESTAMPTZ DEFAULT NOW(),
    delta_from_previous REAL,

    PRIMARY KEY (memory_id, goal_id)
);

CREATE INDEX idx_alignment_by_goal ON memory_goal_alignment(goal_id, aggregate_alignment DESC);
CREATE INDEX idx_alignment_by_memory ON memory_goal_alignment(memory_id);

-- ============================================================
-- JOHARI QUADRANT INDEX
-- ============================================================

CREATE TABLE johari_index (
    memory_id UUID REFERENCES teleological_memories(id),
    embedder_type SMALLINT NOT NULL,  -- 0-11
    quadrant SMALLINT NOT NULL,       -- 0=open, 1=hidden, 2=blind, 3=unknown
    confidence REAL NOT NULL,

    PRIMARY KEY (memory_id, embedder_type)
);

CREATE INDEX idx_johari_quadrant ON johari_index(quadrant, embedder_type);
CREATE INDEX idx_johari_blind ON johari_index(embedder_type) WHERE quadrant = 2;

-- ============================================================
-- PURPOSE EVOLUTION (Time-Series)
-- ============================================================

-- Using TimescaleDB hypertable for time-series
CREATE TABLE purpose_evolution (
    memory_id UUID NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,

    -- Purpose vector snapshot (13 embedders)
    purpose_vector REAL[13] NOT NULL,
    purpose_coherence REAL NOT NULL,

    -- 5-Stage pipeline scores snapshot
    stage_scores REAL[5] NOT NULL,

    -- Johari snapshot
    johari_quadrants BYTEA NOT NULL,

    -- Trigger
    trigger_type VARCHAR(50) NOT NULL,
    trigger_context JSONB,

    PRIMARY KEY (memory_id, timestamp)
);

-- Convert to hypertable (TimescaleDB)
SELECT create_hypertable('purpose_evolution', 'timestamp');

-- Retention policy: keep detailed data for 90 days
SELECT add_retention_policy('purpose_evolution', INTERVAL '90 days');

-- Continuous aggregate for long-term trends
CREATE MATERIALIZED VIEW purpose_evolution_daily
WITH (timescaledb.continuous) AS
SELECT
    memory_id,
    time_bucket('1 day', timestamp) AS day,
    AVG(purpose_coherence) as avg_coherence,
    ARRAY_AGG(purpose_vector ORDER BY timestamp) as daily_vectors
FROM purpose_evolution
GROUP BY memory_id, day;
```

### 11.9 Query Examples

```rust
/// Example queries against the teleological storage

impl TeleologicalStorage {
    /// Find memories causally aligned with a goal
    pub async fn find_causal_aligned(
        &self,
        goal_id: GoalId,
        min_causal_alignment: f32,
        top_k: usize,
    ) -> Result<Vec<TeleologicalMatch>> {
        // Query the E5 causal index specifically
        let causal_results = sqlx::query!(
            r#"
            SELECT
                m.id,
                m.purpose_vector,
                mga.per_space_alignment[5] as causal_alignment,
                m.e5_causal_direction
            FROM teleological_memories m
            JOIN memory_goal_alignment mga ON m.id = mga.memory_id
            WHERE mga.goal_id = $1
              AND mga.per_space_alignment[5] >= $2
            ORDER BY mga.per_space_alignment[5] DESC
            LIMIT $3
            "#,
            goal_id,
            min_causal_alignment,
            top_k as i32
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(causal_results.into_iter().map(|r| TeleologicalMatch {
            memory_id: r.id,
            causal_alignment: r.causal_alignment,
            causal_direction: CausalDirection::from(r.e5_causal_direction),
            purpose_vector: r.purpose_vector.try_into().unwrap(),
        }).collect())
    }

    /// Find memories with similar purpose patterns
    pub async fn find_similar_purpose(
        &self,
        purpose: &PurposeVector,
        top_k: usize,
    ) -> Result<Vec<(MemoryId, f32)>> {
        // Search the 13D purpose vector index
        sqlx::query!(
            r#"
            SELECT id, purpose_vector <=> $1::real[] as distance
            FROM teleological_memories
            ORDER BY purpose_vector <=> $1::real[]
            LIMIT $2
            "#,
            &purpose.alignments[..],
            top_k as i32
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| (r.id, 1.0 - r.distance)) // Convert distance to similarity
                .collect()
        })
    }

    /// Find blind spots: high semantic but low causal awareness
    pub async fn find_blind_spots(&self) -> Result<Vec<BlindSpotMemory>> {
        sqlx::query!(
            r#"
            SELECT
                m.id,
                m.purpose_vector,
                j_semantic.confidence as semantic_confidence,
                j_causal.confidence as causal_confidence
            FROM teleological_memories m
            JOIN johari_index j_semantic ON m.id = j_semantic.memory_id
                AND j_semantic.embedder_type = 0  -- E1 semantic
            JOIN johari_index j_causal ON m.id = j_causal.memory_id
                AND j_causal.embedder_type = 4    -- E5 causal
            WHERE j_semantic.quadrant = 0  -- Open for semantic
              AND j_causal.quadrant = 2    -- Blind for causal
            ORDER BY j_causal.confidence DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter().map(|r| BlindSpotMemory {
                memory_id: r.id,
                purpose_vector: r.purpose_vector.try_into().unwrap(),
                semantic_confidence: r.semantic_confidence,
                causal_confidence: r.causal_confidence,
            }).collect()
        })
    }

    /// Detect purpose drift over time
    pub async fn detect_purpose_drift(
        &self,
        memory_id: MemoryId,
        window_days: i32,
    ) -> Result<Option<DriftReport>> {
        let snapshots = sqlx::query!(
            r#"
            SELECT timestamp, purpose_vector, trigger_type
            FROM purpose_evolution
            WHERE memory_id = $1
              AND timestamp >= NOW() - make_interval(days => $2)
            ORDER BY timestamp
            "#,
            memory_id,
            window_days
        )
        .fetch_all(&self.pool)
        .await?;

        if snapshots.len() < 2 {
            return Ok(None);
        }

        let first = &snapshots[0];
        let last = &snapshots[snapshots.len() - 1];

        let mut max_drift = 0.0f32;
        let mut max_drift_space = 0;

        for i in 0..13 {
            let drift = last.purpose_vector[i] - first.purpose_vector[i];
            if drift.abs() > max_drift.abs() {
                max_drift = drift;
                max_drift_space = i;
            }
        }

        if max_drift.abs() > 0.15 {
            Ok(Some(DriftReport {
                memory_id,
                max_drift,
                drifted_space: EmbedderType::from_index(max_drift_space),
                start_time: first.timestamp,
                end_time: last.timestamp,
                snapshot_count: snapshots.len(),
            }))
        } else {
            Ok(None)
        }
    }
}
```

---

## 12. Mathematical Memory Foundations

### 12.1 Modern Hopfield Networks for Associative Memory

The 13-embedding array enables **exponential capacity** associative memory:

```
Classical Hopfield (1982):
  Capacity: ~0.14N patterns for N neurons

Modern Hopfield (Ramsauer et al., 2020):
  Capacity: Exponential in dimension d
  Energy: E(ξ, X) = -log Σᵢ exp(ξᵀxᵢ) + ½ξᵀξ + const
```

**Multi-Space Hopfield**: Apply associative memory PER embedding space:

```rust
/// Modern Hopfield associative memory per embedding space (13 embedders)
pub struct MultiSpaceHopfield {
    /// One Hopfield network per embedding space (E1-E13)
    networks: [HopfieldNetwork; 13],

    /// Cross-space attention for binding
    cross_attention: CrossSpaceAttention,

    /// 5-stage pipeline integration
    stage_weights: [f32; 5],
}

impl MultiSpaceHopfield {
    /// Retrieve associated memory given partial cue
    pub fn retrieve(
        &self,
        cue: &PartialFingerprint,
        iterations: usize,
    ) -> SemanticFingerprint {
        let mut result = SemanticFingerprint::default();

        // Retrieve in each space where cue is present
        for (i, space_cue) in cue.available_spaces().enumerate() {
            result.set_space(i, self.networks[i].retrieve(space_cue, iterations));
        }

        // Cross-space attention to fill missing spaces
        result = self.cross_attention.complete(result, cue);

        result
    }

    /// Store new memory with Hebbian learning
    pub fn store(&mut self, memory: &SemanticFingerprint) {
        for i in 0..13 {
            self.networks[i].store(memory.get_space(i));
        }

        // Update cross-space associations
        self.cross_attention.update(memory);

        // Update 5-stage pipeline weights based on memory characteristics
        self.update_stage_weights(memory);
    }

    fn update_stage_weights(&mut self, memory: &SemanticFingerprint) {
        // Adjust weights based on memory's stage-specific characteristics
        self.stage_weights[0] += 0.01 * memory.splade.norm;  // Stage 1: SPLADE
        self.stage_weights[1] += 0.01 * memory.semantic.truncate(128).len() as f32;  // Stage 2: Matryoshka
        self.stage_weights[2] += 0.01;  // Stage 3: RRF (always applicable)
        self.stage_weights[3] += 0.01 * memory.causal.alignment_causality;  // Stage 4: Teleological
        self.stage_weights[4] += 0.01 * memory.late_interaction.len() as f32;  // Stage 5: MaxSim
    }
}
```

### 12.2 Sparse Distributed Memory (SDM) for High-Dimensional Storage

Kanerva's SDM provides robust storage in high-dimensional spaces:

```rust
/// SDM-inspired storage for teleological vectors
pub struct TeleologicalSDM {
    /// Address space (random binary vectors)
    address_space: Vec<BitVec>,

    /// Content storage (counters per dimension)
    content_counters: Vec<Vec<i32>>,

    /// Activation radius
    radius: usize,
}

impl TeleologicalSDM {
    /// Store teleological fingerprint at address
    pub fn store(
        &mut self,
        address: &PurposeVector,
        content: &SemanticFingerprint,
    ) {
        // Find activated addresses (within Hamming radius)
        let activated = self.find_activated(address);

        // Update counters in activated locations
        for idx in activated {
            for (dim, value) in content.flatten().enumerate() {
                if value > 0.0 {
                    self.content_counters[idx][dim] += 1;
                } else {
                    self.content_counters[idx][dim] -= 1;
                }
            }
        }
    }

    /// Retrieve by purpose address
    pub fn retrieve(&self, address: &PurposeVector) -> SemanticFingerprint {
        let activated = self.find_activated(address);

        // Sum counters from activated locations
        let mut result = vec![0i32; self.total_dims()];
        for idx in activated {
            for (dim, count) in self.content_counters[idx].iter().enumerate() {
                result[dim] += count;
            }
        }

        // Threshold to binary/continuous
        SemanticFingerprint::from_counters(&result)
    }
}
```

### 12.3 The Binding Problem: How 13 Representations Become One Memory

The "binding problem" asks: how do distributed representations bind into unified percepts?

**Phase-Coherent Binding** across embedding spaces (13 embedders):

```rust
/// Binding mechanism for multi-space teleological vectors
pub struct PhaseCoherentBinding {
    /// Oscillator phase per embedding space (E1-E13)
    phases: [f32; 13],

    /// Coupling strength between spaces
    coupling: [[f32; 13]; 13],

    /// Natural frequency per space
    frequencies: [f32; 13],

    /// 5-stage pipeline phase alignment
    stage_phases: [f32; 5],
}

impl PhaseCoherentBinding {
    /// Compute binding strength between embedding spaces
    pub fn binding_strength(&self, space_i: usize, space_j: usize) -> f32 {
        // Phase coherence: spaces in-phase = bound together
        let phase_diff = (self.phases[space_i] - self.phases[space_j]).cos();

        // Modulated by coupling strength
        self.coupling[space_i][space_j] * phase_diff
    }

    /// Global binding coherence (how unified is this memory?)
    pub fn global_coherence(&self) -> f32 {
        let mut total = 0.0;
        let mut count = 0;

        for i in 0..13 {
            for j in (i + 1)..13 {
                total += self.binding_strength(i, j);
                count += 1;
            }
        }

        total / count as f32
    }

    /// 5-Stage pipeline coherence
    pub fn stage_coherence(&self) -> f32 {
        // How well do the 5 stages bind together?
        let mut total = 0.0;
        for i in 0..5 {
            for j in (i + 1)..5 {
                let phase_diff = (self.stage_phases[i] - self.stage_phases[j]).cos();
                total += phase_diff;
            }
        }
        total / 10.0  // 5 choose 2 = 10 pairs
    }

    /// Evolve phases via Kuramoto synchronization
    pub fn synchronize(&mut self, dt: f32) {
        let mut new_phases = self.phases;

        for i in 0..13 {
            let mut delta = self.frequencies[i];

            // Coupling term: attract to other phases
            for j in 0..13 {
                if i != j {
                    delta += self.coupling[i][j] * (self.phases[j] - self.phases[i]).sin();
                }
            }

            new_phases[i] += delta * dt;
        }

        self.phases = new_phases;

        // Also synchronize stage phases
        self.synchronize_stages(dt);
    }

    fn synchronize_stages(&mut self, dt: f32) {
        // Map embedder phases to stage phases
        self.stage_phases[0] = self.phases[12];  // Stage 1: E13 SPLADE
        self.stage_phases[1] = self.phases[0];   // Stage 2: E1 Matryoshka
        self.stage_phases[2] = (0..13).map(|i| self.phases[i]).sum::<f32>() / 13.0;  // Stage 3: RRF avg
        self.stage_phases[3] = self.phases[4];   // Stage 4: E5 Causal (teleological)
        self.stage_phases[4] = self.phases[11];  // Stage 5: E12 MaxSim
    }
}
```

---

## 13. Multi-Embedding UTL Integration

### 13.1 Extended UTL Formula for 13-Space Teleology with 5-Stage Pipeline

The Unified Teleological Lens (UTL) formula extended for multi-space and 5-stage retrieval:

```
Classic UTL:
  L = f((ΔS × ΔC) · wₑ · cos φ)

Multi-Embedding UTL (13 embedders):
  L_multi = sigmoid(2.0 · (Σᵢ₌₁¹³ τᵢλ_S·ΔSᵢ) · (Σⱼ₌₁¹³ τⱼλ_C·ΔCⱼ) · wₑ · cos φ)

5-Stage Augmented UTL:
  L_5stage = L_multi × Π_{k=1}^{5} (1 + αₖ·stage_score_k)

Where:
  τᵢ = teleological weight for embedder i (purpose importance)
  λ_S = semantic scaling factor
  λ_C = causal scaling factor (with asymmetric weighting for E5)
  ΔSᵢ = semantic delta in space i
  ΔCⱼ = causal delta in space j
  wₑ = embedded surprise factor
  φ = phase alignment across spaces
  αₖ = stage importance weight (Stage 1-5)
  stage_score_k = performance score at stage k

RRF Integration (Stage 3):
  L_rrf = Σᵢ τᵢ / (60 + rankᵢ)  -- Reciprocal Rank Fusion
```

```rust
/// Multi-embedding UTL computation (13 embedders + 5-stage pipeline)
pub struct MultiEmbeddingUTL {
    /// Per-space teleological weights (learned) - 13 embedders
    tau: [f32; 13],

    /// 5-stage pipeline importance weights
    alpha: [f32; 5],

    /// Semantic scaling factor
    lambda_s: f32,

    /// Causal scaling factor
    lambda_c: f32,

    /// Embedded surprise weight
    w_e: f32,
}

impl MultiEmbeddingUTL {
    /// Compute multi-space UTL score with 5-stage pipeline integration
    pub fn compute(
        &self,
        before: &SemanticFingerprint,
        after: &SemanticFingerprint,
        goal: &TeleologicalGoal,
    ) -> UTLScore {
        // Compute per-space deltas (13 embedders)
        let mut semantic_deltas = [0.0f32; 13];
        let mut causal_deltas = [0.0f32; 13];

        for i in 0..13 {
            let sim_before = cosine_sim(before.get_space(i), goal.get_space(i));
            let sim_after = cosine_sim(after.get_space(i), goal.get_space(i));

            semantic_deltas[i] = sim_after - sim_before;

            // Causal delta considers asymmetric direction for E5
            if i == 4 { // E5 is causal
                causal_deltas[i] = asymmetric_delta(before.get_space(i), after.get_space(i));
            } else {
                causal_deltas[i] = semantic_deltas[i];
            }
        }

        // Weighted sum of semantic deltas (13 embedders)
        let weighted_semantic: f32 = (0..13)
            .map(|i| self.tau[i] * self.lambda_s * semantic_deltas[i])
            .sum();

        // Weighted sum of causal deltas (13 embedders)
        let weighted_causal: f32 = (0..13)
            .map(|i| self.tau[i] * self.lambda_c * causal_deltas[i])
            .sum();

        // Phase alignment
        let phase = compute_phase_alignment(before, after, goal);

        // Base UTL score
        let raw = weighted_semantic * weighted_causal * self.w_e * phase.cos();
        let base_score = sigmoid(2.0 * raw);

        // 5-Stage pipeline augmentation
        let stage_scores = self.compute_stage_scores(before, after, goal);
        let stage_multiplier: f32 = (0..5)
            .map(|k| 1.0 + self.alpha[k] * stage_scores[k])
            .product();

        let final_score = (base_score * stage_multiplier).min(1.0);

        UTLScore {
            value: final_score,
            semantic_contribution: weighted_semantic,
            causal_contribution: weighted_causal,
            phase_alignment: phase,
            per_space_deltas: semantic_deltas,
            stage_scores,
        }
    }

    /// Compute 5-stage pipeline scores
    fn compute_stage_scores(
        &self,
        before: &SemanticFingerprint,
        after: &SemanticFingerprint,
        goal: &TeleologicalGoal,
    ) -> [f32; 5] {
        [
            // Stage 1: SPLADE sparse score improvement
            after.splade.sparse_score(&goal.splade) - before.splade.sparse_score(&goal.splade),
            // Stage 2: Matryoshka 128D similarity improvement
            cosine_sim(after.semantic.truncate(128), goal.semantic.truncate(128))
                - cosine_sim(before.semantic.truncate(128), goal.semantic.truncate(128)),
            // Stage 3: RRF fusion score (aggregate improvement)
            0.1,  // Computed separately via RRF formula
            // Stage 4: Teleological alignment improvement
            after.teleological.as_ref().map(|t| t.aggregate).unwrap_or(0.0)
                - before.teleological.as_ref().map(|t| t.aggregate).unwrap_or(0.0),
            // Stage 5: MaxSim late interaction improvement
            after.late_interaction_maxsim(&goal.late_interaction)
                - before.late_interaction_maxsim(&goal.late_interaction),
        ]
    }
}
```

### 13.2 Purpose-Weighted Similarity

Similarity weighted by teleological purpose:

```rust
/// Teleological similarity: not just similar, but similar FOR THE SAME PURPOSE
/// Integrates 5-stage pipeline scoring with RRF fusion
pub fn teleological_similarity(
    a: &TeleologicalFingerprint,
    b: &TeleologicalFingerprint,
) -> f32 {
    // Raw embedding similarity (per space) - 13 embedders
    let embedding_sims: [f32; 13] = (0..13)
        .map(|i| {
            if i == 4 { // E5 Causal - use asymmetric similarity
                asymmetric_causal_sim(a.embeddings.get_space(i), b.embeddings.get_space(i))
            } else {
                cosine_sim(a.embeddings.get_space(i), b.embeddings.get_space(i))
            }
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    // Purpose alignment (both aligned to same purpose?) - 13D
    let purpose_alignment = cosine_sim(
        &a.purpose_vector.alignments,
        &b.purpose_vector.alignments,
    );

    // RRF fusion for embedding aggregate (Stage 3 style)
    let embedding_aggregate = rrf_fusion_similarity(&embedding_sims);

    // 5-Stage pipeline bonus
    let stage_bonus = compute_stage_alignment(a, b);

    // Teleological similarity = RRF similarity * purpose alignment * stage bonus
    embedding_aggregate * purpose_alignment * (1.0 + 0.1 * stage_bonus)
}

/// RRF fusion for similarity scores
fn rrf_fusion_similarity(sims: &[f32; 13]) -> f32 {
    const K: f32 = 60.0;

    // Convert similarities to ranks
    let mut indexed: Vec<(usize, f32)> = sims.iter().enumerate()
        .map(|(i, &s)| (i, s))
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Compute RRF score
    let rrf: f32 = indexed.iter().enumerate()
        .map(|(rank, _)| 1.0 / (K + (rank + 1) as f32))
        .sum();

    rrf / (13.0 / K)  // Normalize
}

/// Compute 5-stage alignment between two fingerprints
fn compute_stage_alignment(a: &TeleologicalFingerprint, b: &TeleologicalFingerprint) -> f32 {
    let stage1 = a.embeddings.splade.sparse_score(&b.embeddings.splade);
    let stage2 = cosine_sim(a.embeddings.semantic.truncate(128), b.embeddings.semantic.truncate(128));
    let stage5 = a.embeddings.late_interaction_maxsim(&b.embeddings.late_interaction);

    (stage1 + stage2 + stage5) / 3.0
}
```

---

## 14. Meta-UTL: Self-Aware Memory System

### 14.1 The System That Learns About Its Own Learning

**Meta-UTL** applies UTL to the UTL system itself:

```rust
/// Meta-level UTL that monitors the learning system's own learning
pub struct MetaUTL {
    /// Current UTL parameters
    utl: MultiEmbeddingUTL,

    /// History of UTL parameter evolution
    parameter_history: Vec<UTLParameterSnapshot>,

    /// Meta-goal: "improve UTL accuracy"
    meta_goal: TeleologicalGoal,

    /// Meta-learning rate
    meta_lr: f32,
}

impl MetaUTL {
    /// Compute meta-UTL: how well is our UTL system learning?
    pub fn meta_score(&self) -> f32 {
        // Compute UTL accuracy on recent predictions
        let recent_predictions = self.get_recent_predictions(100);
        let accuracy = compute_prediction_accuracy(&recent_predictions);

        // Compute improvement delta
        let previous_accuracy = self.parameter_history.last()
            .map(|s| s.accuracy)
            .unwrap_or(0.5);

        let delta_accuracy = accuracy - previous_accuracy;

        // Meta-UTL score: are we getting better at learning?
        let meta_semantic = delta_accuracy; // Improvement in meaning capture
        let meta_causal = self.compute_causal_learning_improvement(); // Improvement in causality

        sigmoid(2.0 * (meta_semantic * meta_causal * self.meta_lr))
    }

    /// Self-modify UTL parameters based on meta-score
    pub fn meta_learn(&mut self) {
        let meta_score = self.meta_score();

        if meta_score < 0.5 {
            // Learning is stagnating - increase exploration
            self.utl.w_e *= 1.1; // Increase surprise weight
        } else if meta_score > 0.8 {
            // Learning is good - exploit current strategy
            self.utl.w_e *= 0.95; // Decrease surprise weight
        }

        // Record parameter state
        self.parameter_history.push(UTLParameterSnapshot {
            timestamp: Utc::now(),
            tau: self.utl.tau,
            lambda_s: self.utl.lambda_s,
            lambda_c: self.utl.lambda_c,
            w_e: self.utl.w_e,
            accuracy: self.meta_score(),
        });
    }
}
```

### 14.2 Self-Aware Memory Operations

Memory operations that are aware of their own teleological impact:

```rust
/// Self-aware memory store
pub struct SelfAwareMemoryStore {
    /// Primary storage
    storage: TeleologicalStorage,

    /// Meta-UTL for self-monitoring
    meta_utl: MetaUTL,

    /// Memory operation history
    operation_history: Vec<MemoryOperation>,
}

impl SelfAwareMemoryStore {
    /// Store with self-awareness
    pub async fn store_aware(
        &mut self,
        memory: TeleologicalFingerprint,
    ) -> Result<StorageResult> {
        // Pre-store: predict impact on system teleology
        let predicted_impact = self.meta_utl.predict_storage_impact(&memory);

        // Store
        let result = self.storage.store(memory.clone()).await?;

        // Post-store: verify actual impact
        let actual_impact = self.meta_utl.measure_storage_impact(&memory);

        // Meta-learn from prediction error
        let prediction_error = (actual_impact - predicted_impact).abs();
        if prediction_error > 0.1 {
            self.meta_utl.learn_from_error(predicted_impact, actual_impact);
        }

        // Record operation
        self.operation_history.push(MemoryOperation {
            op_type: OperationType::Store,
            memory_id: result.id,
            predicted_impact,
            actual_impact,
            prediction_error,
            timestamp: Utc::now(),
        });

        Ok(result)
    }

    /// Retrieve with self-awareness
    pub async fn retrieve_aware(
        &mut self,
        query: &TeleologicalQuery,
    ) -> Result<RetrievalResult> {
        // Pre-retrieve: predict which memories will be most useful
        let predicted_useful = self.meta_utl.predict_useful_memories(query);

        // Retrieve
        let results = self.storage.retrieve(query).await?;

        // Post-retrieve: measure actual utility
        let actual_useful = self.measure_retrieval_utility(&results, query);

        // Meta-learn from utility prediction
        if (actual_useful - predicted_useful).abs() > 0.1 {
            self.meta_utl.learn_from_retrieval(query, &results, actual_useful);
        }

        Ok(results)
    }

    /// Consolidate memories with self-awareness
    pub async fn consolidate_aware(&mut self) -> Result<ConsolidationReport> {
        // Identify memories for consolidation based on teleological analysis
        let candidates = self.identify_consolidation_candidates().await?;

        // Predict consolidation benefit
        let predicted_benefit = self.meta_utl.predict_consolidation_benefit(&candidates);

        if predicted_benefit < 0.3 {
            return Ok(ConsolidationReport::Skipped { reason: "Insufficient predicted benefit" });
        }

        // Consolidate
        let result = self.storage.consolidate(&candidates).await?;

        // Measure actual benefit
        let actual_benefit = self.meta_utl.measure_consolidation_benefit(&result);

        // Meta-learn
        self.meta_utl.learn_from_consolidation(predicted_benefit, actual_benefit);

        Ok(result)
    }
}
```

---

## 15. What This Replaces & Why It Matters

### 15.1 What Gets Removed

| Component | Old Approach | New Approach | Why It's Better |
|-----------|--------------|--------------|-----------------|
| `FuseMoE` single-vector fusion | Top-4 experts → 1536D | 13-array fingerprint + 5-stage pipeline | 100% information preserved |
| `KnowledgeNode.embedding: Vector1536` | Single fused vector | `TeleologicalFingerprint` | Full semantic decomposition |
| Simple cosine similarity | `cos(a, b)` on fused | Per-space + weighted | Query-specific relevance |
| Static alignment threshold | Fixed θ = 0.7 | Adaptive per-space thresholds | Space-appropriate sensitivity |
| One-time embedding | Embed once, store | Continuous purpose evolution | Temporal awareness |
| No self-monitoring | Silent failures | Meta-UTL self-awareness | Self-correcting system |

### 15.2 Implementation Priority

**Phase 1: Foundation (Immediate)**
1. Replace `Vector1536` with `SemanticFingerprint` struct
2. Implement per-embedder storage schema
3. Add purpose vector computation

**Phase 2: Indexing (Week 2)**
1. Create 13 per-space HNSW indexes + 5-stage pipeline structure
2. Implement purpose pattern index (13D with stage roles)
3. Add goal hierarchy storage

**Phase 3: Advanced (Week 3-4)**
1. Implement Johari quadrant classification
2. Add temporal purpose evolution tracking
3. Implement Meta-UTL self-awareness

### 15.3 The World's Greatest Memory System

This architecture enables **mathematically computed memory** through:

1. **Complete Semantic Capture**: 13 orthogonal meaning dimensions
2. **Teleological Purpose**: Every memory has measurable purpose
3. **Johari Self-Awareness**: System knows what it knows and doesn't know
4. **Temporal Evolution**: Memory purpose changes over time
5. **Meta-Learning**: System learns how to learn better
6. **Binding Coherence**: Distributed representations unify through phase

**The Result**: Memory isn't just stored—it's *understood*, *purposeful*, and *self-aware*.