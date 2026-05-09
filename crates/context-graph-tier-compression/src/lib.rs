// Clean-room 8/7/5/3-bit groupwise linear scalar quantization for tiered
// cold storage. Inspired by ruvnet/RuVector
// `crates/ruvector-temporal-tensor/src/`. Per
// `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
// — read for ideas only, no upstream code copied.
//
// Per `docs/ruvectorfindings/05_QUICK_WINS_VS_DEEP_INTEGRATION.md` Tier 1.3
// and `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §6.2` (tiered
// storage policy):
//
//   - HOT tier:   raw `f32` (no compression).
//   - WARM tier:  8-bit linear scalar quantization (4× compression).
//   - COOL tier:  5-bit linear scalar quantization (~6.4× compression).
//   - COLD tier:  3-bit linear scalar quantization (~10.7× compression).
//   - 7-bit:      intermediate tier (~4.57× compression).
//
// Algorithm: per-vector min / max + uniform N-level quantization. For each
// value `v` and level count `L = 2^bits`:
//   q   = round((v - min) / (max - min) * (L - 1))    // ∈ [0, L-1]
//   v̂  = min + (q / (L - 1)) * (max - min)
// Reconstruction error per element is bounded by (max - min) / (2*(L-1))
// (half the bin width).
//
// Sub-byte bit packing: for 7/5/3-bit widths, values are packed contiguously
// LSB-first into a byte stream of `ceil(n * bits / 8)` bytes. The trailing
// byte is zero-padded; the n stored in the header determines how many values
// to read back.
//
// On-disk format (`serialize` / `deserialize`):
//
//   bytes 0..4   = magic "CGTC" (0x43 0x47 0x54 0x43)
//   byte  4      = format version (currently 1)
//   byte  5      = raw bit width (one of 8, 7, 5, 3)
//   bytes 6..14  = n as u64 little-endian
//   bytes 14..18 = min as f32 little-endian
//   bytes 18..22 = max as f32 little-endian
//   bytes 22..   = packed quantized payload
//
// Header is 22 bytes. Failure modes are explicit:
//   TIER_COMPRESSION_INVALID_INPUT     — empty input, non-finite values,
//                                         truncated blob, wrong magic, wrong
//                                         bit width on the wire.
//   TIER_COMPRESSION_NUMERICAL_INVARIANT — min > max in a deserialized blob.
//
// No fallback. No silent recovery. Every error has a `code()` and a
// remediation hint.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type TierCompressionResult<T> = Result<T, TierCompressionError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum TierCompressionError {
    #[error("tier compression invalid input at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "tier compression numerical invariant failed at {field}: {message}; remediation: {remediation}"
    )]
    NumericalInvariant {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
}

impl TierCompressionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "TIER_COMPRESSION_INVALID_INPUT",
            Self::NumericalInvariant { .. } => "TIER_COMPRESSION_NUMERICAL_INVARIANT",
        }
    }

    fn invalid(field: &'static str, message: impl Into<String>, remediation: &'static str) -> Self {
        Self::InvalidInput {
            field,
            message: message.into(),
            remediation,
        }
    }

    fn invariant(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::NumericalInvariant {
            field,
            message: message.into(),
            remediation,
        }
    }
}

/// Supported bit widths for groupwise linear scalar quantization. Mapped
/// directly to doc 05 Tier 1.3's "8/7/5/3-bit tier compressor" choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BitWidth {
    Eight,
    Seven,
    Five,
    Three,
}

impl BitWidth {
    pub fn bits(&self) -> u32 {
        match self {
            Self::Eight => 8,
            Self::Seven => 7,
            Self::Five => 5,
            Self::Three => 3,
        }
    }

    /// Number of distinct quantization levels (2^bits).
    pub fn levels(&self) -> u32 {
        1u32 << self.bits()
    }

    /// All four widths in canonical doc 05 order (largest first).
    pub fn all() -> [Self; 4] {
        [Self::Eight, Self::Seven, Self::Five, Self::Three]
    }

