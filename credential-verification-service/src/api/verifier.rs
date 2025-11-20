//! Handlers for verification endpoints.

pub async fn verify() -> Result<String, String> {
    Ok("Verified".to_owned())
}
