use concordium_rust_sdk::base::web3id::v1::anchor::RequestedSubjectClaims;

use crate::{api_types::ErrorDetail, validation::validation_context::ValidationContext};

use chrono::NaiveDate;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    RequestedStatement::{AttributeInRange, AttributeInSet, AttributeNotInSet, RevealAttribute},
    RequestedSubjectClaims::Identity,
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
const ATTRIBUTE_TAG_DOB: AttributeTag = AttributeTag(3);
const ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE: AttributeTag = AttributeTag(4);
const ATTRIBUTE_TAG_NATIONALITY: AttributeTag = AttributeTag(5);
const ATTRIBUTE_TAG_ID_DOC_TYPE: AttributeTag = AttributeTag(6);
const ATTRIBUTE_TAG_ID_DOC_ISSUER: AttributeTag = AttributeTag(8);
const ATTRIBUTE_TAG_ID_DOC_ISSUED_AT: AttributeTag = AttributeTag(9);
const ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT: AttributeTag = AttributeTag(10);
const ATTRIBUTE_TAG_LEGAL_COUNTRY: AttributeTag = AttributeTag(15);

/// The entry point to validate a Vector of Requested Subject claims.
/// Both the create verification request and the verify presentation api's
/// have structures that contain a vector of requested subject claims.
/// This function handles the enumeration and validation of that structure
/// and appends new error details into the provided Validation Context.
pub fn validate(
    requested_subject_claims: &[RequestedSubjectClaims],
    ctx: &mut ValidationContext,
    path: &str, // requested subject claims path on the request
) {
    for (claim_idx, claim) in requested_subject_claims.iter().enumerate() {
        match claim {
            Identity(id_claim) => {
                for (idx, statement) in id_claim.statements.iter().enumerate() {
                    let statement_path = format!("{path}[{claim_idx}].statements[{idx}]");
                    match statement {
                        RevealAttribute(_) => {
                            // Nothing to validate here.
                        }
                        AttributeInRange(statement) => {
                            validate_range_statement(statement, &statement_path, ctx);
                        }
                        AttributeInSet(statement) => {
                            validate_set_statement(statement, ctx, &statement_path);
                        }
                        AttributeNotInSet(statement) => {
                            validate_set_statement(statement, ctx, &statement_path);
                        }
                    }
                }
            }
        }
    }
}

fn ensure_string<'a>(
    attr: &'a Web3IdAttribute,
    ctx: &mut ValidationContext,
    path: &str,
) -> Option<&'a str> {
    match attr {
        Web3IdAttribute::String(v) => Some(v.as_ref()),
        _ => {
            ctx.add_error_detail(ErrorDetail {
                code: "ATTRIBUTE_NOT_STRING".to_string(),
                path: path.to_string(),
                message: "Expected string value in attribute".to_string(),
            });
            None
        }
    }
}

/// ISO8601 strings representing dates as `YYYYMMDD`.
fn validate_date_is_iso8601(date: &str, path: &str, ctx: &mut ValidationContext) -> bool {
    let mut is_valid = false;
    // Must be exactly 8 characters
    if date.len() != 8 {
        let message = format!(
            "The given date should be 8 characters long (ISO8601 `YYYYMMDD` format) but given date `{}` is {} characters long.",
            date,
            date.len()
        );

        ctx.add_error_detail(ErrorDetail {
            code: "INVALID_DATE_FORMAT".to_string(),
            path: path.to_string(),
            message,
        });
        // no need to continue to further validations. client has enough info in above.
        return is_valid;
    }

    match NaiveDate::parse_from_str(date, "%Y%m%d") {
        Ok(_) => {
            is_valid = true;
        }
        Err(e) => {
            let message = format!(
                "Failed to parse `{}` as ISO8601 `YYYYMMDD` format: {}",
                date, e
            );

            ctx.add_error_detail(ErrorDetail {
                code: "INVALID_DATE_FORMAT".to_string(),
                path: path.to_string(),
                message,
            });
        }
    }

    is_valid
}

