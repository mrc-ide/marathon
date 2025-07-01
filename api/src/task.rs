use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::ExitStatus;
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, Copy, Eq, PartialEq, sqlx::Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Generate a new random task ID.
    #[allow(clippy::new_without_default)]
    pub fn new() -> TaskId {
        TaskId(Uuid::now_v7())
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for TaskId {
    type Err = <Uuid as std::str::FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(TaskId)
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequest {
    pub cmdline: Vec<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug)]
pub struct TaskResult {
    pub status: ExitStatus,
    pub output: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, sqlx::FromRow)]
pub struct TaskInfo {
    pub id: TaskId,
    pub status: TaskStatus,
}

/// The different states a task may be in.
#[derive(Debug, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskStatus {
    Pending,
    Claimed,
    Running,
    Complete,
    Failed,
}

impl TaskStatus {
    /// Returns true if the task is finished and won't make any more progress.
    pub fn is_finished(self) -> bool {
        use TaskStatus::*;
        match self {
            Pending | Claimed | Running => false,
            Complete | Failed => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assertor::*;

    #[test]
    fn task_id_is_unique() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_that!(id1).is_not_equal_to(id2);
    }

    #[test]
    fn can_round_trip_task_id() {
        let id = TaskId::new();
        assert_that!(id.to_string().parse::<TaskId>())
            .ok()
            .is_equal_to(id);
    }
}
