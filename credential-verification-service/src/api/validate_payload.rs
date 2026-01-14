//! Helpers for validating the statements/claims in a request to this service.
use crate::types::ServerError;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    RequestedStatement::{AttributeInRange, AttributeInSet, AttributeNotInSet, RevealAttribute},
    RequestedSubjectClaims::{self, Identity},
};
use concordium_rust_sdk::id::constants::ArCurve;
use concordium_rust_sdk::id::id_proof_types::{
    AttributeInRangeStatement, AttributeInSetStatement, AttributeNotInSetStatement,
};
use concordium_rust_sdk::id::types::AttributeTag;
use concordium_rust_sdk::web3id::Web3IdAttribute;
use rust_iso3166;
use std::collections::BTreeSet;

/// Attribute tags
pub const ATTRIBUTE_TAG_DOB: AttributeTag = AttributeTag(3);
pub const ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE: AttributeTag = AttributeTag(4);
pub const ATTRIBUTE_TAG_NATIONALITY: AttributeTag = AttributeTag(5);
pub const ATTRIBUTE_TAG_ID_DOC_TYPE: AttributeTag = AttributeTag(6);
pub const ATTRIBUTE_TAG_ID_DOC_ISSUER: AttributeTag = AttributeTag(8);
pub const ATTRIBUTE_TAG_ID_DOC_ISSUED_AT: AttributeTag = AttributeTag(9);
pub const ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT: AttributeTag = AttributeTag(10);
pub const ATTRIBUTE_TAG_LEGAL_COUNTRY: AttributeTag = AttributeTag(15);

pub fn ensure_string(attr: &Web3IdAttribute) -> Result<&str, ServerError> {
    match attr {
        Web3IdAttribute::String(v) => Ok(v.as_ref()),
        _ => Err(ServerError::PayloadValidation(
            "Expected string value in attribute".to_string(),
        )),
    }
}

/// ISO8601 strings representing dates as `YYYYMMDD`.
pub fn is_iso8601(date: &str) -> Result<(), ServerError> {
    // Must be exactly 8 characters
    if date.len() != 8 {
        return Err(ServerError::PayloadValidation(format!(
            "The given date should be 8 characters of length (iso8601 `YYYYMMDD` format) but given date `{0}` is of length {1}.",
            date,
            date.len()
        )));
    }

    // Must be all digits
    if !date.chars().all(|c| c.is_ascii_digit()) {
        return Err(ServerError::PayloadValidation(
            "Date characters must be digits (iso8601 `YYYYMMDD` format).".to_string(),
        ));
    }

    // Parse month (chars 4-5, 0-indexed)
    let month: u32 = match date[4..6].parse() {
        Ok(m) => m,
        Err(e) => {
            return Err(ServerError::PayloadValidation(format!(
                "Month must be present (iso8601 `YYYYMMDD` format).: {0}",
                e
            )));
        }
    };
    if !(1..=12).contains(&month) {
        return Err(ServerError::PayloadValidation(
            "Month must be between 1-12 (iso8601 `YYYYMMDD` format).".to_string(),
        ));
    }

    // Parse day (chars 6-7)
    let day: u32 = match date[6..8].parse() {
        Ok(d) => d,
        Err(e) => {
            return Err(ServerError::PayloadValidation(format!(
                "Day must be present (iso8601 `YYYYMMDD` format).: {0}",
                e
            )));
        }
    };
    if !(1..=31).contains(&day) {
        return Err(ServerError::PayloadValidation(
            "Day must be between 1-31 (iso8601 `YYYYMMDD` format).".to_string(),
        ));
    }
    Ok(())
}

/// ISO3166_1_alpha2 codes consist of 2 upper case characters representing countries/region (e.g. `GB, DE, DK`).
pub fn is_iso3166_1_alpha2(code: &str) -> bool {
    rust_iso3166::from_alpha2(code).is_some()
        && code.len() == 2
        && code.chars().all(|c| c.is_ascii_uppercase())
}

