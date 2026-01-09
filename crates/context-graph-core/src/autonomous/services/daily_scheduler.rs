//! Daily Scheduler Service (NORTH-018)
//!
//! Manages autonomous daily task scheduling for the North Star system.
//! Schedules and executes recurring tasks based on configured hours and windows.

use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::autonomous::{DailySchedule, ScheduledCheckType};

/// Extended scheduled check type with custom task support
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SchedulerCheckType {
    /// Daily drift check
    DriftCheck,
    /// Consolidation window for heavy operations
    Consolidation,
    /// Statistics collection and reporting
    StatsReport,
    /// Evening preparation/optimization
    Prep,
    /// Custom named task
    Custom(String),
}

impl From<ScheduledCheckType> for SchedulerCheckType {
    fn from(check: ScheduledCheckType) -> Self {
        match check {
            ScheduledCheckType::DriftCheck => SchedulerCheckType::DriftCheck,
            ScheduledCheckType::ConsolidationWindow => SchedulerCheckType::Consolidation,
            ScheduledCheckType::StatisticsCollection => SchedulerCheckType::StatsReport,
            ScheduledCheckType::IndexOptimization => SchedulerCheckType::Prep,
        }
    }
}

/// A scheduled task with timing information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Type of check to perform
    pub check_type: SchedulerCheckType,
    /// Hour of day (0-23) when this task is scheduled
    pub scheduled_hour: u32,
    /// Last time this task was run
    pub last_run: Option<DateTime<Utc>>,
    /// Next scheduled run time
    pub next_run: Option<DateTime<Utc>>,
    /// Whether this task is enabled
    pub enabled: bool,
}

impl ScheduledTask {
    /// Create a new scheduled task
    pub fn new(check_type: SchedulerCheckType, hour: u32) -> Self {
        assert!(hour <= 23, "Hour must be 0-23, got {}", hour);
        Self {
            check_type,
            scheduled_hour: hour,
            last_run: None,
            next_run: None,
            enabled: true,
        }
    }

    /// Check if this task is due at the given hour
    pub fn is_due(&self, current_hour: u32) -> bool {
        self.enabled && self.scheduled_hour == current_hour
    }

    /// Mark task as completed
    pub fn mark_completed(&mut self) {
        self.last_run = Some(Utc::now());
    }
}

/// Result of executing scheduled tasks
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScheduleResult {
    /// Tasks that were executed
    pub executed_tasks: Vec<SchedulerCheckType>,
    /// Tasks that were skipped (disabled or not due)
    pub skipped_tasks: Vec<SchedulerCheckType>,
    /// Next scheduled task time
    pub next_scheduled: Option<DateTime<Utc>>,
}

/// Daily scheduler service for autonomous task management
#[derive(Clone, Debug)]
pub struct DailyScheduler {
    /// Configured daily schedule (reserved for future runtime schedule modification)
    #[allow(dead_code)]
    schedule: DailySchedule,
    /// All registered tasks
    tasks: HashMap<SchedulerCheckType, ScheduledTask>,
    /// Consolidation window (start_hour, end_hour)
    consolidation_window: (u32, u32),
}

impl Default for DailyScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl DailyScheduler {
    /// Create a new DailyScheduler with default schedule
    pub fn new() -> Self {
        let schedule = DailySchedule::default();
        Self::with_schedule(schedule)
    }

    /// Create a DailyScheduler with custom schedule configuration
    pub fn with_config(schedule: DailySchedule) -> Self {
        Self::with_schedule(schedule)
    }

    /// Internal constructor with schedule
    fn with_schedule(schedule: DailySchedule) -> Self {
        let consolidation_window = schedule.consolidation_window;
        let mut scheduler = Self {
            schedule: schedule.clone(),
            tasks: HashMap::new(),
            consolidation_window,
        };

        // Register default tasks from schedule
        scheduler.schedule_task(SchedulerCheckType::DriftCheck, schedule.drift_check_hour);
        scheduler.schedule_task(SchedulerCheckType::StatsReport, schedule.stats_hour);
        scheduler.schedule_task(SchedulerCheckType::Prep, schedule.prep_hour);

        // Register consolidation task at the start of the window
        scheduler.schedule_task(SchedulerCheckType::Consolidation, consolidation_window.0);

        scheduler
    }

