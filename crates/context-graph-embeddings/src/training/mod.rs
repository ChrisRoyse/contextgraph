//! Causal embedder fine-tuning infrastructure.
//!
//! Transforms the static E5 causal inference system into a trainable model
//! with LLM-supervised contrastive learning.
//!
//! # Architecture
//!
//! ```text
//! LLM (Hermes 2 Pro) ──> Training Pairs ──> DataLoader
//!                                               │
//!                                         ┌─────┴─────┐
//!                                         │  Trainer   │
//!                                         │  ┌───────┐ │
//!                                         │  │ Loss:  │ │
//!                                         │  │ InfoNCE│ │
//!                                         │  │ Dir.   │ │
//!                                         │  │ Sep.   │ │
//!                                         │  │ Soft   │ │
//!                                         │  └───────┘ │
//!                                         │  ┌───────┐ │
//!                                         │  │AdamW  │ │
//!                                         │  │Optim  │ │
//!                                         │  └───────┘ │
//!                                         └─────┬─────┘
//!                                               │
//!                                    W_cause, W_effect (trained)
//! ```
//!
//! # Modules
//!
//! - [`data`]: Training pair structures and data loading
//! - [`loss`]: Contrastive + directional + separation + soft label losses
//! - [`optimizer`]: AdamW with warmup + cosine decay
//! - [`trainer`]: Training loop with momentum encoder
//! - [`evaluation`]: Directional accuracy, MRR, AUC metrics
//! - [`distillation`]: Online LLM→embedder teaching loop
//! - [`lora`]: LoRA adapters for NomicBERT attention
//! - [`multitask`]: Direction classification + mechanism prediction heads

pub mod data;
pub mod distillation;
pub mod evaluation;
pub mod lora;
pub mod loss;
pub mod multitask;
pub mod optimizer;
pub mod pipeline;
pub mod trainer;
