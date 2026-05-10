use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::Device;
use context_graph_embeddings::training::lora::{LoraConfig, LoraLayers};
use std::collections::HashMap;

pub struct LoRAAdapterHandle {
    pub name: String,
    pub layers: LoraLayers,
}

pub struct LoraState {
    pub adapters: HashMap<String, LoRAAdapterHandle>,
    pub rank: usize,
    pub alpha: usize,
    pub dropout: f32,
}

pub trait LoraAttachable {
    fn project_param_names(&self) -> Vec<String>;
    fn attach_lora(&mut self, name: &str, adapter: LoRAAdapterHandle) -> Result<(), TrainerError>;
}

pub fn attach_loras(
    predictor: &mut dyn LoraAttachable,
    aux: &mut dyn LoraAttachable,
    trainable_embedders: &mut [Box<dyn LoraAttachable>],
    rank: usize,
    alpha: usize,
    dropout: f32,
) -> Result<LoraState, TrainerError> {
    if rank == 0 || alpha == 0 || !(0.0..=0.5).contains(&dropout) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "LoRA rank/alpha/dropout config invalid",
        ));
    }
    let mut adapters = HashMap::new();
    attach_for("predictor", predictor, rank, alpha, dropout, &mut adapters)?;
    attach_for("aux", aux, rank, alpha, dropout, &mut adapters)?;
    for (idx, embedder) in trainable_embedders.iter_mut().enumerate() {
        attach_for(
            &format!("embedder_{idx}"),
            embedder.as_mut(),
            rank,
            alpha,
            dropout,
            &mut adapters,
        )?;
    }
    Ok(LoraState {
        adapters,
        rank,
        alpha,
        dropout,
    })
}

pub fn detach_loras(_state: LoraState) -> Result<(), TrainerError> {
    Ok(())
}

fn attach_for(
    prefix: &str,
    target: &mut dyn LoraAttachable,
    rank: usize,
    alpha: usize,
    dropout: f32,
    state: &mut HashMap<String, LoRAAdapterHandle>,
) -> Result<(), TrainerError> {
    for name in target.project_param_names() {
        let config = LoraConfig {
            rank,
            alpha: alpha as f32,
            dropout,
            apply_key: true,
            ..LoraConfig::default()
        };
        let layers = LoraLayers::new(config, &Device::Cpu).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("failed to construct LoRA layers for {prefix}.{name}: {err}"),
            )
        })?;
        let key = format!("{prefix}.{name}");
        let handle = LoRAAdapterHandle {
            name: key.clone(),
            layers,
        };
        target.attach_lora(&name, handle_clone(&handle)?)?;
        state.insert(key, handle);
    }
    Ok(())
}

fn handle_clone(handle: &LoRAAdapterHandle) -> Result<LoRAAdapterHandle, TrainerError> {
    let config = handle.layers.config.clone();
    Ok(LoRAAdapterHandle {
        name: handle.name.clone(),
        layers: LoraLayers::new(config, &Device::Cpu).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("failed to clone LoRA handle: {err}"),
            )
        })?,
    })
}