/// ISO3166_1_alpha2 codes consist of 2 upper case characters representing countries/regions (e.g. `GB, DE, DK`).
fn validate_is_country_code_valid_iso3166_1_alpha2(code: &str) -> bool {
    rust_iso3166::from_alpha2(code).is_some()
        && code.len() == 2
        && code.chars().all(|c| c.is_ascii_uppercase())
}

/// ISO3166-2 codes consist of a ISO3166_1_alpha2 code, then a dash, and then 1-3 alphanumerical characters
/// representing countries/regions (e.g. `ES-B`, `US-CA`).
fn validate_is_country_code_valid_iso3166_2(code: &str) -> bool {
    if code.len() < 4 || code.len() > 6 {
        // 2 letters + '-' + 1-3 characters
        return false;
    }

    rust_iso3166::iso3166_2::from_code(code).is_some()
}

#[derive(Debug)]
#[allow(dead_code)]
enum IdDocType {
    NA,
    Passport,
    NationalIdCard,
    DriversLicense,
    ImmigrationCard,
}

impl IdDocType {
    /// Try to parse a string into an IdDocType
    fn validate_doc_type_string(code: &str, path: &str, ctx: &mut ValidationContext) -> bool {
        match code {
            "0" => true,
            "1" => true,
            "2" => true,
            "3" => true,
            "4" => true,
            _ => {
                let message = format!(
                    "Invalid ID document type `{}`. Must be one of: 0 (N/A), 1 (Passport), 2 (NationalIdCard), 3 (DriversLicense), or 4 (ImmigrationCard).",
                    code
                );
                ctx.add_error_detail(ErrorDetail {
                    code: "INVALID_ID_DOC_TYPE".to_string(),
                    path: path.to_string(),
                    message,
                });
                false
            }
        }
    }
}

/// Helper to determine if string provided is numeric
fn parse_u64_or_provide_error(
    attr: &Web3IdAttribute,
    path: &str,
    ctx: &mut ValidationContext,
    error: &mut ErrorDetail,
) -> Option<u64> {
    let s = ensure_string(attr, ctx, path)?;
    match s.parse::<u64>() {
        Ok(n) => Some(n),
        Err(_) => {
            error.path = path.to_string();
            ctx.add_error_detail(error.clone());
            None
        }
    }
}

fn validate_range_statement(
    statement: &AttributeInRangeStatement<ArCurve, AttributeTag, Web3IdAttribute>,
    path: &str,
    ctx: &mut ValidationContext,
) -> bool {
    let mut is_valid = true;

    let mut not_numeric_error = ErrorDetail {
        code: "ATTRIBUTE_IN_RANGE_STATEMENT_NOT_NUMERIC".to_string(),
        path: path.to_string(),
        message: "Attribute in range statement, is a numeric range check between a lower and upper bound. These must be numeric values.".to_string(),
    };

    // parse lower and upper bounds to make sure they are numeric
    let upper_bound = parse_u64_or_provide_error(
        &statement.upper,
        &format!("{path}.upper"),
        ctx,
        &mut not_numeric_error,
    );
    let lower_bound = parse_u64_or_provide_error(
        &statement.lower,
        &format!("{path}.lower"),
        ctx,
        &mut not_numeric_error,
    );

    match (upper_bound, lower_bound) {
        (Some(upper), Some(lower)) => {
            if upper < lower {
                is_valid = false;
                let path = format!("{path}.upper");
                ctx.add_error_detail(ErrorDetail {
                    code: "ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID".to_string(),
                    path,
                    message: format!(
                        "Provided `upper bound: {}` must be greater than `lower bound: {}`.",
                        &statement.upper, &statement.lower
                    ),
                });
            }
        }
        // parse_u64_or_provide_error provides errors if a value
        // was not parseable so here is empty
        _ => {
            is_valid = false;
        }
    }

    // if the above is numerically valid until now, then we can proceed to assess
    // the statement for specific tags
    if is_valid {
        match statement.attribute_tag {
            ATTRIBUTE_TAG_DOB
            | ATTRIBUTE_TAG_ID_DOC_ISSUED_AT
            | ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT => {
                // check that upper bound contains a string, and is a valid date
                let is_valid_upper_bound = ensure_string(&statement.upper, ctx, path).map_or_else(
                    || false,
                    |upper_bound| {
                        let path = format!("{path}.upper");
                        validate_date_is_iso8601(upper_bound, &path, ctx)
                    },
                );

                // check that lower bound contains a string, and is a valid date
                let is_valid_lower_bound = ensure_string(&statement.lower, ctx, path).map_or_else(
                    || false,
                    |lower_bound| {
                        let path = format!("{path}.lower");
                        validate_date_is_iso8601(lower_bound, &path, ctx)
                    },
                );

                // if upper or lower is invalid, then we have an invalid statement
                if !is_valid_upper_bound || !is_valid_lower_bound {
                    is_valid = false;
                }
            }
            _ => {
                // If we enter this block, the attribute tag specified is invalid
                is_valid = false;
                let path = format!("{path}.attributeTag");

                let message = format!(
                    "Attribute tag `{0}` is not allowed to be used in range statements. \
                    Only `ATTRIBUTE_TAG_DOB(3)`, `ATTRIBUTE_TAG_ID_DOC_ISSUED_AT(9)`, and `ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT(10)` allowed in range statements.",
                    statement.attribute_tag
                );

                ctx.add_error_detail(ErrorDetail {
                    code: "ATTRIBUTE_IN_RANGE_STATEMENT_INVALID_ATTRIBUTE_TAG".to_string(),
                    path: path.to_string(),
                    message,
                });
            }
        }
    }

    is_valid
}

