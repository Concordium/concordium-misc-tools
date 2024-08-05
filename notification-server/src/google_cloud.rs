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
    /// Creates a new instance of `GoogleCloud` configured for interacting with the Google Cloud Messaging API.
    ///
    /// # Arguments
    /// * `client` - A `reqwest::Client` for making HTTP requests.
    /// * `backoff_policy` - An `ExponentialBackoff` policy to handle retries for transient errors.
    /// * `service_account` - An implementation of the `TokenProvider` trait to fetch access tokens.
    /// * `project_id` - The project ID associated with your Google Cloud project.
    ///
    /// # Returns
    /// Returns an instance of `GoogleCloud`.
    ///
    /// # Errors
    /// Returns an `Err` if there is a problem constructing the URL or any other initial setup issue.
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

    /// Validates a device token by attempting a minimal push notification request to Google's FCM API with `validate_only` set to true.
    /// This method is designed to verify if a provided device token is correctly formatted and recognized by Google without actually sending a notification.
    ///
    /// # Arguments
    /// * `device_token` - The device token that needs validation.
    ///
    /// # Returns
    /// Returns `Ok(true)` if the token is valid, `Ok(false)` if the token is invalid, and `Err` if there is an error in sending the request or processing the API response.
    ///
    /// # Errors
    /// Errors are generally related to network issues or server errors that prevent the API from processing the request.
    pub async fn validate_device_token(&self, device_token: &str) -> anyhow::Result<bool> {
        self.send_push_notification_with_validate(device_token, None)
            .await
    }

    /// Sends a push notification to a device using Google's FCM API.
    /// This method should be used when a real notification needs to be dispatched to an end-user device.
    ///
    /// # Arguments
    /// * `device_token` - The device token to which the notification will be sent.
    /// * `information` - A `NotificationInformation` struct containing data to be sent in the notification.
    ///
    /// # Returns
    /// Returns `Ok(())` on successful dispatch of the notification, and `Err` on failure.
    ///
    /// # Errors
    /// Errors may include issues with network connectivity, token validation, or errors from the Google API response.
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
                Ok(res) if res.status() == StatusCode::TOO_MANY_REQUESTS => {
                    Err(backoff::Error::transient(anyhow!(
                        "Too many requests sent to Google API: {}",
                        &payload
                    )))
                }
                Ok(res) if res.status().is_client_error() => {
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
    use backoff::ExponentialBackoff;
    use gcp_auth::Token;
    use reqwest::Client;
    use std::sync::Arc;

    pub struct MockTokenProvider {
        pub token_response: Arc<String>,
    }

    use std::time::Duration;
    use enum_iterator::{all, Sequence};
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;

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

    #[derive(Debug, Clone, Copy, Sequence)]
    enum RetryStatusCode {
        TooManyRequests = 429,
        InternalServerError = 500,
        NotImplemented = 501,
        BadGateway = 502,
        ServiceUnavailable = 503,
        GatewayTimeout = 504,
        HTTPVersionNotSupported = 505,
        VariantAlsoNegotiates = 506,
        InsufficientStorage = 507,
        LoopDetected = 508,
        NotExtended = 510,
        NetworkAuthenticationRequired = 511,
    }

    #[derive(Debug, Clone, Copy, Sequence)]
    pub enum ZeroRetryStatusCode {
        BadRequest = 400,
        Unauthorized = 401,
        PaymentRequired = 402,
        Forbidden = 403,
        NotFound = 404,
        MethodNotAllowed = 405,
        NotAcceptable = 406,
        ProxyAuthenticationRequired = 407,
        RequestTimeout = 408,
        Conflict = 409,
        Gone = 410,
        LengthRequired = 411,
        PreconditionFailed = 412,
        PayloadTooLarge = 413,
        UriTooLong = 414,
        UnsupportedMediaType = 415,
        RangeNotSatisfiable = 416,
        ExpectationFailed = 417,
        ImATeapot = 418,
        MisdirectedRequest = 421,
        UnprocessableEntity = 422,
        Locked = 423,
        FailedDependency = 424,
        TooEarly = 425,
        UpgradeRequired = 426,
        PreconditionRequired = 428,
        RequestHeaderFieldsTooLarge = 431,
        UnavailableForLegalReasons = 451,
    }

    impl Arbitrary for RetryStatusCode {
        fn arbitrary(g: &mut Gen) -> Self {
            let codes = all::<RetryStatusCode>().collect::<Vec<_>>();
            *g.choose(&codes).unwrap()
        }
    }

    impl Arbitrary for ZeroRetryStatusCode {
        fn arbitrary(g: &mut Gen) -> Self {
            let codes = all::<ZeroRetryStatusCode>().collect::<Vec<_>>();
            *g.choose(&codes).unwrap()
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

    #[quickcheck]
    fn test_retry_on_retry_status_codes(server_side_status_code: RetryStatusCode) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
        };

        let mock = server.mock("POST", "/v1/projects/fake_project_id/messages:send")
              .with_status(server_side_status_code as usize)
              .with_body("Service temporarily unavailable")
              .expect_at_least(2)
              .create();

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(10)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id").unwrap();
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        mock.assert();
        result.is_err()
    }

    #[quickcheck]
    fn test_retry_on_zero_retry_status_codes(status_code: ZeroRetryStatusCode) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
        };

        let mock = server.mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(status_code as usize)
            .with_body("Client side error")
            .expect(1)
            .create();

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(10)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id").unwrap();
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        mock.assert();
        result.is_err()
    }

    #[quickcheck]
    fn test_retry_on_zero_retry_status_codes_eventually_succeeds(server_side_status_code: RetryStatusCode) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
        };

        let failing_calls = server.mock("POST", "/v1/projects/fake_project_id/messages:send")
              .with_status(server_side_status_code as usize)
              .with_body("Service temporarily unavailable")
              .expect(2)
              .create();

        let succeeding_call = server.mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(200)
            .with_body("Service temporarily unavailable")
            .expect(1)
            .create();


        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id").unwrap();
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        failing_calls.assert();
        succeeding_call.assert();
        result.unwrap_or_else(|_| false)
    }

}
