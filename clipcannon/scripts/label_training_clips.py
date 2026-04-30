#!/usr/bin/env python3
"""Label training clips with full teleological constellation coordinates.

Uses the precomputed face landmarks file (from precompute_face_landmarks.py)
as the SINGLE SOURCE OF TRUTH for face positions and mouth bboxes. No more
scene_map interpolation, no more geometric mouth bbox derivation.

For each word in the transcript, creates a 2-second clip (49 frames at 25fps)
centered on that word, with labels for:
  - Linguistic: words, phonemes, visemes per frame
  - Face geometry per frame (from YOLOv5-Face landmarks)
  - Mouth bbox per frame (from actual mouth corners)
  - Prosody: f0, energy, speaking rate
  - Emotion: arousal, valence
  - Category: speaking/breathing/emotional

Usage:
    # First run precompute_face_landmarks.py to generate landmarks file
    python scripts/label_training_clips.py
"""

import json
import logging
import os
import sqlite3
import sys
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Optional

import numpy as np

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger("label")

PROJECT_ID = "proj_2ea7221d"
DB_PATH = Path.home() / ".clipcannon" / "projects" / PROJECT_ID / "analysis.db"
SOURCE_VIDEO = Path.home() / ".clipcannon" / "projects" / PROJECT_ID / "source" / "2026-04-03 04-23-11.mp4"
VOCALS_WAV = Path.home() / ".clipcannon" / "projects" / PROJECT_ID / "stems" / "vocals.wav"
LANDMARKS_FILE = Path.home() / ".clipcannon" / "models" / "santa" / "face_landmarks_25fps.npz"
OUTPUT_DIR = Path.home() / "echomimic_v3" / "datasets" / "santa_teleological"
OUTPUT_LABELS = OUTPUT_DIR / "labels.json"

# Clip parameters
# 49 frames at 25fps: (49-1)/4 + 1 = 13 latent temporal frames (required by VAE)
CLIP_FRAMES = 49
TARGET_FPS = 25
CLIP_DURATION_MS = int(CLIP_FRAMES * 1000 / TARGET_FPS)  # 1960 ms (not 2000)
SRC_WIDTH = 2560
SRC_HEIGHT = 1440


@dataclass
class TeleologicalLabel:
    """Complete teleological label for a training clip."""
    clip_id: str
    # Temporal anchor
    start_ms: int
    end_ms: int
    center_word: str
    center_word_ms: int

    # Linguistic units
    words: list = field(default_factory=list)
    phonemes: list = field(default_factory=list)
    visemes: list = field(default_factory=list)
    viseme_sequence: list = field(default_factory=list)  # per-frame viseme (49 entries)

    # Face geometry per frame (from precomputed landmarks)
    face_bbox_per_frame: list = field(default_factory=list)   # [x1,y1,x2,y2] per frame
    mouth_bbox_per_frame: list = field(default_factory=list)  # [x1,y1,x2,y2] per frame
    mouth_center_per_frame: list = field(default_factory=list)  # [x,y] per frame

    # Detection quality (flags for filtering bad clips)
    frames_detected: int = 0       # How many of 49 frames had real detection
    mean_face_score: float = 0.0

    # Prosody
    prosody_f0_mean: float = 0.0
    prosody_f0_std: float = 0.0
    prosody_energy_mean: float = 0.0
    prosody_energy_peak: float = 0.0
    prosody_speaking_rate: float = 0.0
    prosody_energy_level: str = "medium"
    prosody_contour_type: str = "flat"
    prosody_has_emphasis: bool = False
    prosody_has_breath: bool = False

    # Emotion
    emotion_arousal: float = 0.19
    emotion_valence: float = 0.51
    emotion_energy: float = 0.0

    # Context flags
    has_speech: bool = True
    clip_category: str = "speaking"


