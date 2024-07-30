use crate::models::NotificationInformation;
use anyhow::anyhow;
use backoff::{future::retry, ExponentialBackoff};
use gcp_auth::{CustomServiceAccount, TokenProvider};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf};

const SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/firebase.messaging"];

pub struct GoogleCloud<T> where
    T: TokenProvider,
{
    client:          Client,
    service_account: T,
    url:             String,
    backoff_policy:  ExponentialBackoff,
}

impl<T> GoogleCloud<T>
where
    T: TokenProvider, {
    pub fn new(
        client: Client,
        backoff_policy: ExponentialBackoff,
        service_account: T,
    ) -> anyhow::Result<Self> {
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
