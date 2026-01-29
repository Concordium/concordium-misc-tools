use tracing::debug;

use crate::{
    api_types::VerifyPresentationRequest,
    types::ValidationError,
    validation::{requested_subject_claims_validator, validation_context::ValidationContext},
};

pub const VERIFY_SUBJECT_CLAIMS_VALIDATION_PATH: &str = "verificationRequest.subjectClaims";

/// Validator entry point for validating the Verify Presentation API Request
pub fn validate(request: &VerifyPresentationRequest) -> Result<(), ValidationError> {
    debug!(
        "Starting validation for verify api request, with audit record: {:?}",
        &request.audit_record_id
    );

    // create validation context for this api request.
    let mut validation_context = ValidationContext::new();

    // validate function will push new error details into the validator context
    requested_subject_claims_validator::validate(
        &request.verification_request.subject_claims,
        &mut validation_context,
        VERIFY_SUBJECT_CLAIMS_VALIDATION_PATH,
    );

    // finally if the validator context contains any error, we will then build
    // and return the ErrorResponse which us a client friendly error response.
    if validation_context.has_errors() {
        debug!(
            "Validation errors found for verify presentation api call. errors: {:?}",
            &validation_context
        );
        return Err(validation_context.into_validation_error());
    } else {
        debug!(
            "No errors found for verify presentation request with audit record id: {:?}",
            &request.audit_record_id
        );
    }

    Ok(())
}
