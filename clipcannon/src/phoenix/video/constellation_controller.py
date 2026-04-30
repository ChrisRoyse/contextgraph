"""Constellation Controller - runtime expression controller for the reasoning layer.

The Gemma 4 reasoning layer sets emotional states and queues specific expressions.
This controller translates those high-level commands into per-frame natural language
prompts for EchoMimicV3 video generation.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from phoenix.video.expression_skills import SkillLibrary


@dataclass
class _QueuedExpression:
    """An expression skill queued at a specific time."""
    skill: str
    start_time: float      # seconds
    duration: float        # seconds
    intensity: float = 1.0


@dataclass
class _Transition:
    """A smooth transition between two constellations."""
    from_constellation: str
    to_constellation: str
    start_time: float
    duration: float


class ConstellationController:
    """Runtime controller that translates emotional intent into frame prompts.

    Usage:
        controller = ConstellationController(skill_library)
        controller.set_emotional_state("happy_storytelling", intensity=0.8)
        controller.queue_expression("genuine_laugh", duration=1.0, at_time=2.5)
        prompt = controller.get_prompt_for_frame(frame_idx=50, fps=25)
    """

    def __init__(self, skill_library: SkillLibrary) -> None:
        self._library = skill_library
        self._current_constellation: str | None = None
        self._intensity: float = 1.0
        self._queue: list[_QueuedExpression] = []
        self._transitions: list[_Transition] = []
        self._constellation_start_time: float = 0.0

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def set_emotional_state(
        self,
        constellation: str,
        intensity: float = 1.0,
    ) -> None:
        """Set the current behavioral constellation.

        Args:
            constellation: Name of constellation (e.g. "happy_storytelling").
            intensity: 0.0 (subtle) to 1.0 (full expression).
        """
        # Validate the constellation exists
        self._library.get_constellation(constellation)
        self._current_constellation = constellation
        self._intensity = max(0.0, min(1.0, intensity))
        self._constellation_start_time = 0.0

    def queue_expression(
        self,
        skill: str,
        duration: float,
        at_time: float | None = None,
    ) -> None:
        """Queue a specific expression skill.

        Args:
            skill: Skill name (e.g. "genuine_laugh").
            duration: How long to hold this expression in seconds.
            at_time: When to start (seconds). None = immediately after last queued.
        """
        self._library.get_skill(skill)

        if at_time is None:
            if self._queue:
                last = self._queue[-1]
                at_time = last.start_time + last.duration
            else:
                at_time = 0.0

        self._queue.append(_QueuedExpression(
            skill=skill,
            start_time=at_time,
            duration=duration,
        ))
        self._queue.sort(key=lambda q: q.start_time)

    def get_prompt_for_frame(self, frame_idx: int, fps: int = 25) -> str:
        """Get the natural language prompt for a specific frame.

        Checks the queue first, then falls back to the current constellation.
        Transitions are blended when active.
        """
        if frame_idx < 0:
            return ""

        t = frame_idx / fps

        # 1. Check if a queued expression covers this time
        queued_prompt = self._prompt_from_queue(t)
        if queued_prompt:
            return queued_prompt

        # 2. Check for active transitions
        transition_prompt = self._prompt_from_transition(t)
        if transition_prompt:
            return transition_prompt

        # 3. Fall back to current constellation
        if self._current_constellation is None:
            return ""

        return self._prompt_from_constellation(t)

    def get_prompt_sequence(
        self,
        duration_s: float,
        fps: int = 25,
    ) -> list[tuple[int, str]]:
        """Get (frame_idx, prompt_str) for the full duration.

        Returns prompts at every key-frame boundary (skill transitions within
        constellations, queued expression edges, transition blend points).
        """
        total_frames = int(duration_s * fps)
        if total_frames <= 0:
            return []

        # Collect key-frame times from all sources
        key_times: set[float] = {0.0}

        # Queue boundaries
        for qe in self._queue:
            key_times.add(qe.start_time)
            key_times.add(qe.start_time + qe.duration)

        # Transition boundaries
        for tr in self._transitions:
            key_times.add(tr.start_time)
            key_times.add(tr.start_time + tr.duration)

        # Constellation skill boundaries
        if self._current_constellation:
            const = self._library.get_constellation(self._current_constellation)
            n_skills = len(const.skill_sequence) or 1
            skill_dur = duration_s / n_skills
            t = 0.0
            while t < duration_s:
                key_times.add(t)
                t += skill_dur

        # Convert to frame indices and generate prompts
        result: list[tuple[int, str]] = []
        seen_frames: set[int] = set()
        for t in sorted(key_times):
            frame = int(t * fps)
            if frame >= total_frames or frame in seen_frames:
                continue
            seen_frames.add(frame)
            prompt = self.get_prompt_for_frame(frame, fps)
            if prompt:
                result.append((frame, prompt))

        return result

    def blend_constellations(
        self,
        a: str,
        b: str,
        ratio: float,
    ) -> str:
        """Blend two constellations and return a composite prompt.

        Args:
            a: First constellation name.
            b: Second constellation name.
            ratio: 0.0 = pure a, 1.0 = pure b.

        Returns:
            A blended natural language prompt.
        """
        const_a = self._library.get_constellation(a)
        const_b = self._library.get_constellation(b)

        ratio = max(0.0, min(1.0, ratio))

        # Pick skills weighted by ratio
        skills_a = const_a.skill_sequence
        skills_b = const_b.skill_sequence

        # Take first skill from each for the blend
        prompt_a = self._library.skill_to_prompt(
            skills_a[0], intensity=1.0 - ratio
        ) if skills_a else ""
        prompt_b = self._library.skill_to_prompt(
            skills_b[0], intensity=ratio
        ) if skills_b else ""

        if ratio < 0.3:
            return f"{prompt_a}, with a hint of {prompt_b}"
        if ratio > 0.7:
            return f"{prompt_b}, with a hint of {prompt_a}"
        return f"blending {prompt_a} and {prompt_b}"

    def transition_to(
        self,
        new_constellation: str,
        duration_s: float = 0.5,
    ) -> None:
        """Schedule a smooth transition to a new constellation.

        The transition starts at the current time (or 0 if nothing is set).
        """
        self._library.get_constellation(new_constellation)

        start = 0.0
        if self._transitions:
            last = self._transitions[-1]
            start = last.start_time + last.duration

        old = self._current_constellation or new_constellation
        self._transitions.append(_Transition(
            from_constellation=old,
            to_constellation=new_constellation,
            start_time=start,
            duration=duration_s,
        ))
        self._current_constellation = new_constellation

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _prompt_from_queue(self, t: float) -> str | None:
        """Return prompt if a queued expression covers time t."""
        for qe in self._queue:
            if qe.start_time <= t < qe.start_time + qe.duration:
                # Determine phase within the expression
                elapsed = t - qe.start_time
                frac = elapsed / qe.duration if qe.duration > 0 else 1.0
                if frac < 0.2:
                    intensity = frac / 0.2   # onset ramp
                elif frac > 0.7:
                    intensity = (1.0 - frac) / 0.3  # offset ramp
                else:
                    intensity = 1.0  # peak
                return self._library.skill_to_prompt(qe.skill, intensity)
        return None

    def _prompt_from_transition(self, t: float) -> str | None:
        """Return blended prompt during a transition."""
        for tr in self._transitions:
            if tr.start_time <= t < tr.start_time + tr.duration:
                elapsed = t - tr.start_time
                ratio = elapsed / tr.duration if tr.duration > 0 else 1.0
                return self.blend_constellations(
                    tr.from_constellation, tr.to_constellation, ratio
                )
        return None

    def _prompt_from_constellation(self, t: float) -> str:
        """Return prompt from the current constellation at time t."""
        if not self._current_constellation:
            return ""
        const = self._library.get_constellation(self._current_constellation)
        skills = const.skill_sequence
        if not skills:
            return ""

        # Each skill gets ~2 seconds in the cycle
        skill_dur = 2.0
        cycle_len = len(skills) * skill_dur
        t_in_cycle = t % cycle_len if const.cycle else min(t, cycle_len - 0.001)
        skill_idx = int(t_in_cycle / skill_dur) % len(skills)

        return self._library.skill_to_prompt(skills[skill_idx], self._intensity)
