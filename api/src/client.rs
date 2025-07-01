use crate::messages::{
    QueuePullResponse, QueuePullResponseInner, QueueUpdateRequest, TaskSubmitResponse,
};
use crate::responses::ResponseExt;
use crate::task::{TaskId, TaskInfo, TaskRequest, TaskStatus};
use anyhow::anyhow;
use reqwest::StatusCode;
use std::time::Duration;

pub struct Client {
    base_url: reqwest::Url,
    client: reqwest::blocking::Client,
}

impl Client {
    pub fn new(url: reqwest::Url) -> Client {
        Client {
            base_url: url,
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn url(&self) -> &reqwest::Url {
        &self.base_url
    }

    pub fn task_submit(&self, request: &TaskRequest) -> crate::Result<TaskId> {
        let response = self
            .client
            .post(self.base_url.join("tasks")?)
            .json(request)
            .send()?
            .api_response::<TaskSubmitResponse>()?;

        Ok(response.id)
    }

    pub fn task_get(&self, id: TaskId) -> crate::Result<TaskInfo> {
        let response = self
            .client
            .get(self.base_url.join("tasks/")?.join(&id.to_string())?)
            .send()?
            .api_response::<TaskInfo>()
            .map_err(|e| {
                if e.has_status(StatusCode::NOT_FOUND) {
                    anyhow!("Unknown task {id}")
                } else {
                    e.into()
                }
            })?;

        Ok(response)
    }

    pub fn task_list(&self) -> crate::Result<Vec<TaskInfo>> {
        let response = self
            .client
            .get(self.base_url.join("tasks")?)
            .send()?
            .api_response::<Vec<TaskInfo>>()?;

        Ok(response)
    }

    /// Pull an item from the server's work queue.
    pub fn queue_pull(&self) -> crate::Result<Result<QueuePullResponseInner, Option<Duration>>> {
        let response = self.client.post(self.base_url.join("queue/pull")?).send()?;

        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .cloned();

        match response.api_response::<QueuePullResponse>()? {
            Some(inner) => Ok(Ok(inner)),
            None => {
                let interval = retry_after
                    .and_then(|v| v.to_str().ok().and_then(|i| i.parse::<u64>().ok()))
                    .map(Duration::from_secs);
                Ok(Err(interval))
            }
        }
    }

    pub fn queue_update(&self, task: TaskId, status: TaskStatus) -> crate::Result<()> {
        self.client
            .post(self.base_url.join("queue/task")?)
            .json(&QueueUpdateRequest { task, status })
            .send()?
            .api_response::<()>()?;
        Ok(())
    }
}
