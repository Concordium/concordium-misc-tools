
## Error API Response Examples



Error Response Structure:

```
{
  "error": {
    "code": 400, // http status code
    "type": "MACHINE_READABLE_CODE",
    "message": "Helpful top level Error message",
    "trace_id": "1234-abcd", // trace id for the request 
    "retryable": false, // boolean whether it is retry-able or not
    "details": [ // Error details array, contains all of the issues that happened for the request
      {
        "type": "ATTRIBUTE_IN_RANGE_STATEMENT_INVALID_ATTRIBUTE_TAG",
        "path": "requestedClaims.statements[0].attributeTag",
        "message": "Attribute tag is not valid for this range statement. Please check the valid attribute tags here: {} for the attribute in range statement."
      }
    ]
  }
}
```


## 5XX Errors

Currently for errors related to:

- node timeouts
- attempts to get account sequence number and retry get of sequence number
- service unavailable
- essentially all 500 range

Will take the following format:

```
{
  "error": {
    "code": 500,
    "type": "INTERNAL_SERVER_ERROR",
    "message": "Internal error.",
    "trace_id": "1234-abcd", 
    "retryable": true,
  }
}
```


## Http 4XX 

There are Two levels of validations and as follows:

### Json schema validations

json schema validations: required fields, variant matching etc - these will take the default json schema validation for now.

examples:

Response to unknown variant:

```
invalid json in request: Failed to deserialize the JSON body into the target type: requestedClaims[0].type: unknown variant `identityzzzz`, expected `identity` at line 12 column 28%
```

Response to required field missing:

```
invalid json in request: Failed to deserialize the JSON body into the target type: missing field `connectionId` at line 48 column 1%
```


Currently these are handled by library internals for json schema validation and will not follow the custom error format defined above.



## Request Validations - Statements Validations

These will follow the custom error payload defined above and the following will be implemented:


### Attribute in range statement checks


Attribute tag is not valid for range statement

```
Http Status code: 400
{
  "error": {
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 400,
    "type": "VALIDATION_ERROR",
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
    "code": 422,
    "type": "PRESENTATION_VERIFICATION_ERROR", // machine readable code
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



