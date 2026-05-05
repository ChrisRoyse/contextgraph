# Context Caching (KV Cache on Disk)

Enabled by default. No code changes needed. Each request constructs a hard-disk cache; subsequent requests with overlapping prefixes get a "cache hit" on the overlap.

## Cache Persistence Rules

Due to Sliding Window Attention, each cached prefix is an independent, complete unit. A request hits cache only if it **fully matches** a cache prefix unit.

Persistence happens at:
1. **Request boundaries**: Each request produces two cache prefix units — at end of user input and end of model output.
2. **Common prefix detection**: When system detects a common prefix across multiple requests, it persists that common prefix as a unit.
3. **Fixed token intervals**: For long inputs/outputs, units carved out at fixed intervals.

### Example 1: Multi-round Conversation

```python
# First request
[{"role": "system", "content": "You are a helpful assistant"},
 {"role": "user", "content": "What is the capital of China?"}]

# Second request hits cache for the first request's prefix unit
[{"role": "system", "content": "You are a helpful assistant"},
 {"role": "user", "content": "What is the capital of China?"},
 {"role": "assistant", "content": "The capital of China is Beijing."},
 {"role": "user", "content": "What is the capital of the United States?"}]
```

### Example 2: Long Text Q&A

After the first two requests with the same `<financial report content>` prefix but different questions, the system persists that common prefix; the third request hits cache.

## Checking Cache Hit Status

`usage` field includes:
- `prompt_cache_hit_tokens` — input tokens served from cache
- `prompt_cache_miss_tokens` — input tokens that missed cache

## Notes

- Cache only matches input prefix; output remains computed (randomness from `temperature`, etc., persists).
- Best-effort, not 100% guaranteed.
- Cache construction takes seconds. Auto-cleared when unused (hours to days).
