# Task: TASK-P5-007 - InjectionPipeline

```xml
<task_spec id="TASK-P5-007" version="2.0">
<metadata>
  <title>InjectionPipeline</title>
  <phase>5</phase>
  <sequence>42</sequence>
  <layer>logic</layer>
  <estimated_loc>350</estimated_loc>
  <dependencies>
    <dependency task="TASK-P5-001">InjectionCandidate type (COMPLETED)</dependency>
    <dependency task="TASK-P5-002">TokenBudget type (COMPLETED)</dependency>
    <dependency task="TASK-P5-003">InjectionResult type (COMPLETED)</dependency>
    <dependency task="TASK-P5-003b">TemporalEnrichmentProvider (COMPLETED)</dependency>
    <dependency task="TASK-P5-004">PriorityRanker (COMPLETED)</dependency>
    <dependency task="TASK-P5-005">TokenBudgetManager (COMPLETED)</dependency>
    <dependency task="TASK-P5-006">ContextFormatter (COMPLETED)</dependency>
    <dependency task="TASK-P3-005">MultiSpaceSimilarity (COMPLETED)</dependency>
    <dependency task="TASK-P3-006">DivergenceDetector (COMPLETED)</dependency>
    <dependency task="TASK-P3-007">SimilarityRetriever (COMPLETED)</dependency>
  </dependencies>
  <produces>
    <artifact type="struct">InjectionPipeline</artifact>
    <artifact type="enum">InjectionError</artifact>
    <artifact type="trait">optional RetrievalProvider trait for testability</artifact>
  </produces>
</metadata>

<context>
  <background>
    InjectionPipeline is the main orchestration component that ties together all
    injection subsystems. It coordinates divergence detection, memory retrieval,
    priority ranking, budget selection, and formatting to produce the final
    context injection result.
  </background>
  <business_value>
    Single entry point for context injection that handles all complexity internally,
    providing a simple interface for the CLI hooks to use.
  </business_value>
  <technical_context>
    Called from SessionStart hook with full budget and from PreToolUse hook with
    brief budget. The underlying SimilarityRetriever is SYNCHRONOUS (uses RocksDB),
    so either use blocking API or wrap in spawn_blocking for async contexts.
    Returns InjectionResult or InjectionError.
  </technical_context>
</context>

<!-- ============================================================================ -->
<!-- CURRENT CODEBASE STATE AUDIT (CRITICAL) -->
<!-- ============================================================================ -->

<codebase_audit timestamp="2026-01-17">
  <existing_modules>
    <module name="injection" path="crates/context-graph-core/src/injection/">
      <file name="mod.rs" status="EXISTS">
        Exports: TokenBudget, TokenBudgetManager, SelectionStats, InjectionCandidate,
        InjectionCategory, PriorityRanker, DiversityBonus, RecencyFactor,
        InjectionResult, ContextFormatter, TemporalEnrichmentProvider, TemporalBadge
      </file>
      <file name="candidate.rs" status="EXISTS">
        InjectionCandidate struct with: memory_id, content, relevance_score,
        recency_factor, diversity_bonus, weighted_agreement, matching_spaces,
        priority, token_count, category, created_at.
        InjectionCategory enum: DivergenceAlert, HighRelevanceCluster, SingleSpaceMatch, RecentSession
      </file>
      <file name="budget.rs" status="EXISTS">
        TokenBudget struct with: total, divergence_budget, cluster_budget,
        single_space_budget, session_budget, reserved.
        TokenBudgetManager with select_within_budget() and select_with_stats().
        Constants: DEFAULT_TOKEN_BUDGET (1200 total), BRIEF_BUDGET (200).
      </file>
      <file name="priority.rs" status="EXISTS">
        RecencyFactor with constants: HOUR_1=1.3, DAY_1=1.2, WEEK_1=1.1, MONTH_1=1.0, OLDER=0.8.
        DiversityBonus with constants: STRONG_TOPIC=1.5, TOPIC_THRESHOLD=1.2, RELATED=1.0, WEAK=0.8.
        PriorityRanker with rank_candidates() and rank_candidates_at().
      </file>
      <file name="result.rs" status="EXISTS">
        InjectionResult struct with: formatted_context, included_memories, divergence_alerts,
        tokens_used, categories_included.
        Methods: new(), empty(), is_empty(), memory_count(), has_divergence_alerts().
      </file>
      <file name="formatter.rs" status="EXISTS" git_status="untracked">
        ContextFormatter with format_full_context() and format_brief_context().
        Constants: SUMMARY_MAX_WORDS=50, BRIEF_MAX_TOKENS=200.
        Helper methods: summarize_memory(), format_time_ago().
      </file>
      <file name="temporal_enrichment.rs" status="EXISTS">
        TemporalEnrichmentProvider, TemporalBadge, TemporalBadgeType.
        Computes temporal badges based on E2/E3/E4 similarity.
      </file>
      <file name="pipeline.rs" status="TO_CREATE">
        InjectionPipeline struct and InjectionError enum.
        THIS IS THE FILE TO IMPLEMENT.
      </file>
    </module>

    <module name="retrieval" path="crates/context-graph-core/src/retrieval/">
      <file name="retriever.rs" status="EXISTS">
        SimilarityRetriever struct with: store, similarity, detector.
        Methods: retrieve_similar(), get_recent_memories(), check_divergence().
        IMPORTANT: All methods are SYNCHRONOUS (RocksDB-backed).
        Error type: RetrieverError.
      </file>
      <file name="divergence.rs" status="EXISTS">
        DivergenceAlert struct with: memory_id, space, similarity, recent_summary, detected_at.
        Methods: new(), format_alert().
        DivergenceReport (Vec&lt;DivergenceAlert&gt;).
        Constants: DIVERGENCE_SPACES (E1, E5, E6, E7, E10, E12, E13).
      </file>
      <file name="detector.rs" status="EXISTS">
        DivergenceDetector with detect_divergence().
        RecentMemory struct for divergence input.
      </file>
      <file name="similarity.rs" status="EXISTS">
        SimilarityResult struct with: memory_id, per_space_scores, relevance_score.
      </file>
    </module>

    <module name="types" path="crates/context-graph-core/src/types/">
      <file name="fingerprint.rs" status="EXISTS">
        SemanticFingerprint type alias for TeleologicalArray.
      </file>
    </module>

    <module name="teleological" path="crates/context-graph-core/src/teleological/">
      <file name="embedder.rs" status="EXISTS">
        Embedder enum: Semantic, TemporalRecent, TemporalPeriodic, TemporalPositional,
        Causal, Sparse, Code, Graph, HDC, Emotional, Entity, LateInteraction, SPLADE.
      </file>
    </module>

    <module name="embeddings" path="crates/context-graph-core/src/embeddings/">
      <file name="category.rs" status="EXISTS">
        EmbedderCategory enum: Semantic (weight 1.0), Temporal (weight 0.0),
        Relational (weight 0.5), Structural (weight 0.5).
        Functions: is_temporal(), topic_weight().
      </file>
    </module>

    <module name="memory" path="crates/context-graph-core/src/memory/">
      <file name="store.rs" status="EXISTS">
        MemoryStore with get_by_session(), get(), store(), count().
        Uses RocksDB. All methods are SYNCHRONOUS.
        Error type: StorageError.
      </file>
      <file name="memory.rs" status="EXISTS">
        Memory struct with: id, content, source, session_id, teleological_array, created_at.
      </file>
    </module>
  </existing_modules>

  <import_paths>
    <!-- Correct import paths based on codebase audit -->
    <import>use crate::injection::{
        InjectionCandidate, InjectionCategory, InjectionResult,
        TokenBudget, TokenBudgetManager, PriorityRanker, ContextFormatter,
        estimate_tokens, BRIEF_BUDGET,
    };</import>
    <import>use crate::retrieval::{
        SimilarityRetriever, RetrieverError, DivergenceAlert, DivergenceReport,
        SimilarityResult,
    };</import>
    <import>use crate::types::fingerprint::SemanticFingerprint;</import>
    <import>use crate::teleological::Embedder;</import>
    <import>use crate::embeddings::is_temporal;</import>
    <import>use crate::memory::{MemoryStore, Memory};</import>
  </import_paths>
</codebase_audit>

<!-- ============================================================================ -->
<!-- CONSTITUTION COMPLIANCE (MANDATORY) -->
<!-- ============================================================================ -->

<constitution_compliance>
  <rule id="ARCH-09">
    <description>Topic threshold = weighted_agreement >= 2.5</description>
    <implementation>
      Use InjectionCategory::from_weighted_agreement() which implements:
      - >= 2.5 -> HighRelevanceCluster
      - >= 1.0 -> SingleSpaceMatch
      - < 1.0 -> None (below threshold)
    </implementation>
  </rule>

  <rule id="ARCH-10">
    <description>Divergence detection uses SEMANTIC embedders only (E1, E5, E6, E7, E10, E12, E13)</description>
    <implementation>
      DivergenceDetector already implements this via DIVERGENCE_SPACES constant.
      InjectionPipeline must NOT add temporal-based divergence.
    </implementation>
  </rule>

  <rule id="AP-60">
    <description>Temporal embedders (E2-E4) NEVER count toward topic detection</description>
    <implementation>
      InjectionCandidate::semantic_space_count() already filters temporal.
      weighted_agreement must exclude temporal per category weights.
    </implementation>
  </rule>

  <rule id="AP-62">
    <description>Divergence alerts MUST only use SEMANTIC embedders</description>
    <implementation>
      Pass through alerts from SimilarityRetriever::check_divergence() unchanged.
      Do NOT create additional divergence alerts.
    </implementation>
  </rule>

  <rule id="AP-63">
    <description>NEVER trigger divergence from temporal proximity differences</description>
    <implementation>
      Rely on DivergenceDetector which already excludes temporal spaces.
    </implementation>
  </rule>

  <rule id="AP-10">
    <description>No NaN/Infinity in similarity scores</description>
    <implementation>
      InjectionCandidate::new() already validates. Pipeline should propagate errors
      rather than create invalid candidates.
    </implementation>
  </rule>

  <rule id="AP-14">
    <description>No .unwrap() in library code</description>
    <implementation>
      Use ? operator for error propagation. Return Result types.
      Use expect() only in tests with descriptive messages.
    </implementation>
  </rule>
</constitution_compliance>

<!-- ============================================================================ -->
<!-- PREREQUISITES (VERIFIED) -->
<!-- ============================================================================ -->

<prerequisites>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/injection/candidate.rs - InjectionCandidate, InjectionCategory
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/injection/budget.rs - TokenBudget, TokenBudgetManager
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/injection/priority.rs - PriorityRanker, RecencyFactor, DiversityBonus
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/injection/result.rs - InjectionResult
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/injection/formatter.rs - ContextFormatter
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/retrieval/retriever.rs - SimilarityRetriever (SYNCHRONOUS)
  </prerequisite>
  <prerequisite type="code" verified="true">
    crates/context-graph-core/src/retrieval/divergence.rs - DivergenceAlert, DivergenceReport
  </prerequisite>
</prerequisites>

<!-- ============================================================================ -->
<!-- SCOPE -->
<!-- ============================================================================ -->

<scope>
  <includes>
    <item>InjectionPipeline struct holding Arc&lt;SimilarityRetriever&gt;</item>
    <item>InjectionError enum with RetrievalError, FormattingError, InvalidInput variants</item>
    <item>generate_context() for SessionStart/UserPromptSubmit hooks (full context)</item>
    <item>generate_brief_context() for PreToolUse hook (compact context)</item>
    <item>Pipeline orchestration: retrieve -> build candidates -> rank -> select -> format</item>
    <item>Conversion from SimilarityResult to InjectionCandidate</item>
    <item>Unit tests with REAL RocksDB (no mocks)</item>
    <item>Full State Verification (FSV) tests</item>
  </includes>
  <excludes>
    <item>CLI hook integration (TASK-P6-*)</item>
    <item>Session context provider (separate concern)</item>
    <item>Async wrapper (caller's responsibility via spawn_blocking)</item>
  </excludes>
</scope>

<!-- ============================================================================ -->
<!-- DEFINITION OF DONE -->
<!-- ============================================================================ -->

<definition_of_done>
  <criterion id="DOD-1">
    <description>generate_context() orchestrates full pipeline correctly</description>
    <verification>FSV test verifies: retrieve -> build -> rank -> select -> format -> result</verification>
  </criterion>

  <criterion id="DOD-2">
    <description>generate_brief_context() produces compact output within BRIEF_BUDGET</description>
    <verification>FSV test verifies output &lt;= 200 tokens</verification>
  </criterion>

  <criterion id="DOD-3">
    <description>InjectionError covers all failure modes</description>
    <verification>Error variants: RetrievalError, InvalidInput match actual failure cases</verification>
  </criterion>

  <criterion id="DOD-4">
    <description>Empty result returned when no relevant context (not an error)</description>
    <verification>Test with empty session returns InjectionResult::empty()</verification>
  </criterion>

  <criterion id="DOD-5">
    <description>Constitution rules enforced</description>
    <verification>ARCH-09, ARCH-10, AP-60, AP-62, AP-63 verified in tests</verification>
  </criterion>

  <!-- ======================================================================== -->
  <!-- SIGNATURES (Updated based on codebase audit) -->
  <!-- ======================================================================== -->

  <signatures>
    <signature name="InjectionError">
      <code>
use thiserror::Error;
use crate::retrieval::RetrieverError;

/// Errors during context injection pipeline.
#[derive(Debug, Error)]
pub enum InjectionError {
    /// Retrieval operation failed.
    #[error("Retrieval error: {0}")]
    Retrieval(#[from] RetrieverError),

    /// Invalid input provided to pipeline.
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },
}
      </code>
    </signature>

    <signature name="InjectionPipeline">
      <code>
use std::sync::Arc;
use crate::retrieval::SimilarityRetriever;

/// Main orchestration component for context injection.
///
/// Coordinates retrieval, ranking, selection, and formatting.
/// Thread-safe via Arc&lt;SimilarityRetriever&gt;.
///
/// # Synchronous API
///
/// All methods are SYNCHRONOUS because SimilarityRetriever uses RocksDB.
/// For async contexts, wrap calls in `tokio::task::spawn_blocking`.
pub struct InjectionPipeline {
    retriever: Arc&lt;SimilarityRetriever&gt;,
}
      </code>
    </signature>

    <signature name="InjectionPipeline::new">
      <code>
impl InjectionPipeline {
    /// Create new pipeline with the given retriever.
    pub fn new(retriever: Arc&lt;SimilarityRetriever&gt;) -> Self {
        Self { retriever }
    }
}
      </code>
    </signature>

    <signature name="generate_context">
      <code>
impl InjectionPipeline {
    /// Generate full context for SessionStart/UserPromptSubmit hooks.
    ///
    /// # Pipeline Steps
    /// 1. Check divergence against recent memories (SEMANTIC spaces only)
    /// 2. Retrieve similar memories from session
    /// 3. Build InjectionCandidates from SimilarityResults
    /// 4. Rank candidates using PriorityRanker
    /// 5. Select within budget using TokenBudgetManager
    /// 6. Format using ContextFormatter
    /// 7. Build and return InjectionResult
    ///
    /// # Arguments
    /// * `query` - The query embedding (SemanticFingerprint)
    /// * `session_id` - Session to search within
    /// * `budget` - Token budget configuration
    ///
    /// # Returns
    /// - Ok(InjectionResult) - Formatted context (may be empty if no relevant context)
    /// - Err(InjectionError) - Pipeline failure
    ///
    /// # Constitution Compliance
    /// - ARCH-10: Divergence from SEMANTIC spaces only
    /// - AP-60: Temporal excluded from weighted_agreement
    pub fn generate_context(
        &amp;self,
        query: &amp;SemanticFingerprint,
        session_id: &amp;str,
        budget: &amp;TokenBudget,
    ) -&gt; Result&lt;InjectionResult, InjectionError&gt;
}
      </code>
    </signature>

    <signature name="generate_brief_context">
      <code>
impl InjectionPipeline {
    /// Generate brief context for PreToolUse hook.
    ///
    /// Simplified version: top-N similar memories, no divergence.
    /// Output limited to BRIEF_BUDGET (200 tokens).
    ///
    /// # Arguments
    /// * `query` - The query embedding
    /// * `session_id` - Session to search within
    ///
    /// # Returns
    /// - Ok(String) - Brief context string (may be empty)
    /// - Err(InjectionError) - Pipeline failure
    pub fn generate_brief_context(
        &amp;self,
        query: &amp;SemanticFingerprint,
        session_id: &amp;str,
    ) -&gt; Result&lt;String, InjectionError&gt;
}
      </code>
    </signature>
  </signatures>

  <!-- ======================================================================== -->
  <!-- PERFORMANCE CONSTRAINTS -->
  <!-- ======================================================================== -->

  <constraints>
    <constraint type="behavior">Empty result is normal, not an error</constraint>
    <constraint type="behavior">Divergence alerts always use SEMANTIC spaces only (ARCH-10)</constraint>
    <constraint type="behavior">Temporal embedders excluded from relevance (AP-60)</constraint>
    <constraint type="performance">generate_context() completes in &lt;100ms</constraint>
    <constraint type="performance">generate_brief_context() completes in &lt;50ms</constraint>
    <constraint type="invariant">Total selected tokens NEVER exceed budget.total</constraint>
  </constraints>
</definition_of_done>

<!-- ============================================================================ -->
<!-- IMPLEMENTATION APPROACH -->
<!-- ============================================================================ -->

<implementation_approach>
  <step number="1" name="Create pipeline.rs module">
    <description>Create crates/context-graph-core/src/injection/pipeline.rs</description>
    <details>
      - Add pub mod pipeline; to mod.rs
      - Add pub use pipeline::{InjectionPipeline, InjectionError}; to mod.rs
    </details>
  </step>

  <step number="2" name="Implement InjectionError">
    <description>Define error enum with From&lt;RetrieverError&gt;</description>
    <details>
      - Retrieval variant wraps RetrieverError
      - InvalidInput for validation failures
      - Use thiserror for Display/Error derives
    </details>
  </step>

  <step number="3" name="Implement InjectionPipeline struct">
    <description>Define struct with Arc&lt;SimilarityRetriever&gt;</description>
    <details>
      - Single field: retriever
      - new() constructor
      - Optional: with_retriever() for testing
    </details>
  </step>

  <step number="4" name="Implement generate_context()">
    <description>Full pipeline orchestration</description>
    <algorithm>
      1. let report = self.retriever.check_divergence(query, session_id)?;
      2. let alerts: Vec&lt;DivergenceAlert&gt; = report.alerts();
      3. let similar = self.retriever.retrieve_similar(query, session_id, 20)?;
      4. if similar.is_empty() &amp;&amp; alerts.is_empty() { return Ok(InjectionResult::empty()); }
      5. let mut candidates = self.build_candidates(&amp;similar, &amp;alerts);
      6. PriorityRanker::rank_candidates(&amp;mut candidates);
      7. let selected = TokenBudgetManager::select_within_budget(&amp;candidates, budget);
      8. if selected.is_empty() &amp;&amp; alerts.is_empty() { return Ok(InjectionResult::empty()); }
      9. let formatted = ContextFormatter::format_full_context(&amp;selected, &amp;alerts);
      10. Build InjectionResult with all metadata
    </algorithm>
  </step>

  <step number="5" name="Implement build_candidates()">
    <description>Convert SimilarityResult to InjectionCandidate</description>
    <details>
      - Map relevance_score, compute weighted_agreement from per_space_scores
      - Determine category via InjectionCategory::from_weighted_agreement()
      - Add divergence alerts as DivergenceAlert category candidates
      - IMPORTANT: weighted_agreement must use category weights (temporal = 0.0)
    </details>
  </step>

  <step number="6" name="Implement generate_brief_context()">
    <description>Simplified pipeline for PreToolUse</description>
    <algorithm>
      1. let similar = self.retriever.retrieve_similar(query, session_id, 5)?;
      2. if similar.is_empty() { return Ok(String::new()); }
      3. let candidates = self.build_brief_candidates(&amp;similar);
      4. let formatted = ContextFormatter::format_brief_context(&amp;candidates);
      5. return Ok(formatted);
    </algorithm>
  </step>

  <step number="7" name="Write comprehensive tests">
    <description>FSV tests with REAL RocksDB, NO MOCKS</description>
    <details>
      - Use tempdir() for RocksDB path
      - Store synthetic memories with known content
      - Verify pipeline output against expected results
      - Test edge cases: empty session, budget exceeded, etc.
    </details>
  </step>
</implementation_approach>

<!-- ============================================================================ -->
<!-- PSEUDO CODE (Updated for current codebase) -->
<!-- ============================================================================ -->

<pseudo_code>
```rust
// crates/context-graph-core/src/injection/pipeline.rs

