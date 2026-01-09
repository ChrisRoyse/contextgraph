# TASK-INTEG-003: Consciousness MCP Handlers

```xml
<task_spec id="TASK-INTEG-003" version="1.0">
<metadata>
  <title>Implement Consciousness MCP Tool Handlers</title>
  <status>todo</status>
  <layer>integration</layer>
  <sequence>23</sequence>
  <implements>
    <requirement_ref>REQ-MCP-CONSCIOUSNESS-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-INTEG-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
Consciousness handlers expose the Global Workspace Theory (GWT) and Kuramoto oscillator
state of the teleological memory system. These provide system health and synchronization
metrics.
</context>

<objective>
Implement MCP handlers for consciousness/get_state and consciousness/sync_level tools
that expose the GWT workspace and Kuramoto synchronization state.
</objective>

<rationale>
Consciousness tools enable:
1. System health monitoring via sync level
2. Attention distribution analysis
3. Coalition formation tracking
4. Phase coherence metrics for debugging
</rationale>

<input_context_files>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#5-consciousness-tools</file>
</input_context_files>

<prerequisites>
  <check>TASK-INTEG-001 complete (MemoryHandler exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement consciousness/get_state handler</item>
    <item>Implement consciousness/sync_level handler</item>
    <item>Model Global Workspace Theory state</item>
    <item>Model Kuramoto oscillator synchronization</item>
    <item>Compute attention distribution across embedders</item>
  </in_scope>
  <out_of_scope>
    <item>Full GWT implementation (simplified model)</item>
    <item>Full Kuramoto dynamics (approximation)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/consciousness.rs">
      use crate::protocol::{HandlerError, ErrorCode};

      /// Consciousness state model (GWT + Kuramoto).
      pub struct ConsciousnessState {
          workspace: GlobalWorkspace,
          kuramoto: KuramotoState,
          attention: AttentionDistribution,
      }

      /// Consciousness MCP handler.
      pub struct ConsciousnessHandler {
          state: Arc<RwLock<ConsciousnessState>>,
          search_engine: Arc<TeleologicalSearchEngine>,
      }

      impl ConsciousnessHandler {
          pub fn new(
              search_engine: Arc<TeleologicalSearchEngine>,
          ) -> Self;

          /// Handle consciousness/get_state request.
          pub async fn handle_get_state(
              &self,
              params: GetStateParams,
          ) -> Result<GetStateResponse, HandlerError>;

          /// Handle consciousness/sync_level request.
          pub async fn handle_sync_level(
              &self,
              params: SyncLevelParams,
          ) -> Result<SyncLevelResponse, HandlerError>;

          /// Update consciousness state (called periodically).
          pub async fn update_state(&self);
      }

      // Response types
      #[derive(Debug, Serialize)]
      pub struct GetStateResponse {
          pub global_workspace: GlobalWorkspaceInfo,
          pub kuramoto_state: KuramotoStateInfo,
          pub attention_distribution: AttentionInfo,
          pub consciousness_metrics: ConsciousnessMetrics,
          pub timestamp: DateTime<Utc>,
      }

      #[derive(Debug, Serialize)]
      pub struct SyncLevelResponse {
          pub sync_level: f32,
          pub phase_coherence: f32,
          pub status: String,
          pub status_interpretation: StatusInterpretation,
          pub thresholds: SyncThresholds,
          pub trend: Option<SyncTrend>,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>get_state returns full consciousness model</constraint>
    <constraint>sync_level is lightweight health check</constraint>
    <constraint>Kuramoto phases map to 13 embedders</constraint>
    <constraint>State updates periodically</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-mcp handlers::consciousness</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-mcp/src/handlers/consciousness.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::protocol::{HandlerError, ErrorCode};
use context_graph_storage::teleological::search::TeleologicalSearchEngine;
use context_graph_core::teleology::embedder::Embedder;

/// Global Workspace Theory model.
#[derive(Debug, Default)]
pub struct GlobalWorkspace {
    active_coalitions: Vec<Coalition>,
    broadcast_queue: Vec<BroadcastItem>,
    workspace_capacity: WorkspaceCapacity,
}

#[derive(Debug)]
pub struct Coalition {
    coalition_id: String,
    strength: f32,
    members: Vec<String>,
    dominant_theme: String,
}

#[derive(Debug)]
pub struct BroadcastItem {
    content_summary: String,
    priority: f32,
    awaiting_broadcast: bool,
}

#[derive(Debug, Default)]
pub struct WorkspaceCapacity {
    current: usize,
    max: usize,
    utilization: f32,
}

/// Kuramoto oscillator model for 13 embedders.
#[derive(Debug)]
pub struct KuramotoState {
    sync_level: f32,
    phase_coherence: f32,
    coupling_strength: f32,
    oscillator_count: usize,
    oscillator_phases: [f32; 13],
    order_parameter: OrderParameter,
}

#[derive(Debug, Default)]
pub struct OrderParameter {
    r: f32,
    psi: f32,
}

impl Default for KuramotoState {
    fn default() -> Self {
        // Initialize with random phases
        let mut phases = [0.0f32; 13];
        for i in 0..13 {
            phases[i] = (i as f32 * std::f32::consts::PI * 2.0) / 13.0;
        }

        Self {
            sync_level: 0.5,
            phase_coherence: 0.5,
            coupling_strength: 0.5,
            oscillator_count: 13,
            oscillator_phases: phases,
            order_parameter: OrderParameter::default(),
        }
    }
}

impl KuramotoState {
    fn compute_order_parameter(&mut self) {
        // r * e^(i*psi) = (1/N) * sum(e^(i*theta_j))
        let mut sum_cos = 0.0f32;
        let mut sum_sin = 0.0f32;

        for phase in &self.oscillator_phases {
            sum_cos += phase.cos();
            sum_sin += phase.sin();
        }

        let n = self.oscillator_count as f32;
        let r = ((sum_cos / n).powi(2) + (sum_sin / n).powi(2)).sqrt();
        let psi = (sum_sin / n).atan2(sum_cos / n);

        self.order_parameter = OrderParameter { r, psi };
        self.sync_level = r;
    }
}

/// Attention distribution across embedders.
#[derive(Debug, Default)]
pub struct AttentionDistribution {
    focused_embedders: Vec<Embedder>,
    attention_weights: [f32; 13],
    attention_entropy: f32,
}

impl AttentionDistribution {
    fn compute_entropy(&self) -> f32 {
        let mut entropy = 0.0f32;
        for &w in &self.attention_weights {
            if w > 0.0 {
                entropy -= w * w.ln();
            }
        }
        entropy
    }
}

/// Full consciousness state.
#[derive(Debug, Default)]
pub struct ConsciousnessState {
    workspace: GlobalWorkspace,
    kuramoto: KuramotoState,
    attention: AttentionDistribution,
    last_updated: Option<DateTime<Utc>>,
}

pub struct ConsciousnessHandler {
    state: Arc<RwLock<ConsciousnessState>>,
    search_engine: Arc<TeleologicalSearchEngine>,
}

impl ConsciousnessHandler {
    pub fn new(search_engine: Arc<TeleologicalSearchEngine>) -> Self {
        Self {
            state: Arc::new(RwLock::new(ConsciousnessState::default())),
            search_engine,
        }
    }

    pub async fn handle_get_state(
        &self,
        params: GetStateParams,
    ) -> Result<GetStateResponse, HandlerError> {
        // Update state if needed
        self.update_state().await;

        let state = self.state.read().await;

        // Build response
        let workspace = if params.include_workspace.unwrap_or(true) {
            Some(GlobalWorkspaceInfo {
                active_coalitions: state.workspace.active_coalitions.iter()
                    .map(|c| CoalitionInfo {
                        coalition_id: c.coalition_id.clone(),
                        strength: c.strength,
                        members: c.members.clone(),
                        dominant_theme: c.dominant_theme.clone(),
                    })
                    .collect(),
                broadcast_queue: state.workspace.broadcast_queue.iter()
                    .map(|b| BroadcastInfo {
                        content_summary: b.content_summary.clone(),
                        priority: b.priority,
                        awaiting_broadcast: b.awaiting_broadcast,
                    })
                    .collect(),
                workspace_capacity: CapacityInfo {
                    current: state.workspace.workspace_capacity.current,
                    max: state.workspace.workspace_capacity.max,
                    utilization: state.workspace.workspace_capacity.utilization,
                },
            })
        } else {
            None
        };

        let kuramoto = if params.include_oscillators.unwrap_or(true) {
            Some(KuramotoStateInfo {
                sync_level: state.kuramoto.sync_level,
                phase_coherence: state.kuramoto.phase_coherence,
                coupling_strength: state.kuramoto.coupling_strength,
                oscillator_count: state.kuramoto.oscillator_count,
                oscillator_phases: Embedder::all()
                    .map(|e| (format!("{:?}", e).to_lowercase(), state.kuramoto.oscillator_phases[e.index()]))
                    .collect(),
                order_parameter: OrderParameterInfo {
                    r: state.kuramoto.order_parameter.r,
                    psi: state.kuramoto.order_parameter.psi,
                },
            })
        } else {
            None
        };

        let attention = if params.include_attention_distribution.unwrap_or(true) {
            Some(AttentionInfo {
                focused_embedders: state.attention.focused_embedders.iter()
                    .map(|e| format!("{:?}", e).to_lowercase())
                    .collect(),
                attention_weights: Embedder::all()
                    .map(|e| (format!("{:?}", e).to_lowercase(), state.attention.attention_weights[e.index()]))
                    .collect(),
                attention_entropy: state.attention.attention_entropy,
            })
        } else {
            None
        };

        let metrics = ConsciousnessMetrics {
            integration_phi: self.compute_phi(&state),
            complexity: self.compute_complexity(&state),
            global_availability: self.compute_availability(&state),
        };

        Ok(GetStateResponse {
            global_workspace: workspace,
            kuramoto_state: kuramoto,
            attention_distribution: attention,
            consciousness_metrics: metrics,
            timestamp: Utc::now(),
        })
    }

    pub async fn handle_sync_level(
        &self,
        _params: SyncLevelParams,
    ) -> Result<SyncLevelResponse, HandlerError> {
        let state = self.state.read().await;

        let status = if state.kuramoto.sync_level >= 0.7 {
            "synchronized"
        } else if state.kuramoto.sync_level >= 0.5 {
            "partially_synchronized"
        } else if state.kuramoto.sync_level >= 0.3 {
            "desynchronizing"
        } else {
            "desynchronized"
        };

        let interpretation = StatusInterpretation {
            level: if state.kuramoto.sync_level >= 0.7 { "good" }
                   else if state.kuramoto.sync_level >= 0.5 { "warning" }
                   else { "critical" }.to_string(),
            description: self.get_status_description(&state),
            recommendation: if state.kuramoto.sync_level < 0.5 {
                Some("Consider consolidating memories to improve coherence".to_string())
            } else {
                None
            },
        };

        let thresholds = SyncThresholds {
            critical_low: 0.3,
            warning_low: 0.5,
            optimal_min: 0.7,
            current_zone: if state.kuramoto.sync_level >= 0.7 { "optimal" }
                         else if state.kuramoto.sync_level >= 0.5 { "warning" }
                         else if state.kuramoto.sync_level >= 0.3 { "low" }
                         else { "critical" }.to_string(),
        };

        Ok(SyncLevelResponse {
            sync_level: state.kuramoto.sync_level,
            phase_coherence: state.kuramoto.phase_coherence,
            status: status.to_string(),
            status_interpretation: interpretation,
            thresholds,
            trend: None, // TODO: track trend
        })
    }

    pub async fn update_state(&self) {
        let mut state = self.state.write().await;

        // Update Kuramoto phases based on recent activity
        self.update_kuramoto_phases(&mut state).await;

        // Update attention distribution
        self.update_attention(&mut state).await;

        // Update workspace coalitions
        self.update_coalitions(&mut state).await;

        state.last_updated = Some(Utc::now());
    }

    async fn update_kuramoto_phases(&self, state: &mut ConsciousnessState) {
        // Simplified: compute phases from recent search patterns
        // In reality, would use proper Kuramoto dynamics

        // Simulate phase evolution
        for i in 0..13 {
            state.kuramoto.oscillator_phases[i] += 0.1 * (i as f32 / 13.0);
            state.kuramoto.oscillator_phases[i] %= std::f32::consts::PI * 2.0;
        }

        state.kuramoto.compute_order_parameter();
        state.kuramoto.phase_coherence = state.kuramoto.sync_level;
    }

    async fn update_attention(&self, state: &mut ConsciousnessState) {
        // Update based on recent query patterns
        // Simplified: uniform attention
        for i in 0..13 {
            state.attention.attention_weights[i] = 1.0 / 13.0;
        }
        state.attention.attention_entropy = state.attention.compute_entropy();

        // Top 3 embedders
        let mut indexed: Vec<_> = Embedder::all()
            .map(|e| (e, state.attention.attention_weights[e.index()]))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        state.attention.focused_embedders = indexed.into_iter()
            .take(3)
            .map(|(e, _)| e)
            .collect();
    }

    async fn update_coalitions(&self, state: &mut ConsciousnessState) {
        // Update workspace coalitions based on recent memory access
        state.workspace.workspace_capacity = WorkspaceCapacity {
            current: 3,
            max: 7,
            utilization: 3.0 / 7.0,
        };
    }

    fn compute_phi(&self, _state: &ConsciousnessState) -> f32 {
        0.72 // Placeholder
    }

    fn compute_complexity(&self, _state: &ConsciousnessState) -> f32 {
        0.68 // Placeholder
    }

    fn compute_availability(&self, _state: &ConsciousnessState) -> f32 {
        0.85 // Placeholder
    }

    fn get_status_description(&self, state: &ConsciousnessState) -> String {
        if state.kuramoto.sync_level >= 0.7 {
            "Embedder oscillators are well synchronized".to_string()
        } else if state.kuramoto.sync_level >= 0.5 {
            "Partial synchronization - some embedders diverging".to_string()
        } else {
            "Low synchronization - system coherence degraded".to_string()
        }
    }
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct GetStateParams {
    pub include_oscillators: Option<bool>,
    pub include_workspace: Option<bool>,
    pub include_attention_distribution: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SyncLevelParams {}

#[derive(Debug, Serialize)]
pub struct GetStateResponse {
    pub global_workspace: Option<GlobalWorkspaceInfo>,
    pub kuramoto_state: Option<KuramotoStateInfo>,
    pub attention_distribution: Option<AttentionInfo>,
    pub consciousness_metrics: ConsciousnessMetrics,
    pub timestamp: DateTime<Utc>,
}

// Additional types...

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_state() {
        // Test consciousness state retrieval
    }

    #[tokio::test]
    async fn test_sync_level() {
        // Test lightweight sync check
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/consciousness.rs">
    Consciousness MCP handler implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/mod.rs">
    Add: pub mod consciousness;
  </file>
  <file path="crates/context-graph-mcp/src/handlers/core.rs">
    Add dispatch routes for consciousness/* tools
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>consciousness/get_state returns full model</criterion>
  <criterion>consciousness/sync_level returns lightweight check</criterion>
  <criterion>Kuramoto phases computed correctly</criterion>
  <criterion>Attention distribution normalized</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp handlers::consciousness -- --nocapture</command>
</test_commands>
</task_spec>
```
