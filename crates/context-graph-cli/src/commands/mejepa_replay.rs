//! Operator CLI for byte-for-byte ME-JEPA prediction replay.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use context_graph_mejepa::{
    parse_prediction_id_hex, replay_prediction_from_db, PredictionReplayReport,
};
use serde::Serialize;

pub const DEFAULT_MEJEPA_INFER_DB: &str = "/mnt/d/contextgraph/storage/contextgraph-rocksdb";

#[derive(Args, Debug, Clone)]
pub struct ReplayArgs {
    /// Inference RocksDB path containing CF_MEJEPA_LIVE_PREDICTIONS.
    #[arg(long, env = "CONTEXTGRAPH_MEJEPA_INFER_DB", default_value = DEFAULT_MEJEPA_INFER_DB)]
    pub db_path: PathBuf,

    /// Prediction id to replay as 32 hexadecimal characters.
    #[arg(long)]
    pub prediction_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayOutput {
    pub command: String,
    pub replay: PredictionReplayReport,
}

pub async fn replay_prediction(args: ReplayArgs) -> Result<PredictionReplayReport> {
    let prediction_id = parse_prediction_id_hex(&args.prediction_id)?;
    replay_prediction_from_db(&args.db_path, prediction_id)
        .with_context(|| format!("MEJEPA_REPLAY_FAILED: prediction_id={}", args.prediction_id))
}

pub async fn replay_prediction_cli(args: ReplayArgs) -> Result<ReplayOutput> {
    let replay = replay_prediction(args).await?;
    Ok(ReplayOutput {
        command: "context-graph-cli mejepa replay".to_string(),
        replay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_prediction_id_fails_closed() {
        let err = parse_prediction_id_hex("not-hex").unwrap_err().to_string();
        assert!(err.contains("MEJEPA_PREDICTION_REPLAY_ID_INVALID"));
    }
}
