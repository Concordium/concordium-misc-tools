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
    requested_subject_claims: &Vec<RequestedSubjectClaims>,
    ctx: &mut ValidationContext,
    path: &str, // requested subject claims path on the request
) {
    for claim in requested_subject_claims {
        match claim {
            Identity(id_claim) => {
                for (idx, statement) in id_claim.statements.iter().enumerate() {
                    match statement {
                        RevealAttribute(_) => {
                            // Nothing to validate here.
                        }
                        AttributeInRange(statement) => {
                            validate_range_statement(statement, path, ctx);
                        }
                        AttributeInSet(statement) => {
                            validate_set_statement(statement, ctx, path);
                        }
                        AttributeNotInSet(statement) => {
                            validate_set_statement(statement, ctx, path);
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
                return false;
            }
        }
    }
}

fn validate_range_statement(
    statement: &AttributeInRangeStatement<ArCurve, AttributeTag, Web3IdAttribute>,
    path: &str,
    ctx: &mut ValidationContext,
) -> bool {
    let mut is_valid = true;

    if statement.upper <= statement.lower {
        is_valid = false;
        let message = format!(
            "Provided `upper bound: {0}` must be greater than `lower bound: {1}`.",
            statement.upper, statement.lower
        );

        ctx.add_error_detail(ErrorDetail {
            code: "ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID".to_string(),
            path: path.to_string(),
            message,
        });
    };

    match statement.attribute_tag {
        ATTRIBUTE_TAG_DOB | ATTRIBUTE_TAG_ID_DOC_ISSUED_AT | ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT => {
            // check that upper bound contains a string, and is a valid date
            let is_valid_upper_bound = ensure_string(&statement.upper, ctx, path).map_or_else(
                || false,
                |upper_bound| validate_date_is_iso8601(upper_bound, path, ctx),
            );

            // check that lower bound contains a string, and is a valid date
            let is_valid_lower_bound = ensure_string(&statement.lower, ctx, path).map_or_else(
                || false,
                |lower_bound| validate_date_is_iso8601(lower_bound, path, ctx),
            );

            // if upper or lower is invalid, then we have an invalid statement
            if !is_valid_upper_bound || !is_valid_lower_bound {
                is_valid = false;
            }
        }
        _ => {
            // If we enter this block, the attribute tag specified is invalid
            is_valid = false;

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
            let attribute_tag = statement.attribute_tag().0.to_string();

            for (i, v) in get_values() {
                if !validate_is_country_code_valid_iso3166_1_alpha2(&v)
                    && !validate_is_country_code_valid_iso3166_2(&v)
                {
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
                IdDocType::validate_doc_type_string(&v, path, ctx);
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
