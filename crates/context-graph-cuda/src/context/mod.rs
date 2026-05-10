//! Green Contexts GPU partitioning module.
//!
//! Provides 70% inference / 30% background SM partitioning for RTX 5090+.
//! Fails closed on older GPUs or failed CUDA device-attribute checks.

pub mod green_contexts;

pub use green_contexts::{
    should_enable_green_contexts, should_enable_green_contexts_with_config, GreenContext,
    GreenContexts, GreenContextsConfig, BACKGROUND_PARTITION_PERCENT,
    GREEN_CONTEXTS_MIN_COMPUTE_MAJOR, GREEN_CONTEXTS_MIN_COMPUTE_MINOR,
    INFERENCE_PARTITION_PERCENT, MIN_SMS_FOR_PARTITIONING,
};
