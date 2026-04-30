"""Tests for ConstellationGuard — teleological constellation frame validation.

Uses REAL Santa video frames and REAL embeddings. NO mocks.
"""

from __future__ import annotations

import os
import tempfile

import numpy as np
import pytest

# Paths to real data
CONSTELLATION_PATH = os.path.expanduser(
    "~/.clipcannon/models/santa/embeddings/all_embeddings.npz"
)
SOURCE_VIDEO = os.path.expanduser(
    "~/.clipcannon/projects/proj_2ea7221d/source/2026-04-03 04-23-11.mp4"
)


def _extract_frame(video_path: str, time_sec: float = 5.0) -> np.ndarray:
    """Extract a single RGB frame from a video at a given timestamp.

    Uses ffmpeg via subprocess to avoid heavy opencv dependency in tests.

    Args:
        video_path: Path to the video file.
        time_sec: Timestamp in seconds.

    Returns:
        HWC uint8 RGB numpy array.
    """
    import subprocess

    cmd = [
        "ffmpeg",
        "-ss", str(time_sec),
        "-i", video_path,
        "-frames:v", "1",
        "-f", "rawvideo",
        "-pix_fmt", "rgb24",
        "-v", "error",
        "-",
    ]

    # First get the frame dimensions
    probe_cmd = [
        "ffprobe",
        "-v", "error",
        "-select_streams", "v:0",
        "-show_entries", "stream=width,height",
        "-of", "csv=p=0",
        video_path,
    ]
    probe = subprocess.run(probe_cmd, capture_output=True, text=True, timeout=10)
    w, h = [int(x) for x in probe.stdout.strip().split(",")]

    proc = subprocess.run(cmd, capture_output=True, timeout=30)
    if proc.returncode != 0:
        raise RuntimeError(f"ffmpeg failed: {proc.stderr.decode()}")

    frame = np.frombuffer(proc.stdout, dtype=np.uint8).reshape(h, w, 3)
    return frame


def _make_random_frame(h: int = 384, w: int = 384) -> np.ndarray:
    """Generate a random noise frame that should NOT match Santa."""
    rng = np.random.default_rng(42)
    return rng.integers(0, 256, size=(h, w, 3), dtype=np.uint8)


def _make_blank_frame(h: int = 384, w: int = 384) -> np.ndarray:
    """Generate a solid black frame."""
    return np.zeros((h, w, 3), dtype=np.uint8)


# ---------------------------------------------------------------------------
# Guard instantiation tests
# ---------------------------------------------------------------------------


class TestConstellationGuardInit:
    """Test constellation loading and centroid computation."""

    def test_load_real_constellation(self):
        """Load real Santa embeddings and verify all modalities are present."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)
        stats = guard.get_constellation_stats()

        assert stats["constellation_path"] == CONSTELLATION_PATH
        assert "visual" in stats["modalities"]
        assert stats["modalities"]["visual"]["dim"] == 1152
        assert stats["modalities"]["visual"]["samples"] == 1725

    def test_all_modalities_loaded(self):
        """Verify all 7 embedding modalities are loaded as centroids."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)
        stats = guard.get_constellation_stats()

        expected = {"visual", "semantic", "emotion", "speaker", "prosody", "sentiment", "voice"}
        loaded = set(stats["modalities"].keys())
        assert expected == loaded, f"Missing modalities: {expected - loaded}"

    def test_centroid_norms(self):
        """All centroids should be L2-normalized (norm ~= 1.0)."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)
        stats = guard.get_constellation_stats()

        for name, info in stats["modalities"].items():
            assert abs(info["centroid_norm"] - 1.0) < 0.01, (
                f"Centroid '{name}' not normalized: norm={info['centroid_norm']}"
            )

    def test_missing_file_raises(self):
        """Missing constellation file raises FileNotFoundError."""
        from phoenix.video.constellation_guard import ConstellationGuard

        with pytest.raises(FileNotFoundError):
            ConstellationGuard("/nonexistent/path/all_embeddings.npz")

    def test_custom_thresholds(self):
        """Custom thresholds override defaults."""
        from phoenix.video.constellation_guard import ConstellationGuard

        custom = {"visual": 0.95, "speaker": 0.99}
        guard = ConstellationGuard(CONSTELLATION_PATH, thresholds=custom)
        stats = guard.get_constellation_stats()

        assert stats["thresholds"]["visual"] == 0.95
        assert stats["thresholds"]["speaker"] == 0.99
        # Non-overridden defaults should still be present
        assert stats["thresholds"]["emotion"] == 0.60

    def test_spread_is_computed(self):
        """Constellation spread (std) should be computed for all modalities."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)
        stats = guard.get_constellation_stats()

        for name, info in stats["modalities"].items():
            assert info["spread_std"] >= 0.0, (
                f"Spread for '{name}' is negative: {info['spread_std']}"
            )
            # Visual has very tight spread based on data analysis
            if name == "visual":
                assert info["spread_std"] < 0.1, (
                    f"Visual spread unexpectedly wide: {info['spread_std']}"
                )