# Viseme mapping based on CMU ARPAbet phonemes
PHONEME_TO_VISEME = {
    "SIL": "SIL", "": "SIL",
    # Bilabials (lip closure)
    "P": "P", "B": "P", "M": "P",
    # Labiodentals
    "F": "F", "V": "F",
    # Dentals
    "TH": "TH", "DH": "TH",
    # Alveolars
    "T": "T", "D": "T", "N": "T", "L": "T", "S": "T", "Z": "T",
    # Post-alveolars
    "SH": "SH", "ZH": "SH", "CH": "SH", "JH": "SH",
    # Velars
    "K": "K", "G": "K", "NG": "K",
    "HH": "SIL",
    # Approximants
    "R": "R", "W": "W", "Y": "Y",
    # Front vowels
    "IY": "IY", "IY0": "IY", "IY1": "IY", "IY2": "IY",
    "IH": "IH", "IH0": "IH", "IH1": "IH", "IH2": "IH",
    "EH": "EH", "EH0": "EH", "EH1": "EH", "EH2": "EH",
    "AE": "AH", "AE0": "AH", "AE1": "AH", "AE2": "AH",
    "AH": "AH", "AH0": "AH", "AH1": "AH", "AH2": "AH",
    "ER": "ER", "ER0": "ER", "ER1": "ER", "ER2": "ER",
    "AA": "AA", "AA0": "AA", "AA1": "AA", "AA2": "AA",
    "AO": "AO", "AO0": "AO", "AO1": "AO", "AO2": "AO",
    "UH": "UH", "UH0": "UH", "UH1": "UH", "UH2": "UH",
    "UW": "UW", "UW0": "UW", "UW1": "UW", "UW2": "UW",
    "EY": "EY", "EY0": "EY", "EY1": "EY", "EY2": "EY",
    "AY": "AY", "AY0": "AY", "AY1": "AY", "AY2": "AY",
    "OY": "OY", "OY0": "OY", "OY1": "OY", "OY2": "OY",
    "AW": "AW", "AW0": "AW", "AW1": "AW", "AW2": "AW",
    "OW": "OW", "OW0": "OW", "OW1": "OW", "OW2": "OW",
}

ALL_VISEMES = [
    "SIL", "P", "F", "TH", "T", "SH", "K", "R", "W", "Y",
    "IY", "IH", "EH", "AH", "ER", "AA", "AO", "UH", "UW",
    "EY", "AY", "OY", "AW", "OW",
]


def load_cmu_dict():
    """Load CMU pronunciation dictionary."""
    import nltk
    from nltk.corpus import cmudict
    try:
        return cmudict.dict()
    except LookupError:
        nltk.download("cmudict", quiet=True)
        return cmudict.dict()


def word_to_visemes(word: str, cmu: dict, word_start: int, word_end: int) -> list:
    """Convert a word to (viseme, start_ms, end_ms) tuples."""
    word_clean = word.lower().strip(".,!?;:\"'()[]")
    phones = cmu.get(word_clean, [])
    if not phones:
        return [("SIL", word_start, word_end)]
    phoneme_list = phones[0]
    if not phoneme_list:
        return [("SIL", word_start, word_end)]
    duration = word_end - word_start
    step = duration / len(phoneme_list)
    visemes = []
    for i, phoneme in enumerate(phoneme_list):
        viseme = PHONEME_TO_VISEME.get(phoneme, "SIL")
        p_start = int(word_start + i * step)
        p_end = int(word_start + (i + 1) * step)
        visemes.append((viseme, p_start, p_end))
    return visemes


def build_frame_viseme_sequence(visemes: list, clip_start_ms: int, n_frames: int) -> list:
    """Convert (viseme, start_ms, end_ms) to per-frame viseme labels."""
    frame_duration_ms = CLIP_DURATION_MS / n_frames
    sequence = []
    for frame_idx in range(n_frames):
        frame_ms = clip_start_ms + frame_idx * frame_duration_ms
        active = "SIL"
        for viseme, v_start, v_end in visemes:
            if v_start <= frame_ms < v_end:
                active = viseme
                break
        sequence.append(active)
    return sequence


def load_precomputed_landmarks():
    """Load the precomputed face landmarks file.

    This is the single source of truth for face positions and mouth bboxes.
    Produced by precompute_face_landmarks.py using YOLOv5-Face.
    """
    if not LANDMARKS_FILE.exists():
        raise FileNotFoundError(
            f"Face landmarks file not found: {LANDMARKS_FILE}\n"
            f"Run: python scripts/precompute_face_landmarks.py"
        )
    data = np.load(LANDMARKS_FILE)
    log.info("Loaded landmarks: %d frames at %dfps",
             len(data["frame_idx"]), int(data["fps"]))
    log.info("Detection rate: %d/%d (%.1f%%)",
             int(data["detected"].sum()), len(data["detected"]),
             data["detected"].mean() * 100)
    return {
        "timestamp_ms": data["timestamp_ms"],
        "detected": data["detected"],
        "bboxes": data["bboxes"],
        "keypoints": data["keypoints"],
        "mouth_bboxes": data["mouth_bboxes"],
        "scores": data["scores"],
        "fps": int(data["fps"]),
    }


