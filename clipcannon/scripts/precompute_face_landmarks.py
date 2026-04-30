#!/usr/bin/env python3
"""Precompute face landmarks for every 25fps frame of the source video.

Uses YOLOv5-Face (the detector from HunyuanVideo-Avatar's det_align) which
reliably detects bearded faces. InsightFace and MediaPipe both fail on
Santa's beard.

Outputs a single npz file with per-frame landmarks that the labeling and
training scripts both use. This is the SINGLE SOURCE OF TRUTH for face
positions — no more scene_map interpolation, no more per-frame detection
drift.

For each frame at 25fps, stores:
  - frame_idx (int)
  - timestamp_ms (int)
  - detected (bool)
  - bbox: [x1, y1, x2, y2] in source pixel coords
  - keypoints: 5 points (left_eye, right_eye, nose, mouth_left, mouth_right)
  - mouth_bbox: derived from mouth corners
  - score (detection confidence)

Usage:
    python scripts/precompute_face_landmarks.py
"""

import gc
import logging
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger("precompute")

# Paths
PROJECT_ID = "proj_2ea7221d"
SOURCE_VIDEO = Path.home() / ".clipcannon" / "projects" / PROJECT_ID / "source" / "2026-04-03 04-23-11.mp4"
OUTPUT_FILE = Path.home() / ".clipcannon" / "models" / "santa" / "face_landmarks_25fps.npz"
HUNYUAN_PATH = Path("/home/cabdru/HunyuanVideo-Avatar")
YOLO_FACE_WEIGHTS = HUNYUAN_PATH / "weights" / "ckpts" / "det_align" / "detface.pt"

# Target sampling rate
TARGET_FPS = 25


def get_video_duration_s(video_path: Path) -> float:
    """Get video duration using ffprobe."""
    result = subprocess.run(
        ["ffprobe", "-v", "quiet", "-show_entries", "format=duration",
         "-of", "default=noprint_wrappers=1:nokey=1", str(video_path)],
        capture_output=True, text=True, check=True,
    )
    return float(result.stdout.strip())


def compute_mouth_bbox(mouth_left: np.ndarray, mouth_right: np.ndarray,
                      nose: np.ndarray) -> tuple:
    """Compute a mouth bbox from the two mouth corners.

    The mouth corners mark the horizontal extent. We use the distance from
    nose to mouth midpoint to estimate the vertical extent.
    """
    cx = (mouth_left[0] + mouth_right[0]) / 2.0
    cy = (mouth_left[1] + mouth_right[1]) / 2.0
    mouth_width = abs(mouth_right[0] - mouth_left[0])

    # Width: 1.5x the mouth corner distance (covers cheek area where mouth can open)
    bb_w = mouth_width * 1.5
    # Height: 1.0x the mouth width gives a squarish region that fits an open mouth
    bb_h = mouth_width * 1.0

    x1 = int(cx - bb_w / 2)
    y1 = int(cy - bb_h / 2)
    x2 = int(cx + bb_w / 2)
    y2 = int(cy + bb_h / 2)
    return (x1, y1, x2, y2)


