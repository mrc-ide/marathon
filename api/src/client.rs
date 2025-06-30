use crate::messages::TaskSubmitResponse;
use crate::responses::ResponseExt;
use crate::{TaskInfo, TaskRequest};
use uuid::Uuid;

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

    pub fn task_submit(&self, request: &TaskRequest) -> crate::Result<Uuid> {
        let response = self
            .client
            .post(self.base_url.join("tasks")?)
            .json(request)
            .send()?
            .api_response::<TaskSubmitResponse>()?;

        Ok(response.id)
    }

    pub fn task_get(&self, id: Uuid) -> crate::Result<TaskInfo> {
        let response = self
            .client
            .get(self.base_url.join("tasks/")?.join(&id.to_string())?)
            .send()?
            .api_response::<TaskInfo>()?;

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
}
