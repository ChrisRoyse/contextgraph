"""Micro-Expression Constellation System - Skill Library.

Hierarchical behavioral control: Action Units -> Groups -> Skills -> Constellations.
Extracts expression vocabulary from real FLAME training data via clustering.
"""

from __future__ import annotations

import pathlib
from dataclasses import dataclass, field

import numpy as np
from sklearn.cluster import KMeans


# ---------------------------------------------------------------------------
# FLAME coefficient -> FACS Action Unit mapping
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ActionUnit:
    """A single FACS Action Unit derived from FLAME coefficients."""
    name: str
    au_id: str           # e.g. "AU26"
    region: str          # brow / eye / mouth / jaw / nose / cheek
    intensity_range: tuple[float, float] = (0.0, 1.0)


# Map FLAME expression indices to FACS AUs.
FLAME_TO_AU: dict[int, ActionUnit] = {
    0:  ActionUnit("jaw_open",        "AU26", "jaw"),
    1:  ActionUnit("lip_spread",      "AU12", "mouth"),
    2:  ActionUnit("lip_funneler",    "AU22", "mouth"),
    3:  ActionUnit("lip_pucker",      "AU18", "mouth"),
    4:  ActionUnit("lip_tightener",   "AU23", "mouth"),
    5:  ActionUnit("brow_raise",      "AU1+2", "brow"),
    6:  ActionUnit("eye_squint",      "AU6+7", "eye"),
    7:  ActionUnit("mouth_stretch",   "AU27", "mouth"),
    8:  ActionUnit("lip_press",       "AU24", "mouth"),
    9:  ActionUnit("lip_corner_pull", "AU12b", "mouth"),
    10: ActionUnit("cheek_raise",     "AU6",  "cheek"),
    15: ActionUnit("dimpler",         "AU14", "cheek"),
    22: ActionUnit("lip_suck",        "AU28", "mouth"),
    24: ActionUnit("nose_wrinkle",    "AU9",  "nose"),
    31: ActionUnit("chin_raise",      "AU17", "jaw"),
    37: ActionUnit("brow_lower",      "AU4",  "brow"),
    42: ActionUnit("eye_widen",       "AU5",  "eye"),
}


@dataclass
class MicroExpressionGroup:
    """Level 1: Atomic micro-expression group (a few AUs firing together)."""
    name: str
    action_units: list[ActionUnit]
    description: str
    centroid: np.ndarray | None = field(default=None, repr=False)


@dataclass
class ExpressionPhase:
    """One phase of a skill (onset / peak / offset)."""
    groups: list[str]       # group names active in this phase
    duration_s: float       # seconds
    intensity: float = 1.0  # 0-1 multiplier


@dataclass
class ExpressionSkill:
    """Level 2: A named behavioral expression with temporal phases."""
    name: str
    phases: dict[str, ExpressionPhase]   # onset / peak / offset
    prompt: str                           # natural language prompt


@dataclass
class BehavioralConstellation:
    """Level 3: An emotional state expressed as a cycling skill sequence."""
    name: str
    skill_sequence: list[str]
    cycle: bool
    emotion: str
    intensity_range: tuple[float, float] = (0.0, 1.0)
    description: str = ""


# ---------------------------------------------------------------------------
# Hardcoded skill prompts  (what EchoMimicV3 sees)
# ---------------------------------------------------------------------------

