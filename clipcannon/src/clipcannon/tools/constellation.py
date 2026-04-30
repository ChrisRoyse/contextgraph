"""Constellation/expression MCP tool dispatch for ClipCannon.

Handles dispatch for micro-expression constellation control and
clone video generation with behavioral state management.
"""
from __future__ import annotations

import json
import logging
import os
import subprocess
import time
from pathlib import Path

logger = logging.getLogger(__name__)

FLAME_PATH = Path.home() / ".clipcannon" / "models" / "{person}" / "flame_params.npz"
LORA_PATH = Path.home() / ".clipcannon" / "models" / "{person}" / "echov3_lora" / "final"
REF_IMAGE_PATH = Path.home() / ".clipcannon" / "models" / "{person}" / "reference" / "{person}_portrait.jpg"
AUDIO_REF_PATH = Path.home() / ".clipcannon" / "models" / "{person}" / "reference"

# Cache loaded skill libraries per person
_skill_library_cache: dict[str, object] = {}


def _error(code: str, message: str, details: dict | None = None) -> dict:
    return {"error": {"code": code, "message": message, "details": details or {}}}


def _get_skill_library(person: str):
    """Load or return cached SkillLibrary for a person."""
    if person in _skill_library_cache:
        return _skill_library_cache[person]

    from phoenix.video.expression_skills import SkillLibrary

    flame_path = str(FLAME_PATH).format(person=person)
    embeddings_path = str(Path.home() / ".clipcannon" / "models" / person / "embeddings" / "all_embeddings.npz")

    if not Path(flame_path).exists():
        raise FileNotFoundError(f"FLAME params not found for '{person}' at {flame_path}")
    if not Path(embeddings_path).exists():
        raise FileNotFoundError(f"Embeddings not found for '{person}' at {embeddings_path}")

    library = SkillLibrary()
    library.extract_from_training_data(embeddings_path, flame_path)
    _skill_library_cache[person] = library
    return library


async def _handle_list_constellations(arguments: dict) -> dict:
    person = str(arguments.get("person", "santa"))
    try:
        library = _get_skill_library(person)
        constellations = library.list_constellations()
        result = []
        for name in constellations:
            c = library.get_constellation(name)
            # skill_sequence can be strings or ExpressionSkill objects
            skills = [s if isinstance(s, str) else s.name for s in c.skill_sequence]
            result.append({
                "name": name,
                "emotion": c.emotion,
                "description": c.description,
                "skills": skills,
            })
        return {"person": person, "constellations": result, "count": len(result)}
    except Exception as e:
        return _error("CONSTELLATION_ERROR", str(e))


async def _handle_list_skills(arguments: dict) -> dict:
    person = str(arguments.get("person", "santa"))
    try:
        library = _get_skill_library(person)
        skills = library.list_skills()
        result = []
        for name in skills:
            skill = library.get_skill(name)
            prompt = library.skill_to_prompt(name)
            result.append({
                "name": name,
                "prompt": prompt[:100],
                "phases": len(skill.phases) if hasattr(skill, 'phases') else 0,
            })
        return {"person": person, "skills": result, "count": len(result)}
    except Exception as e:
        return _error("SKILL_ERROR", str(e))


