use crate::task::{TaskId, TaskRequest, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskSubmitResponse {
    pub id: TaskId,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct QueueUpdateRequest {
    pub task: TaskId,
    pub status: TaskStatus,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct QueuePullResponseInner {
    pub task: TaskId,
    pub request: TaskRequest,
}

pub type QueuePullResponse = Option<QueuePullResponseInner>;