    fn from_raw(raw: u8) -> TierCompressionResult<Self> {
        match raw {
            8 => Ok(Self::Eight),
            7 => Ok(Self::Seven),
            5 => Ok(Self::Five),
            3 => Ok(Self::Three),
            other => Err(TierCompressionError::invalid(
                "blob.bits",
                format!("unsupported bit width on the wire: {other}; supported are 8, 7, 5, 3"),
                "use one of BitWidth::Eight, Seven, Five, Three",
            )),
        }
    }
}

/// In-memory representation of a quantized vector blob.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompressedBlob {
    pub bits: BitWidth,
    pub n: usize,
    pub min: f32,
    pub max: f32,
    pub data: Vec<u8>,
}

impl CompressedBlob {
    /// Bit-width (1) + version (1) + n (8) + min (4) + max (4) + magic (4) = 22.
    pub const HEADER_LEN: usize = 22;
    pub const MAGIC: [u8; 4] = *b"CGTC";
    pub const VERSION: u8 = 1;

    /// Number of bytes the packed payload occupies, or `None` if `n * bits`
    /// would overflow `usize`. Callers that constructed the blob via
    /// `encode` (where `n = values.len()`) can `.expect()` safely; callers
    /// that received the blob via `deserialize` MUST propagate `None` as an
    /// invalid-input error to avoid silent truncation on attacker-controlled
    /// `n` values.
    pub fn payload_len(&self) -> Option<usize> {
        packed_len(self.n, self.bits.bits())
    }

    /// Total on-disk size of the serialized blob (header + payload), or
    /// `None` on overflow.
    pub fn serialized_len(&self) -> Option<usize> {
        self.payload_len()
            .and_then(|p| p.checked_add(Self::HEADER_LEN))
    }
}

/// Quantize `values` at the given bit width.
pub fn encode(values: &[f32], bits: BitWidth) -> TierCompressionResult<CompressedBlob> {
    if values.is_empty() {
        return Err(TierCompressionError::invalid(
            "values",
            "cannot encode an empty vector",
            "supply at least one value",
        ));
    }
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(TierCompressionError::invalid(
                "values",
                format!("values[{i}] is non-finite: {v}"),
                "scrub NaN/Inf before encoding",
            ));
        }
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for v in values {
        if *v < min {
            min = *v;
        }
        if *v > max {
            max = *v;
        }
    }
    let n_bits = bits.bits();
    let levels = bits.levels();
    let max_q = (levels - 1) as f64;
    let range = (max as f64) - (min as f64);

    // values.len() is bounded by an in-memory slice, so packed_len cannot
    // overflow here — `.expect()` is safe.
    let mut data = Vec::with_capacity(
        packed_len(values.len(), n_bits).expect("packed_len internal overflow on values.len()"),
    );
    let mut acc: u64 = 0;
    let mut bits_in_acc: u32 = 0;

    for v in values {
        let q: u32 = if range == 0.0 {
            0
        } else {
            let normalized = ((*v as f64) - (min as f64)) / range * max_q;
            normalized.round().clamp(0.0, max_q) as u32
        };
        // Append `n_bits` LSBs of `q` to the accumulator at offset `bits_in_acc`.
        acc |= (q as u64) << bits_in_acc;
        bits_in_acc += n_bits;
        while bits_in_acc >= 8 {
            data.push((acc & 0xFF) as u8);
            acc >>= 8;
            bits_in_acc -= 8;
        }
    }
    if bits_in_acc > 0 {
        data.push((acc & 0xFF) as u8);
    }
    debug_assert_eq!(
        data.len(),
        packed_len(values.len(), n_bits).expect("packed_len internal overflow on values.len()")
    );

    Ok(CompressedBlob {
        bits,
        n: values.len(),
        min,
        max,
        data,
    })
}

