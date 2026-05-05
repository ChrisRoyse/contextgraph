# FIM Completion (Beta)

Fill-In-the-Middle: provide a prefix and (optional) suffix; model completes the middle. Common for content/code completion.

Notes:
- Max tokens of FIM completion: **4K**.
- Use `base_url="https://api.deepseek.com/beta"`.
- Available only in **non-thinking mode**.
- Uses the legacy `completions` (not `chat.completions`) endpoint.

```python
from openai import OpenAI

client = OpenAI(api_key="<your api key>", base_url="https://api.deepseek.com/beta")

response = client.completions.create(
    model="deepseek-v4-pro",
    prompt="def fib(a):",
    suffix="    return fib(a-1) + fib(a-2)",
    max_tokens=128
)
print(response.choices[0].text)
```

## Integration with Continue (VSCode plugin)

Continue supports code completion; configure with DeepSeek as the FIM provider.
