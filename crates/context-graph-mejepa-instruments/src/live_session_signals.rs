use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_LIVE_SESSION_SIGNALS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::e_reasoning::{EReasoningInstrument, ReasoningEvent, ReasoningInput};
use crate::features::{fnv1a64, validate_single_line, validate_text_field};
use crate::{Instrument, InstrumentError, InstrumentResult};

const LIVE_SESSION_SCHEMA_VERSION: u32 = 1;
const MAX_PROMPTS: usize = 10_000;
const MAX_VERDICTS: usize = 100_000;
const MAX_OVERRIDES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionSource {
    pub session_id: String,
    pub turn_id: String,
    pub prompt_history: Vec<UserPromptEvent>,
    pub prior_verdicts: Vec<SessionVerdictObservation>,
    pub operator_overrides: Vec<OperatorOverrideObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPromptEvent {
    pub event_id: String,
    pub unix_secs: i64,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionVerdictKind {
    Pass,
    Fail,
    Ood,
    Abstain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionVerdictObservation {
    pub prediction_id: String,
    pub verdict: SessionVerdictKind,
    pub fingerprint_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorOverrideKind {
    Confirmed,
    Refuted,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorOverrideObservation {
    pub prediction_id: String,
    pub outcome: OperatorOverrideKind,
    pub unix_secs: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionLabel {
    pub session_id: String,
    pub turn_id: String,
    pub prompt_event_count: u32,
    pub prompt_token_count: u32,
    pub prior_verdict_count: u32,
    pub prior_pass_count: u32,
    pub prior_fail_count: u32,
    pub prior_ood_count: u32,
    pub prior_abstain_count: u32,
    pub operator_override_count: u32,
    pub confirmed_override_count: u32,
    pub refuted_override_count: u32,
    pub inconclusive_override_count: u32,
    pub refuted_override_fraction: f32,
    pub latest_prompt_unix_secs: i64,
    pub dominant_goal_terms: Vec<String>,
    pub goal_embedding_dim: u32,
    pub goal_embedding_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorGoalSignal {
    pub session_id: String,
    pub turn_id: String,
    pub prompt_event_count: u32,
    pub prompt_text_sha256: String,
    pub dominant_goal_terms: Vec<String>,
    pub goal_embedding_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorVerdictSignal {
    pub session_id: String,
    pub turn_id: String,
    pub prediction_id: String,
    pub verdict: SessionVerdictKind,
    pub fingerprint_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorOverrideSignal {
    pub session_id: String,
    pub turn_id: String,
    pub prediction_id: String,
    pub outcome: OperatorOverrideKind,
    pub unix_secs: i64,
    pub reason_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum LiveSessionSignalRecord {
    Label(LiveSessionLabel),
    OperatorGoal(OperatorGoalSignal),
    PriorVerdict(PriorVerdictSignal),
    OperatorOverride(OperatorOverrideSignal),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedLiveSessionSignal {
    pub schema_version: u32,
    pub signal: LiveSessionSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionExtraction {
    pub labels: Vec<LiveSessionLabel>,
    pub goals: Vec<OperatorGoalSignal>,
    pub verdicts: Vec<PriorVerdictSignal>,
    pub overrides: Vec<OperatorOverrideSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveSessionRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub live_session_json: PathBuf,
}

impl LiveSessionExtraction {
    pub fn persisted_records(&self) -> Vec<PersistedLiveSessionSignal> {
        self.labels
            .iter()
            .cloned()
            .map(|label| PersistedLiveSessionSignal {
                schema_version: LIVE_SESSION_SCHEMA_VERSION,
                signal: LiveSessionSignalRecord::Label(label),
            })
            .chain(
                self.goals
                    .iter()
                    .cloned()
                    .map(|goal| PersistedLiveSessionSignal {
                        schema_version: LIVE_SESSION_SCHEMA_VERSION,
                        signal: LiveSessionSignalRecord::OperatorGoal(goal),
                    }),
            )
            .chain(
                self.verdicts
                    .iter()
                    .cloned()
                    .map(|verdict| PersistedLiveSessionSignal {
                        schema_version: LIVE_SESSION_SCHEMA_VERSION,
                        signal: LiveSessionSignalRecord::PriorVerdict(verdict),
                    }),
            )
            .chain(self.overrides.iter().cloned().map(|override_signal| {
                PersistedLiveSessionSignal {
                    schema_version: LIVE_SESSION_SCHEMA_VERSION,
                    signal: LiveSessionSignalRecord::OperatorOverride(override_signal),
                }
            }))
            .collect()
    }
}

impl LiveSessionSignalRecord {
    fn identity_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::Label(label) => (&label.session_id, &label.turn_id, "label"),
            Self::OperatorGoal(goal) => (&goal.session_id, &goal.turn_id, "operator_goal"),
            Self::PriorVerdict(verdict) => (
                &verdict.session_id,
                &verdict.turn_id,
                &verdict.prediction_id,
            ),
            Self::OperatorOverride(override_signal) => (
                &override_signal.session_id,
                &override_signal.turn_id,
                &override_signal.prediction_id,
            ),
        }
    }
}

pub fn extract_live_session_signals(
    source: &LiveSessionSource,
) -> InstrumentResult<LiveSessionExtraction> {
    validate_source(source)?;
    let transcript = prompt_transcript(&source.prompt_history);
    let events = reasoning_events(source);
    let reasoning = ReasoningInput {
        task_id: format!("live_session::{}::{}", source.session_id, source.turn_id),
        transcript: transcript.clone(),
        events,
    };
    let goal_vector = EReasoningInstrument.encode(&reasoning)?;
    let goal_hash = sha256_f32_vector(&goal_vector);
    let dominant_terms = dominant_goal_terms(&transcript, 8);
    let prompt_token_count = token_count(&transcript);
    let verdict_counts = count_verdicts(&source.prior_verdicts);
    let override_counts = count_overrides(&source.operator_overrides);
    let latest_prompt_unix_secs = source
        .prompt_history
        .iter()
        .map(|prompt| prompt.unix_secs)
        .max()
        .unwrap_or(0);
    let override_count = source.operator_overrides.len();
    let refuted_fraction = if override_count == 0 {
        0.0
    } else {
        override_counts.refuted as f32 / override_count as f32
    };
    let label = LiveSessionLabel {
        session_id: source.session_id.clone(),
        turn_id: source.turn_id.clone(),
        prompt_event_count: to_u32("prompt_event_count", source.prompt_history.len())?,
        prompt_token_count: to_u32("prompt_token_count", prompt_token_count)?,
        prior_verdict_count: to_u32("prior_verdict_count", source.prior_verdicts.len())?,
        prior_pass_count: verdict_counts.pass,
        prior_fail_count: verdict_counts.fail,
        prior_ood_count: verdict_counts.ood,
        prior_abstain_count: verdict_counts.abstain,
        operator_override_count: to_u32("operator_override_count", override_count)?,
        confirmed_override_count: override_counts.confirmed,
        refuted_override_count: override_counts.refuted,
        inconclusive_override_count: override_counts.inconclusive,
        refuted_override_fraction: refuted_fraction,
        latest_prompt_unix_secs,
        dominant_goal_terms: dominant_terms.clone(),
        goal_embedding_dim: to_u32("goal_embedding_dim", goal_vector.len())?,
        goal_embedding_sha256: goal_hash.clone(),
    };
    let goal = OperatorGoalSignal {
        session_id: source.session_id.clone(),
        turn_id: source.turn_id.clone(),
        prompt_event_count: label.prompt_event_count,
        prompt_text_sha256: sha256_text(&transcript),
        dominant_goal_terms: dominant_terms,
        goal_embedding_sha256: goal_hash,
    };
    let verdicts = source
        .prior_verdicts
        .iter()
        .map(|verdict| PriorVerdictSignal {
            session_id: source.session_id.clone(),
            turn_id: source.turn_id.clone(),
            prediction_id: verdict.prediction_id.clone(),
            verdict: verdict.verdict,
            fingerprint_id: verdict.fingerprint_id.clone(),
        })
        .collect();
    let overrides = source
        .operator_overrides
        .iter()
        .map(|override_signal| OperatorOverrideSignal {
            session_id: source.session_id.clone(),
            turn_id: source.turn_id.clone(),
            prediction_id: override_signal.prediction_id.clone(),
            outcome: override_signal.outcome,
            unix_secs: override_signal.unix_secs,
            reason_sha256: sha256_text(&override_signal.reason),
        })
        .collect();
    Ok(LiveSessionExtraction {
        labels: vec![label],
        goals: vec![goal],
        verdicts,
        overrides,
    })
}

pub fn write_live_session_raw_output(
    root: impl AsRef<Path>,
    row_id: &str,
    extraction: &LiveSessionExtraction,
) -> InstrumentResult<LiveSessionRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-live-session-signals-v1",
            err.to_string(),
            "ensure the D-root live-session raw-output directory is writable",
        )
    })?;
    let paths = LiveSessionRawOutputPaths {
        row_id: row_id.to_string(),
        root: root.clone(),
        live_session_json: root.join("live_session.json"),
    };
    write_json_readback(&paths.live_session_json, extraction)?;
    Ok(paths)
}

pub struct LiveSessionSignalStore {
    db: DB,
}

impl LiveSessionSignalStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_LIVE_SESSION_SIGNALS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_LIVE_SESSION_SIGNALS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        if db.cf_handle(CF_MEJEPA_LIVE_SESSION_SIGNALS).is_none() {
            return Err(InstrumentError::store(
                "cf_handle",
                CF_MEJEPA_LIVE_SESSION_SIGNALS,
                "column family missing after RocksDB open",
                "open live-session signal storage with the canonical CF descriptor",
            ));
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &LiveSessionExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for record in extraction.persisted_records() {
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedLiveSessionSignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB iterator state and live-session CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    err.to_string(),
                    "live-session signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    err.to_string(),
                    "only mutate live-session rows through LiveSessionSignalStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_LIVE_SESSION_SIGNALS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedLiveSessionSignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_LIVE_SESSION_SIGNALS,
                err.to_string(),
                "ensure live-session signal records remain JSON-serializable",
            )
        })?;
        let key = live_session_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self
            .db
            .get_cf(self.cf()?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_cf",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    "live-session row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_LIVE_SESSION_SIGNALS,
                "readback bytes differ from written live-session payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_LIVE_SESSION_SIGNALS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_LIVE_SESSION_SIGNALS,
                    "column-family handle not found",
                    "open the store through LiveSessionSignalStore::open",
                )
            })
    }
}

#[derive(Default)]
struct VerdictCounts {
    pass: u32,
    fail: u32,
    ood: u32,
    abstain: u32,
}

#[derive(Default)]
struct OverrideCounts {
    confirmed: u32,
    refuted: u32,
    inconclusive: u32,
}

fn validate_source(source: &LiveSessionSource) -> InstrumentResult<()> {
    validate_single_line(
        "source.session_id",
        &source.session_id,
        "persist a stable single-line session id",
    )?;
    validate_single_line(
        "source.turn_id",
        &source.turn_id,
        "persist a stable single-line turn id",
    )?;
    if source.prompt_history.is_empty() {
        return invalid(
            "source.prompt_history",
            "live-session signal extraction requires at least one UserPromptSubmit event",
            "persist UserPromptSubmit history before encoding E_LiveSession",
        );
    }
    if source.prompt_history.len() > MAX_PROMPTS {
        return invalid(
            "source.prompt_history",
            format!(
                "prompt history contains {} events; max is {MAX_PROMPTS}",
                source.prompt_history.len()
            ),
            "summarize or shard very long prompt histories before live-session extraction",
        );
    }
    if source.prior_verdicts.len() > MAX_VERDICTS {
        return invalid(
            "source.prior_verdicts",
            format!(
                "prior verdict count {} exceeds max {MAX_VERDICTS}",
                source.prior_verdicts.len()
            ),
            "summarize or shard very long verdict histories before live-session extraction",
        );
    }
    if source.operator_overrides.len() > MAX_OVERRIDES {
        return invalid(
            "source.operator_overrides",
            format!(
                "operator override count {} exceeds max {MAX_OVERRIDES}",
                source.operator_overrides.len()
            ),
            "summarize or shard very long override histories before live-session extraction",
        );
    }
    validate_prompt_history(&source.prompt_history)?;
    validate_verdicts(&source.prior_verdicts)?;
    validate_overrides(&source.operator_overrides)?;
    Ok(())
}

fn validate_prompt_history(prompts: &[UserPromptEvent]) -> InstrumentResult<()> {
    let mut seen = BTreeSet::new();
    for (idx, prompt) in prompts.iter().enumerate() {
        validate_single_line(
            "source.prompt_history[i].event_id",
            &prompt.event_id,
            "persist UserPromptSubmit event ids as single-line identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        if !seen.insert(prompt.event_id.clone()) {
            return invalid(
                "source.prompt_history[i].event_id",
                format!("duplicate UserPromptSubmit event id {}", prompt.event_id),
                "deduplicate prompt history before live-session extraction",
            );
        }
        if prompt.unix_secs <= 0 {
            return invalid(
                "source.prompt_history[i].unix_secs",
                format!("prompt {} has non-positive unix_secs", prompt.event_id),
                "persist real prompt timestamps as positive unix seconds",
            );
        }
        validate_text_field(
            "source.prompt_history[i].prompt",
            &prompt.prompt,
            "persist the raw UserPromptSubmit text before live-session extraction",
        )
        .map_err(|err| annotate_idx(err, idx))?;
    }
    Ok(())
}

fn validate_verdicts(verdicts: &[SessionVerdictObservation]) -> InstrumentResult<()> {
    let mut seen = BTreeSet::new();
    for (idx, verdict) in verdicts.iter().enumerate() {
        validate_single_line(
            "source.prior_verdicts[i].prediction_id",
            &verdict.prediction_id,
            "persist prediction ids as single-line identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        if !seen.insert(verdict.prediction_id.clone()) {
            return invalid(
                "source.prior_verdicts[i].prediction_id",
                format!(
                    "duplicate prior verdict prediction id {}",
                    verdict.prediction_id
                ),
                "deduplicate prior verdicts before live-session extraction",
            );
        }
        if let Some(fingerprint_id) = &verdict.fingerprint_id {
            validate_single_line(
                "source.prior_verdicts[i].fingerprint_id",
                fingerprint_id,
                "persist fingerprint ids as single-line identifiers",
            )
            .map_err(|err| annotate_idx(err, idx))?;
        }
    }
    Ok(())
}

fn validate_overrides(overrides: &[OperatorOverrideObservation]) -> InstrumentResult<()> {
    let mut seen = BTreeSet::new();
    for (idx, override_signal) in overrides.iter().enumerate() {
        validate_single_line(
            "source.operator_overrides[i].prediction_id",
            &override_signal.prediction_id,
            "persist prediction ids as single-line identifiers",
        )
        .map_err(|err| annotate_idx(err, idx))?;
        if !seen.insert(override_signal.prediction_id.clone()) {
            return invalid(
                "source.operator_overrides[i].prediction_id",
                format!(
                    "duplicate operator override prediction id {}",
                    override_signal.prediction_id
                ),
                "deduplicate operator overrides before live-session extraction",
            );
        }
        if override_signal.unix_secs <= 0 {
            return invalid(
                "source.operator_overrides[i].unix_secs",
                format!(
                    "override {} has non-positive unix_secs",
                    override_signal.prediction_id
                ),
                "persist real operator override timestamps as positive unix seconds",
            );
        }
        validate_text_field(
            "source.operator_overrides[i].reason",
            &override_signal.reason,
            "persist the operator override rationale before live-session extraction",
        )
        .map_err(|err| annotate_idx(err, idx))?;
    }
    Ok(())
}

fn prompt_transcript(prompts: &[UserPromptEvent]) -> String {
    let mut sorted = prompts.to_vec();
    sorted.sort_by_key(|prompt| (prompt.unix_secs, prompt.event_id.clone()));
    sorted
        .into_iter()
        .map(|prompt| format!("UserPromptSubmit {}: {}", prompt.event_id, prompt.prompt))
        .collect::<Vec<_>>()
        .join("\n")
}

fn reasoning_events(source: &LiveSessionSource) -> Vec<ReasoningEvent> {
    let mut events = Vec::new();
    for prompt in &source.prompt_history {
        events.push(ReasoningEvent {
            actor: "operator".to_string(),
            event_type: "user_prompt_submit".to_string(),
            text: prompt.prompt.clone(),
        });
    }
    for verdict in &source.prior_verdicts {
        events.push(ReasoningEvent {
            actor: "mejepa".to_string(),
            event_type: format!("prior_verdict::{:?}", verdict.verdict).to_lowercase(),
            text: verdict.prediction_id.clone(),
        });
    }
    for override_signal in &source.operator_overrides {
        events.push(ReasoningEvent {
            actor: "operator".to_string(),
            event_type: format!("operator_override::{:?}", override_signal.outcome).to_lowercase(),
            text: override_signal.reason.clone(),
        });
    }
    events
}

fn dominant_goal_terms(transcript: &str, limit: usize) -> Vec<String> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for token in goal_tokens(transcript) {
        *counts.entry(token).or_default() += 1;
    }
    let mut pairs: Vec<(String, u32)> = counts.into_iter().collect();
    pairs.sort_by(|(left_term, left_count), (right_term, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_term.cmp(right_term))
    });
    pairs
        .into_iter()
        .take(limit)
        .map(|(term, _)| term)
        .collect()
}

