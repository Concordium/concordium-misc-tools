use tracing::debug;

use crate::{
    api_types::CreateVerificationRequest,
    types::ValidationError,
    validation::{requested_subject_claims_validator, validation_context::ValidationContext},
};

pub const REQUESTED_CLAIMS_VERIFICATION_REQUEST_PATH: &str = "requestedClaims";

/// Validator entry point for validating the Create Verification API Request.
pub fn validate(request: &CreateVerificationRequest) -> Result<(), ValidationError> {
    debug!(
        "Starting validation for create verification request, with connection id: {:?}",
        &request.connection_id
    );

    // create validation context for this api request.
    let mut validation_context = ValidationContext::new();

    // validate function will push new error details into the validator context
    requested_subject_claims_validator::validate(
        &request.requested_claims,
        &mut validation_context,
        REQUESTED_CLAIMS_VERIFICATION_REQUEST_PATH,
    );

    // finally if the validator context contains any error, we will then build
    // and return the ErrorResponse which us a client friendly error response.
    if validation_context.has_errors() {
        debug!(
            "Validation errors found for create verification request api call. errors: {:?}",
            &validation_context
        );
        return Err(validation_context.into_validation_error());
    } else {
        // no errors
        debug!("Request level validation Passed for create verification request API call.");
    }

    Ok(())
}
