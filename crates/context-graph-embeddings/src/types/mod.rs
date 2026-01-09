//! Core types for the 13-model embedding pipeline (Multi-Array Teleological Storage).
//!
//! All embeddings are stored as SEPARATE arrays - the 13-embedding array IS the
//! teleological vector. Per-space indexing with RRF score fusion.

mod concatenated; // Module name kept for file system, exports MultiArrayEmbedding
pub mod dimensions;
mod embedding;
mod input;
mod model_id;

pub use concatenated::MultiArrayEmbedding;
pub use embedding::ModelEmbedding;
pub use input::{ImageFormat, InputType, ModelInput};
pub use model_id::ModelId;
pub use model_id::TokenizerFamily;
