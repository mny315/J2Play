import copy
import contextlib
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))

from validate_profile import main, validate, validate_catalog_location
from profile_support import read_profile


class DeviceProfileTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.profile = read_profile("sony-ericsson/featurephone.json")
        cls.nokia_profile = read_profile("nokia/featurephone.json")
        cls.nokia_s60_keypad_profile = read_profile("nokia/s60-keypad.json")
        cls.nokia_touch_profile = read_profile("nokia/s60-touch.json")

    def test_catalog_profiles_require_the_manufacturer_directory(self):
        validate_catalog_location(
            Path("profiles/sony-ericsson/featurephone.json"), self.profile
        )
        with self.assertRaisesRegex(ValueError, "profiles/sony-ericsson/"):
            validate_catalog_location(Path("profiles/featurephone.json"), self.profile)

    def test_composition_rejects_partial_or_manual_host_policy(self):
        missing_capacity = copy.deepcopy(self.profile)
        del missing_capacity["composition"]["host"]["capacity_limits"]
        with self.assertRaisesRegex(ValueError, "capacity_limits"):
            validate(missing_capacity)

        manual_with_policy = copy.deepcopy(self.profile)
        manual_with_policy["composition"]["host"]["automatic"] = False
        with self.assertRaisesRegex(ValueError, "schema alternative"):
            validate(manual_with_policy)

    def test_validator_rejects_extra_command_line_arguments(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            status = main(["validate_profile.py", "one.json", "two.json"])
        self.assertEqual(status, 2)
        self.assertIn("usage: validate_profile.py [profile.json]", stderr.getvalue())

    def test_archive_name_hint_must_use_a_declared_screen_rotation(self):
        profile = copy.deepcopy(self.nokia_profile)
        profile["device"]["archive_name_hints"][0]["fullscreen_canvas"] = {
            "width": 360,
            "height": 640,
        }
        with self.assertRaisesRegex(ValueError, "match a declared screen mode"):
            validate(profile)

    def test_nested_source_references_must_be_declared(self):
        for path in (
            ("device", "archive_name_hints", 0),
            ("display", "screen_modes", 0),
            ("java", "jsrs", "75"),
            ("input", "canvas_keys", 0),
        ):
            with self.subTest(path=path):
                profile = copy.deepcopy(self.profile)
                field = profile
                for key in path:
                    field = field[key]
                field["sources"] = ["missing-source"]
                with self.assertRaisesRegex(ValueError, "unknown source IDs"):
                    validate(profile)

    def test_archive_name_hint_models_are_normalized_before_deduplication(self):
        profile = copy.deepcopy(self.nokia_s60_keypad_profile)
        duplicate = copy.deepcopy(profile["device"]["archive_name_hints"][0])
        duplicate.pop("models", None)
        duplicate["model"] = " Nokia 3250 "
        profile["device"]["archive_name_hints"].append(duplicate)
        with self.assertRaisesRegex(ValueError, "models must be unique"):
            validate(profile)

    def test_pointer_motion_requires_pointer_events(self):
        profile = copy.deepcopy(self.profile)
        profile["input"]["pointer"]["motion_events"] = {
            "value": True,
            "confidence": "inferred",
            "sources": ["se-reference-whitepaper-r2a"],
        }
        with self.assertRaisesRegex(ValueError, "pointer motion requires pointer events"):
            validate(profile)

    def test_m3g_compatibility_texture_dimension_respects_device_and_budget(self):
        below_device = copy.deepcopy(self.profile)
        below_device["m3g"]["compatibility"]["max_texture_dimension"] = 128
        with self.assertRaisesRegex(ValueError, "must not be below"):
            validate(below_device)

        over_budget = copy.deepcopy(self.profile)
        over_budget["m3g"]["compatibility"]["max_texture_dimension"] = 2048
        with self.assertRaisesRegex(ValueError, "exceeds the texture pixel budget"):
            validate(over_budget)

    def test_v2_profile_without_family_extensions_remains_valid(self):
        profile = copy.deepcopy(self.profile)
        profile.pop("family_id")
        profile["input"].pop("pointer")
        profile["display"].pop("default_screen_mode")
        profile["display"].pop("screen_modes")
        profile["device"].pop("archive_name_hints")
        profile["profile_id"] = "legacy-profile"
        validate(profile)

    def test_unknown_cannot_hide_a_guessed_value(self):
        profile = copy.deepcopy(self.profile)
        profile["limits"]["jar_bytes"]["value"] = 1234
        with self.assertRaisesRegex(ValueError, "must match exactly one schema alternative"):
            validate(profile)

    def test_second_profile_can_have_its_own_dimensions_and_key_codes(self):
        profile = copy.deepcopy(self.profile)
        profile["profile_id"] = "nokia-test-device"
        profile["device"]["manufacturer"] = "Nokia"
        profile["device"]["model"] = "Test Device"
        profile["display"]["physical_width"]["value"] = 176
        profile["display"]["physical_height"]["value"] = 208
        profile["display"]["fullscreen_canvas_width"]["value"] = 176
        profile["display"]["fullscreen_canvas_height"]["value"] = 208
        profile["display"]["non_fullscreen_drawable_area"]["value"] = {
            "width": 176,
            "height": 182,
        }
        profile["display"].pop("default_screen_mode")
        profile["display"].pop("screen_modes")
        profile["device"].pop("archive_name_hints")
        profile["input"]["canvas_keys"][0]["key_code"] = -101
        del profile["java"]["jsrs"]["184"]
        profile["m3g"] = None
        validate(profile)

    def test_screen_mode_must_match_default_base_dimensions(self):
        profile = copy.deepcopy(self.profile)
        profile["display"]["default_screen_mode"] = "176x220"
        with self.assertRaisesRegex(ValueError, "default mode must match"):
            validate(profile)

    def test_screen_modes_reject_rotated_duplicates(self):
        profile = copy.deepcopy(self.profile)
        duplicate = copy.deepcopy(profile["display"]["screen_modes"][0])
        duplicate["id"] = "220x176"
        duplicate["fullscreen_canvas"] = {"width": 220, "height": 176}
        duplicate["non_fullscreen_drawable_area"] = {
            "width": 176,
            "height": 176,
        }
        profile["display"]["screen_modes"].append(duplicate)
        with self.assertRaisesRegex(ValueError, "unique including rotation"):
            validate(profile)

    def test_rotated_drawable_area_must_fit_rotated_fullscreen(self):
        profile = copy.deepcopy(self.nokia_touch_profile)
        profile["display"]["screen_modes"][0][
            "rotated_non_fullscreen_drawable_area"
        ] = {"width": 641, "height": 360}
        with self.assertRaisesRegex(ValueError, "rotated drawable area must fit"):
            validate(profile)

    def test_base_drawable_area_must_fit_fullscreen_canvas(self):
        profile = copy.deepcopy(self.nokia_s60_keypad_profile)
        profile["display"]["non_fullscreen_drawable_area"] = {
            "value": {"width": 321, "height": 240},
            "confidence": "inferred",
            "sources": ["nokia-e71-data-sheet"],
        }
        with self.assertRaisesRegex(ValueError, "base drawable area must fit"):
            validate(profile)

    def test_missing_nested_field_is_rejected_by_schema(self):
        profile = copy.deepcopy(self.profile)
        del profile["display"]["orientation"]
        with self.assertRaisesRegex(ValueError, "missing required property 'orientation'"):
            validate(profile)

    def test_wrong_nested_type_is_rejected_by_schema(self):
        profile = copy.deepcopy(self.profile)
        profile["limits"]["jar_bytes"]["sources"] = "hardware-probe"
        with self.assertRaisesRegex(ValueError, "expected type array"):
            validate(profile)

    def test_runtime_rate_above_schema_maximum_is_rejected(self):
        profile = copy.deepcopy(self.profile)
        profile["runtime"]["interpreter_instructions_per_second"] = 1_000_000_001
        with self.assertRaisesRegex(ValueError, "must be at most 1000000000"):
            validate(profile)

        profile = copy.deepcopy(self.profile)
        profile["runtime"]["frames_per_second"] = 1_001
        with self.assertRaisesRegex(ValueError, "must be at most 1000"):
            validate(profile)

    def test_boolean_constants_reject_numbers(self):
        for value in (0, 0.0, 1, 1.0):
            with self.subTest(value=value):
                profile = copy.deepcopy(self.profile)
                if value == 0:
                    profile["composition"]["host"] = {"automatic": value}
                else:
                    profile["composition"]["host"]["automatic"] = value
                with self.assertRaisesRegex(ValueError, "schema alternative"):
                    validate(profile)

        profile = copy.deepcopy(self.profile)
        profile["java"]["jsrs"]["184"]["value"] = 1
        with self.assertRaisesRegex(ValueError, "must equal True"):
            validate(profile)

    def test_duplicate_key_is_rejected(self):
        profile = copy.deepcopy(self.profile)
        profile["input"]["canvas_keys"][1] = copy.deepcopy(
            profile["input"]["canvas_keys"][0]
        )
        with self.assertRaisesRegex(ValueError, "duplicate key"):
            validate(profile)

    def test_arbitrary_jsr_identifier_is_rejected(self):
        profile = copy.deepcopy(self.profile)
        profile["java"]["jsrs"]["bluetooth"] = {
            "value": True,
            "confidence": "confirmed",
            "sources": ["se-reference-whitepaper-r2a"],
        }
        with self.assertRaisesRegex(ValueError, "does not match"):
            validate(profile)


if __name__ == "__main__":
    unittest.main()