def get_landmarks_for_frame(landmarks: dict, ms: int):
    """Look up the nearest landmark entry for a given timestamp (ms).

    Uses simple nearest-neighbor matching on the timestamp array.
    """
    ts = landmarks["timestamp_ms"]
    # Find nearest frame index
    idx = int(np.searchsorted(ts, ms))
    if idx >= len(ts):
        idx = len(ts) - 1
    elif idx > 0 and (ms - ts[idx - 1]) < (ts[idx] - ms):
        idx -= 1

    return {
        "idx": idx,
        "detected": bool(landmarks["detected"][idx]),
        "bbox": landmarks["bboxes"][idx].tolist(),
        "mouth_bbox": landmarks["mouth_bboxes"][idx].tolist(),
        "keypoints": landmarks["keypoints"][idx].tolist(),
        "score": float(landmarks["scores"][idx]),
    }


def load_analysis_data(db_path: Path, project_id: str) -> dict:
    """Load words, prosody, emotion, silence gaps from ClipCannon DB.

    Note: the diarization in this particular project collapsed all speakers
    into speaker_id=1, so we cannot filter by speaker. Instead, we filter
    out words that fall inside silence_gaps longer than 1 second (those
    are likely non-speech regions).
    """
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row

    data = {}

    rows = conn.execute(
        """SELECT w.word, w.start_ms, w.end_ms, w.confidence,
                  s.text as segment_text, s.start_ms as seg_start, s.end_ms as seg_end
           FROM transcript_words w
           JOIN transcript_segments s ON w.segment_id = s.segment_id
           WHERE s.project_id=?
           ORDER BY w.start_ms""",
        (project_id,),
    ).fetchall()
    data["words"] = [dict(r) for r in rows]
    log.info("Loaded %d words", len(data["words"]))

    rows = conn.execute(
        """SELECT start_ms, end_ms, f0_mean, f0_std, energy_mean, energy_peak,
                  speaking_rate_wpm, energy_level, pitch_contour_type,
                  has_emphasis, has_breath, transcript_text
           FROM prosody_segments WHERE project_id=?
           ORDER BY start_ms""",
        (project_id,),
    ).fetchall()
    data["prosody"] = [dict(r) for r in rows]
    log.info("Loaded %d prosody segments", len(data["prosody"]))

    rows = conn.execute(
        """SELECT start_ms, end_ms, AVG(arousal) as arousal,
                  AVG(valence) as valence, AVG(energy) as energy
           FROM emotion_curve WHERE project_id=?
           GROUP BY start_ms, end_ms ORDER BY start_ms""",
        (project_id,),
    ).fetchall()
    data["emotion"] = [dict(r) for r in rows]
    log.info("Loaded %d emotion windows", len(data["emotion"]))

    rows = conn.execute(
        """SELECT start_ms, end_ms, duration_ms, type
           FROM silence_gaps WHERE project_id=?
           ORDER BY start_ms""",
        (project_id,),
    ).fetchall()
    data["silence_gaps"] = [dict(r) for r in rows]
    log.info("Loaded %d silence gaps", len(data["silence_gaps"]))

    conn.close()
    return data


def get_prosody_for_range(start_ms: int, end_ms: int, prosody: list) -> Optional[dict]:
    """Find the prosody segment that most overlaps this range."""
    best = None
    best_overlap = 0
    for p in prosody:
        overlap = min(end_ms, p["end_ms"]) - max(start_ms, p["start_ms"])
        if overlap > best_overlap:
            best_overlap = overlap
            best = p
    return best


def get_emotion_for_range(start_ms: int, end_ms: int, emotion: list) -> tuple:
    """Average emotion across overlapping windows."""
    arousal_vals = []
    valence_vals = []
    energy_vals = []
    for e in emotion:
        overlap = min(end_ms, e["end_ms"]) - max(start_ms, e["start_ms"])
        if overlap > 0:
            arousal_vals.append(e["arousal"])
            valence_vals.append(e["valence"])
            energy_vals.append(e.get("energy", 0))
    if not arousal_vals:
        return 0.19, 0.51, 0.0
    return (float(np.mean(arousal_vals)),
            float(np.mean(valence_vals)),
            float(np.mean(energy_vals)))


