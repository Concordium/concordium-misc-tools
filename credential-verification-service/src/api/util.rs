use concordium_rust_sdk::endpoints::{QueryError, RPCError};

pub trait QueryErrorExt {
    /// If the query error is account sequence mismatch error
    fn is_account_sequence_number_error(&self) -> bool;
}

impl QueryErrorExt for QueryError {
    fn is_account_sequence_number_error(&self) -> bool {
        match self {
            QueryError::RPCError(RPCError::CallError(err)) => {
                err.message() == "Duplicate nonce" || err.message() == "Nonce too large"
            }
            _ => false,
        }
    }
}
