# 2026-04-30 Monolith Integration FSV

## Scope

Next integration step after O*NET ZIP validation: make the vendored
`clipcannon/` Python project resolvable from the monorepo, wire root build/test
targets, document license boundaries, and manually verify runtime state by
reading the persisted SQLite source of truth.

## Sources Reviewed

- uv workspaces and dependency management: https://docs.astral.sh/uv/concepts/projects/workspaces/
- uv optional dependency and conflict behavior: https://docs.astral.sh/uv/concepts/projects/dependencies/
- uv sync/extra behavior: https://docs.astral.sh/uv/concepts/projects/sync/
- PyAV source-build requirements: https://pyav.org/docs/stable/overview/installation.html
- AudioCraft installation requirements: https://github.com/facebookresearch/audiocraft
- Python license expression spec: https://packaging.python.org/en/latest/specifications/license-expression/
- SPDX license list: https://spdx.org/licenses/

## Root Causes Found And Fixed

1. `clipcannon/pyproject.toml` advertised `requires-python = ">=3.12"`.
   `uv` therefore had to solve future Python versions and failed. Fixed to
   `>=3.12,<3.13`, matching the existing Ruff target and verified interpreter.

2. `clipcannon[phase2]` declared `audiocraft>=1.3.0`. AudioCraft requires an
   older torch stack, and its `av==11.0.0` dependency has no CPython 3.12 wheel.
   Source build requires FFmpeg development headers not present on this host.
   Fixed by removing AudioCraft from managed extras and no longer advertising
   `musicgen` as a selectable managed model.

3. `clipcannon[phase3]` declared `resemble-enhance>=0.0.1`. That package pins
   `torch==2.1.1`, which has no CPython 3.12 wheel. Fixed by removing it from
   managed extras, defaulting voice enhancement to false, and changing
   enhancement failure behavior to return `ENHANCEMENT_FAILED` instead of using
   raw TTS output as a fallback when enhancement is explicitly requested.

4. Package license metadata used `BSL-1.1`, which Hatchling rejects. The valid
   SPDX identifier for Business Source License 1.1 is `BUSL-1.1`. Fixed package
   metadata and added root `LICENSING.md`.

5. `voiceagent` was present in `clipcannon/src/voiceagent` but absent from
   Hatchling wheel packages. Fixed by adding `src/voiceagent` to the package
   list and verifying import from the installed editable package.

6. `tests/voiceagent/conftest.py` imported `torch` at module import time, which
   made non-GPU voiceagent tests fail under the dev environment. Fixed by moving
   the import into the GPU fixture; GPU tests still fail explicitly if `torch`
   or CUDA is missing.

## Files Updated

- `pyproject.toml`
- `LICENSING.md`
- `.gitignore`
- `Makefile`
- `clipcannon/pyproject.toml`
- `clipcannon/uv.lock`
- `clipcannon/src/clipcannon/tools/audio.py`
- `clipcannon/src/clipcannon/tools/audio_defs.py`
- `clipcannon/src/clipcannon/tools/voice.py`
- `clipcannon/src/clipcannon/tools/voice_defs.py`
- `clipcannon/src/voiceagent/meeting/config.py`
- `clipcannon/tests/test_music_planner.py`
- `clipcannon/tests/voiceagent/conftest.py`

## Source Of Truth

- Python dependency truth: `clipcannon/pyproject.toml` and `clipcannon/uv.lock`.
- Installed package truth: editable metadata read through `importlib.metadata`.
- Runtime state truth: SQLite database
  `tmp/clipcannon_integration_fsv/home/.clipcannon/projects/fsv_integration_project/analysis.db`
  and WAV files under that project `audio/` directory.
- Tool-schema truth: imported `AUDIO_TOOL_DEFINITIONS` and
  `VOICE_TOOL_DEFINITIONS`.

## Verification Commands

Passed:

```bash
uv lock --project clipcannon
make check-python
make test-python-smoke
cd clipcannon && uv run --locked --extra dev ruff check pyproject.toml \
  src/clipcannon/tools/audio.py src/clipcannon/tools/audio_defs.py \
  src/clipcannon/tools/voice.py src/clipcannon/tools/voice_defs.py \
  src/voiceagent/meeting/config.py tests/test_music_planner.py \
  tests/voiceagent/conftest.py
make check-rust
```