/// ISO3166-2 codes consist of a ISO3166_1_alpha2 code, then a dash, and then 1-3 alphanumerical characters
/// representing countries/region (e.g. `ES-B`, `US-CA`).
pub fn is_iso3166_2(code: &str) -> bool {
    if code.len() < 4 || code.len() > 6 {
        // 2 letters + '-' + 1-3 characters
        return false;
    }

    rust_iso3166::iso3166_2::from_code(code).is_some()
}

#[derive(Debug)]
pub enum IdDocType {
    NA,
    Passport,
    NationalIdCard,
    DriversLicense,
    ImmigrationCard,
}

impl IdDocType {
    /// Try to parse a string into an IdDocType
    pub fn parse(code: &str) -> Result<IdDocType, ServerError> {
        match code {
            "0" => Ok(IdDocType::NA),
            "1" => Ok(IdDocType::Passport),
            "2" => Ok(IdDocType::NationalIdCard),
            "3" => Ok(IdDocType::DriversLicense),
            "4" => Ok(IdDocType::ImmigrationCard),
            _ => Err(ServerError::PayloadValidation(format!(
                "Invalid ID document type `{}`. Must be one of: 0 (N/A), 1 (Passport), 2 (NationalIdCard), 3 (DriversLicense), or 4 (ImmigrationCard).",
                code
            ))),
        }
    }
}

