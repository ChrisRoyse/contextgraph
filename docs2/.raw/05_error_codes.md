# Error Codes

| Code | Description |
|---|---|
| 400 - Invalid Format | Invalid request body format. Modify body per error hints. |
| 401 - Authentication Fails | Wrong API key. Create/check API key. |
| 402 - Insufficient Balance | Out of balance. Top up. |
| 422 - Invalid Parameters | Request contains invalid parameters. Modify per error hints. |
| 429 - Rate Limit Reached | Sending too quickly. Pace requests; consider falling back to other providers temporarily. |
| 500 - Server Error | Server issue. Retry after wait; contact support if persists. |
| 503 - Server Overloaded | High traffic. Retry after wait. |