    /// Get all tasks that are due at the current hour
    pub fn get_due_tasks(&self, current_hour: u32) -> Vec<ScheduledTask> {
        assert!(
            current_hour <= 23,
            "Hour must be 0-23, got {}",
            current_hour
        );

        self.tasks
            .values()
            .filter(|task| {
                if !task.enabled {
                    return false;
                }

                // Special handling for consolidation - due during entire window
                if task.check_type == SchedulerCheckType::Consolidation {
                    return self.is_in_window(current_hour, self.consolidation_window);
                }

                task.scheduled_hour == current_hour
            })
            .cloned()
            .collect()
    }

    /// Schedule a new task at the specified hour
    pub fn schedule_task(&mut self, check_type: SchedulerCheckType, hour: u32) {
        assert!(hour <= 23, "Hour must be 0-23, got {}", hour);

        let task = ScheduledTask::new(check_type.clone(), hour);
        self.tasks.insert(check_type, task);
    }

    /// Execute all tasks that are due at the current hour
    pub fn execute_due_tasks(&mut self, current_hour: u32) -> ScheduleResult {
        assert!(
            current_hour <= 23,
            "Hour must be 0-23, got {}",
            current_hour
        );

        let mut result = ScheduleResult::default();

        // Collect due tasks first to avoid borrow issues
        let due_types: Vec<SchedulerCheckType> = self
            .tasks
            .values()
            .filter(|task| {
                if !task.enabled {
                    result.skipped_tasks.push(task.check_type.clone());
                    return false;
                }

                if task.check_type == SchedulerCheckType::Consolidation {
                    return self.is_in_window(current_hour, self.consolidation_window);
                }

                task.scheduled_hour == current_hour
            })
            .map(|t| t.check_type.clone())
            .collect();

        // Execute and mark completed
        for check_type in due_types {
            if let Some(task) = self.tasks.get_mut(&check_type) {
                task.mark_completed();
                result.executed_tasks.push(check_type);
            }
        }

        // Find next scheduled task
        result.next_scheduled = self.calculate_next_scheduled(current_hour);

        result
    }

    /// Check if an hour falls within a window (handles wrap-around)
    pub fn is_in_window(&self, hour: u32, window: (u32, u32)) -> bool {
        let (start, end) = window;
        if start <= end {
            hour >= start && hour < end
        } else {
            // Wrap-around case (e.g., 22 to 2)
            hour >= start || hour < end
        }
    }

    /// Mark a specific task as completed
    pub fn mark_completed(&mut self, task: &ScheduledTask) {
        if let Some(stored_task) = self.tasks.get_mut(&task.check_type) {
            stored_task.mark_completed();
        }
    }

    /// Get the next scheduled task
    pub fn get_next_scheduled(&self) -> Option<ScheduledTask> {
        let now = Utc::now();
        let current_hour = now.time().hour();

        // Find the task with the nearest scheduled hour
        self.tasks
            .values()
            .filter(|t| t.enabled)
            .min_by_key(|t| {
                // Calculate hours until this task
                let task_hour = t.scheduled_hour;
                if task_hour > current_hour {
                    task_hour - current_hour
                } else if task_hour < current_hour {
                    24 - current_hour + task_hour
                } else {
                    // Same hour - if not run today, it's next
                    if t.last_run
                        .map_or(true, |lr| lr.date_naive() != now.date_naive())
                    {
                        0
                    } else {
                        24
                    }
                }
            })
            .cloned()
    }

    /// Disable a task by check type
    pub fn disable_task(&mut self, check_type: SchedulerCheckType) {
        if let Some(task) = self.tasks.get_mut(&check_type) {
            task.enabled = false;
        }
    }

    /// Enable a task by check type
    pub fn enable_task(&mut self, check_type: SchedulerCheckType) {
        if let Some(task) = self.tasks.get_mut(&check_type) {
            task.enabled = true;
        }
    }

