//! Hook types for Claude Code native integration
//!
//! # Architecture
//! This module defines the data types for hook input/output that match
//! Claude Code's native hook system specification.
//!
//! # Constitution References
//! - IDENTITY-002: IC thresholds and timeout requirements
//! - AP-25: Kuramoto N=13
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks (use Claude Code native)
//! - AP-53: Hook logic in shell scripts calling CLI
//!
//! # NO BACKWARDS COMPATIBILITY
//! This module FAILS FAST on any error. Do not add fallback logic.

mod types;

pub use types::{
    ConsciousnessState,
    ConversationMessage,
    HookEventType,
    HookInput,
    HookOutput,
    HookPayload,
    ICClassification,
    ICLevel,
    JohariQuadrant,
    SessionEndStatus,
};
