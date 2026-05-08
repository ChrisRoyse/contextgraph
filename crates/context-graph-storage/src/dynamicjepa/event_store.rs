use context_graph_core::dynamicjepa::{
    ActionId, AdapterRunRecord, EventId, NormalizedAction, NormalizedOutcome, NormalizedState,
    OutcomeId, RawDomainEvent, StateId, StateTransition, TransitionId,
};
use context_graph_core::dynamicjepa::{DynamicJepaRecord, DynamicJepaResult};
use rocksdb::{WriteBatch, DB};
use uuid::Uuid;

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_RUNS, CF_DJ_NORMALIZED_STATES, CF_DJ_OUTCOMES, CF_DJ_RAW_EVENTS,
    CF_DJ_TRANSITIONS,
};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;

pub fn put_raw_event_and_adapter_run_started(
    db: &DB,
    raw_event: &RawDomainEvent,
    adapter_run: &AdapterRunRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    raw_event.validate_record()?;
    adapter_run.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_RAW_EVENTS)?,
        raw_event.event_id.into_bytes(),
        encode_record(raw_event)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_ADAPTER_RUNS)?,
        adapter_run.adapter_run_id.as_bytes(),
        encode_record(adapter_run)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_raw_event_and_adapter_run_started")
}

pub fn put_adapter_run_success_batch(
    db: &DB,
    adapter_run: &AdapterRunRecord,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    put_adapter_run_success_with_audits_batch(
        db,
        adapter_run,
        state,
        action,
        outcome,
        transition,
        std::slice::from_ref(audit),
    )
}

pub fn put_adapter_run_success_with_audits_batch(
    db: &DB,
    adapter_run: &AdapterRunRecord,
    state: &NormalizedState,
    action: &NormalizedAction,
    outcome: &NormalizedOutcome,
    transition: &StateTransition,
    audits: &[DjAuditRecord],
) -> DynamicJepaResult<()> {
    adapter_run.validate_record()?;
    state.validate_record()?;
    action.validate_record()?;
    outcome.validate_record()?;
    transition.validate_record()?;
    if audits.is_empty() {
        return Err(context_graph_core::dynamicjepa::DynamicJepaError::validation(
            "DjAuditRecord",
            "adapter success batch requires at least one audit row",
            "write run_adapter_success and compile_transition audit provenance with the same RocksDB batch",
        ));
    }
    for audit in audits {
        audit.validate()?;
    }
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_ADAPTER_RUNS)?,
        adapter_run.adapter_run_id.as_bytes(),
        encode_record(adapter_run)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_NORMALIZED_STATES)?,
        state.state_id.into_bytes(),
        encode_record(state)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_ACTIONS)?,
        action.action_id.into_bytes(),
        encode_record(action)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_OUTCOMES)?,
        outcome.outcome_id.into_bytes(),
        encode_record(outcome)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_TRANSITIONS)?,
        transition.transition_id.into_bytes(),
        encode_record(transition)?,
    );
    let audit_refs = audits.iter().collect::<Vec<_>>();
    write_batch_with_audit_witnesses(db, batch, &audit_refs, "put_adapter_run_success_batch")
}

pub fn put_adapter_run_failure_batch(
    db: &DB,
    adapter_run: &AdapterRunRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    adapter_run.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_ADAPTER_RUNS)?,
        adapter_run.adapter_run_id.as_bytes(),
        encode_record(adapter_run)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_adapter_run_failure_batch")
}

pub fn put_raw_event(db: &DB, record: &RawDomainEvent) -> DynamicJepaResult<()> {
    put_record(db, CF_DJ_RAW_EVENTS, record.event_id.into_bytes(), record)
}

pub fn get_raw_event(db: &DB, id: EventId) -> DynamicJepaResult<Option<RawDomainEvent>> {
    get_record(db, CF_DJ_RAW_EVENTS, id.into_bytes())
}

pub fn list_raw_events(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<RawDomainEvent>> {
    list_records(db, CF_DJ_RAW_EVENTS, limit, offset)
}

pub fn count_raw_events(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_RAW_EVENTS)
}

pub fn put_normalized_state(db: &DB, record: &NormalizedState) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_NORMALIZED_STATES,
        record.state_id.into_bytes(),
        record,
    )
}

pub fn get_normalized_state(db: &DB, id: StateId) -> DynamicJepaResult<Option<NormalizedState>> {
    get_record(db, CF_DJ_NORMALIZED_STATES, id.into_bytes())
}

pub fn list_normalized_states(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<NormalizedState>> {
    list_records(db, CF_DJ_NORMALIZED_STATES, limit, offset)
}

pub fn count_normalized_states(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_NORMALIZED_STATES)
}

pub fn put_action(db: &DB, record: &NormalizedAction) -> DynamicJepaResult<()> {
    put_record(db, CF_DJ_ACTIONS, record.action_id.into_bytes(), record)
}

pub fn get_action(db: &DB, id: ActionId) -> DynamicJepaResult<Option<NormalizedAction>> {
    get_record(db, CF_DJ_ACTIONS, id.into_bytes())
}

pub fn list_actions(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<NormalizedAction>> {
    list_records(db, CF_DJ_ACTIONS, limit, offset)
}

pub fn count_actions(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_ACTIONS)
}

pub fn put_outcome(db: &DB, record: &NormalizedOutcome) -> DynamicJepaResult<()> {
    put_record(db, CF_DJ_OUTCOMES, record.outcome_id.into_bytes(), record)
}

pub fn get_outcome(db: &DB, id: OutcomeId) -> DynamicJepaResult<Option<NormalizedOutcome>> {
    get_record(db, CF_DJ_OUTCOMES, id.into_bytes())
}

pub fn list_outcomes(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<NormalizedOutcome>> {
    list_records(db, CF_DJ_OUTCOMES, limit, offset)
}

pub fn count_outcomes(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_OUTCOMES)
}

pub fn put_transition(db: &DB, record: &StateTransition) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_TRANSITIONS,
        record.transition_id.into_bytes(),
        record,
    )
}

pub fn get_transition(db: &DB, id: TransitionId) -> DynamicJepaResult<Option<StateTransition>> {
    get_record(db, CF_DJ_TRANSITIONS, id.into_bytes())
}

pub fn list_transitions(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<StateTransition>> {
    list_records(db, CF_DJ_TRANSITIONS, limit, offset)
}

pub fn count_transitions(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_TRANSITIONS)
}

pub fn put_adapter_run(db: &DB, record: &AdapterRunRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_ADAPTER_RUNS,
        record.adapter_run_id.as_bytes(),
        record,
    )
}

pub fn get_adapter_run(db: &DB, id: Uuid) -> DynamicJepaResult<Option<AdapterRunRecord>> {
    get_record(db, CF_DJ_ADAPTER_RUNS, id.as_bytes())
}

pub fn list_adapter_runs(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<AdapterRunRecord>> {
    list_records(db, CF_DJ_ADAPTER_RUNS, limit, offset)
}

pub fn count_adapter_runs(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_ADAPTER_RUNS)
}
