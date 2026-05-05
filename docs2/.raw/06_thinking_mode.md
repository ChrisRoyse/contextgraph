# Thinking Mode

The DeepSeek model supports thinking mode: before outputting the final answer, the model first emits chain-of-thought (CoT) reasoning to improve accuracy.

## Toggle and Effort Control

| | OpenAI Format | Anthropic Format |
|---|---|---|
| Thinking Mode Toggle | `{"thinking": {"type": "enabled/disabled"}}` | `{"thinking": {"type": "enabled/disabled"}}` |
| Thinking Effort | `{"reasoning_effort": "high/max"}` | `{"output_config": {"effort": "high/max"}}` |

Notes:
- Thinking toggle defaults to **enabled**.
- Default effort: `high` for regular requests; `max` automatically for some complex agent requests (Claude Code, OpenCode).
- For compatibility, `low`/`medium` map to `high`; `xhigh` maps to `max`.

When using OpenAI SDK, pass `thinking` via `extra_body`:

```python
response = client.chat.completions.create(
  model="deepseek-v4-pro",
  reasoning_effort="high",
  extra_body={"thinking": {"type": "enabled"}}
)
```

## Input/Output Parameters

Thinking mode does NOT support: `temperature`, `top_p`, `presence_penalty`, `frequency_penalty`. These are silently ignored (no error).

CoT is returned in `reasoning_content`, alongside `content`. Concatenation rules in subsequent turns:
- **No tool call between two user messages**: previous `reasoning_content` is dropped (ignored if passed).
- **Tool call between two user messages**: `reasoning_content` MUST be passed back in all subsequent requests for that turn (else 400 error).

## Sample Code (Multi-turn, no tools)

```python
from openai import OpenAI
client = OpenAI(api_key="<DeepSeek API Key>", base_url="https://api.deepseek.com")

# Turn 1
messages = [{"role": "user", "content": "9.11 and 9.8, which is greater?"}]
response = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=messages,
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}},
)
reasoning_content = response.choices[0].message.reasoning_content
content = response.choices[0].message.content

# Turn 2 — reasoning_content from Turn 1 is ignored by API
messages.append(response.choices[0].message)
messages.append({'role': 'user', 'content': "How many Rs are in 'strawberry'?"})
response = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=messages,
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}},
)
```

## Tool Calls in Thinking Mode

Multiple sub-turns of reasoning + tool calls before final answer. `reasoning_content` MUST be passed back in all subsequent requests during the tool-call sequence.

```python
import os, json
from openai import OpenAI
from datetime import datetime

tools = [
  {"type": "function", "function": {
    "name": "get_date",
    "description": "Get the current date",
    "parameters": {"type": "object", "properties": {}}}},
  {"type": "function", "function": {
    "name": "get_weather",
    "description": "Get weather of a location.",
    "parameters": {"type": "object", "properties": {
      "location": {"type": "string"},
      "date": {"type": "string"}}, "required": ["location","date"]}}},
]

def get_date_mock(): return datetime.now().strftime("%Y-%m-%d")
def get_weather_mock(location, date): return "Cloudy 7~13°C"
TOOL_CALL_MAP = {"get_date": get_date_mock, "get_weather": get_weather_mock}

def run_turn(turn, messages):
    sub = 1
    while True:
        resp = client.chat.completions.create(
            model='deepseek-v4-pro', messages=messages, tools=tools,
            reasoning_effort="high",
            extra_body={"thinking": {"type": "enabled"}})
        messages.append(resp.choices[0].message)  # includes content + reasoning_content + tool_calls
        tool_calls = resp.choices[0].message.tool_calls
        if tool_calls is None: break
        for t in tool_calls:
            r = TOOL_CALL_MAP[t.function.name](**json.loads(t.function.arguments))
            messages.append({"role": "tool", "tool_call_id": t.id, "content": r})
        sub += 1

client = OpenAI(api_key=os.environ['DEEPSEEK_API_KEY'], base_url="https://api.deepseek.com")
messages = [{"role":"user","content":"How's the weather in Hangzhou Tomorrow"}]
run_turn(1, messages)
messages.append({"role":"user","content":"How's the weather in Guangzhou Tomorrow"})
run_turn(2, messages)
```

`messages.append(response.choices[0].message)` is equivalent to:

```python
messages.append({
  'role': 'assistant',
  'content': response.choices[0].message.content,
  'reasoning_content': response.choices[0].message.reasoning_content,
  'tool_calls': response.choices[0].message.tool_calls,
})
```
