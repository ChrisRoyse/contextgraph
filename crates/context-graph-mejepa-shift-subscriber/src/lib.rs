// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

//! Phase 7 durable JSONL shift subscriber for ME-JEPA.

mod cache;
mod error;
mod models;
mod runner;
mod shift_transform;
mod status;
mod subscriber;
mod tail;
mod watermark;

pub use cache::{InstrumentCache, SlotKey};
pub use error::{Result, SubscriberError};
pub use models::{
    decode_session_hex32, CaptureAuditBundle, LatencySnapshot, LiveSubscriberStatus, PanicInfo,
    ShiftEntry, ShiftId, ShiftOutcome, ShiftSide, SkipReason, SubscriberMetrics, UtmlFactorBundle,
    WatermarkRecord,
};
pub use runner::SubscriberRunSummary;
pub use shift_transform::shift_to_inference;
pub use subscriber::{MeJepaShiftSubscriberConfig, ShiftSubscriber};
pub use tail::ShiftLogTail;
pub use watermark::WatermarkWriter;
