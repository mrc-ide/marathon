use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskSubmitResponse {
    pub id: Uuid,
}