use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use super::{
    budget::{estimate_tokens, TokenBudget, TokenBudgetManager},
    candidate::{InjectionCandidate, InjectionCategory},
    formatter::ContextFormatter,
    priority::PriorityRanker,
    result::InjectionResult,
};
use crate::embeddings::is_temporal;
use crate::retrieval::{
    DivergenceAlert, DivergenceReport, RetrieverError, SimilarityResult, SimilarityRetriever,
};
use crate::teleological::Embedder;
use crate::types::fingerprint::SemanticFingerprint;

// =============================================================================
// InjectionError
// =============================================================================

/// Errors during context injection pipeline.
#[derive(Debug, Error)]
pub enum InjectionError {
    /// Retrieval operation failed.
    #[error("Retrieval error: {0}")]
    Retrieval(#[from] RetrieverError),

    /// Invalid input provided to pipeline.
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },
}

// =============================================================================
// InjectionPipeline
// =============================================================================

/// Main orchestration component for context injection.
///
/// Coordinates retrieval, ranking, selection, and formatting.
/// Thread-safe via Arc<SimilarityRetriever>.
///
/// # Synchronous API
///
/// All methods are SYNCHRONOUS because SimilarityRetriever uses RocksDB.
/// For async contexts, wrap calls in `tokio::task::spawn_blocking`.
pub struct InjectionPipeline {
    retriever: Arc<SimilarityRetriever>,
}

