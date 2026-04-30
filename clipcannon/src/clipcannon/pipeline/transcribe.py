"""WhisperX transcription pipeline stage for ClipCannon.

Transcribes audio using WhisperX with wav2vec2 forced alignment
for 20-50ms word-level precision. Falls back to faster-whisper
if WhisperX is not installed.

Includes a multi-layer anti-hallucination pipeline:
1. VAD tuning (vad_onset/vad_offset for WhisperX)
2. Threshold parameters (for faster-whisper fallback)
3. Post-transcription filtering:
   - Known hallucination phrase detection
   - Repetition/looping detection and delooping
   - Confidence-based word and segment filtering
   - Single-word segment removal

Per the constitution, WhisperX with forced alignment is MANDATORY
for production use. The faster-whisper fallback is for development
and testing only.
"""

from __future__ import annotations

import asyncio
import json
import logging
import math
import wave
import zlib
from typing import TYPE_CHECKING

import numpy as np

from clipcannon.db.connection import get_connection
from clipcannon.exceptions import PipelineError
from clipcannon.pipeline.orchestrator import StageResult
from clipcannon.pipeline.source_resolution import resolve_audio_input
from clipcannon.provenance import (
    ExecutionInfo,
    InputInfo,
    ModelInfo,
    OutputInfo,
    record_provenance,
    sha256_file,
    sha256_string,
)

if TYPE_CHECKING:
    from pathlib import Path

    from clipcannon.config import ClipCannonConfig

logger = logging.getLogger(__name__)

OPERATION = "transcription"
STAGE = "whisperx"


# ============================================================
# ANTI-HALLUCINATION CONFIGURATION
# ============================================================

# VAD thresholds for WhisperX (pyannote-based)
# Higher vad_onset = more selective about what counts as speech
_VAD_ONSET = 0.5
_VAD_OFFSET = 0.363

# Thresholds for faster-whisper fallback (ignored by WhisperX batched mode)
_NO_SPEECH_THRESHOLD = 0.4
_LOG_PROB_THRESHOLD = -0.7
_COMPRESSION_RATIO_THRESHOLD = 2.0
# Silence duration (seconds) that triggers hallucination suppression
_HALLUCINATION_SILENCE_THRESHOLD = 2.0

# Post-transcription filtering thresholds
_MIN_WORD_CONFIDENCE = 0.3
_MIN_SEGMENT_CONFIDENCE = 0.4
# Compression ratio above this indicates repetitive/hallucinated text
_MAX_COMPRESSION_RATIO = 2.0
# Known hallucination phrases -- ~35% of all Whisper hallucinations
# are just the top 2, and >50% come from the top 10.
# Substring-matched against normalized segment text.
_HALLUCINATION_PHRASES: list[str] = [
    "thank you for watching",
    "thanks for watching",
    "thank you for listening",
    "thanks for listening",
    "please subscribe",
    "like and subscribe",
    "subscribe to my channel",
    "subtitles by the amara.org community",
    "subtitles by",
    "transcript emily beynon",
    "please like and subscribe",
    "thanks for tuning in",
    "see you in the next video",
    "don't forget to subscribe",
    "hit the bell",
    "leave a comment below",
    "visit our website",
    "see you next time",
    "bye bye",
    "bye-bye",
    "music playing",
]

# Single-word or very short phrases that are hallucinated only when
# they appear as the ENTIRE segment text (not as part of real speech).
_HALLUCINATION_EXACT: list[str] = [
    "music",
    "applause",
    "laughter",
    "silence",
    "the end",
    "you",
    "check out my",
]


# ============================================================
# AUDIO CONTENT DIAGNOSTICS
# ============================================================
# Peak level below this is treated as digital silence: nothing for any
# VAD or ASR model to latch onto. Chosen well below the -40 dBFS floor
# of typical room-noise recordings but above pure numerical noise.
_SILENCE_PEAK_DBFS = -50.0
# Minimum percentage of the source that must exceed the audible threshold
# (per 50ms frame) before we will even try WhisperX. Anything below this
# is a silent/near-silent source; fail fast rather than burn GPU time.
_MIN_AUDIBLE_PCT = 1.0
# Per-frame audible threshold used to compute the audible percentage.
_AUDIBLE_FRAME_DBFS = -50.0