SKILL_PROMPTS: dict[str, str] = {
    "genuine_laugh":     "laughing genuinely with eyes crinkled, mouth wide open, head tilting back slightly",
    "warm_smile":        "warm gentle smile with eyes soft, slight head tilt, relaxed expression",
    "gentle_nod":        "nodding gently with soft approving expression",
    "eye_contact":       "direct warm eye contact with relaxed brow",
    "slight_gesture":    "subtle hand gesture while speaking naturally",
    "thoughtful_pause":  "looking down thoughtfully, slight frown, lips pressed together",
    "gaze_down":         "gaze directed downward, introspective expression",
    "voice_waver":       "expression showing emotional strain, lips slightly trembling",
    "brave_smile":       "smiling through emotion, eyes slightly wet, chin raised",
    "deep_breath":       "taking a deep breath, chest rising, composing expression",
    "eyes_wide":         "eyes widened with excitement or surprise, brows raised",
    "animated_gesture":  "animated expressive gesture with enthusiastic face",
    "emphatic_nod":      "emphatic confident nod with conviction in eyes",
    "brow_furrow":       "brow furrowed with concern, eyes soft and empathetic",
    "soft_eyes":         "eyes soft and gentle, showing deep empathy",
    "slow_nod":          "slow deliberate nod showing understanding",
    "lip_press":         "lips pressed together with restrained emotion",
    "head_tilt":         "head tilted slightly to one side, attentive expression",
    "gaze_up":           "gaze directed upward, searching for thought or memory",
    "slight_frown":      "slight frown of concentration, not displeasure",
    "pause":             "momentary stillness, face neutral and contemplative",
    "slow_blink":        "slow deliberate blink, calm and reflective",
    "chin_rest":         "chin resting on hand, deep in thought",
    "rapid_speech":      "animated expression with rapid engaged speaking",
    "lean_forward":      "leaning forward with engaged eager expression",
    "hand_gesture":      "expressive hand gesture emphasising a point",
    "steady_gaze":       "steady unwavering gaze, serious and present",
    "slow_speech":       "measured expression, speaking slowly and deliberately",
    "minimal_movement":  "very still, calm, serious facial expression",
    "slight_nod":        "barely perceptible nod, quiet affirmation",
    "eye_twinkle":       "eyes bright with subdued amusement, slight crinkle",
    "half_smile":        "asymmetric half smile, knowing and warm",
    "slight_pause":      "brief pause, hint of amusement in expression",
    "warm_chuckle":      "soft chuckle with gentle smile, eyes bright",
}


# ---------------------------------------------------------------------------
# Hardcoded constellation definitions
# ---------------------------------------------------------------------------

CONSTELLATIONS: dict[str, dict] = {
    "warm_conversational": {
        "emotion": "joy+trust",
        "skills": ["warm_smile", "gentle_nod", "eye_contact", "slight_gesture"],
        "cycle": True,
        "description": "Default friendly interview state",
    },
    "emotional_recall": {
        "emotion": "sadness+trust",
        "skills": ["thoughtful_pause", "gaze_down", "voice_waver", "brave_smile", "deep_breath"],
        "cycle": True,
        "description": "Remembering emotional events",
    },
    "happy_storytelling": {
        "emotion": "joy+anticipation",
        "skills": ["genuine_laugh", "eyes_wide", "animated_gesture", "warm_smile", "emphatic_nod"],
        "cycle": True,
        "description": "Telling a happy or funny story",
    },
    "empathetic_concern": {
        "emotion": "sadness+trust",
        "skills": ["brow_furrow", "soft_eyes", "slow_nod", "lip_press", "head_tilt"],
        "cycle": True,
        "description": "Showing care about someone else's pain",
    },
    "thoughtful_reflection": {
        "emotion": "anticipation+trust",
        "skills": ["gaze_up", "slight_frown", "pause", "slow_blink", "chin_rest"],
        "cycle": True,
        "description": "Deep thinking about a topic",
    },
    "excited_sharing": {
        "emotion": "joy+surprise",
        "skills": ["eyes_wide", "rapid_speech", "lean_forward", "hand_gesture", "genuine_laugh"],
        "cycle": True,
        "description": "Excited to tell something",
    },
    "solemn_gravity": {
        "emotion": "trust+sadness",
        "skills": ["steady_gaze", "slow_speech", "minimal_movement", "slight_nod", "deep_breath"],
        "cycle": True,
        "description": "Speaking about something serious",
    },
    "gentle_humor": {
        "emotion": "joy+surprise",
        "skills": ["eye_twinkle", "half_smile", "slight_pause", "warm_chuckle"],
        "cycle": True,
        "description": "Mild amusement, not full laugh",
    },
}


# ---------------------------------------------------------------------------
# Skill Library
# ---------------------------------------------------------------------------

