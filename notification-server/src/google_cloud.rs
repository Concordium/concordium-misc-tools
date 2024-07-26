use crate::notification_information::NotificationInformation;
use anyhow::anyhow;
use gcp_auth::{CustomServiceAccount, TokenProvider};
use reqwest::Client;
use serde_json::json;
use std::{collections::HashMap, path::PathBuf};

const SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/firebase.messaging"];

pub struct GoogleCloud {
    service_account: CustomServiceAccount,
    url:             String,
}

impl GoogleCloud {
    pub fn new(credentials_path: PathBuf) -> anyhow::Result<Self> {
        let service_account = CustomServiceAccount::from_file(credentials_path)?;
        let project_id = service_account
            .project_id()
            .ok_or(anyhow!("Project ID not found in service account"))?;
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            project_id
        );
        Ok(Self {
            service_account,
            url,
        })
    }

    pub async fn send_push_notification(
        &self,
        device_token: &str,
        information: NotificationInformation,
    ) -> anyhow::Result<()> {
        let client = Client::new();
        let access_token = &self.service_account.token(SCOPES).await?;

        let entity_data: HashMap<String, String> = information.into_hashmap();

        let payload = json!({
            "message": {
                "token": device_token,
                "data": entity_data
            }
        });

        let res = client
            .post(&self.url)
            .bearer_auth(access_token.as_str())
            .json(&payload)
            .send()
            .await?;

        if res.status().is_success() {
            println!("{}", res.text().await.unwrap());
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to send push notification: {}",
                res.text().await?
            ))
        }
    }
}
