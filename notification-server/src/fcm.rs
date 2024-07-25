use std::collections::HashMap;
use anyhow::anyhow;
use serde_json::json;
use reqwest::Client;
use crate::notification_information::NotificationInformation;

async fn send_push_notification(
    device_token: &str,
    information: NotificationInformation,
    access_token: &str,
    project_id: &str,
) -> anyhow::Result<()> {
    let client = Client::new();
    let url = format!("https://fcm.googleapis.com/v1/projects/{}/messages:send", project_id);

    let entity_data = information.into_hashmap();

    let payload = json!({
        "message": {
            "token": device_token,
            "data": entity_data
        }
    });

    let res = client
        .post(&url)
        .bearer_auth(access_token)
        .json(&payload)
        .await?;

    if res.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to send push notification: {}", res.text().await?))
    }
}
