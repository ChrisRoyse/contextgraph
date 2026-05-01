use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct PairKindBitset(pub u8);

impl PairKindBitset {
    pub const COSINE_AGREEMENT: u8 = 0b0000_0001;
    pub const RANK_DISAGREEMENT: u8 = 0b0000_0010;
    pub const MODALITY_CONTRADICTION: u8 = 0b0000_0100;
    pub const SPARSE_DENSE_MISMATCH: u8 = 0b0000_1000;
    pub const TEMPORAL_SURPRISE: u8 = 0b0001_0000;
    pub const CAUSAL_DIRECTION: u8 = 0b0010_0000;
    pub const SAFETY_PROXIMITY: u8 = 0b0100_0000;
    pub const ALL_KIND_BITS: u8 = 0b0111_1111;

    pub fn has(self, kind: u8) -> bool {
        (self.0 & kind) != 0
    }

    pub fn set(&mut self, kind: u8) {
        self.0 |= kind;
    }

    pub fn clear(&mut self, kind: u8) {
        self.0 &= !kind;
    }

    pub fn iter_kinds(self) -> impl Iterator<Item = &'static str> {
        let mut out = Vec::new();
        if self.has(Self::COSINE_AGREEMENT) {
            out.push("cosine_agreement");
        }
        if self.has(Self::RANK_DISAGREEMENT) {
            out.push("rank_disagreement");
        }
        if self.has(Self::MODALITY_CONTRADICTION) {
            out.push("modality_contradiction");
        }
        if self.has(Self::SPARSE_DENSE_MISMATCH) {
            out.push("sparse_dense_mismatch");
        }
        if self.has(Self::TEMPORAL_SURPRISE) {
            out.push("temporal_surprise");
        }
        if self.has(Self::CAUSAL_DIRECTION) {
            out.push("causal_direction_disagreement");
        }
        if self.has(Self::SAFETY_PROXIMITY) {
            out.push("safety_proximity");
        }
        out.into_iter()
    }

    pub fn validate(self) -> DynamicJepaResult<()> {
        if self.0 & !Self::ALL_KIND_BITS != 0 {
            return Err(DynamicJepaError::validation(
                "PairKindBitset",
                format!(
                    "unknown pair-kind bits set: 0b{:08b}",
                    self.0 & !Self::ALL_KIND_BITS
                ),
                "write only the declared PairKindBitset constants",
            ));
        }
        if !self.has(Self::COSINE_AGREEMENT) {
            return Err(DynamicJepaError::validation(
                "PairKindBitset",
                "cosine_agreement bit must always be emitted",
                "set PairKindBitset::COSINE_AGREEMENT before persisting pairwise readings",
            ));
        }
        Ok(())
    }
}
