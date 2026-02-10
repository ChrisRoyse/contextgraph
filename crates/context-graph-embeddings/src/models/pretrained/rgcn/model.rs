//! R-GCN model implementation using Candle.
//!
//! Implements a 2-layer Relational Graph Convolutional Network for
//! memory graph reasoning with 8 relation types.
//!
//! # Architecture
//!
//! - Layer 1: RGCNLayer(32, 64) + ReLU
//! - Layer 2: RGCNLayer(64, 32)
//!
//! # Weight Loading
//!
//! Weights are loaded from SafeTensors files exported by the Python training script.

use std::collections::HashMap;
use std::path::Path;

use candle_core::{DType, Device, IndexOp, Tensor};
use safetensors::SafeTensors;
use tracing::{debug, info};

use crate::error::{EmbeddingError, EmbeddingResult};

use super::constants::{HIDDEN_DIM, INPUT_DIM, NUM_BASES, NUM_RELATIONS, OUTPUT_DIM};

/// R-GCN layer weights.
#[derive(Clone)]
struct RGCNLayerWeights {
    /// Basis matrices [num_bases, in_dim, out_dim] or weight [num_relations, in_dim, out_dim]
    weight: Tensor,
    /// Attention coefficients [num_relations, num_bases] (if using basis decomposition)
    att: Option<Tensor>,
    /// Self-loop weight [in_dim, out_dim]
    root: Tensor,
    /// Bias [out_dim]
    bias: Option<Tensor>,
    /// Whether using basis decomposition
    use_bases: bool,
}

impl RGCNLayerWeights {
    /// Compute relation-specific weight matrix.
    fn get_relation_weight(&self, relation: usize, _device: &Device) -> EmbeddingResult<Tensor> {
        if self.use_bases {
            // Basis decomposition: W_r = Σ_b a_rb * B_b
            let att = self.att.as_ref().ok_or_else(|| EmbeddingError::ConfigError {
                message: "Attention coefficients missing for basis decomposition".to_string(),
            })?;

            // Get attention coefficients for this relation
            let att_r = att.i(relation).map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to get attention for relation {}: {}", relation, e),
            })?;

            // Weighted sum of basis matrices
            let mut result: Option<Tensor> = None;
            for b in 0..NUM_BASES {
                let att_coef: f64 = att_r
                    .i(b)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to get attention coef: {}", e),
                    })?
                    .to_scalar::<f32>()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to extract scalar coef: {}", e),
                    })? as f64;
                let basis = self.weight.i(b).map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to get basis {}: {}", b, e),
                })?;
                let scaled = basis
                    .affine(att_coef, 0.0)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to scale basis: {}", e),
                    })?;

                result = Some(match result {
                    None => scaled,
                    Some(r) => (r + scaled).map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to sum bases: {}", e),
                    })?,
                });
            }

            result.ok_or_else(|| EmbeddingError::ConfigError {
                message: "No bases to sum".to_string(),
            })
        } else {
            // Full relation-specific weights
            self.weight.i(relation).map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to get weight for relation {}: {}", relation, e),
            })
        }
    }
}

/// Relational Graph Convolutional Network for memory graph.
///
/// # Example
///
/// ```ignore
/// use context_graph_embeddings::models::pretrained::rgcn::RelationalGCN;
///
/// let rgcn = RelationalGCN::load("models/rgcn/model.safetensors", &device)?;
///
/// // node_features: [num_nodes, 32]
/// // edge_index: [(src, dst), ...]
/// // edge_types: [relation_type, ...]
/// let embeddings = rgcn.forward(&node_features, &edge_index, &edge_types)?;
/// ```
#[derive(Clone)]
pub struct RelationalGCN {
    /// Layer 1 weights
    layer1: RGCNLayerWeights,
    /// Layer 2 weights
    layer2: RGCNLayerWeights,
    /// Device for computation
    device: Device,
    /// Whether the model is loaded
    loaded: bool,
}

impl RelationalGCN {
    /// Load the model from a SafeTensors file.
    ///
    /// # Arguments
    ///
    /// * `weights_path` - Path to the SafeTensors weights file
    /// * `device` - Device to load tensors to
    ///
    /// # Errors
    ///
    /// Returns error if the file cannot be read or tensors are invalid.
    pub fn load<P: AsRef<Path>>(weights_path: P, device: &Device) -> EmbeddingResult<Self> {
        let weights_path = weights_path.as_ref();
        info!("Loading R-GCN from {:?}", weights_path);

        // Read the SafeTensors file
        let data = std::fs::read(weights_path)?;

        let safetensors =
            SafeTensors::deserialize(&data).map_err(|e| EmbeddingError::ConfigError {
                message: format!("Failed to parse SafeTensors from {:?}: {}", weights_path, e),
            })?;

        // Load layer 1 weights
        let layer1 = Self::load_layer_weights(&safetensors, "conv1", device)?;

        // Load layer 2 weights
        let layer2 = Self::load_layer_weights(&safetensors, "conv2", device)?;

        debug!(
            "Loaded R-GCN: layer1_root={:?}, layer2_root={:?}",
            layer1.root.shape(),
            layer2.root.shape()
        );

        Ok(Self {
            layer1,
            layer2,
            device: device.clone(),
            loaded: true,
        })
    }