Observed:

- Python smoke: `104 passed, 5 skipped`.
- Rust check: passed; existing benchmark warning noise remains.
- Touched-file Ruff check: passed.
- Full `make lint-python`: failed with 530 pre-existing Ruff findings across the
  vendored ClipCannon tree. This remains an open cleanup gate; it was not hidden
  by narrowing the full lint target.

Installed metadata readback:

```text
Name: clipcannon
Version: 0.1.0
License-Expression: BUSL-1.1
Requires-Python: <3.13,>=3.12
Provides-Extra: ['dev', 'ml', 'phase2', 'phase3']
```

Tool schema readback:

```text
music_model_enum: ['ace-step']
speak_enhance_default: False
speak_optimized_enhance_default: False
```

Dependency tree top-level readback:

```text
clipcannon v0.1.0
├── numpy v2.4.3
├── scipy v1.17.1
├── pytest v9.0.2 (extra: dev)
├── ruff v0.15.7 (extra: dev)
├── torch v2.10.0 (extra: ml)
├── ace-step v0.1.0 (extra: phase2)
└── soundfile v0.13.1 (extra: phase3)
```

## Manual FSV Runtime Cases

Happy path: generated known SFX input `tick`, `duration_ms=250`.

- Before: `audio_asset_count=0`, no WAV files.
- Result: returned `audio_asset_id=audio_59c4092deab8`, duration `250`.
- After: `audio_asset_count=1`; WAV exists with 1 channel, 44100 Hz, 11025
  frames, 22094 bytes.

Edge case 1: invalid SFX type `not_a_real_sfx`.

- Before: `audio_asset_count=1`.
- Result: `INVALID_PARAMETER`.
- After: `audio_asset_count=1`; no new WAV file.

Edge case 2: maximum valid duration, `chime`, `duration_ms=30000`.

- Before: `audio_asset_count=1`.
- Result: returned `audio_asset_id=audio_082e2e28d530`, duration `30000`.
- After: `audio_asset_count=2`; WAV exists with 1 channel, 44100 Hz, 1323000
  frames, 2646044 bytes.

Edge case 3: missing edit id.

- Before: `audio_asset_count=2`.
- Result: `EDIT_NOT_FOUND`.
- After: `audio_asset_count=2`; no new WAV file.

Edge case 4: unsupported `musicgen` model.

- Before: `audio_asset_count=2`.
- Result: `INVALID_PARAMETER`, supported models `['ace-step']`.
- After: `audio_asset_count=2`; no new WAV file.

Final database readback:

```json
{
  "audio_assets": [
    {
      "asset_id": "audio_59c4092deab8",
      "duration_ms": 250,
      "file_exists": true,
      "file_bytes": 22094,
      "generation_params": "{\"sfx_type\": \"tick\", \"duration_ms\": 250, \"params\": {}}",
      "model_used": "dsp",
      "sample_rate": 44100,
      "type": "sfx",
      "wav_channels": 1,
      "wav_frames": 11025,
      "wav_sample_rate": 44100
    },
    {
      "asset_id": "audio_082e2e28d530",
      "duration_ms": 30000,
      "file_exists": true,
      "file_bytes": 2646044,
      "generation_params": "{\"sfx_type\": \"chime\", \"duration_ms\": 30000, \"params\": {}}",
      "model_used": "dsp",
      "sample_rate": 44100,
      "type": "sfx",
      "wav_channels": 1,
      "wav_frames": 1323000,
      "wav_sample_rate": 44100
    }
  ]
}
```

## Open Gates

- Full Python lint is still not clean: 530 pre-existing Ruff issues.
- MusicGen/AudioCraft and Resemble Enhance are not part of the managed Python
  3.12 integration because their published dependency pins are incompatible
  with CPython 3.12. They should not be reintroduced until upstream packages
  publish compatible torch/PyAV metadata or the repo explicitly supports a
  separate older-Python environment.
