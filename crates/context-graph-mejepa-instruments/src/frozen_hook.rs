use crate::{InstrumentError, InstrumentResult, Panel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenSnapshot {
    pub name: String,
    pub element_count: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenGuard {
    snapshots: BTreeMap<String, FrozenSnapshot>,
}

impl FrozenGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_buffer(
        &mut self,
        name: impl Into<String>,
        values: &[f32],
    ) -> InstrumentResult<FrozenSnapshot> {
        let name = validate_name(name.into())?;
        validate_values(&name, values)?;
        if self.snapshots.contains_key(&name) {
            return Err(InstrumentError::invalid(
                "FrozenGuard.name",
                format!("frozen buffer {name:?} is already registered"),
                "register each frozen target buffer exactly once per guard",
            ));
        }
        let snapshot = FrozenSnapshot {
            name: name.clone(),
            element_count: values.len(),
            sha256: hash_f32s(values),
        };
        self.snapshots.insert(name, snapshot.clone());
        Ok(snapshot)
    }

    pub fn register_panel(
        &mut self,
        name: impl Into<String>,
        panel: &Panel,
    ) -> InstrumentResult<FrozenSnapshot> {
        self.register_buffer(name, panel.data())
    }

    pub fn verify_buffer(&self, name: &str, values: &[f32]) -> InstrumentResult<FrozenSnapshot> {
        let name = validate_name(name.to_string())?;
        validate_values(&name, values)?;
        let expected = self.snapshots.get(&name).ok_or_else(|| {
            InstrumentError::invalid(
                "FrozenGuard.name",
                format!("frozen buffer {name:?} was not registered"),
                "register the target buffer before training or verification",
            )
        })?;
        let actual_hash = hash_f32s(values);
        if expected.element_count != values.len() || expected.sha256 != actual_hash {
            return Err(InstrumentError::frozen_violation(
                "FrozenGuard.buffer",
                format!(
                    "frozen buffer {name:?} changed: expected len/hash {}/{}, got {}/{}",
                    expected.element_count,
                    expected.sha256,
                    values.len(),
                    actual_hash
                ),
                "stop the training step; frozen target parameters or target panel data were mutated",
            ));
        }
        Ok(expected.clone())
    }

    pub fn verify_panel(&self, name: &str, panel: &Panel) -> InstrumentResult<FrozenSnapshot> {
        self.verify_buffer(name, panel.data())
    }

    pub fn assert_no_gradient(&self, name: &str, gradient: &[f32]) -> InstrumentResult<()> {
        let name = validate_name(name.to_string())?;
        if !self.snapshots.contains_key(&name) {
            return Err(InstrumentError::invalid(
                "FrozenGuard.name",
                format!("frozen buffer {name:?} was not registered"),
                "register the target buffer before checking gradient flow",
            ));
        }
        validate_values(&name, gradient)?;
        for (idx, value) in gradient.iter().enumerate() {
            if *value != 0.0 {
                return Err(InstrumentError::frozen_violation(
                    "FrozenGuard.gradient",
                    format!("gradient[{idx}] for frozen buffer {name:?} is nonzero: {value}"),
                    "detach frozen targets and disable requires_grad before the optimizer step",
                ));
            }
        }
        Ok(())
    }

    pub fn snapshot(&self, name: &str) -> Option<&FrozenSnapshot> {
        self.snapshots.get(name)
    }

    pub fn snapshots(&self) -> &BTreeMap<String, FrozenSnapshot> {
        &self.snapshots
    }
}

fn validate_name(name: String) -> InstrumentResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(InstrumentError::invalid(
            "FrozenGuard.name",
            "frozen buffer name is empty",
            "supply a stable lowercase ASCII buffer name",
        ));
    }
    if trimmed != name {
        return Err(InstrumentError::invalid(
            "FrozenGuard.name",
            format!("frozen buffer name has surrounding whitespace: {name:?}"),
            "trim and normalize the name at the writer boundary",
        ));
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(InstrumentError::invalid(
            "FrozenGuard.name",
            format!("frozen buffer name contains invalid characters: {name:?}"),
            "use lowercase ASCII letters, digits, underscore, dash, or dot",
        ));
    }
    Ok(name)
}

fn validate_values(name: &str, values: &[f32]) -> InstrumentResult<()> {
    if values.is_empty() {
        return Err(InstrumentError::invalid(
            "FrozenGuard.values",
            format!("frozen buffer {name:?} has no values"),
            "register real target bytes; empty frozen buffers are not meaningful",
        ));
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "FrozenGuard.values",
                format!("frozen buffer {name:?} value[{idx}] is non-finite: {value}"),
                "reject NaN/Inf before registering or verifying a frozen buffer",
            ));
        }
    }
    Ok(())
}

pub fn hash_f32s(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex32(hasher.finalize().into())
}

fn hex32(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_guard_detects_mutation_and_gradient() {
        let mut guard = FrozenGuard::new();
        let original = vec![0.25, 0.5, 0.75];
        let snapshot = guard.register_buffer("target.panel", &original).unwrap();
        assert_eq!(snapshot.element_count, 3);
        assert_eq!(
            guard.verify_buffer("target.panel", &original).unwrap(),
            snapshot
        );

        let mut changed = original.clone();
        changed[1] += 0.25;
        let err = guard.verify_buffer("target.panel", &changed).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTR_FROZEN_VIOLATION");

        let err = guard
            .assert_no_gradient("target.panel", &[0.0, 0.0, 1.0])
            .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTR_FROZEN_VIOLATION");
    }

    #[test]
    fn frozen_guard_rejects_bad_registration() {
        let mut guard = FrozenGuard::new();
        assert_eq!(
            guard.register_buffer("Target", &[1.0]).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        assert_eq!(
            guard.register_buffer("target", &[]).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
