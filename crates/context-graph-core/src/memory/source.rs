//! Memory source types for discriminating origin of captured memories.
//!
//! # Constitution Compliance
//! - ARCH-11: Memory sources: HookDescription, ClaudeResponse, MDFileChunk
//! - Hook types per .claude/settings.json native hook architecture

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Discriminated source type for Memory origin.
///
/// Per constitution.yaml ARCH-11 and memory_sources section:
/// - HookDescription: From Claude Code hook events
/// - ClaudeResponse: From session end/stop captured responses
/// - MDFileChunk: From markdown file watcher chunks
/// - CausalExplanation: legacy generated causal explanation records from the retired E5+LLM path
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemorySource {
    /// Memory captured from a Claude Code hook event.
    HookDescription {
        /// The type of hook that triggered capture.
        hook_type: HookType,
        /// Tool name if applicable (PreToolUse, PostToolUse).
        tool_name: Option<String>,
    },
    /// Memory captured from Claude's response.
    ClaudeResponse {
        /// The type of response captured.
        response_type: ResponseType,
    },
    /// Memory captured from markdown file chunking.
    MDFileChunk {
        /// Path to the source markdown file.
        file_path: String,
        /// Zero-based index of this chunk.
        chunk_index: u32,
        /// Total number of chunks from the file.
        total_chunks: u32,
    },
    /// Memory created from a legacy generated causal explanation.
    ///
    /// Legacy source type for old generated causal explanations. The E5+LLM
    /// generation path is retired; new ME-JEPA training should use active
    /// embedders and durable reality evidence instead of creating these records.
    CausalExplanation {
        /// UUID of the original memory that was analyzed.
        source_fingerprint_id: Uuid,
        /// Link to the associated CausalRelationship (for dual lookup).
        causal_relationship_id: Uuid,
        /// Type of causal mechanism: "direct", "mediated", "feedback", "temporal"
        mechanism_type: String,
        /// LLM confidence score [0.0, 1.0].
        confidence: f32,
    },
}

/// Hook event types matching .claude/settings.json native hooks.
///
/// Per constitution.yaml claude_code.hooks section:
/// - SessionStart: Session initialization
/// - UserPromptSubmit: User sends a prompt
/// - PreToolUse: Before tool execution (Edit, Write, Bash)
/// - PostToolUse: After any tool execution
/// - Stop: Claude stops responding
/// - SessionEnd: Session cleanup
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum HookType {
    /// Session initialization hook (timeout: 5000ms).
    SessionStart,
    /// User prompt submission hook (timeout: 2000ms).
    UserPromptSubmit,
    /// Pre-tool-use hook for Edit|Write|Bash (timeout: 500ms).
    PreToolUse,
    /// Post-tool-use hook for all tools (timeout: 3000ms, async).
    PostToolUse,
    /// Stop hook when Claude stops (timeout: 3000ms).
    Stop,
    /// Session end hook (timeout: 30000ms).
    SessionEnd,
}

/// Response types for ClaudeResponse memory source.
///
/// Distinguishes the context in which Claude's response was captured.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResponseType {
    /// Summary captured at session end.
    SessionSummary,
    /// Response captured at Stop hook.
    StopResponse,
    /// Significant response worth persisting.
    SignificantResponse,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HookType::SessionStart => "SessionStart",
            HookType::UserPromptSubmit => "UserPromptSubmit",
            HookType::PreToolUse => "PreToolUse",
            HookType::PostToolUse => "PostToolUse",
            HookType::Stop => "Stop",
            HookType::SessionEnd => "SessionEnd",
        })
    }
}

impl std::fmt::Display for ResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ResponseType::SessionSummary => "SessionSummary",
            ResponseType::StopResponse => "StopResponse",
            ResponseType::SignificantResponse => "SignificantResponse",
        })
    }
}

