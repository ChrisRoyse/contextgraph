use std::sync::Arc;
use std::time::Duration;

use rocksdb::DB;
use tokio::sync::watch;
use tokio::time::{interval, Interval, MissedTickBehavior};

use crate::heal::errors::HealError;
use crate::heal::scheduler_state::{SchedulerState, SelfOptimConfig, TickName};

pub async fn run_self_optimization_scheduler(
    db: Arc<DB>,
    config: SelfOptimConfig,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), HealError> {
    let mut state = SchedulerState::open(db, config)?;
    if *shutdown_rx.borrow() {
        state.mark_stopped()?;
        return Ok(());
    }

    let mut observe = new_interval(state.config().observe_period);
    let mut drift = new_interval(state.config().drift_period);
    let mut active_learning = new_interval(state.config().active_learning_period);
    let mut promote = new_interval(state.config().promote_period);
    let mut continual_backprop = new_interval(state.config().continual_backprop_period);
    let mut constellation = new_interval(state.config().constellation_period);
    let mut emergency_eviction = new_interval(state.config().emergency_eviction_period);

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    state.mark_stopped()?;
                    return Ok(());
                }
            }
            _ = observe.tick() => state.run_tick(TickName::Observe)?,
            _ = drift.tick() => state.run_tick(TickName::DriftCheck)?,
            _ = active_learning.tick() => state.run_tick(TickName::ActiveLearning)?,
            _ = promote.tick() => state.run_tick(TickName::Promote)?,
            _ = continual_backprop.tick() => state.run_tick(TickName::ContinualBackprop)?,
            _ = constellation.tick() => state.run_tick(TickName::ConstellationFreshness)?,
            _ = emergency_eviction.tick() => state.run_tick(TickName::EmergencyEviction)?,
        }
    }
}

fn new_interval(period: Duration) -> Interval {
    let mut timer = interval(period);
    timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::scheduler_state::read_status_snapshot;
    use rocksdb::{ColumnFamilyDescriptor, Options};

    fn open_test_db(path: &std::path::Path) -> Arc<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
            .into_iter()
            .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
            .collect::<Vec<_>>();
        Arc::new(DB::open_cf_descriptors(&opts, path, descriptors).expect("open test db"))
    }

    fn test_config(root: &std::path::Path, period: Duration) -> SelfOptimConfig {
        SelfOptimConfig {
            status_path: root.join("self_optimization_status.json"),
            hygiene_archive_root: root.join("archive"),
            witness_chain_path: root.join("witness-chain.bin"),
            ..SelfOptimConfig::default().with_all_periods(period)
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_scheduler_fires_all_tickers_and_persists_stopped_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path(), Duration::from_millis(20));
        let status_path = config.status_path.clone();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let scheduler = tokio::spawn(run_self_optimization_scheduler(db, config, shutdown_rx));
        tokio::time::sleep(Duration::from_millis(180)).await;
        shutdown_tx.send(true).expect("send shutdown");
        tokio::time::timeout(Duration::from_secs(5), scheduler)
            .await
            .expect("scheduler timeout")
            .expect("scheduler join")
            .expect("scheduler result");

        let snapshot = read_status_snapshot(&status_path).expect("read scheduler status");
        assert_eq!(snapshot.status, "stopped");
        for tick in TickName::all() {
            assert!(
                snapshot
                    .ticker_counts
                    .get(tick.as_str())
                    .copied()
                    .unwrap_or_default()
                    > 0,
                "{} did not tick",
                tick.as_str()
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_requested_during_active_loop_stops_after_persisting_current_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path(), Duration::from_millis(10));
        let status_path = config.status_path.clone();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let scheduler = tokio::spawn(run_self_optimization_scheduler(db, config, shutdown_rx));
        tokio::time::sleep(Duration::from_millis(25)).await;
        let before = read_status_snapshot(&status_path).expect("read before shutdown");
        shutdown_tx.send(true).expect("send shutdown");
        tokio::time::timeout(Duration::from_secs(5), scheduler)
            .await
            .expect("scheduler timeout")
            .expect("scheduler join")
            .expect("scheduler result");
        let after = read_status_snapshot(&status_path).expect("read after shutdown");
        assert_eq!(after.status, "stopped");
        assert!(after.ticker_counts.values().sum::<u64>() >= before.ticker_counts.values().sum());
    }
}
