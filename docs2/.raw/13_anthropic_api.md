# Anthropic API Compatibility

DeepSeek supports the Anthropic API format. Base URL: `https://api.deepseek.com/anthropic`.

## Setup

```bash
pip install anthropic
export ANTHROPIC_BASE_URL=https://api.deepseek.com/anthropic
export ANTHROPIC_API_KEY=${YOUR_API_KEY}
```

```python
import anthropic
client = anthropic.Anthropic()

message = client.messages.create(
    model="deepseek-v4-pro",
    max_tokens=1000,
    system="You are a helpful assistant.",
    messages=[
        {"role": "user",
         "content": [{"type": "text", "text": "Hi, how are you?"}]}
    ]
)
print(message.content)
```

If an unsupported model name is passed, the backend automatically maps it to `deepseek-v4-flash`.

## Compatibility Reference

### HTTP Headers
| Header | Status |
|---|---|
| anthropic-beta | Ignored |
| anthropic-version | Ignored |
| x-api-key | Fully supported |

### Top-level Fields
| Field | Status |
|---|---|
| model | Use DeepSeek model name |
| max_tokens | Fully supported |
| container | Ignored |
| mcp_servers | Ignored |
| metadata | Ignored |
| service_tier | Ignored |
| stop_sequences | Fully supported |
| stream | Fully supported |
| system | Fully supported |
| temperature | Fully supported (range [0.0, 2.0]) |
| thinking | Supported (`budget_tokens` ignored) |
| output_config | Only `effort` supported |
| top_k | Ignored |
| top_p | Fully supported |

### Tools
- `tools.name`, `tools.input_schema`, `tools.description`: Fully supported
- `tools.cache_control`: Ignored
- `tool_choice`: `none`, `auto`, `any`, `tool` all supported (`disable_parallel_tool_use` ignored)

### Message Content Variants
| Variant | Status |
|---|---|
| string | Fully supported |
| `text` block | Fully supported (`cache_control`, `citations` ignored) |
| `image` | NOT supported |
| `document` | NOT supported |
| `search_result` | NOT supported |
| `thinking` | Supported |
| `redacted_thinking` | NOT supported |
| `tool_use` | id/input/name supported (cache_control ignored) |
| `tool_result` | tool_use_id/content supported (cache_control, is_error ignored) |
| `server_tool_use` / `web_search_tool_result` / `code_execution_tool_result` / `mcp_tool_use` / `mcp_tool_result` / `container_upload` | NOT supported |