class SkillLibrary:
    """Extracts and manages the micro-expression skill vocabulary."""

    def __init__(self) -> None:
        self._groups: dict[str, MicroExpressionGroup] = {}
        self._skills: dict[str, ExpressionSkill] = {}
        self._constellations: dict[str, BehavioralConstellation] = {}
        self._cluster_labels: np.ndarray | None = None
        self._centroids: np.ndarray | None = None

    # ------------------------------------------------------------------
    # Core extraction
    # ------------------------------------------------------------------

    def extract_from_training_data(
        self,
        embeddings_path: str | pathlib.Path,
        flame_params_path: str | pathlib.Path,
        n_groups: int = 40,
        n_skills: int = 25,
    ) -> None:
        """Build the full skill hierarchy from real training data.

        Args:
            embeddings_path: Path to all_embeddings.npz
            flame_params_path: Path to flame_params.npz
            n_groups: Number of micro-expression group clusters.
            n_skills: Number of skill clusters.
        """
        emb_path = pathlib.Path(embeddings_path)
        fp_path = pathlib.Path(flame_params_path)
        if not emb_path.exists():
            raise FileNotFoundError(f"Embeddings not found: {emb_path}")
        if not fp_path.exists():
            raise FileNotFoundError(f"FLAME params not found: {fp_path}")

        flame = np.load(str(fp_path))
        expression = flame["expression"]   # (N, 100)

        # --- Level 1: cluster into micro-expression groups ---
        km_groups = KMeans(n_clusters=n_groups, random_state=42, n_init=10)
        self._cluster_labels = km_groups.fit_predict(expression)
        self._centroids = km_groups.cluster_centers_

        neutral = expression.mean(axis=0)
        used_names: set[str] = set()

        for cid in range(n_groups):
            centroid = self._centroids[cid]
            deviation = centroid - neutral
            active_aus = self._identify_active_aus(deviation)
            base_name = self._auto_name_from_aus(active_aus, cid)
            name = base_name
            suffix = 2
            while name in used_names:
                name = f"{base_name}_{suffix}"
                suffix += 1
            used_names.add(name)
            group = MicroExpressionGroup(
                name=name,
                action_units=active_aus,
                description=self._describe_aus(active_aus),
                centroid=centroid,
            )
            self._groups[name] = group

        # --- Level 2: build skills from group sequences ---
        self._build_skills(expression, n_skills)

        # --- Level 3: register constellations ---
        for cname, cdef in CONSTELLATIONS.items():
            self._constellations[cname] = BehavioralConstellation(
                name=cname,
                skill_sequence=cdef["skills"],
                cycle=cdef["cycle"],
                emotion=cdef["emotion"],
                description=cdef.get("description", ""),
            )

    # ------------------------------------------------------------------
    # Public accessors
    # ------------------------------------------------------------------

    def get_skill(self, name: str) -> ExpressionSkill:
        if name not in self._skills:
            raise KeyError(f"Unknown skill: {name!r}. Available: {self.list_skills()}")
        return self._skills[name]

    def get_constellation(self, name: str) -> BehavioralConstellation:
        if name not in self._constellations:
            raise KeyError(
                f"Unknown constellation: {name!r}. "
                f"Available: {self.list_constellations()}"
            )
        return self._constellations[name]

    def list_skills(self) -> list[str]:
        return sorted(self._skills.keys())

    def list_constellations(self) -> list[str]:
        return sorted(self._constellations.keys())

    def list_groups(self) -> list[str]:
        return sorted(self._groups.keys())

    # ------------------------------------------------------------------
    # Prompt generation
    # ------------------------------------------------------------------

    def skill_to_prompt(self, skill_name: str, intensity: float = 1.0) -> str:
        """Convert a skill to a natural-language prompt fragment."""
        sk = self.get_skill(skill_name)
        base = sk.prompt
        if intensity < 0.3:
            return f"very subtly {base}"
        if intensity < 0.6:
            return f"slightly {base}"
        if intensity > 0.9:
            return f"intensely {base}"
        return base

    def constellation_to_prompt_sequence(
        self,
        constellation_name: str,
        duration_s: float,
        fps: int = 25,
    ) -> list[tuple[int, str]]:
        """Generate (frame_idx, prompt_str) for every key-frame in a constellation."""
        const = self.get_constellation(constellation_name)
        skills = const.skill_sequence
        if not skills:
            return []

        total_frames = int(duration_s * fps)
        result: list[tuple[int, str]] = []

        # Distribute skills evenly across duration, cycling if needed
        skill_dur_s = duration_s / len(skills)
        t = 0.0
        idx = 0
        while t < duration_s:
            skill_name = skills[idx % len(skills)]
            frame = int(t * fps)
            if frame < total_frames:
                prompt = self.skill_to_prompt(skill_name)
                result.append((frame, prompt))
            # Also emit the peak (midpoint of skill window)
            peak_frame = int((t + skill_dur_s * 0.5) * fps)
            if peak_frame < total_frames and peak_frame != frame:
                peak_prompt = self.skill_to_prompt(skill_name, intensity=1.0)
                result.append((peak_frame, peak_prompt))

            t += skill_dur_s
            idx += 1
            if not const.cycle and idx >= len(skills):
                break

        result.sort(key=lambda x: x[0])
        return result

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _identify_active_aus(deviation: np.ndarray, threshold: float = 0.5) -> list[ActionUnit]:
        """Find which AUs are active based on deviation from neutral."""
        active: list[ActionUnit] = []
        for idx, au in FLAME_TO_AU.items():
            if idx < len(deviation) and abs(deviation[idx]) > threshold:
                active.append(au)
        # If nothing passes threshold, take top-3 by magnitude
        if not active:
            mapped_indices = [i for i in FLAME_TO_AU if i < len(deviation)]
            ranked = sorted(mapped_indices, key=lambda i: abs(deviation[i]), reverse=True)
            active = [FLAME_TO_AU[i] for i in ranked[:3]]
        return active

    @staticmethod
    def _auto_name_from_aus(aus: list[ActionUnit], cluster_id: int) -> str:
        """Generate a human-readable name from active AUs."""
        if not aus:
            return f"expression_{cluster_id:02d}"
        parts: list[str] = []
        for au in aus[:3]:
            parts.append(au.name)
        return "_".join(parts)

    @staticmethod
    def _describe_aus(aus: list[ActionUnit]) -> str:
        region_groups: dict[str, list[str]] = {}
        for au in aus:
            region_groups.setdefault(au.region, []).append(au.name)
        parts = []
        for region, names in sorted(region_groups.items()):
            parts.append(f"{region}: {', '.join(names)}")
        return "; ".join(parts) if parts else "neutral"

    def _build_skills(self, expression: np.ndarray, n_skills: int) -> None:
        """Build Level-2 skills from temporal clustering + hardcoded prompts."""
        # Temporal sequence clustering: 10-frame windows of cluster labels
        seq_len = 10
        if self._cluster_labels is None or len(self._cluster_labels) < seq_len:
            return

        labels = self._cluster_labels
        n_frames = len(labels)

        # Build sequence feature vectors (one-hot histogram over group IDs)
        n_groups = int(labels.max()) + 1
        sequences: list[np.ndarray] = []
        for i in range(n_frames - seq_len):
            window = labels[i : i + seq_len]
            hist = np.bincount(window, minlength=n_groups).astype(np.float32)
            hist /= hist.sum() + 1e-8
            sequences.append(hist)

        if len(sequences) < n_skills:
            n_skills = max(1, len(sequences))

        seq_arr = np.stack(sequences)
        km_skills = KMeans(n_clusters=n_skills, random_state=42, n_init=10)
        km_skills.fit_predict(seq_arr)

        # Register every skill from SKILL_PROMPTS as a proper ExpressionSkill
        group_names = list(self._groups.keys())
        for sname, prompt_text in SKILL_PROMPTS.items():
            # Assign onset/peak/offset groups from the first available groups
            onset_g = group_names[:1] if group_names else []
            peak_g = group_names[1:2] if len(group_names) > 1 else onset_g
            offset_g = group_names[:1] if group_names else []

            self._skills[sname] = ExpressionSkill(
                name=sname,
                phases={
                    "onset":  ExpressionPhase(groups=onset_g, duration_s=0.2),
                    "peak":   ExpressionPhase(groups=peak_g, duration_s=0.5, intensity=1.0),
                    "offset": ExpressionPhase(groups=offset_g, duration_s=0.3, intensity=0.3),
                },
                prompt=prompt_text,
            )
