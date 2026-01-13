# Example Payloads

## Request Example

```json
{
  "connectionId": "conn_8291yuw",
  "resourceId": "some string for resource",
  "contextString": "optional context string here",
  "publicInfo": {
    "key1": "6676616C756531",
    "key2": "6676616C756532"
  },
  "requestedClaims": [
    {
      "type": "identity",
      "source": ["identityCredential","accountCredential"],
      "issuers": ["did:ccd:testnet:idp:0","did:ccd:testnet:idp:1","did:ccd:testnet:idp:2"],
      "statements": [
        {
          "type": "AttributeInRange",
          "attributeTag": "dob",
          "lower": "18000101",
          "upper": "20080106"
        },
        {
          "type": "AttributeInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "GB"
          ]
        },
        {
          "type": "AttributeNotInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "GB"
          ]
        },
        {
          "type": "RevealAttribute",
          "attributeTag": "firstName"
        }
      ]
    }
  ]
}
```

## List of allowed attribute tags

### Individual ID:

| Attribute tag        | Description               |
| -------------------- | ------------------------- |
| `firstName`          | First name                |
| `lastName`           | Last name                 |
| `dob`                | Date of birth             |
| `idDocType`          | Identity document type    |
| `sex`                | Sex                       |
| `countryOfResidence` | Country of residence      |
| `nationality`        | Nationality               |
| `idDocNo`            | Identity document number  |
| `idDocIssuer`        | Issuing authority         |
| `idDocIssuedAt`      | ID valid from             |
| `idDocExpiresAt`     | ID valid to               |
| `nationalIdNo`       | National ID number        |
| `taxIdNo`            | Tax identification number |
	
### Company ID:	

| Attribute tag      | Description                  |
| ------------------ | ---------------------------- |
| `legalName`        | Legal company name           |
| `legalCountry`     | Country of registration      |
| `businessNumber`   | Business registration number |
| `lei`              | LEI code                     |
| `registrationAuth` | Registration authority       |


## List of allowed attribute value format

| Attribute                                                                                    | Value format                                                                                                      |
| -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `firstName`, `lastName`, `legalName`, `businessNumber`, `nationalIdNo`, `taxIdNo`, `idDocNo`, `registrationAuth` | `string`      |
| `dob`,`idDocIssuedAt`, `idDocExpiresAt`                                                                                        | `ISO 8601 (YYYYMMDD)`               |
| `idDocType`                                                                                  | `0 = n/a`, `1 = passport`, `2 = national ID card`, `3 = driving license`, `4 = immigration card`, or `eID string` |
| `sex`                                                                                        | `ISO/IEC 5218`                       |
| `countryOfResidence`, `nationality`, `legalCountry`                                          | `ISO 3166-1 alpha-2`              |
| `idDocIssuer`                                                                                | `ISO 3166-1 alpha-2` or `ISO 3166-2` (if applicable)   |
| `lei`                                                                                        | `ISO17442`         |

## Example curl command

```json
 curl -POST "localhost:8000/verifiable-presentations/create-verification-request" -H "Content-Type: application/json" --data '
{
  "connectionId": "conn_8291yuw",
  "resourceId": "some string for resource",
  "contextString": "optional context string here",
  "publicInfo": {
    "key1": "6676616C756531",
    "key2": "6676616C756532"
  },
  "requestedClaims": [
    {
      "type": "identity",
      "source": ["identityCredential","accountCredential"],
      "issuers": ["did:ccd:testnet:idp:0","did:ccd:testnet:idp:1","did:ccd:testnet:idp:2"],
      "statements": [
        {
          "type": "AttributeInRange",
          "attributeTag": "dob",
          "lower": "18000101",
          "upper": "20080106"
        },
        {
          "type": "AttributeInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "GB"
          ]
        },
        {
          "type": "AttributeNotInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "GB"
          ]
        },
        {
          "type": "RevealAttribute",
          "attributeTag": "firstName"
        }
      ]
    }
  ]
}
' -v
```

Returned Verification Request Sample:

```json
{
    "type": "ConcordiumVerificationRequestV1",
    "context": {
        "type": "ConcordiumUnfilledContextInformationV1",
        "given": [
            {
                "label": "Nonce",
                "context": "9e5c21e162b4f2d7d81b0ca98c13c2d47a4142516ba225b52b3c7d83f5f41a78"
            },
            {
                "label": "ConnectionID",
                "context": "conn_8291yuw"
            },
            {
                "label": "ResourceID",
                "context": "some string for resource"
            },
            {
                "label": "ContextString",
                "context": "optional context string here"
            }
        ],
        "requested": [
            "BlockHash"
        ]
    },
    "subjectClaims": [
        {
            "type": "identity",
            "statements": [
                {
                    "type": "AttributeInRange",
                    "attributeTag": "dob",
                    "lower": "18000101",
                    "upper": "20080106"
                },
                {
                    "type": "AttributeInSet",
                    "attributeTag": "countryOfResidence",
                    "set": [
                        "IE",
                        "IN",
                        "GB",
                        "US"
                    ]
                },
                {
                    "type": "AttributeNotInSet",
                    "attributeTag": "countryOfResidence",
                    "set": [
                        "IE",
                        "IN",
                        "GB",
                        "US"
                    ]
                },
                {
                    "type": "RevealAttribute",
                    "attributeTag": "firstName"
                }
            ],
            "issuers": [
                "did:ccd:testnet:idp:0",
                "did:ccd:testnet:idp:1",
                "did:ccd:testnet:idp:2"
            ],
            "source": [
                "identityCredential",
                "accountCredential"
            ]
        }
    ],
    "transactionRef": "5096a7702fc8e13ffb0bb4c4f9431c2cc674001f9752d2049da562e6703306a7"
}
```