//! Neuromodulation MCP Handlers
//!
//! TASK-NEUROMOD-MCP: MCP tool handlers for neuromodulation system.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference (lines 162-206)
//!
//! Neuromodulators:
//! - Dopamine (DA): [1, 5] - Controls Hopfield beta
//! - Serotonin (5HT): [0, 1] - Scales embedding space weights E1-E13
//! - Noradrenaline (NE): [0.5, 2] - Controls attention temperature
//! - Acetylcholine (ACh): [0.001, 0.002] - UTL learning rate (READ-ONLY, GWT-managed)
//!
//! ## Tools
//!
//! - get_neuromodulation_state: Get all neuromodulator levels and their effects
//! - adjust_neuromodulator: Adjust a specific neuromodulator level (ACh is read-only)

use serde_json::json;
use tracing::{debug, error, warn};

use context_graph_core::neuromod::{ModulatorType, NeuromodulationManager};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

/// Default ACh value when GWT is not available (baseline)
const DEFAULT_ACH: f32 = 0.001;

impl Handlers {
    /// get_neuromodulation_state tool implementation.
    ///
    /// TASK-NEUROMOD-MCP: Get complete neuromodulation system state.
    /// FAIL FAST if NeuromodulationManager not initialized.
    ///
    /// Returns:
    /// - dopamine: object - DA level, range, effect on Hopfield beta
    /// - serotonin: object - 5HT level, range, effect on space weights
    /// - noradrenaline: object - NE level, range, effect on attention temp
    /// - acetylcholine: object - ACh level (read-only), range, effect on UTL lr
    /// - system_state: string - Overall neuromodulation state
    pub(super) async fn call_get_neuromodulation_state(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_neuromodulation_state tool call");

        // FAIL FAST: Check neuromod manager
        let neuromod_manager = match &self.neuromod_manager {
            Some(nm) => nm,
            None => {
                error!("get_neuromodulation_state: NeuromodulationManager not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::NEUROMOD_NOT_INITIALIZED,
                    "NeuromodulationManager not initialized - use with_neuromod() constructor",
                );
            }
        };

        // Get ACh from GWT meta-cognitive if available
        // Use async read since MetaCognitiveProvider trait methods are async (TASK-07)
        let (ach_value, ach_source) = if let Some(mc) = &self.meta_cognitive {
            let guard = mc.read().await;
            (guard.acetylcholine().await, "gwt")
        } else {
            (DEFAULT_ACH, "default_no_gwt")
        };

        // Get full neuromodulation state
        let state = {
            let manager = neuromod_manager.read();
            manager.get_state(ach_value)
        };

        // Get ranges for each modulator (static function)
        let da_range = NeuromodulationManager::get_range(ModulatorType::Dopamine);
        let serotonin_range = NeuromodulationManager::get_range(ModulatorType::Serotonin);
        let ne_range = NeuromodulationManager::get_range(ModulatorType::Noradrenaline);
        let ach_range = NeuromodulationManager::get_range(ModulatorType::Acetylcholine);

        // Build response using the Level structs' .value field
        // Note: Each Level type has different metadata fields
        self.tool_result_with_pulse(
            id,
            json!({
                "dopamine": {
                    "level": state.dopamine.value,
                    "last_trigger": state.dopamine.last_trigger.map(|t| t.to_rfc3339()),
                    "range": {
                        "min": da_range.0,
                        "baseline": da_range.1,
                        "max": da_range.2
                    },
                    "parameter": "hopfield.beta",
                    "trigger": "memory_enters_workspace",
                    "effect": "Higher DA increases Hopfield network pattern completion strength"
                },
                "serotonin": {
                    "level": state.serotonin.value,
                    "space_weights": state.serotonin.space_weights.to_vec(),
                    "range": {
                        "min": serotonin_range.0,
                        "baseline": serotonin_range.1,
                        "max": serotonin_range.2
                    },
                    "parameter": "space_weights",
                    "effect": "5HT scales embedding space weights E1-E13 for mood-congruent retrieval"
                },
                "noradrenaline": {
                    "level": state.noradrenaline.value,
                    "last_threat": state.noradrenaline.last_threat.map(|t| t.to_rfc3339()),
                    "threat_count": state.noradrenaline.threat_count,
                    "range": {
                        "min": ne_range.0,
                        "baseline": ne_range.1,
                        "max": ne_range.2
                    },
                    "parameter": "attention.temp",
                    "trigger": "threat_detection",
                    "effect": "Higher NE sharpens attention focus (lower temperature)"
                },
                "acetylcholine": {
                    "level": state.acetylcholine,
                    "range": {
                        "min": ach_range.0,
                        "baseline": ach_range.1,
                        "max": ach_range.2
                    },
                    "parameter": "utl.lr",
                    "trigger": "meta_cognitive.dream",
                    "effect": "ACh controls UTL learning rate - higher during dreams",
                    "read_only": true,
                    "managed_by": "GWT MetaCognitiveLoop",
                    "source": ach_source
                },
                "derived_parameters": {
                    "hopfield_beta": state.hopfield_beta(),
                    "attention_temp": state.attention_temp(),
                    "utl_learning_rate": state.utl_learning_rate(),
                    "is_alert": state.is_alert(),
                    "is_learning_elevated": state.is_learning_elevated()
                },
                "constitution_reference": {
                    "dopamine": { "range": "[1, 5]", "parameter": "hopfield.beta" },
                    "serotonin": { "range": "[0, 1]", "parameter": "space_weights" },
                    "noradrenaline": { "range": "[0.5, 2]", "parameter": "attention.temp" },
                    "acetylcholine": { "range": "[0.001, 0.002]", "parameter": "utl.lr" }
                }
            }),
        )
    }