fn validate_set_statement<S>(
    statement: &S,
    ctx: &mut ValidationContext,
    path: &str, // e.g. "claims[0].identity.statements[5]"
) -> bool
where
    S: HasSet<Item = Web3IdAttribute>,
{
    let mut is_valid = true;

    if statement.set().is_empty() {
        is_valid = false;
        ctx.add_error_detail(ErrorDetail {
            code: "INVALID_SET_CANNNOT_BE_EMPTY".to_string(),
            path: "dummy path".to_string(),
            message: "Set statement should not be empty.".to_string(),
        });
        return is_valid;
    }

    // helper for iterating with index and path
    let mut get_values = || {
        statement
            .set()
            .iter()
            .enumerate()
            .filter_map(|(i, attr)| {
                let p = format!("{path}.set[{i}]");
                ensure_string(attr, ctx, &p).map(|s| (i, s.to_string()))
            })
            .collect::<Vec<_>>()
    };

    match statement.attribute_tag() {
        ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE
        | ATTRIBUTE_TAG_NATIONALITY
        | ATTRIBUTE_TAG_LEGAL_COUNTRY => {
            for (i, v) in get_values() {
                if !validate_is_country_code_valid_iso3166_1_alpha2(&v) {
                    is_valid = false;
                    let path = format!("{path}.set[{i}]");
                    ctx.add_error_detail(
                        ErrorDetail {
                            code: "COUNTRY_CODE_INVALID".to_string(), 
                            path,
                            message: "Country code must be 2 letter and both uppercase following the ISO3166-1 alpha-2 uppercase standard. (e.g `DE`)".to_string() 
                        }
                    );
                }
            }
        }
        ATTRIBUTE_TAG_ID_DOC_ISSUER => {
            for (i, v) in get_values() {
                if !validate_is_country_code_valid_iso3166_1_alpha2(&v)
                    && !validate_is_country_code_valid_iso3166_2(&v)
                {
                    is_valid = false;
                    let path = format!("{path}.set[{i}]");
                    ctx.add_error_detail(
                        ErrorDetail {
                            code: "INVALID_ISSUER_CODE".to_string(), 
                            path,
                            message: "Must be ISO3166-1 alpha-2 uppercase (e.g. `DE`) or ISO3166-2 (e.g. `US-CA`)".to_string()
                        }
                    );
                }
            }
        }
        ATTRIBUTE_TAG_ID_DOC_TYPE => {
            for (i, v) in get_values() {
                let path = format!("{path}.set[{i}]");
                IdDocType::validate_doc_type_string(&v, &path, ctx);
            }
        }
        _ => {
            is_valid = false;

            let message = format!(
                "Attribute tag {} not allowed for set statements (allowed: 4, 5, 15, 8, 6)",
                statement.attribute_tag()
            );

            ctx.add_error_detail(ErrorDetail {
                code: "UNSUPPORTED_ATTRIBUTE_TAG".to_string(),
                path: path.to_string(),
                message,
            });
        }
    }

    is_valid
}

