use serde::{Deserialize, Serialize};

use crate::error::TctError;

pub const DEFAULT_RATE_WINDOW_SIZE_CALLS: u32 = 1_000;
pub const DEFAULT_RATE_WINDOW_HOURS: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RollingWindow {
    pub window_size_calls: u32,
    pub window_hours: u32,
}

impl Default for RollingWindow {
    fn default() -> Self {
        Self {
            window_size_calls: DEFAULT_RATE_WINDOW_SIZE_CALLS,
            window_hours: DEFAULT_RATE_WINDOW_HOURS,
        }
    }
}

impl RollingWindow {
    pub fn try_new(window_size_calls: u32, window_hours: u32) -> Result<Self, TctError> {
        if window_size_calls == 0 || window_hours == 0 {
            return Err(TctError::invalid(
                "RollingWindow",
                format!(
                    "window_size_calls and window_hours must both be positive; got {window_size_calls}/{window_hours}"
                ),
            ));
        }
        Ok(Self {
            window_size_calls,
            window_hours,
        })
    }
}
