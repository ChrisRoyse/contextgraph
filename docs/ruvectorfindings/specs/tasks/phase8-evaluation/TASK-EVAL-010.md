# TASK-EVAL-010 — Cross-task transfer (`build_patch_similarity_graph`; E_Diff cosines + top-k truncation)

```xml
<task_spec id="TASK-EVAL-010" version="1.0">

<metadata>
  <title>Implement `build_patch_similarity_graph` in `crates/context-graph-mejepa/src/eval/task_graph_build.rs` — pairwise E_Diff cosines + top-k truncation + idempotent persistence</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-EVAL-07">build_patch_similarity_graph + idempotency on corpus_sha</item>
    <item ref="component:EvalRunner.build_patch_similarity_graph">N² E_Diff cosines; threshold 0.7; top-k 32; CF_MEJEPA_TASK_GRAPH persist</item>
    <item ref="BR-EVAL-07">Corpus-sha-cached; rebuild only on corpus change</item>
    <item ref="ER-EVAL-09">MEJEPA_EVAL_TASK_GRAPH_REBUILD_FAIL on E_Diff dispatch failure</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-002</task_ref><!-- PatchGraph, PatchGraphEdge -->
    <task_ref>TASK-EVAL-003</task_ref><!-- CF_MEJEPA_TASK_GRAPH + CF_MEJEPA_TASK_GRAPH_HISTORY + serializers + key builders -->
    <task_ref>TASK-INSTR-***</task_ref><!-- E_Diff InstrumentSlot encoder available; produces 512-d embedding per task patch -->
    <task_ref>TASK-CORPUS-007</task_ref><!-- Corpus iterator + corpus_sha -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (task_graph_build.rs); 1 modified (runner.rs to expose build_patch_similarity_graph)</estimated_files>
</metadata>

<context>
This task lands the **cross-task transfer graph**: a sparse undirected graph where nodes are tasks (keyed by `task_sha = sha256(patch_bytes)`) and edges are E_Diff cosine similarities `&gt;= 0.7` truncated to top-32 neighbors per node. Phase 3 (TRAINING) reads this graph to upweight similar tasks during mini-batch sampling (cross-task transfer per doc 09 §3.7.5). The build is **idempotent on corpus_sha**: when corpus hasn't changed since last build, return the cached graph from CF_MEJEPA_TASK_GRAPH without recomputing. Performance target: 2,400-task corpus with 512-d E_Diff embeddings → ~2.88M pairwise cosines on candle-cuda batch=1024 ≈ 3 s; top-k truncation O(E log k) ≈ 10 s. Total well under 30-minute eval budget per NFR-EVAL-02. This task is a Phase 8 **writer**; Phase 3 is the **reader** — no direct call coupling, only RocksDB-mediated handoff.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets eval_pipeline_walltime_p50_max_min 30; rocksdb_cfs CF_MEJEPA_TASK_GRAPH naming)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-07; AC-EVAL-05-* cross-task scenarios; ER-EVAL-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (build_patch_similarity_graph 6-step behavior; CF_MEJEPA_TASK_GRAPH key/value format; idempotency rule; performance estimate ~13 s on RTX 5090)</file>
  <file purpose="prior_task">TASK-EVAL-002 (PatchGraph), TASK-EVAL-003 (CF + serializers)</file>
  <file purpose="cross_domain">crates/context-graph-mejepa-instruments/src/instruments/e_diff.rs (E_Diff slot encoder produces 512-d embedding from a patch)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-002 + 003 shipped: PatchGraph value type, CF descriptors, key builders, serializers</check>
  <check>TASK-INSTR-*** shipped: E_Diff slot encoder callable as `encode_e_diff(patch_bytes: &[u8]) -> Result<[f32; 512], InstrError>`</check>
  <check>TASK-CORPUS-007/021 shipped: `Corpus` iterator yielding `(task_sha, patch_bytes, language)` and computed `corpus_sha = sha256(concat(sorted_task_shas))`</check>
  <check>Phase 1 instrument cache shared with subscriber (TASK-SHIFT-006) is reusable here for the 2,400-entry E_Diff embedding cache</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/task_graph_build.rs` with:
      - `pub fn build_patch_similarity_graph(corpus: &Corpus, e_diff: &dyn EDiffEncoder, config: &EvalConfig, storage: &RocksDbStore) -> Result<PatchGraph, GraphError>` per TECH-EVALUATION 6-step behavior:
        1. **Compute corpus_sha**: `let corpus_sha = corpus.compute_sha()?;` (sha256 of sorted task_shas concatenated)
        2. **Idempotency check**: read CF_MEJEPA_TASK_GRAPH metadata header keyed by `b"_meta_corpus_sha"`; if present and equal to `corpus_sha`, deserialize the cached graph and return → `Err(GraphError::CorpusShaUnchanged { cached_at })` is the explicit signal that no rebuild was needed (caller maps to `Ok(cached_graph)` after a fresh load)
        3. **Encode all patches**: for each `(task_sha, patch_bytes)` in corpus, call `e_diff.encode(&patch_bytes)?` → `[f32; 512]`. Failures map to `Err(GraphError::EmbedderUnavailable { embedder: "E_Diff" })` on the first failure. Validate every embedding finite + L2-normalize.
        4. **Pairwise cosines**: build `Vec<PatchGraphEdge>` from upper-triangle iteration; for each pair `(i, j)` with `i < j`, compute `cosine = dot(emb[i], emb[j])`; if `cosine >= config.task_graph_cosine_threshold` (default 0.7), push `PatchGraphEdge { source_task_sha, target_task_sha, cosine }`. Use rayon `par_iter` over `i` for parallelism.
        5. **Top-k truncation**: for each source node, sort its outgoing edges by cosine descending; truncate to `config.task_graph_top_k` (default 32). The graph is undirected but stored bidirectionally for fast O(1) `similar_tasks` lookups.
        6. **Persist**: write the meta header (`b"_meta_corpus_sha"` → corpus_sha bytes, `b"_meta_built_at"` → unix_i64 BE bytes); write each edge to CF_MEJEPA_TASK_GRAPH keyed by `task_graph_edge_key(source, target)`; ALSO write to CF_MEJEPA_TASK_GRAPH_HISTORY keyed by `task_graph_history_key(corpus_sha, source, target)` for time-travel queries; clear stale rows from prior corpus_sha via prefix-scan + delete (atomicity via WriteBatch where available).
        7. Return `Ok(PatchGraph { edges, adjacency, corpus_sha, built_at: unix_i64 })`
      - `pub fn load_cached_graph(storage: &RocksDbStore, corpus_sha: &[u8; 32]) -> Result<Option<PatchGraph>, GraphError>`: read meta header; compare to `corpus_sha`; if match, prefix-scan CF_MEJEPA_TASK_GRAPH and rebuild PatchGraph; else None
      - `pub trait EDiffEncoder: Send + Sync { fn encode(&self, patch_bytes: &[u8]) -> Result<[f32; 512], String>; fn dim(&self) -> usize { 512 } }` — interface SPEC-INSTRUMENTS implements; this task only declares the trait + a TestEDiffEncoder for tests
      - `fn cosine_similarity(a: &[f32; 512], b: &[f32; 512]) -> f32` — assumes pre-normalized inputs; returns dot product; validates finite
      - `fn l2_normalize(v: &mut [f32; 512])` — in-place
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Add `pub fn EvalRunner::build_patch_similarity_graph(&self, corpus: &Corpus) -> Result<PatchGraph, GraphError>` delegating into `task_graph_build::build_patch_similarity_graph`. This is callable independently of `run_holdout_eval` (graph build is corpus-driven, not eval-driven; SPEC-TRAINING calls it whenever it wants the latest graph).
    - Add unit + integration tests in `task_graph_build.rs` `#[cfg(test)] mod tests` for:
      - `build_with_3_tasks_returns_3_edges_when_all_above_threshold` (synthetic embeddings forced cosine = 0.9)
      - `build_with_unrelated_tasks_returns_zero_edges` (cosines &lt; 0.7)
      - `top_k_truncation_caps_edges_per_node` (10 tasks, top_k=3, expect each node has at most 3 outgoing)
      - `idempotent_on_unchanged_corpus_sha` (build once, build again with same corpus → second call returns cached graph; CF row count unchanged)
      - `rebuild_clears_stale_edges_on_corpus_change` (corpus_sha A → 5 edges; corpus_sha B → 2 edges; CF only contains the 2 fresh edges)
      - `embedder_failure_returns_embedder_unavailable_error`
      - `nan_embedding_returns_dim_mismatch_or_nan_error`
      - `cosine_self_similarity_excluded` (graph is upper-triangle only; no self-loops)
      - `history_cf_records_corpus_sha_dimension` (after 2 builds with different corpus_sha, CF_MEJEPA_TASK_GRAPH_HISTORY contains both)
      - `similar_tasks_query_returns_top_k_above_threshold` (tests `PatchGraph::similar_tasks` end-to-end after build)
  </in_scope>
  <out_of_scope>
    - Phase 3 mini-batch sampler (consumes the graph; SPEC-TRAINING owns)
    - E_Diff slot encoder body (TASK-INSTR-*** owns; this task only declares the trait)
    - MCP tool surface (graph is not MCP-exposed; CLI subcommand `eval-build-graph` lands in TASK-EVAL-016)
    - Tier compression of stale graph history (SPEC-HYGIENE)
    - Q1/Q2/Q3 / state-transfer (TASK-EVAL-011, 012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/task_graph_build.rs">
use super::config::EvalConfig;
use super::error::GraphError;
use super::storage::RocksDbStore;
use super::task_graph::{PatchGraph, PatchGraphEdge};

pub trait EDiffEncoder: Send + Sync {
    fn encode(&amp;self, patch_bytes: &amp;[u8]) -&gt; Result&lt;[f32; 512], String&gt;;
    fn dim(&amp;self) -&gt; usize { 512 }
}

pub fn build_patch_similarity_graph(
    corpus: &amp;Corpus,
    e_diff: &amp;dyn EDiffEncoder,
    config: &amp;EvalConfig,
    storage: &amp;RocksDbStore,
) -&gt; Result&lt;PatchGraph, GraphError&gt;;

pub fn load_cached_graph(
    storage: &amp;RocksDbStore,
    corpus_sha: &amp;[u8; 32],
) -&gt; Result&lt;Option&lt;PatchGraph&gt;, GraphError&gt;;

fn cosine_similarity(a: &amp;[f32; 512], b: &amp;[f32; 512]) -&gt; f32;
fn l2_normalize(v: &amp;mut [f32; 512]);
    </signature>
  </signatures>
  <constraints>
    - Pairwise iteration is upper-triangle ONLY (`for i in 0..n { for j in (i+1)..n { ... } }`); avoids double-counting and self-loops
    - L2 normalization in-place before cosine; cosine then equals dot product (deterministic, no division)
    - Cosine result clamped to `[-1, 1]` for numerical safety; reject NaN by `Err(GraphError::DimMismatch { expected: 512, actual: 0 })` (overload — DimMismatch is the canonical NaN/finite error per TECH-EVALUATION)
    - Top-k truncation MUST be **stable**: ties on cosine break by `target_task_sha` ascending (deterministic, byte-equal output across runs)
    - WriteBatch atomicity is **best-effort** — RocksDB may not honor it across CFs in all configurations; document this in module docs and accept that a crash mid-build leaves a partially-stale graph that will be detected by the meta-header mismatch on next build (idempotency is the safety net)
    - rayon parallelism: bounded thread pool (4 threads) to keep GPU memory headroom for E_Diff encoding; scope encoded embeddings to a contiguous Vec before parallel cosine pass
    - All `Err` returns wrapped with `.log_context("eval::task_graph_build")` per `&lt;error_handling&gt;`
    - File length budget: &lt; 500 lines
    - Rust 2021 edition; Rust 1.84+
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 10 passing tests
    - 3-task synthetic input produces deterministic, byte-equal graph across 3 runs
    - Cached load is &lt; 100 ms when corpus_sha unchanged (no recomputation)
    - Idempotency proven: 2 calls to build with same corpus → identical output, CF row count unchanged
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/task_graph_build.rs:
  pub fn build_patch_similarity_graph(corpus, e_diff, config, storage) -&gt; Result&lt;PatchGraph, GraphError&gt; {
      let corpus_sha = corpus.compute_sha()?;
      // 1. idempotency
      if let Some(cached) = load_cached_graph(storage, &corpus_sha)? {
          return Ok(cached);
      }
      // 2. encode
      let entries: Vec&lt;_&gt; = corpus.iter().collect();
      if entries.is_empty() { return Err(GraphError::EmptyCorpus); }
      let mut embeddings: Vec&lt;[f32; 512]&gt; = Vec::with_capacity(entries.len());
      for (sha, patch, _lang) in &entries {
          let mut emb = e_diff.encode(patch).map_err(|e| GraphError::EmbedderUnavailable { embedder: format!("E_Diff: {}", e) })?;
          if emb.iter().any(|x| !x.is_finite()) {
              return Err(GraphError::DimMismatch { expected: 512, actual: 0 });
          }
          l2_normalize(&mut emb);
          embeddings.push(emb);
      }
      // 3. pairwise cosines (upper triangle, parallel by source)
      let n = entries.len();
      let edges: Vec&lt;PatchGraphEdge&gt; = (0..n).into_par_iter().flat_map(|i| {
          let mut local = Vec::new();
          for j in (i+1)..n {
              let cos = cosine_similarity(&embeddings[i], &embeddings[j]).clamp(-1.0, 1.0);
              if cos &gt;= config.task_graph_cosine_threshold {
                  local.push(PatchGraphEdge { source_task_sha: entries[i].0, target_task_sha: entries[j].0, cosine: cos });
              }
          }
          local
      }).collect();
      // 4. top-k truncation per source (stable)
      let mut adj: BTreeMap&lt;[u8; 32], Vec&lt;([u8; 32], f32)&gt;&gt; = BTreeMap::new();
      for e in &edges {
          adj.entry(e.source_task_sha).or_default().push((e.target_task_sha, e.cosine));
          adj.entry(e.target_task_sha).or_default().push((e.source_task_sha, e.cosine)); // bidirectional
      }
      for (_src, neighbors) in adj.iter_mut() {
          neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then(a.0.cmp(&b.0)));
          neighbors.truncate(config.task_graph_top_k);
      }
      let truncated_edges: Vec&lt;PatchGraphEdge&gt; = adj.iter().flat_map(|(src, ns)| ns.iter().map(move |(tgt, cos)| PatchGraphEdge { source_task_sha: *src, target_task_sha: *tgt, cosine: *cos })).collect();

      // 5. persist (clear stale + write fresh + meta header)
      let cf = storage.cf_handle(CF_MEJEPA_TASK_GRAPH)?;
      // clear stale
      for kv in storage.iter_cf(&cf)? {
          let (k, _) = kv?;
          if !k.starts_with(b"_meta_") { storage.delete_cf(&cf, &k)?; }
      }
      // write meta + edges
      storage.put_cf(&cf, b"_meta_corpus_sha", &corpus_sha)?;
      storage.put_cf(&cf, b"_meta_built_at", &(now_unix() as i64).to_be_bytes())?;
      for e in &truncated_edges {
          let key = task_graph_edge_key(&e.source_task_sha, &e.target_task_sha);
          let value = bincode::serialize(e).map_err(|x| GraphError::DimMismatch { expected: 0, actual: 0 })?;
          storage.put_cf(&cf, &key, &value)?;
          // history CF
          let cf_hist = storage.cf_handle(CF_MEJEPA_TASK_GRAPH_HISTORY)?;
          let hist_key = task_graph_history_key(&corpus_sha, &e.source_task_sha, &e.target_task_sha);
          storage.put_cf(&cf_hist, &hist_key, &value)?;
      }

      Ok(PatchGraph { edges: truncated_edges, adjacency: adj, corpus_sha, built_at: now_unix() })
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/task_graph_build.rs">build_patch_similarity_graph + load_cached_graph + EDiffEncoder trait + cosine_similarity + l2_normalize + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">add `EvalRunner::build_patch_similarity_graph(&self, &Corpus) -> Result<PatchGraph, GraphError>` delegating to task_graph_build</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod task_graph_build;` + re-export `EDiffEncoder`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `rayon = { workspace = true }` if not already present (TASK-EVAL-004 likely added)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>3-task synthetic input produces deterministic byte-equal graph across 3 runs</criterion>
  <criterion>top-k truncation caps edges per node at `task_graph_top_k`</criterion>
  <criterion>Idempotent on unchanged corpus_sha (cached load &lt; 100 ms; CF row count unchanged)</criterion>
  <criterion>Rebuild on changed corpus_sha clears stale edges</criterion>
  <criterion>Embedder failure returns Err(EmbedderUnavailable)</criterion>
  <criterion>NaN embedding returns Err(DimMismatch) — fail-closed</criterion>
  <criterion>Cosine clamped to [-1, 1]</criterion>
  <criterion>No self-loops (upper-triangle iteration)</criterion>
  <criterion>CF_MEJEPA_TASK_GRAPH_HISTORY records both corpus_sha versions after rebuild</criterion>
  <criterion>PatchGraph::similar_tasks returns expected top-k after build</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::task_graph_build</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
