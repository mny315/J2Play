import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))

from validate_profile import validate
from profile_support import read_family, read_profile


class DeviceProfileCatalogTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.profile = read_profile("sony-ericsson/featurephone.json")
        cls.se_profiles = read_family("sony-ericsson")
        cls.siemens_profiles = read_family("siemens")
        cls.benq_siemens_profiles = read_family("benq-siemens")
        cls.samsung_profiles = read_family("samsung")
        cls.nokia_profile = read_profile("nokia/featurephone.json")
        cls.nokia_s40_v1_keypad_profile = read_profile("nokia/s40-v1-keypad.json")
        cls.nokia_s40_v2_keypad_profile = read_profile("nokia/s40-v2-keypad.json")
        cls.nokia_s40_touch_profile = read_profile("nokia/s40-touch.json")
        cls.nokia_asha_touch_profile = read_profile("nokia/asha-touch.json")
        cls.nokia_s60_keypad_profile = read_profile("nokia/s60-keypad.json")
        cls.nokia_s60_v1_keypad_profile = read_profile("nokia/s60-v1-keypad.json")
        cls.nokia_s60_v2_keypad_profile = read_profile("nokia/s60-v2-keypad.json")
        cls.nokia_touch_profile = read_profile("nokia/s60-touch.json")

    def test_se_featurephone_profile_is_valid(self):
        validate(self.profile)
        self.assertEqual(self.profile["family_id"], "se-featurephone")
        self.assertEqual(self.profile["profile_id"], "se-featurephone")
        self.assertEqual(self.profile["display"]["default_screen_mode"], "240x320")
        self.assertEqual(
            self.profile["composition"]["host"]["capacity_limits"]["heap_bytes"],
            6 * 1024 * 1024,
        )
        self.assertEqual(
            {
                mode["id"]: (
                    mode["fullscreen_canvas"],
                    mode["non_fullscreen_drawable_area"],
                )
                for mode in self.profile["display"]["screen_modes"]
            },
            {
                "176x220": (
                    {"width": 176, "height": 220},
                    {"width": 176, "height": 176},
                ),
                "240x320": (
                    {"width": 240, "height": 320},
                    {"width": 240, "height": 266},
                ),
            },
        )

    def test_all_sony_ericsson_java_platform_profiles_are_independent(self):
        self.assertEqual(
            set(self.se_profiles),
            {
                "se-featurephone",
                "se-jp1-keypad",
                "se-jp2-keypad",
                "se-jp3-keypad",
                "se-jp4-keypad",
                "se-jp5-keypad",
                "se-jp6-keypad",
                "se-jp6-no-bluetooth-keypad",
                "se-jp6-touch",
                "se-jp7-basic-keypad",
                "se-jp7-media-keypad",
                "se-jp7-no-bluetooth-keypad",
                "se-jp8-keypad",
                "se-jp8-late-keypad",
                "se-jp8-touch",
                "se-entry-keypad",
                "se-entry-3d-keypad",
            },
        )
        for profile_id, profile in self.se_profiles.items():
            with self.subTest(profile=profile_id):
                validate(profile)
                self.assertEqual(profile["device"]["manufacturer"], "Sony Ericsson")

        model_profiles = {
            model: (profile_id, hint["fullscreen_canvas"])
            for profile_id, profile in self.se_profiles.items()
            for hint in profile["device"].get("archive_name_hints", [])
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(
            model_profiles["Sony Ericsson T610"],
            ("se-jp1-keypad", {"width": 128, "height": 160}),
        )
        self.assertEqual(
            model_profiles["Sony Ericsson K800"],
            ("se-featurephone", {"width": 240, "height": 320}),
        )
        self.assertEqual(
            model_profiles["Sony Ericsson Aino U10i"],
            ("se-jp8-touch", {"width": 240, "height": 432}),
        )
        self.assertTrue(
            self.se_profiles["se-jp8-touch"]["input"]["pointer"]["events"]["value"]
        )
        self.assertIsNone(self.se_profiles["se-jp7-basic-keypad"]["m3g"])
        self.assertNotIn(
            "82", self.se_profiles["se-jp6-no-bluetooth-keypad"]["java"]["jsrs"]
        )
        self.assertNotIn(
            "234-camera", self.se_profiles["se-jp7-media-keypad"]["java"]["jsrs"]
        )
        self.assertNotIn(
            "82",
            self.se_profiles["se-jp7-no-bluetooth-keypad"]["java"]["jsrs"],
        )

    def test_siemens_profiles_are_independent_and_keep_platform_keymaps(self):
        self.assertEqual(
            set(self.siemens_profiles),
            {
                "siemens-midp1-basic-keypad",
                "siemens-midp1-color-keypad",
                "siemens-sgold-keypad",
                "siemens-featurephone",
                "siemens-sxg75-keypad",
            },
        )
        self.assertEqual(
            set(self.benq_siemens_profiles),
            {"benq-siemens-featurephone", "benq-siemens-ef81-keypad"},
        )
        for profile_id, profile in {
            **self.siemens_profiles,
            **self.benq_siemens_profiles,
        }.items():
            with self.subTest(profile=profile_id):
                validate(profile)

        models = {
            model: (profile_id, hint["fullscreen_canvas"])
            for profile_id, profile in {
                **self.siemens_profiles,
                **self.benq_siemens_profiles,
            }.items()
            for hint in profile["device"]["archive_name_hints"]
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(
            models["Siemens A56i"],
            ("siemens-midp1-basic-keypad", {"width": 101, "height": 64}),
        )
        self.assertEqual(
            models["Siemens C65"],
            ("siemens-sgold-keypad", {"width": 130, "height": 130}),
        )
        self.assertEqual(
            models["Siemens SXG75"],
            ("siemens-sxg75-keypad", {"width": 240, "height": 320}),
        )
        self.assertEqual(
            models["BenQ-Siemens E71"],
            ("benq-siemens-featurephone", {"width": 240, "height": 320}),
        )

        sgold_keys = {
            key["name"]: key["key_code"]
            for key in self.siemens_profiles["siemens-featurephone"]["input"][
                "canvas_keys"
            ]
        }
        self.assertEqual(
            {name: sgold_keys[name] for name in ("UP", "DOWN", "LEFT", "RIGHT", "FIRE")},
            {"UP": -59, "DOWN": -60, "LEFT": -61, "RIGHT": -62, "FIRE": -26},
        )
        self.assertEqual(sgold_keys["SOFT_LEFT"], -1)
        self.assertEqual(sgold_keys["SOFT_RIGHT"], -4)

        sg2_keys = {
            key["name"]: key
            for key in self.benq_siemens_profiles["benq-siemens-featurephone"][
                "input"
            ]["canvas_keys"]
        }
        self.assertEqual(sg2_keys["SOFT_LEFT"]["key_code"], -1)
        self.assertEqual(sg2_keys["SOFT_RIGHT"]["key_code"], -4)
        self.assertEqual(sg2_keys["SOFT_LEFT"]["game_action"], "FIRE")
        self.assertEqual(sg2_keys["SOFT_RIGHT"]["game_action"], "FIRE")

        ef81_keys = {
            key["name"]: key["key_code"]
            for key in self.benq_siemens_profiles["benq-siemens-ef81-keypad"][
                "input"
            ]["canvas_keys"]
        }
        self.assertEqual(
            {name: ef81_keys[name] for name in ("UP", "DOWN", "LEFT", "RIGHT", "FIRE")},
            {"UP": -1, "DOWN": -2, "LEFT": -3, "RIGHT": -4, "FIRE": -5},
        )
        self.assertEqual(ef81_keys["SOFT_LEFT"], -6)
        self.assertEqual(ef81_keys["SOFT_RIGHT"], -7)
        self.assertEqual(ef81_keys["CLEAR"], -8)
        ef81_profile = self.benq_siemens_profiles["benq-siemens-ef81-keypad"]
        self.assertTrue(ef81_profile["java"]["jsrs"]["184"]["value"])
        self.assertEqual(ef81_profile["m3g"]["version"]["value"], "1.0")
        self.assertGreaterEqual(
            ef81_profile["m3g"]["properties"]["max_viewport_height"]["value"],
            320,
        )

        self.assertEqual(
            self.siemens_profiles["siemens-midp1-basic-keypad"]["java"][
                "vendor_apis"
            ]["value"],
            ["siemens-game"],
        )
        self.assertIsNone(self.siemens_profiles["siemens-sgold-keypad"]["m3g"])
        self.assertEqual(
            self.benq_siemens_profiles["benq-siemens-featurephone"]["m3g"][
                "properties"
            ]["max_transforms_per_vertex"]["value"],
            32,
        )

    def test_siemens_profiles_use_two_explicit_automatic_hosts(self):
        for profile in self.siemens_profiles.values():
            self.assertEqual(
                profile["composition"]["persona"]["automatic_host"],
                "siemens-featurephone",
            )
        for profile in self.benq_siemens_profiles.values():
            self.assertEqual(
                profile["composition"]["persona"]["automatic_host"],
                "benq-siemens-featurephone",
            )
        self.assertTrue(
            self.siemens_profiles["siemens-featurephone"]["composition"]["host"][
                "automatic"
            ]
        )
        self.assertTrue(
            self.benq_siemens_profiles["benq-siemens-featurephone"]["composition"][
                "host"
            ]["automatic"]
        )

    def test_samsung_profiles_cover_keypad_touch_and_sdk_exceptions(self):
        self.assertEqual(
            set(self.samsung_profiles),
            {
                "samsung-midp1-keypad",
                "samsung-midp2-cldc10-keypad",
                "samsung-midp2-keypad",
                "samsung-featurephone",
                "samsung-midp20-touch",
                "samsung-midp21-touch",
                "samsung-f480-touch",
                "samsung-midp20-m3g-touch",
                "samsung-wvga-touch",
                "samsung-touch",
            },
        )
        for profile_id, profile in self.samsung_profiles.items():
            with self.subTest(profile=profile_id):
                validate(profile)
                self.assertEqual(profile["device"]["manufacturer"], "Samsung")

        models = {
            model: (profile_id, hint["fullscreen_canvas"])
            for profile_id, profile in self.samsung_profiles.items()
            for hint in profile["device"]["archive_name_hints"]
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(len(models), 159)
        sdk_contracts = {
            ("samsung-midp2-keypad", 128, 160): {
                "Samsung SGH-E250",
                "Samsung SGH-J700",
                "Samsung GT-C5010",
                "Samsung GT-E2550",
            },
            ("samsung-midp2-keypad", 172, 205): {"Samsung SGH-J800"},
            ("samsung-midp2-keypad", 176, 225): {"Samsung SGH-B2700"},
            ("samsung-featurephone", 240, 320): {"Samsung SGH-S5320"},
            ("samsung-midp20-touch", 240, 320): {
                "Samsung GT-B3410W",
                "Samsung GT-C3300",
                "Samsung GT-S3510",
            },
            ("samsung-midp21-touch", 240, 320): {"Samsung GT-C3510"},
            ("samsung-f480-touch", 240, 320): {"Samsung F480"},
            ("samsung-midp20-m3g-touch", 240, 400): {
                "Samsung GT-S5230N",
                "Samsung GT-S5250",
            },
            ("samsung-midp20-m3g-touch", 240, 432): {"Samsung SGH-F700"},
            ("samsung-touch", 240, 320): {"Samsung GT-S3370"},
            ("samsung-touch", 240, 400): {
                "Samsung GT-B7722",
                "Samsung GT-S5620",
                "Samsung GT-S8000",
                "Samsung GT-S8530",
                "Samsung SGH-M8800",
                "Samsung SGH-S5230",
            },
            ("samsung-wvga-touch", 480, 800): {"Samsung GT-S8500"},
        }
        self.assertEqual(sum(map(len, sdk_contracts.values())), 23)
        for (profile_id, width, height), sdk_models in sdk_contracts.items():
            for model in sdk_models:
                self.assertEqual(
                    models[model],
                    (profile_id, {"width": width, "height": height}),
                    model,
                )
        self.assertEqual(
            models["Samsung SGH-E700"],
            ("samsung-midp1-keypad", {"width": 128, "height": 160}),
        )
        self.assertEqual(
            models["Samsung SGH-E250"],
            ("samsung-midp2-keypad", {"width": 128, "height": 160}),
        )
        for e380_model in ("Samsung E380", "Samsung SGH-E380"):
            self.assertEqual(
                models[e380_model],
                ("samsung-midp2-keypad", {"width": 176, "height": 220}),
            )
        for e810_model in ("Samsung E810", "Samsung SGH-E810"):
            self.assertEqual(
                models[e810_model],
                (
                    "samsung-midp2-cldc10-keypad",
                    {"width": 128, "height": 160},
                ),
            )
        self.assertEqual(
            models["Samsung SGH-B2700"],
            ("samsung-midp2-keypad", {"width": 176, "height": 225}),
        )
        self.assertEqual(
            models["Samsung SGH-F480"],
            ("samsung-f480-touch", {"width": 240, "height": 320}),
        )
        self.assertEqual(
            models["Samsung SGH-F700"],
            ("samsung-midp20-m3g-touch", {"width": 240, "height": 432}),
        )
        self.assertEqual(
            models["Samsung GT-S8530"],
            ("samsung-touch", {"width": 240, "height": 400}),
        )
        for wvga_model in ("Samsung GT-S8500", "Samsung GT-S8600", "Samsung Wave 3"):
            self.assertEqual(
                models[wvga_model],
                ("samsung-wvga-touch", {"width": 480, "height": 800}),
            )
        self.assertEqual(
            self.samsung_profiles["samsung-wvga-touch"]["display"][
                "default_screen_mode"
            ],
            "480x800",
        )
        wvga = self.samsung_profiles["samsung-wvga-touch"]
        self.assertEqual(
            wvga["display"]["non_fullscreen_drawable_area"]["value"],
            {"width": 480, "height": 602},
        )
        self.assertTrue(wvga["m3g"]["properties"]["support_mipmapping"]["value"])
        self.assertEqual(
            wvga["m3g"]["properties"]["max_texture_dimension"]["value"],
            2048,
        )
        self.assertEqual(wvga["m3g"]["properties"]["num_texture_units"]["value"], 2)
        self.assertGreaterEqual(wvga["m3g"]["budgets"]["texture_pixels"], 2048 * 2048)
        for unsupported_runtime in (
            "Samsung GalaxyTab",
            "Samsung Blackjack",
            "Samsung SPH-I600",
            "Samsung SGH-I900",
            "Samsung i718",
            "Samsung SGH-D720",
            "Samsung SGH-D730",
            "Samsung SGH-G810",
            "Samsung SGH-i560",
        ):
            self.assertNotIn(unsupported_runtime, models)

        keypad_keys = {
            key["name"]: key
            for key in self.samsung_profiles["samsung-featurephone"]["input"][
                "canvas_keys"
            ]
        }
        self.assertEqual(
            {name: keypad_keys[name]["key_code"] for name in ("UP", "DOWN", "LEFT", "RIGHT", "FIRE")},
            {"UP": -1, "DOWN": -2, "LEFT": -3, "RIGHT": -4, "FIRE": -5},
        )
        self.assertEqual(keypad_keys["SOFT_LEFT"]["key_code"], -6)
        self.assertEqual(keypad_keys["SOFT_RIGHT"]["key_code"], -7)
        self.assertEqual(keypad_keys["SOFT_LEFT"]["game_action"], "FIRE")
        self.assertEqual(keypad_keys["SOFT_RIGHT"]["game_action"], "FIRE")

        f480_keys = {
            key["name"]: key["key_code"]
            for key in self.samsung_profiles["samsung-f480-touch"]["input"][
                "canvas_keys"
            ]
        }
        self.assertEqual(f480_keys["CLEAR"], -45)
        self.assertTrue(
            self.samsung_profiles["samsung-f480-touch"]["input"]["pointer"][
                "events"
            ]["value"]
        )
        self.assertIsNone(self.samsung_profiles["samsung-midp21-touch"]["m3g"])
        self.assertTrue(
            self.samsung_profiles["samsung-touch"]["java"]["jsrs"]["184"][
                "value"
            ]
        )
        self.assertEqual(
            self.samsung_profiles["samsung-touch"]["java"]["compatibility_apis"][
                "value"
            ],
            ["samsung-audioclip", "sensor-probe"],
        )

    def test_samsung_profiles_use_compatible_keypad_and_touch_hosts(self):
        for profile_id, profile in self.samsung_profiles.items():
            lineage = profile["composition"]["persona"]["lineage"]
            expected_host = {
                "samsung-keypad": "samsung-featurephone",
                "samsung-touch": "samsung-touch",
            }[lineage]
            if profile_id == "samsung-wvga-touch":
                expected_host = "samsung-wvga-touch"
            self.assertEqual(
                profile["composition"]["persona"]["automatic_host"],
                expected_host,
                profile_id,
            )
        self.assertTrue(
            self.samsung_profiles["samsung-featurephone"]["composition"]["host"][
                "automatic"
            ]
        )
        self.assertTrue(
            self.samsung_profiles["samsung-touch"]["composition"]["host"][
                "automatic"
            ]
        )
        self.assertTrue(
            self.samsung_profiles["samsung-wvga-touch"]["composition"]["host"][
                "automatic"
            ]
        )

    def test_nokia_featurephone_profile_is_valid_and_independent(self):
        validate(self.nokia_profile)
        self.assertEqual(self.nokia_profile["family_id"], "nokia-featurephone")
        self.assertEqual(self.nokia_profile["profile_id"], "nokia-featurephone")
        self.assertEqual(
            {
                mode["id"]: (
                    mode["fullscreen_canvas"],
                    mode["non_fullscreen_drawable_area"],
                )
                for mode in self.nokia_profile["display"]["screen_modes"]
            },
            {
                "128x160": (
                    {"width": 128, "height": 160},
                    {"width": 128, "height": 115},
                ),
                "208x208": ({"width": 208, "height": 208}, None),
                "240x320": (
                    {"width": 240, "height": 320},
                    {"width": 240, "height": 248},
                ),
                "320x480": ({"width": 320, "height": 480}, None),
            },
        )
        self.assertEqual(
            self.nokia_profile["limits"]["heap_bytes"]["value"], 2_097_152
        )
        self.assertEqual(
            self.nokia_profile["java"]["vendor_apis"]["value"],
            ["nokia-ui", "nokia-sound"],
        )
        self.assertNotIn(
            "mascot-capsule-micro3d-v3",
            self.nokia_profile["java"]["vendor_apis"]["value"],
        )
        self.assertIsNone(
            self.nokia_profile["m3g"]["properties"]["depth_bits"]["value"]
        )
        model_screens = {
            model: hint["fullscreen_canvas"]
            for hint in self.nokia_profile["device"]["archive_name_hints"]
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(model_screens["Nokia 5200"], {"width": 128, "height": 160})
        self.assertEqual(model_screens["Nokia C3-00"], {"width": 320, "height": 240})
        self.assertEqual(model_screens["Nokia 6260 slide"], {"width": 320, "height": 480})
        self.assertEqual(len(model_screens), 126)

    def test_series40_generation_and_touch_profiles_are_independent(self):
        expected = [
            (self.nokia_s40_v1_keypad_profile, "nokia-s40-v1-keypad", "128x128", False),
            (self.nokia_s40_v2_keypad_profile, "nokia-s40-v2-keypad", "128x160", False),
            (self.nokia_s40_touch_profile, "nokia-s40-touch", "240x320", True),
            (self.nokia_asha_touch_profile, "nokia-asha-touch", "240x320", True),
        ]
        for profile, profile_id, default_mode, pointer in expected:
            with self.subTest(profile=profile_id):
                validate(profile)
                self.assertEqual(profile["display"]["default_screen_mode"], default_mode)
                self.assertEqual(profile["input"]["pointer"]["events"]["value"], pointer)

        s40_v1_modes = {
            mode["id"] for mode in self.nokia_s40_v1_keypad_profile["display"]["screen_modes"]
        }
        self.assertEqual(s40_v1_modes, {"96x65", "128x128", "128x160"})
        s40_v2_modes = {
            mode["id"] for mode in self.nokia_s40_v2_keypad_profile["display"]["screen_modes"]
        }
        self.assertEqual(s40_v2_modes, {"104x208", "128x128", "128x160", "208x208"})
        touch_models = {
            model
            for hint in self.nokia_s40_touch_profile["device"]["archive_name_hints"]
            for model in hint["models"]
        }
        self.assertIn("Nokia Asha 311", touch_models)
        self.assertEqual(self.nokia_asha_touch_profile["limits"]["jar_bytes"]["value"], 5_242_880)

    def test_nokia_s60_touch_profile_is_valid_and_independent(self):
        validate(self.nokia_touch_profile)
        self.assertEqual(self.nokia_touch_profile["family_id"], "nokia-s60-touch")
        self.assertEqual(self.nokia_touch_profile["profile_id"], "nokia-s60-touch")
        modes = {
            mode["id"]: mode
            for mode in self.nokia_touch_profile["display"]["screen_modes"]
        }
        self.assertEqual(
            modes["360x640"]["rotated_non_fullscreen_drawable_area"],
            {"width": 502, "height": 288},
        )
        self.assertIsNone(modes["640x480"]["non_fullscreen_drawable_area"])
        self.assertTrue(
            self.nokia_touch_profile["input"]["pointer"]["events"]["value"]
        )
        self.assertTrue(
            self.nokia_touch_profile["input"]["pointer"]["motion_events"]["value"]
        )
        self.assertNotIn("256", self.nokia_touch_profile["java"]["jsrs"])
        self.assertIsNone(self.nokia_touch_profile["limits"]["heap_bytes"]["value"])
        self.assertEqual(
            self.nokia_touch_profile["java"]["system_properties"]["value"][
                "microedition.encoding"
            ],
            "ISO-8859-1",
        )
        self.assertEqual(
            self.nokia_touch_profile["m3g"]["properties"]["max_texture_dimension"]["value"],
            1024,
        )

    def test_nokia_s60_v1_keypad_profile_is_valid(self):
        validate(self.nokia_s60_v1_keypad_profile)
        self.assertEqual(
            self.nokia_s60_v1_keypad_profile["profile_id"],
            "nokia-s60-v1-keypad",
        )
        self.assertEqual(
            self.nokia_s60_v1_keypad_profile["display"]["screen_modes"],
            [
                {
                    "id": "176x208",
                    "fullscreen_canvas": {"width": 176, "height": 208},
                    "non_fullscreen_drawable_area": None,
                    "confidence": "inferred",
                    "sources": [
                        "nokia-javaone-s60-evolution",
                        "nokia-3650-data-sheet",
                        "nokia-ui-api-v1-1",
                    ],
                }
            ],
        )
        self.assertEqual(
            self.nokia_s60_v1_keypad_profile["java"]["configuration"]["value"],
            "CLDC-1.0",
        )
        self.assertIsNone(self.nokia_s60_v1_keypad_profile["m3g"])

    def test_nokia_s60_v2_keypad_profile_is_valid(self):
        validate(self.nokia_s60_v2_keypad_profile)
        self.assertEqual(
            self.nokia_s60_v2_keypad_profile["profile_id"],
            "nokia-s60-v2-keypad",
        )
        self.assertEqual(
            {
                mode["id"]: (
                    mode["fullscreen_canvas"],
                    mode["non_fullscreen_drawable_area"],
                )
                for mode in self.nokia_s60_v2_keypad_profile["display"][
                    "screen_modes"
                ]
            },
            {
                "176x208": ({"width": 176, "height": 208}, None),
                "352x416": ({"width": 352, "height": 416}, None),
            },
        )
        model_screens = {
            model: hint["fullscreen_canvas"]
            for hint in self.nokia_s60_v2_keypad_profile["device"]["archive_name_hints"]
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(model_screens["Nokia 6600"], {"width": 176, "height": 208})
        self.assertEqual(model_screens["Nokia N90"], {"width": 352, "height": 416})

    def test_nokia_s60_keypad_profile_is_valid_and_scalable(self):
        validate(self.nokia_s60_keypad_profile)
        self.assertEqual(
            self.nokia_s60_keypad_profile["family_id"], "nokia-s60-keypad"
        )
        self.assertEqual(
            self.nokia_s60_keypad_profile["profile_id"], "nokia-s60-keypad"
        )
        self.assertEqual(
            self.nokia_s60_keypad_profile["display"]["fullscreen_canvas_width"][
                "value"
            ],
            240,
        )
        self.assertEqual(
            self.nokia_s60_keypad_profile["display"]["fullscreen_canvas_height"][
                "value"
            ],
            320,
        )
        self.assertEqual(
            {
                mode["id"]: (
                    mode["fullscreen_canvas"],
                    mode["non_fullscreen_drawable_area"],
                )
                for mode in self.nokia_s60_keypad_profile["display"]["screen_modes"]
            },
            {
                "176x208": ({"width": 176, "height": 208}, None),
                "208x208": ({"width": 208, "height": 208}, None),
                "240x320": ({"width": 240, "height": 320}, None),
                "352x416": ({"width": 352, "height": 416}, None),
                "800x352": ({"width": 800, "height": 352}, None),
            },
        )
        model_screens = {
            model: hint["fullscreen_canvas"]
            for hint in self.nokia_s60_keypad_profile["device"]["archive_name_hints"]
            for model in ([hint["model"]] if "model" in hint else hint["models"])
        }
        self.assertEqual(model_screens["Nokia 3250"], {"width": 176, "height": 208})
        self.assertEqual(model_screens["Nokia 5500"], {"width": 208, "height": 208})
        self.assertEqual(model_screens["Nokia E71"], {"width": 320, "height": 240})
        self.assertEqual(model_screens["Nokia N80"], {"width": 352, "height": 416})
        self.assertEqual(model_screens["Nokia E90"], {"width": 800, "height": 352})
        self.assertFalse(
            self.nokia_s60_keypad_profile["input"]["pointer"]["events"]["value"]
        )

    def test_se_featurephone_key_table_is_profile_data(self):
        expected = {
            "UP": (-1, "UP"), "DOWN": (-2, "DOWN"),
            "LEFT": (-3, "LEFT"), "RIGHT": (-4, "RIGHT"),
            "FIRE": (-5, "FIRE"), "SOFT_LEFT": (-6, None),
            "SOFT_RIGHT": (-7, None), "CLEAR": (-8, None),
            "BACK": (-11, None), "GAME_BUTTON_A": (-13, "GAME_A"),
            "GAME_BUTTON_B": (-14, "GAME_B"), "STAR": (42, "GAME_C"),
            "POUND": (35, "GAME_D"), "NUM0": (48, None),
            "NUM1": (49, None), "NUM2": (50, "UP"),
            "NUM3": (51, None), "NUM4": (52, "LEFT"),
            "NUM5": (53, "FIRE"), "NUM6": (54, "RIGHT"),
            "NUM7": (55, "GAME_A"), "NUM8": (56, "DOWN"),
            "NUM9": (57, "GAME_B"),
        }
        actual = {
            item["name"]: (item["key_code"], item["game_action"])
            for item in self.profile["input"]["canvas_keys"]
        }
        self.assertEqual(actual, expected)

    def test_nokia_game_actions_do_not_reuse_sony_ericsson_mapping(self):
        actual = {
            item["name"]: (item["key_code"], item["game_action"])
            for item in self.nokia_profile["input"]["canvas_keys"]
        }
        self.assertEqual(actual["STAR"], (42, "GAME_D"))
        self.assertEqual(actual["POUND"], (35, "GAME_B"))
        self.assertEqual(actual["NUM7"], (55, "GAME_C"))
        self.assertEqual(actual["NUM9"], (57, "GAME_A"))
        self.assertEqual(actual["END"], (-11, None))
        self.assertNotIn("BACK", actual)
        self.assertNotIn("GAME_BUTTON_A", actual)
        self.assertNotIn("GAME_BUTTON_B", actual)


if __name__ == "__main__":
    unittest.main()
