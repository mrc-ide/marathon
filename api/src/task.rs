use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::ExitStatus;
use uuid::Uuid;

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
    pub id: Uuid,
    pub status: TaskStatus,
}

/// The different states a task may be in.
#[derive(Debug, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

impl TaskStatus {
    /// Returns true if the task is finished and won't make any more progress.
    pub fn is_finished(self) -> bool {
        use TaskStatus::*;
        match self {
            Pending | Running => false,
            Complete | Failed => true,
        }
    }
}
