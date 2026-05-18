use super::ablation::{ablation_report_key, AblationReport};
use super::error::{EvalError, EvalErrorCode};
use super::fingerprint_ship_gate::{fingerprint_ship_gate_window_key, FingerprintShipGateWindow};
use super::graph::PatchSimilarityGraph;
use super::novel_pattern::{ontology_growth_audit_key, OntologyGrowthAuditEntry};
use super::queue::{
    curiosity_telemetry_window_key, ActiveLearningLabel, ActiveLearningQueueEntry,
    ActiveLearningQueueState, CuriosityTelemetryWindow, LegacyActiveLearningQueueEntry,
    LegacyActiveLearningQueueState,
};
use super::telemetry::ProductionTelemetryWindow;
use super::types::{EvalReport, OpenResearchQuestionStatus};
use crate::calibration::cf;
use crate::system_cost::SystemCostCounters;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;

pub use context_graph_mejepa_cf::{
    CF_MEJEPA_ABLATION_REPORTS, CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS, CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_CURIOSITY_TELEMETRY, CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_MODEL_PROMOTIONS, CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT, CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY, CF_MEJEPA_RESEARCH_QUESTIONS, CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
};

#[derive(Clone)]
pub struct RocksDbEvalStore {
    db: Arc<DB>,
    system_cost_counters: Option<Arc<SystemCostCounters>>,
}

impl RocksDbEvalStore {
    pub fn new(db: Arc<DB>) -> Result<Self, EvalError> {
        Self::new_inner(db, None)
    }

    pub fn new_with_system_cost_counters(
        db: Arc<DB>,
        system_cost_counters: Arc<SystemCostCounters>,
    ) -> Result<Self, EvalError> {
        Self::new_inner(db, Some(system_cost_counters))
    }