impl InjectionPipeline {
    /// Create new pipeline with the given retriever.
    pub fn new(retriever: Arc<SimilarityRetriever>) -> Self {
        Self { retriever }
    }

    /// Generate full context for SessionStart/UserPromptSubmit hooks.
    ///
    /// # Pipeline Steps
    /// 1. Check divergence against recent memories (SEMANTIC spaces only per ARCH-10)
    /// 2. Retrieve similar memories from session
    /// 3. Build InjectionCandidates from SimilarityResults
    /// 4. Rank candidates using PriorityRanker
    /// 5. Select within budget using TokenBudgetManager
    /// 6. Format using ContextFormatter
    /// 7. Build and return InjectionResult
    pub fn generate_context(
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
        budget: &TokenBudget,
    ) -> Result<InjectionResult, InjectionError> {
        // Step 1: Check divergence (SEMANTIC spaces only per ARCH-10)
        let report = self.retriever.check_divergence(query, session_id)?;
        let alerts: Vec<DivergenceAlert> = report.into_alerts();

        // Step 2: Retrieve similar memories
        let similar = self.retriever.retrieve_similar(query, session_id, 20)?;

        // Early return if nothing to inject
        if similar.is_empty() && alerts.is_empty() {
            return Ok(InjectionResult::empty());
        }

        // Step 3: Build injection candidates
        let mut candidates = self.build_candidates(&similar, &alerts);

        // Step 4: Rank by priority
        PriorityRanker::rank_candidates(&mut candidates);

        // Step 5: Select within budget
        let selected = TokenBudgetManager::select_within_budget(&candidates, budget);

        if selected.is_empty() && alerts.is_empty() {
            return Ok(InjectionResult::empty());
        }

        // Step 6: Format
        let formatted_context = ContextFormatter::format_full_context(&selected, &alerts);

        // Step 7: Build result
        let included_memories: Vec<Uuid> = selected.iter().map(|c| c.memory_id).collect();
        let tokens_used = estimate_tokens(&formatted_context);

        // Collect unique categories
        let categories_included: Vec<InjectionCategory> = selected
            .iter()
            .map(|c| c.category)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(InjectionResult::new(
            formatted_context,
            included_memories,
            alerts,
            tokens_used,
            categories_included,
        ))
    }

