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
          "lower": "1800010",
          "upper": "20080106"
        },
        {
          "type": "AttributeInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "UK"
          ]
        },
        {
          "type": "AttributeNotInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "UK"
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
          "lower": "1800010",
          "upper": "20080106"
        },
        {
          "type": "AttributeInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "UK"
          ]
        },
        {
          "type": "AttributeNotInSet",
          "attributeTag": "countryOfResidence",
          "set": [
            "IE",
            "IN",
            "US",
            "UK"
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
                    "lower": "1800010",
                    "upper": "20080106"
                },
                {
                    "type": "AttributeInSet",
                    "attributeTag": "countryOfResidence",
                    "set": [
                        "IE",
                        "IN",
                        "UK",
                        "US"
                    ]
                },
                {
                    "type": "AttributeNotInSet",
                    "attributeTag": "countryOfResidence",
                    "set": [
                        "IE",
                        "IN",
                        "UK",
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