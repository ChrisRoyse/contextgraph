# Chat Prefix Completion (Beta)

Provide an assistant prefix message; the model completes the rest.

Requirements:
- Last message role MUST be `assistant`.
- Set `prefix: True` on the last message.
- Use `base_url="https://api.deepseek.com/beta"` to enable Beta features.

```python
from openai import OpenAI

client = OpenAI(api_key="<your api key>", base_url="https://api.deepseek.com/beta")

messages = [
    {"role": "user", "content": "Please write quick sort code"},
    {"role": "assistant", "content": "```python\n", "prefix": True}
]
response = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=messages,
    stop=["```"],
)
print(response.choices[0].message.content)
```