    /// Create an uninitialized model (for testing).
    pub fn uninitialized(device: &Device) -> EmbeddingResult<Self> {
        let layer1 = Self::create_random_layer(INPUT_DIM, HIDDEN_DIM, device)?;
        let layer2 = Self::create_random_layer(HIDDEN_DIM, OUTPUT_DIM, device)?;

        Ok(Self {
            layer1,
            layer2,
            device: device.clone(),
            loaded: false,
        })
    }

    /// Forward pass through the R-GCN.
    ///
    /// # Arguments
    ///
    /// * `node_features` - Node feature tensor [num_nodes, input_dim]
    /// * `edge_index` - Edge list as (source, target) pairs
    /// * `edge_types` - Relation type for each edge
    ///
    /// # Returns
    ///
    /// Node embeddings [num_nodes, output_dim]
    pub fn forward(
        &self,
        node_features: &Tensor,
        edge_index: &[(usize, usize)],
        edge_types: &[u8],
    ) -> EmbeddingResult<Tensor> {
        if edge_index.len() != edge_types.len() {
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "Edge index length {} != edge types length {}",
                    edge_index.len(),
                    edge_types.len()
                ),
            });
        }

        // Layer 1: RGCN + ReLU
        let h = self.rgcn_layer(node_features, edge_index, edge_types, &self.layer1)?;
        let h = h.relu().map_err(|e| EmbeddingError::GpuError {
            message: format!("ReLU failed: {}", e),
        })?;

        // Layer 2: RGCN
        let out = self.rgcn_layer(&h, edge_index, edge_types, &self.layer2)?;

        Ok(out)
    }

    /// Compute link prediction score between two nodes.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Node embeddings from forward pass
    /// * `source` - Source node index
    /// * `target` - Target node index
    ///
    /// # Returns
    ///
    /// Dot product score (higher = more likely connected)
    pub fn link_score(
        &self,
        embeddings: &Tensor,
        source: usize,
        target: usize,
    ) -> EmbeddingResult<f32> {
        let src_emb = embeddings.i(source).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to get source embedding: {}", e),
        })?;
        let dst_emb = embeddings.i(target).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to get target embedding: {}", e),
        })?;

        let score = (src_emb * dst_emb)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to compute dot product: {}", e),
            })?
            .sum_all()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to sum: {}", e),
            })?
            .to_scalar::<f32>()
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to extract scalar: {}", e),
            })?;

        Ok(score)
    }

    /// Check if the model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    // Internal methods

    fn rgcn_layer(
        &self,
        x: &Tensor,
        edge_index: &[(usize, usize)],
        edge_types: &[u8],
        layer: &RGCNLayerWeights,
    ) -> EmbeddingResult<Tensor> {
        let (num_nodes, _) = x.dims2().map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to get input dims: {}", e),
        })?;

        let (_, out_dim) = layer.root.dims2().map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to get root dims: {}", e),
        })?;

        // Accumulate messages per destination node
        let mut accumulators: Vec<Vec<f32>> = vec![vec![0.0; out_dim]; num_nodes];

        // Group edges by relation type
        let mut edges_by_type: HashMap<u8, Vec<(usize, usize)>> = HashMap::new();
        for (i, &(src, dst)) in edge_index.iter().enumerate() {
            edges_by_type
                .entry(edge_types[i])
                .or_default()
                .push((src, dst));
        }

        // Aggregate messages by relation type
        for (rel_type, edges) in edges_by_type {
            if rel_type as usize >= NUM_RELATIONS {
                continue;
            }

            let w_r = layer.get_relation_weight(rel_type as usize, &self.device)?;

            // Compute messages: h_src @ W_r
            for (src, dst) in edges {
                if src >= num_nodes || dst >= num_nodes {
                    continue;
                }

                let src_emb = x.i(src).map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to get src embedding: {}", e),
                })?;

                // msg = src_emb @ W_r
                let msg = src_emb
                    .unsqueeze(0)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to unsqueeze: {}", e),
                    })?
                    .matmul(&w_r)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed matmul: {}", e),
                    })?
                    .squeeze(0)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to squeeze: {}", e),
                    })?;

                // Extract message values and accumulate
                let msg_vec: Vec<f32> = msg.to_vec1().map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to extract msg: {}", e),
                })?;

                for (i, val) in msg_vec.iter().enumerate() {
                    accumulators[dst][i] += val;
                }
            }
        }

        // Convert accumulators to tensor
        let flat_acc: Vec<f32> = accumulators.into_iter().flatten().collect();
        let mut out =
            Tensor::from_slice(&flat_acc, (num_nodes, out_dim), &self.device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create output tensor: {}", e),
                }
            })?;

        // Add self-loop: h_i @ W_0
        let self_loop = x
            .matmul(&layer.root)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed self-loop matmul: {}", e),
            })?;

        out = (out + self_loop).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to add self-loop: {}", e),
        })?;

        // Add bias if present
        if let Some(ref bias) = layer.bias {
            let bias_broadcast = bias
                .unsqueeze(0)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to unsqueeze bias: {}", e),
                })?
                .broadcast_as((num_nodes, out_dim))
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to broadcast bias: {}", e),
                })?;

            out = (out + bias_broadcast).map_err(|e| EmbeddingError::GpuError {
                message: format!("Failed to add bias: {}", e),
            })?;
        }

        Ok(out)
    }

    fn load_layer_weights(
        safetensors: &SafeTensors,
        prefix: &str,
        device: &Device,
    ) -> EmbeddingResult<RGCNLayerWeights> {
        // Try to load basis decomposition first
        let use_bases = safetensors.tensor(&format!("{}_bases", prefix)).is_ok();

        let (weight, att) = if use_bases {
            let bases = Self::load_tensor(safetensors, &format!("{}_bases", prefix), device)?;
            let att = Self::load_tensor(safetensors, &format!("{}_att", prefix), device)?;
            (bases, Some(att))
        } else {
            let weight = Self::load_tensor(safetensors, &format!("{}_weight", prefix), device)?;
            (weight, None)
        };

        let root = Self::load_tensor(safetensors, &format!("{}_root", prefix), device)?;

        let bias = Self::load_tensor(safetensors, &format!("{}_bias", prefix), device).ok();

        Ok(RGCNLayerWeights {
            weight,
            att,
            root,
            bias,
            use_bases,
        })
    }

    fn load_tensor(
        safetensors: &SafeTensors,
        name: &str,
        device: &Device,
    ) -> EmbeddingResult<Tensor> {
        let view = safetensors
            .tensor(name)
            .map_err(|e| EmbeddingError::ConfigError {
                message: format!("Tensor '{}' not found: {}", name, e),
            })?;

        let shape: Vec<usize> = view.shape().iter().map(|&d| d).collect();
        let data: Vec<f32> = view
            .data()
            .chunks(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        Tensor::from_slice(&data, &shape[..], device).map_err(|e| EmbeddingError::GpuError {
            message: format!("Failed to create tensor '{}': {}", name, e),
        })
    }

    fn create_random_layer(
        in_dim: usize,
        out_dim: usize,
        device: &Device,
    ) -> EmbeddingResult<RGCNLayerWeights> {
        let weight =
            Tensor::randn(0f32, 0.1f32, (NUM_BASES, in_dim, out_dim), device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create random weight: {}", e),
                }
            })?;

        let att = Tensor::randn(0f32, 0.1f32, (NUM_RELATIONS, NUM_BASES), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create random att: {}", e),
            }
        })?;

        let root = Tensor::randn(0f32, 0.1f32, (in_dim, out_dim), device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create random root: {}", e),
            }
        })?;

        let bias = Tensor::zeros(out_dim, DType::F32, device).map_err(|e| {
            EmbeddingError::GpuError {
                message: format!("Failed to create random bias: {}", e),
            }
        })?;

        Ok(RGCNLayerWeights {
            weight,
            att: Some(att),
            root,
            bias: Some(bias),
            use_bases: true,
        })
    }
}

