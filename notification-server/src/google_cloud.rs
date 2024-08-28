use backoff::{future::retry, ExponentialBackoff};
use gcp_auth::TokenProvider;
use reqwest::{Client, StatusCode};
use serde_json::json;
use thiserror::Error;

use crate::{
    google_cloud::NotificationError::{
        AuthenticationError, ClientError, InvalidArgumentError, ServerError, UnregisteredError,
    },
    models::notification::NotificationInformation,
};

const SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/firebase.messaging"];

#[derive(Debug, PartialEq, Error)]
pub enum NotificationError {
    #[error("Server Error: {0}")]
    ServerError(String),
    #[error("Client Error: {0}")]
    ClientError(String),
    #[error("Device token had an invalid format")]
    InvalidArgumentError,
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("Device token has been unregistered")]
    UnregisteredError,
}

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
    /// Creates a new instance of `GoogleCloud` configured for interacting with
    /// the Google Cloud Messaging API.
    ///
    /// # Arguments
    /// * `client` - A `reqwest::Client` used for making HTTP requests.
    /// * `backoff_policy` - An `ExponentialBackoff` policy to handle retries
    ///   for transient errors.
    /// * `service_account` - An implementation of the `TokenProvider` trait to
    ///   fetch access tokens.
    /// * `project_id` - The project ID associated with your Google Cloud
    ///   project.
    ///
    /// # Returns
    /// Returns an instance of `GoogleCloud`.
    pub fn new(
        client: Client,
        backoff_policy: ExponentialBackoff,
        service_account: T,
        project_id: &str,
    ) -> Self {
        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            project_id
        );
        Self {
            client,
            service_account,
            url,
            backoff_policy,
        }
    }

    /// Validates a device token by making a minimal push notification request
    /// to Google's FCM API with `validate_only` set to true. This method
    /// checks if a provided device token is correctly formatted and recognized
    /// by Google without sending a real notification.
    ///
    /// # Arguments
    /// * `device_token` - The device token that needs validation.
    ///
    /// # Returns
    /// Returns `Ok(())` if the token is valid, indicating successful
    /// validation. Returns `Err(NotificationError)` if the token is invalid
    /// or if there is an error in sending the request or processing the API
    /// response.
    ///
    /// # Errors
    /// Errors include network issues, token validation errors, or other
    /// server/client errors as defined by `NotificationError`.
    pub async fn validate_device_token(&self, device_token: &str) -> Result<(), NotificationError> {
        self.send_push_notification_with_validate(device_token, None)
            .await
    }

    /// Sends a push notification to a device using Google's FCM API.
    /// This method is intended for when an actual notification needs to be
    /// dispatched to an end-user device.
    ///
    /// # Arguments
    /// * `device_token` - The device token to which the notification will be
    ///   sent.
    /// * `information` - A `NotificationInformation` struct containing the data
    ///   to be sent in the notification.
    ///
    /// # Returns
    /// Returns `Ok(())` on successful dispatch of the notification.
    /// Returns `Err(NotificationError)` on failure, which includes any issue
    /// related to network connectivity, token validation, or errors from the
    /// Google API.
    ///
    /// # Errors
    /// Specific errors are returned as `NotificationError` which includes
    /// various client and server-side issues.
    pub async fn send_push_notification(
        &self,
        device_token: &str,
        information: &NotificationInformation,
    ) -> Result<(), NotificationError> {
        self.send_push_notification_with_validate(device_token, Some(information))
            .await
    }

    async fn send_push_notification_with_validate(
        &self,
        device_token: &str,
        information: Option<&NotificationInformation>,
    ) -> Result<(), NotificationError> {
        let access_token = self.service_account.token(SCOPES).await.map_err(|err| {
            AuthenticationError(format!("Authentication error received: {}", err))
        })?;
        let mut payload = json!({});
        if Option::is_none(&information) {
            payload["validate_only"] = json!(true);
        }
        let entity_data = if let Some(information) = information {
            json!(information)
        } else {
            json!({})
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
                Ok(res) => match res.status() {
                    StatusCode::TOO_MANY_REQUESTS => Err(backoff::Error::transient(ServerError(
                        "Too many requests to the Google API".to_string(),
                    ))),
                    StatusCode::BAD_REQUEST => Err(backoff::Error::permanent(InvalidArgumentError)),
                    StatusCode::NOT_FOUND => Err(backoff::Error::permanent(UnregisteredError)),
                    _ if res.status().is_client_error() => {
                        Err(backoff::Error::permanent(ClientError(
                            "Error calling Google API, not related to the input of the user"
                                .to_string(),
                        )))
                    }
                    _ if res.status().is_success() => Ok(()),
                    _ if res.status().is_server_error() => {
                        Err(backoff::Error::transient(ServerError(format!(
                            "Google API responded with a server error status code: {}",
                            res.status().as_u16()
                        ))))
                    }
                    _ => Err(backoff::Error::transient(ServerError(format!(
                        "Google API responded with status code: {}",
                        res.status().as_u16()
                    )))),
                },
                Err(e) => Err(backoff::Error::transient(ServerError(e.to_string()))),
            }
        };
        retry(self.backoff_policy.clone(), operation).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notification::{
        CCDTransactionNotificationInformation, CIS2EventNotificationInformation,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use backoff::ExponentialBackoff;
    use concordium_rust_sdk::{
        base::{hashes::TransactionHash, smart_contracts::OwnedContractName},
        cis2::{MetadataUrl, TokenId},
        id::types::AccountAddress,
        types::{hashes::Hash, ContractAddress},
    };
    use enum_iterator::{all, Sequence};
    use gcp_auth::Token;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use reqwest::Client;
    use std::{cmp::PartialEq, str::FromStr, sync::Arc, time::Duration};

    pub struct MockTokenProvider {
        pub token_response: Arc<String>,
        pub should_fail:    bool,
    }

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
            match &self.should_fail {
                true => Err(gcp_auth::Error::Str("Mock token provider failed")),
                false => Ok(Arc::new(generate_mock_token())),
            }
        }

        async fn project_id(&self) -> Result<Arc<str>, gcp_auth::Error> {
            Err(gcp_auth::Error::Str(
                "Project id cannot be called in this test",
            ))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Sequence)]
    enum RetryStatusCode {
        TooManyRequests     = 429,
        InternalServerError = 500,
        NotImplemented      = 501,
        BadGateway          = 502,
        ServiceUnavailable  = 503,
        GatewayTimeout      = 504,
        HTTPVersionNotSupported = 505,
        VariantAlsoNegotiates = 506,
        InsufficientStorage = 507,
        LoopDetected        = 508,
        NotExtended         = 510,
        NetworkAuthenticationRequired = 511,
    }

    fn is_status_code_causing_correct_notification_error(
        status_code: usize,
        actual_notification_error: NotificationError,
    ) -> bool {
        match all::<RetryStatusCode>().find(|&x| x as usize == status_code) {
            Some(_) => matches!(actual_notification_error, ServerError(_)),
            None => match all::<ZeroRetryStatusCode>().find(|&x| x as usize == status_code) {
                Some(zero_retry_status_code) => {
                    if zero_retry_status_code == ZeroRetryStatusCode::BadRequest {
                        matches!(actual_notification_error, InvalidArgumentError)
                    } else if zero_retry_status_code == ZeroRetryStatusCode::NotFound {
                        matches!(actual_notification_error, UnregisteredError)
                    } else {
                        matches!(actual_notification_error, ClientError(_))
                    }
                }
                None => false,
            },
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Sequence)]
    enum ZeroRetryStatusCode {
        BadRequest           = 400,
        Unauthorized         = 401,
        PaymentRequired      = 402,
        Forbidden            = 403,
        NotFound             = 404,
        MethodNotAllowed     = 405,
        NotAcceptable        = 406,
        ProxyAuthenticationRequired = 407,
        RequestTimeout       = 408,
        Conflict             = 409,
        Gone                 = 410,
        LengthRequired       = 411,
        PreconditionFailed   = 412,
        PayloadTooLarge      = 413,
        UriTooLong           = 414,
        UnsupportedMediaType = 415,
        RangeNotSatisfiable  = 416,
        ExpectationFailed    = 417,
        ImATeapot            = 418,
        MisdirectedRequest   = 421,
        UnprocessableEntity  = 422,
        Locked               = 423,
        FailedDependency     = 424,
        TooEarly             = 425,
        UpgradeRequired      = 426,
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
    async fn test_send_push_notification_ccd() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let expected_body = json!({
            "message": {
                "token": "valid_device_token",
                "data": {
                    "recipient": "4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp",
                    "amount": "100",
                    "type": "ccd-tx",
                    "reference": "3d1c2f4fb9a0eb468bfe39e75c59897c1a375082a6440f4a5da77102182ba055",
                }
            }
        });

        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .match_body(mockito::Matcher::Json(expected_body))
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let notification_information =
            NotificationInformation::CCD(CCDTransactionNotificationInformation::new(
                AccountAddress::from_str("4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp")
                    .unwrap(),
                "100".to_string(),
                TransactionHash::from_str(
                    "3d1c2f4fb9a0eb468bfe39e75c59897c1a375082a6440f4a5da77102182ba055",
                )
                .unwrap(),
            ));
        assert!(gc
            .send_push_notification("valid_device_token", &notification_information)
            .await
            .is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_send_push_notification_cis2_without_metadata_url() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let expected_body = json!({
            "message": {
                "token": "test_token",
                "data": {
                    "recipient": "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G",
                    "amount": "200",
                    "type": "cis2-tx",
                    "token_id": "ffffff",
                    "contract_address": "{\"index\":3,\"subindex\":0}",
                    "contract_name": "init_contract",
                    "reference": "6a6d250ecefb518253db4c0d7759b2f4ff2862217ed2c8343879a77e0c2c97a2",
                }
            }
        });

        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .match_body(mockito::Matcher::Json(expected_body.clone()))
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let notification_information =
            NotificationInformation::CIS2(CIS2EventNotificationInformation::new(
                AccountAddress::from_str("3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G")
                    .unwrap(),
                "200".to_string(),
                TokenId::from_str("ffffff").unwrap(),
                ContractAddress::new(3, 0),
                OwnedContractName::new("init_contract".to_string()).unwrap(),
                None,
                TransactionHash::from_str(
                    "6a6d250ecefb518253db4c0d7759b2f4ff2862217ed2c8343879a77e0c2c97a2",
                )
                .unwrap(),
            ));
        println!("Expected: {}", expected_body);
        assert!(gc
            .send_push_notification("test_token", &notification_information)
            .await
            .is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_send_push_notification_cis2_with_metadata_url() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let expected_body = json!({
            "message": {
                "token": "test_token",
                "data": {
                    "recipient": "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G",
                    "amount": "200",
                    "type": "cis2-tx",
                    "contract_address": "{\"index\":112,\"subindex\":2}",
                    "contract_name": "init_contract",
                    "token_id": "ffffff",
                    "token_metadata": "{\"url\":\"https://example.com\",\"hash\":null}",
                    "reference": "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110",
                }
            }
        });
        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .match_body(mockito::Matcher::Json(expected_body))
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let notification_information =
            NotificationInformation::CIS2(CIS2EventNotificationInformation::new(
                AccountAddress::from_str("3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G")
                    .unwrap(),
                "200".to_string(),
                TokenId::from_str("ffffff").unwrap(),
                ContractAddress::new(112, 2),
                OwnedContractName::new("init_contract".to_string()).unwrap(),
                Some(MetadataUrl::new("https://example.com".to_string(), None).unwrap()),
                TransactionHash::from_str(
                    "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110",
                )
                .unwrap(),
            ));
        assert!(gc
            .send_push_notification("test_token", &notification_information)
            .await
            .is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_send_push_notification_cis2_with_metadata_url_and_hash() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let expected_body = json!({
            "message": {
                "token": "test_token",
                "data": {
                    "recipient": "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G",
                    "amount": "200",
                    "type": "cis2-tx",
                    "contract_address": "{\"index\":111,\"subindex\":1}",
                    "contract_name": "init_contract",
                    "token_id": "ffffff",
                    "token_metadata": "{\"url\":\"https://example.com\",\"hash\":\"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"}",
                    "reference": "8a3a09bffa6ead269f79be4192fcb7773cc4e10a2e90c0dec3eb9ca5200c06bc"
                }
            }
        });
        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .match_body(mockito::Matcher::Json(expected_body))
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(1)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let notification_information =
            NotificationInformation::CIS2(CIS2EventNotificationInformation::new(
                AccountAddress::from_str("3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G")
                    .unwrap(),
                "200".to_string(),
                TokenId::from_str("ffffff").unwrap(),
                ContractAddress::new(111, 1),
                OwnedContractName::new("init_contract".to_string()).unwrap(),
                Some(
                    MetadataUrl::new(
                        "https://example.com".to_string(),
                        Some(
                            Hash::from_str(
                                "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
                            )
                            .unwrap(),
                        ),
                    )
                    .unwrap(),
                ),
                TransactionHash::from_str(
                    "8a3a09bffa6ead269f79be4192fcb7773cc4e10a2e90c0dec3eb9ca5200c06bc",
                )
                .unwrap(),
            ));
        assert!(gc
            .send_push_notification("test_token", &notification_information)
            .await
            .is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_validate_device_token_success() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
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
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        assert!(gc.validate_device_token("valid_device_token").await.is_ok());
        mock.assert();
    }

    #[quickcheck]
    fn test_retry_on_retry_status_codes(status_code: RetryStatusCode) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(status_code as usize)
            .with_body("Service temporarily unavailable")
            .expect_at_least(2)
            .create();

        let client = Client::new();
        let backoff_policy = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_millis(50)),
            max_interval: Duration::from_millis(1),
            initial_interval: Duration::from_millis(1),
            ..ExponentialBackoff::default()
        };

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        mock.assert();
        result.map_or_else(
            |err| is_status_code_causing_correct_notification_error(status_code as usize, err),
            |_| false,
        )
    }

    #[quickcheck]
    fn test_retry_on_zero_retry_status_codes(status_code: ZeroRetryStatusCode) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
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

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        mock.assert();
        result.map_or_else(
            |err| is_status_code_causing_correct_notification_error(status_code as usize, err),
            |_| false,
        )
    }

    #[quickcheck]
    fn test_retry_on_zero_retry_status_codes_eventually_succeeds(
        server_side_status_code: RetryStatusCode,
    ) -> bool {
        let mut server = mockito::Server::new();
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    false,
        };

        let failing_calls = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(server_side_status_code as usize)
            .with_body("Service temporarily unavailable")
            .expect(2)
            .create();

        let succeeding_call = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
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

        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(gc.validate_device_token("valid_device_token"));
        failing_calls.assert();
        succeeding_call.assert();
        result.is_ok()
    }

    #[tokio::test]
    async fn should_not_continue_on_auth_failed() {
        let mut server = mockito::Server::new_async().await;
        let mock_provider = MockTokenProvider {
            token_response: Arc::new("mock_token".to_string()),
            should_fail:    true,
        };
        let mock = server
            .mock("POST", "/v1/projects/fake_project_id/messages:send")
            .with_status(200)
            .with_body(json!({"success": true}).to_string())
            .expect(0)
            .create_async()
            .await;

        let client = Client::new();
        let backoff_policy = ExponentialBackoff::default();
        let mut gc = GoogleCloud::new(client, backoff_policy, mock_provider, "mock_project_id");
        gc.url = format!(
            "{}{}",
            server.url(),
            "/v1/projects/fake_project_id/messages:send".to_string()
        );

        mock.assert();
        let result = gc.validate_device_token("valid_device_token").await;
        assert!(matches!(result, Err(AuthenticationError(_))));
    }
}
