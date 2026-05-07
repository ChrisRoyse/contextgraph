---
namespace: discoveries
agent_id: agent-09-simplifier-v2
timestamp: 2026-04-14T17:30:00Z
status: low
tags: constellation, tests, rebuild-orphan
severity: low
topic: missing-test-three-way-rebuild-chain-leaves-no-orphans
---

# Missing regression test: three-way rebuild chain A → B → C

## Scope
`crates/context-graph-storage/tests/constellation_integration.rs` (tests 11 + 12)

## Observation
The two new tests cover:
- **Test 11** (`rebuild_for_same_selector_atomically_deletes_old_primary`): A → B under the same selector. Primary CF ends with only B.
- **Test 12** (`restore_same_uuid_is_idempotent_and_keeps_the_row`): A → A (same UUID). Row stays.

These nail the two branches of the probe (`prior != new` and `prior == new`). The failure mode that is **not** exercised is: does the orphan-sweep generalize beyond one step? Three consecutive rebuilds under the same selector — A → B → C — should land exactly one row (C) with A and B both gone.

The current implementation trivially satisfies this because the probe reads the *current* index pointer each time (which always names the most recent prior UUID, not A). But if a future refactor ever caches the prior UUID across calls, or stops re-probing the index on each store, the chain case would regress silently: test 11 would still pass but A would leak.

This is pure regression insurance. The cost is ~15 lines.

## Severity
**low** — speculative protection, not a concrete defect. Worth adding because `tasks/lessons.md` already lists "secondary index drift on rebuild" as a past source of bugs.

## Minimal diff
Append to `constellation_integration.rs`:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn rebuild_chain_a_b_c_leaves_only_c() {
    let (_td, store) = open_store();
    let selector = ConstellationSelector::Session {
        session_id: "rebuild-chain-abc".into(),
    };

    let a = hand_built_constellation(selector.clone());
    let b = hand_built_constellation(selector.clone());
    let c = hand_built_constellation(selector.clone());
    let (a_id, b_id, c_id) = (a.id, b.id, c.id);
    assert!([a_id, b_id, c_id].iter().collect::<std::collections::HashSet<_>>().len() == 3);

    store.store_constellation(&a).await.unwrap();
    store.store_constellation(&b).await.unwrap();
    store.store_constellation(&c).await.unwrap();

    let ids = store.list_constellation_ids().await.unwrap();
    assert_eq!(
        ids, vec![c_id],
        "three-way rebuild chain must leave only C — A+B orphans are a regression"
    );
    assert!(store.get_constellation(a_id).await.unwrap().is_none());
    assert!(store.get_constellation(b_id).await.unwrap().is_none());
    assert_eq!(
        store
            .find_constellation_by_selector(&selector)
            .await
            .unwrap(),
        Some(c_id)
    );
}
```

## Verdict
Cheap to add; adds real coverage against a future refactor regression. Recommend for a follow-up PR, not a blocker.
