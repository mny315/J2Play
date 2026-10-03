"""Profile fixtures shared by validator and catalog tests."""

import json
from pathlib import Path

PROFILES = Path(__file__).resolve().parents[2] / "profiles"


def read_profile(relative):
    return json.loads((PROFILES / relative).read_text(encoding="utf-8"))


def read_family(manufacturer):
    return {
        profile["profile_id"]: profile
        for path in (PROFILES / manufacturer).glob("*.json")
        for profile in [read_profile(path)]
    }
