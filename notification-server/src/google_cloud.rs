use crate::models::NotificationInformation;
use anyhow::anyhow;
use backoff::{future::retry, ExponentialBackoff};
use gcp_auth::TokenProvider;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::collections::HashMap;

const SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/firebase.messaging"];

#[derive(Debug)]
pub struct GoogleCloud<T>
where
    T: TokenProvider, {
    client:          Client,
    service_account: T,
    url:             String,
    backoff_policy:  ExponentialBackoff,
}

impl<T> GoogleCloud<T>
where
    T: TokenProvider,
{
    pub fn new(
        client: Client,
        backoff_policy: ExponentialBackoff,
        service_account: T,
        project_id: &str,
    ) -> anyhow::Result<Self> {
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            project_id
        );
        Ok(Self {
            client,
            service_account,
            url,
            backoff_policy,
        })
    }

    pub async fn validate_device_token(&self, device_token: &str) -> anyhow::Result<bool> {
        self.send_push_notification_with_validate(device_token, None)
            .await
    }

    pub async fn send_push_notification(
        &self,
        device_token: &str,
        information: NotificationInformation,
    ) -> anyhow::Result<()> {
        self.send_push_notification_with_validate(device_token, Some(information))
            .await
            .map(|_| ())
    }

    async fn send_push_notification_with_validate(
        &self,
        device_token: &str,
        information: Option<NotificationInformation>,
    ) -> anyhow::Result<bool> {
        let access_token = self.service_account.token(SCOPES).await?;
        let mut payload = json!({});
        if Option::is_none(&information) {
            payload["validate_only"] = json!(true);
        }
        let entity_data: HashMap<String, String> = if let Some(information) = information {
            information.into_hashmap()
        } else {
            HashMap::new()
        };
        payload["message"] = json!({
            "token": device_token,
            "data": entity_data
        });

        let operation = || async {
            let response = self
                .client
                .post(&self.url)
                .bearer_auth(access_token.as_str())
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(res) if res.status().is_success() => Ok(true),
                Ok(res) if res.status() == StatusCode::BAD_REQUEST => {
                    match res.json::<Value>().await {
                        Ok(content) => {
                            if content["error"]["status"] == "INVALID_ARGUMENT" {
                                Ok(false)
                            } else {
                                Err(backoff::Error::permanent(anyhow!(
                                    "Bad request sent to Google API: {}",
                                    &payload
                                )))
                            }
                        }
                        Err(err) => Err(backoff::Error::permanent(anyhow!(
                            "Content returned from Google API is not valid json: {}",
                            err
                        ))),
                    }
                }
                Ok(res) => {
                    let status = res.status();
                    let error_text = res.text().await.unwrap_or_default();
                    if status.is_server_error() {
                        Err(backoff::Error::transient(anyhow!(
                            "Google API responded with server error: {} with text {}",
                            status,
                            error_text
                        )))
                    } else {
                        Err(backoff::Error::permanent(anyhow!(
                            "Google API responded with client error: {} with text {}",
                            status,
                            error_text
                        )))
                    }
                }
                Err(e) => Err(backoff::Error::transient(anyhow!("Network error: {}", e))),
            }
        };
        retry(self.backoff_policy.clone(), operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use backoff::{Clock, ExponentialBackoff};
    use gcp_auth::Token;
    use reqwest::Client;
    use std::sync::Arc;

    pub struct MockTokenProvider {
        pub token_response: Arc<String>,
    }

    use std::time::{Duration, Instant};

    fn generate_mock_token() -> Token {
        let json_data = json!({
            "access_token": "abc123xyz",
            "expires_in": 3600
        });

        let token: Token = serde_json::from_value(json_data).unwrap();
        token
    }

    #[async_trait]
    impl TokenProvider for MockTokenProvider {
        async fn token(&self, _scopes: &[&str]) -> Result<Arc<Token>, gcp_auth::Error> {
            Ok(Arc::new(generate_mock_token()))
        }

        async fn project_id(&self) -> Result<Arc<str>, gcp_auth::Error> {
            Err(gcp_auth::Error::Str(
                "Project id cannot be called in this test",
            ))
        }
    }

    #[tokio::test]
    async fn test_validate_device_token_success() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
        };
        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff::default();
        let mut gc =
            GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id").unwrap();
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        match gc.validate_device_token("valid_device_token").await {
            Ok(value) => assert!(value),
            Err(_) => assert!(false),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn test_server_error_during_token_validation() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
        };
        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(500)
            .with_body("Internal server error")
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff::default();

        let mut gc =
            GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id").unwrap();
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let result = gc.validate_device_token("valid_device_token").await;
        assert!(result.is_err());
        mock.assert();
    }
}
