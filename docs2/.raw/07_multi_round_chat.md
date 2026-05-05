# Multi-round Conversation

The DeepSeek `/chat/completions` API is **stateless** — the server does not record context. Clients must concatenate the full conversation history on every request.

```python
from openai import OpenAI
client = OpenAI(api_key="<DeepSeek API Key>", base_url="https://api.deepseek.com")

# Round 1
messages = [{"role": "user", "content": "What's the highest mountain in the world?"}]
response = client.chat.completions.create(model="deepseek-v4-pro", messages=messages)
messages.append(response.choices[0].message)

# Round 2
messages.append({"role": "user", "content": "What is the second?"})
response = client.chat.completions.create(model="deepseek-v4-pro", messages=messages)
messages.append(response.choices[0].message)
```

Round 2 messages payload:

```json
[
  {"role": "user", "content": "What's the highest mountain in the world?"},
  {"role": "assistant", "content": "The highest mountain in the world is Mount Everest."},
  {"role": "user", "content": "What is the second?"}
]
```
