use context_graph_mejepa_embedders::{EmbedderId, EmbedderOutput};
use serde::{Deserialize, Serialize};

use crate::features::{
    bounded_ratio, fnv1a64, normalize_l2, validate_finite_output, validate_single_line,
};
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderHeadInput {
    pub slot: InstrumentSlot,
    pub source_id: String,
    pub embedder_output: EmbedderOutput,
    pub analyzer_evidence: Vec<AnalyzerFeatureEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzerFeatureEvidence {
    pub slot: InstrumentSlot,
    pub source_id: String,
    pub analyzer: String,
    pub feature_version: String,
    pub vector: Vec<f32>,
}

impl AnalyzerFeatureEvidence {
    pub fn from_instrument<I, In>(
        source_id: impl Into<String>,
        analyzer: impl Into<String>,
        feature_version: impl Into<String>,
        instrument: &I,
        input: &In,
    ) -> InstrumentResult<Self>
    where
        I: Instrument<Input = In>,
    {
        let source_id = source_id.into();
        let analyzer = analyzer.into();
        let feature_version = feature_version.into();
        validate_single_line(
            "analyzer_evidence.source_id",
            &source_id,
            "bind analyzer features to the same source_id as the neural forward output",
        )?;
        validate_single_line(
            "analyzer_evidence.analyzer",
            &analyzer,
            "record the concrete analyzer or typed instrument that produced these features",
        )?;
        validate_single_line(
            "analyzer_evidence.feature_version",
            &feature_version,
            "version analyzer feature schemas so persisted heads remain auditable",
        )?;
        let vector = instrument.encode(input)?;
        Ok(Self {
            slot: instrument.slot(),
            source_id,
            analyzer,
            feature_version,
            vector,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedderBackedInstrumentHead {
    slot: InstrumentSlot,
}

impl EmbedderBackedInstrumentHead {
    pub fn new(slot: InstrumentSlot) -> InstrumentResult<Self> {
        if allowed_embedders_for_slot(slot).is_empty() {
            return Err(InstrumentError::invalid(
                "slot",
                format!("slot {} has no embedder-backed head mapping", slot.slug()),
                "use explicit analyzer/scalar/oracle instruments for non-neural slots",
            ));
        }
        Ok(Self { slot })
    }
}

impl Instrument for EmbedderBackedInstrumentHead {
    type Input = EmbedderHeadInput;

    fn slot(&self) -> InstrumentSlot {
        self.slot
    }

    fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>> {
        if input.slot != self.slot {
            return Err(InstrumentError::invalid(
                "input.slot",
                format!(
                    "input slot {} does not match head slot {}",
                    input.slot.slug(),
                    self.slot.slug()
                ),
                "construct one EmbedderBackedInstrumentHead per InstrumentSlot and pass matching input",
            ));
        }
        validate_embedder_head_input(input)?;
        let dim = self.slot.dim();
        let mut out = vec![0.0_f32; dim];
        let seed = projection_seed(self.slot, input.embedder_output.embedder);
        for (idx, value) in input.embedder_output.vector.iter().enumerate() {
            let h = fnv1a64(&seeded_bytes(seed, idx));
            let bucket = (h as usize) % dim;
            let sign = if h & (1 << 63) == 0 { 1.0 } else { -1.0 };
            out[bucket] += *value * sign;
        }
        let analyzer_feature_count: usize = input
            .analyzer_evidence
            .iter()
            .map(|evidence| evidence.vector.len())
            .sum();
        for evidence in &input.analyzer_evidence {
            let analyzer_seed = projection_seed(self.slot, input.embedder_output.embedder)
                ^ fnv1a64(evidence.analyzer.as_bytes())
                ^ fnv1a64(evidence.feature_version.as_bytes());
            for (idx, value) in evidence.vector.iter().enumerate() {
                let h = fnv1a64(&seeded_bytes(analyzer_seed, idx));
                let bucket = (h as usize) % dim;
                let sign = if h & (1 << 63) == 0 { 1.0 } else { -1.0 };
                out[bucket] += *value * sign * 0.5;
            }
        }
        out[0] += bounded_ratio(input.embedder_output.vector.len() as f32, 32_768.0);
        out[1] += bounded_ratio(analyzer_feature_count as f32, 8_192.0);
        out[2] += bounded_ratio(input.analyzer_evidence.len() as f32, 32.0);
        normalize_l2(&mut out);
        let norm = out.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
        if norm <= f64::EPSILON {
            return Err(InstrumentError::invariant(
                "embedder_head.output",
                "projected embedder head produced a zero-norm vector",
                "inspect the upstream neural embedder output; zero vectors must not be persisted as instrument evidence",
            ));
        }
        validate_finite_output("embedder_head.output", &out)?;
        Ok(out)
    }
}

pub fn allowed_embedders_for_slot(slot: InstrumentSlot) -> &'static [EmbedderId] {
    match slot {
        InstrumentSlot::EAst
        | InstrumentSlot::ECfg
        | InstrumentSlot::EDataFlow
        | InstrumentSlot::ETypeGraph
        | InstrumentSlot::EDiff => &[EmbedderId::E7],
        InstrumentSlot::ETest | InstrumentSlot::EProblem | InstrumentSlot::ECommitMsg => {
            &[EmbedderId::E1, EmbedderId::E14]
        }
        InstrumentSlot::ETrace | InstrumentSlot::EReasoning => &[EmbedderId::E10],
        _ => &[],
    }
}

fn validate_embedder_head_input(input: &EmbedderHeadInput) -> InstrumentResult<()> {
    input.embedder_output.validate().map_err(|err| {
        InstrumentError::invalid(
            "input.embedder_output",
            format!("{}: {err}", err.code()),
            "run the real neural EmbedderForward wrapper and pass its validated output",
        )
    })?;
    if input.source_id.trim().is_empty() || input.source_id.chars().any(char::is_control) {
        return Err(InstrumentError::invalid(
            "input.source_id",
            "source_id must be a non-empty single-line identifier",
            "propagate the AST chunk hash, task id, or trace id used for the embedder forward pass",
        ));
    }
    if input.source_id != input.embedder_output.source_id {
        return Err(InstrumentError::invalid(
            "input.source_id",
            format!(
                "head source_id {:?} differs from embedder output source_id {:?}",
                input.source_id, input.embedder_output.source_id
            ),
            "join instrument heads to embedder outputs by exact source id; do not reuse vectors across sources",
        ));
    }
    let allowed = allowed_embedders_for_slot(input.slot);
    if !allowed.contains(&input.embedder_output.embedder) {
        return Err(InstrumentError::invalid(
            "input.embedder_output.embedder",
            format!(
                "embedder {} is not allowed for slot {}; allowed={allowed:?}",
                input.embedder_output.embedder,
                input.slot.slug()
            ),
            "route code slots to E7, text slots to E1/E14, and trace/reasoning slots to E10",
        ));
    }
    if input.analyzer_evidence.is_empty() {
        return Err(InstrumentError::invalid(
            "input.analyzer_evidence",
            "embedder-backed heads require typed analyzer evidence",
            "construct analyzer evidence with AnalyzerFeatureEvidence::from_instrument from the real typed source-of-truth instrument",
        ));
    }
    for (evidence_idx, evidence) in input.analyzer_evidence.iter().enumerate() {
        validate_single_line(
            "input.analyzer_evidence[i].source_id",
            &evidence.source_id,
            "bind analyzer features to the exact source_id used for the neural forward pass",
        )
        .map_err(|err| annotate_evidence_error(err, evidence_idx))?;
        validate_single_line(
            "input.analyzer_evidence[i].analyzer",
            &evidence.analyzer,
            "record the concrete analyzer or typed instrument that produced the evidence vector",
        )
        .map_err(|err| annotate_evidence_error(err, evidence_idx))?;
        validate_single_line(
            "input.analyzer_evidence[i].feature_version",
            &evidence.feature_version,
            "version analyzer features so persisted head evidence can be replayed",
        )
        .map_err(|err| annotate_evidence_error(err, evidence_idx))?;
        if evidence.source_id != input.source_id {
            return Err(InstrumentError::invalid(
                "input.analyzer_evidence[i].source_id",
                format!(
                    "analyzer evidence {evidence_idx} source_id {:?} differs from head source_id {:?}",
                    evidence.source_id, input.source_id
                ),
                "do not join analyzer features or neural vectors across different source ids",
            ));
        }
        if evidence.slot != input.slot {
            return Err(InstrumentError::invalid(
                "input.analyzer_evidence[i].slot",
                format!(
                    "analyzer evidence {evidence_idx} slot {} differs from head slot {}",
                    evidence.slot.slug(),
                    input.slot.slug()
                ),
                "feed each embedder-backed head with analyzer evidence for the same instrument slot",
            ));
        }
        if evidence.vector.len() != input.slot.dim() {
            return Err(InstrumentError::invalid(
                "input.analyzer_evidence[i].vector",
                format!(
                    "analyzer evidence {evidence_idx} vector len {} does not match slot {} dim {}",
                    evidence.vector.len(),
                    input.slot.slug(),
                    input.slot.dim()
                ),
                "use AnalyzerFeatureEvidence::from_instrument so analyzer vector dimensions match the head slot",
            ));
        }
        for (value_idx, value) in evidence.vector.iter().enumerate() {
            if !value.is_finite() {
                return Err(InstrumentError::invariant(
                    "input.analyzer_evidence[i].vector",
                    format!(
                        "analyzer_evidence[{evidence_idx}].vector[{value_idx}] is non-finite: {value}"
                    ),
                    "reject non-finite analyzer features before projecting an instrument head",
                ));
            }
        }
        let norm = evidence
            .vector
            .iter()
            .map(|value| (*value as f64) * (*value as f64))
            .sum::<f64>();
        if norm <= f64::EPSILON {
            return Err(InstrumentError::invariant(
                "input.analyzer_evidence[i].vector",
                format!("analyzer evidence {evidence_idx} produced a zero-norm vector"),
                "inspect the upstream analyzer; zero vectors must not be used as semantic-head evidence",
            ));
        }
    }
    Ok(())
}

fn annotate_evidence_error(err: InstrumentError, idx: usize) -> InstrumentError {
    InstrumentError::invalid(
        "input.analyzer_evidence[i]",
        format!("analyzer evidence {idx} invalid: {err}"),
        "fix the analyzer evidence source of truth before encoding an embedder-backed head",
    )
}

fn projection_seed(slot: InstrumentSlot, embedder: EmbedderId) -> u64 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(slot.slug().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(embedder.slug().as_bytes());
    fnv1a64(&bytes)
}

fn seeded_bytes(seed: u64, index: usize) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    bytes[8..].copy_from_slice(&(index as u64).to_le_bytes());
    bytes
}