fn token_count(transcript: &str) -> usize {
    goal_tokens(transcript).count()
}

fn goal_tokens(transcript: &str) -> impl Iterator<Item = String> + '_ {
    transcript
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(str::trim)
        .filter(|token| token.len() >= 3)
        .map(str::to_ascii_lowercase)
        .filter(|token| !STOPWORDS.contains(&token.as_str()))
}

const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "that",
    "this",
    "from",
    "into",
    "have",
    "has",
    "are",
    "was",
    "were",
    "will",
    "can",
    "all",
    "our",
    "you",
    "your",
    "userpromptsubmit",
];

fn count_verdicts(verdicts: &[SessionVerdictObservation]) -> VerdictCounts {
    let mut counts = VerdictCounts::default();
    for verdict in verdicts {
        match verdict.verdict {
            SessionVerdictKind::Pass => counts.pass += 1,
            SessionVerdictKind::Fail => counts.fail += 1,
            SessionVerdictKind::Ood => counts.ood += 1,
            SessionVerdictKind::Abstain => counts.abstain += 1,
        }
    }
    counts
}

fn count_overrides(overrides: &[OperatorOverrideObservation]) -> OverrideCounts {
    let mut counts = OverrideCounts::default();
    for override_signal in overrides {
        match override_signal.outcome {
            OperatorOverrideKind::Confirmed => counts.confirmed += 1,
            OperatorOverrideKind::Refuted => counts.refuted += 1,
            OperatorOverrideKind::Inconclusive => counts.inconclusive += 1,
        }
    }
    counts
}

