//! GWT workspace tool implementations.
//!
//! TASK-GWT-001: Workspace operations - get_workspace_status, trigger_workspace_broadcast, adjust_coupling.

use serde_json::json;
use tracing::{debug, error};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;

impl Handlers {
    /// get_workspace_status tool implementation.
    ///
    /// TASK-GWT-001: Returns Global Workspace status including active memory,
    /// competing candidates, broadcast state, and coherence threshold.
    ///
    /// FAIL FAST on missing workspace provider - no stubs or fallbacks.
    ///
    /// Returns:
    /// - active_memory: UUID of currently active (conscious) memory, or null
    /// - is_broadcasting: Whether broadcast window is active
    /// - has_conflict: Whether multiple memories compete (r > 0.8)
    /// - coherence_threshold: Threshold for workspace entry (default 0.8)
    /// - conflict_memories: List of conflicting memory UUIDs if has_conflict
    pub(crate) async fn call_get_workspace_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling get_workspace_status tool call");

        // FAIL FAST: Check workspace provider
        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("get_workspace_status: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (tokio RwLock)
        // TASK-07: WorkspaceProvider trait methods are async
        let ws = workspace.read().await;

        // Get active memory if broadcasting
        let active_memory = ws.get_active_memory().await;

        // Check broadcast state
        let is_broadcasting = ws.is_broadcasting().await;

        // Check for conflict
        let has_conflict = ws.has_conflict().await;

        // Get coherence threshold
        let coherence_threshold = ws.coherence_threshold().await;

        // Get conflict details if present
        let conflict_memories = ws.get_conflict_details().await;

        self.tool_result_with_pulse(
            id,
            json!({
                "active_memory": active_memory.map(|id| id.to_string()),
                "is_broadcasting": is_broadcasting,
                "has_conflict": has_conflict,
                "coherence_threshold": coherence_threshold,
                "conflict_memories": conflict_memories.map(|ids|
                    ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
                ),
                "broadcast_duration_ms": 100 // Constitution default
            }),
        )
    }

