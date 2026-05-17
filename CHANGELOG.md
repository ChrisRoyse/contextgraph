# Changelog

## Unreleased

- Added TASK-EK-001 compression-progress telemetry. New ME-JEPA training certificates may include `conditional_description_length_bits`, while legacy JSON certificate rows without the field keep their original hash body because the field is optional and omitted when absent.
- Added `mcp__cgreality__mejepa_compression_progress`, `context-graph-cli mejepa compression-progress`, and the weekly `Compression Progress` report section. These surfaces fail closed on missing or non-finite certificate bits with explicit `MEJEPA_CP_*` errors.