async def _handle_generate_video(arguments: dict) -> dict:
    person = str(arguments.get("person", "santa"))
    audio_path = str(arguments.get("audio_path", ""))
    constellation = str(arguments.get("constellation", "warm_conversational"))
    intensity = float(arguments.get("intensity", 0.8))
    seed = int(arguments.get("seed", 42))
    output_path = str(arguments.get("output_path", ""))

    if not audio_path:
        return _error("MISSING_PARAMETER", "audio_path is required")
    if not Path(audio_path).exists():
        return _error("FILE_NOT_FOUND", f"Audio not found: {audio_path}")

    # Check LoRA exists
    lora_path = str(LORA_PATH).format(person=person)
    if not Path(lora_path).exists():
        return _error("MODEL_NOT_FOUND", f"LoRA not found for '{person}' at {lora_path}. Train first.")

    # Check reference image
    ref_path = str(REF_IMAGE_PATH).format(person=person)
    if not Path(ref_path).exists():
        return _error("FILE_NOT_FOUND", f"Reference image not found: {ref_path}")

    # Generate prompt from constellation
    try:
        library = _get_skill_library(person)
        from phoenix.video.constellation_controller import ConstellationController
        controller = ConstellationController(library)
        controller.set_emotional_state(constellation, intensity)
        # Get the initial prompt (the full constellation description)
        prompt = controller.get_prompt_for_frame(0)
    except Exception as e:
        return _error("CONSTELLATION_ERROR", f"Failed to build prompt: {e}")

    # Build full prompt
    full_prompt = (
        f"SANTA {prompt} "
        "Natural subtle head movements, genuine facial expressions. "
        "Clear focused eyes. Professional interview framing. "
        "Wood paneling background. Warm lighting."
    )

    # Auto output path
    if not output_path:
        demos_dir = Path.home() / ".clipcannon" / "models" / person / "demos"
        demos_dir.mkdir(parents=True, exist_ok=True)
        output_path = str(demos_dir / f"{person}_{constellation}_{seed}.mp4")

    # Build inference command
    echomimic_dir = "/home/cabdru/echomimic_v3"
    cmd = [
        "python", f"{echomimic_dir}/infer_flash.py",
        "--image_path", ref_path,
        "--audio_path", audio_path,
        "--prompt", full_prompt,
        "--negative_prompt", "tears, crying, watery eyes, blurry, distorted, deformed",
        "--num_inference_steps", "8",
        "--config_path", f"{echomimic_dir}/config/config.yaml",
        "--model_name", f"{echomimic_dir}/pretrained_weights/Wan2.1-Fun-V1.1-1.3B-InP",
        "--ckpt_idx", "50000",
        "--transformer_path", f"{echomimic_dir}/pretrained_weights/echomimicv3-flash-pro/diffusion_pytorch_model.safetensors",
        "--lora_path", lora_path,
        "--save_path", str(Path(output_path).parent),
        "--wav2vec_model_dir", f"{echomimic_dir}/pretrained_weights/chinese-wav2vec2-base",
        "--sampler_name", "Flow_DPM++",
        "--video_length", "249",
        "--guidance_scale", "7.0",
        "--audio_guidance_scale", "3.5",
        "--seed", str(seed),
        "--enable_teacache",
        "--teacache_threshold", "0.1",
        "--num_skip_start_steps", "5",
        "--weight_dtype", "bfloat16",
        "--sample_size", "480", "480",
        "--fps", "25",
        "--shift", "5.0",
    ]

    env = os.environ.copy()
    env["FFMPEG_PATH"] = "/usr/bin"
    env["PYTORCH_CUDA_ALLOC_CONF"] = ""
    env["PYTHONPATH"] = echomimic_dir

    logger.info("Generating %s video: constellation=%s, seed=%d", person, constellation, seed)
    t0 = time.time()

    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=600,
                                cwd=echomimic_dir, env=env)
        elapsed = time.time() - t0

        # Find the output file (EchoMimicV3 names it based on image name)
        ref_stem = Path(ref_path).stem
        expected_output = Path(output_path).parent / f"{ref_stem}_output.mp4"

        if expected_output.exists():
            # Rename to desired output path
            if str(expected_output) != output_path:
                expected_output.rename(output_path)

            return {
                "output_path": output_path,
                "constellation": constellation,
                "intensity": intensity,
                "duration_s": elapsed,
                "person": person,
                "seed": seed,
            }
        else:
            return _error("GENERATION_FAILED",
                          f"Output not found at {expected_output}",
                          {"stderr": result.stderr[-500:] if result.stderr else ""})
    except subprocess.TimeoutExpired:
        return _error("TIMEOUT", "Video generation timed out after 600s")
    except Exception as e:
        return _error("GENERATION_ERROR", str(e))


async def _handle_expression_sequence(arguments: dict) -> dict:
    person = str(arguments.get("person", "santa"))
    constellation = str(arguments.get("constellation", ""))
    duration_s = float(arguments.get("duration_s", 5.0))
    intensity = float(arguments.get("intensity", 0.8))
    fps = int(arguments.get("fps", 25))

    if not constellation:
        return _error("MISSING_PARAMETER", "constellation is required")

    try:
        library = _get_skill_library(person)
        from phoenix.video.constellation_controller import ConstellationController
        controller = ConstellationController(library)
        controller.set_emotional_state(constellation, intensity)
        sequence = controller.get_prompt_sequence(duration_s, fps)

        # Sample every 25 frames (1 per second) for display
        sampled = [(idx, prompt) for idx, prompt in sequence if idx % fps == 0]

        return {
            "constellation": constellation,
            "intensity": intensity,
            "total_frames": len(sequence),
            "duration_s": duration_s,
            "sample_prompts": [{"frame": idx, "time_s": idx/fps, "prompt": p} for idx, p in sampled],
        }
    except Exception as e:
        return _error("SEQUENCE_ERROR", str(e))


async def dispatch_constellation_tool(name: str, arguments: dict) -> dict:
    """Route constellation tool calls to handlers."""
    handlers = {
        "clipcannon_list_constellations": _handle_list_constellations,
        "clipcannon_list_skills": _handle_list_skills,
        "clipcannon_clone_video": _handle_generate_video,
        "clipcannon_expression_sequence": _handle_expression_sequence,
    }
    handler = handlers.get(name)
    if handler is None:
        return _error("UNKNOWN_TOOL", f"Unknown constellation tool: {name}")
    return await handler(arguments)