    /// Calculate next scheduled time from current hour
    fn calculate_next_scheduled(&self, current_hour: u32) -> Option<DateTime<Utc>> {
        let now = Utc::now();

        self.tasks
            .values()
            .filter(|t| t.enabled && t.scheduled_hour != current_hour)
            .min_by_key(|t| {
                if t.scheduled_hour > current_hour {
                    t.scheduled_hour - current_hour
                } else {
                    24 - current_hour + t.scheduled_hour
                }
            })
            .map(|t| {
                // Calculate DateTime for next scheduled
                let hours_until = if t.scheduled_hour > current_hour {
                    t.scheduled_hour - current_hour
                } else {
                    24 - current_hour + t.scheduled_hour
                };
                now + chrono::Duration::hours(hours_until as i64)
            })
    }

    /// Get all registered tasks
    pub fn get_all_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    /// Get a specific task by check type
    pub fn get_task(&self, check_type: &SchedulerCheckType) -> Option<&ScheduledTask> {
        self.tasks.get(check_type)
    }

    /// Check if the scheduler has a specific task type
    pub fn has_task(&self, check_type: &SchedulerCheckType) -> bool {
        self.tasks.contains_key(check_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_new() {
        let scheduler = DailyScheduler::new();

        // Should have default tasks registered
        assert!(scheduler.has_task(&SchedulerCheckType::DriftCheck));
        assert!(scheduler.has_task(&SchedulerCheckType::StatsReport));
        assert!(scheduler.has_task(&SchedulerCheckType::Prep));
        assert!(scheduler.has_task(&SchedulerCheckType::Consolidation));

        println!("[PASS] test_scheduler_new");
    }

    #[test]
    fn test_scheduler_with_config() {
        let schedule = DailySchedule {
            consolidation_window: (1, 4),
            drift_check_hour: 8,
            stats_hour: 14,
            prep_hour: 20,
        };

        let scheduler = DailyScheduler::with_config(schedule);

        let drift_task = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert_eq!(drift_task.scheduled_hour, 8);

        let stats_task = scheduler
            .get_task(&SchedulerCheckType::StatsReport)
            .unwrap();
        assert_eq!(stats_task.scheduled_hour, 14);

        let prep_task = scheduler.get_task(&SchedulerCheckType::Prep).unwrap();
        assert_eq!(prep_task.scheduled_hour, 20);

        println!("[PASS] test_scheduler_with_config");
    }

    #[test]
    fn test_scheduled_task_new() {
        let task = ScheduledTask::new(SchedulerCheckType::DriftCheck, 6);

        assert_eq!(task.check_type, SchedulerCheckType::DriftCheck);
        assert_eq!(task.scheduled_hour, 6);
        assert!(task.last_run.is_none());
        assert!(task.next_run.is_none());
        assert!(task.enabled);

        println!("[PASS] test_scheduled_task_new");
    }

    #[test]
    #[should_panic(expected = "Hour must be 0-23")]
    fn test_scheduled_task_invalid_hour() {
        ScheduledTask::new(SchedulerCheckType::DriftCheck, 25);
    }

    #[test]
    fn test_scheduled_task_is_due() {
        let task = ScheduledTask::new(SchedulerCheckType::DriftCheck, 6);

        assert!(task.is_due(6));
        assert!(!task.is_due(5));
        assert!(!task.is_due(7));

        println!("[PASS] test_scheduled_task_is_due");
    }

    #[test]
    fn test_scheduled_task_disabled_not_due() {
        let mut task = ScheduledTask::new(SchedulerCheckType::DriftCheck, 6);
        task.enabled = false;

        assert!(!task.is_due(6));

        println!("[PASS] test_scheduled_task_disabled_not_due");
    }

    #[test]
    fn test_scheduled_task_mark_completed() {
        let mut task = ScheduledTask::new(SchedulerCheckType::DriftCheck, 6);
        assert!(task.last_run.is_none());

        task.mark_completed();
        assert!(task.last_run.is_some());

        println!("[PASS] test_scheduled_task_mark_completed");
    }

    #[test]
    fn test_get_due_tasks_at_scheduled_hour() {
        let scheduler = DailyScheduler::new();

        // Default drift check is at hour 6
        let due = scheduler.get_due_tasks(6);
        assert!(!due.is_empty());
        assert!(due
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::DriftCheck));

        println!("[PASS] test_get_due_tasks_at_scheduled_hour");
    }

    #[test]
    fn test_get_due_tasks_at_non_scheduled_hour() {
        let scheduler = DailyScheduler::new();

        // Hour 10 has no default tasks
        let due = scheduler.get_due_tasks(10);
        assert!(due.is_empty());

        println!("[PASS] test_get_due_tasks_at_non_scheduled_hour");
    }

    #[test]
    fn test_get_due_tasks_consolidation_window() {
        let scheduler = DailyScheduler::new();

        // Default consolidation window is (0, 2)
        let due_0 = scheduler.get_due_tasks(0);
        assert!(due_0
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::Consolidation));

        let due_1 = scheduler.get_due_tasks(1);
        assert!(due_1
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::Consolidation));