    fn new_inner(
        db: Arc<DB>,
        system_cost_counters: Option<Arc<SystemCostCounters>>,
    ) -> Result<Self, EvalError> {
        for cf_name in context_graph_mejepa_cf::EVAL_CFS {
            if db.cf_handle(cf_name).is_none() {
                return Err(EvalError::new(
                    EvalErrorCode::Store,
                    format!("missing eval column family {cf_name}"),
                ));
            }
        }
        Ok(Self {
            db,
            system_cost_counters,
        })
    }

    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    pub fn system_cost_counters(&self) -> Option<Arc<SystemCostCounters>> {
        self.system_cost_counters.clone()
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

    pub fn load_recent_reports(&self, limit: usize) -> Result<Vec<EvalReport>, EvalError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cf_handle = cf(&self.db, CF_MEJEPA_EVAL_REPORTS).map_err(EvalError::from)?;
        let mut reports = Vec::with_capacity(limit);
        for item in self
            .db
            .iterator_cf(cf_handle, IteratorMode::End)
            .take(limit)
        {
            let (_key, value) = item?;
            let report: EvalReport = bincode::deserialize(&value)?;
            report.validate()?;
            reports.push(report);
        }
        reports.reverse();
        Ok(reports)
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

    pub fn persist_ablation_report(&self, report: &AblationReport) -> Result<(), EvalError> {
        report.validate()?;
        let key = ablation_report_key(report);
        self.put_readback_bin(CF_MEJEPA_ABLATION_REPORTS, &key, report)?;
        let readback = self.load_ablation_report_by_key(&key)?.ok_or_else(|| {
            EvalError::new(EvalErrorCode::ReportPersistFail, "missing ablation report")
        })?;
        if readback != *report {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                "persisted ablation report differs from input",
            ));
        }
        Ok(())
    }

    pub fn load_ablation_report_by_key(
        &self,
        key: &[u8],
    ) -> Result<Option<AblationReport>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_ABLATION_REPORTS).map_err(EvalError::from)?;
        let Some(value) = self.db.get_cf(cf_handle, key)? else {
            return Ok(None);
        };
        let report: AblationReport = bincode::deserialize(&value)?;
        report.validate()?;
        Ok(Some(report))
    }

    pub fn load_ablation_reports_chronological(&self) -> Result<Vec<AblationReport>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_ABLATION_REPORTS).map_err(EvalError::from)?;
        let mut reports = Vec::new();
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let (_key, value) = item?;
            let report: AblationReport = bincode::deserialize(&value)?;
            report.validate()?;
            reports.push(report);
        }
        reports.sort_by(|left, right| {
            left.generated_at_unix_ms
                .cmp(&right.generated_at_unix_ms)
                .then_with(|| left.report_date.cmp(&right.report_date))
                .then_with(|| left.report_id.cmp(&right.report_id))
        });
        Ok(reports)
    }

    pub fn persist_fingerprint_ship_gate_window(
        &self,
        window: &FingerprintShipGateWindow,
    ) -> Result<(), EvalError> {
        let key = fingerprint_ship_gate_window_key(window)?;
        self.put_readback_bin(CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS, &key, window)
    }

    pub fn load_fingerprint_ship_gate_windows_chronological(
        &self,
    ) -> Result<Vec<FingerprintShipGateWindow>, EvalError> {
        let cf_handle =
            cf(&self.db, CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS).map_err(EvalError::from)?;
        let mut windows = Vec::new();
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let (_key, value) = item?;
            let window: FingerprintShipGateWindow = bincode::deserialize(&value)?;
            window.validate()?;
            windows.push(window);
        }
        windows.sort_by(|left, right| {
            left.generated_at_unix_ms
                .cmp(&right.generated_at_unix_ms)
                .then_with(|| left.report_date.cmp(&right.report_date))
                .then_with(|| left.window_id.cmp(&right.window_id))
        });
        Ok(windows)
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
        let Some(bytes) = self.get_raw(CF_MEJEPA_ACTIVE_LEARNING_QUEUE, b"active")? else {
            return Ok(None);
        };
        match bincode::deserialize::<ActiveLearningQueueState>(&bytes) {
            Ok(queue) => Ok(Some(queue)),
            Err(current_err) => {
                let legacy: LegacyActiveLearningQueueState =
                    bincode::deserialize(&bytes).map_err(|_| EvalError::from(current_err))?;
                Ok(Some(legacy.into()))
            }
        }
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

    pub fn load_label(
        &self,
        task_id: &crate::types::TaskId,
    ) -> Result<Option<ActiveLearningLabel>, EvalError> {
        task_id
            .validate("active_learning_label.task_id")
            .map_err(EvalError::from)?;
        self.get_bin(CF_MEJEPA_ACTIVE_LEARNING_LABELS, task_id.0.as_bytes())
    }

    pub fn load_evicted_entry(
        &self,
        task_id: &crate::types::TaskId,
    ) -> Result<Option<ActiveLearningQueueEntry>, EvalError> {
        task_id
            .validate("active_learning_eviction.task_id")
            .map_err(EvalError::from)?;
        let Some(bytes) =
            self.get_raw(CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, task_id.0.as_bytes())?
        else {
            return Ok(None);
        };
        match bincode::deserialize::<ActiveLearningQueueEntry>(&bytes) {
            Ok(entry) => Ok(Some(entry)),
            Err(current_err) => {
                let legacy: LegacyActiveLearningQueueEntry =
                    bincode::deserialize(&bytes).map_err(|_| EvalError::from(current_err))?;
                Ok(Some(legacy.into()))
            }
        }
    }

    pub fn persist_curiosity_telemetry_window(
        &self,
        window: &CuriosityTelemetryWindow,
    ) -> Result<(), EvalError> {
        window.validate()?;
        let key = curiosity_telemetry_window_key(window)?;
        self.put_readback_bin(CF_MEJEPA_CURIOSITY_TELEMETRY, &key, window)
    }

    pub fn load_curiosity_telemetry_windows_chronological(
        &self,
    ) -> Result<Vec<CuriosityTelemetryWindow>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_CURIOSITY_TELEMETRY).map_err(EvalError::from)?;
        let mut windows = Vec::new();
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let (_key, value) = item?;
            let window: CuriosityTelemetryWindow = bincode::deserialize(&value)?;
            window.validate()?;
            windows.push(window);
        }
        windows.sort_by(|left, right| {
            left.generated_at_unix_ms
                .cmp(&right.generated_at_unix_ms)
                .then_with(|| left.window_id.cmp(&right.window_id))
        });
        Ok(windows)
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

    pub fn persist_ontology_growth_audit(
        &self,
        entry: &OntologyGrowthAuditEntry,
    ) -> Result<(), EvalError> {
        entry.validate()?;
        self.put_readback_bin(
            CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
            &ontology_growth_audit_key(entry),
            entry,
        )
    }

    pub fn load_ontology_growth_audit_entries(
        &self,
    ) -> Result<Vec<OntologyGrowthAuditEntry>, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT).map_err(EvalError::from)?;
        let mut entries = Vec::new();
        for item in self.db.iterator_cf(cf_handle, IteratorMode::Start) {
            let (_key, value) = item?;
            let entry: OntologyGrowthAuditEntry = bincode::deserialize(&value)?;
            entry.validate()?;
            entries.push(entry);
        }
        Ok(entries)
    }

    pub fn count_ontology_growth_audit(&self) -> Result<usize, EvalError> {
        let cf_handle = cf(&self.db, CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT).map_err(EvalError::from)?;
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
        if let Some(counters) = &self.system_cost_counters {
            counters.record_rocksdb_write(bytes.len() as u64);
        }
        Ok(())
    }

    fn get_bin<T: DeserializeOwned>(
        &self,
        cf_name: &str,
        key: &[u8],
    ) -> Result<Option<T>, EvalError> {
        let Some(bytes) = self.get_raw(cf_name, key)? else {
            return Ok(None);
        };
        Ok(Some(bincode::deserialize(&bytes)?))
    }

    fn get_raw(&self, cf_name: &str, key: &[u8]) -> Result<Option<Vec<u8>>, EvalError> {
        let cf_handle = cf(&self.db, cf_name).map_err(EvalError::from)?;
        let Some(bytes) = self.db.get_cf(cf_handle, key)? else {
            return Ok(None);
        };
        Ok(Some(bytes))
    }
}

pub fn report_key(report: &EvalReport) -> Vec<u8> {
    format!(
        "{}::{:020}",
        report.report_date, report.generated_at_unix_ms
    )
    .into_bytes()
}
