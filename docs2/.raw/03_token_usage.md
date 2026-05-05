# Token & Token Usage

Tokens are the basic units used by models to represent natural language text, and also the units we use for billing. They can be intuitively understood as 'characters' or 'words'. Typically, a Chinese word, an English word, a number, or a symbol is counted as a token.

Approximate conversion ratios (vary by model):
- 1 English character ≈ 0.3 token
- 1 Chinese character ≈ 0.6 token

Actual token count is based on the model's `usage` field returned in API responses.

## Calculate token usage offline

DeepSeek provides a `deepseek_tokenizer.zip` demo package for offline token counting (downloadable from the docs page).
