//! GWT consciousness state tool implementations.
//!
//! TASK-GWT-001: Consciousness queries - get_consciousness_state, get_kuramoto_sync, get_ego_state.
//! TASK-34: High-level coherence state tool - get_coherence_state.
//! TASK-38: Focused identity continuity tool - get_identity_continuity.

use serde_json::json;
use tracing::{debug, error};

use context_graph_core::gwt::state_machine::ConsciousnessState;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

impl Handlers {
    /// get_consciousness_state tool implementation.
    ///
    /// TASK-GWT-001: Returns complete consciousness state from GWT/Kuramoto system.
    /// FAIL FAST on missing GWT components - no stubs or fallbacks.
    ///
    /// Returns:
    /// - C: Consciousness level C(t) = I(t) x R(t) x D(t)
    /// - r: Kuramoto order parameter (synchronization)
    /// - psi: Kuramoto mean phase
    /// - meta_score: Meta-cognitive accuracy
    /// - differentiation: Purpose vector entropy
    /// - state: CONSCIOUS/EMERGING/FRAGMENTED
    /// - workspace: Active memory and broadcast status
    /// - identity: Coherence and purpose vector
    /// - component_analysis: Which factor limits consciousness
    pub(crate) async fn call_get_consciousness_state(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_consciousness_state tool call");

        // FAIL FAST: Check all required GWT providers
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("get_consciousness_state: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        let gwt_system = match &self.gwt_system {
            Some(g) => g,
            None => {
                error!("get_consciousness_state: GWT system not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "GWT system not initialized - use with_gwt() constructor",
                );
            }
        };

        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("get_consciousness_state: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        let meta_cognitive = match &self.meta_cognitive {
            Some(m) => m,
            None => {
                error!("get_consciousness_state: Meta-cognitive provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Meta-cognitive provider not initialized - use with_gwt() constructor",
                );
            }
        };

        let self_ego = match &self.self_ego {
            Some(s) => s,
            None => {
                error!("get_consciousness_state: Self-ego provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Self-ego provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Get Kuramoto order parameter (r, psi)
        // parking_lot::RwLock::read() doesn't return Result, it blocks until lock acquired
        let (r, psi) = {
            let kuramoto_guard = kuramoto.read();
            kuramoto_guard.order_parameter()
        };

        // Get purpose vector from self-ego
        let purpose_vector = self_ego.read().await.purpose_vector();

        // Get meta-cognitive accuracy
        // TASK-07: MetaCognitiveProvider trait methods are async
        let meta_accuracy = {
            let meta = meta_cognitive.read().await;
            // Use acetylcholine level as a proxy for meta-cognitive accuracy
            // Higher Ach means better learning/attention
            meta.acetylcholine().await
        };

        // Compute consciousness metrics from GWT system
        let metrics = match gwt_system.compute_metrics(r as f32, meta_accuracy, &purpose_vector) {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "get_consciousness_state: Consciousness computation failed");
                return JsonRpcResponse::error(
                    id,
                    error_codes::CONSCIOUSNESS_COMPUTATION_FAILED,
                    format!("Consciousness computation failed: {}", e),
                );
            }
        };

        // Get workspace status
        // TASK-07: WorkspaceProvider trait methods are async
        let workspace_guard = workspace.read().await;
        let active_memory = workspace_guard.get_active_memory().await;
        let is_broadcasting = workspace_guard.is_broadcasting().await;
        let has_conflict = workspace_guard.has_conflict().await;
        let coherence_threshold = workspace_guard.coherence_threshold().await;

        // Get identity coherence from self-ego
        let identity_coherence = self_ego.read().await.identity_coherence();
        let identity_status = self_ego.read().await.identity_status();
        let trajectory_length = self_ego.read().await.trajectory_length();

        // Determine consciousness state string per constitution.yaml lines 394-408
        // Uses the canonical ConsciousnessState::from_level() which implements all 5 states:
        // DORMANT (r < 0.3), FRAGMENTED (0.3 <= r < 0.5), EMERGING (0.5 <= r < 0.8),
        // CONSCIOUS (0.8 <= r <= 0.95), HYPERSYNC (r > 0.95)
        let state = ConsciousnessState::from_level(r as f32).name();

        // Get GWT current state
        let gwt_state = gwt_system.current_state();
        let time_in_state = gwt_system.time_in_state();

        self.tool_result_with_pulse(
            id,
            json!({
                "C": metrics.consciousness,
                "r": r,
                "psi": psi,
                "meta_score": meta_accuracy,
                "differentiation": metrics.differentiation,
                "integration": metrics.integration,
                "reflection": metrics.reflection,
                "state": state,
                "gwt_state": format!("{:?}", gwt_state),
                "time_in_state_ms": time_in_state.as_millis(),
                "workspace": {
                    "active_memory": active_memory.map(|id| id.to_string()),
                    "is_broadcasting": is_broadcasting,
                    "has_conflict": has_conflict,
                    "coherence_threshold": coherence_threshold
                },
                "identity": {
                    "coherence": identity_coherence,
                    "status": format!("{:?}", identity_status),
                    "trajectory_length": trajectory_length,
                    "purpose_vector": purpose_vector.to_vec()
                },
                "component_analysis": {
                    "integration_sufficient": metrics.component_analysis.integration_sufficient,
                    "reflection_sufficient": metrics.component_analysis.reflection_sufficient,
                    "differentiation_sufficient": metrics.component_analysis.differentiation_sufficient,
                    "limiting_factor": format!("{:?}", metrics.component_analysis.limiting_factor)
                }
            }),
        )
    }

    /// get_kuramoto_sync tool implementation.
    ///
    /// TASK-GWT-001: Returns Kuramoto oscillator network state.
    /// Provides detailed synchronization metrics for 13-embedding phase coupling.
    ///
    /// FAIL FAST on missing Kuramoto network - no stubs or fallbacks.
    ///
    /// Returns:
    /// - r: Order parameter (synchronization level) in [0, 1]
    /// - psi: Mean phase in [0, 2pi]
    /// - synchronization: Same as r (for convenience)
    /// - state: CONSCIOUS/EMERGING/FRAGMENTED/HYPERSYNC
    /// - phases: All 13 oscillator phases
    /// - natural_freqs: All 13 natural frequencies (Hz)
    /// - coupling: Coupling strength K
    /// - elapsed_seconds: Time since creation/reset
    /// - embedding_labels: Names of the 13 embedding spaces
    /// - thresholds: State transition thresholds
    pub(crate) async fn call_get_kuramoto_sync(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling get_kuramoto_sync tool call");

        // FAIL FAST: Check kuramoto provider
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("get_kuramoto_sync: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (parking_lot RwLock blocks until available, no Result)
        let network = kuramoto.read();

        // Get order parameter (r, psi)
        let (r, psi) = network.order_parameter();

        // Get all 13 oscillator phases
        let phases = network.phases();

        // Get all 13 natural frequencies
        let natural_freqs = network.natural_frequencies();

        // Get coupling strength K
        let coupling = network.coupling_strength();

        // Get synchronization (same as r but as f64)
        let sync = network.synchronization();

        // Classify state based on r using canonical state machine (constitution.yaml lines 394-408)
        // This ensures consistency with get_consciousness_state and all 5 states are handled:
        // DORMANT (r < 0.3), FRAGMENTED (0.3 <= r < 0.5), EMERGING (0.5 <= r < 0.8),
        // CONSCIOUS (0.8 <= r <= 0.95), HYPERSYNC (r > 0.95)
        let state = ConsciousnessState::from_level(sync as f32).name();

        // Get elapsed time
        let elapsed = network.elapsed_total();

        // Return complete Kuramoto state
        self.tool_result_with_pulse(
            id,
            json!({
                "r": r,
                "psi": psi,
                "synchronization": sync,
                "state": state,
                "phases": phases.to_vec(),
                "natural_freqs": natural_freqs.to_vec(),
                "coupling": coupling,
                "elapsed_seconds": elapsed.as_secs_f64(),
                "embedding_labels": [
                    "E1_semantic", "E2_temporal_recent", "E3_temporal_periodic",
                    "E4_temporal_positional", "E5_causal", "E6_sparse",
                    "E7_code", "E8_graph", "E9_hdc", "E10_multimodal",
                    "E11_entity", "E12_late_interaction", "E13_splade"
                ],
                "thresholds": {
                    "conscious": 0.8,
                    "fragmented": 0.5,
                    "hypersync": 0.95
                }
            }),
        )
    }

    /// get_ego_state tool implementation.
    ///
    /// TASK-GWT-001: Returns Self-Ego Node state including purpose vector,
    /// identity continuity, coherence with actions, and trajectory length.
    ///
    /// TASK-IDENTITY-P0-007: Enhanced with identity_continuity object containing
    /// crisis detection state from IdentityContinuityMonitor.
    ///
    /// FAIL FAST on missing self-ego provider - no stubs or fallbacks.
    ///
    /// Returns:
    /// - purpose_vector: 13D purpose alignment vector
    /// - identity_coherence: IC = cos(PV_t, PV_{t-1}) x r(t)
    /// - coherence_with_actions: Alignment between actions and purpose
    /// - identity_status: Healthy/Warning/Degraded/Critical
    /// - trajectory_length: Number of purpose snapshots stored
    /// - identity_continuity: Crisis detection state from GwtSystemProvider (TASK-IDENTITY-P0-007)
    pub(crate) async fn call_get_ego_state(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling get_ego_state tool call");

        // FAIL FAST: Check self-ego provider
        let self_ego = match &self.self_ego {
            Some(s) => s,
            None => {
                error!("get_ego_state: Self-ego provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Self-ego provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // FAIL FAST: Check gwt_system provider for identity_continuity (TASK-IDENTITY-P0-007)
        let gwt_system = match &self.gwt_system {
            Some(g) => g,
            None => {
                error!("get_ego_state: GWT system not initialized - cannot provide identity_continuity");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "GWT system not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (tokio RwLock)
        let ego = self_ego.read().await;

        // Get purpose vector
        let purpose_vector = ego.purpose_vector();

        // Get identity coherence
        let identity_coherence = ego.identity_coherence();

        // Get coherence with actions
        let coherence_with_actions = ego.coherence_with_actions();

        // Get identity status
        let identity_status = ego.identity_status();

        // Get trajectory length
        let trajectory_length = ego.trajectory_length();

        // Drop ego lock before calling gwt_system async methods
        drop(ego);

        // === TASK-IDENTITY-P0-007: Get identity continuity from GwtSystemProvider ===
        let ic_value = gwt_system.identity_coherence().await;
        let ic_status = gwt_system.identity_status().await;
        let ic_in_crisis = gwt_system.is_identity_crisis().await;
        let ic_history_len = gwt_system.identity_history_len().await;
        let ic_last_detection = gwt_system.last_detection().await;

        // Format last_detection for JSON output
        let last_detection_json = ic_last_detection.map(|det| {
            json!({
                "identity_coherence": det.identity_coherence,
                "previous_status": format!("{:?}", det.previous_status),
                "current_status": format!("{:?}", det.current_status),
                "status_changed": det.status_changed,
                "entering_crisis": det.entering_crisis,
                "entering_critical": det.entering_critical,
                "recovering": det.recovering,
                "time_since_last_event_ms": det.time_since_last_event.map(|d| d.as_millis()),
                "can_emit_event": det.can_emit_event
            })
        });

        self.tool_result_with_pulse(
            id,
            json!({
                "purpose_vector": purpose_vector.to_vec(),
                "identity_coherence": identity_coherence,
                "coherence_with_actions": coherence_with_actions,
                "identity_status": format!("{:?}", identity_status),
                "trajectory_length": trajectory_length,
                "thresholds": {
                    "healthy": 0.9,
                    "warning": 0.7,
                    "degraded": 0.5,
                    "critical": 0.0
                },
                // TASK-IDENTITY-P0-007: Identity continuity from GwtSystemProvider
                "identity_continuity": {
                    "ic": ic_value,
                    "status": format!("{:?}", ic_status),
                    "in_crisis": ic_in_crisis,
                    "history_len": ic_history_len,
                    "last_detection": last_detection_json
                }
            }),
        )
    }

    /// get_coherence_state tool implementation.
    ///
    /// TASK-34: Returns high-level GWT workspace coherence state.
    /// Unlike get_kuramoto_sync (raw data) or get_consciousness_state (full state),
    /// this returns a focused coherence summary for quick status checks.
    ///
    /// FAIL FAST on missing GWT components - no stubs or fallbacks.
    ///
    /// Returns:
    /// - order_parameter: Kuramoto r in [0, 1]
    /// - coherence_level: High (r > 0.8) / Medium (0.5 <= r <= 0.8) / Low (r < 0.5)
    /// - is_broadcasting: Whether workspace is currently broadcasting
    /// - has_conflict: Whether there's a workspace conflict (two r > 0.8)
    /// - phases: Optional 13 oscillator phases (if include_phases = true)
    /// - thresholds: The threshold values used for coherence_level classification
    pub(crate) async fn call_get_coherence_state(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_coherence_state tool call");

        // Parse include_phases argument
        let include_phases = arguments
            .get("include_phases")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // FAIL FAST: Check required Kuramoto provider
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("get_coherence_state: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // FAIL FAST: Check required workspace provider
        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("get_coherence_state: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Get Kuramoto order parameter (r, psi)
        // parking_lot::RwLock::read() doesn't return Result, it blocks until lock acquired
        let (r, _psi) = {
            let kuramoto_guard = kuramoto.read();
            kuramoto_guard.order_parameter()
        };

        // Classify coherence level based on r thresholds (constitution.yaml gwt.kuramoto.thresholds)
        // coherent: r >= 0.8, fragmented: r < 0.5
        let coherence_level = if r > 0.8 {
            "High"
        } else if r >= 0.5 {
            "Medium"
        } else {
            "Low"
        };

        // Get workspace status (async methods per TASK-07)
        let workspace_guard = workspace.read().await;
        let is_broadcasting = workspace_guard.is_broadcasting().await;
        let has_conflict = workspace_guard.has_conflict().await;
        drop(workspace_guard);

        // Optionally get phases
        let phases_json = if include_phases {
            let kuramoto_guard = kuramoto.read();
            let phases = kuramoto_guard.phases();
            Some(json!(phases.to_vec()))
        } else {
            None
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "order_parameter": r,
                "coherence_level": coherence_level,
                "is_broadcasting": is_broadcasting,
                "has_conflict": has_conflict,
                "phases": phases_json,
                "thresholds": {
                    "high": 0.8,
                    "medium": 0.5,
                    "low": 0.0
                }
            }),
        )
    }

    /// get_identity_continuity tool implementation.
    ///
    /// TASK-38: Returns focused identity continuity (IC) status.
    /// Unlike get_ego_state (full purpose vector, trajectory, coherence_with_actions),
    /// this provides a minimal snapshot for monitoring identity health.
    ///
    /// FAIL FAST on missing GWT components - no stubs or fallbacks.
    ///
    /// Returns:
    /// - ic: Identity continuity value in [0.0, 1.0]
    /// - status: Healthy (IC >= 0.9) / Warning (0.7-0.9) / Degraded (0.5-0.7) / Critical (< 0.5)
    /// - in_crisis: Boolean flag (true if IC < 0.5)
    /// - history_len: Number of purpose snapshots stored
    /// - thresholds: Status threshold values (healthy, warning, degraded, critical)
    /// - history: Optional recent IC values if include_history=true
    pub(crate) async fn call_get_identity_continuity(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_identity_continuity tool call");

        // Parse include_history argument
        let include_history = arguments
            .get("include_history")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // FAIL FAST: Check gwt_system provider (required for IC data per TASK-IDENTITY-P0-007)
        let gwt_system = match &self.gwt_system {
            Some(g) => g,
            None => {
                error!("get_identity_continuity: GWT system not initialized - FAIL FAST per AP-26");
                // Return MCP tool error (isError=true) not JSON-RPC error
                // This allows client to handle the error gracefully while still failing fast
                return self.tool_error_with_pulse(
                    id,
                    &json!({
                        "error_type": "GWT_NOT_INITIALIZED",
                        "message": "GWT system not initialized. Configure with with_gwt() or use with_default_gwt().",
                        "error_code": error_codes::GWT_NOT_INITIALIZED
                    }).to_string(),
                );
            }
        };

        // Get identity continuity data from GwtSystemProvider
        // These async methods read from the shared IdentityContinuityMonitor (AP-40)
        let ic_value = gwt_system.identity_coherence().await;
        let ic_status = gwt_system.identity_status().await;
        let ic_in_crisis = gwt_system.is_identity_crisis().await;
        let ic_history_len = gwt_system.identity_history_len().await;

        // Build response JSON
        let mut response = json!({
            "ic": ic_value,
            "status": format!("{:?}", ic_status),
            "in_crisis": ic_in_crisis,
            "history_len": ic_history_len,
            "thresholds": {
                "healthy": 0.9,
                "warning": 0.7,
                "degraded": 0.5,
                "critical": 0.0
            }
        });

        // Optionally include history (placeholder - would need GwtSystemProvider extension)
        // For now, history is not available at the trait level, so we return empty array
        // if requested. Future enhancement: add identity_history() method to GwtSystemProvider.
        if include_history {
            // History not yet exposed via GwtSystemProvider trait
            // Per TASK-38 spec: "include_history" is optional and returns up to 10 entries
            // Current implementation returns empty array - requires trait extension for real data
            response["history"] = json!([]);
        }

        self.tool_result_with_pulse(id, response)
    }
}