fn live_session_record_key(record: &PersistedLiveSessionSignal, value: &[u8]) -> String {
    let (session_id, turn_id, discriminator) = record.signal.identity_parts();
    format!(
        "live_session::{session_id}::{turn_id}::{discriminator}::{:016x}",
        fnv1a64(value)
    )
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(
        field,
        value,
        "use a stable row id for raw-output directories",
    )?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("{field} must be a single safe path component, got {value:?}"),
            "use a stable row id without path separators for raw-output directories",
        );
    }
    Ok(())
}

fn to_u32(field: &'static str, value: usize) -> InstrumentResult<u32> {
    u32::try_from(value).map_err(|_| {
        InstrumentError::invalid(
            field,
            format!("{field} value {value} exceeds u32"),
            "shard live-session extraction before counter overflow",
        )
    })
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_f32_vector(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn write_json_readback<T: Serialize>(path: &Path, value: &T) -> InstrumentResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-live-session-signals-v1",
            err.to_string(),
            "ensure live-session raw output remains JSON-serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-live-session-signals-v1",
            err.to_string(),
            "ensure the raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_after_write",
            "python-live-session-signals-v1",
            err.to_string(),
            "verify live-session raw-output bytes after writing",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write",
            "python-live-session-signals-v1",
            "raw-output readback bytes differ from written payload",
            "do not advance reward-signal checkpoints until raw bytes match",
        ));
    }
    Ok(())
}

