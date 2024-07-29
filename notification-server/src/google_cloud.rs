use crate::models::NotificationInformation;
use anyhow::anyhow;
use backoff::{future::retry, ExponentialBackoff};
use gcp_auth::{CustomServiceAccount, TokenProvider};
use log::info;
use reqwest::{Client, StatusCode};
use serde_json::json;
use std::{collections::HashMap, path::PathBuf};
use tracing::error;

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
        let access_token = &self.service_account.token(SCOPES).await?;

        let entity_data: HashMap<String, String> = HashMap::new();

        let payload = json!({
            "message": {
                "token": device_token,
                "data": entity_data
            },
            "validate_only": true
        });

        let res = client
            .post(&self.url)
            .bearer_auth(access_token.as_str())
            .json(&payload)
            .send()
            .await?;
        if res.status().is_success() {
            Ok(true)
        } else {
            let status_code = res.status();
            let error_text = res.text().await.map_err(|e| {
                error!("Failed to read error response: {}", e);
                e
            })?;
            if status_code == StatusCode::BAD_REQUEST {
                match serde_json::from_str(&error_text) {
                    Ok(json) => {
                        if let Some(error) = json["error"]["status"] == "INVALID_ARGUMENT" {
                            Ok(false)
                        } else {
                            Err(anyhow!(
                                "Invalid response code returned from service: {}",
                                error_text
                            ))
                        }
                    }
                    Err(err) => Err(anyhow!("Failed to parse error response: {}", err)),
                }
            } else {
                Err(anyhow!(
                    "Invalid status code received: {} with text {}",
                    status_code,
                    error_text
                ))
            }
        }
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
