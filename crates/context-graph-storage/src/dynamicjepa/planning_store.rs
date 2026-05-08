use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, GuardDecisionId, GuardDecisionRecord, NormalizedAction,
    PlanTraceId, PlanTraceRecord, PredictionRecord, SkillId, SkillPolicyRecord,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_GUARD_DECISIONS, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS,
    CF_DJ_SKILL_POLICIES,
};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;

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

pub fn put_plan_batch(
    db: &DB,
    skill_policy: Option<&SkillPolicyRecord>,
    actions: &[NormalizedAction],
    predictions: &[PredictionRecord],
    guard_decisions: &[GuardDecisionRecord],
    plan_trace: &PlanTraceRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    if let Some(skill_policy) = skill_policy {
        skill_policy.validate_record()?;
    }
    for action in actions {
        action.validate_record()?;
    }
    for prediction in predictions {
        prediction.validate_record()?;
    }
    for guard in guard_decisions {
        guard.validate_record()?;
    }
    plan_trace.validate_record()?;
    audit.validate()?;

    let mut batch = WriteBatch::default();
    if let Some(skill_policy) = skill_policy {
        batch.put_cf(
            cf(db, CF_DJ_SKILL_POLICIES)?,
            skill_policy.skill_id.into_bytes(),
            encode_record(skill_policy)?,
        );
    }
    for action in actions {
        batch.put_cf(
            cf(db, CF_DJ_ACTIONS)?,
            action.action_id.into_bytes(),
            encode_record(action)?,
        );
    }
    for prediction in predictions {
        batch.put_cf(
            cf(db, CF_DJ_PREDICTIONS)?,
            prediction.prediction_id.into_bytes(),
            encode_record(prediction)?,
        );
    }
    for guard in guard_decisions {
        batch.put_cf(
            cf(db, CF_DJ_GUARD_DECISIONS)?,
            guard.guard_decision_id.into_bytes(),
            encode_record(guard)?,
        );
    }
    batch.put_cf(
        cf(db, CF_DJ_PLAN_TRACES)?,
        plan_trace.plan_trace_id.into_bytes(),
        encode_record(plan_trace)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_plan_batch")
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