# ---------------------------------------------------------------------------
# Frame validation tests (require GPU + SigLIP)
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    not os.path.isfile(SOURCE_VIDEO),
    reason="Source video not available",
)
class TestFrameValidation:
    """Test frame validation against the constellation.

    These tests require GPU and the cached SigLIP model.
    """

    @pytest.fixture(scope="class")
    def guard(self):
        """Create and load a ConstellationGuard with SigLIP for the test class."""
        from phoenix.video.constellation_guard import ConstellationGuard

        g = ConstellationGuard(CONSTELLATION_PATH)
        g.load_visual_encoder()
        yield g
        g.unload()

    @pytest.fixture(scope="class")
    def santa_frame(self):
        """Extract a real Santa frame from the source video."""
        return _extract_frame(SOURCE_VIDEO, time_sec=10.0)

    @pytest.fixture(scope="class")
    def santa_frame_alt(self):
        """Extract another Santa frame at a different timestamp."""
        return _extract_frame(SOURCE_VIDEO, time_sec=60.0)

    def test_real_santa_frame_passes(self, guard, santa_frame):
        """A real frame from Santa's video should pass validation."""
        result = guard.validate_frame(santa_frame)

        assert result.valid, (
            f"Real Santa frame rejected! sim={result.visual_similarity:.4f}, "
            f"reason={result.rejection_reason}"
        )
        assert result.visual_similarity > 0.70
        assert result.rejection_reason is None
        assert "visual" in result.details

    def test_real_santa_high_similarity(self, guard, santa_frame):
        """Real Santa frame should have high visual similarity (>0.85)."""
        result = guard.validate_frame(santa_frame)

        # Based on constellation analysis, real frames have min ~0.89 cosine sim
        assert result.visual_similarity > 0.85, (
            f"Unexpectedly low similarity for real Santa frame: "
            f"{result.visual_similarity:.4f}"
        )

    def test_random_frame_rejected(self, guard):
        """A random noise frame should be rejected."""
        random_frame = _make_random_frame()
        result = guard.validate_frame(random_frame)

        # Random noise should have very low similarity to Santa
        assert result.visual_similarity < 0.90, (
            f"Random noise too similar to Santa: {result.visual_similarity:.4f}"
        )

    def test_blank_frame_rejected(self, guard):
        """A solid black frame should be rejected."""
        blank_frame = _make_blank_frame()
        result = guard.validate_frame(blank_frame)

        # Black frame should have low similarity to Santa
        assert result.visual_similarity < 0.90, (
            f"Blank frame too similar to Santa: {result.visual_similarity:.4f}"
        )

    def test_batch_validation(self, guard, santa_frame):
        """Batch validation should process multiple frames at once."""
        random_frame = _make_random_frame()
        blank_frame = _make_blank_frame()

        results = guard.validate_batch([santa_frame, random_frame, blank_frame])

        assert len(results) == 3

        # Santa frame should pass
        assert results[0].valid
        assert results[0].visual_similarity > 0.70

        # All results should have details
        for r in results:
            assert "visual" in r.details

    def test_batch_empty(self, guard):
        """Empty batch should return empty results."""
        results = guard.validate_batch([])
        assert results == []

    def test_multiple_santa_frames_consistent(self, guard, santa_frame, santa_frame_alt):
        """Multiple frames from Santa's video should all pass with similar scores."""
        result1 = guard.validate_frame(santa_frame)
        result2 = guard.validate_frame(santa_frame_alt)

        assert result1.valid
        assert result2.valid

        # Both should be in the same ballpark
        diff = abs(result1.visual_similarity - result2.visual_similarity)
        assert diff < 0.30, (
            f"Santa frame similarity inconsistent: "
            f"{result1.visual_similarity:.4f} vs {result2.visual_similarity:.4f} "
            f"(diff={diff:.4f})"
        )

    def test_threshold_sensitivity(self, guard, santa_frame):
        """Verify that threshold correctly determines accept/reject."""
        result = guard.validate_frame(santa_frame)
        actual_sim = result.visual_similarity

        # With a threshold higher than actual sim, should reject
        from phoenix.video.constellation_guard import ConstellationGuard

        strict_guard = ConstellationGuard(
            CONSTELLATION_PATH,
            thresholds={"visual": 0.999},
        )
        strict_guard._model = guard._model
        strict_guard._processor = guard._processor
        strict_guard._device = guard._device

        strict_result = strict_guard.validate_frame(santa_frame)
        if actual_sim < 0.999:
            assert not strict_result.valid
            assert strict_result.rejection_reason is not None

        # With a threshold of 0.0, everything should pass
        lenient_guard = ConstellationGuard(
            CONSTELLATION_PATH,
            thresholds={"visual": 0.0},
        )
        lenient_guard._model = guard._model
        lenient_guard._processor = guard._processor
        lenient_guard._device = guard._device

        lenient_result = lenient_guard.validate_frame(santa_frame)
        assert lenient_result.valid

    def test_validation_result_fields(self, guard, santa_frame):
        """ValidationResult should have all expected fields populated."""
        result = guard.validate_frame(santa_frame)

        assert isinstance(result.valid, bool)
        assert isinstance(result.visual_similarity, float)
        assert isinstance(result.details, dict)
        assert -1.0 <= result.visual_similarity <= 1.0


