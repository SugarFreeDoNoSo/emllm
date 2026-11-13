"""Perfil nativo finito: límites lógicos Rust y supervisión externa separados."""

import json
import math
from pathlib import Path
import sys

DEFAULT_PROFILE = Path(__file__).with_name("native-profile.json")
LIMITS = {"package_bytes", "tensors", "base_nodes", "template_instructions", "ports",
          "roots", "closed_nodes", "reachable_nodes", "numeric_slots"}
PHASES = {"load", "base", "template", "encode", "compose", "compile", "plan",
          "evaluate-original", "evaluate-planned", "compare", "select", "emit"}


class ProfileError(ValueError):
    """Perfil incompleto, desconocido o con límites no representables."""


def validate_profile(profile):
    if not isinstance(profile, dict) or set(profile) != {"schema_version", "name", "limits", "supervision"}:
        raise ProfileError("profile fields")
    if type(profile["schema_version"]) is not int or profile["schema_version"] != 1:
        raise ProfileError("schema_version")
    if not isinstance(profile["name"], str) or not profile["name"].strip():
        raise ProfileError("name")
    limits = profile["limits"]
    if not isinstance(limits, dict) or set(limits) != LIMITS:
        raise ProfileError("limits fields")
    for name, value in limits.items():
        if type(value) is not int or not 0 <= value <= sys.maxsize:
            raise ProfileError(f"invalid structural limit: {name}")
    supervision = profile["supervision"]
    if not isinstance(supervision, dict) or set(supervision) != {"max_rss_bytes", "phase_timeout_seconds"}:
        raise ProfileError("supervision fields")
    rss = supervision["max_rss_bytes"]
    if type(rss) is not int or not 0 < rss <= sys.maxsize:
        raise ProfileError("max_rss_bytes")
    seconds = supervision["phase_timeout_seconds"]
    if type(seconds) not in (int, float) or not 0 < seconds <= sys.maxsize or not math.isfinite(seconds):
        raise ProfileError("phase_timeout_seconds")
    return profile


def load_profile(path=DEFAULT_PROFILE):
    return validate_profile(json.loads(Path(path).read_text()))
