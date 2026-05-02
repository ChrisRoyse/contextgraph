use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};

pub const COUNTER_TO_GRID_DOMAIN_ID: &str = "counter_to_grid";
pub const COUNTER_TO_GRID_DOMAIN_VERSION: &str = "1.0.0";
pub const COUNTER_TO_GRID_EXPECTED_INSTRUMENT_COUNT: usize = 2;
pub const COUNTER_TO_GRID_BRIDGE_INSTRUMENT_IDS: [&str; COUNTER_TO_GRID_EXPECTED_INSTRUMENT_COUNT] =
    ["abstract_position_scalar", "abstract_action_kind_onehot"];

pub fn counter_to_grid_counter_action_kind(action_label: &str) -> DynamicJepaResult<&'static str> {
    match action_label {
        "step_up" => Ok("increment"),
        "step_down" => Ok("decrement"),
        "noop" => Ok("noop"),
        other => Err(DynamicJepaError::BridgeActionMappingIncomplete {
            source_domain: "counter_world".to_string(),
            action_label: other.to_string(),
        }),
    }
}

pub fn counter_to_grid_grid_action_kind(action_label: &str) -> DynamicJepaResult<&'static str> {
    match action_label {
        "up" | "down" => Ok("lateral"),
        "left" | "right" => Ok("perpendicular"),
        "noop" => Ok("noop"),
        other => Err(DynamicJepaError::BridgeActionMappingIncomplete {
            source_domain: "gridworld_5x5".to_string(),
            action_label: other.to_string(),
        }),
    }
}

pub fn validate_counter_to_grid_bridge_instrument_count(actual: usize) -> DynamicJepaResult<()> {
    if actual == COUNTER_TO_GRID_EXPECTED_INSTRUMENT_COUNT {
        Ok(())
    } else {
        Err(DynamicJepaError::BridgeInstrumentCountDrift {
            expected: COUNTER_TO_GRID_EXPECTED_INSTRUMENT_COUNT,
            actual,
        })
    }
}

pub fn counter_to_grid_bridge_action_effect(action_kind: &str) -> DynamicJepaResult<f32> {
    match action_kind {
        "increment" | "lateral" => Ok(1.0),
        "decrement" | "perpendicular" => Ok(-1.0),
        "noop" => Ok(0.0),
        other => Err(DynamicJepaError::BridgeActionMappingIncomplete {
            source_domain: COUNTER_TO_GRID_DOMAIN_ID.to_string(),
            action_label: other.to_string(),
        }),
    }
}