pub fn payload_validation(claims: Vec<RequestedSubjectClaims>) -> Result<(), ServerError> {
    for claim in claims {
        match claim {
            Identity(id_claim) => {
                for statement in id_claim.statements {
                    match statement {
                        RevealAttribute(_) => {
                            // Nothing to validate here.
                        }
                        AttributeInRange(sta) => validate_range_statement(sta)?,
                        AttributeInSet(sta) => validate_set_statement(&sta)?,
                        AttributeNotInSet(sta) => validate_set_statement(&sta)?,
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn validate_range_statement(
    statement: AttributeInRangeStatement<ArCurve, AttributeTag, Web3IdAttribute>,
) -> Result<(), ServerError> {
    let error_prefix_message = format!(
        "Validation of range statement with attribute tag `{0}` failed. ",
        statement.attribute_tag
    );

    if statement.upper < statement.lower {
        return Err(ServerError::PayloadValidation(format!(
            "{0}Provided `upper bound: {1}` must be greater than `lower bound: {2}`.",
            error_prefix_message, statement.upper, statement.lower
        )));
    };

    match statement.attribute_tag {
        ATTRIBUTE_TAG_DOB | ATTRIBUTE_TAG_ID_DOC_ISSUED_AT | ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT => {
            // Ensure upper and lower bounds are strings
            let upper_str = ensure_string(&statement.upper).map_err(|e| {
                ServerError::PayloadValidation(format!(
                    "{error_prefix_message}Upper bound value error: {e}"
                ))
            })?;
            let lower_str = ensure_string(&statement.lower).map_err(|e| {
                ServerError::PayloadValidation(format!(
                    "{error_prefix_message}Lower bound value error: {e}"
                ))
            })?;

            // Validate dates with ISO8601
            is_iso8601(upper_str).map_err(|e| {
                ServerError::PayloadValidation(format!(
                    "{error_prefix_message}Upper bound value invalid format: {e}"
                ))
            })?;
            is_iso8601(lower_str).map_err(|e| {
                ServerError::PayloadValidation(format!(
                    "{error_prefix_message}Lower bound value invalid format: {e}"
                ))
            })?;
        }
        _ => {
            return Err(ServerError::PayloadValidation(format!(
                "{error_prefix_message}Attribute tag `{0}` is not allowed to be used in range statements. \
                Only `ATTRIBUTE_TAG_DOB(3)`, `ATTRIBUTE_TAG_ID_DOC_ISSUED_AT(9)`, and `ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT(10)` allowed in range statements.",
                statement.attribute_tag
            )));
        }
    }

    Ok(())
}

pub trait HasSet<'a> {
    type Item;
    fn set(&'a self) -> &'a BTreeSet<Web3IdAttribute>;
    fn attribute_tag(&self) -> AttributeTag;
}

impl<'a> HasSet<'a> for AttributeInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
    type Item = Web3IdAttribute;

    fn set(&'a self) -> &'a BTreeSet<Web3IdAttribute> {
        &self.set
    }

    fn attribute_tag(&self) -> AttributeTag {
        self.attribute_tag
    }
}

impl<'a> HasSet<'a> for AttributeNotInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
    type Item = Web3IdAttribute;

    fn set(&'a self) -> &'a BTreeSet<Web3IdAttribute> {
        &self.set
    }

    fn attribute_tag(&self) -> AttributeTag {
        self.attribute_tag
    }
}

pub fn validate_set_statement<S>(statement: &S) -> Result<(), ServerError>
where
    S: for<'a> HasSet<'a, Item = Web3IdAttribute>,
{
    let error_prefix_message = format!(
        "Validation of in-set/not-in-set statement with attribute tag `{0}` failed. ",
        statement.attribute_tag()
    );

    if statement.set().is_empty() {
        return Err(ServerError::PayloadValidation(
            "{error_prefix_message}Set Statement should not be empty.".to_string(),
        ));
    }

    match statement.attribute_tag() {
        ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE
        | ATTRIBUTE_TAG_NATIONALITY
        | ATTRIBUTE_TAG_LEGAL_COUNTRY => {
            // 1. Ensure all values are strings
            let values: Vec<&str> = statement
                .set()
                .iter()
                .map(|attr| {
                    ensure_string(attr).map_err(|e| {
                        ServerError::PayloadValidation(format!(
                            "{error_prefix_message}Value in set statement must be a string: {e}"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?;

            // 2. Validate ISO codes
            for code in &values {
                if !is_iso3166_1_alpha2(code) {
                    return Err(ServerError::PayloadValidation(format!(
                        "{error_prefix_message}Value `{0}` of attribute tag `{1}` must be ISO3166-1 Alpha-2 code in upper case (e.g. `DE`)",
                        code,
                        statement.attribute_tag(),
                    )));
                }
            }
        }

        ATTRIBUTE_TAG_ID_DOC_ISSUER => {
            // 1. Ensure all values are strings
            let values: Vec<&str> = statement
                .set()
                .iter()
                .map(|attr| {
                    ensure_string(attr).map_err(|e| {
                        ServerError::PayloadValidation(format!(
                            "{error_prefix_message}Value in set statement must be a string: {e}"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?;

            // 2. Validate ISO codes
            for v in &values {
                if !is_iso3166_1_alpha2(v) && !is_iso3166_2(v) {
                    return Err(ServerError::PayloadValidation(format!(
                        "{error_prefix_message}Value `{0}` of attribute tag `{1}` must be ISO3166-1 Alpha-2 code in upper case (e.g. `DE`) or ISO3166-2 codes (e.g. `ES-B`, `US-CA`)",
                        v,
                        statement.attribute_tag(),
                    )));
                }
            }
        }

        ATTRIBUTE_TAG_ID_DOC_TYPE => {
            // 1. Ensure all values are strings
            let values: Vec<&str> = statement
                .set()
                .iter()
                .map(|attr| {
                    ensure_string(attr).map_err(|e| {
                        ServerError::PayloadValidation(format!(
                            "{error_prefix_message}Value in set statement must be a string: {e}"
                        ))
                    })
                })
                .collect::<Result<_, _>>()?;

            // 2. Validate ID doc type
            for v in &values {
                IdDocType::parse(v).map_err(|e| {
                    ServerError::PayloadValidation(format!(
                        "{error_prefix_message}ID doc type invalid format: {e}"
                    ))
                })?;
            }
        }

        _ => {
            return Err(ServerError::PayloadValidation(format!(
                "{error_prefix_message}Attribute tag `{0}` is not allowed to be used in-set/not-in-set statements.
                Only `ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE(4)`, `ATTRIBUTE_TAG_NATIONALITY(5)`, `ATTRIBUTE_TAG_LEGAL_COUNTRY(15)`, `ATTRIBUTE_TAG_ID_DOC_ISSUER(8)`, and `ATTRIBUTE_TAG_ID_DOC_TYPE(6)` allowed in in-set/not-in-set statements.",
                statement.attribute_tag()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use concordium_rust_sdk::id::constants::AttributeKind;
    use concordium_rust_sdk::id::id_proof_types::{
        AttributeInRangeStatement, AttributeInSetStatement,
    };
    use concordium_rust_sdk::id::types::AttributeTag;
    use concordium_rust_sdk::web3id::Web3IdAttribute;
    use std::collections::BTreeSet;
    use std::marker::PhantomData;

    fn assert_error_contains(result: Result<(), ServerError>, expected: &str) {
        let err = result.expect_err("expected error but got Ok");
        let msg = err.to_string();
        assert!(msg.contains(expected), "unexpected error message: {}", msg);
    }

    #[test]
    fn test_iso8601_valid() {
        assert!(is_iso8601("20240131").is_ok());
    }

    #[test]
    fn test_iso8601_non_digits() {
        assert_error_contains(is_iso8601("2024ABCD"), "Date characters must be digits");
    }

    #[test]
    fn test_iso8601_invalid_month() {
        assert_error_contains(is_iso8601("20241301"), "Month must be between 1-12");
    }

    #[test]
    fn test_iso8601_invalid_day() {
        assert_error_contains(is_iso8601("20240199"), "Day must be between 1-31");
    }

    #[test]
    fn test_iso3166_1_alpha2_valid() {
        assert!(is_iso3166_1_alpha2("DE"));
        assert!(is_iso3166_1_alpha2("US"));
    }

    #[test]
    fn test_iso3166_1_alpha2_lowercase_invalid() {
        assert!(!is_iso3166_1_alpha2("de"));
    }

    #[test]
    fn test_iso3166_1_alpha2_invalid_code() {
        assert!(!is_iso3166_1_alpha2("ZZ"));
    }

    #[test]
    fn test_iso3166_2_valid() {
        assert!(is_iso3166_2("DE-BE"));
        assert!(is_iso3166_2("US-CA"));
        assert!(is_iso3166_2("FR-75"));
        //
    }

    #[test]
    fn test_iso3166_2_missing_dash() {
        assert!(!is_iso3166_2("DEBE"));
    }

    #[test]
    fn test_iso3166_2_invalid_country() {
        assert!(!is_iso3166_2("ZZ-123"));
    }

    #[test]
    fn test_iso3166_2_too_long_suffix() {
        assert!(!is_iso3166_2("DE-ABCD"));
    }

    #[test]
    fn test_id_doc_type_valid() {
        assert!(IdDocType::parse("0").is_ok());
        assert!(IdDocType::parse("1").is_ok());
        assert!(IdDocType::parse("4").is_ok());
    }

    #[test]
    fn test_id_doc_type_invalid() {
        let err = IdDocType::parse("9").unwrap_err();
        assert!(
            err.to_string().contains("Invalid ID document type"),
            "unexpected error: {}",
            err
        );

        let err = IdDocType::parse("passport").unwrap_err();
        assert!(
            err.to_string().contains("Invalid ID document type"),
            "unexpected error: {}",
            err
        );
    }

    // --------------------
    // Helpers to create set statements
    // --------------------

    fn make_country_set_statement(
        values: Vec<&str>,
    ) -> AttributeInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
        let set: BTreeSet<Web3IdAttribute> = values
            .into_iter()
            .map(|v| Web3IdAttribute::String(AttributeKind::try_new(v.into()).unwrap()))
            .collect();

        AttributeInSetStatement {
            attribute_tag: ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE,
            set,
            _phantom: PhantomData,
        }
    }

    fn make_id_doc_type_set_statement(
        values: Vec<&str>,
    ) -> AttributeInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
        let set: BTreeSet<Web3IdAttribute> = values
            .into_iter()
            .map(|v| Web3IdAttribute::String(AttributeKind::try_new(v.into()).unwrap()))
            .collect();

        AttributeInSetStatement {
            attribute_tag: ATTRIBUTE_TAG_ID_DOC_TYPE,
            set,
            _phantom: PhantomData,
        }
    }

    // --------------------
    // Set statement tests
    // --------------------

    #[test]
    fn test_set_statement_valid_countries() {
        let stmt = make_country_set_statement(vec!["DE", "US", "GB"]);
        assert!(validate_set_statement(&stmt).is_ok());
    }

    #[test]
    fn test_set_statement_invalid_country() {
        let stmt = make_country_set_statement(vec!["DE", "ZZ"]);
        assert_error_contains(
            validate_set_statement(&stmt),
            "must be ISO3166-1 Alpha-2 code",
        );
    }

    #[test]
    fn test_set_statement_empty() {
        let stmt = make_country_set_statement(vec![]);
        assert_error_contains(
            validate_set_statement(&stmt),
            "Set Statement should not be empty",
        );
    }

    #[test]
    fn test_set_statement_valid_id_doc_types() {
        let stmt = make_id_doc_type_set_statement(vec!["0", "1", "3"]);
        assert!(validate_set_statement(&stmt).is_ok());
    }

    #[test]
    fn test_set_statement_invalid_id_doc_type() {
        let stmt = make_id_doc_type_set_statement(vec!["0", "5"]);
        assert_error_contains(validate_set_statement(&stmt), "Invalid ID document type");
    }

    #[test]
    fn test_set_statement_disallowed_tag() {
        let mut stmt = make_id_doc_type_set_statement(vec!["0", "5"]);
        stmt.attribute_tag = ATTRIBUTE_TAG_ID_DOC_ISSUED_AT; // Not allowed for set
        assert_error_contains(
            validate_set_statement(&stmt),
            "is not allowed to be used in-set/not-in-set statements",
        );
    }

    // --------------------
    // Helpers to create range statements
    // --------------------

    fn make_range_statement(
        lower: &str,
        upper: &str,
    ) -> AttributeInRangeStatement<ArCurve, AttributeTag, Web3IdAttribute> {
        AttributeInRangeStatement {
            attribute_tag: ATTRIBUTE_TAG_DOB,
            lower: Web3IdAttribute::String(AttributeKind::try_new(lower.into()).unwrap()),
            upper: Web3IdAttribute::String(AttributeKind::try_new(upper.into()).unwrap()),
            _phantom: PhantomData,
        }
    }

    // --------------------
    // Range statement tests
    // --------------------

    #[test]
    fn test_range_statement_valid_dates() {
        let stmt = make_range_statement("19900101", "20200101");
        assert!(validate_range_statement(stmt).is_ok());
    }

    #[test]
    fn test_range_statement_upper_less_than_lower() {
        let stmt = make_range_statement("20200101", "19900101");
        assert_error_contains(
            validate_range_statement(stmt),
            "Provided `upper bound: 19900101` must be greater than `lower bound: 20200101`.",
        );
    }

    #[test]
    fn test_range_statement_invalid_upper_format() {
        let stmt = make_range_statement("19900101", "2020ABCD");
        assert_error_contains(
            validate_range_statement(stmt),
            "Upper bound value invalid format",
        );
    }

    #[test]
    fn test_range_statement_invalid_lower_format() {
        let stmt = make_range_statement("1990ABCD", "20200101");
        assert_error_contains(
            validate_range_statement(stmt),
            "Lower bound value invalid format:",
        );
    }

    #[test]
    fn test_range_statement_disallowed_tag() {
        let mut stmt = make_range_statement("19900101", "20200101");
        stmt.attribute_tag = ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE; // Not allowed for range
        assert_error_contains(
            validate_range_statement(stmt),
            "is not allowed to be used in range statements",
        );
    }
}
