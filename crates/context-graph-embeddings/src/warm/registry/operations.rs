//! State transition operations for the warm model registry.
//!
//! Contains methods for transitioning models between states:
//! - `start_loading`: Pending -> Loading
//! - `update_progress`: Loading -> Loading (with updated progress)
//! - `mark_validating`: Loading -> Validating
//! - `mark_warm`: Validating -> Warm
//! - `mark_failed`: Loading/Validating -> Failed

use super::core::WarmModelRegistry;
use crate::warm::error::{WarmError, WarmResult};
use crate::warm::handle::ModelHandle;
use crate::warm::state::WarmModelState;

impl WarmModelRegistry {
    /// Transition a model from Pending to Loading state.
    ///
    /// Validates that the model exists and is currently in Pending state.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model to start loading
    ///
    /// # Errors
    ///
    /// - [`WarmError::ModelNotRegistered`] if model doesn't exist
    /// - [`WarmError::ModelLoadFailed`] if current state is not Pending
    ///
    /// # State Transition
    ///
    /// `Pending` -> `Loading { progress_percent: 0, bytes_loaded: 0 }`
    pub fn start_loading(&mut self, model_id: &str) -> WarmResult<()> {
        let entry =
            self.entries
                .get_mut(model_id)
                .ok_or_else(|| WarmError::ModelNotRegistered {
                    model_id: model_id.to_string(),
                })?;

        match &entry.state {
            WarmModelState::Pending => {
                entry.state = WarmModelState::Loading {
                    progress_percent: 0,
                    bytes_loaded: 0,
                };
                Ok(())
            }
            other => Err(WarmError::ModelLoadFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Invalid state transition: cannot start loading from {:?} state (expected Pending)",
                    other
                ),
                bytes_read: 0,
                file_size: entry.expected_bytes,
            }),
        }
    }

    /// Update loading progress for a model in Loading state.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model being loaded
    /// * `progress_percent` - Loading progress (0-100)
    /// * `bytes_loaded` - Number of bytes loaded so far
    ///
    /// # Errors
    ///
    /// - [`WarmError::ModelNotRegistered`] if model doesn't exist
    /// - [`WarmError::ModelLoadFailed`] if current state is not Loading
    ///
    /// # State Transition
    ///
    /// `Loading { ... }` -> `Loading { progress_percent, bytes_loaded }`
    pub fn update_progress(
        &mut self,
        model_id: &str,
        progress_percent: u8,
        bytes_loaded: usize,
    ) -> WarmResult<()> {
        let entry =
            self.entries
                .get_mut(model_id)
                .ok_or_else(|| WarmError::ModelNotRegistered {
                    model_id: model_id.to_string(),
                })?;

        match &entry.state {
            WarmModelState::Loading { .. } => {
                entry.state = WarmModelState::Loading {
                    progress_percent: progress_percent.min(100),
                    bytes_loaded,
                };
                Ok(())
            }
            other => Err(WarmError::ModelLoadFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Invalid state transition: cannot update progress in {:?} state (expected Loading)",
                    other
                ),
                bytes_read: bytes_loaded,
                file_size: entry.expected_bytes,
            }),
        }
    }

    /// Transition a model from Loading to Validating state.
    ///
    /// Called when model weights are fully loaded and validation begins.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model to validate
    ///
    /// # Errors
    ///
    /// - [`WarmError::ModelNotRegistered`] if model doesn't exist
    /// - [`WarmError::ModelValidationFailed`] if current state is not Loading
    ///
    /// # State Transition
    ///
    /// `Loading { ... }` -> `Validating`
    pub fn mark_validating(&mut self, model_id: &str) -> WarmResult<()> {
        let entry =
            self.entries
                .get_mut(model_id)
                .ok_or_else(|| WarmError::ModelNotRegistered {
                    model_id: model_id.to_string(),
                })?;

        match &entry.state {
            WarmModelState::Loading { .. } => {
                entry.state = WarmModelState::Validating;
                Ok(())
            }
            other => Err(WarmError::ModelValidationFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Invalid state transition: cannot mark validating from {:?} state (expected Loading)",
                    other
                ),
                expected_output: None,
                actual_output: None,
            }),
        }
    }

    /// Transition a model from Validating to Warm state with a VRAM handle.
    ///
    /// Called when validation succeeds and model is ready for inference.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model that is now warm
    /// * `handle` - VRAM handle for the loaded weights
    ///
    /// # Errors
    ///
    /// - [`WarmError::ModelNotRegistered`] if model doesn't exist
    /// - [`WarmError::ModelValidationFailed`] if current state is not Validating
    ///
    /// # State Transition
    ///
    /// `Validating` -> `Warm` (with handle set)
    pub fn mark_warm(&mut self, model_id: &str, handle: ModelHandle) -> WarmResult<()> {
        let entry =
            self.entries
                .get_mut(model_id)
                .ok_or_else(|| WarmError::ModelNotRegistered {
                    model_id: model_id.to_string(),
                })?;

        match &entry.state {
            WarmModelState::Validating => {
                entry.state = WarmModelState::Warm;
                entry.handle = Some(handle);
                Ok(())
            }
            other => Err(WarmError::ModelValidationFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Invalid state transition: cannot mark warm from {:?} state (expected Validating)",
                    other
                ),
                expected_output: None,
                actual_output: None,
            }),
        }
    }

    /// Transition a model to Failed state.
    ///
    /// Can be called from Loading or Validating states to record a failure.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model that failed
    /// * `error_code` - Numeric error code (e.g., 102 for load failure)
    /// * `error_message` - Human-readable error description
    ///
    /// # Errors
    ///
    /// - [`WarmError::ModelNotRegistered`] if model doesn't exist
    /// - [`WarmError::ModelLoadFailed`] if current state doesn't allow failure transition
    ///
    /// # State Transition
    ///
    /// `Loading` -> `Failed { error_code, error_message }`
    /// `Validating` -> `Failed { error_code, error_message }`
    pub fn mark_failed(
        &mut self,
        model_id: &str,
        error_code: u16,
        error_message: impl Into<String>,
    ) -> WarmResult<()> {
        let entry =
            self.entries
                .get_mut(model_id)
                .ok_or_else(|| WarmError::ModelNotRegistered {
                    model_id: model_id.to_string(),
                })?;

        match &entry.state {
            WarmModelState::Loading { .. } | WarmModelState::Validating => {
                entry.state = WarmModelState::Failed {
                    error_code,
                    error_message: error_message.into(),
                };
                entry.handle = None; // Ensure no handle on failure
                Ok(())
            }
            other => Err(WarmError::ModelLoadFailed {
                model_id: model_id.to_string(),
                reason: format!(
                    "Invalid state transition: cannot mark failed from {:?} state (expected Loading or Validating)",
                    other
                ),
                bytes_read: 0,
                file_size: entry.expected_bytes,
            }),
        }
    }
}
