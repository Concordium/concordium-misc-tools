use crate::api_types::{ErrorBody, ErrorDetail, ErrorResponse};

/// Context representing validation error details. This is used to accumulate
/// errors during the validation of payloads and also provide an implementation
/// for mapping the errors that were found into a client friendly error
/// response structure as seen below in the function `create_error_response`.
#[derive(Debug, Default)]
pub struct ValidationContext {
    pub error_details: Vec<ErrorDetail>,
}

impl ValidationContext {
    pub fn new() -> Self {
        Self {
            error_details: Vec::new(),
        }
    }

    /// push a new error detail into the vec for tracking
    pub fn add_error_detail(&mut self, error_detail: ErrorDetail) {
        self.error_details.push(error_detail);
    }

    /// check if we have collected errors, return true if not empty.
    pub fn has_errors(&self) -> bool {
        !self.error_details.is_empty()
    }

    /// get an error from the context by its code.
    pub fn get_error_by_code(&self, code: &str) -> Option<&ErrorDetail> {
        for error_detail in &self.error_details {
            if error_detail.code == code {
                return Some(error_detail);
            }
        }
        None
    }

    /// Create the error response from the error details in the validation
    /// context and the additional parameters provided
    pub fn create_error_response(
        self,
        code: String,
        message: String,
        trace_id: String,
        retryable: bool,
    ) -> ErrorResponse {
        ErrorResponse {
            error: ErrorBody {
                code,
                details: self.error_details,
                message: message,
                trace_id,
                retryable,
            },
        }
    }
}
