use super::error::{EvalError, EvalErrorCode};
use super::graph::PatchSimilarityGraph;
use super::queue::{ActiveLearningLabel, ActiveLearningQueueState};
use super::telemetry::ProductionTelemetryWindow;
use super::types::{EvalReport, OpenResearchQuestionStatus};
use crate::calibration::cf;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;

pub use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_OOD_ESCALATIONS, CF_MEJEPA_PRODUCTION_TELEMETRY, CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_TASK_GRAPH, CF_MEJEPA_TASK_GRAPH_HISTORY,
};

#[derive(Clone)]
pub struct RocksDbEvalStore {
    db: Arc<DB>,
}

impl RocksDbEvalStore {
    pub fn new(db: Arc<DB>) -> Result<Self, EvalError> {
        for cf_name in context_graph_mejepa_cf::EVAL_CFS {
            if db.cf_handle(cf_name).is_none() {
                return Err(EvalError::new(
                    EvalErrorCode::Store,
                    format!("missing eval column family {cf_name}"),
                ));
            }
        }
        Ok(Self { db })
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn persist_report(&self, report: &EvalReport) -> Result<(), EvalError> {
        report.validate()?;
        let key = report_key(report);
        self.put_readback_bin(CF_MEJEPA_EVAL_REPORTS, &key, report)?;
        let readback = self
            .load_report_by_key(&key)?
            .ok_or_else(|| EvalError::new(EvalErrorCode::ReportPersistFail, "missing report"))?;
        if readback.determinism_hash()? != report.determinism_hash()? {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                "persisted eval report determinism hash differs from input",
            ));
        }
        Ok(())
    }

    pub fn load_latest_report(&self) -> Result<Option<EvalReport>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_EVAL_REPORTS).map_err(EvalError::from)?;
        let mut iter = self.db.iterator_cf(cf_handle, IteratorMode::End);
        let Some(item) = iter.next() else {
            return Ok(None);
        };
        let (_key, value) = item?;
        let report: EvalReport = bincode::deserialize(&value)?;
        report.validate()?;
        Ok(Some(report))
    }

    pub fn load_report_by_key(&self, key: &[u8]) -> Result<Option<EvalReport>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_EVAL_REPORTS).map_err(EvalError::from)?;
        let Some(value) = self.db.get_cf(cf_handle, key)? else {
            return Ok(None);
        };
        let report: EvalReport = bincode::deserialize(&value)?;
        report.validate()?;
        Ok(Some(report))
    }

    pub fn persist_queue(&self, queue: &ActiveLearningQueueState) -> Result<(), EvalError> {
        self.put_readback_bin(CF_MEJEPA_ACTIVE_LEARNING_QUEUE, b"active", queue)?;
        for entry in &queue.evicted {
            self.put_readback_bin(
                CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
                entry.task_id.0.as_bytes(),
                entry,
            )?;
        }
        for entry in &queue.ood_escalations {
            self.put_readback_bin(CF_MEJEPA_OOD_ESCALATIONS, entry.task_id.0.as_bytes(), entry)?;
        }
        Ok(())
    }

    pub fn load_queue(&self) -> Result<Option<ActiveLearningQueueState>, EvalError> {
        self.get_bin(CF_MEJEPA_ACTIVE_LEARNING_QUEUE, b"active")
    }

    pub fn persist_label(&self, label: &ActiveLearningLabel) -> Result<(), EvalError> {
        label
            .task_id
            .validate("active_learning_label.task_id")
            .map_err(EvalError::from)?;
        self.put_readback_bin(
            CF_MEJEPA_ACTIVE_LEARNING_LABELS,
            label.task_id.0.as_bytes(),
            label,
        )
    }

    pub fn persist_graph(&self, graph: &PatchSimilarityGraph) -> Result<(), EvalError> {
        self.put_readback_bin(CF_MEJEPA_TASK_GRAPH, b"active", graph)?;
        let key = format!(
            "{:020}-{}-edges",
            chrono::Utc::now().timestamp_millis(),
            graph.edge_count
        );
        self.put_readback_bin(CF_MEJEPA_TASK_GRAPH_HISTORY, key.as_bytes(), graph)
    }

    pub fn load_graph(&self) -> Result<Option<PatchSimilarityGraph>, EvalError> {
        self.get_bin(CF_MEJEPA_TASK_GRAPH, b"active")
    }

    pub fn persist_research_questions(
        &self,
        questions: &[OpenResearchQuestionStatus],
    ) -> Result<(), EvalError> {
        self.put_readback_bin(CF_MEJEPA_RESEARCH_QUESTIONS, b"open", &questions.to_vec())
    }

    pub fn load_research_questions(
        &self,
    ) -> Result<Option<Vec<OpenResearchQuestionStatus>>, EvalError> {
        self.get_bin(CF_MEJEPA_RESEARCH_QUESTIONS, b"open")
    }

    pub fn persist_production_telemetry(
        &self,
        window: &ProductionTelemetryWindow,
    ) -> Result<(), EvalError> {
        window.validate()?;
        self.put_readback_bin(
            CF_MEJEPA_PRODUCTION_TELEMETRY,
            window.window_id.as_bytes(),
            window,
        )
    }

    pub fn load_production_telemetry(
        &self,
        window_id: &str,
    ) -> Result<Option<ProductionTelemetryWindow>, EvalError> {
        if window_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry window_id must be non-empty",
            ));
        }
        self.get_bin(CF_MEJEPA_PRODUCTION_TELEMETRY, window_id.as_bytes())
    }

    pub fn count_production_telemetry(&self) -> Result<usize, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_PRODUCTION_TELEMETRY).map_err(EvalError::from)?;
        let mut count = 0usize;
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            item?;
            count += 1;
        }
        Ok(count)
    }

    fn put_readback_bin<T: Serialize + DeserializeOwned>(
        &self,
        cf_name: &str,
        key: &[u8],
        value: &T,
    ) -> Result<(), EvalError> {
        let cf_handle = cf(&self.db, cf_name).map_err(EvalError::from)?;
        let bytes = bincode::serialize(value)?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        self.db.put_cf_opt(cf_handle, key, &bytes, &opts)?;
        let readback = self
            .db
            .get_cf(cf_handle, key)?
            .ok_or_else(|| EvalError::new(EvalErrorCode::ReadbackMismatch, "missing readback"))?;
        if readback != bytes {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                format!("readback bytes differ for {cf_name}"),
            ));
        }
        let decoded: T = bincode::deserialize(&readback)?;
        let decoded_bytes = bincode::serialize(&decoded)?;
        if decoded_bytes != bytes {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                format!("decoded readback differs for {cf_name}"),
            ));
        }
        Ok(())
    }

    fn get_bin<T: DeserializeOwned>(
        &self,
        cf_name: &str,
        key: &[u8],
    ) -> Result<Option<T>, EvalError> {
        let cf_handle = cf(&self.db, cf_name).map_err(EvalError::from)?;
        let Some(bytes) = self.db.get_cf(cf_handle, key)? else {
            return Ok(None);
        };
        Ok(Some(bincode::deserialize(&bytes)?))
    }
}

pub fn report_key(report: &EvalReport) -> Vec<u8> {
    format!(
        "{}::{:020}",
        report.report_date, report.generated_at_unix_ms
    )
    .into_bytes()
}
