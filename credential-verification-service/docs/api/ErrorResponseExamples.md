
## Error API Response Examples





### Generic Validations

- type provided in statement is not valid. The type must be one of: {}. Please check the valid attribute tags here: {}.


High level service Error:

```
Http Status code: 503
{
  "error": {
    "code": "SERVICE_UNAVAILABLE",
    "message": "The credential verification service is currently unavailable.",
    "trace_id": "1234-abcd",
    "retryable": true
  }
}
```

Upstream timeout calling the node for example:

```
Http Status code: 504
{
  "error": {
    "code": "NODE_REQUEST_TIMEOUT",
    "message": "The concordium node request timeout was reached. Please try again after some time.",
    "trace_id": "1234-abcd",
    "retryable": true
  }
}
```


Account sequence number retry and fail issues:

```
Http Status code: 500
{
  "error": {
    "code": "REGISTER_DATA_TRANSACTION_ISSUE",
    "message": "There was an issue while attempting to create the verifiable request anchor transaction on chain.",
    "trace_id": "1234-abcd", 
    "retryable": true, 
    "details": [
      {
        "code": "ACCOUNT_SEQUENCE_NUMBER_ISSUE",
        "message": "Multiple attempts trying to get the correct next account sequence number were made and a valid account sequence number could not be retrieved. Please try again later."
      }
    ]
  }
}
```



### Attribute in range statement checks


Attribute tag is not valid for range statement

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_RANGE_STATEMENT_INVALID_ATTRIBUTE_TAG",
        "path": "requestedClaims.statements[0].attributeTag",
        "message": "Attribute tag is not valid for this range statement. Please check the valid attribute tags here: {} for the attribute in range statement."
      }
    ]
  }
}
```

Attribute in range statement - lower bound was greater than upper bound

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_RANGE_STATEMENT_RANGE_INVALID",
        "path": "requestedClaims.statements[0].lower",
        "message": "Attribute in range statement validation failed. Provided `lower` bound: {} was greater than the `upper bound`: {}. This statement is intended to prove that an attribute tag provided occurs within the numeric range of the provided `lower` and `upper` bounds."
      }
    ]
  }
}
```

Attribute in range statement - bound not numeric

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_RANGE_STATEMENT_RANGE_NOT_NUMERIC",
        "path": "requestedClaims.statements[0].lower",
        "message": "Attribute in range statement validation failed. Provided `lower` bound: {} was not numeric. This statement is intended to prove that an attribute tag provided occurs within the numeric range of the provided `lower` and `upper` bounds."
      }
    ]
  }
}
```


Attribute in range statement - dob must be valid DOB

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_RANGE_STATEMENT_DOB_RANGE_NOT_VALID",
        "path": "requestedClaims.statements[0].lower",
        "message": "Attribute in range statement validation failed. `lower` and `upper` for attribute tag `dob` must be valid for date of birth format `YYYYMMDD`. This statement is intended to prove that an attribute tag provided occurs within the numeric range of the provided `lower` and `upper` bounds."
      }
    ]
  }
}
```



### Attribute in set statement

Attribute in set statement for tag `countryOfResidence` must contain a valid country set. 

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_SET_STATEMENT_COUNTRY_CODE_NOT_VALID",
        "path": "requestedClaims.statements[0].set",
        "message": "Attribute in set statement validation failed. set for attribute tag `countryOfResidence` must contain valid country codes. Please see documentation here: {} about valid country codes. This statement is intended to prove that an attribute tag provided occurs within the set provided."
      }
    ]
  }
}
```


Attribute in set statement for tag `dob` is not valid. The attribute in set statement is designed to prove that a provided attribute tag is contained within a set.

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_IN_SET_STATEMENT_ATTRIBUTE_TAG_NOT_VALID",
        "path": "requestedClaims.statements[0].attributeTag",
        "message": "Attribute in set statement validation failed. attribute tag provided: {} is not valid for the attribute in set statement. The attribute in set statement is designed to prove that a provided attribute tag is contained within a set."
      }
    ]
  }
}
```


### Attribute not in set statement

Attribute not in set statement for tag `countryOfResidence` must contain a valid country set. 

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_NOT_IN_SET_STATEMENT_COUNTRY_CODE_NOT_VALID",
        "path": "requestedClaims.statements[0].set",
        "message": "Attribute not in set statement validation failed. set for attribute tag `countryOfResidence` must contain valid country codes. Please see documentation here: {} about valid country codes. This statement is intended to prove that an attribute tag provided is not within the set."
      }
    ]
  }
}
```


Attribute not in set statement for tag `dob` is not valid. The attribute not in set statement is designed to prove that a provided attribute tag is contained within a set.

```
Http Status code: 400
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Create Verification Request did not pass request validation. Please see the details section below for the errors that have occurred.",
    "trace_id": "1234-abcd", 
    "retryable": false,
    "details": [
      {
        "code": "ATTRIBUTE_NOT_IN_SET_STATEMENT_ATTRIBUTE_TAG_NOT_VALID",
        "path": "requestedClaims.statements[0].attributeTag",
        "message": "Attribute not in set statement validation failed. attribute tag provided: {} is not valid for the attribute not in set statement. The attribute not in set statement is designed to prove that a provided attribute tag is not within the set."
      }
    ]
  }
}
```




## Verifiable Presentation Errors

Verifiable request anchor not found or mismatch for the block provided

```
Http Status code: 422
{
  "error": {
    "code": "PRESENTATION_VERIFICATION_ERROR", // machine readable code
    "message": "The provided verifiable presentation did not pass its verification, which means the presentation could not be cryptographically verified.",
    "trace_id": "1234-abcd", // the distributed trace id
    "retryable": false, // whether this request can be retried by the client/caller in its current format. In this case no, its a bad requst Http 400 - validation errors must be resolved.
    "details": [ // array of details for the errors that have occurred
      {
        "code": "VERIFIABLE_REQUEST_ANCHOR_MISMATCH",
        "path": "verificationRequest.transactionRef",
        "message": "The provided transaction hash in transactionRef was not found within the block hash provided in the verifiable presentation context."
      }
    ]
  }
}
```



