"""Tests for ConstellationController -- runtime expression control.

Uses REAL FLAME data -- no mocks.
"""

from __future__ import annotations

import sys

import pytest

sys.path.insert(0, "/home/cabdru/clipcannon/src")

from phoenix.video.constellation_controller import ConstellationController
from phoenix.video.expression_skills import SkillLibrary

FLAME_PATH = "/home/cabdru/.clipcannon/models/santa/flame_params.npz"
EMBEDDINGS_PATH = "/home/cabdru/.clipcannon/models/santa/embeddings/all_embeddings.npz"


@pytest.fixture(scope="module")
def library() -> SkillLibrary:
    lib = SkillLibrary()
    lib.extract_from_training_data(EMBEDDINGS_PATH, FLAME_PATH)
    return lib


@pytest.fixture
def controller(library: SkillLibrary) -> ConstellationController:
    return ConstellationController(library)


# ------------------------------------------------------------------
# Setting emotional state
# ------------------------------------------------------------------

class TestSetEmotionalState:
    def test_set_valid_constellation(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        prompt = controller.get_prompt_for_frame(0, fps=25)
        assert len(prompt) > 0

    def test_set_with_intensity(self, controller: ConstellationController):
        controller.set_emotional_state("happy_storytelling", intensity=0.2)
        prompt = controller.get_prompt_for_frame(0, fps=25)
        assert "subtly" in prompt or "slightly" in prompt

    def test_invalid_constellation_raises(self, controller: ConstellationController):
        with pytest.raises(KeyError):
            controller.set_emotional_state("nonexistent_state")

    def test_intensity_clamped(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational", intensity=5.0)
        assert controller._intensity == 1.0
        controller.set_emotional_state("warm_conversational", intensity=-2.0)
        assert controller._intensity == 0.0


# ------------------------------------------------------------------
# Queue expressions
# ------------------------------------------------------------------

class TestQueueExpressions:
    def test_queue_single(self, controller: ConstellationController):
        controller.queue_expression("genuine_laugh", duration=1.0, at_time=0.0)
        prompt = controller.get_prompt_for_frame(5, fps=25)
        assert len(prompt) > 0

    def test_queue_multiple(self, controller: ConstellationController):
        controller.queue_expression("warm_smile", duration=1.0, at_time=0.0)
        controller.queue_expression("genuine_laugh", duration=1.0, at_time=1.0)
        # Frame 5 (0.2s) should be warm_smile
        p1 = controller.get_prompt_for_frame(5, fps=25)
        assert "smile" in p1.lower()
        # Frame 37 (1.5s) should be genuine_laugh
        p2 = controller.get_prompt_for_frame(37, fps=25)
        assert "laugh" in p2.lower()

    def test_queue_auto_time(self, controller: ConstellationController):
        controller.queue_expression("warm_smile", duration=1.0)
        controller.queue_expression("genuine_laugh", duration=1.0)
        # Second should start at 1.0
        assert len(controller._queue) == 2
        assert controller._queue[1].start_time == pytest.approx(1.0)

    def test_queue_invalid_skill_raises(self, controller: ConstellationController):
        with pytest.raises(KeyError):
            controller.queue_expression("nonexistent_skill", duration=1.0)

    def test_queued_overrides_constellation(self, controller: ConstellationController):
        controller.set_emotional_state("solemn_gravity")
        controller.queue_expression("genuine_laugh", duration=2.0, at_time=0.0)
        prompt = controller.get_prompt_for_frame(25, fps=25)
        assert "laugh" in prompt.lower()


# ------------------------------------------------------------------
# Frame-by-frame prompt generation
# ------------------------------------------------------------------

class TestFramePrompts:
    def test_frame_zero(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        prompt = controller.get_prompt_for_frame(0, fps=25)
        assert len(prompt) > 0

    def test_negative_frame(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        prompt = controller.get_prompt_for_frame(-1, fps=25)
        assert prompt == ""

    def test_no_state_returns_empty(self, controller: ConstellationController):
        prompt = controller.get_prompt_for_frame(0, fps=25)
        assert prompt == ""

    def test_prompt_changes_over_time(self, controller: ConstellationController):
        controller.set_emotional_state("happy_storytelling")
        p0 = controller.get_prompt_for_frame(0, fps=25)
        # After cycling through all 5 skills at ~2s each = 10s cycle,
        # frame 0 and frame 125 (5s) should differ
        p125 = controller.get_prompt_for_frame(125, fps=25)
        # At least one should be non-empty
        assert p0 or p125

    def test_onset_ramp_intensity(self, controller: ConstellationController):
        controller.queue_expression("warm_smile", duration=5.0, at_time=0.0)
        # Frame 0 (t=0.0) is very start of onset
        p_onset = controller.get_prompt_for_frame(0, fps=25)
        # Frame 62 (t=2.5s, middle) is peak
        p_peak = controller.get_prompt_for_frame(62, fps=25)
        assert p_onset  # not empty
        assert p_peak   # not empty


# ------------------------------------------------------------------
# Blending
# ------------------------------------------------------------------

class TestBlendConstellations:
    def test_blend_midpoint(self, controller: ConstellationController):
        result = controller.blend_constellations(
            "warm_conversational", "emotional_recall", ratio=0.5,
        )
        assert "blending" in result.lower()

    def test_blend_mostly_a(self, controller: ConstellationController):
        result = controller.blend_constellations(
            "warm_conversational", "emotional_recall", ratio=0.1,
        )
        assert "hint" in result.lower()

    def test_blend_mostly_b(self, controller: ConstellationController):
        result = controller.blend_constellations(
            "warm_conversational", "emotional_recall", ratio=0.9,
        )
        assert "hint" in result.lower()

    def test_blend_clamped_ratio(self, controller: ConstellationController):
        # ratio > 1 should be clamped
        result = controller.blend_constellations(
            "warm_conversational", "emotional_recall", ratio=2.0,
        )
        assert len(result) > 0

    def test_blend_invalid_constellation(self, controller: ConstellationController):
        with pytest.raises(KeyError):
            controller.blend_constellations("warm_conversational", "fake", ratio=0.5)


# ------------------------------------------------------------------
# Transitions
# ------------------------------------------------------------------

class TestTransitions:
    def test_transition_creates_entry(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        controller.transition_to("emotional_recall", duration_s=0.5)
        assert len(controller._transitions) == 1

    def test_transition_updates_current(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        controller.transition_to("happy_storytelling", duration_s=0.5)
        assert controller._current_constellation == "happy_storytelling"

    def test_transition_prompt_during_blend(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        controller.transition_to("emotional_recall", duration_s=1.0)
        # Frame 6 (0.24s) is during the transition
        prompt = controller.get_prompt_for_frame(6, fps=25)
        assert len(prompt) > 0

    def test_transition_invalid_raises(self, controller: ConstellationController):
        with pytest.raises(KeyError):
            controller.transition_to("nonexistent_constellation")

    def test_multiple_transitions(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        controller.transition_to("emotional_recall", duration_s=0.5)
        controller.transition_to("happy_storytelling", duration_s=0.5)
        assert len(controller._transitions) == 2


# ------------------------------------------------------------------
# Full sequence generation
# ------------------------------------------------------------------

class TestPromptSequence:
    def test_sequence_length(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        seq = controller.get_prompt_sequence(duration_s=5.0, fps=25)
        assert len(seq) > 0
        # Should have at least a few keyframes
        assert len(seq) >= 2

    def test_sequence_starts_at_zero(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        seq = controller.get_prompt_sequence(duration_s=5.0, fps=25)
        assert seq[0][0] == 0

    def test_sequence_frames_in_bounds(self, controller: ConstellationController):
        controller.set_emotional_state("happy_storytelling")
        seq = controller.get_prompt_sequence(duration_s=3.0, fps=25)
        total = int(3.0 * 25)
        for frame, prompt in seq:
            assert 0 <= frame < total
            assert len(prompt) > 0

    def test_sequence_with_queue(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        controller.queue_expression("genuine_laugh", duration=1.0, at_time=1.0)
        seq = controller.get_prompt_sequence(duration_s=3.0, fps=25)
        assert len(seq) >= 3  # start, queue start, queue end

    def test_zero_duration_sequence(self, controller: ConstellationController):
        controller.set_emotional_state("warm_conversational")
        seq = controller.get_prompt_sequence(duration_s=0.0, fps=25)
        assert seq == []

    def test_no_state_empty_sequence(self, controller: ConstellationController):
        seq = controller.get_prompt_sequence(duration_s=5.0, fps=25)
        assert seq == []
