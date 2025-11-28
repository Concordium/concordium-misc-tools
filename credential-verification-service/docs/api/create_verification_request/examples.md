```markdown
# Example Payloads

## Request Example

```json
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