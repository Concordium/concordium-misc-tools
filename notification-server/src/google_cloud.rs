use crate::models::NotificationInformation;
use anyhow::anyhow;
use backoff::{future::retry, ExponentialBackoff};
use gcp_auth::{CustomServiceAccount, TokenProvider};
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::{collections::HashMap, path::PathBuf};

const SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/firebase.messaging"];

pub struct GoogleCloud {
    client:          Client,
    service_account: CustomServiceAccount,
    url:             String,
    backoff_policy:  ExponentialBackoff,
}

impl GoogleCloud {
    pub fn new(
        credentials_path: PathBuf,
        client: Client,
        backoff_policy: ExponentialBackoff,
    ) -> anyhow::Result<Self> {
        let service_account = CustomServiceAccount::from_file(credentials_path)?;
        let project_id = service_account
            .project_id()
            .ok_or(anyhow!("Project ID not found in service account"))?;
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
        let client = Client::new();
        let access_token = self.service_account.token(SCOPES).await?;

        let entity_data: HashMap<String, String> = HashMap::new();

        let payload = json!({
            "message": {
                "token": device_token,
                "data": entity_data
            },
            "validate_only": true
        });

        let backoff = ExponentialBackoff::default();

        let operation = || async {
            let response = client
                .post(&self.url)
                .bearer_auth(access_token.as_str())
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(res) if res.status().is_success() => Ok(true),
                Ok(res) if res.status() == StatusCode::BAD_REQUEST => {
                    let error_text = res.text().await.unwrap_or_default();
                    if error_text.contains("INVALID_ARGUMENT") {
                        Ok(false)
                    } else {
                        Err(backoff::Error::permanent(anyhow!(
                            "Invalid response code returned from service: {}",
                            error_text
                        )))
                    }
                }
                Ok(res) => {
                    let status = res.status();
                    let error_text = res.text().await.unwrap_or_default();
                    if status.is_server_error() {
                        Err(backoff::Error::transient(anyhow!(
                            "Google API responded with server error: {}",
                            status
                        )))
                    } else {
                        Err(backoff::Error::permanent(anyhow!(
                            "Invalid status code received: {} with text {}",
                            status,
                            error_text
                        )))
                    }
                }
                Err(e) => Err(backoff::Error::transient(anyhow!("Network error: {}", e))),
            }
        };

        retry(backoff, operation).await
    }

    pub async fn send_push_notification(
        &self,
        device_token: &str,
        information: NotificationInformation,
    ) -> anyhow::Result<()> {
        let access_token = &self.service_account.token(SCOPES).await?;
        let entity_data: HashMap<String, String> = information.into_hashmap();

        let payload = json!({
            "message": {
                "token": device_token,
                "data": entity_data
            }
        });
        retry(self.backoff_policy.clone(), || async {
            let response = self
                .client
                .post(&self.url)
                .bearer_auth(access_token.as_str())
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(res) if res.status().is_success() => Ok(()),
                Ok(res) if res.status().is_server_error() => Err(backoff::Error::transient(
                    anyhow!("Server error: {}", res.status()),
                )),
                Ok(res) => Err(backoff::Error::permanent(anyhow!(
                    "Failed with status: {}",
                    res.status()
                ))),
                Err(err) => Err(backoff::Error::transient(anyhow!("Network error: {}", err))),
            }
        })
        .await
    }
}
