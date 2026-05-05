# Tool Calls

Allows the model to call external tools.

## Non-thinking Mode Sample

```python
from openai import OpenAI

def send_messages(messages):
    return client.chat.completions.create(
        model="deepseek-v4-pro", messages=messages, tools=tools
    ).choices[0].message

client = OpenAI(api_key="<your api key>", base_url="https://api.deepseek.com")

tools = [{
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "Get weather of a location, the user should supply a location first.",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {"type": "string", "description": "The city and state, e.g. San Francisco, CA"}
            },
            "required": ["location"]
        }
    }
}]

messages = [{"role": "user", "content": "How's the weather in Hangzhou, Zhejiang?"}]
message = send_messages(messages)
tool = message.tool_calls[0]
messages.append(message)
messages.append({"role": "tool", "tool_call_id": tool.id, "content": "24℃"})
message = send_messages(messages)
print(message.content)
```

Flow:
1. User asks weather.
2. Model returns `get_weather({location: 'Hangzhou'})`.
3. User executes function, sends result back.
4. Model returns natural-language summary.

The user provides actual function execution; the model only emits the call.

## Thinking Mode

From DeepSeek-V3.2, tool use works in thinking mode. See Thinking Mode for full requirements (must pass back `reasoning_content` for tool-call sequences).

## strict Mode (Beta)

Strict mode forces the model to comply with the JSON schema.

Setup:
- Use `base_url="https://api.deepseek.com/beta"`.
- Set `strict: true` on each function.
- The server validates the JSON Schema and errors if unsupported.

```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "strict": true,
    "description": "Get weather of a location, the user should supply a location first.",
    "parameters": {
      "type": "object",
      "properties": {
        "location": {"type": "string", "description": "The city and state, e.g. San Francisco, CA"}
      },
      "required": ["location"],
      "additionalProperties": false
    }
  }
}
```

### Supported JSON Schema Types in strict Mode

`object`, `string`, `number`, `integer`, `boolean`, `array`, `enum`, `anyOf`

#### object
- All properties must be `required`.
- `additionalProperties` must be `false`.

```json
{
  "type": "object",
  "properties": {"name": {"type": "string"}, "age": {"type": "integer"}},
  "required": ["name", "age"],
  "additionalProperties": false
}
```

#### string
- Supported: `pattern`, `format` (`email`, `hostname`, `ipv4`, `ipv6`, `uuid`).
- Unsupported: `minLength`, `maxLength`.

#### number / integer
Supported: `const`, `default`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`.

#### array
Unsupported: `minItems`, `maxItems`.

#### enum
```json
{"order_status": {"type": "string", "enum": ["pending", "processing", "shipped", "cancelled"]}}
```

#### anyOf
```json
{"account": {"anyOf": [
  {"type": "string", "format": "email"},
  {"type": "string", "pattern": "^\\d{11}$"}
]}}
```

### $ref and $def

Define reusable modules with `$def`, reference with `$ref`. Also enables recursive structures.

```json
{
  "type": "object",
  "properties": {
    "report_date": {"type": "string"},
    "authors": {"type": "array", "items": {"$ref": "#/$def/author"}}
  },
  "required": ["report_date", "authors"],
  "additionalProperties": false,
  "$def": {
    "author": {
      "type": "object",
      "properties": {
        "name": {"type": "string"},
        "institution": {"type": "string"},
        "email": {"type": "string", "format": "email"}
      },
      "required": ["name", "institution", "email"],
      "additionalProperties": false
    }
  }
}
```
