use crate::TaskRequest;

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

    pub fn submit(&self, request: &TaskRequest) -> crate::Result<()> {
        self.client
            .post(self.base_url.join("task")?)
            .json(request)
            .send()?
            .error_for_status()?;

        Ok(())
    }
}
