---
name: cg-entity
description: |
  Extract and search entities (frameworks, libraries, tools) using E11 KEPLER/TransE embeddings.
  Use for entity extraction, relationship inference, and knowledge validation.
  Keywords: entity, extract, relationship, knowledge, TransE, KEPLER, framework, library.
allowed-tools: mcp__context-graph__extract_entities,mcp__context-graph__search_by_entities,mcp__context-graph__infer_relationship,mcp__context-graph__find_related_entities,mcp__context-graph__validate_knowledge,mcp__context-graph__get_entity_graph
model: inherit
version: 2.0.0
user-invocable: true
---

# Entity Intelligence

Extract and explore entities and their relationships using E11 (V_factuality) KEPLER/TransE embeddings.

Per ARCH-12: E1 is the semantic foundation, E11 enhances with entity facts.
Per ARCH-20: E11 uses entity linking for disambiguation.

## Tools Overview

| Tool | Purpose | Required Params |
|------|---------|-----------------|
| `extract_entities` | Extract and canonicalize entities from text | `text` |
| `search_by_entities` | Find memories containing specific entities | `entities` (array) |
| `infer_relationship` | Infer relationship between two entities | `headEntity`, `tailEntity` |
| `find_related_entities` | Find entities with given relationship | `entity`, `relation` |
| `validate_knowledge` | Score a knowledge triple | `subject`, `predicate`, `object` |
| `get_entity_graph` | Visualize entity relationships | (none required) |

## MCP Tool: extract_entities

Extract and canonicalize entities from text. Resolves variations (e.g., 'postgres' -> 'postgresql', 'k8s' -> 'kubernetes').

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `text` | string | Yes | - | Text to extract entities from |
| `includeUnknown` | boolean | No | true | Include entities not in knowledge base |
| `groupByType` | boolean | No | false | Group results by type (ProgrammingLanguage, Framework, etc.) |

## MCP Tool: search_by_entities

Find memories containing specific entities with entity-aware ranking. Hybrid E11 + Jaccard scoring.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `entities` | array[string] | Yes | - | Entity names to search for (will be canonicalized) |
| `entityTypes` | array[string] | No | - | Filter by type: ProgrammingLanguage, Framework, Database, Cloud, Company, TechnicalTerm, Unknown |
| `matchMode` | string | No | "any" | `any` (match any entity) or `all` (match all) |
| `topK` | integer | No | 10 | Results to return (1-50) |
| `minScore` | number | No | 0.2 | Minimum similarity (0.0-1.0) |
| `includeContent` | boolean | No | false | Include full content text |
| `boostExactMatch` | number | No | 1.15 | Boost for exact entity matches (1.0-3.0) |

## MCP Tool: infer_relationship

Infer relationship between two entities using TransE: r = t - h.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `headEntity` | string | Yes | - | Subject/head entity (e.g., 'Tokio') |
| `tailEntity` | string | Yes | - | Object/tail entity (e.g., 'Rust') |
| `headType` | string | No | - | Type hint for head entity |
| `tailType` | string | No | - | Type hint for tail entity |
| `topK` | integer | No | 5 | Relation candidates to return (1-20) |
| `includeScore` | boolean | No | true | Include raw TransE scores |

## MCP Tool: find_related_entities

Find entities with a given relationship using TransE prediction.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `entity` | string | Yes | - | Source entity |
| `relation` | string | Yes | - | Relationship (e.g., 'depends_on', 'implements', 'created_by') |
| `direction` | string | No | "outgoing" | `outgoing` (h->t) or `incoming` (t<-h) |
| `entityType` | string | No | - | Filter to specific entity type |
| `topK` | integer | No | 10 | Results to return (1-50) |
| `minScore` | number | No | - | Minimum TransE score (-10 to 0) |
| `searchMemories` | boolean | No | true | Filter to entities found in stored memories |

## MCP Tool: validate_knowledge

Score a (subject, predicate, object) triple using TransE: score = -||h + r - t||.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `subject` | string | Yes | - | Subject/head entity |
| `predicate` | string | Yes | - | Relation (e.g., 'created_by', 'depends_on') |
| `object` | string | Yes | - | Object/tail entity |
| `subjectType` | string | No | - | Type hint for subject |
| `objectType` | string | No | - | Type hint for object |

Returns: `valid`, `uncertain`, or `unlikely` with TransE score (0 = perfect match).

## MCP Tool: get_entity_graph

Build and visualize entity relationships discovered in stored memories.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `centerEntity` | string | No | - | Focal entity (if omitted, shows full graph) |
| `maxNodes` | integer | No | 50 | Maximum nodes (1-500) |
| `maxDepth` | integer | No | 2 | Max hops from center (1-5) |
| `entityTypes` | array[string] | No | - | Filter by entity types |
| `minRelationScore` | number | No | 0.3 | Minimum edge score (0.0-1.0) |
| `includeMemoryCounts` | boolean | No | true | Include memory reference counts per node |

## Example Usage

| User Request | Tool | Parameters |
|--------------|------|------------|
| "Extract entities from this text" | `extract_entities` | `{"text": "We use Rust with Tokio for async"}` |
| "Find memories about PostgreSQL" | `search_by_entities` | `{"entities": ["PostgreSQL"], "includeContent": true}` |
| "What's the relationship between Tokio and Rust?" | `infer_relationship` | `{"headEntity": "Tokio", "tailEntity": "Rust"}` |
| "What depends on PostgreSQL?" | `find_related_entities` | `{"entity": "PostgreSQL", "relation": "depends_on", "direction": "incoming"}` |
| "Is Tokio a Rust framework?" | `validate_knowledge` | `{"subject": "Tokio", "predicate": "is_framework_for", "object": "Rust"}` |
| "Show entity graph around Rust" | `get_entity_graph` | `{"centerEntity": "Rust", "maxDepth": 3}` |

## Edge Cases

| Condition | Response |
|-----------|----------|
| Unknown entity | Included if includeUnknown=true, detected via heuristics |
| No memories contain entity | "No memories found containing the specified entities." |
| Invalid entity type | Error listing valid types |
| Empty text for extraction | Error: "text parameter is required" |