    /// adjust_neuromodulator tool implementation.
    ///
    /// TASK-NEUROMOD-MCP: Adjust a specific neuromodulator level.
    /// FAIL FAST if NeuromodulationManager not initialized.
    /// ACh is READ-ONLY (managed by GWT) - returns error if attempted.
    ///
    /// Arguments:
    /// - modulator: string - One of: "dopamine", "serotonin", "noradrenaline"
    /// - delta: f32 - Amount to adjust (positive or negative)
    ///
    /// Returns:
    /// - modulator: string - Which modulator was adjusted
    /// - old_level: f32 - Level before adjustment
    /// - new_level: f32 - Level after adjustment (clamped to range)
    /// - delta_applied: f32 - Actual delta applied (may differ due to clamping)
    /// - clamped: bool - Whether the value was clamped to range
    pub(super) async fn call_adjust_neuromodulator(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling adjust_neuromodulator tool call");

        // FAIL FAST: Check neuromod manager
        let neuromod_manager = match &self.neuromod_manager {
            Some(nm) => nm,
            None => {
                error!("adjust_neuromodulator: NeuromodulationManager not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::NEUROMOD_NOT_INITIALIZED,
                    "NeuromodulationManager not initialized - use with_neuromod() constructor",
                );
            }
        };

        // Parse modulator type (required)
        let modulator_str = match args.get("modulator").and_then(|v| v.as_str()) {
            Some(s) => s.to_lowercase(),
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'modulator' parameter");
            }
        };

        let modulator_type = match modulator_str.as_str() {
            "dopamine" | "da" => ModulatorType::Dopamine,
            "serotonin" | "5ht" | "5-ht" => ModulatorType::Serotonin,
            "noradrenaline" | "norepinephrine" | "ne" => ModulatorType::Noradrenaline,
            "acetylcholine" | "ach" => {
                // ACh is read-only - managed by GWT
                warn!("adjust_neuromodulator: Attempted to adjust ACh (read-only)");
                return JsonRpcResponse::error(
                    id,
                    error_codes::NEUROMOD_ACH_READ_ONLY,
                    "Acetylcholine (ACh) is read-only and managed by GWT MetaCognitiveLoop. \
                     Cannot be adjusted directly. See constitution.yaml:neuromod.Acetylcholine",
                );
            }
            other => {
                return self.tool_error_with_pulse(
                    id,
                    &format!(
                        "Invalid modulator '{}'. Valid options: dopamine, serotonin, noradrenaline",
                        other
                    ),
                );
            }
        };

        // Parse delta (required)
        let delta = match args.get("delta").and_then(|v| v.as_f64()) {
            Some(d) => d as f32,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'delta' parameter");
            }
        };

        // Get old level
        let old_level = {
            let manager = neuromod_manager.read();
            manager.get(modulator_type).unwrap_or(0.0)
        };

        // Get range for this modulator
        let range = NeuromodulationManager::get_range(modulator_type);

        // Apply adjustment
        let adjust_result = {
            let mut manager = neuromod_manager.write();
            manager.adjust(modulator_type, delta)
        };

        match adjust_result {
            Ok(new_level) => {
                let actual_delta = new_level - old_level;
                let clamped = (actual_delta - delta).abs() > 0.0001;

                if clamped {
                    debug!(
                        "adjust_neuromodulator: {} clamped from {} to {} (requested delta: {}, actual: {})",
                        modulator_str, old_level, new_level, delta, actual_delta
                    );
                }

                self.tool_result_with_pulse(
                    id,
                    json!({
                        "modulator": modulator_str,
                        "old_level": old_level,
                        "new_level": new_level,
                        "delta_requested": delta,
                        "delta_applied": actual_delta,
                        "clamped": clamped,
                        "range": {
                            "min": range.0,
                            "baseline": range.1,
                            "max": range.2
                        }
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, modulator = %modulator_str, "adjust_neuromodulator: Adjustment failed");
                JsonRpcResponse::error(
                    id,
                    error_codes::NEUROMOD_ADJUSTMENT_ERROR,
                    format!("Failed to adjust {}: {}", modulator_str, e),
                )
            }
        }
    }
}