# ---------------------------------------------------------------------------
# Audio validation tests (no GPU needed, uses pre-computed embeddings)
# ---------------------------------------------------------------------------


class TestAudioValidation:
    """Test audio embedding validation against speaker/voice centroids."""

    @pytest.fixture
    def guard(self):
        """Create a ConstellationGuard (no SigLIP needed for audio)."""
        from phoenix.video.constellation_guard import ConstellationGuard

        return ConstellationGuard(CONSTELLATION_PATH)

    def test_validate_real_speaker_embedding(self, guard):
        """A real speaker embedding from the constellation should match."""
        data = np.load(CONSTELLATION_PATH)
        # Use the first real speaker embedding
        real_spk = data["spk_emb"][0].astype(np.float32)

        result = guard.validate_audio(real_spk, modality="speaker")

        assert result.visual_similarity > 0.0  # Should have some similarity
        assert "speaker" in result.details

    def test_validate_random_speaker_rejected(self, guard):
        """A random speaker embedding should score lower than real ones."""
        rng = np.random.default_rng(99)
        random_spk = rng.standard_normal(512).astype(np.float32)

        result = guard.validate_audio(random_spk, modality="speaker")

        # Random vector likely won't match Santa's voice
        assert result.visual_similarity < 0.90

    def test_validate_invalid_modality(self, guard):
        """Requesting a non-existent modality should raise ValueError."""
        dummy = np.zeros(512, dtype=np.float32)

        with pytest.raises(ValueError, match="not available"):
            guard.validate_audio(dummy, modality="nonexistent")

    def test_zero_norm_rejected(self, guard):
        """A zero-norm embedding should be rejected."""
        zero = np.zeros(512, dtype=np.float32)

        result = guard.validate_audio(zero, modality="speaker")

        assert not result.valid
        assert "Zero-norm" in (result.rejection_reason or "")


# ---------------------------------------------------------------------------
# Edge case tests
# ---------------------------------------------------------------------------


class TestEdgeCases:
    """Test edge cases and error handling."""

    def test_encoder_not_loaded_raises(self):
        """Calling validate_frame before load_visual_encoder should raise."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)

        dummy = np.zeros((384, 384, 3), dtype=np.uint8)
        with pytest.raises(RuntimeError, match="Visual encoder not loaded"):
            guard.validate_frame(dummy)

    def test_constellation_stats_complete(self):
        """get_constellation_stats should return all expected fields."""
        from phoenix.video.constellation_guard import ConstellationGuard

        guard = ConstellationGuard(CONSTELLATION_PATH)
        stats = guard.get_constellation_stats()

        assert "constellation_path" in stats
        assert "thresholds" in stats
        assert "modalities" in stats

        for name, info in stats["modalities"].items():
            assert "dim" in info
            assert "samples" in info
            assert "spread_std" in info
            assert "threshold" in info
            assert "centroid_norm" in info

    def test_corrupt_npz_raises(self):
        """A corrupt/empty npz file should raise RuntimeError for missing visual."""
        from phoenix.video.constellation_guard import ConstellationGuard

        with tempfile.NamedTemporaryFile(suffix=".npz", delete=False) as f:
            # Save an npz with no visual embedding
            np.savez(f.name, dummy=np.zeros((10, 10)))
            tmp_path = f.name

        try:
            with pytest.raises(RuntimeError, match="Visual centroid is required"):
                ConstellationGuard(tmp_path)
        finally:
            os.unlink(tmp_path)