fn annotate_idx(err: InstrumentError, idx: usize) -> InstrumentError {
    InstrumentError::invalid(
        "source.collection[i]",
        format!("live-session row {idx} invalid: {err}"),
        "fix the live-session source-of-truth row before extraction",
    )
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_session_extracts_goal_and_session_state() {
        let extraction = extract_live_session_signals(&sample_source()).unwrap();
        let label = extraction.labels.first().unwrap();
        assert_eq!(label.prompt_event_count, 2);
        assert_eq!(label.prior_verdict_count, 4);
        assert_eq!(label.prior_fail_count, 1);
        assert_eq!(label.operator_override_count, 3);
        assert_eq!(label.refuted_override_count, 1);
        assert_eq!(label.goal_embedding_dim, 384);
        assert!(label.goal_embedding_sha256.starts_with("sha256:"));
        assert!(label
            .dominant_goal_terms
            .iter()
            .any(|term| term == "reward"));
        assert_eq!(extraction.persisted_records().len(), 9);
    }

    #[test]
    fn live_session_requires_prompt_history() {
        let mut source = sample_source();
        source.prompt_history.clear();
        let err = extract_live_session_signals(&source).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }

    fn sample_source() -> LiveSessionSource {
        LiveSessionSource {
            session_id: "session-rwd-207".to_string(),
            turn_id: "turn-0002".to_string(),
            prompt_history: vec![
                UserPromptEvent {
                    event_id: "prompt-1".to_string(),
                    unix_secs: 1_779_055_000,
                    prompt: "Read the GitHub issues and capture all reward signals for ME-JEPA."
                        .to_string(),
                },
                UserPromptEvent {
                    event_id: "prompt-2".to_string(),
                    unix_secs: 1_779_055_060,
                    prompt: "Finish the live-session reward signal with operator goal evidence."
                        .to_string(),
                },
            ],
            prior_verdicts: vec![
                verdict("pred-pass", SessionVerdictKind::Pass),
                verdict("pred-fail", SessionVerdictKind::Fail),
                verdict("pred-ood", SessionVerdictKind::Ood),
                verdict("pred-abstain", SessionVerdictKind::Abstain),
            ],
            operator_overrides: vec![
                override_row("pred-pass", OperatorOverrideKind::Confirmed),
                override_row("pred-fail", OperatorOverrideKind::Refuted),
                override_row("pred-ood", OperatorOverrideKind::Inconclusive),
            ],
        }
    }

    fn verdict(prediction_id: &str, verdict: SessionVerdictKind) -> SessionVerdictObservation {
        SessionVerdictObservation {
            prediction_id: prediction_id.to_string(),
            verdict,
            fingerprint_id: Some(format!("fp-{prediction_id}")),
        }
    }

    fn override_row(
        prediction_id: &str,
        outcome: OperatorOverrideKind,
    ) -> OperatorOverrideObservation {
        OperatorOverrideObservation {
            prediction_id: prediction_id.to_string(),
            outcome,
            unix_secs: 1_779_055_120,
            reason: format!("{prediction_id} operator feedback is durable reality"),
        }
    }
}