impl std::fmt::Display for MemorySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemorySource::HookDescription {
                hook_type,
                tool_name,
            } => match tool_name {
                Some(tool) => write!(f, "HookDescription({hook_type}, tool={tool})"),
                None => write!(f, "HookDescription({hook_type})"),
            },
            MemorySource::ClaudeResponse { response_type } => {
                write!(f, "ClaudeResponse({response_type})")
            }
            MemorySource::MDFileChunk {
                file_path,
                chunk_index,
                total_chunks,
            } => {
                write!(
                    f,
                    "MDFileChunk({file_path}, {}/{})",
                    chunk_index + 1,
                    total_chunks
                )
            }
            MemorySource::CausalExplanation {
                source_fingerprint_id,
                causal_relationship_id,
                mechanism_type,
                confidence,
            } => {
                write!(
                    f,
                    "CausalExplanation(source={}, rel={}, type={}, conf={:.2})",
                    source_fingerprint_id, causal_relationship_id, mechanism_type, confidence
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_type_variants() {
        // Verify all 6 variants exist per constitution
        let types = [
            HookType::SessionStart,
            HookType::UserPromptSubmit,
            HookType::PreToolUse,
            HookType::PostToolUse,
            HookType::Stop,
            HookType::SessionEnd,
        ];
        assert_eq!(types.len(), 6);
    }

    #[test]
    fn test_response_type_variants() {
        // Verify all 3 variants exist
        let types = [
            ResponseType::SessionSummary,
            ResponseType::StopResponse,
            ResponseType::SignificantResponse,
        ];
        assert_eq!(types.len(), 3);
    }

    #[test]
    fn test_memory_source_variants() {
        // Verify all 3 variants exist per ARCH-11
        let sources = [
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            MemorySource::ClaudeResponse {
                response_type: ResponseType::SessionSummary,
            },
            MemorySource::MDFileChunk {
                file_path: "test.md".to_string(),
                chunk_index: 0,
                total_chunks: 1,
            },
        ];
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let source = MemorySource::HookDescription {
            hook_type: HookType::PostToolUse,
            tool_name: Some("Edit".to_string()),
        };

        let serialized = serde_json::to_string(&source).expect("serialize failed");
        let deserialized: MemorySource =
            serde_json::from_str(&serialized).expect("deserialize failed");

        assert_eq!(source, deserialized);
    }

    #[test]
    fn test_bincode_serialization() {
        let source = MemorySource::MDFileChunk {
            file_path: "/path/to/file.md".to_string(),
            chunk_index: 5,
            total_chunks: 10,
        };

        let bytes = bincode::serialize(&source).expect("bincode serialize failed");
        let restored: MemorySource =
            bincode::deserialize(&bytes).expect("bincode deserialize failed");

        assert_eq!(source, restored);
    }

    #[test]
    fn test_display_implementations() {
        assert_eq!(HookType::SessionStart.to_string(), "SessionStart");
        assert_eq!(ResponseType::StopResponse.to_string(), "StopResponse");

        let source = MemorySource::HookDescription {
            hook_type: HookType::PreToolUse,
            tool_name: Some("Bash".to_string()),
        };
        assert!(source.to_string().contains("PreToolUse"));
        assert!(source.to_string().contains("Bash"));
    }

    #[test]
    fn test_hook_type_copy_trait() {
        // Verify HookType implements Copy
        let hook = HookType::SessionStart;
        let hook_copy = hook; // This works because of Copy
        assert_eq!(hook, hook_copy);
    }

    #[test]
    fn test_response_type_copy_trait() {
        // Verify ResponseType implements Copy
        let resp = ResponseType::SessionSummary;
        let resp_copy = resp; // This works because of Copy
        assert_eq!(resp, resp_copy);
    }

    #[test]
    fn test_memory_source_clone() {
        // Verify MemorySource can be cloned
        let source = MemorySource::ClaudeResponse {
            response_type: ResponseType::SignificantResponse,
        };
        let cloned = source.clone();
        assert_eq!(source, cloned);
    }

    #[test]
    fn test_md_file_chunk_display_format() {
        let source = MemorySource::MDFileChunk {
            file_path: "README.md".to_string(),
            chunk_index: 0, // 0-indexed
            total_chunks: 5,
        };
        // Display should show 1-indexed for human readability
        let display = source.to_string();
        assert!(
            display.contains("1/5"),
            "Display should show 1-indexed chunks: {}",
            display
        );
    }

    #[test]
    fn test_all_hook_types_serializable() {
        for hook in [
            HookType::SessionStart,
            HookType::UserPromptSubmit,
            HookType::PreToolUse,
            HookType::PostToolUse,
            HookType::Stop,
            HookType::SessionEnd,
        ] {
            let source = MemorySource::HookDescription {
                hook_type: hook,
                tool_name: None,
            };
            let bytes = bincode::serialize(&source).expect("serialize failed");
            let restored: MemorySource = bincode::deserialize(&bytes).expect("deserialize failed");
            assert_eq!(source, restored);
        }
    }
}
