# JSON Output

Set `response_format` to ensure valid JSON.

Requirements:
1. Set `response_format={'type': 'json_object'}`.
2. Include the word "json" in system/user prompt + provide an example of desired JSON format.
3. Set `max_tokens` reasonably to prevent mid-JSON truncation.

Note: API may occasionally return empty content; mitigate by tweaking the prompt.

```python
import json
from openai import OpenAI

client = OpenAI(api_key="<your api key>", base_url="https://api.deepseek.com")

system_prompt = """
The user will provide some exam text. Please parse the "question" and "answer" and output them in JSON format.

EXAMPLE INPUT:
Which is the highest mountain in the world? Mount Everest.

EXAMPLE JSON OUTPUT:
{
    "question": "Which is the highest mountain in the world?",
    "answer": "Mount Everest"
}
"""

user_prompt = "Which is the longest river in the world? The Nile River."

messages = [{"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}]

response = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=messages,
    response_format={'type': 'json_object'}
)
print(json.loads(response.choices[0].message.content))
```

Output:
```json
{"question": "Which is the longest river in the world?", "answer": "The Nile River"}
```
