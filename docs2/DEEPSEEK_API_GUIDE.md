# DeepSeek API: Complete Guide for AI Agents

> **Comprehensive reference for everything an AI agent needs to know to code with and use the DeepSeek API.**
> Source: <https://api-docs.deepseek.com/> (extracted 2026-05-05)
> Version: API 1.0.0; Models DeepSeek-V4-Pro / DeepSeek-V4-Flash

---

## Table of Contents

1. [TL;DR Quick Start](#1-tldr-quick-start)
2. [Endpoints, Auth, Headers](#2-endpoints-auth-headers)
3. [Models, Capabilities, Pricing](#3-models-capabilities-pricing)
4. [Tokens & Token Usage](#4-tokens--token-usage)
5. [Rate Limits & Connection Behavior](#5-rate-limits--connection-behavior)
6. [Error Codes](#6-error-codes)
7. [Chat Completions API (full reference)](#7-chat-completions-api-full-reference)
8. [FIM Completions API (Beta)](#8-fim-completions-api-beta)
9. [Models & User Balance Endpoints](#9-models--user-balance-endpoints)
10. [Thinking Mode (Chain-of-Thought)](#10-thinking-mode-chain-of-thought)
11. [Multi-round Conversation](#11-multi-round-conversation)
12. [Chat Prefix Completion (Beta)](#12-chat-prefix-completion-beta)
13. [JSON Output](#13-json-output)
14. [Tool Calls (Function Calling)](#14-tool-calls-function-calling)
15. [Strict-mode Tool Calls (Beta)](#15-strict-mode-tool-calls-beta)
16. [Context (KV) Caching](#16-context-kv-caching)
17. [Anthropic API Compatibility](#17-anthropic-api-compatibility)
18. [Agent Tooling Integrations](#18-agent-tooling-integrations)
19. [Streaming](#19-streaming)
20. [SDK Usage Patterns](#20-sdk-usage-patterns)
21. [FAQ Highlights](#21-faq-highlights)
22. [Change Log Highlights](#22-change-log-highlights)
23. [Practical Cookbook](#23-practical-cookbook)

---

## 1. TL;DR Quick Start

```bash
export DEEPSEEK_API_KEY=sk-...

curl https://api.deepseek.com/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ${DEEPSEEK_API_KEY}" \
  -d '{
    "model": "deepseek-v4-pro",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ],
    "thinking": {"type": "enabled"},
    "reasoning_effort": "high",
    "stream": false
  }'
```

```python
# pip install openai
from openai import OpenAI

client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com")

resp = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[{"role": "user", "content": "Hello!"}],
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}},
)
print(resp.choices[0].message.content)
```

```javascript
// npm i openai
import OpenAI from "openai";
const client = new OpenAI({ apiKey: process.env.DEEPSEEK_API_KEY, baseURL: "https://api.deepseek.com" });

const resp = await client.chat.completions.create({
  model: "deepseek-v4-pro",
  messages: [{ role: "user", content: "Hello!" }],
  reasoning_effort: "high",
  // OpenAI JS SDK passes extra fields through
  thinking: { type: "enabled" },
});
console.log(resp.choices[0].message.content);
```

**Key facts to memorize before coding:**

- The DeepSeek API is **OpenAI-compatible** AND **Anthropic-compatible**.
- Two production models: `deepseek-v4-pro` (premium reasoning) and `deepseek-v4-flash` (cost-efficient).
- Both support a single hybrid model that toggles between **non-thinking** and **thinking** modes.
- 1M-token context window; max 384K output tokens.
- The legacy aliases `deepseek-chat` (= flash non-thinking) and `deepseek-reasoner` (= flash thinking) are deprecated 2026-07-24.
- Thinking mode is **enabled by default**.
- Thinking mode does NOT support `temperature`, `top_p`, `presence_penalty`, `frequency_penalty` (silently ignored).
- KV caching is automatic — no code changes needed; use `prompt_cache_hit_tokens` in usage to confirm.
- The API is **stateless** — clients must send the full conversation history every turn.

---

## 2. Endpoints, Auth, Headers

### Base URLs

| Purpose | URL |
|---|---|
| OpenAI-format chat completions | `https://api.deepseek.com` |
| OpenAI-format with `/v1` (works) | `https://api.deepseek.com/v1` |
| Anthropic-format messages | `https://api.deepseek.com/anthropic` |
| Beta features (FIM, prefix, strict tools) | `https://api.deepseek.com/beta` |

### Authentication

HTTP Bearer auth: `Authorization: Bearer ${DEEPSEEK_API_KEY}`.
For Anthropic format, the SDK uses `x-api-key` (fully supported).

API keys must be created at the DeepSeek Platform (`platform.deepseek.com`). Keys begin with `sk-`.

### Endpoint Catalog

| Method | Path | Purpose |
|---|---|---|
| POST | `/chat/completions` | Main chat endpoint (OpenAI format) |
| POST | `/completions` | FIM completion (Beta only — `/beta` base) |
| GET | `/models` | List available models |
| GET | `/user/balance` | Get current user balance |
| POST | `/anthropic/v1/messages` | Anthropic-style messages |

---

## 3. Models, Capabilities, Pricing

### Model Catalog

| Model ID | Family | Default Mode | Notes |
|---|---|---|---|
| `deepseek-v4-pro` | DeepSeek-V4-Pro | thinking | Top-tier reasoning, 75% promo discount until 2026-05-31 15:59 UTC |
| `deepseek-v4-flash` | DeepSeek-V4-Flash | thinking | Cost-efficient |
| `deepseek-chat` (deprecated 2026-07-24) | alias | non-thinking | Maps to flash non-thinking |
| `deepseek-reasoner` (deprecated 2026-07-24) | alias | thinking | Maps to flash thinking |
| `deepseek-v4-pro[1m]` | Pro w/ explicit 1M ctx flag | thinking | Used in some agent integrations like Claude Code |

### Capability Matrix

| Feature | v4-flash | v4-pro |
|---|---|---|
| Context length | 1M | 1M |
| Max output | 384K | 384K |
| JSON output (`response_format`) | ✓ | ✓ |
| Tool calls | ✓ | ✓ |
| Strict-mode tool calls (Beta) | ✓ | ✓ |
| Chat prefix completion (Beta) | ✓ | ✓ |
| FIM completion (Beta) | non-thinking only | non-thinking only |
| Thinking mode | ✓ | ✓ |
| Vision/image input | ✗ | ✗ |
| Documents/search results (Anthropic content blocks) | ✗ | ✗ |

### Pricing (per 1M tokens, USD)

| | v4-flash | v4-pro (75% off) | v4-pro (full) |
|---|---|---|---|
| Input (cache hit) | $0.0028 | $0.003625 | $0.0145 |
| Input (cache miss) | $0.14 | $0.435 | $1.74 |
| Output | $0.28 | $0.87 | $3.48 |

- Cache-hit input price was reduced to 1/10 of launch price effective 2026-04-26 12:15 UTC.
- expense = tokens × price; charged from topped-up balance OR granted balance (granted preferred when both exist).
- Granted balance can expire; topped-up balance does not.

---

## 4. Tokens & Token Usage

A token = the smallest unit the model recognizes (a word, number, or symbol). Approx ratios (vary by model):

- 1 English character ≈ 0.3 token
- 1 Chinese character ≈ 0.6 token

The authoritative count is the `usage` field of every response. DeepSeek publishes a `deepseek_tokenizer.zip` for offline counting (linked in their token-usage docs).

`usage` fields:

```json
{
  "completion_tokens": 0,
  "prompt_tokens": 0,
  "prompt_cache_hit_tokens": 0,
  "prompt_cache_miss_tokens": 0,
  "total_tokens": 0,
  "completion_tokens_details": { "reasoning_tokens": 0 }
}
```

Note: `prompt_tokens == prompt_cache_hit_tokens + prompt_cache_miss_tokens`. `reasoning_tokens` reports thinking-mode CoT token spend (still billed at output rate).

---

## 5. Rate Limits & Connection Behavior

- DeepSeek **dynamically** rate-limits based on server load and short-term account history. **No tiered plans, no per-account increases.**
- Hitting the limit returns HTTP **429**.
- While the request waits to be scheduled, the API keeps the TCP connection alive:
  - Non-streaming: continuously emits **empty lines**.
  - Streaming: continuously emits **SSE keep-alive comments** (`: keep-alive`).
  These do not affect JSON parsing but raw HTTP parsers must handle them.
- If a request has **not started inference after 10 minutes**, the server closes the connection.

When you hit 429 frequently, the docs recommend pacing requests OR temporarily falling back to another LLM provider.

---

## 6. Error Codes

| HTTP | Meaning | What to do |
|---|---|---|
| 400 | Invalid request body format | Fix per error message |
| 401 | Authentication failure | Check API key (must start `sk-`) |
| 402 | Insufficient balance | Top up |
| 422 | Invalid parameters | Fix per error hints |
| 429 | Rate-limit / concurrency cap reached | Back off; retry; consider provider fallback |
| 500 | Server error | Retry after wait; contact support if persistent |
| 503 | Server overloaded | Retry after wait |

A common thinking-mode failure mode: **400 "The reasoning_content in the thinking mode must be passed back to the API."** Cause: tool-call sequence omitted `reasoning_content` from the assistant message in the next request. See section 14.

---

## 7. Chat Completions API (full reference)

`POST /chat/completions`

### Request Body

| Field | Type | Required | Description |
|---|---|---|---|
| `messages` | array (≥1) | yes | Conversation messages (see message types below) |
| `model` | string | yes | `deepseek-v4-flash` or `deepseek-v4-pro` |
| `thinking` | `{type: "enabled"\|"disabled"}` | no | Default `enabled`. Toggles thinking mode. |
| `reasoning_effort` | `"high"` \| `"max"` | no | Default `high`; auto-`max` for some agents (Claude Code, OpenCode). `low`/`medium`→`high`; `xhigh`→`max`. |
| `max_tokens` | integer | no | Upper bound on generated tokens (incl. reasoning). Bounded by model context. |
| `response_format` | `{type: "text"\|"json_object"}` | no | Default `text`. See JSON Output. |
| `stop` | string \| string[] (≤16) | no | Stop sequences. |
| `stream` | bool | no | Enable SSE streaming. |
| `stream_options` | `{include_usage: bool}` | no | Send usage chunk before `[DONE]`. |
| `temperature` | number ≤2, default 1 | no | Higher = more random. (NOT supported in thinking mode.) |
| `top_p` | number ≤1, default 1 | no | Nucleus sampling. (NOT supported in thinking mode.) |
| `tools` | array (≤128) | no | Function tools. |
| `tool_choice` | `"none"\|"auto"\|"required"` or `{type:"function",function:{name}}` | no | Default `auto` if tools present, else `none`. |
| `logprobs` | bool | no | Return per-token logprobs. |
| `top_logprobs` | integer ≤20 | no | Requires `logprobs:true`. |
| `user_id` | string | no | Custom user id (≤512 chars, `[a-zA-Z0-9\-_]`). Drives KV-cache isolation per user; do NOT include PII. |
| `frequency_penalty` | (deprecated) | — | Ignored. |
| `presence_penalty` | (deprecated) | — | Ignored. |

### Message Types

```json
{ "role": "system",    "content": "...",        "name": "..." }
{ "role": "user",      "content": "...",        "name": "..." }
{ "role": "assistant", "content": "...",
  "prefix": true,                                  // Beta — see §12
  "reasoning_content": "...",                      // Beta — for prefix in thinking mode
  "tool_calls": [...] }
{ "role": "tool",      "content": "...",        "tool_call_id": "..." }
```

### Response (non-streaming)

```json
{
  "id": "...",
  "object": "chat.completion",
  "created": 1705651092,
  "model": "deepseek-v4-pro",
  "system_fingerprint": "...",
  "choices": [
    {
      "index": 0,
      "finish_reason": "stop",
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?",
        "reasoning_content": "...",
        "tool_calls": [
          {
            "id": "...",
            "type": "function",
            "function": { "name": "...", "arguments": "<json>" }
          }
        ]
      },
      "logprobs": null
    }
  ],
  "usage": { ... }
}
```

`finish_reason` ∈ `stop | length | content_filter | tool_calls | insufficient_system_resource`.

### Streaming Response

`Content-Type: text/event-stream`. Each event is a `data: {chunk}` line; terminated with `data: [DONE]`. Each chunk:

```json
{
  "id": "...",
  "object": "chat.completion.chunk",
  "created": 1718345013,
  "model": "deepseek-v4-pro",
  "system_fingerprint": "fp_...",
  "choices": [
    {
      "index": 0,
      "delta": { "role": "assistant", "content": "Hello", "reasoning_content": null },
      "finish_reason": null,
      "logprobs": null
    }
  ],
  "usage": null
}
```

If `stream_options.include_usage:true`, an extra final chunk carries the full `usage` object before `[DONE]`.

---

## 8. FIM Completions API (Beta)

`POST /completions` — only at `https://api.deepseek.com/beta`.

| Field | Type | Required | Description |
|---|---|---|---|
| `model` | string | yes | Currently `deepseek-v4-pro` (`deepseek-v4-flash` also accepted). |
| `prompt` | string | yes | Text before the gap. |
| `suffix` | string | no | Text after the gap. |
| `max_tokens` | int | no | **Capped at 4K for FIM.** |
| `echo` | bool | no | Echo prompt with completion. |
| `logprobs` | int ≤20 | no | Return logprobs for top-N tokens. |
| `stop` | string \| string[] (≤16) | no | Stop sequences. |
| `stream` | bool | no | Streaming. |
| `temperature` | num ≤2 (1) | no | Sampling temp. |
| `top_p` | num ≤1 (1) | no | Nucleus. |
| `frequency_penalty` / `presence_penalty` | deprecated | — | Ignored. |

**Constraints**:
- Available in **non-thinking mode only**.
- Must use Beta base URL.

```python
from openai import OpenAI
client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com/beta")
resp = client.completions.create(
    model="deepseek-v4-pro",
    prompt="def fib(a):",
    suffix="    return fib(a-1) + fib(a-2)",
    max_tokens=128
)
print(resp.choices[0].text)
```

Response object type: `text_completion`.

---

## 9. Models & User Balance Endpoints

### `GET /models`

```json
{
  "object": "list",
  "data": [
    { "id": "deepseek-v4-flash", "object": "model", "owned_by": "deepseek" },
    { "id": "deepseek-v4-pro",   "object": "model", "owned_by": "deepseek" }
  ]
}
```

### `GET /user/balance`

```json
{
  "is_available": true,
  "balance_infos": [
    {
      "currency": "CNY",                  // or "USD"
      "total_balance":   "110.00",
      "granted_balance": "10.00",
      "topped_up_balance": "100.00"
    }
  ]
}
```

`is_available` indicates whether balance is sufficient for API calls.

---

## 10. Thinking Mode (Chain-of-Thought)

DeepSeek-V4 emits a chain-of-thought *before* the final answer to improve correctness.

### Toggle / Effort

| | OpenAI format | Anthropic format |
|---|---|---|
| Toggle | `{"thinking": {"type": "enabled"\|"disabled"}}` (default enabled) | same |
| Effort | `{"reasoning_effort": "high"\|"max"}` | `{"output_config": {"effort": "high"\|"max"}}` |

- Default effort: `high` (regular requests). Auto-`max` for some agent integrations (Claude Code, OpenCode).
- Compatibility mappings: `low`/`medium`→`high`; `xhigh`→`max`.
- **Unsupported in thinking mode (silently ignored)**: `temperature`, `top_p`, `presence_penalty`, `frequency_penalty`.

### OpenAI SDK pattern

```python
resp = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[...],
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}},  # MUST go in extra_body for OpenAI SDK
)
print(resp.choices[0].message.reasoning_content)  # CoT
print(resp.choices[0].message.content)            # final answer
```

### CoT Concatenation Rules

`reasoning_content` returns at the same level as `content`. Whether to pass it back depends on what happened that turn:

| Scenario | Pass `reasoning_content` back? |
|---|---|
| Two user messages with NO tool call between | **No** (server ignores it anyway) |
| Tool calls in the turn | **Yes — required.** Missing it ⇒ 400 |

Practical rule: just append the entire `response.choices[0].message` to your messages array — that includes `content`, `reasoning_content`, `tool_calls`. The server discards `reasoning_content` when not needed.

### Multi-turn thinking sample (no tools)

```python
messages = [{"role": "user", "content": "9.11 and 9.8, which is greater?"}]
resp = client.chat.completions.create(
    model="deepseek-v4-pro", messages=messages,
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}})
messages.append(resp.choices[0].message)
messages.append({"role": "user", "content": "How many Rs in 'strawberry'?"})
resp = client.chat.completions.create(
    model="deepseek-v4-pro", messages=messages,
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}})
```

---

## 11. Multi-round Conversation

The API is **stateless**. Concatenate all prior turns yourself.

```python
# Round 1
messages = [{"role": "user", "content": "What's the highest mountain in the world?"}]
resp = client.chat.completions.create(model="deepseek-v4-pro", messages=messages)
messages.append(resp.choices[0].message)

# Round 2
messages.append({"role": "user", "content": "What is the second?"})
resp = client.chat.completions.create(model="deepseek-v4-pro", messages=messages)
```

Round-2 payload sent:

```json
[
  {"role":"user","content":"What's the highest mountain in the world?"},
  {"role":"assistant","content":"The highest mountain in the world is Mount Everest."},
  {"role":"user","content":"What is the second?"}
]
```

---

## 12. Chat Prefix Completion (Beta)

Force the assistant to *continue* from a given prefix you specify.

Requirements:
- Use Beta base: `https://api.deepseek.com/beta`.
- Last message role MUST be `assistant`.
- Last message MUST set `"prefix": true`.
- Optionally seed CoT for thinking mode via `reasoning_content` on the last assistant message (also requires `prefix:true`).

```python
client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com/beta")
messages = [
    {"role": "user",      "content": "Please write quick sort code"},
    {"role": "assistant", "content": "```python\n", "prefix": True}
]
resp = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=messages,
    stop=["```"]
)
print(resp.choices[0].message.content)
```

Patterns this enables: forcing code blocks, forcing JSON, forcing structured outputs, resuming an interrupted answer.

---

## 13. JSON Output

Set `response_format={"type":"json_object"}`.

Required practices:
1. Include the literal word **"json"** somewhere in the system or user prompt.
2. Provide an example JSON shape in the prompt.
3. Set `max_tokens` high enough that JSON cannot be truncated mid-string (truncation is the #1 cause of unparseable output — `finish_reason:"length"` is the signal).

> Note: occasionally an API call may return empty content; mitigation is prompt tweaking. The model's tokens may also stream whitespace forever if you forgot to instruct it to produce JSON — always include the example.

```python
system = """
The user will provide some exam text. Please parse the "question" and "answer"
and output them in JSON format.

EXAMPLE INPUT: Which is the highest mountain in the world? Mount Everest.
EXAMPLE JSON OUTPUT:
{ "question": "Which is the highest mountain in the world?", "answer": "Mount Everest" }
"""
resp = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[{"role":"system","content":system},
              {"role":"user","content":"Which is the longest river in the world? The Nile River."}],
    response_format={"type": "json_object"}
)
import json; print(json.loads(resp.choices[0].message.content))
```

For *guaranteed schema conformance*, prefer **strict-mode tool calls** (§15).

---

## 14. Tool Calls (Function Calling)

The model emits a `tool_calls` array; you execute the actual functions and feed results back via `role:"tool"` messages with matching `tool_call_id`.

```python
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

def send(messages):
    return client.chat.completions.create(
        model="deepseek-v4-pro", messages=messages, tools=tools
    ).choices[0].message

msg = send(messages)
messages.append(msg)                                        # assistant turn (with tool_calls)
tc  = msg.tool_calls[0]
messages.append({"role":"tool", "tool_call_id": tc.id, "content": "24℃"})
print(send(messages).content)
```

### Tool calls in **thinking mode**

Available since DeepSeek-V3.2. Multiple sub-turns of (reason → call → reason → call → answer) per user turn.

**Critical rule**: every assistant message in a tool-call sequence must be passed back **including** `reasoning_content`. Missing it triggers:

> 400: The reasoning_content in the thinking mode must be passed back to the API.

Easiest way: just `messages.append(response.choices[0].message)` instead of reconstructing dicts. That object carries `content`, `reasoning_content`, and `tool_calls`.

```python
def run_turn(turn, messages):
    while True:
        resp = client.chat.completions.create(
            model='deepseek-v4-pro', messages=messages, tools=tools,
            reasoning_effort="high",
            extra_body={"thinking": {"type": "enabled"}})
        messages.append(resp.choices[0].message)  # carries reasoning_content
        tc = resp.choices[0].message.tool_calls
        if tc is None: break
        for t in tc:
            result = TOOL_MAP[t.function.name](**json.loads(t.function.arguments))
            messages.append({"role":"tool","tool_call_id":t.id,"content":result})
```

Notes for downstream tools / clients:
- **`tool_choice` is rejected by V4 thinking mode in some configurations** (some agent integrations explicitly set `supportsToolChoice: false`). Test before relying on `tool_choice`.
- Some integrations set `requiresAssistantContentForToolCalls: true` — meaning assistant tool-call messages must have non-null `content` (use `""`).

---

## 15. Strict-mode Tool Calls (Beta)

Strict mode *forces* the model to comply with a function's JSON Schema.

Setup:
1. Use `https://api.deepseek.com/beta`.
2. Set `strict: true` on every function in `tools`.
3. The server validates your schema; unsupported types → error.

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
        "location": {"type": "string", "description": "The city and state"}
      },
      "required": ["location"],
      "additionalProperties": false
    }
  }
}
```

### Supported JSON Schema types in strict mode

`object`, `string`, `number`, `integer`, `boolean`, `array`, `enum`, `anyOf`.

#### `object`
- All declared properties **must** be in `required`.
- `additionalProperties: false` is **required**.

#### `string`
- Supported: `pattern`, `format` ∈ {`email`, `hostname`, `ipv4`, `ipv6`, `uuid`}.
- Unsupported: `minLength`, `maxLength`.

#### `number` / `integer`
Supported: `const`, `default`, `minimum`, `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`.

#### `array`
Unsupported: `minItems`, `maxItems`.

#### `enum`
```json
{"order_status": {"type":"string","enum":["pending","processing","shipped","cancelled"]}}
```

#### `anyOf`
```json
{"account": {"anyOf": [
  {"type":"string","format":"email"},
  {"type":"string","pattern":"^\\d{11}$"}
]}}
```

### `$def` and `$ref`

```json
{
  "type":"object",
  "properties":{
    "report_date":{"type":"string"},
    "authors":{"type":"array","items":{"$ref":"#/$def/author"}}
  },
  "required":["report_date","authors"],
  "additionalProperties":false,
  "$def":{
    "author":{
      "type":"object",
      "properties":{
        "name":{"type":"string"},
        "institution":{"type":"string"},
        "email":{"type":"string","format":"email"}
      },
      "required":["name","institution","email"],
      "additionalProperties":false
    }
  }
}
```

`$def`/`$ref` enable reusable types and recursive schemas.

---

## 16. Context (KV) Caching

KV cache is **enabled by default for everyone**, on disk. No code changes.

### Hit rules

Each cached prefix is an **independent, complete unit**. A request hits cache only when it **fully matches** an existing unit. Sliding-window attention means partial overlaps don't count.

Persistence happens at three points:
1. **Request boundaries** — every request emits cache units at:
   - end of user input,
   - end of assistant output.
2. **Common-prefix detection** — when multiple requests share a prefix, the system persists the prefix as a unit (after the first non-hit pair).
3. **Fixed-token intervals** — for very long inputs/outputs, units are inserted regularly so very long prefixes can still be cached.

Examples:
- Multi-round chat: round 2 (`A+B+C`) hits the cached prefix unit `A+B` from round 1.
- Long-doc Q&A: rounds 1 and 2 don't hit; system detects shared prefix, persists it; round 3 with same shared prefix + new question hits.

### Confirming hits

```json
"usage": {
  "prompt_tokens": 1234,
  "prompt_cache_hit_tokens": 1100,
  "prompt_cache_miss_tokens": 134,
  ...
}
```

### Pricing impact

Cache-hit input tokens are billed at the much-cheaper **cache-hit** rate (1/10 the launch price). Output tokens are still computed and billed normally — temperature randomness still applies.

### Notes
- Best-effort, **not 100% guaranteed**.
- Cache construction takes seconds.
- Auto-cleared after hours-to-days of disuse.
- KV cache is **isolated per `user_id`** when you set that field — useful for multi-tenant apps.

---

## 17. Anthropic API Compatibility

Base URL: `https://api.deepseek.com/anthropic`.

```bash
pip install anthropic
export ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic
export ANTHROPIC_API_KEY=${YOUR_API_KEY}
```

```python
import anthropic
client = anthropic.Anthropic()
msg = client.messages.create(
    model="deepseek-v4-pro",
    max_tokens=1000,
    system="You are a helpful assistant.",
    messages=[{"role":"user","content":[{"type":"text","text":"Hi, how are you?"}]}]
)
print(msg.content)
```

If you pass an unsupported model name, the backend silently maps it to `deepseek-v4-flash`.

### Compatibility table

**HTTP headers**: `x-api-key` ✓; `anthropic-beta`, `anthropic-version` ignored.

**Top-level fields**:
| Field | Status |
|---|---|
| `model` | Use DeepSeek model ID |
| `max_tokens`, `system`, `stream`, `stop_sequences`, `top_p` | ✓ |
| `temperature` | ✓ (`[0.0, 2.0]`) |
| `thinking` | ✓ (but `budget_tokens` ignored) |
| `output_config` | only `effort` honored |
| `top_k`, `metadata`, `service_tier`, `container`, `mcp_servers` | Ignored |

**Tools**:
- `tools.{name, input_schema, description}` ✓; `cache_control` ignored.
- `tool_choice`: `none`/`auto`/`any`/`tool` ✓; `disable_parallel_tool_use` ignored.

**Message content blocks**:
| Variant | Status |
|---|---|
| string content | ✓ |
| `text` | ✓ (`cache_control`, `citations` ignored) |
| `thinking` | ✓ |
| `tool_use` | ✓ (id/input/name; cache_control ignored) |
| `tool_result` | ✓ (tool_use_id/content; cache_control + is_error ignored) |
| `image`, `document`, `search_result`, `redacted_thinking`, `server_tool_use`, `web_search_tool_result`, `code_execution_tool_result`, `mcp_tool_use`, `mcp_tool_result`, `container_upload` | NOT supported |

In short: text-only Anthropic-style is fully usable; vision/web/mcp/container content blocks are not.

---

## 18. Agent Tooling Integrations

DeepSeek is documented to drop into many AI coding agents directly — usually via the Anthropic-compat or OpenAI-compat endpoint.

### 18.1 Claude Code

Already-installed Claude Code → just point env vars at DeepSeek (Linux/macOS):

```bash
export ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic
export ANTHROPIC_AUTH_TOKEN=<your DeepSeek API Key>
export ANTHROPIC_MODEL=deepseek-v4-pro[1m]
export ANTHROPIC_DEFAULT_OPUS_MODEL=deepseek-v4-pro[1m]
export ANTHROPIC_DEFAULT_SONNET_MODEL=deepseek-v4-pro[1m]
export ANTHROPIC_DEFAULT_HAIKU_MODEL=deepseek-v4-flash
export CLAUDE_CODE_SUBAGENT_MODEL=deepseek-v4-flash
export CLAUDE_CODE_EFFORT_LEVEL=max
```

PowerShell uses `$env:NAME = "value"` equivalents. Then `cd /path/to/project && claude`.

Fresh install: `npm install -g @anthropic-ai/claude-code` (Node 18+; on Windows install Git for Windows). Verify with `claude --version`.

### 18.2 GitHub Copilot (VS Code Extension)

Install the **DeepSeek V4 for Copilot Chat** extension from the VS Code marketplace (VS Code 1.116+). Run **DeepSeek: Set API Key**, paste your `sk-...`. In the model picker, choose DeepSeek V4 Pro/Flash. Thinking-effort knob is the gear icon on the model. Vision is proxied through another installed Copilot model (Claude / GPT-4o) via **DeepSeek: Set Vision Proxy Model**.

### 18.3 GitHub Copilot CLI

```bash
npm install -g @github/copilot   # Node 22+
```

```bash
export COPILOT_PROVIDER_TYPE=anthropic   # MUST be anthropic, not openai
export COPILOT_PROVIDER_BASE_URL=https://api.deepseek.com/anthropic
export COPILOT_PROVIDER_API_KEY=sk-your-deepseek-api-key
export COPILOT_MODEL=deepseek-v4-pro
# Optional explicit token limits (recommended since pro isn't in Copilot's catalog):
export COPILOT_PROVIDER_MAX_PROMPT_TOKENS=840000
export COPILOT_PROVIDER_MAX_OUTPUT_TOKENS=128000
copilot
```

> Why `anthropic` only: the OpenAI integration in Copilot CLI does not echo `reasoning_content` between turns, which DeepSeek requires in thinking mode → 400 error.

### 18.4 OpenCode

`opencode` ≥ v1.14.24 → `/connect` → `deepseek` → paste API key → select `DeepSeek-V4-Pro`.

### 18.5 Kilo Code, Crush, Pi (pi-mono), Deep Code, OpenClaw, Hermes, AstrBot, nanobot, Reasonix, Langcli, WorkBuddy/CodeBuddy, Oh My Pi

All shipped with documented DeepSeek connectors. Common patterns in their config files:

```yaml
# Oh My Pi style — illustrates the critical compat fields
providers:
  deepseek:
    baseUrl: https://api.deepseek.com
    api: openai-completions
    apiKey: DEEPSEEK_API_KEY
    authHeader: true
    models:
      - id: deepseek-v4-pro
        thinking: { mode: effort, minLevel: high, maxLevel: xhigh }
        compat:
          supportsDeveloperRole: false        # send "system" not "developer"
          maxTokensField: max_tokens          # not max_completion_tokens
          supportsReasoningEffort: true
          reasoningEffortMap: { high: high, xhigh: max }
          supportsToolChoice: false           # V4 thinking rejects tool_choice
          requiresReasoningContentForToolCalls: true
          requiresAssistantContentForToolCalls: true
          extraBody: { thinking: { type: enabled } }
```

The three critical compat flags any custom integration MUST get right:
- `supportsToolChoice: false`
- `requiresReasoningContentForToolCalls: true`
- `requiresAssistantContentForToolCalls: true`

Crush + Pi use OpenAI-compat with similar flags; pricing fields per million tokens for Pro: input $1.74 / output $3.48 / cacheRead $0.145 / cacheWrite $0.

### 18.6 nanobot example config

```json
{
  "agents": { "defaults": { "model": "deepseek-v4-pro", "provider": "deepseek" } },
  "providers": {
    "deepseek": {
      "apiKey": "<your DeepSeek API Key>",
      "apiBase": "https://api.deepseek.com/v1"
    }
  }
}
```

### 18.7 Reasonix

DeepSeek-native CLI — `npx reasonix code`. Defaults to Flash for cost; `/pro` arms Pro for next turn; `/preset max` uses Pro session-wide. API key persisted to `~/.reasonix/config.json` via first-run wizard.

### 18.8 LangChain

Officially supported. Use the OpenAI-compatible endpoint with the LangChain `ChatOpenAI` adapter and override `base_url`.

---

## 19. Streaming

Streaming is identical in shape to OpenAI's: SSE chunks of `chat.completion.chunk`, terminated by `data: [DONE]`. Set `stream: true`. Use `stream_options.include_usage: true` to receive a final usage chunk.

```python
stream = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[{"role":"user","content":"Stream me a story"}],
    stream=True,
    stream_options={"include_usage": True}
)
for chunk in stream:
    if chunk.choices and chunk.choices[0].delta.content:
        print(chunk.choices[0].delta.content, end="", flush=True)
    if chunk.usage:
        print("\nUsage:", chunk.usage)
```

In thinking mode, `delta.reasoning_content` arrives during the CoT phase; `delta.content` arrives during the final-answer phase.

If your raw HTTP parser sees `: keep-alive` comments or empty lines while waiting, ignore them — they're scheduling heartbeats, not part of the JSON body (see §5).

---

## 20. SDK Usage Patterns

### Python (OpenAI SDK)

```python
from openai import OpenAI
client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com")

# 1. Plain non-thinking
client.chat.completions.create(
    model="deepseek-v4-flash",
    messages=[{"role":"user","content":"hi"}],
    extra_body={"thinking": {"type": "disabled"}}
)

# 2. Thinking + tools (most agent code looks like this)
client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=msgs, tools=tools,
    reasoning_effort="high",
    extra_body={"thinking": {"type": "enabled"}}
)

# 3. JSON output
client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=msgs,
    response_format={"type": "json_object"}
)
```

### Node.js (OpenAI SDK)

```javascript
import OpenAI from "openai";
const client = new OpenAI({
  apiKey: process.env.DEEPSEEK_API_KEY,
  baseURL: "https://api.deepseek.com",
});

const r = await client.chat.completions.create({
  model: "deepseek-v4-pro",
  messages: [{ role: "user", content: "hi" }],
  reasoning_effort: "high",
  thinking: { type: "enabled" },                // OpenAI JS SDK passes through
});
```

### curl (raw HTTP)

```bash
curl https://api.deepseek.com/chat/completions \
  -H "Authorization: Bearer $DEEPSEEK_API_KEY" \
  -H "Content-Type: application/json" \
  -d @body.json
```

### Anthropic SDK (Python)

```python
import anthropic
client = anthropic.Anthropic()  # picks ANTHROPIC_BASE_URL/_API_KEY env vars
msg = client.messages.create(
    model="deepseek-v4-pro",
    max_tokens=1000,
    system="...",
    messages=[{"role":"user","content":[{"type":"text","text":"Hi"}]}]
)
```

---

## 21. FAQ Highlights

- **Suspended account**: Submit "Account Suspension Appeal" form; review usually within 3 business days.
- **Email not allowed for signup**: Use Gmail, Outlook, Hotmail, or Yahoo.
- **Account deletion** (`Profile → Delete`): also deletes the Chat account and forfeits Open Platform balance.
- **Top up methods**: PayPal, bank card, Alipay, WeChat Pay (under "Top Up").
- **Balance expiration**: Topped-up balance never expires; granted balance has expiration shown on Billing page.
- **Refunds**: Self-serve via Billing → Refunds (unused balances).
- **Per-key usage**: Usage page → Export by month → unzip → `amount` CSV is per-key breakdown.
- **API speed vs web**: API defaults to non-streaming; the web UI streams. Pass `stream:true` for parity.
- **Empty lines / `: keep-alive` in responses**: anti-timeout heartbeats — handle them in custom parsers.
- **LangChain**: supported via OpenAI-compatible endpoint.

---

## 22. Change Log Highlights

- **2026-04-24** — DeepSeek-V4. New IDs `deepseek-v4-pro`, `deepseek-v4-flash` on both OpenAI & Anthropic endpoints. `deepseek-chat` and `deepseek-reasoner` retire 2026-07-24.
- **2025-12-01** — DeepSeek-V3.2 ship; legacy aliases now point at V3.2 modes.
- **2025-12-01** — V3.2-Speciale via `https://api.deepseek.com/v3.2_speciale_expires_on_20251215` (no tools, expired).
- **2025-09-29** — V3.2-Exp.
- **2025-09-22** — V3.1-Terminus: language-mixing fixes, agent-mode improvements.
- **2025-08-21** — V3.1: hybrid reasoning, single-model thinking/non-thinking, big agent gains (SWE-bench Verified 66.0; Multilingual 54.5; Terminal-bench 31.3).
- **2025-05-28** — `deepseek-reasoner` upgraded to **R1-0528**; massive gains on AIME 2025 (70.0→87.5), GPQA, LCB_v6, Aider; JSON output + function calling support.
- **2025-03-24** — `deepseek-chat` upgraded to **V3-0324**; reasoning, front-end coding, Chinese writing, function calling improvements.
- **2025-01-20** — `deepseek-reasoner` introduced as DeepSeek-R1.
- **2024-12-26** — `deepseek-chat` upgraded to V3.
- **2024-12-10** — `deepseek-chat` upgraded to V2.5-1210.

---

## 23. Practical Cookbook

### 23.1 "Just answer me" — minimum cost

```python
client.chat.completions.create(
    model="deepseek-v4-flash",
    messages=[{"role":"user","content":"what's 2+2?"}],
    extra_body={"thinking": {"type": "disabled"}}
)
```

### 23.2 Long-doc Q&A with cache reuse

Send the same `system` + `<long doc>` prefix across requests; vary the question. After the second hit, the system persists the common prefix and subsequent calls bill at cache-hit rates.

### 23.3 Robust JSON

```python
import json
def chat_json(prompt, schema_example):
    resp = client.chat.completions.create(
        model="deepseek-v4-pro",
        response_format={"type":"json_object"},
        max_tokens=8000,
        messages=[
            {"role":"system","content":f"Return JSON. Example shape:\n{schema_example}"},
            {"role":"user","content":prompt},
        ],
    )
    if resp.choices[0].finish_reason == "length":
        raise RuntimeError("Truncated JSON; raise max_tokens or shorten prompt")
    return json.loads(resp.choices[0].message.content)
```

### 23.4 Tool loop in thinking mode (drop-in)

```python
def chat_with_tools(messages, tools, tool_map):
    while True:
        resp = client.chat.completions.create(
            model="deepseek-v4-pro", messages=messages, tools=tools,
            reasoning_effort="high",
            extra_body={"thinking": {"type": "enabled"}},
        )
        m = resp.choices[0].message
        messages.append(m)                           # MUST include reasoning_content
        if not m.tool_calls:
            return m.content, messages
        for tc in m.tool_calls:
            args = json.loads(tc.function.arguments)
            out  = tool_map[tc.function.name](**args)
            messages.append({"role":"tool","tool_call_id":tc.id,"content":str(out)})
```

### 23.5 Strict-mode structured output via tool

```python
client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com/beta")
tools = [{
  "type": "function",
  "function": {
    "name": "submit_summary",
    "strict": True,
    "description": "Return a strict-schema summary",
    "parameters": {
      "type": "object",
      "properties": {
        "title":   {"type":"string"},
        "rating":  {"type":"integer","minimum":1,"maximum":5},
        "tags":    {"type":"array","items":{"type":"string"}}
      },
      "required": ["title","rating","tags"],
      "additionalProperties": False
    }
  }
}]
resp = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[{"role":"user","content":"Summarize: <text>"}],
    tools=tools,
    tool_choice={"type":"function","function":{"name":"submit_summary"}}
)
arg = json.loads(resp.choices[0].message.tool_calls[0].function.arguments)
```

### 23.6 Streaming with usage at end

```python
stream = client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=[{"role":"user","content":"explain quantum tunneling"}],
    stream=True,
    stream_options={"include_usage": True},
)
final_usage = None
for chunk in stream:
    if chunk.usage:
        final_usage = chunk.usage
        continue
    delta = chunk.choices[0].delta
    if getattr(delta, "reasoning_content", None):
        # CoT chunk — print or hide
        pass
    if delta.content:
        print(delta.content, end="", flush=True)
print("\nUsage:", final_usage)
```

### 23.7 FIM code completion

```python
client = OpenAI(api_key="sk-...", base_url="https://api.deepseek.com/beta")
resp = client.completions.create(
    model="deepseek-v4-pro",
    prompt="def quicksort(arr):\n    if len(arr) <= 1:\n        return arr\n    pivot = arr[0]\n",
    suffix="\n    return left + middle + right",
    max_tokens=256
)
print(resp.choices[0].text)
```

### 23.8 Health check / balance check

```bash
curl https://api.deepseek.com/user/balance \
  -H "Authorization: Bearer $DEEPSEEK_API_KEY"
```

### 23.9 Retry logic

```python
import time, random
from openai import APIStatusError, APIConnectionError

def with_retry(fn, attempts=5):
    for i in range(attempts):
        try:
            return fn()
        except (APIStatusError, APIConnectionError) as e:
            code = getattr(e, "status_code", None)
            if code in (429, 500, 503) and i < attempts - 1:
                time.sleep((2 ** i) + random.random())
                continue
            raise
```

429/500/503 are the codes worth backing off on. 401 (bad key) and 402 (no balance) should fail fast.

### 23.10 Multi-tenant cache isolation

```python
client.chat.completions.create(
    model="deepseek-v4-pro",
    messages=msgs,
    user_id="tenant_abc"      # cache isolated per user_id within your account
)
```

---

## Appendix: Terminology Cheat-Sheet

| Term | Meaning |
|---|---|
| **Thinking mode** | Pre-answer chain-of-thought emitted as `reasoning_content`. Default on V4. |
| **Effort** | Controls how much CoT the model spends. `high` (default) or `max` (auto for some agents). |
| **Cache hit** | Input prefix tokens served from disk cache → 1/10× input price. Reported in `prompt_cache_hit_tokens`. |
| **Beta** | Features only at `https://api.deepseek.com/beta` (FIM, prefix completion, strict tools). |
| **Stateless API** | Server keeps no conversation state; you re-send the entire history each turn. |
| **Stop sequences** | Up to 16 strings that abort generation when produced. |
| **`reasoning_effort` mappings** | `low`/`medium` → `high`; `xhigh` → `max`. `tool_choice` may be unsupported alongside thinking. |
| **deprecated aliases** | `deepseek-chat` ↔ flash non-thinking; `deepseek-reasoner` ↔ flash thinking. Retire 2026-07-24. |
| **`user_id`** | Custom per-tenant id used for cache isolation and content-safety review. ≤512 chars; `[a-zA-Z0-9\-_]`. |
| **`reasoning_tokens`** | Inside `usage.completion_tokens_details` — number of CoT tokens (still billed at output rate). |

---

*End of guide.*
