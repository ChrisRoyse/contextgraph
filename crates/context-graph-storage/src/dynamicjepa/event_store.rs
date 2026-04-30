use context_graph_core::dynamicjepa::DynamicJepaResult;
use context_graph_core::dynamicjepa::{
    ActionId, AdapterRunRecord, EventId, NormalizedAction, NormalizedOutcome, NormalizedState,
    OutcomeId, RawDomainEvent, StateId, StateTransition, TransitionId,
};
use rocksdb::DB;
use uuid::Uuid;

use crate::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_RUNS, CF_DJ_NORMALIZED_STATES, CF_DJ_OUTCOMES, CF_DJ_RAW_EVENTS,
    CF_DJ_TRANSITIONS,
};
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

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
