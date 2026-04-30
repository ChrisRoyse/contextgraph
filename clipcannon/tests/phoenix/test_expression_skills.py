"""Tests for the Micro-Expression Constellation System - SkillLibrary.

Uses REAL FLAME data -- no mocks.
"""

from __future__ import annotations

import sys

import numpy as np
import pytest

sys.path.insert(0, "/home/cabdru/clipcannon/src")

from phoenix.video.expression_skills import (
    CONSTELLATIONS,
    FLAME_TO_AU,
    SKILL_PROMPTS,
    ActionUnit,
    BehavioralConstellation,
    ExpressionSkill,
    MicroExpressionGroup,
    SkillLibrary,
)

FLAME_PATH = "/home/cabdru/.clipcannon/models/santa/flame_params.npz"
EMBEDDINGS_PATH = "/home/cabdru/.clipcannon/models/santa/embeddings/all_embeddings.npz"


@pytest.fixture(scope="module")
def library() -> SkillLibrary:
    """Build the skill library from real training data (once per module)."""
    lib = SkillLibrary()
    lib.extract_from_training_data(EMBEDDINGS_PATH, FLAME_PATH)
    return lib


# ------------------------------------------------------------------
# Data loading / extraction
# ------------------------------------------------------------------

class TestDataLoading:
    def test_flame_params_exist(self):
        data = np.load(FLAME_PATH)
        assert "expression" in data
        assert data["expression"].shape == (3477, 100)

    def test_embeddings_exist(self):
        data = np.load(EMBEDDINGS_PATH)
        assert "flame_exp" in data

    def test_missing_flame_raises(self):
        lib = SkillLibrary()
        with pytest.raises(FileNotFoundError):
            lib.extract_from_training_data(EMBEDDINGS_PATH, "/nonexistent/file.npz")

    def test_missing_embeddings_raises(self):
        lib = SkillLibrary()
        with pytest.raises(FileNotFoundError):
            lib.extract_from_training_data("/nonexistent/file.npz", FLAME_PATH)


class TestGroupExtraction:
    def test_group_count(self, library: SkillLibrary):
        groups = library.list_groups()
        assert len(groups) == 40, f"Expected 40 groups, got {len(groups)}"

    def test_groups_have_action_units(self, library: SkillLibrary):
        for gname in library.list_groups():
            group = library._groups[gname]
            assert len(group.action_units) > 0, f"Group {gname} has no AUs"

    def test_groups_have_descriptions(self, library: SkillLibrary):
        for gname in library.list_groups():
            group = library._groups[gname]
            assert group.description, f"Group {gname} has empty description"

    def test_groups_have_centroids(self, library: SkillLibrary):
        for gname in library.list_groups():
            group = library._groups[gname]
            assert group.centroid is not None
            assert group.centroid.shape == (100,)


# ------------------------------------------------------------------
# Skills
# ------------------------------------------------------------------

class TestSkillExtraction:
    def test_skill_count(self, library: SkillLibrary):
        skills = library.list_skills()
        assert len(skills) >= 25, f"Expected >=25 skills, got {len(skills)}"

    def test_all_prompt_skills_registered(self, library: SkillLibrary):
        for sname in SKILL_PROMPTS:
            assert sname in library.list_skills(), f"Skill {sname} not registered"

    def test_skill_has_phases(self, library: SkillLibrary):
        for sname in library.list_skills():
            sk = library.get_skill(sname)
            assert "onset" in sk.phases
            assert "peak" in sk.phases
            assert "offset" in sk.phases

    def test_skill_phases_have_duration(self, library: SkillLibrary):
        for sname in library.list_skills():
            sk = library.get_skill(sname)
            for phase_name, phase in sk.phases.items():
                assert phase.duration_s > 0, f"{sname}.{phase_name} has zero duration"

    def test_unknown_skill_raises(self, library: SkillLibrary):
        with pytest.raises(KeyError, match="Unknown skill"):
            library.get_skill("nonexistent_skill_xyz")


