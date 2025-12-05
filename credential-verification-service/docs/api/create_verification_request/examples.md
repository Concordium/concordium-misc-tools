# Example Payloads

## Request Example

```
{
    "nonce": "0000000000000000000000000000000000000000000000000000000000000000",
    "connectionId": "MyWalletConnectTopic",
    "contextString": "MyGreateApp",
    "rescourceId": "MyGreateWebsite",
    "subjectClaims": [
        {
            "type": "identity",
            "statements": [
                {
                    "type": "AttributeInRange",
                    "attributeTag": "registrationAuth",
                    "lower": 80,
                    "upper": 1237
                }
            ],
            "issuers": [
                "did:ccd:testnet:idp:0"
            ],
            "source": [
                "identityCredential"
            ]
        }
    ],
   "publicInfo": {
        "cborHex": "a26161016c616e6f746865724669656c646374776f"
    }
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