def is_in_long_silence(center_ms: int, silence_gaps: list) -> bool:
    """Is this moment inside a long silence (>1s)?

    Since diarization failed, we use silence gaps as a proxy to filter out
    interviewer speech and long pauses where Santa isn't speaking.
    """
    for s in silence_gaps:
        duration = s.get("duration_ms", s["end_ms"] - s["start_ms"])
        if duration >= 1000 and s["start_ms"] <= center_ms <= s["end_ms"]:
            return True
    return False


def build_labels():
    """Main pipeline: create labeled clips from ClipCannon data."""
    log.info("=" * 70)
    log.info("  TELEOLOGICAL CONSTELLATION LABELING PIPELINE")
    log.info("=" * 70)

    if not DB_PATH.exists():
        raise FileNotFoundError(f"Database not found: {DB_PATH}")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Load ClipCannon analysis data
    data = load_analysis_data(DB_PATH, PROJECT_ID)

    # Load precomputed face landmarks (the single source of truth)
    landmarks = load_precomputed_landmarks()
    max_landmarks_ms = int(landmarks["timestamp_ms"][-1])
    log.info("Landmarks cover up to %dms", max_landmarks_ms)

    cmu = load_cmu_dict()
    words = data["words"]

    # Pre-compute visemes for all words
    log.info("Computing viseme sequences for all words...")
    all_visemes = []
    for w in words:
        visemes = word_to_visemes(w["word"], cmu, w["start_ms"], w["end_ms"])
        all_visemes.extend(visemes)
    log.info("Total viseme segments: %d", len(all_visemes))

    # Viseme distribution stats
    viseme_counts = {}
    for v, _, _ in all_visemes:
        viseme_counts[v] = viseme_counts.get(v, 0) + 1
    log.info("Viseme distribution:")
    for v in sorted(viseme_counts.keys()):
        log.info("  %-5s: %d", v, viseme_counts[v])

    log.info("Building clip labels (one per word)...")
    labels: list[TeleologicalLabel] = []
    skipped_negative = 0
    skipped_beyond_landmarks = 0
    skipped_silence = 0
    skipped_no_detection = 0

    for i, word in enumerate(words):
        word_center = (word["start_ms"] + word["end_ms"]) // 2
        clip_start = word_center - CLIP_DURATION_MS // 2
        clip_end = clip_start + CLIP_DURATION_MS

        # Filter: clip must start after time 0
        if clip_start < 0:
            skipped_negative += 1
            continue

        # Filter: clip must be within landmark coverage
        if clip_end > max_landmarks_ms:
            skipped_beyond_landmarks += 1
            continue

        # Filter: skip clips inside long silences (likely not Santa speaking)
        if is_in_long_silence(word_center, data["silence_gaps"]):
            skipped_silence += 1
            continue

        # Collect per-frame landmarks for this clip
        face_bboxes = []
        mouth_bboxes = []
        mouth_centers = []
        detected_flags = []
        scores = []

        for frame_idx in range(CLIP_FRAMES):
            frame_ms = clip_start + frame_idx * (CLIP_DURATION_MS / CLIP_FRAMES)
            lm = get_landmarks_for_frame(landmarks, int(frame_ms))
            face_bboxes.append(lm["bbox"])
            mouth_bboxes.append(lm["mouth_bbox"])
            mouth_cx = (lm["mouth_bbox"][0] + lm["mouth_bbox"][2]) / 2
            mouth_cy = (lm["mouth_bbox"][1] + lm["mouth_bbox"][3]) / 2
            mouth_centers.append([mouth_cx, mouth_cy])
            detected_flags.append(lm["detected"])
            scores.append(lm["score"])

        # Filter: require at least 30/49 frames to have real detections (not backfilled)
        n_detected = sum(detected_flags)
        if n_detected < 30:
            skipped_no_detection += 1
            continue

        # Collect words in clip
        words_in_clip = [
            (w["word"], w["start_ms"], w["end_ms"])
            for w in words
            if clip_start <= w["start_ms"] < clip_end
            or clip_start < w["end_ms"] <= clip_end
        ]

        # Collect visemes in clip
        visemes_in_clip = [
            (v, vs, ve)
            for v, vs, ve in all_visemes
            if clip_start <= vs < clip_end or clip_start < ve <= clip_end
        ]

        viseme_seq = build_frame_viseme_sequence(visemes_in_clip, clip_start, CLIP_FRAMES)

        # Prosody + emotion context
        prosody = get_prosody_for_range(clip_start, clip_end, data["prosody"])
        arousal, valence, energy = get_emotion_for_range(clip_start, clip_end, data["emotion"])

        # Classify clip
        has_speech = not is_in_long_silence(word_center, data["silence_gaps"])
        if not has_speech:
            category = "breathing"
        elif prosody and prosody.get("has_emphasis"):
            category = "emotional"
        else:
            category = "speaking"

        label = TeleologicalLabel(
            clip_id=f"{len(labels):05d}",
            start_ms=clip_start,
            end_ms=clip_end,
            center_word=word["word"],
            center_word_ms=word_center,
            words=words_in_clip,
            visemes=visemes_in_clip,
            viseme_sequence=viseme_seq,
            face_bbox_per_frame=face_bboxes,
            mouth_bbox_per_frame=mouth_bboxes,
            mouth_center_per_frame=mouth_centers,
            frames_detected=n_detected,
            mean_face_score=float(np.mean(scores)),
            prosody_f0_mean=float(prosody["f0_mean"]) if prosody else 0.0,
            prosody_f0_std=float(prosody["f0_std"]) if prosody else 0.0,
            prosody_energy_mean=float(prosody["energy_mean"]) if prosody else 0.0,
            prosody_energy_peak=float(prosody["energy_peak"]) if prosody else 0.0,
            prosody_speaking_rate=float(prosody["speaking_rate_wpm"]) if prosody else 0.0,
            prosody_energy_level=prosody["energy_level"] if prosody else "medium",
            prosody_contour_type=prosody["pitch_contour_type"] if prosody else "flat",
            prosody_has_emphasis=bool(prosody["has_emphasis"]) if prosody else False,
            prosody_has_breath=bool(prosody["has_breath"]) if prosody else False,
            emotion_arousal=arousal,
            emotion_valence=valence,
            emotion_energy=energy,
            has_speech=has_speech,
            clip_category=category,
        )
        labels.append(label)

        if (i + 1) % 500 == 0:
            log.info("Processed %d/%d words (kept %d)", i + 1, len(words), len(labels))

    log.info("")
    log.info("=" * 60)
    log.info("  FILTERING SUMMARY")
    log.info("=" * 60)
    log.info("Total words:              %d", len(words))
    log.info("Skipped — negative start: %d", skipped_negative)
    log.info("Skipped — beyond landmarks: %d", skipped_beyond_landmarks)
    log.info("Skipped — long silence:   %d", skipped_silence)
    log.info("Skipped — <30/49 detected: %d", skipped_no_detection)
    log.info("Kept clips:               %d", len(labels))

    # Category breakdown
    by_category = {}
    for l in labels:
        by_category[l.clip_category] = by_category.get(l.clip_category, 0) + 1
    log.info("Categories: %s", by_category)

    # Viseme diversity
    diversity = [len(set(l.viseme_sequence)) for l in labels]
    log.info("Viseme diversity per clip: mean=%.1f max=%d",
             float(np.mean(diversity)), int(np.max(diversity)))

    # Quality stats
    scores = [l.mean_face_score for l in labels]
    log.info("Mean face score across clips: %.2f-%.2f (avg %.2f)",
             float(np.min(scores)), float(np.max(scores)), float(np.mean(scores)))

    # Save to JSON
    labels_dict = {
        "project_id": PROJECT_ID,
        "total_clips": len(labels),
        "clip_frames": CLIP_FRAMES,
        "clip_duration_ms": CLIP_DURATION_MS,
        "target_fps": TARGET_FPS,
        "viseme_categories": ALL_VISEMES,
        "clips": [asdict(l) for l in labels],
    }

    OUTPUT_LABELS.write_text(json.dumps(labels_dict, indent=2), encoding="utf-8")
    log.info("Saved labels: %s (%.1f MB)",
             OUTPUT_LABELS, OUTPUT_LABELS.stat().st_size / 1e6)

    return labels_dict


if __name__ == "__main__":
    build_labels()