# ------------------------------------------------------------------
# Prompt generation
# ------------------------------------------------------------------

class TestPromptGeneration:
    def test_skill_to_prompt_default(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile")
        assert len(prompt) > 0
        assert "smile" in prompt.lower()

    def test_skill_to_prompt_low_intensity(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile", intensity=0.1)
        assert "very subtly" in prompt

    def test_skill_to_prompt_medium_intensity(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile", intensity=0.4)
        assert "slightly" in prompt

    def test_skill_to_prompt_high_intensity(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile", intensity=0.95)
        assert "intensely" in prompt

    def test_every_skill_has_prompt(self, library: SkillLibrary):
        for sname in library.list_skills():
            prompt = library.skill_to_prompt(sname)
            assert isinstance(prompt, str)
            assert len(prompt) > 5, f"Skill {sname} has trivially short prompt"


# ------------------------------------------------------------------
# Constellations
# ------------------------------------------------------------------

class TestConstellations:
    def test_constellation_count(self, library: SkillLibrary):
        consts = library.list_constellations()
        assert len(consts) == len(CONSTELLATIONS)

    def test_all_constellations_registered(self, library: SkillLibrary):
        for cname in CONSTELLATIONS:
            assert cname in library.list_constellations()

    def test_constellation_has_skills(self, library: SkillLibrary):
        for cname in library.list_constellations():
            const = library.get_constellation(cname)
            assert len(const.skill_sequence) > 0

    def test_constellation_skills_exist(self, library: SkillLibrary):
        for cname in library.list_constellations():
            const = library.get_constellation(cname)
            for sname in const.skill_sequence:
                assert sname in library.list_skills(), (
                    f"Constellation {cname} references unknown skill {sname}"
                )

    def test_constellation_has_emotion(self, library: SkillLibrary):
        for cname in library.list_constellations():
            const = library.get_constellation(cname)
            assert len(const.emotion) > 0

    def test_unknown_constellation_raises(self, library: SkillLibrary):
        with pytest.raises(KeyError, match="Unknown constellation"):
            library.get_constellation("nonexistent_constellation")

    def test_constellation_prompt_sequence_duration(self, library: SkillLibrary):
        seq = library.constellation_to_prompt_sequence(
            "warm_conversational", duration_s=5.0, fps=25,
        )
        assert len(seq) > 0
        # All frame indices should be within [0, 125)
        for frame, prompt in seq:
            assert 0 <= frame < 125
            assert len(prompt) > 0

    def test_constellation_prompt_sequence_covers_time(self, library: SkillLibrary):
        seq = library.constellation_to_prompt_sequence(
            "happy_storytelling", duration_s=10.0, fps=25,
        )
        frames = [f for f, _ in seq]
        assert frames[0] == 0, "Sequence should start at frame 0"
        assert frames[-1] >= 200, "Sequence should cover most of 10s"


# ------------------------------------------------------------------
# Edge cases
# ------------------------------------------------------------------

class TestEdgeCases:
    def test_zero_intensity_prompt(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile", intensity=0.0)
        assert "very subtly" in prompt

    def test_negative_intensity_treated_as_low(self, library: SkillLibrary):
        prompt = library.skill_to_prompt("warm_smile", intensity=-0.5)
        assert "very subtly" in prompt

    def test_short_duration_sequence(self, library: SkillLibrary):
        seq = library.constellation_to_prompt_sequence(
            "warm_conversational", duration_s=0.1, fps=25,
        )
        # Should produce at least one prompt (frame 0)
        assert len(seq) >= 1

    def test_zero_duration_sequence(self, library: SkillLibrary):
        seq = library.constellation_to_prompt_sequence(
            "warm_conversational", duration_s=0.0, fps=25,
        )
        assert seq == []

    def test_flame_to_au_mapping_completeness(self):
        for idx, au in FLAME_TO_AU.items():
            assert isinstance(au, ActionUnit)
            assert au.name
            assert au.au_id
            assert au.region in ("brow", "eye", "mouth", "jaw", "nose", "cheek")
