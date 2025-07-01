use crate::messages::{ApiResponse, TaskSubmitResponse};
use crate::TaskRequest;
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

    pub fn submit(&self, request: &TaskRequest) -> crate::Result<Uuid> {
        let response = self
            .client
            .post(self.base_url.join("task")?)
            .json(request)
            .send()?
            .error_for_status()?
            .json::<ApiResponse<TaskSubmitResponse>>()?;

        let ApiResponse::Success { data } = response;

        Ok(data.id)
    }
}
