use crate::{api_types::{CreateVerificationRequest, ErrorResponse}, validation::{requested_subject_claims_validator, validation_context::ValidationContext}};


/// Validator entry point for validating the Create Verification API Request.
pub fn validate(
    request: CreateVerificationRequest
) -> Result<(), ErrorResponse>{

    // create validation context for this api request.
    let mut validation_context = ValidationContext::new();

    // validate function will push new error details into the validator context
    requested_subject_claims_validator::validate(
        request.requested_claims, 
        &mut validation_context, 
        "TODO: dummy path"
    );

    // finally if the validator context contains any error, we will then build 
    // and return the ErrorResponse which us a client friendly error response.
    if validation_context.has_errors() {

        let error_response = validation_context.create_error_response(
            "VALIDATION_ERROR".to_string(), 
            "Validation errors have occurred. Please check the details below for more information.".to_string(), 
            "dummy".to_string(), // TODO - there should be the option to receive the traceid from the request or to generate a fresh one
            false
        );

        return Err(error_response)
    }

    Ok(())
}
    



