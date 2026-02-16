//! E11 Entity tool definitions.
//!
//! Per E11 Design Document, these tools expose E11's unique capabilities:
//! - extract_entities: Extract and canonicalize entities from text
//! - search_by_entities: Find memories containing specific entities
//! - infer_relationship: Infer relationship between two entities using TransE
//! - find_related_entities: Find entities with given relationship
//! - validate_knowledge: Score a (subject, predicate, object) triple
//! - get_entity_graph: Visualize entity relationships in memory
//!
//! Constitution Compliance:
//! - ARCH-12: E1 is the semantic foundation, E11 enhances with entity facts
//! - ARCH-20: E11 SHOULD use entity linking for disambiguation
//! - E11 is RELATIONAL_ENHANCER with topic_weight 0.5
//! - Delta_S method: TransE ||h+r-t||

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns E11 entity tool definitions (6 tools).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // extract_entities - Extract and canonicalize entities from text (Phase 1)
        ToolDefinition::new(
            "extract_entities",
            "Extract and canonicalize entities from text using pattern matching and knowledge base lookup. \
             Resolves variations to canonical forms (e.g., 'postgres' → 'postgresql', 'k8s' → 'kubernetes'). \
             Detects programming languages, frameworks, databases, cloud services, companies, and technical terms. \
             Per ARCH-20: E11 uses entity linking for disambiguation.",
            json!({
                "type": "object",
                "required": ["text"],
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to extract entities from."
                    },
                    "includeUnknown": {
                        "type": "boolean",
                        "description": "Include entities not in knowledge base (detected via heuristics). Default: true.",
                        "default": true
                    },
                    "groupByType": {
                        "type": "boolean",
                        "description": "Group results by entity type (ProgrammingLanguage, Framework, Database, etc). Default: false.",
                        "default": false
                    }
                },
                "additionalProperties": false
            }),
        ),
        // search_by_entities - Find memories containing specific entities (Phase 2)
        ToolDefinition::new(
            "search_by_entities",
            "Find memories containing specific entities with entity-aware ranking. \
             Uses E11 entity embeddings combined with entity Jaccard similarity for hybrid scoring. \
             ENHANCES E1 semantic search with entity precision (ARCH-12). \
             Supports 'any' (match any entity) or 'all' (match all entities) modes.",
            json!({
                "type": "object",
                "required": ["entities"],
                "properties": {
                    "entities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Entity names to search for (e.g., ['PostgreSQL', 'Rust']). Will be canonicalized."
                    },
                    "entityTypes": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm", "Unknown"]
                        },
                        "description": "Filter by entity types."
                    },
                    "matchMode": {
                        "type": "string",
                        "enum": ["any", "all"],
                        "default": "any",
                        "description": "Match any entity or all entities."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum results to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum similarity threshold (0-1, default: 0.2).",
                        "default": 0.2,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeContent": {
                        "type": "boolean",
                        "description": "Include full memory content in results. Default: false.",
                        "default": false
                    },
                    "boostExactMatch": {
                        "type": "number",
                        "description": "Boost multiplier for exact entity matches (1.0-3.0, default: 1.15).",
                        "default": 1.15,
                        "minimum": 1.0,
                        "maximum": 3.0
                    }
                },
                "additionalProperties": false
            }),
        ),
        // infer_relationship - Infer relationship between entities using TransE (Phase 3)
        ToolDefinition::new(
            "infer_relationship",
            "Infer the relationship between two entities using TransE knowledge graph operations. \
             Uses the formula r̂ = t - h to predict the relation vector, then matches against known relations. \
             Returns ranked relation candidates with TransE scores and confidence values. \
             Per constitution: Delta_S method for E11 is 'TransE ||h+r-t||'.",
            json!({
                "type": "object",
                "required": ["headEntity", "tailEntity"],
                "properties": {
                    "headEntity": {
                        "type": "string",
                        "description": "Subject/head entity (e.g., 'Tokio')."
                    },
                    "tailEntity": {
                        "type": "string",
                        "description": "Object/tail entity (e.g., 'Rust')."
                    },
                    "headType": {
                        "type": "string",
                        "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"],
                        "description": "Optional type hint for head entity."
                    },
                    "tailType": {
                        "type": "string",
                        "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"],
                        "description": "Optional type hint for tail entity."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Number of relation candidates to return (1-20, default: 5).",
                        "default": 5,
                        "minimum": 1,
                        "maximum": 20
                    },
                    "includeScore": {
                        "type": "boolean",
                        "description": "Include raw TransE scores in response. Default: true.",
                        "default": true
                    }
                },
                "additionalProperties": false
            }),
        ),
        // find_related_entities - Find entities with given relationship (Phase 3)
        ToolDefinition::new(
            "find_related_entities",
            "Find entities that have a given relationship to a source entity using TransE. \
             Supports both directions: outgoing (h→t, what does X depend_on?) or incoming (t←h, what depends_on X?). \
             Uses TransE prediction: t̂ = h + r for outgoing, ĥ = t - r for incoming. \
             Can optionally filter to entities found in stored memories.",
            json!({
                "type": "object",
                "required": ["entity", "relation"],
                "properties": {
                    "entity": {
                        "type": "string",
                        "description": "Source entity to find relationships for."
                    },
                    "relation": {
                        "type": "string",
                        "description": "Relationship to search for (e.g., 'depends_on', 'implements', 'created_by')."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["outgoing", "incoming"],
                        "default": "outgoing",
                        "description": "Direction: outgoing (h→t) or incoming (t←h)."
                    },
                    "entityType": {
                        "type": "string",
                        "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"],
                        "description": "Filter results to specific entity type."
                    },
                    "topK": {
                        "type": "integer",
                        "description": "Maximum results to return (1-50, default: 10).",
                        "default": 10,
                        "minimum": 1,
                        "maximum": 50
                    },
                    "minScore": {
                        "type": "number",
                        "description": "Minimum TransE score threshold. More negative = less strict.",
                        "minimum": -10,
                        "maximum": 0
                    },
                    "searchMemories": {
                        "type": "boolean",
                        "description": "Filter to entities found in stored memories. Default: true.",
                        "default": true
                    }
                },
                "additionalProperties": false
            }),
        ),
        // validate_knowledge - Score a knowledge triple (Phase 3)
        ToolDefinition::new(
            "validate_knowledge",
            "Score whether a (subject, predicate, object) knowledge triple is valid using TransE. \
             Computes score = -||h + r - t||₂ where h=subject, r=predicate, t=object. \
             Score of 0 is perfect match. Returns validation result: 'valid', 'uncertain', or 'unlikely'. \
             Can also find supporting or contradicting memories in the knowledge graph.",
            json!({
                "type": "object",
                "required": ["subject", "predicate", "object"],
                "properties": {
                    "subject": {
                        "type": "string",
                        "description": "Subject/head entity of the triple."
                    },
                    "predicate": {
                        "type": "string",
                        "description": "Predicate/relation of the triple (e.g., 'created_by', 'depends_on')."
                    },
                    "object": {
                        "type": "string",
                        "description": "Object/tail entity of the triple."
                    },
                    "subjectType": {
                        "type": "string",
                        "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"],
                        "description": "Optional type hint for subject entity."
                    },
                    "objectType": {
                        "type": "string",
                        "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm"],
                        "description": "Optional type hint for object entity."
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_entity_graph - Visualize entity relationships (Phase 4)
        ToolDefinition::new(
            "get_entity_graph",
            "Build and visualize entity relationships discovered in stored memories. \
             Returns a graph with entity nodes and relationship edges. \
             If centerEntity is provided, focuses on that entity's neighborhood. \
             Infers relationships using TransE and weights edges by score and evidence count.",
            json!({
                "type": "object",
                "properties": {
                    "centerEntity": {
                        "type": "string",
                        "description": "Optional focal entity to center the graph on."
                    },
                    "maxNodes": {
                        "type": "integer",
                        "description": "Maximum number of nodes (1-500, default: 50).",
                        "default": 50,
                        "minimum": 1,
                        "maximum": 500
                    },
                    "maxDepth": {
                        "type": "integer",
                        "description": "Maximum hops from center entity (1-5, default: 2).",
                        "default": 2,
                        "minimum": 1,
                        "maximum": 5
                    },
                    "entityTypes": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["ProgrammingLanguage", "Framework", "Database", "Cloud", "Company", "TechnicalTerm", "Unknown"]
                        },
                        "description": "Filter to specific entity types."
                    },
                    "minRelationScore": {
                        "type": "number",
                        "description": "Minimum edge score threshold (0-1, default: 0.3).",
                        "default": 0.3,
                        "minimum": 0,
                        "maximum": 1
                    },
                    "includeMemoryCounts": {
                        "type": "boolean",
                        "description": "Include memory reference counts per node. Default: true.",
                        "default": true
                    }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_tool_count() {
        assert_eq!(definitions().len(), 6);
    }

    #[test]
    fn test_all_tools_have_names_and_descriptions() {
        for tool in definitions() {
            assert!(!tool.name.is_empty(), "Tool missing name");
            assert!(!tool.description.is_empty(), "Tool {} missing description", tool.name);
        }
    }

    #[test]
    fn test_extract_entities_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "extract_entities").unwrap();

        let required = tool
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("text")));

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("text"));
        assert!(props.contains_key("includeUnknown"));
        assert!(props.contains_key("groupByType"));
    }

    #[test]
    fn test_search_by_entities_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "search_by_entities").unwrap();

        let required = tool
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("entities")));

        let props = tool
            .input_schema
            .get("properties")
            .unwrap()
            .as_object()
            .unwrap();
        assert!(props.contains_key("entities"));
        assert!(props.contains_key("matchMode"));
        assert!(props.contains_key("topK"));
    }

    #[test]
    fn test_infer_relationship_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "infer_relationship").unwrap();

        let required = tool
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("headEntity")));
        assert!(required.contains(&json!("tailEntity")));
    }

    #[test]
    fn test_validate_knowledge_schema() {
        let tools = definitions();
        let tool = tools.iter().find(|t| t.name == "validate_knowledge").unwrap();

        let required = tool
            .input_schema
            .get("required")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("subject")));
        assert!(required.contains(&json!("predicate")));
        assert!(required.contains(&json!("object")));
    }

    #[test]
    fn test_descriptions_mention_e11_or_transe() {
        let tools = definitions();
        for tool in &tools {
            // Each tool should reference E11, TransE, or entity-related concepts
            assert!(
                tool.description.contains("E11")
                    || tool.description.contains("TransE")
                    || tool.description.contains("entit")
                    || tool.description.contains("canonical"),
                "Tool {} should mention E11, TransE, or entity concepts",
                tool.name
            );
        }
    }

    #[test]
    fn test_constitution_compliance_mentioned() {
        let tools = definitions();

        // extract_entities should mention ARCH-20
        let extract = tools.iter().find(|t| t.name == "extract_entities").unwrap();
        assert!(
            extract.description.contains("ARCH-20"),
            "extract_entities should mention ARCH-20 (entity linking)"
        );

        // search_by_entities should mention ARCH-12
        let search = tools.iter().find(|t| t.name == "search_by_entities").unwrap();
        assert!(
            search.description.contains("ARCH-12"),
            "search_by_entities should mention ARCH-12 (E1 foundation)"
        );
    }
}
