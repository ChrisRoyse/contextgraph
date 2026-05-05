# Models & Pricing

The prices listed below are in units of per 1M tokens. A token, the smallest unit of text that the model recognizes, can be a word, a number, or even a punctuation mark. We will bill based on the total number of input and output tokens by the model.

## Model Details

| | deepseek-v4-flash | deepseek-v4-pro |
|---|---|---|
| BASE URL (OpenAI) | https://api.deepseek.com | https://api.deepseek.com |
| BASE URL (Anthropic) | https://api.deepseek.com/anthropic | https://api.deepseek.com/anthropic |
| MODEL VERSION | DeepSeek-V4-Flash | DeepSeek-V4-Pro |
| THINKING MODE | Both non-thinking and thinking (default) | Both non-thinking and thinking (default) |
| CONTEXT LENGTH | 1M | 1M |
| MAX OUTPUT | 384K | 384K |
| Json Output | ✓ | ✓ |
| Tool Calls | ✓ | ✓ |
| Chat Prefix Completion (Beta) | ✓ | ✓ |
| FIM Completion (Beta) | Non-thinking only | Non-thinking only |
| 1M INPUT TOKENS (CACHE HIT) | $0.0028 | $0.003625 (75% off) / $0.0145 |
| 1M INPUT TOKENS (CACHE MISS) | $0.14 | $0.435 (75% off) / $1.74 |
| 1M OUTPUT TOKENS | $0.28 | $0.87 (75% off) / $3.48 |

Notes:
- The model names deepseek-chat and deepseek-reasoner will be deprecated; they map to non-thinking and thinking mode of deepseek-v4-flash, respectively.
- Cache-hit input price reduced to 1/10 of launch price effective 2026/4/26 12:15 UTC.
- deepseek-v4-pro currently offered at 75% discount, extended to 2026/05/31 15:59 UTC.

## Deduction Rules

expense = number of tokens × price. Charges deducted from topped-up balance or granted balance (granted balance preferred when both available).
