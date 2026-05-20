//! ME-JEPA Phase 4 inference MCP tool definitions.

use crate::tools::names as tool_names;
use crate::tools::types::ToolDefinition;
use serde_json::{Value, json};

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            tool_names::MEJEPA_VERIFY,
            "System 2 verifier surface: evaluate a System 1 LLM candidate patch through the slot-preserving CUDA ME-JEPA inference compiler. Deterministic fixture compilers are non-countable test helpers and are not exposed by this product tool.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["patch", "context"],
                "properties": {
                    "patch": {"type": "object", "additionalProperties": true},
                    "context": {"type": "object", "additionalProperties": true},
                    "includeProvenance": {"type": "boolean", "default": false}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PROJECT_INGEST,
            "TASK-PY-G-063 reality-compiler ingest: scan a permitted project file or directory tree, materialize the per-project cache/manifest directory under /zfs/archive/contextgraph/projects/<project_id>/ in production, route Python files through the AST chunker, and persist per-file RealityPrediction rows to CF_MEJEPA_LIVE_PREDICTIONS with source-of-truth readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["repoPath"],
                "properties": {
                    "repoPath": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Existing operator-permitted project file or directory path. Directories are scanned recursively; git repositories use git-tracked plus untracked non-ignored file discovery."
                    },
                    "projectId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 96,
                        "pattern": "^[A-Za-z0-9_.-]+$"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["Full", "Incremental"],
                        "default": "Full"
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["SourceOnly", "SourceAndTests", "All"],
                        "default": "SourceOnly"
                    },
                    "overwrite": {
                        "type": "boolean",
                        "default": false,
                        "description": "Required for a Full ingest when manifest.json already exists for the project_id."
                    },
                    "changedPaths": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "minLength": 1
                        },
                        "default": [],
                        "description": "Optional repo-relative paths supplied by a watcher/hook for fast Incremental ingest. When omitted, Incremental mode performs discovery."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PROJECT_REPORT,
            "TASK-PY-G-064 reality-compiler report: read an ingested project's local CF_MEJEPA_LIVE_PREDICTIONS rows, render durable report.json/report.md under /zfs/archive/contextgraph/projects/<project_id>/report/ in production, persist CF_MEJEPA_PROJECT_REPORTS, and return the JSON report.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["projectId"],
                "properties": {
                    "projectId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 96,
                        "pattern": "^(?!\\.)(?!.*\\.\\.)[A-Za-z0-9_.-]+$"
                    },
                    "section": {
                        "type": "string",
                        "enum": ["predictions"]
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PREDICT_LATEST,
            "Read the latest persisted System 2 ME-JEPA RealityPrediction rows for a System 1 agent session from CF_MEJEPA_LIVE_PREDICTIONS, including compact per-slot attribution summaries over the full stored record.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 10
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PREDICT_WHAT_IF,
            "System 2 what-if surface: compile a non-mutating ME-JEPA Phase B prediction for a System 1 candidate patch through the slot-preserving CUDA compiler. Reads calibration/train-cert state from the real inference RocksDB and returns repository SHA readback proving no touched file was modified.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["patch", "context"],
                "properties": {
                    "patch": {"type": "object", "additionalProperties": true},
                    "context": {"type": "object", "additionalProperties": true},
                    "compareToPredictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SEARCH_LATENT_ACTIONS,
            "System 2 planning surface: search System 1 candidate patches in ME-JEPA latent action space, score each candidate by predicted oracle pass, calibrated confidence, in-distribution mass, hierarchy signal, and optional goal-latent alignment, then return the ranked best-of-K list with repository SHA readback proving no touched file was modified.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["context", "candidates"],
                "properties": {
                    "context": {"type": "object", "additionalProperties": true},
                    "candidates": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 128,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["candidateId", "patch"],
                            "properties": {
                                "candidateId": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": 128
                                },
                                "patch": {"type": "object", "additionalProperties": true}
                            }
                        }
                    },
                    "config": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "maxCandidates": {
                                "type": "integer",
                                "minimum": 2,
                                "maximum": 128,
                                "default": 32
                            },
                            "goalLatent": {
                                "type": "array",
                                "minItems": 16,
                                "maxItems": 16,
                                "items": {"type": "number"}
                            },
                            "objectiveWeights": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "predictedOraclePass": {"type": "number", "minimum": 0},
                                    "calibratedConfidence": {"type": "number", "minimum": 0},
                                    "inDistribution": {"type": "number", "minimum": 0},
                                    "hierarchy": {"type": "number", "minimum": 0},
                                    "goalAlignment": {"type": "number", "minimum": 0}
                                }
                            }
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_RANK_CANDIDATES,
            "System 2 repair-ranking surface: rank System 1 counterfactual repair candidates atomically through ME-JEPA. Uses the latent action search backend, then ranks by predicted oracle pass times in-distribution probability while applying objective/safety reports so hard-blocked repairs remain visible but cannot outrank safe candidates.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["context", "candidates"],
                "properties": {
                    "context": {"type": "object", "additionalProperties": true},
                    "candidates": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 128,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["candidateId", "patch"],
                            "properties": {
                                "candidateId": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": 128
                                },
                                "patch": {"type": "object", "additionalProperties": true}
                            }
                        }
                    },
                    "config": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "maxCandidates": {
                                "type": "integer",
                                "minimum": 2,
                                "maximum": 128,
                                "default": 32
                            },
                            "goalLatent": {
                                "type": "array",
                                "minItems": 16,
                                "maxItems": 16,
                                "items": {"type": "number"}
                            }
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_MINCUT_PANEL,
            "TASK-EK-003 structural-hole surface: build an embedder, pairwise-MI, TCT constellation, failure-fingerprint, or inline weighted graph, run a deterministic mincut/sparsest-cut panel, persist MincutReport rows in CF_MEJEPA_MINCUT_REPORTS, and return readback provenance.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["graphSource"],
                "properties": {
                    "graphSource": {
                        "type": "object",
                        "description": "PanelGraphSource: kind=inlineWeightedGraph, embedderSimilarity, pairwiseMiMatrix, constellationInternal, or failureFingerprintGraph.",
                        "additionalProperties": true
                    },
                    "options": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "algorithm": {
                                "type": "string",
                                "enum": ["stoer_wagner", "sparsest_cut_approx"],
                                "default": "stoer_wagner"
                            },
                            "returnTopKCandidateDirections": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 16,
                                "default": 1
                            }
                        }
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "persist": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, write the report to CF_MEJEPA_MINCUT_REPORTS with read-after-write validation."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CHECK_BEDROCK_CONSISTENCY,
            "TASK-EK-016 bedrock consistency verifier: parse a unified patch diff, read persisted chunk foundationality scores from CF_MEJEPA_CHUNK_FOUNDATIONALITY, and report whether the patch touches load-bearing chunks. No inner LLM is invoked.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["patch"],
                "properties": {
                    "patch": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Unified diff text to check against persisted chunk foundationality scores."
                    },
                    "threshold": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.75
                    },
                    "topK": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 5
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_LIBRARY_FOUNDATIONALITY,
            "TASK-EK-017 library-level foundationality surface: read registered libraries, per-library PageRank scores, cross-library PageRank scores, and cross-library reference counts from RocksDB. No inner LLM is invoked.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "libraryId": {
                        "type": "string",
                        "description": "Optional library slug: python-swe-bench-lite, non-python-fixtures, shakespeare-canon, santa-training-video, customer-service-transcripts, or custom:<name>."
                    },
                    "topK": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 10
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PROPOSE_INSTRUMENT,
            "TASK-EK-002 ontology-extension surface: compose Unknown active-learning clusters, pairwise-MI health, and mincut structural-hole evidence into deterministic candidate frozen-instrument proposals. No inner LLM is invoked.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "persist": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, write proposals to CF_MEJEPA_INSTRUMENT_PROPOSALS with read-after-write validation."
                    },
                    "config": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "minClusterSize": {"type": "integer", "minimum": 1, "default": 30},
                            "tauIntra": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.85},
                            "tauFar": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.4},
                            "minExpectedHoldoutImprovement": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.01},
                            "maxProposals": {"type": "integer", "minimum": 1, "maximum": 1024, "default": 16},
                            "pairwiseMiMaxRows": {"type": "integer", "minimum": 1, "default": 1000000}
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PENDING_EMBEDDER_PROPOSALS,
            "TASK-EK-018B absence-detection queue: compose mincut, Unknown/OOD active learning, pairwise-MI residual, curiosity, and foundationality signals into ranked dynamic-embedder absence-shape descriptors in CF_MEJEPA_EMBEDDER_PROPOSALS. No inner LLM is invoked.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "persist": {
                        "type": "boolean",
                        "default": true,
                        "description": "When true, write pending proposals to CF_MEJEPA_EMBEDDER_PROPOSALS with read-after-write validation."
                    },
                    "config": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "maxProposals": {"type": "integer", "minimum": 1, "maximum": 1024, "default": 32},
                            "minSignalMagnitude": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.01},
                            "minCompositeScore": {"type": "number", "minimum": 0, "maximum": 1, "default": 0.0},
                            "pairwiseMiMaxRows": {"type": "integer", "minimum": 1, "default": 1000000}
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PENDING_EMBEDDER_APPROVALS,
            "TASK-EK-018H operator gate readback: list pending dynamic-embedder promotions awaiting operator review from CF_MEJEPA_MODEL_PROMOTIONS. Approval and rejection still flow through mcp__cgreality__mejepa_promote_approval.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PROMOTE_INSTRUMENT_PROPOSAL,
            "TASK-EK-002 operator/falsification gate: mark an instrument proposal under review, accept it if held-out delta clears threshold, or persist a rejected proposal decision in CF_MEJEPA_INSTRUMENT_PROPOSALS.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["proposalId", "decision"],
                "properties": {
                    "proposalId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "decision": {
                        "type": "string",
                        "enum": ["accept", "reject", "mark_under_review"]
                    },
                    "observedHoldoutDelta": {
                        "type": "number",
                        "minimum": -1,
                        "maximum": 1,
                        "default": 0
                    },
                    "minDeltaRequired": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.01
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_EXPLAIN_PREDICTION,
            "Explain a persisted System 2 ME-JEPA Phase B RealityPrediction by reading CF_MEJEPA_LIVE_PREDICTIONS and returning stored verdict, matched failure-shape fingerprint evidence, top risks, per-slot attribution summary, claim reconciliation, reality impact, provenance, and a saliency map normalized from covered chunks.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "includeFingerprintReferences": {
                        "type": "boolean",
                        "description": "When true, return top persisted CF_MEJEPA_FINGERPRINT_REFERENCES rows for the stored matched_fingerprint."
                    },
                    "fingerprintReferenceLimit": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 5
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_INSPECT_PREDICTION,
            "Inspect a persisted System 2 ME-JEPA RealityPrediction by reading CF_MEJEPA_LIVE_PREDICTIONS plus required per-chunk CF_MEJEPA_DDA_SIGNALS rows, returning storage provenance, witness/panel identifiers, conformal computation trace, TCT guard cells, full machine-readable slot attributions, and contributing chunk signal vectors.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_REPLAY_PREDICTION,
            "Replay a persisted System 2 ME-JEPA RealityPrediction byte-for-byte from CF_MEJEPA_LIVE_PREDICTIONS by validating the key/payload provenance and reserializing the decoded prediction.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_REALITY_IMPACT,
            "TASK-PY-G-045 Q5 replay surface: read a persisted RealityPrediction from CF_MEJEPA_LIVE_PREDICTIONS, compare predicted consequences to the durable session shift log, persist CF_MEJEPA_REALITY_IMPACT with readback, and return Confirmed/Missed/NotYetObserved/Surprise rows.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId", "runtimeRoot"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "runtimeRoot": {
                        "type": "string",
                        "description": "Runtime root containing cgreality-shift-log/, or the cgreality-shift-log directory itself."
                    },
                    "replayWindowMs": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 3600000
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_RECORD_AGENT_FEEDBACK,
            "Persist measured System 1 agent feedback for a prior System 2 ME-JEPA RealityPrediction into CF_MEJEPA_AGENT_FEEDBACK, update the active-learning queue when the feedback is surprise-like, and verify RocksDB readback before returning.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId", "feedbackKind", "agentExplanation", "severity"],
                "properties": {
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "agentId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256,
                        "default": "anonymous",
                        "description": "Configured signed agent id. Omit or set to anonymous for unauthenticated feedback."
                    },
                    "identityAttestation": identity_attestation_schema(),
                    "feedbackKind": {
                        "type": "string",
                        "enum": ["confirmed", "surprise", "omission", "calibration"]
                    },
                    "agentExplanation": {
                        "type": "string",
                        "maxLength": 4096
                    },
                    "actualOutcome": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["oracleOutcome", "failedTests", "notes"],
                        "properties": {
                            "oracleOutcome": {
                                "type": "string",
                                "enum": ["pass", "fail", "out_of_distribution", "abstain"]
                            },
                            "failedTests": {
                                "type": "array",
                                "items": {"type": "string", "minLength": 1},
                                "maxItems": 1024
                            },
                            "runtimeMs": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 86400000
                            },
                            "notes": {"type": "string"}
                        }
                    },
                    "severity": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "catastrophic"]
                    },
                    "extraStructuredData": {
                        "type": "object",
                        "additionalProperties": true,
                        "default": {}
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_OBSERVE_SHIFT,
            "Replay a durable System 1 reality-shift log entry through the Phase 7 ME-JEPA System 2 subscriber. Fails closed if the subscriber replay channel is not available.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["shiftId"],
                "properties": {
                    "shiftId": {
                        "type": "string",
                        "pattern": "^01J[0-9A-F]{20}$"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SUBSCRIBER_STATUS,
            "Read the Phase 7 shift-subscriber persisted watermarks and runtime health snapshot.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CAPTURE_AUDIT,
            "Assemble a Phase 7 System 2 ME-JEPA signal-capture audit bundle for a System 1 attempt from persisted panel and inference RocksDB sources of truth.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["attemptId"],
                "properties": {
                    "attemptId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "page": {
                        "type": "integer",
                        "minimum": 0,
                        "default": 0
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CONSTELLATION_INSPECT,
            "Inspect a persisted ME-JEPA TCT constellation from CF_MEJEPA_CONSTELLATION. Returns metadata, counts, support, and thresholds only; raw centroid vectors are never returned.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["runtimeEmbedderVersions"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_TCT_DB is required."
                    },
                    "versionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{64}$"
                    },
                    "runtimeEmbedderVersions": {
                        "type": "object",
                        "additionalProperties": {
                            "type": "string",
                            "pattern": "^[0-9a-fA-F]{64}$"
                        },
                        "minProperties": 21
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_HEAL_STATUS,
            "Inspect the persisted ME-JEPA Phase 5 self-healing RocksDB source of truth: CF counts, active pointers, and latest heal report metadata. Refuses to guess dbPath unless CONTEXTGRAPH_MEJEPA_HEAL_DB is set.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_DAEMON_STATUS,
            "Aggregate ME-JEPA Phase F runtime status in one structured call: process mode, shift subscriber, self-healing store, hygiene quota, and optional CUDA-driver VRAM readback. Missing component SoTs are reported as unavailable instead of guessed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "inferDbPath": {
                        "type": "string",
                        "description": "Inference/hygiene RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used for subscriber and as quotaDbPath fallback."
                    },
                    "panelDbPath": {
                        "type": "string",
                        "description": "Panel RocksDB path for subscriber panel counts. If omitted, CONTEXTGRAPH_MEJEPA_PANEL_DB is used when present."
                    },
                    "healDbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is used when present."
                    },
                    "quotaDbPath": {
                        "type": "string",
                        "description": "Hygiene RocksDB path. If omitted, inferDbPath/CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "archiveRoot": {
                        "type": "string",
                        "description": "Hygiene archive root. If omitted, CONTEXTGRAPH_MEJEPA_HYGIENE_ARCHIVE_ROOT is used when present."
                    },
                    "includeVram": {
                        "type": "boolean",
                        "default": true
                    },
                    "vramBudget": {
                        "type": "string",
                        "enum": ["content_set", "full_phase1"],
                        "default": "content_set"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PAUSE_PREDICTIONS,
            "Pause ME-JEPA prediction serving until now + durationMins by writing a durable pause-state JSON file and reading it back byte-for-byte.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["durationMins"],
                "properties": {
                    "statePath": {
                        "type": "string",
                        "description": "Pause-state JSON path. If omitted, CONTEXTGRAPH_MEJEPA_PAUSE_PATH or the aiwonder default is used."
                    },
                    "durationMins": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10080
                    },
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096,
                        "default": "manual pause"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_OPERATOR_OVERRIDE_PREDICTION,
            "Record an operator override for a persisted ME-JEPA RealityPrediction. Persists CF_MEJEPA_OPERATOR_OVERRIDES, writes a human active-learning label, and returns the 6x sampling-weight flag readback for the next batch.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["predictionId", "overrideVerdict", "reason", "operatorId"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "predictionId": {
                        "type": "string",
                        "pattern": "^[0-9a-fA-F]{32}$"
                    },
                    "overrideVerdict": {
                        "type": "string",
                        "enum": ["pass", "fail", "abstain", "out_of_distribution"]
                    },
                    "reason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096
                    },
                    "operatorId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256
                    },
                    "identityAttestation": identity_attestation_schema()
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_OPERATOR_CONTRIBUTIONS,
            "Read TASK-EK-014 operator-contribution activity, quality ranking, downstream-outcome links, and migration-rate trend from CF_MEJEPA_OPERATOR_CONTRIBUTIONS.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["window"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional Phase 4 inference RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "window": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10000
                    },
                    "operatorId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "markdown"],
                        "default": "json"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_ROLLBACK_TO,
            "Rollback ME-JEPA active weights to a previously promoted witness-chain offset. The target must exist in CF_MEJEPA_HEAL_REPORTS and the weight blob must exist in CF_MEJEPA_WEIGHT_BLOBS; otherwise the tool fails closed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["targetWitnessChainOffset", "witnessChainPath"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    },
                    "witnessChainPath": {
                        "type": "string"
                    },
                    "targetWitnessChainOffset": {
                        "type": "integer",
                        "minimum": 0
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_PROMOTE_APPROVAL,
            "Approve or reject a pending catastrophic ME-JEPA self-optimization promotion. The approval record is read from and written back to CF_MEJEPA_MODEL_PROMOTIONS; non-pending or non-catastrophic promotions fail closed.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["promotionId", "operatorId", "action", "operatorReason"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Self-healing RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_HEAL_DB is required."
                    },
                    "promotionId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "operatorId": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    },
                    "action": {
                        "type": "string",
                        "enum": ["approve", "reject"]
                    },
                    "operatorReason": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 4096,
                        "description": "Single-line operator rationale persisted with the approval/rejection event."
                    },
                    "twoPersonRule": {
                        "type": "boolean",
                        "default": true,
                        "description": "Defaults to true; catastrophic promotions require two distinct operator approvals unless an existing queued record already requires more."
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_GC_RUN,
            "Run the Phase 6 ME-JEPA hygiene GC against a real RocksDB source of truth: tier demotion, compaction, quota enforcement, checkpoint/calibration retention, witness compression, witness verification, and CF_MEJEPA_GC_HISTORY readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_QUOTA_STATUS,
            "Read Phase 6 ME-JEPA quota status by scanning real RocksDB column families and witness archive files. No cached counters are trusted.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_WITNESS_COMPRESS,
            "Compress eligible old ME-JEPA witness-chain segments into Merkle-root entries, persist archives to archiveRoot, and verify chain integrity by readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "archiveRoot"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "archiveRoot": {"type": "string"},
                    "segmentSize": {"type": "integer", "minimum": 1, "default": 1024},
                    "minAgeDays": {"type": "integer", "minimum": 0, "default": 1}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_EVAL_RUN,
            "Run the Phase 8 ME-JEPA holdout evaluation against a real RocksDB source of truth, persist CF_MEJEPA_EVAL_REPORTS, and read back the report hash before returning.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "repoRoot", "outputFsv", "reportDate"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "repoRoot": {"type": "string"},
                    "outputFsv": {"type": "string"},
                    "reportDate": {"type": "string", "minLength": 1}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SHIP_GATE_STATUS,
            "Read Phase 8 EvalReports from CF_MEJEPA_EVAL_REPORTS and return ship-gate status, including the four-consecutive-window stability streak, active cell exemptions, and persisted report hash.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD,
            "Read the latest weekly EvalReport from CF_MEJEPA_EVAL_REPORTS, reconcile it with weekly export files, and return the Phase F operator dashboard/runbook status.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "dRoot": {
                        "type": "string",
                        "description": "aiwonder ME-JEPA root containing state/gold-labels and exports. Defaults to CONTEXTGRAPH_DATA_ROOT or is inferred from exportsRoot when possible."
                    },
                    "exportsRoot": {
                        "type": "string",
                        "description": "Directory containing dated weekly eval exports. Defaults to CONTEXTGRAPH_DATA_ROOT/exports/eval."
                    },
                    "maxCells": {"type": "integer", "minimum": 1, "maximum": 500, "default": 64}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_ACTIVE_LEARNING_QUEUE,
            "Read the Phase 8 active-learning queue from CF_MEJEPA_ACTIVE_LEARNING_QUEUE, ordered by scheduler priority or TASK-EK-006 curiosity.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "rankedBy": {
                        "type": "string",
                        "enum": ["scheduler_priority", "curiosity"],
                        "default": "scheduler_priority"
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_LIST,
            "Read the Phase G failure-shape fingerprint catalog from CF_MEJEPA_FAILURE_FINGERPRINTS and return compact catalog metadata.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {"type": "string"},
                    "kind": {"type": "string", "enum": ["known_good", "known_bad", "unknown"]},
                    "sourceCorpus": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 1000}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_INSPECT,
            "Inspect one failure-shape fingerprint by id, including centroid, variance, tau, references, calibration records, and audit source-of-truth metadata.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["fingerprintId"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "fingerprintId": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "referenceLimit": {"type": "integer", "minimum": 0, "maximum": 10000, "default": 100},
                    "calibrationLimit": {"type": "integer", "minimum": 0, "maximum": 1000, "default": 20}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_CLASSIFY,
            "Classify an observation vector map against CF_MEJEPA_FAILURE_FINGERPRINTS using the deterministic per-embedder Gtau fingerprint guard.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["observationByEmbedder"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "observationByEmbedder": {
                        "type": "object",
                        "minProperties": 1,
                        "additionalProperties": {
                            "type": "array",
                            "minItems": 1,
                            "items": {"type": "number"}
                        }
                    },
                    "topK": {"type": "integer", "minimum": 1, "maximum": 100, "default": 3}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_SUGGEST_NEW,
            "Read CF_MEJEPA_ACTIVE_LEARNING_QUEUE and return deterministic Unknown/OOD cluster suggestions for candidate new fingerprints.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {"type": "string"},
                    "minClusterSize": {"type": "integer", "minimum": 1, "maximum": 1000, "default": 3},
                    "cosineThreshold": {"type": "number", "minimum": -1.0, "maximum": 1.0, "default": 0.98}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_LABEL,
            "Persist an operator label for an Unknown fingerprint queue candidate and add the corresponding fingerprint/reference/audit rows to the catalog source of truth.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["candidateId", "oracleOutcome", "operatorId", "referenceChunkId", "witnessHash", "sourceManifestSha256"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "candidateId": {"type": "string", "pattern": "^[0-9a-fA-F]{32}$"},
                    "oracleOutcome": {"type": "string", "enum": ["pass", "fail", "out_of_distribution", "abstain"]},
                    "method": {"type": "string", "enum": ["human", "oracle_replay", "verified_witness"], "default": "human"},
                    "operatorId": {"type": "string", "minLength": 1, "maxLength": 256},
                    "catalogKind": {"type": "string", "enum": ["unknown", "known_good", "known_bad"], "default": "unknown"},
                    "name": {"type": "string", "minLength": 1, "maxLength": 512},
                    "repo": {"type": "string", "minLength": 1, "maxLength": 512},
                    "mutationCategory": {"type": "string", "enum": ["known_good", "subtle_flip", "off_by_one", "swap_variable", "delete_test_call", "wrong_file", "over_engineer", "compile_error"]},
                    "failureMode": {"type": "string", "minLength": 1, "maxLength": 128},
                    "referenceChunkId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "referenceId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "witnessHash": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "sourceManifestSha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "sourceCorpus": {"type": "string", "minLength": 1, "maxLength": 512, "default": "mcp-fingerprint-label-v1"},
                    "tauByEmbedder": {
                        "type": "object",
                        "additionalProperties": {"type": "number", "minimum": -1.0, "maximum": 1.0}
                    },
                    "allowOverwrite": {"type": "boolean", "default": false}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_PROMOTE_CANONICAL,
            "Mark a fingerprint as canonical and add a canonical reference chunk, writing CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FINGERPRINT_REFERENCES, reverse index, and audit readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["fingerprintId", "taskId", "repo", "mutationCategory", "chunkId", "oracleOutcome", "witnessHash", "sourceManifestSha256", "operatorId"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "fingerprintId": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "taskId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "repo": {"type": "string", "minLength": 1, "maxLength": 512},
                    "mutationCategory": {"type": "string", "enum": ["known_good", "subtle_flip", "off_by_one", "swap_variable", "delete_test_call", "wrong_file", "over_engineer", "compile_error"]},
                    "chunkId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "referenceId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "oracleOutcome": {"type": "string", "enum": ["pass", "fail", "out_of_distribution", "abstain"]},
                    "witnessHash": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "sourceManifestSha256": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "operatorId": {"type": "string", "minLength": 1, "maxLength": 256}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_RECALIBRATE,
            "Force recalibration of one failure-shape fingerprint by replacing tau_by_embedder, writing a calibration record, and verifying readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["fingerprintId", "tauByEmbedder", "sampleCount", "operatorId"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "fingerprintId": {"type": "string", "pattern": "^[0-9a-fA-F]{64}$"},
                    "tauByEmbedder": {
                        "type": "object",
                        "minProperties": 1,
                        "additionalProperties": {"type": "number", "minimum": -1.0, "maximum": 1.0}
                    },
                    "sameSessionBandPercentile": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.10},
                    "sampleCount": {"type": "integer", "minimum": 1},
                    "operatorId": {"type": "string", "minLength": 1, "maxLength": 256}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_FINGERPRINT_CATALOG_STABILITY,
            "Read per-fingerprint catalog stability from CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS plus TASK-FP-008 Fisher/dormancy CFs, returning four-window accuracy/precision drift and EWC/CBP readiness.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {"type": "string"},
                    "windowLimit": {"type": "integer", "minimum": 2, "maximum": 32, "default": 4},
                    "maxAccuracyDrift": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.02},
                    "maxPrecisionDrift": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.02},
                    "dormancyThreshold": {"type": "number", "minimum": 0.0, "maximum": 1.0, "default": 0.05}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_BOOTSTRAP_STATUS,
            "Read ME-JEPA System 2 bootstrap stage and source-of-truth CF counts from the configured inference RocksDB. Refuses to guess a database path.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_AUDIT_REWARD_SIGNALS,
            "Read ME-JEPA reward-signal completeness from durable RocksDB source-of-truth CFs. Returns per-tier coverage, fingerprint feature-span status, lifelong-learning loop status, constellation freshness status, and CF_MEJEPA_SIGNAL_DROP_LOG readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "minCoverage": {
                        "type": "number",
                        "minimum": 0.0,
                        "maximum": 1.0,
                        "default": 0.95
                    },
                    "signalDropSampleLimit": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 1000,
                        "default": 10
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_COMPRESSION_PROGRESS,
            "Read Schmidhuberian compression progress from CF_MEJEPA_TRAIN_CERTS. Returns rolling CP_Phi over recent TrainingCertificate rows, monotonicity, and an ASCII sparkline.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA training RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_TRAIN_DB or CONTEXTGRAPH_MEJEPA_INFER_DB is used."
                    },
                    "window": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1000
                    },
                    "epsilonBits": {
                        "type": "number",
                        "minimum": 0.0,
                        "default": 0.000000001
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_EVAL_BUILD_GRAPH,
            "Build and persist the Phase 8 patch-similarity graph into CF_MEJEPA_TASK_GRAPH and verify readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["dbPath", "outputFsv"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "outputFsv": {"type": "string"},
                    "threshold": {"type": "number", "minimum": -1.0, "maximum": 1.0, "default": 0.85},
                    "topK": {"type": "integer", "minimum": 1, "default": 3}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_AUDIT_PAIRWISE_MI,
            "Compute the UTML pairwise mutual-information audit from real per-slot series, persist the CSV matrix, optionally persist CF_MEJEPA_PAIRWISE_MI rows, reload both sources of truth, and return redundancy/effective-signal summaries.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["outputDir", "step", "seriesBySlot"],
                "properties": {
                    "outputDir": {
                        "type": "string",
                        "description": "Directory under CONTEXTGRAPH_DATA_ROOT where the audit CSV is written"
                    },
                    "step": {
                        "type": "integer",
                        "minimum": 0
                    },
                    "periodSteps": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1
                    },
                    "dbPath": {
                        "type": "string",
                        "description": "Optional ME-JEPA RocksDB path; when supplied the audit persists pair rows to CF_MEJEPA_PAIRWISE_MI"
                    },
                    "persistToCf": {
                        "type": "boolean",
                        "default": false
                    },
                    "corpusShardHash": {
                        "type": "string",
                        "pattern": "^[0-9A-Fa-f]{64}$",
                        "description": "Optional held-out corpus shard hash; defaults to a deterministic digest of the audited matrix"
                    },
                    "createdAtUnixMs": {
                        "type": "integer",
                        "minimum": 1
                    },
                    "seriesBySlot": {
                        "type": "object",
                        "minProperties": 2,
                        "additionalProperties": {
                            "type": "array",
                            "minItems": 2,
                            "items": {"type": "number"}
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SKILL_TO_CODE,
            "TASK-PY-G-120 skill-to-code linkage: read a Level-2 skill and its persisted chunk memberships, optionally join AST chunk JSONL source bytes, and return byte-grounded member chunks without introducing a prediction head.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["skillId"],
                "properties": {
                    "skillId": {"type": "string", "minLength": 1},
                    "dbPath": {"type": "string", "description": "Optional #414 skill RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_TRAIN_DB or CONTEXTGRAPH_MEJEPA_INFER_DB is used."},
                    "chunksJsonl": {"type": "string", "description": "Optional AST chunk JSONL with source_text/source byte spans for literal code readback."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 20},
                    "requireSourceText": {"type": "boolean", "default": false}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_CODE_TO_SKILL,
            "TASK-PY-G-120 code-to-skill linkage: read all persisted skills a chunk participates in, preserving many-to-many membership and optional source-byte provenance.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["chunkId"],
                "properties": {
                    "chunkId": {"type": "string", "minLength": 1},
                    "codeStateKey": {"type": "string", "minLength": 1},
                    "dbPath": {"type": "string"},
                    "chunksJsonl": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 20},
                    "requireSourceText": {"type": "boolean", "default": false}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SKILL_SET_QUERY,
            "TASK-PY-G-120 boolean query over chunk-skill memberships: return chunks that have all required skills and none of the excluded skills.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["mustHave"],
                "properties": {
                    "mustHave": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                    "mustNotHave": {"type": "array", "items": {"type": "string", "minLength": 1}, "default": []},
                    "dbPath": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 100}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_SKILL_COVERAGE_AUDIT,
            "TASK-PY-G-120 skill coverage audit: partition chunk universe into chunks with/without skill memberships and surface active-learning gaps.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {"type": "string"},
                    "chunksJsonl": {"type": "string", "description": "Optional AST chunk JSONL defining the audit universe. If omitted, the universe is all chunks with at least one persisted membership."},
                    "sampleLimit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 100}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_RECORD_MISTAKE,
            "TASK-PY-G-112B mistake-loop writer: persist a durable mistake row and replay row, then apply the exact-panel online-head update through RocksDB source-of-truth readback.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["panelSignatureHash", "mistakeRow", "replayRow", "baseVerdictBeforeUpdate", "nowUnixMs"],
                "properties": {
                    "dbPath": {
                        "type": "string",
                        "description": "Optional #406/#418 RocksDB path. If omitted, CONTEXTGRAPH_MEJEPA_TRAIN_DB or CONTEXTGRAPH_MEJEPA_INFER_DB is used. Legacy /mnt/d roots fail closed."
                    },
                    "panelSignatureHash": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 512,
                        "pattern": "^[A-Za-z0-9:_-]+$"
                    },
                    "mistakeRow": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Bincode-compatible MistakeLogRow JSON. acceptedLabelIds must contain only live-safe labels; target-side oracle/docker/test labels fail closed."
                    },
                    "replayRow": {
                        "type": "object",
                        "additionalProperties": true,
                        "description": "Bincode-compatible ReplayBufferRow JSON with label/skill/ability/membership signatures matching the mistake row."
                    },
                    "baseVerdictBeforeUpdate": {
                        "type": "string",
                        "enum": ["pass", "fail"]
                    },
                    "nowUnixMs": {
                        "type": "integer",
                        "minimum": 1
                    },
                    "onlineHeadConfig": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "learningRate": {"type": "number", "exclusiveMinimum": 0, "maximum": 1},
                            "repeatWindowSize": {"type": "integer", "minimum": 1}
                        }
                    }
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_MISTAKE_HISTORY,
            "TASK-PY-G-112B mistake-loop history: read CF_MEJEPA_MISTAKE_LOG rows with optional replay-buffer and skill-lifecycle audit joins, preserving label/skill/ability/membership signatures.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "dbPath": {"type": "string"},
                    "mistakeId": {"type": "string", "minLength": 1, "maxLength": 512},
                    "predictionIdHex": {"type": "string", "pattern": "^[0-9A-Fa-f]{32}$"},
                    "codeStateKey": {"type": "string", "minLength": 1, "maxLength": 512},
                    "labelSignatureHash": {"type": "string", "minLength": 1, "maxLength": 512},
                    "skillSignatureHash": {"type": "string", "minLength": 1, "maxLength": 512},
                    "abilitySignatureHash": {"type": "string", "minLength": 1, "maxLength": 512},
                    "membershipSignatureHash": {"type": "string", "minLength": 1, "maxLength": 512},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 10000, "default": 100},
                    "includeReplayRows": {"type": "boolean", "default": true},
                    "includeLifecycleAudits": {"type": "boolean", "default": true}
                }
            }),
        ),
        ToolDefinition::new(
            tool_names::MEJEPA_MISTAKE_LOOP_STATUS,
            "TASK-PY-G-112B online-head status: read the exact online_head:<panelSignatureHash> row, byte-readable repeat-mistake metrics, and source CF counts directly from RocksDB.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["panelSignatureHash"],
                "properties": {
                    "dbPath": {"type": "string"},
                    "panelSignatureHash": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 512,
                        "pattern": "^[A-Za-z0-9:_-]+$"
                    },
                    "repeatMetricLimit": {"type": "integer", "minimum": 0, "maximum": 10000, "default": 20}
                }
            }),
        ),
    ]
}

fn identity_attestation_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["sessionId", "nonce", "timestampUnixMs", "signatureHex"],
        "properties": {
            "sessionId": {
                "type": "string",
                "minLength": 1,
                "maxLength": 256
            },
            "nonce": {
                "type": "string",
                "minLength": 8,
                "maxLength": 256
            },
            "timestampUnixMs": {
                "type": "integer",
                "minimum": 1
            },
            "signatureHex": {
                "type": "string",
                "pattern": "^[0-9a-fA-F]{64}$"
            }
        }
    })
}
