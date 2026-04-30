use context_graph_core::dynamicjepa::{
    DynamicJepaResult, GuardDecisionId, GuardDecisionRecord, PlanTraceId, PlanTraceRecord, SkillId,
    SkillPolicyRecord,
};
use rocksdb::DB;

use crate::dynamicjepa::column_families::{
    CF_DJ_GUARD_DECISIONS, CF_DJ_PLAN_TRACES, CF_DJ_SKILL_POLICIES,
};
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

pub fn put_skill_policy(db: &DB, record: &SkillPolicyRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_SKILL_POLICIES,
        record.skill_id.into_bytes(),
        record,
    )
}

pub fn get_skill_policy(db: &DB, id: SkillId) -> DynamicJepaResult<Option<SkillPolicyRecord>> {
    get_record(db, CF_DJ_SKILL_POLICIES, id.into_bytes())
}

pub fn list_skill_policies(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<SkillPolicyRecord>> {
    list_records(db, CF_DJ_SKILL_POLICIES, limit, offset)
}

pub fn count_skill_policies(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_SKILL_POLICIES)
}

pub fn put_plan_trace(db: &DB, record: &PlanTraceRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_PLAN_TRACES,
        record.plan_trace_id.into_bytes(),
        record,
    )
}

pub fn get_plan_trace(db: &DB, id: PlanTraceId) -> DynamicJepaResult<Option<PlanTraceRecord>> {
    get_record(db, CF_DJ_PLAN_TRACES, id.into_bytes())
}

pub fn list_plan_traces(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<PlanTraceRecord>> {
    list_records(db, CF_DJ_PLAN_TRACES, limit, offset)
}

pub fn count_plan_traces(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_PLAN_TRACES)
}

pub fn put_guard_decision(db: &DB, record: &GuardDecisionRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_GUARD_DECISIONS,
        record.guard_decision_id.into_bytes(),
        record,
    )
}

pub fn get_guard_decision(
    db: &DB,
    id: GuardDecisionId,
) -> DynamicJepaResult<Option<GuardDecisionRecord>> {
    get_record(db, CF_DJ_GUARD_DECISIONS, id.into_bytes())
}

pub fn list_guard_decisions(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<GuardDecisionRecord>> {
    list_records(db, CF_DJ_GUARD_DECISIONS, limit, offset)
}

pub fn count_guard_decisions(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_GUARD_DECISIONS)
}