def main():
    if not SOURCE_VIDEO.exists():
        log.error("Source video not found: %s", SOURCE_VIDEO)
        sys.exit(1)
    if not YOLO_FACE_WEIGHTS.exists():
        log.error("YOLOv5-Face weights not found: %s", YOLO_FACE_WEIGHTS)
        sys.exit(1)

    # Probe source dimensions and duration
    duration_s = get_video_duration_s(SOURCE_VIDEO)
    total_frames = int(duration_s * TARGET_FPS)
    log.info("Source: %s", SOURCE_VIDEO)
    log.info("Duration: %.2fs", duration_s)
    log.info("Target FPS: %d", TARGET_FPS)
    log.info("Total frames to extract+detect: %d", total_frames)

    OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)

    # Extract all frames at 25fps into a temp directory
    # Using FFmpeg with -r 25 forces 25fps output regardless of source fps
    log.info("Extracting %d frames via FFmpeg at %dfps...", total_frames, TARGET_FPS)
    with tempfile.TemporaryDirectory(prefix="face_lm_", dir="/tmp") as tmp:
        tmp_path = Path(tmp)
        frame_pattern = tmp_path / "frame_%06d.jpg"

        ffmpeg_cmd = [
            "ffmpeg", "-y", "-loglevel", "warning",
            "-i", str(SOURCE_VIDEO),
            "-r", str(TARGET_FPS),  # force 25fps output
            "-q:v", "3",  # high quality JPEG
            str(frame_pattern),
        ]
        result = subprocess.run(ffmpeg_cmd, capture_output=True, text=True)
        if result.returncode != 0:
            log.error("FFmpeg failed: %s", result.stderr)
            sys.exit(1)

        frame_files = sorted(tmp_path.glob("frame_*.jpg"))
        log.info("Extracted %d frames to %s", len(frame_files), tmp_path)

        if len(frame_files) < total_frames * 0.95:
            log.error("Expected ~%d frames but got %d — extraction failure",
                      total_frames, len(frame_files))
            sys.exit(1)

        # Load YOLOv5-Face detector
        log.info("Loading YOLOv5-Face detector...")
        sys.path.insert(0, str(HUNYUAN_PATH))
        import torch
        from hymm_sp.data_kits.face_align import AlignImage
        import cv2

        align = AlignImage(device="cuda", det_path=str(YOLO_FACE_WEIGHTS))
        log.info("Detector loaded")

        # Arrays to fill
        n = len(frame_files)
        frame_idx = np.arange(n, dtype=np.int32)
        timestamp_ms = (frame_idx * (1000.0 / TARGET_FPS)).astype(np.int32)
        detected = np.zeros(n, dtype=bool)
        bboxes = np.zeros((n, 4), dtype=np.float32)
        keypoints = np.zeros((n, 5, 2), dtype=np.float32)
        mouth_bboxes = np.zeros((n, 4), dtype=np.int32)
        scores = np.zeros(n, dtype=np.float32)

        log.info("Running YOLOv5-Face on %d frames...", n)
        last_valid_kps = None
        last_valid_bbox = None
        last_valid_mouth = None
        last_valid_score = 0.0

        for i, frame_file in enumerate(frame_files):
            frame = cv2.imread(str(frame_file))
            if frame is None:
                log.warning("Failed to read frame %d", i)
                continue

            kps_list, score_list, bbox_list = align(frame, maxface=True)

            if kps_list and score_list[0] >= 0.15:
                kps = kps_list[0]  # (5, 2) - left_eye, right_eye, nose, mouth_L, mouth_R
                bbox_raw = bbox_list[0]  # [x, y, w, h] from YOLO
                score = float(score_list[0])

                x1 = float(bbox_raw[0])
                y1 = float(bbox_raw[1])
                x2 = float(bbox_raw[0] + bbox_raw[2])
                y2 = float(bbox_raw[1] + bbox_raw[3])

                mouth_bbox = compute_mouth_bbox(kps[3], kps[4], kps[2])

                detected[i] = True
                bboxes[i] = [x1, y1, x2, y2]
                keypoints[i] = kps
                mouth_bboxes[i] = mouth_bbox
                scores[i] = score

                last_valid_kps = kps
                last_valid_bbox = [x1, y1, x2, y2]
                last_valid_mouth = mouth_bbox
                last_valid_score = score
            elif last_valid_kps is not None:
                # Fall back to last detection
                detected[i] = False
                bboxes[i] = last_valid_bbox
                keypoints[i] = last_valid_kps
                mouth_bboxes[i] = last_valid_mouth
                scores[i] = last_valid_score
            else:
                # No detection yet — mark as undetected, fill later
                detected[i] = False

            if (i + 1) % 500 == 0:
                det_rate = detected[: i + 1].sum() / (i + 1) * 100
                log.info("  %d/%d (%.1f%% detected)", i + 1, n, det_rate)

        # Backfill any undetected frames at the start with first valid detection
        first_valid_i = None
        for i in range(n):
            if detected[i]:
                first_valid_i = i
                break
        if first_valid_i is None:
            log.error("NO FACE DETECTED IN ANY FRAME - cannot proceed")
            sys.exit(1)
        if first_valid_i > 0:
            log.info("Backfilling %d initial frames with first valid detection (frame %d)",
                     first_valid_i, first_valid_i)
            bboxes[:first_valid_i] = bboxes[first_valid_i]
            keypoints[:first_valid_i] = keypoints[first_valid_i]
            mouth_bboxes[:first_valid_i] = mouth_bboxes[first_valid_i]
            scores[:first_valid_i] = scores[first_valid_i]

        det_rate = detected.sum() / n * 100
        log.info("Final detection rate: %d/%d (%.1f%%)", int(detected.sum()), n, det_rate)

        # Save
        np.savez(
            OUTPUT_FILE,
            frame_idx=frame_idx,
            timestamp_ms=timestamp_ms,
            detected=detected,
            bboxes=bboxes,
            keypoints=keypoints,
            mouth_bboxes=mouth_bboxes,
            scores=scores,
            fps=TARGET_FPS,
            source_video=str(SOURCE_VIDEO),
        )
        log.info("Saved landmarks: %s (%.1f KB)",
                 OUTPUT_FILE, OUTPUT_FILE.stat().st_size / 1024)

        # Summary stats
        log.info("")
        log.info("=" * 60)
        log.info("  SUMMARY")
        log.info("=" * 60)
        log.info("  Total frames:     %d", n)
        log.info("  Detected:         %d (%.1f%%)", int(detected.sum()), det_rate)
        log.info("  Backfilled start: %d", first_valid_i)
        log.info("  Score range:      %.2f - %.2f", scores.min(), scores.max())
        log.info("  Mean face width:  %.0f px", (bboxes[:, 2] - bboxes[:, 0]).mean())
        log.info("  Mean mouth width: %.0f px", (mouth_bboxes[:, 2] - mouth_bboxes[:, 0]).mean())


if __name__ == "__main__":
    main()
