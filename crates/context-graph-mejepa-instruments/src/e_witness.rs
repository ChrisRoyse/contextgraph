// E_Witness instrument — deterministic encoder over verified 73-byte witness
// chain entries. Empty or malformed chains fail closed before any vector is
// emitted, so missing provenance cannot masquerade as an all-zero slot.

use context_graph_witness::{verify_chain_bytes, WitnessEntry, HASH_SIZE, WITNESS_ENTRY_SIZE};
use serde::{Deserialize, Serialize};

use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

pub const CANONICAL_WITNESS_FORMAT_VERSION: u8 = 1;
const MAX_WITNESS_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessChainInput {
    pub format_version: u8,
    pub chain_bytes: Vec<u8>,
}

impl WitnessChainInput {
    pub fn from_entries(entries: &[WitnessEntry]) -> InstrumentResult<Self> {
        if entries.is_empty() {
            return Err(InstrumentError::invalid(
                "entries",
                "witness chain contains no entries",
                "append at least one witness entry before encoding E_Witness",
            ));
        }
        let mut chain_bytes = Vec::with_capacity(entries.len() * WITNESS_ENTRY_SIZE);
        for entry in entries {
            chain_bytes.extend_from_slice(&entry.to_bytes());
        }
        Ok(Self {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION,
            chain_bytes,
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EWitnessInstrument;

impl Instrument for EWitnessInstrument {
    type Input = WitnessChainInput;

    fn slot(&self) -> InstrumentSlot {
        InstrumentSlot::EWitness
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        validate_input(input)?;
        let verification = verify_chain_bytes(&input.chain_bytes).map_err(|err| {
            InstrumentError::invalid(
                "input.chain_bytes",
                format!(
                    "witness chain verification failed: {err}; code={}",
                    err.code()
                ),
                "repair the witness chain at the source of truth before encoding",
            )
        })?;

        let entry_count = verification.entries as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for chunk in input.chain_bytes.chunks_exact(WITNESS_ENTRY_SIZE) {
            entries.push(WitnessEntry::from_bytes(chunk).map_err(|err| {
                InstrumentError::invalid(
                    "input.chain_bytes",
                    format!("witness entry decode failed after chain verification: {err}"),
                    "report invariant drift between verify_chain_bytes and WitnessEntry::from_bytes",
                )
            })?);
        }

        let mut out = vec![0.0_f32; InstrumentSlot::EWitness.dim()];
        let inv_n = 1.0_f32 / entry_count as f32;
        let mut type_counts = [0usize; 64];
        let mut unique_types = [false; 256];
        let mut monotonic_timestamps = true;
        let mut previous_ts = entries[0].timestamp_ns;
        let first_ts = previous_ts;
        let mut last_ts = previous_ts;

        for (idx, entry) in entries.iter().enumerate() {
            let weight = (idx + 1) as f32 / entry_count as f32;
            for k in 0..HASH_SIZE {
                out[k] += normalized_byte(entry.action_hash[k]) * inv_n;
                out[32 + k] += normalized_byte(entry.prev_hash[k]) * inv_n;
                out[96 + k] += normalized_byte(entry.action_hash[k]) * weight * inv_n;
            }
            let type_idx = entry.witness_type as usize;
            type_counts[type_idx.min(63)] += 1;
            unique_types[type_idx] = true;
            if idx > 0 && entry.timestamp_ns < previous_ts {
                monotonic_timestamps = false;
            }
            previous_ts = entry.timestamp_ns;
            last_ts = entry.timestamp_ns;
        }

        for (k, byte) in verification.last_chain_hash.iter().enumerate() {
            out[64 + k] = normalized_byte(*byte);
        }
        for (k, count) in type_counts.iter().enumerate() {
            out[132 + k] = *count as f32 * inv_n;
        }

        let unique_type_count = unique_types.iter().filter(|seen| **seen).count();
        out[128] = (entry_count as f32).ln_1p() / (MAX_WITNESS_ENTRIES as f32).ln_1p();
        out[129] = unique_type_count as f32 / 256.0;
        out[130] = if monotonic_timestamps { 1.0 } else { 0.0 };
        out[131] = duration_feature(first_ts, last_ts);
        for (offset, byte) in entries[entry_count - 1]
            .action_hash
            .iter()
            .take(28)
            .enumerate()
        {
            out[196 + offset] = normalized_byte(*byte);
        }
        for (offset, byte) in entries[entry_count - 1]
            .prev_hash
            .iter()
            .take(32)
            .enumerate()
        {
            out[224 + offset] = normalized_byte(*byte);
        }

        validate_output(&out)?;
        Ok(out)
    }
}

fn validate_input(input: &WitnessChainInput) -> InstrumentResult<()> {
    if input.format_version != CANONICAL_WITNESS_FORMAT_VERSION {
        return Err(InstrumentError::invalid(
            "input.format_version",
            format!(
                "unsupported witness format version {}; expected {}",
                input.format_version, CANONICAL_WITNESS_FORMAT_VERSION
            ),
            "migrate or regenerate the witness chain using the canonical version",
        ));
    }
    if input.chain_bytes.is_empty() {
        return Err(InstrumentError::invalid(
            "input.chain_bytes",
            "witness chain byte source is empty",
            "persist at least one 73-byte witness entry before encoding",
        ));
    }
    if !input.chain_bytes.len().is_multiple_of(WITNESS_ENTRY_SIZE) {
        return Err(InstrumentError::invalid(
            "input.chain_bytes",
            format!(
                "witness chain byte length {} is not divisible by entry size {}",
                input.chain_bytes.len(),
                WITNESS_ENTRY_SIZE
            ),
            "read whole witness entries from the source of truth",
        ));
    }
    let entries = input.chain_bytes.len() / WITNESS_ENTRY_SIZE;
    if entries > MAX_WITNESS_ENTRIES {
        return Err(InstrumentError::invalid(
            "input.chain_bytes",
            format!("witness chain has {entries} entries; max supported is {MAX_WITNESS_ENTRIES}"),
            "shard very long chains before E_Witness encoding",
        ));
    }
    Ok(())
}

fn validate_output(out: &[f32]) -> InstrumentResult<()> {
    if out.len() != InstrumentSlot::EWitness.dim() {
        return Err(InstrumentError::invariant(
            "e_witness.output",
            format!(
                "E_Witness produced {} dims, expected {}",
                out.len(),
                InstrumentSlot::EWitness.dim()
            ),
            "fix EWitnessInstrument::encode layout",
        ));
    }
    for (idx, value) in out.iter().enumerate() {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                "e_witness.output",
                format!("output[{idx}] is non-finite: {value}"),
                "inspect the witness chain aggregation math",
            ));
        }
    }
    Ok(())
}

fn normalized_byte(byte: u8) -> f32 {
    byte as f32 / 255.0
}

fn duration_feature(first_ts: u64, last_ts: u64) -> f32 {
    let duration = last_ts.saturating_sub(first_ts);
    ((duration as f64 + 1.0).ln() / 1_000_000_000_f64.ln()).min(1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_witness::{shake256_32, ZERO_HASH};

    fn entry(prev: [u8; HASH_SIZE], payload: &[u8], ts: u64, typ: u8) -> WitnessEntry {
        WitnessEntry::new(prev, shake256_32(payload), ts, typ)
    }

    fn sample_chain() -> WitnessChainInput {
        let first = entry(ZERO_HASH, b"known-good-source", 100, 1);
        let second = entry(first.chain_hash(), b"oracle-pass", 200, 2);
        WitnessChainInput::from_entries(&[first, second]).unwrap()
    }

    #[test]
    fn e_witness_encodes_verified_chain() {
        let inst = EWitnessInstrument;
        let encoded = inst.encode(&sample_chain()).unwrap();
        assert_eq!(encoded.len(), InstrumentSlot::EWitness.dim());
        assert!(encoded.iter().all(|v| v.is_finite()));
        assert!(encoded.iter().any(|v| *v != 0.0));
        assert_eq!(encoded[130], 1.0);
    }

    #[test]
    fn e_witness_rejects_empty_and_truncated_chains() {
        let inst = EWitnessInstrument;
        let empty = WitnessChainInput {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION,
            chain_bytes: vec![],
        };
        assert_eq!(
            inst.encode(&empty).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        let truncated = WitnessChainInput {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION,
            chain_bytes: vec![0u8; WITNESS_ENTRY_SIZE - 1],
        };
        assert_eq!(
            inst.encode(&truncated).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn e_witness_rejects_bad_format_version_and_tamper() {
        let inst = EWitnessInstrument;
        let mut bad_version = sample_chain();
        bad_version.format_version = 2;
        assert_eq!(
            inst.encode(&bad_version).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );

        let mut tampered = sample_chain();
        tampered.chain_bytes[WITNESS_ENTRY_SIZE] ^= 0x7f;
        assert_eq!(
            inst.encode(&tampered).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn e_witness_is_deterministic_and_mutation_sensitive() {
        let inst = EWitnessInstrument;
        let a = sample_chain();
        let encoded_a = inst.encode(&a).unwrap();
        let encoded_b = inst.encode(&a).unwrap();
        assert_eq!(encoded_a, encoded_b);

        let first = entry(ZERO_HASH, b"known-good-source", 100, 1);
        let changed = entry(first.chain_hash(), b"oracle-fail", 200, 2);
        let changed_chain = WitnessChainInput::from_entries(&[first, changed]).unwrap();
        let encoded_changed = inst.encode(&changed_chain).unwrap();
        assert_ne!(encoded_a, encoded_changed);
    }
}
