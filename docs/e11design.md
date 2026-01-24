# E11 Entity Embedder Integration Design

## Executive Summary

This document outlines the full integration of E11 Entity embedder capabilities into the context-graph MCP server tools. E11 provides **entity-aware intelligence** that enhances E1's semantic foundation with named entity understanding, relationship inference, and knowledge graph operations.

**Current State**: E11 has powerful TransE and entity linking capabilities that are **completely unused** in MCP tools.

**Goal**: Expose E11's unique intelligence through dedicated MCP tools that enable entity-based memory retrieval, relationship inference, and knowledge validation.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [E11 Capabilities Inventory](#2-e11-capabilities-inventory)
3. [Proposed MCP Tools](#3-proposed-mcp-tools)
4. [Integration Architecture](#4-integration-architecture)
5. [Data Structures](#5-data-structures)
6. [Implementation Phases](#6-implementation-phases)
7. [API Contracts](#7-api-contracts)
8. [Testing Strategy](#8-testing-strategy)
9. [Constitution Compliance](#9-constitution-compliance)
10. [Performance Considerations](#10-performance-considerations)

---

## 1. Architecture Overview

### 1.1 E11's Role in the Fingerprint System

Per CLAUDE.md constitution:

```yaml
E11_Entity:
  purpose: V_factuality
  role: RELATIONAL_ENHANCER
  category: Enhances E1 with entity facts
  topic_weight: 0.5
  dimension: 384D
  model: sentence-transformers/all-MiniLM-L6-v2
```

E11 is a **RELATIONAL_ENHANCER** that adds entity relationship understanding on top of E1's semantic foundation. It does NOT compete with E1 - it augments semantic retrieval with entity-aware precision.

### 1.2 Current Integration Gap

| Embedder | Dedicated MCP Tools | Status |
|----------|---------------------|--------|
| E1 Semantic | `search_graph`, `inject_context` | ✅ Foundation |
| E5 Causal | `search_causes`, `get_causal_chain` | ✅ Integrated |
| E8 Graph | `search_connections`, `get_graph_path` | ✅ Integrated |
| E10 Intent | `search_by_intent`, `find_contextual_matches` | ✅ Integrated |
| E4 Sequence | `get_conversation_context`, `traverse_memory_chain` | ✅ Integrated |
| **E11 Entity** | **None** | ❌ **Gap** |

### 1.3 Integration Philosophy

Following ARCH-12 (E1 is foundation) and ARCH-16 (enhancers refine):

```
Query Flow:
  User Query
      │
      ▼
  E1 Semantic Retrieval (baseline candidates)
      │
      ▼
  E11 Entity Enhancement (entity-aware reranking)
      │
      ▼
  Final Results
```

E11 tools should:
1. **Enhance** E1 results, not bypass them
2. Provide **entity-specific** intelligence unavailable in E1
3. Enable **TransE operations** for relationship reasoning

---

## 2. E11 Capabilities Inventory

### 2.1 Entity Embedding Model

**Location**: `crates/context-graph-embeddings/src/models/pretrained/entity/`

| Component | File | Purpose |
|-----------|------|---------|
| Model | `model.rs` | all-MiniLM-L6-v2 BERT, 384D output |
| TransE | `transe.rs` | Knowledge graph operations |
| Encoding | `encoding.rs` | Entity/relation text formatting |
| Forward | `forward.rs` | GPU-accelerated inference |

**Key Methods**:
```rust
// Entity encoding
EntityModel::encode_entity("Alice", Some("PERSON")) → "[PERSON] Alice"
EntityModel::encode_relation("works_at") → "works at"

// TransE operations
EntityModel::transe_score(head, relation, tail) → f32  // -||h + r - t||₂
EntityModel::predict_tail(head, relation) → Vec<f32>   // t̂ = h + r
EntityModel::predict_relation(head, tail) → Vec<f32>   // r̂ = t - h
```

### 2.2 Entity Detection & Linking

**Location**: `crates/context-graph-core/src/entity/mod.rs`

| Component | Purpose |
|-----------|---------|
| `detect_entities(text)` | Extract entities from text |
| `get_entity_kb()` | Knowledge base with 100+ entity mappings |
| `entity_jaccard_similarity()` | Entity set overlap scoring |
| `compute_e11_similarity_with_entities()` | Hybrid embedding + entity scoring |

**Entity Types**:
```rust
pub enum EntityType {
    ProgrammingLanguage,  // Rust, Python, JavaScript
    Framework,            // React, Django, Tokio
    Database,             // PostgreSQL, MongoDB, Redis
    Cloud,                // AWS, GCP, Kubernetes
    Company,              // Anthropic, OpenAI, Google
    TechnicalTerm,        // REST, GraphQL, gRPC
    Unknown,              // Detected but not in KB
}
```

**Disambiguation Examples**:
```
"postgres" → "postgresql"
"k8s" → "kubernetes"
"py" → "python"
"rustlang" → "rust_language"
```

### 2.3 TransE Knowledge Graph Model

TransE models relationships as translations in embedding space:

```
For valid triple (h, r, t): h + r ≈ t

Score = -||h + r - t||₂
  - 0 = perfect match
  - More negative = worse match
```

**Operations**:

| Operation | Formula | Use Case |
|-----------|---------|----------|
| Score Triple | `-\|\|h + r - t\|\|₂` | Validate "Alice works_at Anthropic" |
| Predict Tail | `t̂ = h + r` | "Who works_at Anthropic?" |
| Predict Relation | `r̂ = t - h` | "What's the relation between Alice and Anthropic?" |

---

## 3. Proposed MCP Tools

### 3.1 Tool Overview

| Tool | Purpose | Primary E11 Feature |
|------|---------|---------------------|
| `search_by_entities` | Find memories containing specific entities | Entity detection + E11 similarity |
| `extract_entities` | Get entities from text with canonical links | `detect_entities()` |
| `infer_relationship` | Infer relationship between two entities | `predict_relation()` |
| `find_related_entities` | Find entities with given relationship | `predict_tail()` |
| `validate_knowledge` | Score a (subject, predicate, object) triple | `transe_score()` |
| `get_entity_graph` | Visualize entity relationships in memory | TransE + graph traversal |

### 3.2 Tool: `search_by_entities`

**Purpose**: Find memories that mention specific entities, with entity-aware ranking.

**Use Cases**:
- "What have I stored about PostgreSQL?"
- "Find memories mentioning React or Vue"
- "Show me everything about Anthropic"

**Parameters**:
```typescript
interface SearchByEntitiesRequest {
  entities: string[];              // Entity names to search for
  entityTypes?: EntityType[];      // Filter by entity type
  matchMode: "any" | "all";        // Any entity or all entities
  topK?: number;                   // Max results (default: 10)
  minScore?: number;               // Min similarity (default: 0.2)
  includeContent?: boolean;        // Include full text (default: false)
  boostExactMatch?: number;        // Boost for exact entity match (default: 1.3)
}
```

**Algorithm**:
1. Detect entities in query, resolve to canonical IDs
2. Search E1 for semantic candidates (5x over-fetch)
3. Extract entities from each candidate memory
4. Compute hybrid score: `0.7 * E11_cosine + 0.3 * entity_jaccard`
5. Boost memories with exact entity matches
6. Return top-K ranked results

**Response**:
```typescript
interface SearchByEntitiesResponse {
  results: EntitySearchResult[];
  detected_query_entities: EntityLink[];
  total_candidates: number;
  search_time_ms: number;
}

interface EntitySearchResult {
  memory_id: string;
  score: number;
  e11_similarity: number;
  entity_overlap: number;
  matched_entities: EntityLink[];
  content?: string;
}
```

### 3.3 Tool: `extract_entities`

**Purpose**: Extract and canonicalize entities from text.

**Use Cases**:
- "What entities are in this memory?"
- Pre-processing for entity-aware storage
- Building entity indexes

**Parameters**:
```typescript
interface ExtractEntitiesRequest {
  text: string;                    // Text to extract from
  includeUnknown?: boolean;        // Include non-KB entities (default: true)
  groupByType?: boolean;           // Group results by entity type
}
```

**Response**:
```typescript
interface ExtractEntitiesResponse {
  entities: EntityLink[];
  by_type?: Record<EntityType, EntityLink[]>;
  total_count: number;
}

interface EntityLink {
  surface_form: string;      // As found in text
  canonical_id: string;      // Normalized ID
  entity_type: EntityType;
}
```

### 3.4 Tool: `infer_relationship`

**Purpose**: Infer the relationship between two entities using TransE.

**Use Cases**:
- "What's the relationship between this function and that module?"
- "How are these two concepts related?"
- Building knowledge graphs from memories

**Parameters**:
```typescript
interface InferRelationshipRequest {
  head_entity: string;             // Subject entity
  tail_entity: string;             // Object entity
  head_type?: EntityType;          // Optional type hint
  tail_type?: EntityType;          // Optional type hint
  topK?: number;                   // Top-K relation candidates
  includeScore?: boolean;          // Include TransE scores
}
```

**Algorithm**:
1. Embed head entity with E11: `h = E11("[TYPE] head_entity")`
2. Embed tail entity with E11: `t = E11("[TYPE] tail_entity")`
3. Compute predicted relation: `r̂ = t - h`
4. Search for known relations closest to r̂ in embedding space
5. Score candidates with TransE: `score = -||h + r - t||₂`
6. Return ranked relation candidates

**Response**:
```typescript
interface InferRelationshipResponse {
  head: EntityLink;
  tail: EntityLink;
  inferred_relations: RelationCandidate[];
  predicted_vector?: number[];  // Raw r̂ vector if requested
}

interface RelationCandidate {
  relation: string;           // e.g., "works_at", "depends_on"
  score: number;              // TransE score (higher = better)
  confidence: number;         // Normalized [0, 1]
}
```

**Known Relations** (to be expanded):
```rust
const KNOWN_RELATIONS: &[&str] = &[
    "works_at", "created_by", "depends_on", "imports",
    "extends", "implements", "uses", "configures",
    "located_in", "part_of", "version_of", "alternative_to",
];
```

### 3.5 Tool: `find_related_entities`

**Purpose**: Find entities that have a given relationship to a source entity.

**Use Cases**:
- "What other databases are similar to PostgreSQL?"
- "Find frameworks that 'depend_on' React"
- "Who else 'works_at' this company?"

**Parameters**:
```typescript
interface FindRelatedEntitiesRequest {
  entity: string;                  // Source entity
  relation: string;                // Relationship to find
  direction: "outgoing" | "incoming";  // h→t or t←h
  entityType?: EntityType;         // Filter result types
  topK?: number;                   // Max results (default: 10)
  minScore?: number;               // Min TransE score
  searchMemories?: boolean;        // Search stored memories (default: true)
}
```

**Algorithm**:
1. Embed source entity: `h = E11(entity)`
2. Embed relation: `r = E11(relation)`
3. Predict target: `t̂ = h + r` (outgoing) or `ĥ = t - r` (incoming)
4. Search E11 space for entities closest to prediction
5. Optionally filter to entities found in stored memories
6. Score with TransE and return ranked results

**Response**:
```typescript
interface FindRelatedEntitiesResponse {
  source_entity: EntityLink;
  relation: string;
  direction: string;
  related_entities: RelatedEntity[];
  search_time_ms: number;
}

interface RelatedEntity {
  entity: EntityLink;
  transe_score: number;
  found_in_memories: boolean;
  memory_ids?: string[];      // If found in stored memories
}
```

### 3.6 Tool: `validate_knowledge`

**Purpose**: Score whether a (subject, predicate, object) triple is valid.

**Use Cases**:
- Fact-checking during memory injection
- Knowledge consistency validation
- Detecting contradictions

**Parameters**:
```typescript
interface ValidateKnowledgeRequest {
  subject: string;                 // Head entity
  predicate: string;               // Relation
  object: string;                  // Tail entity
  subject_type?: EntityType;
  object_type?: EntityType;
}
```

**Algorithm**:
1. Embed all three: `h = E11(subject)`, `r = E11(predicate)`, `t = E11(object)`
2. Compute TransE score: `score = -||h + r - t||₂`
3. Normalize to [0, 1] confidence
4. Compare against stored knowledge if available

**Response**:
```typescript
interface ValidateKnowledgeResponse {
  triple: {
    subject: EntityLink;
    predicate: string;
    object: EntityLink;
  };
  transe_score: number;           // Raw score (0 = perfect)
  confidence: number;             // Normalized [0, 1]
  validation: "valid" | "uncertain" | "unlikely";
  supporting_memories?: string[]; // Memories that support this
  contradicting_memories?: string[]; // Memories that contradict
}
```

**Confidence Thresholds**:
```rust
const VALID_THRESHOLD: f32 = -0.5;      // score > -0.5 → valid
const UNCERTAIN_THRESHOLD: f32 = -1.5;  // -1.5 < score < -0.5 → uncertain
// score < -1.5 → unlikely
```

### 3.7 Tool: `get_entity_graph`

**Purpose**: Visualize entity relationships discovered in memory.

**Use Cases**:
- "Show me the entity graph for this project"
- "What entities are connected to PostgreSQL?"
- Knowledge graph visualization

**Parameters**:
```typescript
interface GetEntityGraphRequest {
  center_entity?: string;          // Optional focal entity
  maxNodes?: number;               // Max nodes (default: 50)
  maxDepth?: number;               // Max hops from center (default: 2)
  entityTypes?: EntityType[];      // Filter node types
  minRelationScore?: number;       // Min edge score (default: 0.3)
  includeMemoryCounts?: boolean;   // Include memory reference counts
}
```

**Response**:
```typescript
interface GetEntityGraphResponse {
  nodes: EntityNode[];
  edges: EntityEdge[];
  center_entity?: EntityLink;
  total_memories_scanned: number;
}

interface EntityNode {
  id: string;                     // Canonical entity ID
  label: string;                  // Display name
  entity_type: EntityType;
  memory_count: number;           // How many memories mention this
  importance: number;             // Based on frequency + centrality
}

interface EntityEdge {
  source: string;                 // Source entity ID
  target: string;                 // Target entity ID
  relation: string;               // Inferred relationship
  weight: number;                 // TransE score
  memory_ids: string[];           // Memories supporting this edge
}
```

---

## 4. Integration Architecture

### 4.1 Module Structure

```
crates/context-graph-mcp/src/handlers/tools/
├── entity_tools.rs          # NEW: Tool implementations
├── entity_dtos.rs           # NEW: Request/Response DTOs
├── mod.rs                   # Add entity_tools module
└── dispatch.rs              # Add tool dispatch entries

crates/context-graph-core/src/entity/
├── mod.rs                   # Existing: Entity detection
├── linking.rs               # NEW: Enhanced entity linking
├── relations.rs             # NEW: Relation knowledge base
└── graph.rs                 # NEW: Entity graph construction
```

### 4.2 Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                      MCP Tool Request                        │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    entity_tools.rs                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │search_by_    │  │infer_        │  │validate_     │       │
│  │entities      │  │relationship  │  │knowledge     │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
└─────────┼─────────────────┼─────────────────┼───────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│              context-graph-core/entity/                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │detect_       │  │predict_      │  │transe_       │       │
│  │entities()    │  │relation()    │  │score()       │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
└─────────┼─────────────────┼─────────────────┼───────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│           context-graph-embeddings/entity/                   │
│  ┌──────────────────────────────────────────────────┐       │
│  │              EntityModel (E11)                    │       │
│  │  - embed() → 384D                                │       │
│  │  - transe_score(h, r, t)                         │       │
│  │  - predict_tail(h, r)                            │       │
│  │  - predict_relation(h, t)                        │       │
│  └──────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

### 4.3 Storage Integration

Entity metadata should be stored alongside fingerprints:

```sql
-- Add to memory storage schema
ALTER TABLE memories ADD COLUMN entity_metadata JSONB;

-- Entity metadata structure
{
  "entities": [
    {"surface_form": "PostgreSQL", "canonical_id": "postgresql", "type": "Database"},
    {"surface_form": "Rust", "canonical_id": "rust_language", "type": "ProgrammingLanguage"}
  ],
  "extracted_at": "2024-01-24T12:00:00Z"
}
```

### 4.4 Index Requirements

For efficient entity search:

```rust
// E11-specific HNSW index for entity embeddings
struct E11EntityIndex {
    hnsw: HnswIndex<384>,           // E11 dimension
    entity_to_memories: HashMap<String, Vec<Uuid>>,  // canonical_id → memory IDs
    memory_entities: HashMap<Uuid, Vec<EntityLink>>, // memory → entities
}
```

---

## 5. Data Structures

### 5.1 Core Types

```rust
// crates/context-graph-core/src/entity/types.rs

/// Detected entity with canonical linking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityLink {
    pub surface_form: String,
    pub canonical_id: String,
    pub entity_type: EntityType,
    pub confidence: f32,
}

/// Entity type categories
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntityType {
    ProgrammingLanguage,
    Framework,
    Database,
    Cloud,
    Company,
    TechnicalTerm,
    Person,
    Location,
    Unknown,
}

/// A knowledge triple (subject, predicate, object)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeTriple {
    pub subject: EntityLink,
    pub predicate: String,
    pub object: EntityLink,
    pub transe_score: f32,
    pub source_memories: Vec<Uuid>,
}

/// Entity relationship for graph construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRelation {
    pub from_entity: String,  // canonical_id
    pub to_entity: String,    // canonical_id
    pub relation: String,
    pub score: f32,
    pub evidence_count: usize,
}
```

### 5.2 Relation Knowledge Base

```rust
// crates/context-graph-core/src/entity/relations.rs

/// Known relation types with embeddings
pub struct RelationKnowledgeBase {
    relations: HashMap<String, RelationInfo>,
    embeddings: HashMap<String, Vec<f32>>,  // Pre-computed E11 embeddings
}

pub struct RelationInfo {
    pub name: String,
    pub inverse: Option<String>,    // "created_by" ↔ "creator_of"
    pub domain: Vec<EntityType>,    // Valid subject types
    pub range: Vec<EntityType>,     // Valid object types
    pub symmetric: bool,
}

impl RelationKnowledgeBase {
    pub fn default_relations() -> Self {
        let mut kb = Self::new();

        // Technical relations
        kb.add("depends_on", Some("dependency_of"), false);
        kb.add("imports", Some("imported_by"), false);
        kb.add("extends", Some("extended_by"), false);
        kb.add("implements", Some("implemented_by"), false);
        kb.add("uses", Some("used_by"), false);
        kb.add("configures", Some("configured_by"), false);

        // Organizational relations
        kb.add("works_at", Some("employs"), false);
        kb.add("created_by", Some("creator_of"), false);
        kb.add("maintained_by", Some("maintains"), false);

        // Categorical relations
        kb.add("part_of", Some("contains"), false);
        kb.add("version_of", None, false);
        kb.add("alternative_to", None, true);  // Symmetric
        kb.add("similar_to", None, true);      // Symmetric

        kb
    }
}
```

---

## 6. Implementation Phases

### Phase 1: Foundation (Week 1)

**Goal**: Core infrastructure and basic entity search.

**Tasks**:
1. Create `entity_dtos.rs` with all request/response types
2. Create `entity_tools.rs` module structure
3. Implement `extract_entities` tool (exposes existing capability)
4. Add entity metadata storage to memory injection
5. Add tool dispatch entries

**Files**:
- `crates/context-graph-mcp/src/handlers/tools/entity_dtos.rs` (NEW)
- `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` (NEW)
- `crates/context-graph-mcp/src/handlers/tools/mod.rs` (MODIFY)
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (MODIFY)

**Deliverables**:
- [ ] `extract_entities` tool working
- [ ] Entity metadata stored with memories
- [ ] Unit tests passing

### Phase 2: Entity Search (Week 2)

**Goal**: Entity-aware memory retrieval.

**Tasks**:
1. Implement `search_by_entities` tool
2. Add entity Jaccard scoring to retrieval
3. Implement entity index for fast lookup
4. Add entity-based result boosting

**Files**:
- `crates/context-graph-core/src/entity/index.rs` (NEW)
- `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` (MODIFY)

**Deliverables**:
- [ ] `search_by_entities` tool working
- [ ] Entity index populated during injection
- [ ] Benchmark: <100ms for 10K memories

### Phase 3: TransE Operations (Week 3)

**Goal**: Relationship inference and validation.

**Tasks**:
1. Create relation knowledge base
2. Pre-compute relation embeddings
3. Implement `infer_relationship` tool
4. Implement `validate_knowledge` tool
5. Implement `find_related_entities` tool

**Files**:
- `crates/context-graph-core/src/entity/relations.rs` (NEW)
- `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` (MODIFY)

**Deliverables**:
- [ ] `infer_relationship` returning ranked relations
- [ ] `validate_knowledge` scoring triples
- [ ] `find_related_entities` with memory search
- [ ] Relation KB with 20+ relations

### Phase 4: Entity Graph (Week 4)

**Goal**: Graph visualization and advanced features.

**Tasks**:
1. Implement `get_entity_graph` tool
2. Build entity graph from memories
3. Add graph traversal with TransE scoring
4. Integration testing with full pipeline

**Files**:
- `crates/context-graph-core/src/entity/graph.rs` (NEW)
- `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` (MODIFY)

**Deliverables**:
- [ ] `get_entity_graph` returning nodes/edges
- [ ] Graph construction from memories
- [ ] End-to-end integration tests
- [ ] Performance benchmarks

### Phase 5: Enhancement Integration (Week 5)

**Goal**: Integrate E11 into existing tools.

**Tasks**:
1. Add entity boost to `search_graph`
2. Entity-aware `inject_context`
3. Entity metadata in `get_memetic_status`
4. Documentation and examples

**Deliverables**:
- [ ] `search_graph` with optional entity boosting
- [ ] Entity extraction during `inject_context`
- [ ] Updated CLAUDE.md with E11 tools
- [ ] Usage examples in docs/

---

## 7. API Contracts

### 7.1 MCP Tool Registration

```json
{
  "name": "search_by_entities",
  "description": "Find memories containing specific entities with entity-aware ranking. Uses E11 entity embeddings combined with entity Jaccard similarity for precise entity matching.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entities": {
        "type": "array",
        "items": { "type": "string" },
        "description": "Entity names to search for (e.g., ['PostgreSQL', 'Rust'])"
      },
      "entityTypes": {
        "type": "array",
        "items": { "type": "string", "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"] },
        "description": "Filter by entity types"
      },
      "matchMode": {
        "type": "string",
        "enum": ["any", "all"],
        "default": "any",
        "description": "Match any entity or all entities"
      },
      "topK": {
        "type": "integer",
        "default": 10,
        "minimum": 1,
        "maximum": 50,
        "description": "Maximum results to return"
      },
      "minScore": {
        "type": "number",
        "default": 0.2,
        "minimum": 0,
        "maximum": 1,
        "description": "Minimum similarity threshold"
      },
      "includeContent": {
        "type": "boolean",
        "default": false,
        "description": "Include full memory content"
      }
    },
    "required": ["entities"]
  }
}
```

### 7.2 Example Requests/Responses

**search_by_entities**:
```json
// Request
{
  "entities": ["PostgreSQL", "Rust"],
  "matchMode": "any",
  "topK": 5
}

// Response
{
  "results": [
    {
      "memory_id": "abc123",
      "score": 0.89,
      "e11_similarity": 0.85,
      "entity_overlap": 1.0,
      "matched_entities": [
        {"surface_form": "PostgreSQL", "canonical_id": "postgresql", "entity_type": "Database"}
      ]
    }
  ],
  "detected_query_entities": [
    {"surface_form": "PostgreSQL", "canonical_id": "postgresql", "entity_type": "Database"},
    {"surface_form": "Rust", "canonical_id": "rust_language", "entity_type": "ProgrammingLanguage"}
  ],
  "total_candidates": 47,
  "search_time_ms": 23
}
```

**infer_relationship**:
```json
// Request
{
  "head_entity": "Tokio",
  "tail_entity": "Rust",
  "topK": 3
}

// Response
{
  "head": {"surface_form": "Tokio", "canonical_id": "tokio", "entity_type": "Framework"},
  "tail": {"surface_form": "Rust", "canonical_id": "rust_language", "entity_type": "ProgrammingLanguage"},
  "inferred_relations": [
    {"relation": "implemented_in", "score": -0.23, "confidence": 0.91},
    {"relation": "depends_on", "score": -0.45, "confidence": 0.78},
    {"relation": "part_of", "score": -0.67, "confidence": 0.62}
  ]
}
```

**validate_knowledge**:
```json
// Request
{
  "subject": "Claude",
  "predicate": "created_by",
  "object": "Anthropic"
}

// Response
{
  "triple": {
    "subject": {"surface_form": "Claude", "canonical_id": "anthropic_claude", "entity_type": "Company"},
    "predicate": "created_by",
    "object": {"surface_form": "Anthropic", "canonical_id": "anthropic", "entity_type": "Company"}
  },
  "transe_score": -0.12,
  "confidence": 0.94,
  "validation": "valid",
  "supporting_memories": ["mem_xyz"]
}
```

---

## 8. Testing Strategy

### 8.1 Unit Tests

```rust
// crates/context-graph-mcp/src/handlers/tools/entity_tools.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_detection_programming_languages() {
        let text = "Building a web server with Rust and PostgreSQL";
        let entities = extract_entities(text);

        assert!(entities.contains_canonical("rust_language"));
        assert!(entities.contains_canonical("postgresql"));
    }

    #[test]
    fn test_transe_score_valid_triple() {
        // "Tokio implemented_in Rust" should score well
        let h = embed_entity("Tokio", EntityType::Framework);
        let r = embed_relation("implemented_in");
        let t = embed_entity("Rust", EntityType::ProgrammingLanguage);

        let score = transe_score(&h, &r, &t);
        assert!(score > -0.5, "Valid triple should have good score");
    }

    #[test]
    fn test_transe_score_invalid_triple() {
        // "PostgreSQL works_at Google" should score poorly
        let h = embed_entity("PostgreSQL", EntityType::Database);
        let r = embed_relation("works_at");
        let t = embed_entity("Google", EntityType::Company);

        let score = transe_score(&h, &r, &t);
        assert!(score < -1.5, "Invalid triple should have poor score");
    }

    #[test]
    fn test_predict_relation_accuracy() {
        // Given "React" and "JavaScript", should infer "implemented_in" or "depends_on"
        let head = embed_entity("React", EntityType::Framework);
        let tail = embed_entity("JavaScript", EntityType::ProgrammingLanguage);

        let relations = infer_relations(&head, &tail, 3);
        let relation_names: Vec<_> = relations.iter().map(|r| &r.relation).collect();

        assert!(relation_names.contains(&"implemented_in") || relation_names.contains(&"depends_on"));
    }
}
```

### 8.2 Integration Tests

```rust
// tests/integration/entity_tools_test.rs

#[tokio::test]
async fn test_search_by_entities_end_to_end() {
    let server = setup_test_server().await;

    // Inject test memories
    server.inject("Working with PostgreSQL and Rust for the backend").await;
    server.inject("Frontend uses React and TypeScript").await;
    server.inject("Database migration from MySQL to PostgreSQL").await;

    // Search for PostgreSQL
    let response = server.call("search_by_entities", json!({
        "entities": ["PostgreSQL"],
        "topK": 10
    })).await;

    assert_eq!(response.results.len(), 2);  // Should find 2 memories
    assert!(response.results[0].matched_entities.iter().any(|e| e.canonical_id == "postgresql"));
}

#[tokio::test]
async fn test_entity_graph_construction() {
    let server = setup_test_server().await;

    // Inject memories with entity relationships
    server.inject("Tokio is a Rust async runtime").await;
    server.inject("Axum web framework built on Tokio").await;
    server.inject("PostgreSQL driver for Rust using Tokio").await;

    let graph = server.call("get_entity_graph", json!({
        "center_entity": "Tokio",
        "maxDepth": 2
    })).await;

    // Should have Tokio, Rust, Axum, PostgreSQL nodes
    assert!(graph.nodes.len() >= 4);
    // Should have edges connecting them
    assert!(graph.edges.len() >= 3);
}
```

### 8.3 Benchmark Tests

```rust
// benches/entity_benchmark.rs

#[bench]
fn bench_entity_search_10k_memories(b: &mut Bencher) {
    let index = setup_10k_memory_index();
    let query_entities = vec!["PostgreSQL", "Rust"];

    b.iter(|| {
        search_by_entities(&index, &query_entities, 10)
    });
}

#[bench]
fn bench_transe_inference(b: &mut Bencher) {
    let h = random_entity_embedding();
    let t = random_entity_embedding();

    b.iter(|| {
        infer_relations(&h, &t, 5)
    });
}
```

---

## 9. Constitution Compliance

### 9.1 Architecture Rules

| Rule | Compliance |
|------|------------|
| ARCH-12: E1 is foundation | ✅ Entity search starts with E1, E11 enhances |
| ARCH-16: Enhancers refine | ✅ E11 reranks E1 results with entity precision |
| ARCH-20: E11 entity linking | ✅ Full entity detection and canonical linking |
| AP-02: No cross-embedder comparison | ✅ E11 only compared to E11 |

### 9.2 Weight Profile

E11 is a RELATIONAL_ENHANCER with weight 0.5 in topic detection:

```yaml
RELATIONAL_ENHANCERS:
  embedders: [E8, E11]
  topic_weight: 0.5
  role: "Enhance E1 with relationship context"
```

### 9.3 Delta_S Method

Per constitution, E11 uses TransE scoring for Delta_S:

```yaml
delta_methods:
  Delta_S:
    E11: "TransE ||h+r-t||"
```

---

## 10. Performance Considerations

### 10.1 Latency Budgets

| Operation | Target | Notes |
|-----------|--------|-------|
| Entity detection | <10ms | Pattern matching, no GPU |
| E11 embedding | <5ms | GPU warm model |
| Entity search (10K) | <100ms | HNSW + entity index |
| TransE scoring | <1ms | Vector arithmetic |
| Relation inference | <50ms | Top-K search |

### 10.2 Memory Requirements

| Component | Size | Notes |
|-----------|------|-------|
| E11 model | ~80MB | Already loaded in warm provider |
| Entity KB | ~100KB | Static hashmap |
| Relation embeddings | ~50KB | 50 relations × 384D × 4B |
| Entity index | ~4MB/10K | Entity → memory mappings |

### 10.3 Optimization Strategies

1. **Lazy Entity Extraction**: Only extract on injection, cache in metadata
2. **Relation Embedding Cache**: Pre-compute all known relation embeddings
3. **Entity Index**: Inverted index from canonical_id → memory UUIDs
4. **Batch TransE**: Vectorized scoring for multiple candidates

---

## Appendix A: Entity Knowledge Base Expansion

Future entity categories to add:

```rust
// ML/AI
("bert", "bert_model", TechnicalTerm),
("gpt", "gpt_model", TechnicalTerm),
("transformer", "transformer_architecture", TechnicalTerm),
("embedding", "embedding_vector", TechnicalTerm),

// DevOps
("jenkins", "jenkins_ci", Cloud),
("terraform", "terraform_iac", Cloud),
("ansible", "ansible_automation", Cloud),

// Data Science
("pandas", "pandas_library", Framework),
("numpy", "numpy_library", Framework),
("pytorch", "pytorch_framework", Framework),
("tensorflow", "tensorflow_framework", Framework),
```

---

## Appendix B: Relation Ontology

Complete relation taxonomy:

```
Technical Relations:
├── depends_on / dependency_of
├── imports / imported_by
├── extends / extended_by
├── implements / implemented_by
├── uses / used_by
├── configures / configured_by
├── calls / called_by
└── wraps / wrapped_by

Organizational Relations:
├── works_at / employs
├── created_by / creator_of
├── maintained_by / maintains
├── owned_by / owns
└── sponsored_by / sponsors

Categorical Relations:
├── part_of / contains
├── instance_of / has_instance
├── version_of
├── fork_of / forked_to
├── alternative_to (symmetric)
├── similar_to (symmetric)
└── compatible_with (symmetric)

Temporal Relations:
├── preceded_by / precedes
├── replaced_by / replaces
└── evolved_from / evolved_to
```

---

## Appendix C: Migration Plan

For existing memories without entity metadata:

```rust
// Backfill entity metadata for existing memories
async fn migrate_entity_metadata(store: &MemoryStore) -> Result<()> {
    let memories = store.list_all().await?;

    for memory in memories {
        if memory.entity_metadata.is_none() {
            let entities = detect_entities(&memory.content);
            store.update_entity_metadata(memory.id, entities).await?;
        }
    }

    Ok(())
}
```

---

*Document Version: 1.0*
*Last Updated: 2024-01-24*
*Author: Claude Code*
