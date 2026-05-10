use crate::aux::AuxHeadEnsemble;
use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Tensor;

pub const DEFAULT_BATCHES_PER_CYCLE: usize = 50;
pub const DEFAULT_TEACHER_CONFIDENCE_THRESHOLD: f32 = 0.5;
pub const EXCESSIVE_SKIP_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct BatchInput {
    pub panel_t01: Tensor,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DistillationResult {
    pub cycle_id: u64,
    pub mean_loss: f32,
    pub skipped_count: u64,
    pub processed_count: u64,
    pub excessive_skips: bool,
}

pub trait PredictorTrunk {
    fn hidden_forward(&self, panel: &Tensor) -> Result<Tensor, TrainerError>;
    fn backward_through_hidden(&mut self, hidden_grad: &Tensor) -> Result<(), TrainerError>;
}

#[derive(Debug, Clone)]
pub struct AuxHeadDistiller {
    pub interval_steps: u64,
    pub batches_per_cycle: usize,
    pub quality_gate: bool,
    pub teacher_confidence_threshold: f32,
    pub cycle_id: u64,
}

impl AuxHeadDistiller {
    pub fn new(interval_steps: u64, batches_per_cycle: usize) -> Self {
        Self {
            interval_steps,
            batches_per_cycle,
            quality_gate: true,
            teacher_confidence_threshold: DEFAULT_TEACHER_CONFIDENCE_THRESHOLD,
            cycle_id: 0,
        }
    }

    pub fn should_run(&self, step: u64) -> bool {
        step > 0 && self.interval_steps > 0 && step.is_multiple_of(self.interval_steps)
    }

    pub fn run_cycle(
        &mut self,
        predictor: &mut dyn PredictorTrunk,
        aux_heads_frozen_snapshot: &AuxHeadEnsemble,
        aux_heads_trainable: &mut AuxHeadEnsemble,
        batches: &[BatchInput],
    ) -> Result<DistillationResult, TrainerError> {
        self.cycle_id += 1;
        let mut loss_sum = 0.0f32;
        let mut processed = 0u64;
        let mut skipped = 0u64;
        for batch in batches.iter().take(self.batches_per_cycle) {
            let hidden = predictor.hidden_forward(&batch.panel_t01)?;
            let teacher = aux_heads_frozen_snapshot.forward_logits(&hidden)?;
            let student = aux_heads_trainable.forward_logits(&hidden)?;
            let teacher_probs = tensor_probs(&teacher)?;
            let confidence = teacher_probs.iter().copied().fold(0.0f32, f32::max);
            if self.quality_gate && confidence < self.teacher_confidence_threshold {
                skipped += 1;
                continue;
            }
            let kl = kl_from_probs_and_logits(&teacher_probs, &student)?;
            let grad = Tensor::new(kl, hidden.device())?;
            predictor.backward_through_hidden(&grad)?;
            loss_sum += kl;
            processed += 1;
        }
        let total = processed + skipped;
        Ok(DistillationResult {
            cycle_id: self.cycle_id,
            mean_loss: if processed == 0 {
                0.0
            } else {
                loss_sum / processed as f32
            },
            skipped_count: skipped,
            processed_count: processed,
            excessive_skips: total > 0 && skipped as f32 / total as f32 > EXCESSIVE_SKIP_THRESHOLD,
        })
    }
}

fn tensor_probs(tensor: &Tensor) -> Result<Vec<f32>, TrainerError> {
    let vals = tensor
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    softmax(&vals)
}

fn kl_from_probs_and_logits(
    teacher_probs: &[f32],
    student_logits: &Tensor,
) -> Result<f32, TrainerError> {
    let student_vals = student_logits
        .to_dtype(candle_core::DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?;
    let student_probs = softmax(&student_vals)?;
    if teacher_probs.len() != student_probs.len() {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "distillation teacher/student logits length mismatch",
        ));
    }
    Ok(teacher_probs
        .iter()
        .zip(student_probs.iter())
        .filter(|(t, _)| **t > 0.0)
        .map(|(t, s)| t * (t.ln() - s.max(1e-8).ln()))
        .sum())
}

fn softmax(logits: &[f32]) -> Result<Vec<f32>, TrainerError> {
    if logits.is_empty() || logits.iter().any(|v| !v.is_finite()) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "distillation logits are empty or non-finite",
        ));
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits.iter().map(|v| (*v - max).exp()).collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(1e-8);
    Ok(exp.iter().map(|v| v / sum).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_gate() {
        let d = AuxHeadDistiller::new(1000, 50);
        assert!(!d.should_run(0));
        assert!(d.should_run(1000));
        assert!(!d.should_run(999));
    }
}