    /// trigger_workspace_broadcast tool implementation.
    ///
    /// TASK-GWT-001: Triggers winner-take-all selection with a specific memory.
    /// Forces memory into workspace competition. Requires write lock on workspace.
    ///
    /// FAIL FAST on missing providers - no stubs or fallbacks.
    ///
    /// Arguments:
    /// - memory_id: UUID of memory to broadcast
    /// - importance: Importance score [0,1] (default 0.8)
    /// - alignment: North star alignment [0,1] (default 0.8)
    /// - force: Force broadcast even if below coherence threshold
    ///
    /// Returns:
    /// - success: Whether broadcast was successful
    /// - memory_id: UUID of the memory
    /// - new_r: Current Kuramoto order parameter
    /// - was_selected: Whether this memory won WTA selection
    pub(crate) async fn call_trigger_workspace_broadcast(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling trigger_workspace_broadcast tool call");

        // FAIL FAST: Check workspace provider
        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("trigger_workspace_broadcast: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // FAIL FAST: Check kuramoto provider (needed for order parameter)
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("trigger_workspace_broadcast: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Parse memory_id (required)
        let memory_id_str = match args.get("memory_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'memory_id' parameter");
            }
        };

        let memory_id = match uuid::Uuid::parse_str(memory_id_str) {
            Ok(id) => id,
            Err(e) => {
                return self.tool_error_with_pulse(
                    id,
                    &format!("Invalid UUID format for memory_id: {}", e),
                );
            }
        };

        // Parse optional parameters
        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;
        let alignment = args
            .get("alignment")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;
        let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

        // Get current order parameter from Kuramoto network
        let r = {
            let kuramoto_guard = kuramoto.read();
            kuramoto_guard.synchronization() as f32
        };

        // Check if memory qualifies for workspace (r >= 0.8 unless forced)
        let coherence_threshold = 0.8;
        if r < coherence_threshold && !force {
            return self.tool_result_with_pulse(
                id,
                json!({
                    "success": false,
                    "memory_id": memory_id.to_string(),
                    "new_r": r,
                    "was_selected": false,
                    "reason": format!(
                        "Order parameter r={:.3} below coherence threshold {}. Use force=true to override.",
                        r, coherence_threshold
                    )
                }),
            );
        }

        // Acquire write lock and trigger selection
        let ws = workspace.write().await;

        // Create candidate and trigger WTA selection
        let candidates = vec![(memory_id, r, importance, alignment)];
        let winner = match ws.select_winning_memory(candidates).await {
            Ok(w) => w,
            Err(e) => {
                error!(error = %e, "trigger_workspace_broadcast: WTA selection failed");
                return JsonRpcResponse::error(
                    id,
                    error_codes::WORKSPACE_ERROR,
                    format!("Workspace selection failed: {}", e),
                );
            }
        };

        let was_selected = winner == Some(memory_id);

        // GAP-1 FIX: Wire workspace events to neuromodulation
        // When a memory enters workspace (was_selected), increase dopamine
        let dopamine_triggered = if was_selected {
            if let Some(neuromod) = &self.neuromod_manager {
                let mut manager = neuromod.write();
                manager.on_workspace_entry();
                let new_dopamine = manager.get_hopfield_beta();
                debug!(
                    memory_id = %memory_id,
                    dopamine = new_dopamine,
                    "Workspace entry triggered dopamine increase"
                );
                Some(new_dopamine)
            } else {
                None
            }
        } else {
            None
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "memory_id": memory_id.to_string(),
                "new_r": r,
                "was_selected": was_selected,
                "is_broadcasting": ws.is_broadcasting().await,
                "dopamine_triggered": dopamine_triggered
            }),
        )
    }

    /// adjust_coupling tool implementation.
    ///
    /// TASK-GWT-001: Adjusts Kuramoto oscillator network coupling strength K.
    /// Higher K leads to faster synchronization. K is clamped to [0, 10].
    ///
    /// FAIL FAST on missing kuramoto provider - no stubs or fallbacks.
    ///
    /// Arguments:
    /// - new_K: New coupling strength (clamped to [0, 10])
    ///
    /// Returns:
    /// - old_K: Previous coupling strength
    /// - new_K: New coupling strength (after clamping)
    /// - predicted_r: Predicted order parameter after adjustment
    pub(crate) async fn call_adjust_coupling(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling adjust_coupling tool call");

        // FAIL FAST: Check kuramoto provider
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("adjust_coupling: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Parse new_K (required)
        let new_k = match args.get("new_K").and_then(|v| v.as_f64()) {
            Some(k) => k,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'new_K' parameter");
            }
        };

        // Acquire write lock (parking_lot RwLock)
        let mut kuramoto_guard = kuramoto.write();

        // Get old coupling strength
        let old_k = kuramoto_guard.coupling_strength();

        // Set new coupling strength (will be clamped internally to [0, 10])
        kuramoto_guard.set_coupling_strength(new_k);

        // Get the actual new K (after clamping)
        let actual_new_k = kuramoto_guard.coupling_strength();

        // Get current synchronization for prediction
        let current_r = kuramoto_guard.synchronization();

        // Simple prediction: higher K tends to increase r
        // This is a rough approximation based on Kuramoto dynamics
        let predicted_r = if actual_new_k > old_k {
            // Increasing K tends to increase r
            (current_r + 0.1 * (actual_new_k - old_k)).min(1.0)
        } else {
            // Decreasing K may decrease r
            (current_r - 0.05 * (old_k - actual_new_k)).max(0.0)
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "old_K": old_k,
                "new_K": actual_new_k,
                "predicted_r": predicted_r,
                "current_r": current_r,
                "K_clamped": new_k != actual_new_k
            }),
        )
    }

    // ========== NORTH STAR TOOLS ==========
    // Real implementations are in handlers/north_star.rs (TASK-NORTHSTAR-001)
}