impl std::fmt::Debug for RelationalGCN {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelationalGCN")
            .field("loaded", &self.loaded)
            .field("device", &self.device)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uninitialized_model() {
        let device = Device::Cpu;
        let model = RelationalGCN::uninitialized(&device).unwrap();
        assert!(!model.is_loaded());
    }

    #[test]
    fn test_forward_pass() {
        let device = Device::Cpu;
        let model = RelationalGCN::uninitialized(&device).unwrap();

        // Create small test graph
        let num_nodes = 5;
        let node_features = Tensor::randn(0f32, 1f32, (num_nodes, INPUT_DIM), &device).unwrap();

        let edge_index = vec![
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (0, 2),
            (1, 3),
        ];
        let edge_types = vec![0, 1, 2, 3, 0, 1]; // Various relation types

        let embeddings = model.forward(&node_features, &edge_index, &edge_types).unwrap();

        let shape = embeddings.dims2().unwrap();
        assert_eq!(shape.0, num_nodes);
        assert_eq!(shape.1, OUTPUT_DIM);
    }

    #[test]
    fn test_link_score() {
        let device = Device::Cpu;
        let model = RelationalGCN::uninitialized(&device).unwrap();

        let num_nodes = 5;
        let node_features = Tensor::randn(0f32, 1f32, (num_nodes, INPUT_DIM), &device).unwrap();

        let edge_index = vec![(0, 1), (1, 2)];
        let edge_types = vec![0, 1];

        let embeddings = model.forward(&node_features, &edge_index, &edge_types).unwrap();

        let score = model.link_score(&embeddings, 0, 1).unwrap();
        assert!(score.is_finite());
    }
}