def _amp_to_dbfs(amp: float) -> float:
    """Convert a linear amplitude (0..1) to dBFS; clamp -infinity to -120."""
    if amp <= 0.0:
        return -120.0
    return 20.0 * math.log10(amp)


def _compute_audio_diagnostics(audio_path: Path) -> dict[str, object]:
    """Measure silence/level statistics of a PCM WAV file.

    Returns duration, peak dBFS, RMS dBFS, audible duration and audible
    percentage. Used to fail fast (with a precise diagnostic) when the
    input is effectively silent, before we waste ~40s on WhisperX.

    Args:
        audio_path: Path to a PCM WAV file (as produced by audio_extract).

    Raises:
        PipelineError: if the file cannot be read as PCM WAV.
    """
    try:
        with wave.open(str(audio_path), "rb") as wf:
            n_frames = wf.getnframes()
            sample_rate = wf.getframerate()
            channels = wf.getnchannels()
            sample_width = wf.getsampwidth()
            raw = wf.readframes(n_frames)
    except wave.Error as exc:
        raise PipelineError(
            f"Cannot read audio diagnostics from {audio_path}: {exc}. "
            f"Upstream audio_extract stage may have produced a corrupt WAV.",
            stage_name=STAGE,
            operation=OPERATION,
        ) from exc

    if sample_width == 2:
        samples = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    elif sample_width == 4:
        samples = np.frombuffer(raw, dtype=np.int32).astype(np.float32) / 2_147_483_648.0
    elif sample_width == 1:
        samples = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    else:
        raise PipelineError(
            f"Unsupported PCM sample width {sample_width} bytes in {audio_path}.",
            stage_name=STAGE,
            operation=OPERATION,
        )

    if channels > 1:
        samples = samples.reshape(-1, channels).mean(axis=1)

    duration_s = float(len(samples)) / sample_rate if sample_rate else 0.0

    if len(samples) == 0:
        return {
            "duration_s": 0.0,
            "peak_dbfs": -120.0,
            "rms_dbfs": -120.0,
            "audible_duration_s": 0.0,
            "audible_pct": 0.0,
            "sample_rate": sample_rate,
            "channels": channels,
        }

    peak = float(np.abs(samples).max())
    rms = float(np.sqrt(np.mean(samples * samples)))

    frame_size = max(1, sample_rate // 20)  # 50 ms frames
    n_full_frames = len(samples) // frame_size
    if n_full_frames > 0:
        trimmed = samples[: n_full_frames * frame_size].reshape(n_full_frames, frame_size)
        frame_rms = np.sqrt(np.mean(trimmed * trimmed, axis=1))
        threshold = 10.0 ** (_AUDIBLE_FRAME_DBFS / 20.0)
        audible_frames = int((frame_rms > threshold).sum())
    else:
        audible_frames = 0
    audible_duration_s = audible_frames * (frame_size / sample_rate)
    audible_pct = 100.0 * audible_duration_s / duration_s if duration_s > 0 else 0.0

    return {
        "duration_s": duration_s,
        "peak_dbfs": _amp_to_dbfs(peak),
        "rms_dbfs": _amp_to_dbfs(rms),
        "audible_duration_s": audible_duration_s,
        "audible_pct": audible_pct,
        "sample_rate": sample_rate,
        "channels": channels,
    }


def _reject_if_silent(audio_path: Path, diag: dict[str, object]) -> None:
    """Raise PipelineError with a full, self-diagnosing message if the
    audio is silent or near-silent. Does NOT try to recover — this is a
    required stage and silence is not transcribable.
    """
    peak_dbfs = float(diag["peak_dbfs"])
    rms_dbfs = float(diag["rms_dbfs"])
    duration_s = float(diag["duration_s"])
    audible_duration_s = float(diag["audible_duration_s"])
    audible_pct = float(diag["audible_pct"])

    def _msg(reason: str) -> str:
        return (
            f"{reason}\n"
            f"  audio: {audio_path}\n"
            f"  duration: {duration_s:.2f}s\n"
            f"  peak: {peak_dbfs:.1f} dBFS (silence threshold: "
            f"{_SILENCE_PEAK_DBFS:.0f} dBFS)\n"
            f"  RMS: {rms_dbfs:.1f} dBFS\n"
            f"  audible: {audible_duration_s:.2f}s / {duration_s:.2f}s "
            f"({audible_pct:.2f}%, threshold: {_MIN_AUDIBLE_PCT:.1f}%)\n"
            f"  sample_rate: {diag['sample_rate']} Hz, channels: {diag['channels']}\n"
            f"Resolution: supply a source video whose audio track contains "
            f"audible speech. This source contains no speech to transcribe; "
            f"clipcannon's ingest pipeline requires speech for downstream "
            f"semantic, speaker, prosody, narrative, chronemic and highlight stages."
        )

    if peak_dbfs < _SILENCE_PEAK_DBFS:
        raise PipelineError(
            _msg("Audio is digital silence; transcription is impossible."),
            stage_name=STAGE,
            operation=OPERATION,
        )
    if audible_pct < _MIN_AUDIBLE_PCT:
        raise PipelineError(
            _msg("Audio has too little audible content to transcribe."),
            stage_name=STAGE,
            operation=OPERATION,
        )


# ============================================================
# BACKEND DETECTION
# ============================================================
def _check_whisperx_available() -> bool:
    """Check if WhisperX is importable."""
    try:
        import whisperx  # noqa: F401

        return True
    except ImportError:
        return False


def _check_faster_whisper_available() -> bool:
    """Check if faster-whisper is importable."""
    try:
        from faster_whisper import WhisperModel  # noqa: F401

        return True
    except ImportError:
        return False


# ============================================================
# POST-TRANSCRIPTION HALLUCINATION FILTERING
# ============================================================
def _is_hallucination_phrase(text: str) -> bool:
    """Check if text matches a known hallucination phrase.

    Uses two-tier matching:
    - Substring match for multi-word phrases (catches them anywhere)
    - Exact match for single-word/short phrases (avoids false positives
      when the word appears naturally in real speech)

    Args:
        text: Segment text to check.

    Returns:
        True if the text is a known hallucination.
    """
    normalized = text.strip().lower()
    if not normalized:
        return True

    # Tier 1: Substring match for multi-word phrases
    if any(phrase in normalized for phrase in _HALLUCINATION_PHRASES):
        return True

    # Tier 2: Exact match for short/ambiguous phrases
    # Strip trailing punctuation for comparison
    cleaned = normalized.rstrip(".,!?;:")
    return cleaned in _HALLUCINATION_EXACT


def _detect_repetition(text: str, min_ngram: int = 2) -> bool:
    """Detect repeated n-grams indicating hallucination loops.

    When Whisper hallucinates, it often gets stuck in a loop
    repeating the same phrase over and over. Catches both
    single-word stutters ("the the the") and multi-word loops
    ("hello world hello world").

    Args:
        text: Text to check for repetition.
        min_ngram: Minimum n-gram size to check.

    Returns:
        True if repetition is detected.
    """
    words = text.lower().split()
    if len(words) < min_ngram * 2:
        return False

    max_n = min(len(words) // 2, 20)
    for n in range(min_ngram, max_n + 1):
        for i in range(len(words) - 2 * n + 1):
            ngram = tuple(words[i : i + n])
            next_ngram = tuple(words[i + n : i + 2 * n])
            if ngram == next_ngram:
                return True
    return False


def _deloop_text(text: str, min_ngram: int = 2) -> str:
    """Collapse repeated phrases to single occurrence.

    Args:
        text: Text with potential repetitions.
        min_ngram: Minimum n-gram size for delooping.

    Returns:
        Text with repeated phrases collapsed.
    """
    words = text.split()
    result: list[str] = []
    i = 0
    while i < len(words):
        matched = False
        max_n = min(len(words) - i, 20)
        for n in range(max_n, min_ngram - 1, -1):
            if i + 2 * n > len(words):
                continue
            phrase = words[i : i + n]
            next_phrase = words[i + n : i + 2 * n]
            if phrase == next_phrase:
                result.extend(phrase)
                # Skip all consecutive repetitions
                j = i + n
                while j + n <= len(words) and words[j : j + n] == phrase:
                    j += n
                i = j
                matched = True
                break
        if not matched:
            result.append(words[i])
            i += 1
    return " ".join(result)


def _compression_ratio(text: str) -> float:
    """Compute compression ratio of text.

    High ratio = repetitive/compressible = likely hallucinated.

    Args:
        text: Text to analyze.

    Returns:
        Compression ratio (uncompressed / compressed size).
    """
    text_bytes = text.encode("utf-8")
    if not text_bytes:
        return 0.0
    compressed = zlib.compress(text_bytes)
    return len(text_bytes) / len(compressed)


def _word_confidence(word: dict[str, object]) -> float | None:
    """Extract confidence score from a word dict.

    WhisperX uses 'score', faster-whisper uses 'confidence'.
    Returns None if neither key is present.

    Args:
        word: Word dict from transcript segment.

    Returns:
        Confidence score as float, or None.
    """
    score = word.get("score") or word.get("confidence")
    if score is None:
        return None
    return float(score)


def _filter_hallucinations(
    segments: list[dict[str, object]],
) -> tuple[list[dict[str, object]], int]:
    """Apply multi-layer hallucination filtering to transcript segments.

    Filtering layers:
    1. Known hallucination phrase detection
    2. Repetition/looping detection and delooping
    3. Compression ratio filtering (repetitive text)
    4. Confidence-based word filtering
    5. Confidence-based segment filtering

    Args:
        segments: Raw transcript segments from Whisper/WhisperX.

    Returns:
        Tuple of (filtered_segments, removed_count).
    """
    filtered: list[dict[str, object]] = []
    removed = 0

    for seg in segments:
        text = str(seg.get("text", "")).strip()

        # Layer 1: Known hallucination phrases
        if _is_hallucination_phrase(text):
            logger.debug(
                "Filtered hallucination phrase: %r at %.1f-%.1fs",
                text[:50],
                float(seg.get("start", 0)),
                float(seg.get("end", 0)),
            )
            removed += 1
            continue

        # Layer 2: Repetition detection
        if _detect_repetition(text):
            delooped = _deloop_text(text)
            if _is_hallucination_phrase(delooped):
                logger.debug("Filtered repeated hallucination: %r", text[:50])
                removed += 1
                continue
            seg["text"] = delooped
            logger.debug("Delooped repetition: %r -> %r", text[:50], delooped[:50])

        # Layer 3: Compression ratio (catches subtle repetition)
        text = str(seg.get("text", ""))
        if len(text) > 20:
            ratio = _compression_ratio(text)
            if ratio > _MAX_COMPRESSION_RATIO:
                delooped = _deloop_text(text)
                if len(delooped.split()) < 3:
                    logger.debug(
                        "Filtered high-compression segment: %r (ratio=%.2f)",
                        text[:50],
                        ratio,
                    )
                    removed += 1
                    continue
                seg["text"] = delooped

        # Layer 4: Word-level confidence filtering
        words = seg.get("words", [])
        if isinstance(words, list) and words:
            good_words = []
            for w in words:
                if not isinstance(w, dict):
                    continue
                conf = _word_confidence(w)
                if conf is not None and conf < _MIN_WORD_CONFIDENCE:
                    continue
                good_words.append(w)

            if not good_words:
                logger.debug(
                    "Filtered all-low-confidence segment: %r",
                    text[:50],
                )
                removed += 1
                continue

            # Layer 5: Segment-level confidence check
            scores = [c for w in good_words if (c := _word_confidence(w)) is not None]
            if scores:
                avg_conf = sum(scores) / len(scores)
                if avg_conf < _MIN_SEGMENT_CONFIDENCE:
                    logger.debug(
                        "Filtered low-confidence segment: %r (avg=%.3f)",
                        text[:50],
                        avg_conf,
                    )
                    removed += 1
                    continue

            seg["words"] = good_words
            seg["text"] = " ".join(
                str(w.get("word", "")).strip()
                for w in good_words
                if str(w.get("word", "")).strip()
            )

        # Keep the segment
        final_text = str(seg.get("text", "")).strip()
        if final_text:
            filtered.append(seg)
        else:
            removed += 1

    return filtered, removed


# ============================================================
# WHISPERX TRANSCRIPTION
# ============================================================
async def _transcribe_whisperx(
    audio_path: Path,
    model_name: str,
    compute_type: str,
    device: str,
) -> dict[str, object]:
    """Transcribe using WhisperX with forced alignment.

    Configures VAD thresholds and anti-hallucination settings.
    Note: WhisperX batched mode ignores no_speech_threshold,
    log_prob_threshold, etc. -- post-transcription filtering
    handles hallucination prevention instead.

    Args:
        audio_path: Path to the audio file.
        model_name: Whisper model size (e.g., "large-v3").
        compute_type: Compute type (e.g., "int8", "float16").
        device: Device string ("cuda" or "cpu").

    Returns:
        Dictionary with 'segments' and 'language' keys.
    """
    import whisperx

    def _run() -> dict[str, object]:
        # 1. Load model with tuned VAD and anti-hallucination options
        model = whisperx.load_model(
            model_name,
            device=device,
            compute_type=compute_type,
            vad_options={
                "vad_onset": _VAD_ONSET,
                "vad_offset": _VAD_OFFSET,
            },
            asr_options={
                "suppress_blank": True,
                "condition_on_previous_text": False,
                "initial_prompt": None,
            },
        )

        audio = whisperx.load_audio(str(audio_path))
        result = model.transcribe(
            audio,
            batch_size=16,
        )

        detected_language = result.get("language", "en")
        logger.info("WhisperX detected language: %s", detected_language)

        # 2. Forced alignment with wav2vec2
        align_model, align_metadata = whisperx.load_align_model(
            language_code=detected_language,
            device=device,
        )
        aligned = whisperx.align(
            result["segments"],
            align_model,
            align_metadata,
            audio,
            device=device,
            return_char_alignments=False,
        )

        # Clean up models
        del model, align_model
        import torch

        if torch.cuda.is_available():
            torch.cuda.empty_cache()

        return {
            "segments": aligned.get("segments", result.get("segments", [])),
            "language": detected_language,
        }

    return await asyncio.to_thread(_run)


# ============================================================
# FASTER-WHISPER FALLBACK
# ============================================================
async def _transcribe_faster_whisper(
    audio_path: Path,
    model_name: str,
    compute_type: str,
    device: str,
) -> dict[str, object]:
    """Fallback transcription using faster-whisper.

    Includes full hallucination prevention parameters that
    faster-whisper supports (unlike WhisperX batched mode).

    Args:
        audio_path: Path to the audio file.
        model_name: Whisper model size.
        compute_type: Compute type.
        device: Device string.

    Returns:
        Dictionary with 'segments' and 'language' keys.
    """
    from faster_whisper import WhisperModel

    def _run() -> dict[str, object]:
        model = WhisperModel(
            model_name,
            device=device,
            compute_type=compute_type,
        )
        # beam_size=1 (greedy) is ~3-5x faster than beam_size=5 with
        # minimal quality loss. Combined with vad_filter=True this
        # keeps transcription under 2 minutes for 8-minute videos.
        segments_iter, info = model.transcribe(
            str(audio_path),
            beam_size=1,
            word_timestamps=True,
            vad_filter=True,
            vad_parameters={"min_silence_duration_ms": 500},
            no_speech_threshold=_NO_SPEECH_THRESHOLD,
            log_prob_threshold=_LOG_PROB_THRESHOLD,
            compression_ratio_threshold=_COMPRESSION_RATIO_THRESHOLD,
            condition_on_previous_text=False,
            hallucination_silence_threshold=_HALLUCINATION_SILENCE_THRESHOLD,
            temperature=0.0,
        )

        detected_language = info.language
        logger.info("faster-whisper detected language: %s", detected_language)

        segments = []
        for seg in segments_iter:
            words = [
                {
                    "word": w.word.strip(),
                    "start": w.start,
                    "end": w.end,
                    "score": w.probability,
                }
                for w in (seg.words or [])
            ]

            segments.append(
                {
                    "start": seg.start,
                    "end": seg.end,
                    "text": seg.text.strip(),
                    "words": words,
                }
            )

        del model
        import gc
        import torch

        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

        return {
            "segments": segments,
            "language": detected_language,
        }

    return await asyncio.to_thread(_run)


# ============================================================
# DATABASE INSERT
# ============================================================
def _insert_transcript(
    db_path: Path,
    project_id: str,
    segments: list[dict[str, object]],
    language: str,
) -> tuple[int, int]:
    """Insert transcript segments and words into the database.

    Args:
        db_path: Path to the project database.
        project_id: Project identifier.
        segments: List of transcript segments (already filtered).
        language: Detected language code.

    Returns:
        Tuple of (segment_count, word_count).
    """
    conn = get_connection(db_path, enable_vec=False, dict_rows=False)
    try:
        segment_count = 0
        word_count = 0

        for seg in segments:
            start_s = float(seg.get("start", 0))
            end_s = float(seg.get("end", 0))
            text = str(seg.get("text", ""))

            if not text.strip():
                continue

            start_ms = int(start_s * 1000)
            end_ms = int(end_s * 1000)
            words_in_text = len(text.split())

            # Insert segment
            cursor = conn.execute(
                "INSERT INTO transcript_segments "
                "(project_id, start_ms, end_ms, text, language, word_count) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (project_id, start_ms, end_ms, text, language, words_in_text),
            )
            segment_id = cursor.lastrowid
            segment_count += 1

            # Insert words
            words = seg.get("words", [])
            if isinstance(words, list):
                word_rows: list[tuple[object, ...]] = []
                for w in words:
                    if not isinstance(w, dict):
                        continue
                    w_text = str(w.get("word", "")).strip()
                    if not w_text:
                        continue
                    w_start = float(w.get("start", start_s))
                    w_end = float(w.get("end", end_s))
                    w_conf = w.get("score") or w.get("confidence")
                    w_conf_float = float(w_conf) if w_conf is not None else None

                    word_rows.append(
                        (
                            segment_id,
                            w_text,
                            int(w_start * 1000),
                            int(w_end * 1000),
                            w_conf_float,
                        )
                    )

                if word_rows:
                    conn.executemany(
                        "INSERT INTO transcript_words "
                        "(segment_id, word, start_ms, end_ms, confidence) "
                        "VALUES (?, ?, ?, ?, ?)",
                        word_rows,
                    )
                    word_count += len(word_rows)

        conn.commit()
        return segment_count, word_count

    except Exception as exc:
        conn.rollback()
        raise PipelineError(
            f"Failed to insert transcript: {exc}",
            stage_name=STAGE,
            operation=OPERATION,
        ) from exc
    finally:
        conn.close()


# ============================================================
# MAIN PIPELINE STAGE
# ============================================================
async def run_transcribe(
    project_id: str,
    db_path: Path,
    project_dir: Path,
    config: ClipCannonConfig,
) -> StageResult:
    """Execute the transcription pipeline stage.

    Uses WhisperX with wav2vec2 forced alignment for 20-50ms
    word-level precision. Falls back to faster-whisper if
    WhisperX is not installed.

    Applies post-transcription hallucination filtering to both
    backends before inserting into the database.

    Args:
        project_id: Project identifier.
        db_path: Path to the project database.
        project_dir: Path to the project directory.
        config: ClipCannon configuration.

    Returns:
        StageResult indicating success or failure.
    """
    try:
        # Resolve audio input (vocals.wav preferred, audio_16k.wav fallback)
        audio_path = resolve_audio_input(project_dir)

        # Fail fast on silent / near-silent audio BEFORE loading WhisperX.
        # Rationale: loading WhisperX + running VAD on a 40s silent clip
        # takes ~40s of GPU time and produces an opaque "no segments"
        # error. Measure first; produce a self-diagnosing error instead.
        diag = await asyncio.to_thread(_compute_audio_diagnostics, audio_path)
        logger.info(
            "Audio diagnostics: duration=%.2fs peak=%.1f dBFS rms=%.1f dBFS "
            "audible=%.2fs (%.2f%%) sr=%d ch=%d",
            diag["duration_s"],
            diag["peak_dbfs"],
            diag["rms_dbfs"],
            diag["audible_duration_s"],
            diag["audible_pct"],
            diag["sample_rate"],
            diag["channels"],
        )
        _reject_if_silent(audio_path, diag)

        model_name = str(config.get("processing.whisper_model"))
        compute_type = str(config.get("processing.whisper_compute_type"))
        gpu_device = str(config.get("gpu.device"))
        device = "cuda" if "cuda" in gpu_device else "cpu"

        # Determine backend
        use_whisperx = _check_whisperx_available()
        use_faster_whisper = _check_faster_whisper_available()
        backend_name = "whisperx"

        if use_whisperx:
            logger.info(
                "Transcribing with WhisperX: model=%s, compute=%s, device=%s",
                model_name,
                compute_type,
                device,
            )
            result = await _transcribe_whisperx(
                audio_path,
                model_name,
                compute_type,
                device,
            )
        elif use_faster_whisper:
            logger.warning(
                "WhisperX not available, falling back to faster-whisper. "
                "Word alignment precision will be reduced."
            )
            backend_name = "faster-whisper"
            result = await _transcribe_faster_whisper(
                audio_path,
                model_name,
                compute_type,
                device,
            )
        else:
            return StageResult(
                success=False,
                operation=OPERATION,
                error_message=(
                    "Neither whisperx nor faster-whisper is installed. "
                    "Install with: pip install whisperx  or  pip install faster-whisper"
                ),
            )

        segments = result.get("segments", [])
        language = str(result.get("language", "en"))

        if not isinstance(segments, list) or len(segments) == 0:
            # Audio was NOT silent (we passed _reject_if_silent above) yet
            # WhisperX returned zero segments. This usually means non-speech
            # audio (music, ambient, tones) or an unsupported language.
            raise PipelineError(
                f"WhisperX/faster-whisper produced zero segments despite audible audio. "
                f"Audio diagnostics: peak={diag['peak_dbfs']:.1f} dBFS, "
                f"RMS={diag['rms_dbfs']:.1f} dBFS, "
                f"audible={diag['audible_duration_s']:.2f}s/"
                f"{diag['duration_s']:.2f}s ({diag['audible_pct']:.2f}%). "
                f"Likely causes: non-speech audio (music/ambient/noise), "
                f"unsupported language, or VAD rejected all frames. "
                f"Input audio: {audio_path}.",
                stage_name=STAGE,
                operation=OPERATION,
            )

        # Apply post-transcription hallucination filtering
        raw_count = len(segments)
        segments, removed_count = _filter_hallucinations(segments)

        if removed_count > 0:
            logger.info(
                "Hallucination filter: removed %d/%d segments (%.1f%%)",
                removed_count,
                raw_count,
                removed_count / raw_count * 100,
            )

        if not segments:
            raise PipelineError(
                f"All {raw_count} WhisperX segments were classified as "
                f"hallucinations by the post-filter (known phrases, repetition, "
                f"low confidence, or high compression ratio). "
                f"Audio diagnostics: peak={diag['peak_dbfs']:.1f} dBFS, "
                f"audible={diag['audible_duration_s']:.2f}s/"
                f"{diag['duration_s']:.2f}s. "
                f"This means the model produced only spurious output. "
                f"Input audio: {audio_path}.",
                stage_name=STAGE,
                operation=OPERATION,
            )

        # Insert into database
        segment_count, word_count = _insert_transcript(
            db_path,
            project_id,
            segments,
            language,
        )

        # Compute provenance hash
        transcript_json = json.dumps(segments, sort_keys=True, default=str)
        transcript_hash = sha256_string(transcript_json)
        input_sha256 = await asyncio.to_thread(sha256_file, audio_path)

        record_id = record_provenance(
            db_path=db_path,
            project_id=project_id,
            operation=OPERATION,
            stage=backend_name,
            input_info=InputInfo(
                file_path=str(audio_path),
                sha256=input_sha256,
                size_bytes=audio_path.stat().st_size,
            ),
            output_info=OutputInfo(
                sha256=transcript_hash,
                record_count=segment_count,
            ),
            model_info=ModelInfo(
                name=backend_name,
                version=model_name,
                quantization=compute_type,
                parameters={
                    "beam_size": 1,
                    "vad_filter": True,
                    "vad_onset": _VAD_ONSET,
                    "vad_offset": _VAD_OFFSET,
                    "hallucination_filter": True,
                    "segments_removed": removed_count,
                    "language": language,
                },
            ),
            execution_info=ExecutionInfo(
                gpu_device=device if device == "cuda" else None,
            ),
            parent_record_id="prov_001",
            description=(
                f"Transcription ({backend_name}): {segment_count} segments, "
                f"{word_count} words, {removed_count} hallucinations removed, "
                f"language={language}"
            ),
        )

        logger.info(
            "Transcription complete: %d segments, %d words, "
            "%d hallucinations removed (backend=%s, lang=%s)",
            segment_count,
            word_count,
            removed_count,
            backend_name,
            language,
        )

        return StageResult(
            success=True,
            operation=OPERATION,
            provenance_record_id=record_id,
        )

    except PipelineError:
        raise
    except Exception as exc:
        error_msg = f"{type(exc).__name__}: {exc}"
        logger.error("Transcription failed: %s", error_msg)
        raise PipelineError(
            f"Transcription failed: {error_msg}",
            stage_name=STAGE,
            operation=OPERATION,
        ) from exc