/// Reconstruct a vector from its quantized blob.
pub fn decode(blob: &CompressedBlob) -> TierCompressionResult<Vec<f32>> {
    if !blob.min.is_finite() || !blob.max.is_finite() {
        return Err(TierCompressionError::invariant(
            "blob.min/max",
            format!(
                "non-finite range bounds: min={}, max={}",
                blob.min, blob.max
            ),
            "rebuild the blob with finite bounds",
        ));
    }
    if blob.min > blob.max {
        return Err(TierCompressionError::invariant(
            "blob.min/max",
            format!("min ({}) > max ({})", blob.min, blob.max),
            "rebuild the blob; min/max may have been corrupted",
        ));
    }
    let expected = packed_len(blob.n, blob.bits.bits()).ok_or_else(|| {
        TierCompressionError::invalid(
            "blob.n",
            format!(
                "blob.n = {} overflows usize when multiplied by {} bits",
                blob.n,
                blob.bits.bits()
            ),
            "supply a blob whose n*bits fits in usize",
        )
    })?;
    if blob.data.len() != expected {
        return Err(TierCompressionError::invalid(
            "blob.data",
            format!(
                "blob.data length mismatch: have {} bytes, need exactly {} for n={} at {} bits",
                blob.data.len(),
                expected,
                blob.n,
                blob.bits.bits()
            ),
            "decode only canonical blobs produced by encode or deserialize",
        ));
    }
    let n_bits = blob.bits.bits();
    let levels = blob.bits.levels();
    let max_q = (levels - 1) as f64;
    let range = (blob.max as f64) - (blob.min as f64);
    let mask: u64 = if n_bits == 64 {
        !0
    } else {
        (1u64 << n_bits) - 1
    };

    let mut out = Vec::with_capacity(blob.n);
    let mut acc: u64 = 0;
    let mut bits_in_acc: u32 = 0;
    let mut byte_idx: usize = 0;

    for _ in 0..blob.n {
        while bits_in_acc < n_bits {
            if byte_idx >= blob.data.len() {
                return Err(TierCompressionError::invalid(
                    "blob.data",
                    "ran out of payload bytes before all values were decoded",
                    "ensure blob.data is at least packed_len(n, bits) bytes",
                ));
            }
            acc |= (blob.data[byte_idx] as u64) << bits_in_acc;
            bits_in_acc += 8;
            byte_idx += 1;
        }
        let q = (acc & mask) as u32;
        acc >>= n_bits;
        bits_in_acc -= n_bits;
        let v_hat = if range == 0.0 {
            blob.min
        } else {
            ((blob.min as f64) + (q as f64 / max_q) * range) as f32
        };
        out.push(v_hat);
    }
    Ok(out)
}

/// Serialize a `CompressedBlob` to its canonical on-disk byte layout. See
/// the module-level docs for the exact header format.
pub fn serialize(blob: &CompressedBlob) -> TierCompressionResult<Vec<u8>> {
    if !blob.min.is_finite() || !blob.max.is_finite() {
        return Err(TierCompressionError::invariant(
            "blob.min/max",
            format!(
                "non-finite range bounds: min={}, max={}",
                blob.min, blob.max
            ),
            "rebuild the blob with finite bounds before serializing",
        ));
    }
    if blob.min > blob.max {
        return Err(TierCompressionError::invariant(
            "blob.min/max",
            format!("min ({}) > max ({})", blob.min, blob.max),
            "rebuild the blob; min/max may have been corrupted",
        ));
    }
    let payload_len = blob.payload_len().ok_or_else(|| {
        TierCompressionError::invalid(
            "blob.n",
            format!(
                "blob.n = {} overflows usize when multiplied by {} bits",
                blob.n,
                blob.bits.bits()
            ),
            "supply a blob whose n*bits fits in usize",
        )
    })?;
    if blob.data.len() != payload_len {
        return Err(TierCompressionError::invalid(
            "blob.data",
            format!(
                "payload length must be exactly {} bytes for n={} at {} bits; got {}",
                payload_len,
                blob.n,
                blob.bits.bits(),
                blob.data.len()
            ),
            "serialize only canonical blobs produced by encode or deserialize",
        ));
    }
    let mut out = Vec::with_capacity(
        payload_len
            .checked_add(CompressedBlob::HEADER_LEN)
            .ok_or_else(|| {
                TierCompressionError::invalid(
                    "blob.n",
                    "serialized length overflowed usize",
                    "supply a smaller blob",
                )
            })?,
    );
    out.extend_from_slice(&CompressedBlob::MAGIC);
    out.push(CompressedBlob::VERSION);
    out.push(blob.bits.bits() as u8);
    out.extend_from_slice(&(blob.n as u64).to_le_bytes());
    out.extend_from_slice(&blob.min.to_le_bytes());
    out.extend_from_slice(&blob.max.to_le_bytes());
    out.extend_from_slice(&blob.data);
    Ok(out)
}

