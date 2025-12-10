# Example Payloads

## Request Example

```json
{
  "nonce": "1234....",
  "connectionId": "conn_8291yuw",
  "resourceId": "some string for resource",
  "contextString": "context string here",
  "publicInfo": {
    "key1": "6676616C756531",
    "key2": "6676616C756532"
  },
  "subjectClaims": [
    {
      "type": "Identity",
      "source": ["Identity"],
      "issuers": [2,3],
      "claims": [
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
            "IE",
            "IN",
            "US",
            "UK"
          ]
        },
        {
          "type": "ATTRIBUTE_IN_SET",
          "tag": "residence",
          "set": [
            "IE",
            "IN",
            "US",
            "UK"
          ]
        }
      ]
    }
  ]
}
```