        // Hour 2 is outside the window (window is [0, 2))
        let due_2 = scheduler.get_due_tasks(2);
        assert!(!due_2
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::Consolidation));

        println!("[PASS] test_get_due_tasks_consolidation_window");
    }

    #[test]
    fn test_schedule_custom_task() {
        let mut scheduler = DailyScheduler::new();

        let custom_type = SchedulerCheckType::Custom("backup".to_string());
        scheduler.schedule_task(custom_type.clone(), 3);

        assert!(scheduler.has_task(&custom_type));
        let task = scheduler.get_task(&custom_type).unwrap();
        assert_eq!(task.scheduled_hour, 3);

        println!("[PASS] test_schedule_custom_task");
    }

    #[test]
    fn test_execute_due_tasks() {
        let mut scheduler = DailyScheduler::new();

        // Execute at drift check hour
        let result = scheduler.execute_due_tasks(6);

        assert!(!result.executed_tasks.is_empty());
        assert!(result
            .executed_tasks
            .contains(&SchedulerCheckType::DriftCheck));

        // Verify task was marked completed
        let task = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert!(task.last_run.is_some());

        println!("[PASS] test_execute_due_tasks");
    }

    #[test]
    fn test_execute_due_tasks_returns_next_scheduled() {
        let mut scheduler = DailyScheduler::new();

        let result = scheduler.execute_due_tasks(6);

        // There should be a next scheduled task
        assert!(result.next_scheduled.is_some());

        println!("[PASS] test_execute_due_tasks_returns_next_scheduled");
    }

    #[test]
    fn test_is_in_window_normal() {
        let scheduler = DailyScheduler::new();

        // Window (2, 5) - normal case
        assert!(scheduler.is_in_window(2, (2, 5)));
        assert!(scheduler.is_in_window(3, (2, 5)));
        assert!(scheduler.is_in_window(4, (2, 5)));
        assert!(!scheduler.is_in_window(5, (2, 5)));
        assert!(!scheduler.is_in_window(1, (2, 5)));

        println!("[PASS] test_is_in_window_normal");
    }

    #[test]
    fn test_is_in_window_wraparound() {
        let scheduler = DailyScheduler::new();

        // Window (22, 2) - wraps around midnight
        assert!(scheduler.is_in_window(22, (22, 2)));
        assert!(scheduler.is_in_window(23, (22, 2)));
        assert!(scheduler.is_in_window(0, (22, 2)));
        assert!(scheduler.is_in_window(1, (22, 2)));
        assert!(!scheduler.is_in_window(2, (22, 2)));
        assert!(!scheduler.is_in_window(21, (22, 2)));

        println!("[PASS] test_is_in_window_wraparound");
    }

    #[test]
    fn test_mark_completed() {
        let mut scheduler = DailyScheduler::new();

        let task = scheduler
            .get_task(&SchedulerCheckType::DriftCheck)
            .unwrap()
            .clone();
        assert!(task.last_run.is_none());

        scheduler.mark_completed(&task);

        let updated = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert!(updated.last_run.is_some());

        println!("[PASS] test_mark_completed");
    }

    #[test]
    fn test_get_next_scheduled() {
        let scheduler = DailyScheduler::new();

        let next = scheduler.get_next_scheduled();
        assert!(next.is_some());

        println!("[PASS] test_get_next_scheduled");
    }

    #[test]
    fn test_disable_task() {
        let mut scheduler = DailyScheduler::new();

        // Verify task is enabled
        let task = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert!(task.enabled);

        // Disable it
        scheduler.disable_task(SchedulerCheckType::DriftCheck);

        let task = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert!(!task.enabled);

        // Should not be in due tasks
        let due = scheduler.get_due_tasks(6);
        assert!(!due
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::DriftCheck));

        println!("[PASS] test_disable_task");
    }

    #[test]
    fn test_enable_task() {
        let mut scheduler = DailyScheduler::new();

        // Disable then enable
        scheduler.disable_task(SchedulerCheckType::DriftCheck);
        scheduler.enable_task(SchedulerCheckType::DriftCheck);

        let task = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert!(task.enabled);

        println!("[PASS] test_enable_task");
    }

    #[test]
    fn test_schedule_result_default() {
        let result = ScheduleResult::default();

        assert!(result.executed_tasks.is_empty());
        assert!(result.skipped_tasks.is_empty());
        assert!(result.next_scheduled.is_none());

        println!("[PASS] test_schedule_result_default");
    }

    #[test]
    fn test_scheduler_check_type_from_scheduled_check_type() {
        let drift = SchedulerCheckType::from(ScheduledCheckType::DriftCheck);
        assert_eq!(drift, SchedulerCheckType::DriftCheck);

        let consolidation = SchedulerCheckType::from(ScheduledCheckType::ConsolidationWindow);
        assert_eq!(consolidation, SchedulerCheckType::Consolidation);

        let stats = SchedulerCheckType::from(ScheduledCheckType::StatisticsCollection);
        assert_eq!(stats, SchedulerCheckType::StatsReport);

        let prep = SchedulerCheckType::from(ScheduledCheckType::IndexOptimization);
        assert_eq!(prep, SchedulerCheckType::Prep);

        println!("[PASS] test_scheduler_check_type_from_scheduled_check_type");
    }

    #[test]
    fn test_get_all_tasks() {
        let scheduler = DailyScheduler::new();

        let tasks = scheduler.get_all_tasks();
        assert_eq!(tasks.len(), 4); // Drift, Stats, Prep, Consolidation

        println!("[PASS] test_get_all_tasks");
    }

    #[test]
    fn test_has_task() {
        let scheduler = DailyScheduler::new();

        assert!(scheduler.has_task(&SchedulerCheckType::DriftCheck));
        assert!(!scheduler.has_task(&SchedulerCheckType::Custom("nonexistent".to_string())));

        println!("[PASS] test_has_task");
    }

    #[test]
    fn test_multiple_tasks_same_hour() {
        let mut scheduler = DailyScheduler::new();

        // Add custom task at same hour as drift check
        let custom = SchedulerCheckType::Custom("custom_at_6".to_string());
        scheduler.schedule_task(custom.clone(), 6);

        let due = scheduler.get_due_tasks(6);
        assert!(due.len() >= 2);
        assert!(due
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::DriftCheck));
        assert!(due.iter().any(|t| t.check_type == custom));

        println!("[PASS] test_multiple_tasks_same_hour");
    }

    #[test]
    fn test_consolidation_window_wraparound() {
        let schedule = DailySchedule {
            consolidation_window: (22, 2),
            drift_check_hour: 6,
            stats_hour: 12,
            prep_hour: 18,
        };

        let scheduler = DailyScheduler::with_config(schedule);

        // Should be due during entire window
        for hour in [22, 23, 0, 1] {
            let due = scheduler.get_due_tasks(hour);
            assert!(
                due.iter()
                    .any(|t| t.check_type == SchedulerCheckType::Consolidation),
                "Consolidation should be due at hour {}",
                hour
            );
        }

        // Should not be due outside window
        let due = scheduler.get_due_tasks(2);
        assert!(!due
            .iter()
            .any(|t| t.check_type == SchedulerCheckType::Consolidation));

        println!("[PASS] test_consolidation_window_wraparound");
    }

    #[test]
    fn test_execute_with_disabled_tasks() {
        let mut scheduler = DailyScheduler::new();

        // Disable drift check
        scheduler.disable_task(SchedulerCheckType::DriftCheck);

        let result = scheduler.execute_due_tasks(6);

        // Drift check should be in skipped, not executed
        assert!(!result
            .executed_tasks
            .contains(&SchedulerCheckType::DriftCheck));
        assert!(result
            .skipped_tasks
            .contains(&SchedulerCheckType::DriftCheck));

        println!("[PASS] test_execute_with_disabled_tasks");
    }

    #[test]
    fn test_scheduled_task_serialization() {
        let task = ScheduledTask::new(SchedulerCheckType::DriftCheck, 6);

        let json = serde_json::to_string(&task).expect("serialize");
        let deserialized: ScheduledTask = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.check_type, task.check_type);
        assert_eq!(deserialized.scheduled_hour, task.scheduled_hour);
        assert_eq!(deserialized.enabled, task.enabled);

        println!("[PASS] test_scheduled_task_serialization");
    }

    #[test]
    fn test_schedule_result_serialization() {
        let result = ScheduleResult {
            executed_tasks: vec![SchedulerCheckType::DriftCheck],
            skipped_tasks: vec![SchedulerCheckType::Prep],
            next_scheduled: Some(Utc::now()),
        };

        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: ScheduleResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.executed_tasks.len(), 1);
        assert_eq!(deserialized.skipped_tasks.len(), 1);
        assert!(deserialized.next_scheduled.is_some());

        println!("[PASS] test_schedule_result_serialization");
    }

    #[test]
    fn test_custom_check_type_serialization() {
        let custom = SchedulerCheckType::Custom("my_task".to_string());

        let json = serde_json::to_string(&custom).expect("serialize");
        let deserialized: SchedulerCheckType = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized, custom);

        println!("[PASS] test_custom_check_type_serialization");
    }

    #[test]
    #[should_panic(expected = "Hour must be 0-23")]
    fn test_get_due_tasks_invalid_hour() {
        let scheduler = DailyScheduler::new();
        scheduler.get_due_tasks(24);
    }

    #[test]
    #[should_panic(expected = "Hour must be 0-23")]
    fn test_execute_due_tasks_invalid_hour() {
        let mut scheduler = DailyScheduler::new();
        scheduler.execute_due_tasks(25);
    }

    #[test]
    #[should_panic(expected = "Hour must be 0-23")]
    fn test_schedule_task_invalid_hour() {
        let mut scheduler = DailyScheduler::new();
        scheduler.schedule_task(SchedulerCheckType::DriftCheck, 30);
    }

    #[test]
    fn test_scheduler_default_schedule_values() {
        let scheduler = DailyScheduler::new();

        // Verify default schedule values from DailySchedule
        let drift = scheduler.get_task(&SchedulerCheckType::DriftCheck).unwrap();
        assert_eq!(drift.scheduled_hour, 6);

        let stats = scheduler
            .get_task(&SchedulerCheckType::StatsReport)
            .unwrap();
        assert_eq!(stats.scheduled_hour, 12);

        let prep = scheduler.get_task(&SchedulerCheckType::Prep).unwrap();
        assert_eq!(prep.scheduled_hour, 18);

        let consolidation = scheduler
            .get_task(&SchedulerCheckType::Consolidation)
            .unwrap();
        assert_eq!(consolidation.scheduled_hour, 0);

        println!("[PASS] test_scheduler_default_schedule_values");
    }

    #[test]
    fn test_execute_no_tasks_due() {
        let mut scheduler = DailyScheduler::new();

        // Hour 10 has no default tasks
        let result = scheduler.execute_due_tasks(10);

        assert!(result.executed_tasks.is_empty());
        assert!(result.next_scheduled.is_some());

        println!("[PASS] test_execute_no_tasks_due");
    }
}
