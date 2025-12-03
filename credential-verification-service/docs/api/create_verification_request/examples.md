# Example Payloads

## Request Example

```
{
  "connectionId": "conn_8291yuw",
  "resourceId": "some string for resource",
  "contextString": "context string here",
  "publicInfo": {
    "key": "value",
    "key2": "value"
  },
  claims: [
    {
      "identityCredentialType": "Identity",
      "trustedIdps": [1,2,3],
      "issuers": [2,3],
      "provingStatements": [
        {
          "type": "ATTRIBUTE_IN_RANGE",
          "tag": "age",
          "lowerBound": 18,
          "upperBound": 100
        },
        {
          "type": "ATTRIBUTE_IN_SET",
          "tag": "nationality",
          "set": [
            IE,
            IN,
            US,
            UK
          ],
        },
        {
          "type": "ATTRIBUTE_IN_SET",
          "tag": "residence",
          "set": [
            IE,
            IN,
            US,
            UK
          ],
        }
      ]
    }
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