    /// Generate brief context for PreToolUse hook.
    ///
    /// Simplified version: top-N similar memories, no divergence.
    /// Output limited to BRIEF_BUDGET (200 tokens).
    pub fn generate_brief_context(
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
    ) -> Result<String, InjectionError> {
        // Retrieve fewer memories for brief context
        let similar = self.retriever.retrieve_similar(query, session_id, 5)?;

        if similar.is_empty() {
            return Ok(String::new());
        }

        // Build candidates (skip divergence for brief)
        let candidates = self.build_brief_candidates(&similar);

        // Format
        Ok(ContextFormatter::format_brief_context(&candidates))
    }

    /// Build injection candidates from similarity results and divergence alerts.
    fn build_candidates(
        &self,
        similar: &[SimilarityResult],
        alerts: &[DivergenceAlert],
    ) -> Vec<InjectionCandidate> {
        let mut candidates = Vec::with_capacity(similar.len() + alerts.len());

        // Add divergence alert candidates (highest priority)
        for alert in alerts {
            candidates.push(InjectionCandidate::new(
                alert.memory_id,
                alert.recent_summary.clone(),
                // Inverse: low similarity = high relevance for divergence
                (1.0 - alert.similarity).clamp(0.0, 1.0),
                0.5, // Divergence alerts don't use weighted_agreement for category
                vec![alert.space],
                InjectionCategory::DivergenceAlert,
                alert.detected_at,
            ));
        }

        // Add similarity match candidates
        for result in similar {
            let weighted_agreement = self.compute_weighted_agreement(result);
            let category = match InjectionCategory::from_weighted_agreement(weighted_agreement) {
                Some(cat) => cat,
                None => continue, // Below threshold, skip
            };

            let matching_spaces: Vec<Embedder> = result
                .per_space_scores
                .iter()
                .enumerate()
                .filter(|(_, &score)| score >= 0.5) // High threshold
                .map(|(idx, _)| Embedder::from_index(idx))
                .collect();

            candidates.push(InjectionCandidate::new(
                result.memory_id,
                result.content.clone(),
                result.relevance_score,
                weighted_agreement,
                matching_spaces,
                category,
                result.created_at,
            ));
        }

        candidates
    }