/// Parse a byte stream back into a `CompressedBlob`.
pub fn deserialize(bytes: &[u8]) -> TierCompressionResult<CompressedBlob> {
    if bytes.len() < CompressedBlob::HEADER_LEN {
        return Err(TierCompressionError::invalid(
            "bytes",
            format!(
                "byte stream too short: {} < {} (header)",
                bytes.len(),
                CompressedBlob::HEADER_LEN
            ),
            "do not truncate the header",
        ));
    }
    if bytes[0..4] != CompressedBlob::MAGIC {
        return Err(TierCompressionError::invalid(
            "bytes.magic",
            format!(
                "wrong magic bytes: expected {:?}, got {:?}",
                CompressedBlob::MAGIC,
                &bytes[0..4]
            ),
            "ensure you are deserializing a context-graph-tier-compression blob",
        ));
    }
    let version = bytes[4];
    if version != CompressedBlob::VERSION {
        return Err(TierCompressionError::invalid(
            "bytes.version",
            format!(
                "unsupported version: got {}, this build supports {}",
                version,
                CompressedBlob::VERSION
            ),
            "regenerate the blob with the current writer or update the reader",
        ));
    }
    let bits = BitWidth::from_raw(bytes[5])?;
    let n_le: [u8; 8] = bytes[6..14].try_into().expect("slice length 8");
    let n_u64 = u64::from_le_bytes(n_le);
    let n = usize::try_from(n_u64).map_err(|_| {
        TierCompressionError::invalid(
            "bytes.n",
            format!("on-disk n = {n_u64} exceeds usize::MAX on this host; cannot decode"),
            "regenerate the blob on a host where n fits in usize",
        )
    })?;
    let min_le: [u8; 4] = bytes[14..18].try_into().expect("slice length 4");
    let min = f32::from_le_bytes(min_le);
    let max_le: [u8; 4] = bytes[18..22].try_into().expect("slice length 4");
    let max = f32::from_le_bytes(max_le);

    let payload = &bytes[CompressedBlob::HEADER_LEN..];
    let expected = packed_len(n, bits.bits()).ok_or_else(|| {
        TierCompressionError::invalid(
            "bytes.n",
            format!(
                "on-disk n = {} overflows usize when multiplied by {} bits",
                n,
                bits.bits()
            ),
            "supply a blob whose n*bits fits in usize",
        )
    })?;
    if payload.len() != expected {
        return Err(TierCompressionError::invalid(
            "bytes.payload",
            format!(
                "payload length mismatch: have {} bytes, need exactly {} for n={} at {} bits",
                payload.len(),
                expected,
                n,
                bits.bits()
            ),
            "ensure the on-disk blob is exactly one canonical serialized blob",
        ));
    }
    let data = payload.to_vec();
    Ok(CompressedBlob {
        bits,
        n,
        min,
        max,
        data,
    })
}

/// Number of payload bytes for `n` values at `bits` bits each
/// (= ceil(n * bits / 8)). Returns `None` on usize overflow so callers can
/// fail-closed against attacker- or disk-controlled `n`. The four supported
/// bit widths only multiply by 3..=8, so overflow happens only at
/// `n` close to `usize::MAX`, but a corrupted on-disk header could supply
/// such a value and silent wrap would defeat the truncation check in
/// `deserialize`.
pub fn packed_len(n: usize, bits: u32) -> Option<usize> {
    n.checked_mul(bits as usize)
        .and_then(|p| p.checked_add(7))
        .map(|p| p / 8)
}

/// Maximum reconstruction error (per element, absolute) for a vector
/// quantized at `bits` over `[min, max]`. Equal to half the bin width:
/// `(max - min) / (2 * (2^bits - 1))`.
pub fn max_reconstruction_error(min: f32, max: f32, bits: BitWidth) -> f32 {
    let levels = bits.levels();
    if levels <= 1 {
        return f32::INFINITY;
    }
    let range = max - min;
    range / (2.0 * (levels - 1) as f32)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tier_compression_tests;
