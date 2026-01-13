use reqwest::RequestBuilder;
use std::fmt::Display;

pub fn create_client(base_url: String) -> RestClient {
    RestClient {
        client: reqwest::Client::new(),
        base_url,
    }
}

#[derive(Debug, Clone)]
pub struct RestClient {
    client: reqwest::Client,
    base_url: String,
}

#[allow(dead_code)]
impl RestClient {
    pub fn get(&self, path: impl Display) -> RequestBuilder {
        self.client.get(format!("{}/{}", self.base_url, path))
    }

    pub fn put(&self, path: impl Display) -> RequestBuilder {
        self.client.put(format!("{}/{}", self.base_url, path))
    }

    pub fn post(&self, path: impl Display) -> RequestBuilder {
        self.client.post(format!("{}/{}", self.base_url, path))
    }
}