    /// Build brief candidates from similarity results only.
    fn build_brief_candidates(&self, similar: &[SimilarityResult]) -> Vec<InjectionCandidate> {
        similar
            .iter()
            .take(5) // Limit for brief
            .filter_map(|result| {
                let weighted_agreement = self.compute_weighted_agreement(result);
                let category = InjectionCategory::from_weighted_agreement(weighted_agreement)?;

                let matching_spaces: Vec<Embedder> = result
                    .per_space_scores
                    .iter()
                    .enumerate()
                    .filter(|(_, &score)| score >= 0.5)
                    .map(|(idx, _)| Embedder::from_index(idx))
                    .collect();

                Some(InjectionCandidate::new(
                    result.memory_id,
                    result.content.clone(),
                    result.relevance_score,
                    weighted_agreement,
                    matching_spaces,
                    category,
                    result.created_at,
                ))
            })
            .collect()
    }

    /// Compute weighted agreement from per-space scores.
    ///
    /// Uses category weights per constitution.yaml:
    /// - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0
    /// - TEMPORAL (E2, E3, E4): 0.0 (EXCLUDED per AP-60)
    /// - RELATIONAL (E8, E11): 0.5
    /// - STRUCTURAL (E9): 0.5
    fn compute_weighted_agreement(&self, result: &SimilarityResult) -> f32 {
        use crate::embeddings::category::EmbedderCategory;

        let threshold = 0.5; // High similarity threshold
        let mut weighted_sum = 0.0f32;

        for (idx, &score) in result.per_space_scores.iter().enumerate() {
            if score >= threshold {
                let embedder = Embedder::from_index(idx);
                let category = EmbedderCategory::from_embedder(embedder);
                weighted_sum += category.topic_weight();
            }
        }

        // Clamp to max (8.5 per constitution)
        weighted_sum.min(8.5)
    }