trait HasSet {
    type Item;
    fn set(&self) -> &BTreeSet<Web3IdAttribute>;
    fn attribute_tag(&self) -> AttributeTag;
}

impl HasSet for AttributeInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
    type Item = Web3IdAttribute;

    fn set(&self) -> &BTreeSet<Web3IdAttribute> {
        &self.set
    }

    fn attribute_tag(&self) -> AttributeTag {
        self.attribute_tag
    }
}

impl HasSet for AttributeNotInSetStatement<ArCurve, AttributeTag, Web3IdAttribute> {
    type Item = Web3IdAttribute;

    fn set(&self) -> &BTreeSet<Web3IdAttribute> {
        &self.set
    }

    fn attribute_tag(&self) -> AttributeTag {
        self.attribute_tag
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use concordium_rust_sdk::{
        base::web3id::v1::anchor::{RequestedIdentitySubjectClaims, RequestedStatement},
        id::constants::AttributeKind,
    };

    use super::*;

    // --------------------
    // Helpers to create set statements
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

    #[test]
    fn test_iso8601_valid() {
        let mut ctx = ValidationContext::new();
        let is_valid = validate_date_is_iso8601("20240131", "dummy", &mut ctx);
        assert!(is_valid);
        assert!(ctx.error_details.is_empty());
    }

    #[test]
    fn test_iso8601_invalid_characters() {
        let mut ctx = ValidationContext::new();
        let is_valid = validate_date_is_iso8601("2024ABCD", "dummy", &mut ctx);
        assert!(!is_valid);
        assert!(ctx.error_details.len() == 1);

        let detail = &ctx.error_details[0];

        assert_eq!(detail.code, "INVALID_DATE_FORMAT".to_string());
        assert_eq!(detail.message, "Failed to parse `2024ABCD` as ISO8601 `YYYYMMDD` format: input contains invalid characters".to_string())
    }

    #[test]
    fn test_iso8601_invalid_month() {
        let mut ctx = ValidationContext::new();
        let is_valid = validate_date_is_iso8601("20241301", "dummy", &mut ctx);
        assert!(!is_valid);
        assert!(ctx.error_details.len() == 1);
        let detail = &ctx.error_details[0];

        assert_eq!(detail.code, "INVALID_DATE_FORMAT".to_string());
        assert_eq!(
            detail.message,
            "Failed to parse `20241301` as ISO8601 `YYYYMMDD` format: input is out of range"
                .to_string()
        )
    }

    #[test]
    fn test_iso8601_invalid_day() {
        let mut ctx = ValidationContext::new();
        let is_valid = validate_date_is_iso8601("20241232", "dummy", &mut ctx);
        assert!(!is_valid);
        assert!(ctx.error_details.len() == 1);
        let detail = &ctx.error_details[0];

        assert_eq!(detail.code, "INVALID_DATE_FORMAT".to_string());
        assert_eq!(
            detail.message,
            "Failed to parse `20241232` as ISO8601 `YYYYMMDD` format: input is out of range"
                .to_string()
        )
    }

    #[test]
    fn test_iso3166_1_alpha2_valid() {
        assert!(validate_is_country_code_valid_iso3166_1_alpha2("DE"));
        assert!(validate_is_country_code_valid_iso3166_1_alpha2("US"));
    }

    #[test]
    fn test_iso3166_1_alpha2_lowercase_invalid() {
        assert!(!validate_is_country_code_valid_iso3166_1_alpha2("de"));
    }

    #[test]
    fn test_iso3166_1_alpha2_invalid_code() {
        assert!(!validate_is_country_code_valid_iso3166_1_alpha2("ZZ"));
    }

    #[test]
    fn test_iso3166_2_valid() {
        assert!(validate_is_country_code_valid_iso3166_2("DE-BE"));
        assert!(validate_is_country_code_valid_iso3166_2("US-CA"));
        assert!(validate_is_country_code_valid_iso3166_2("FR-75"));
    }

    #[test]
    fn test_iso3166_2_missing_dash() {
        assert!(!validate_is_country_code_valid_iso3166_2("DEBE"));
    }

    #[test]
    fn test_iso3166_2_invalid_country() {
        assert!(!validate_is_country_code_valid_iso3166_2("ZZ-123"));
    }

    #[test]
    fn test_iso3166_2_too_long_suffix() {
        assert!(!validate_is_country_code_valid_iso3166_2("DE-ABCD"));
    }

    #[test]
    fn test_id_doc_type_valid() {
        let mut ctx = ValidationContext::new();
        assert!(IdDocType::validate_doc_type_string("0", "dummy", &mut ctx));
        assert!(IdDocType::validate_doc_type_string("1", "dummy", &mut ctx));
        assert!(IdDocType::validate_doc_type_string("4", "dummy", &mut ctx));
    }

    #[test]
    fn test_id_doc_type_invalid() {
        let mut ctx = ValidationContext::new();
        let is_valid = IdDocType::validate_doc_type_string("5", "dummy", &mut ctx);
        assert!(!is_valid);
        assert!(ctx.error_details.len() == 1);
        let detail = &ctx.error_details[0];

        assert_eq!(detail.code, "INVALID_ID_DOC_TYPE".to_string());
        assert_eq!(detail.message, "Invalid ID document type `5`. Must be one of: 0 (N/A), 1 (Passport), 2 (NationalIdCard), 3 (DriversLicense), or 4 (ImmigrationCard).".to_string())
    }

    #[test]
    fn test_range_statement_valid_dates() {
        let mut ctx = ValidationContext::new();
        let stmt = make_range_statement("19900101", "20200101");

        validate_range_statement(&stmt, "some.path", &mut ctx);

        assert!(!ctx.has_errors());
    }

    #[test]
    fn test_range_statement_upper_bound_less_than_lower() {
        let mut ctx = ValidationContext::new();
        let stmt = make_range_statement("20200101", "19900101");
        let path = "requestedClaims[0]".to_string();

        validate_range_statement(&stmt, &path, &mut ctx);
        println!("context: {:?}", &ctx);

        // assertions - ensure context has just one error related to the bounds issue
        assert!(ctx.has_errors());

        let error_details = ctx.error_details;
        assert_eq!(1, error_details.len());

        let expected_code = "ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID".to_string();
        let expected_message =
            "Provided `upper bound: 19900101` must be greater than `lower bound: 20200101`."
                .to_string();
        let error_detail = &error_details[0];
        assert_eq!(error_detail.code, expected_code);
        assert_eq!(error_detail.message, expected_message);

        let expected_path = "requestedClaims[0].upper";
        assert_eq!(expected_path, error_detail.path);
    }

    #[test]
    fn test_range_statement_upper_bound_date_not_valid() {
        let mut ctx = ValidationContext::new();
        let stmt = make_range_statement("20200101", "1ascas");
        let path = "dummy".to_string();

        validate_range_statement(&stmt, &path, &mut ctx);
        println!("context: {:?}", ctx);

        // assertions - ensure context has just one error related to the bounds issue
        assert!(ctx.has_errors());

        let error_details = ctx.error_details;
        assert_eq!(1, error_details.len());

        let expected_code = "ATTRIBUTE_IN_RANGE_STATEMENT_NOT_NUMERIC".to_string();
        let expected_message = "Attribute in range statement, is a numeric range check between a lower and upper bound. These must be numeric values.".to_string();
        let error_detail = &error_details[0];
        assert_eq!(error_detail.code, expected_code);
        assert_eq!(error_detail.message, expected_message);

        let expected_path = "dummy.upper";
        assert_eq!(expected_path, error_detail.path);
    }

    #[test]
    fn validate_requested_subject_claims_attribute_in_range_statement_dob_invalid() {
        let mut ctx = ValidationContext::new();
        let stmt = make_range_statement("19900101", "100000000000");
        let path = "requestedClaims[0]".to_string();

        validate_range_statement(&stmt, &path, &mut ctx);
        println!("context: {:?}", ctx);

        // assertions - ensure context has just one error related to the bounds issue
        assert!(ctx.has_errors());

        let error_details = ctx.error_details;
        assert_eq!(1, error_details.len());

        let expected_code = "INVALID_DATE_FORMAT".to_string();
        let expected_message = "The given date should be 8 characters long (ISO8601 `YYYYMMDD` format) but given date `100000000000` is 12 characters long.".to_string();
        let error_detail = &error_details[0];
        assert_eq!(error_detail.code, expected_code);
        assert_eq!(error_detail.message, expected_message);
        let expected_path = "requestedClaims[0].upper";
        assert_eq!(expected_path, error_detail.path);
    }

    #[test]
    fn validate_requested_subject_claims_attribute_in_range_statement_lower_and_upper_date_invalid()
    {
        let mut ctx = ValidationContext::new();
        let stmt = make_range_statement("1990010101", "2020010101");
        let path = "requestedClaims[0]".to_string();

        validate_range_statement(&stmt, &path, &mut ctx);
        println!("context: {:?}", ctx);

        // assertions - ensure context has just one error related to the bounds issue
        assert!(ctx.has_errors());

        let error_details = ctx.error_details;
        assert_eq!(2, error_details.len());

        let expected_code = "INVALID_DATE_FORMAT".to_string();
        let expected_message =
            "The given date should be 8 characters long (ISO8601 `YYYYMMDD` format) but given date";

        for error_detail in error_details {
            assert_eq!(error_detail.code, expected_code);
            assert!(error_detail.message.starts_with(expected_message));
        }
    }

    #[test]
    fn test_set_statement_valid_countries() {
        let stmt = make_country_set_statement(vec!["DE", "US", "GB"]);
        let mut ctx = ValidationContext::new();
        let path = "dummy";

        let result = validate_set_statement(&stmt, &mut ctx, path);
        assert!(result);
    }

    #[test]
    fn test_set_statement_invalid_country() {
        let stmt = make_country_set_statement(vec!["DE", "ZZ", "GB"]);
        let mut ctx = ValidationContext::new();
        let path = "dummy";

        let is_valid_set_statement = validate_set_statement(&stmt, &mut ctx, path);
        assert!(!is_valid_set_statement);

        assert_eq!(1, ctx.error_details.len());
        let detail = &ctx.error_details[0];
        assert_eq!(detail.code, "COUNTRY_CODE_INVALID".to_string());
        assert_eq!(detail.message, "Country code must be 2 letter and both uppercase following the ISO3166-1 alpha-2 uppercase standard. (e.g `DE`)".to_string())
    }

    #[test]
    fn test_set_statement_empty() {
        let stmt = make_country_set_statement(vec![]);
        let mut ctx = ValidationContext::new();
        let path = "dummy";

        let is_valid = validate_set_statement(&stmt, &mut ctx, path);
        assert!(!is_valid);

        assert_eq!(1, ctx.error_details.len());
        let detail = &ctx.error_details[0];
        assert_eq!(detail.code, "INVALID_SET_CANNNOT_BE_EMPTY".to_string());
        assert_eq!(
            detail.message,
            "Set statement should not be empty.".to_string()
        )
    }

    // --------------------
    // Requested Subject Level Claims Checks
    // --------------------

    #[test]
    fn validate_requested_subject_claims_passes() {
        let mut ctx = ValidationContext::new();
        let path = "requestedClaims";

        let identity_claims = RequestedIdentitySubjectClaims {
            statements: vec![],
            issuers: vec![],
            source: vec![],
        };

        let requested_subject_claims = RequestedSubjectClaims::Identity(identity_claims);
        let vec_requested_subject_claims = vec![requested_subject_claims];

        validate(&vec_requested_subject_claims, &mut ctx, path);

        assert!(ctx.has_errors());
    }

    #[test]
    fn validate_requested_subject_claims_invalid_range_and_invalid_set() {
        let mut ctx = ValidationContext::new();
        let path = "requestedClaims";

        // valid date of birth range statement
        let range_statement =
            RequestedStatement::AttributeInRange(make_range_statement("19880101", "19780101"));
        let country_statement =
            RequestedStatement::AttributeInSet(make_country_set_statement(vec!["AAA"]));

        let identity_claims = RequestedIdentitySubjectClaims {
            statements: vec![range_statement, country_statement],
            issuers: vec![],
            source: vec![],
        };

        let requested_subject_claims = RequestedSubjectClaims::Identity(identity_claims);
        let vec_requested_subject_claims = vec![requested_subject_claims];

        // call validate now
        validate(&vec_requested_subject_claims, &mut ctx, path);
        println!("*** ctx: {:?}", &ctx);

        // assertions for expected errors
        assert!(ctx.has_errors());
        assert_eq!(ctx.error_details.len(), 2);

        let bounds_invalid = &ctx.get_error_by_code("ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID");
        assert!(bounds_invalid.is_some());
        assert_eq!(
            bounds_invalid.unwrap().message,
            "Provided `upper bound: 19780101` must be greater than `lower bound: 19880101`."
        );

        let country_code_invalid_invalid = ctx.get_error_by_code("COUNTRY_CODE_INVALID");
        assert!(country_code_invalid_invalid.is_some());
        assert_eq!(
            country_code_invalid_invalid.unwrap().message,
            "Country code must be 2 letter and both uppercase following the ISO3166-1 alpha-2 uppercase standard. (e.g `DE`)"
        );
    }

    #[test]
    fn validate_requested_subject_claims_valid_doc_id_set_statement() {
        let mut ctx = ValidationContext::new();
        let path = "requestedClaims";

        let statement =
            RequestedStatement::AttributeInSet(make_id_doc_type_set_statement(vec!["0", "1", "3"]));

        let identity_claims = RequestedIdentitySubjectClaims {
            statements: vec![statement],
            issuers: vec![],
            source: vec![],
        };

        let requested_subject_claims = RequestedSubjectClaims::Identity(identity_claims);
        let vec_requested_subject_claims = vec![requested_subject_claims];

        validate(&vec_requested_subject_claims, &mut ctx, path);

        assert!(!ctx.has_errors());
    }

    #[test]
    fn validate_requested_subject_claims_invalid_id_doc_type() {
        let mut ctx = ValidationContext::new();
        let path = "requestedClaims";

        let statement =
            RequestedStatement::AttributeInSet(make_id_doc_type_set_statement(vec!["0", "1", "5"]));

        let identity_claims = RequestedIdentitySubjectClaims {
            statements: vec![statement],
            issuers: vec![],
            source: vec![],
        };

        let requested_subject_claims = RequestedSubjectClaims::Identity(identity_claims);
        let vec_requested_subject_claims = vec![requested_subject_claims];

        validate(&vec_requested_subject_claims, &mut ctx, path);

        assert!(ctx.has_errors());
        assert!(ctx.error_details.len() == 1);
        let detail = &ctx.error_details[0];
        assert_eq!(detail.code, "INVALID_ID_DOC_TYPE".to_string());
        assert_eq!(detail.message, "Invalid ID document type `5`. Must be one of: 0 (N/A), 1 (Passport), 2 (NationalIdCard), 3 (DriversLicense), or 4 (ImmigrationCard).".to_string());
    }

    #[test]
    fn validate_requested_subject_claims_invalid_tag_for_id_doc_type_statement() {
        let mut ctx = ValidationContext::new();
        let path = "requestedClaims";

        let statement = RequestedStatement::AttributeInSet({
            let mut s = make_id_doc_type_set_statement(vec!["0", "1", "3"]);
            s.attribute_tag = ATTRIBUTE_TAG_ID_DOC_ISSUED_AT;
            s
        });

        let identity_claims = RequestedIdentitySubjectClaims {
            statements: vec![statement],
            issuers: vec![],
            source: vec![],
        };

        let requested_subject_claims = RequestedSubjectClaims::Identity(identity_claims);
        let vec_requested_subject_claims = vec![requested_subject_claims];

        validate(&vec_requested_subject_claims, &mut ctx, path);

        assert!(ctx.has_errors());
        assert!(ctx.error_details.len() == 1);
        let detail = &ctx.error_details[0];
        assert_eq!(detail.code, "UNSUPPORTED_ATTRIBUTE_TAG".to_string());
        assert_eq!(
            detail.message,
            "Attribute tag idDocIssuedAt not allowed for set statements (allowed: 4, 5, 15, 8, 6)"
                .to_string()
        );
    }
}
