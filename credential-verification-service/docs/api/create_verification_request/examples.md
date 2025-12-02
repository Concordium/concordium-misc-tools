# Example Payloads

## Request Example

```
{
  "connectionId": "conn_8291yuw",
  "description": "Age verification check for Alcohol Purchase",
  "claimType": "Identity",
  "trustedIDPs": [0, 1, 2, 3, 4],
  "verificationChecks": [
    { "type": "AT_LEAST_AGE_X", "target": 18 },
    { "type": "NATIONALITY_IN_REGION", "target": "EU" }
  ]
}
```


Verification Request Internal Sample:

```
{
  "type": "ConcordiumVerificationRequestV1",
  "context": {
    "type": "ConcordiumUnfilledContextInformationV1",
    "given": [
      {
        "label": "Nonce",
        "context": "0101010101010101010101010101010101010101010101010101010101010101"
      },
      {
        "label": "ConnectionID",
        "context": "testconnection"
      },
      {
        "label": "ContextString",
        "context": "testcontext"
      }
    ],
    "requested": [
      "BlockHash",
      "ResourceID"
    ]
  },
  "subjectClaims": [
    {
      "type": "identity",
      "statements": [
        {
          "type": "AttributeInRange",
          "attributeTag": "dob",
          "lower": 80,
          "upper": 1237
        },
        {
          "type": "AttributeInSet",
          "attributeTag": "sex",
          "set": [
            "aa",
            "ff",
            "zz"
          ]
        },
        {
          "type": "AttributeNotInSet",
          "attributeTag": "lastName",
          "set": [
            "aa",
            "ff",
            "zz"
          ]
        },
        {
          "type": "AttributeInRange",
          "attributeTag": "countryOfResidence",
          "lower": {
            "type": "date-time",
            "timestamp": "2023-08-27T23:12:15Z"
          },
          "upper": {
            "type": "date-time",
            "timestamp": "2023-08-29T23:12:15Z"
          }
        },
        {
          "type": "RevealAttribute",
          "attributeTag": "nationality"
        }
      ],
      "issuers": [
        "did:ccd:testnet:idp:0",
        "did:ccd:testnet:idp:1",
        "did:ccd:testnet:idp:17"
      ],
      "source": [
        "identityCredential",
        "accountCredential"
      ]
    }
  ],
  "transactionRef": "0000000000000000000000000000000000000000000000000000000000000000"
}
```