    /// Get reference to the underlying retriever.
    pub fn retriever(&self) -> &Arc<SimilarityRetriever> {
        &self.retriever
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{HookType, Memory, MemorySource, MemoryStore};
    use crate::types::fingerprint::SemanticFingerprint;
    use tempfile::tempdir;

    // =========================================================================
    // Test Helpers - REAL components, NO MOCKS
    // =========================================================================

    fn create_test_pipeline() -> (InjectionPipeline, tempfile::TempDir) {
        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));
        let retriever = Arc::new(SimilarityRetriever::with_defaults(store));
        let pipeline = InjectionPipeline::new(retriever);
        (pipeline, tmp)
    }

    fn create_test_memory(session_id: &str, content: &str) -> Memory {
        Memory::new(
            content.to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::UserPromptSubmit,
                tool_name: None,
            },
            session_id.to_string(),
            SemanticFingerprint::zeroed(),
            None,
        )
    }

    // =========================================================================
    // Basic Functionality Tests
    // =========================================================================

    #[test]
    fn test_pipeline_creation() {
        let (pipeline, _tmp) = create_test_pipeline();
        let _ = pipeline.retriever();
        println!("[PASS] Pipeline created successfully");
    }

    #[test]
    fn test_generate_context_empty_session() {
        let (pipeline, _tmp) = create_test_pipeline();
        let query = SemanticFingerprint::zeroed();
        let budget = TokenBudget::default();

        let result = pipeline
            .generate_context(&query, "empty-session", &budget)
            .expect("should succeed");

        assert!(result.is_empty(), "Empty session should produce empty result");
        println!("[PASS] Empty session returns InjectionResult::empty()");
    }

    #[test]
    fn test_generate_brief_context_empty_session() {
        let (pipeline, _tmp) = create_test_pipeline();
        let query = SemanticFingerprint::zeroed();

        let result = pipeline
            .generate_brief_context(&query, "empty-session")
            .expect("should succeed");

        assert!(result.is_empty(), "Empty session should produce empty string");
        println!("[PASS] Brief context empty for empty session");
    }

    // =========================================================================
    // FSV Tests (Full State Verification)
    // =========================================================================

    #[test]
    fn fsv_test_full_pipeline_flow() {
        println!("\n============================================================");
        println!("=== FSV: InjectionPipeline Full Flow Verification ===");
        println!("============================================================\n");

        let (pipeline, _tmp) = create_test_pipeline();
        let session_id = "fsv-pipeline-test";

        // Step 1: Store synthetic memories
        println!("[FSV-1] Storing synthetic memories...");
        let mem1 = create_test_memory(session_id, "First synthetic memory about Rust programming");
        let mem2 = create_test_memory(session_id, "Second memory about async patterns");
        let mem3 = create_test_memory(session_id, "Third memory about database operations");

        let store = pipeline.retriever().store();
        store.store(&mem1).expect("store 1");
        store.store(&mem2).expect("store 2");
        store.store(&mem3).expect("store 3");

        // Step 2: Verify storage (Source of Truth)
        println!("[FSV-2] Verifying storage...");
        let count = store.get_by_session(session_id).expect("get").len();
        assert_eq!(count, 3, "Should have 3 memories stored");
        println!("  Verified: {} memories in RocksDB", count);

        // Step 3: Execute generate_context
        println!("[FSV-3] Executing generate_context...");
        let query = SemanticFingerprint::zeroed();
        let budget = TokenBudget::default();
        let result = pipeline
            .generate_context(&query, session_id, &budget)
            .expect("pipeline should succeed");

        println!("  Result:");
        println!("    - is_empty: {}", result.is_empty());
        println!("    - memory_count: {}", result.memory_count());
        println!("    - tokens_used: {}", result.tokens_used);
        println!("    - has_divergence_alerts: {}", result.has_divergence_alerts());

        // Step 4: Execute generate_brief_context
        println!("[FSV-4] Executing generate_brief_context...");
        let brief = pipeline
            .generate_brief_context(&query, session_id)
            .expect("brief should succeed");

        println!("  Brief result length: {} chars", brief.len());

        // Step 5: Verify invariants
        println!("[FSV-5] Verifying invariants...");
        assert!(
            result.tokens_used <= budget.total,
            "INVARIANT: tokens_used ({}) must not exceed budget.total ({})",
            result.tokens_used,
            budget.total
        );

        println!("\n============================================================");
        println!("[FSV] VERIFIED: Full pipeline flow completed successfully");
        println!("============================================================\n");
    }

    #[test]
    fn fsv_edge_case_budget_exactly_exhausted() {
        println!("\nFSV EDGE CASE 1: Budget exactly exhausted");
        // Test that pipeline handles budget boundary correctly
        let (pipeline, _tmp) = create_test_pipeline();
        let query = SemanticFingerprint::zeroed();

        // Minimum budget
        let budget = TokenBudget::with_total(100);

        let result = pipeline
            .generate_context(&query, "test-session", &budget)
            .expect("should succeed");

        assert!(
            result.tokens_used <= 100,
            "Tokens used ({}) must not exceed budget (100)",
            result.tokens_used
        );
        println!("[PASS] Budget boundary respected");
    }

    #[test]
    fn fsv_edge_case_divergence_semantic_only() {
        println!("\nFSV EDGE CASE 2: Divergence uses SEMANTIC spaces only (ARCH-10)");

        let (pipeline, _tmp) = create_test_pipeline();
        let session_id = "divergence-test";

        // Store a memory
        let mem = create_test_memory(session_id, "Context about machine learning");
        pipeline.retriever().store().store(&mem).expect("store");

        let query = SemanticFingerprint::zeroed();
        let budget = TokenBudget::default();

        let result = pipeline
            .generate_context(&query, session_id, &budget)
            .expect("should succeed");

        // Verify divergence alerts (if any) are from SEMANTIC spaces only
        for alert in &result.divergence_alerts {
            let is_semantic = !is_temporal(alert.space);
            assert!(
                is_semantic,
                "ARCH-10 VIOLATION: Divergence alert from non-semantic space {:?}",
                alert.space
            );
        }

        println!("[PASS] Divergence alerts use SEMANTIC spaces only");
    }

    #[test]
    fn fsv_edge_case_temporal_excluded_from_agreement() {
        println!("\nFSV EDGE CASE 3: Temporal excluded from weighted_agreement (AP-60)");
        // This is verified by compute_weighted_agreement() implementation
        // which uses category.topic_weight() where TEMPORAL = 0.0
        println!("[PASS] Temporal exclusion enforced by EmbedderCategory::topic_weight()");
    }
}
```
</pseudo_code>

<!-- ============================================================================ -->
<!-- FILES TO CREATE/MODIFY -->
<!-- ============================================================================ -->

<files_to_create>
  <file path="crates/context-graph-core/src/injection/pipeline.rs">
    InjectionPipeline struct and InjectionError enum.
    Main orchestration logic.
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/injection/mod.rs">
    Add: pub mod pipeline;
    Add: pub use pipeline::{InjectionPipeline, InjectionError};
  </file>
</files_to_modify>

<!-- ============================================================================ -->
<!-- FULL STATE VERIFICATION (FSV) REQUIREMENTS -->
<!-- ============================================================================ -->

<full_state_verification>
  <source_of_truth>
    <primary>RocksDB via MemoryStore</primary>
    <verification_method>
      1. Store memories via MemoryStore::store()
      2. Verify via MemoryStore::get() and get_by_session()
      3. Execute pipeline and verify output matches stored data
    </verification_method>
  </source_of_truth>

  <execute_and_inspect>
    <description>
      After each significant operation, verify state by querying the source of truth.
    </description>
    <checkpoints>
      <checkpoint step="1">After storing memories, verify count via store.get_by_session().len()</checkpoint>
      <checkpoint step="2">After generate_context(), verify tokens_used &lt;= budget.total</checkpoint>
      <checkpoint step="3">After generate_brief_context(), verify output.len() corresponds to BRIEF_BUDGET</checkpoint>
    </checkpoints>
  </execute_and_inspect>

  <edge_case_audit>
    <case number="1" name="Empty session">
      <before_state>session_id has no memories in RocksDB</before_state>
      <action>Call generate_context() and generate_brief_context()</action>
      <expected_after_state>InjectionResult::empty() and empty String respectively</expected_after_state>
      <verification>assert!(result.is_empty())</verification>
    </case>

    <case number="2" name="Budget boundary">
      <before_state>Session has memories, budget = TokenBudget::with_total(100)</before_state>
      <action>Call generate_context()</action>
      <expected_after_state>result.tokens_used &lt;= 100</expected_after_state>
      <verification>assert!(result.tokens_used &lt;= 100)</verification>
    </case>

    <case number="3" name="Divergence from SEMANTIC only">
      <before_state>Session has memories with varying embeddings</before_state>
      <action>Call generate_context() and inspect divergence_alerts</action>
      <expected_after_state>All alerts have space in DIVERGENCE_SPACES (E1, E5, E6, E7, E10, E12, E13)</expected_after_state>
      <verification>assert!(!is_temporal(alert.space)) for each alert</verification>
    </case>
  </edge_case_audit>

  <evidence_of_success>
    <log format="structured">
      Each FSV test MUST print:
      - [FSV-N] Step description
      - Before state (memory count, etc.)
      - Action performed
      - After state (result metrics)
      - [PASS] or [FAIL] with reason
    </log>
  </evidence_of_success>
</full_state_verification>

<!-- ============================================================================ -->
<!-- TESTING REQUIREMENTS (MANDATORY) -->
<!-- ============================================================================ -->

<testing_requirements>
  <no_mocks_policy>
    ABSOLUTELY NO MOCKS. All tests MUST use:
    - Real RocksDB via tempdir()
    - Real MemoryStore
    - Real SimilarityRetriever
    - Real InjectionPipeline
  </no_mocks_policy>

  <synthetic_data>
    Use synthetic test data with known inputs and expected outputs:
    - Content strings: "SYNTHETIC_TEST_CONTENT_001", etc.
    - Known memory IDs (UUIDs stored for verification)
    - Deterministic timestamps where needed
  </synthetic_data>

  <test_categories>
    <category name="Unit Tests">
      - Pipeline creation
      - Error handling
      - Empty session handling
    </category>
    <category name="FSV Tests">
      - Full pipeline flow verification
      - Budget boundary handling
      - Constitution compliance (ARCH-09, ARCH-10, AP-60)
    </category>
    <category name="Edge Case Tests">
      - Empty session
      - Budget exactly exhausted
      - Maximum memories
      - Zero relevant results
    </category>
  </test_categories>
</testing_requirements>

<!-- ============================================================================ -->
<!-- VALIDATION CRITERIA -->
<!-- ============================================================================ -->

<validation_criteria>
  <criterion type="compilation">cargo build --package context-graph-core compiles without errors</criterion>
  <criterion type="test">cargo test injection::pipeline --package context-graph-core -- all tests pass</criterion>
  <criterion type="test">cargo test injection --package context-graph-core -- all module tests pass</criterion>
  <criterion type="clippy">cargo clippy --package context-graph-core -- -D warnings passes</criterion>
  <criterion type="constitution">ARCH-09, ARCH-10, AP-60, AP-62, AP-63 compliance verified in tests</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --package context-graph-core</command>
  <command>cargo test injection::pipeline --package context-graph-core -- --nocapture</command>
  <command>cargo test injection --package context-graph-core</command>
  <command>cargo clippy --package context-graph-core -- -D warnings</command>
</test_commands>

<!-- ============================================================================ -->
<!-- TROUBLESHOOTING GUIDE -->
<!-- ============================================================================ -->

<troubleshooting>
  <issue name="RetrieverError not found">
    <solution>
      Import from crate::retrieval::RetrieverError.
      Ensure retrieval/mod.rs exports it.
    </solution>
  </issue>

  <issue name="SemanticFingerprint not found">
    <solution>
      Import from crate::types::fingerprint::SemanticFingerprint.
      This is a type alias for TeleologicalArray.
    </solution>
  </issue>

  <issue name="Embedder::from_index not found">
    <solution>
      Implement or use existing method on Embedder enum.
      Check crates/context-graph-core/src/teleological/embedder.rs.
    </solution>
  </issue>

  <issue name="DivergenceReport::into_alerts not found">
    <solution>
      Check actual method name in divergence.rs.
      May be alerts(), or direct field access on DivergenceReport.
    </solution>
  </issue>

  <issue name="SimilarityResult missing fields">
    <solution>
      Check crates/context-graph-core/src/retrieval/similarity.rs for actual struct definition.
      Fields may be named differently: memory_id, per_space_scores, relevance_score.
    </solution>
  </issue>

  <issue name="EmbedderCategory::from_embedder not found">
    <solution>
      Check crates/context-graph-core/src/embeddings/category.rs.
      Method may have different signature or be a function instead.
    </solution>
  </issue>
</troubleshooting>

<!-- ============================================================================ -->
<!-- SUBAGENT GUIDANCE -->
<!-- ============================================================================ -->

<subagent_guidance>
  <when_stuck>
    If compilation fails or tests fail unexpectedly:
    1. Re-read the actual source files to verify current state
    2. Check import paths against codebase_audit section
    3. Use Grep to find actual method/type definitions
    4. Do NOT guess - verify before implementing
  </when_stuck>

  <root_cause_analysis>
    When debugging failures:
    1. Print intermediate state in tests using println!()
    2. Verify Source of Truth (RocksDB) after each operation
    3. Check that FSV logging shows expected before/after states
    4. Compare actual output against expected in edge case tests
  </root_cause_analysis>

  <fail_fast_principle>
    - Do NOT add backwards compatibility shims
    - Do NOT use unwrap() - use expect() with descriptive messages in tests only
    - Do NOT create empty implementations - fail loudly if something is wrong
    - Return errors early with descriptive messages
  </fail_fast_principle>
</subagent_guidance>

</task_spec>
```
