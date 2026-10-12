#!/usr/bin/env python3
"""Dependency-free schema and device-specific profile validation."""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCHEMA = ROOT / "profiles/schema/device-profile-v2.schema.json"


def fail(path, message):
    raise ValueError(f"{path}: {message}")


def _type_matches(value, expected):
    return {
        "null": value is None,
        "boolean": type(value) is bool,
        "integer": type(value) is int,
        "number": type(value) in (int, float),
        "string": isinstance(value, str),
        "array": isinstance(value, list),
        "object": isinstance(value, dict),
    }[expected]


def _resolve_ref(root, reference):
    if not reference.startswith("#/"):
        raise ValueError(f"schema: unsupported external $ref {reference!r}")
    node = root
    for token in reference[2:].split("/"):
        node = node[token.replace("~1", "/").replace("~0", "~")]
    return node


def validate_schema(instance, schema, root=None, path="profile"):
    """Validate the project schemas' supported Draft 2020-12 vocabulary."""
    root = schema if root is None else root
    if "$ref" in schema:
        validate_schema(instance, _resolve_ref(root, schema["$ref"]), root, path)
    for subschema in schema.get("allOf", []):
        validate_schema(instance, subschema, root, path)
    if "oneOf" in schema:
        matches = 0
        errors = []
        for subschema in schema["oneOf"]:
            try:
                validate_schema(instance, subschema, root, path)
                matches += 1
            except ValueError as error:
                errors.append(str(error))
        if matches != 1:
            fail(path, f"must match exactly one schema alternative ({'; '.join(errors)})")

    if "const" in schema and (
            instance != schema["const"]
            or isinstance(instance, bool) != isinstance(schema["const"], bool)):
        fail(path, f"must equal {schema['const']!r}")
    if "enum" in schema and instance not in schema["enum"]:
        fail(path, f"must be one of {schema['enum']!r}")
    expected_types = schema.get("type")
    if expected_types is not None:
        expected_types = [expected_types] if isinstance(expected_types, str) else expected_types
        if not any(_type_matches(instance, item) for item in expected_types):
            fail(path, f"expected type {' or '.join(expected_types)}")

    if isinstance(instance, str):
        if len(instance) < schema.get("minLength", 0):
            fail(path, "string is too short")
        if "pattern" in schema and re.fullmatch(schema["pattern"], instance) is None:
            fail(path, f"does not match {schema['pattern']!r}")
    if type(instance) in (int, float):
        if "minimum" in schema and instance < schema["minimum"]:
            fail(path, f"must be at least {schema['minimum']}")
        if "maximum" in schema and instance > schema["maximum"]:
            fail(path, f"must be at most {schema['maximum']}")
    if isinstance(instance, list):
        if len(instance) < schema.get("minItems", 0):
            fail(path, "array has too few items")
        if "maxItems" in schema and len(instance) > schema["maxItems"]:
            fail(path, "array has too many items")
        if schema.get("uniqueItems"):
            serialized = [json.dumps(item, sort_keys=True) for item in instance]
            if len(serialized) != len(set(serialized)):
                fail(path, "array items must be unique")
        if isinstance(schema.get("items"), dict):
            for index, item in enumerate(instance):
                validate_schema(item, schema["items"], root, f"{path}[{index}]")
    if isinstance(instance, dict):
        for name in schema.get("required", []):
            if name not in instance:
                fail(path, f"missing required property {name!r}")
        properties = schema.get("properties", {})
        additional = schema.get("additionalProperties", True)
        for name, value in instance.items():
            child_path = f"{path}.{name}"
            if name in properties:
                validate_schema(value, properties[name], root, child_path)
            elif additional is False:
                fail(path, f"unexpected property {name!r}")
            elif isinstance(additional, dict):
                validate_schema(value, additional, root, child_path)
            if "propertyNames" in schema:
                validate_schema(name, schema["propertyNames"], root, child_path)


def validate(profile, schema=None):
    schema = schema or json.loads(DEFAULT_SCHEMA.read_text(encoding="utf-8"))
    validate_schema(profile, schema)

    source_ids = [source["id"] for source in profile["evidence_sources"]]
    known_sources = set(source_ids)
    if len(source_ids) != len(known_sources):
        fail("profile.evidence_sources", "source IDs must be unique")

    def check_sources(node, path="profile"):
        if isinstance(node, dict):
            if "confidence" in node and "sources" in node:
                unknown = set(node["sources"]) - known_sources
                if unknown:
                    fail(path, f"unknown source IDs: {sorted(unknown)!r}")
            for name, child in node.items():
                check_sources(child, f"{path}.{name}")
        elif isinstance(node, list):
            for index, child in enumerate(node):
                check_sources(child, f"{path}[{index}]")

    check_sources(profile)
    for field in ("vendor_apis", "compatibility_apis"):
        values = profile["java"][field]["value"]
        if values is not None and any(
            re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", value) is None
            for value in values
        ):
            fail(f"profile.java.{field}", "capabilities must be lowercase slugs")
    compatibility_jsrs = profile["java"]["compatibility_jsrs"]["value"] or []
    if any(re.fullmatch(r"[1-9][0-9]*(?:-[a-z][a-z0-9-]*)?", value) is None for value in compatibility_jsrs):
        fail("profile.java.compatibility_jsrs", "JSR IDs must be numeric slugs")
    supports_m3g = "184" in profile["java"]["jsrs"] or "184" in compatibility_jsrs
    if supports_m3g != (profile["m3g"] is not None):
        fail("profile.m3g", "must be present exactly when JSR-184 is declared")
    if profile["m3g"] is not None:
        m3g = profile["m3g"]
        compatibility_dimension = m3g.get("compatibility", {}).get(
            "max_texture_dimension"
        )
        if compatibility_dimension is not None:
            device_dimension = m3g["properties"]["max_texture_dimension"]["value"]
            if compatibility_dimension < device_dimension:
                fail(
                    "profile.m3g.compatibility.max_texture_dimension",
                    "must not be below the device property",
                )
            if compatibility_dimension * compatibility_dimension > m3g["budgets"]["texture_pixels"]:
                fail(
                    "profile.m3g.compatibility.max_texture_dimension",
                    "exceeds the texture pixel budget",
                )
    bluetooth_properties = profile["java"].get("bluetooth_properties")
    supports_bluetooth = "82" in profile["java"]["jsrs"] or "82" in compatibility_jsrs
    if bluetooth_properties is not None and not supports_bluetooth:
        fail("profile.java.bluetooth_properties", "requires JSR-82 support")
    if bluetooth_properties is not None and bluetooth_properties["value"] is not None:
        if any(not name or not value for name, value in bluetooth_properties["value"].items()):
            fail("profile.java.bluetooth_properties", "names and values must not be empty")
    keys = profile["input"]["canvas_keys"]
    names = [item["name"] for item in keys]
    codes = [item["key_code"] for item in keys]
    if len(names) != len(set(names)) or len(codes) != len(set(codes)):
        fail("profile.input.canvas_keys", "duplicate key name or key code")
    display = profile["display"]
    canvas_width = display["fullscreen_canvas_width"]["value"]
    canvas_height = display["fullscreen_canvas_height"]["value"]
    font_height_evidence = display.get("lcd_ui_font_heights")
    if font_height_evidence is not None:
        font_heights = font_height_evidence["value"]
        ordered_heights = [
            font_heights["small"],
            font_heights["medium"],
            font_heights["large"],
        ]
        if ordered_heights != sorted(ordered_heights):
            fail(
                "profile.display.lcd_ui_font_heights",
                "heights must be ordered small, medium, large",
            )
    if canvas_width is not None and canvas_height is not None:
        if canvas_width > 4096 or canvas_height > 4096 or canvas_width * canvas_height > 4_194_304:
            fail("profile.display", "full-screen Canvas dimensions exceed implementation bounds")
        normal = display["non_fullscreen_drawable_area"]["value"]
        if normal is not None and (
            normal["width"] > canvas_width or normal["height"] > canvas_height
        ):
            fail(
                "profile.display.non_fullscreen_drawable_area",
                "base drawable area must fit inside its full-screen Canvas",
            )
    default_screen_mode = display.get("default_screen_mode")
    screen_modes = display.get("screen_modes")
    if (default_screen_mode is None) != (screen_modes is None):
        fail(
            "profile.display",
            "default_screen_mode and screen_modes must be declared together",
        )
    if screen_modes is not None:
        mode_ids = [mode["id"] for mode in screen_modes]
        if len(mode_ids) != len(set(mode_ids)):
            fail("profile.display.screen_modes", "screen mode IDs must be unique")
        orientation_independent_sizes = []
        for index, mode in enumerate(screen_modes):
            path = f"profile.display.screen_modes[{index}]"
            fullscreen = mode["fullscreen_canvas"]
            normal = mode["non_fullscreen_drawable_area"]
            width, height = fullscreen["width"], fullscreen["height"]
            if width > 4096 or height > 4096 or width * height > 4_194_304:
                fail(path, "full-screen Canvas dimensions exceed implementation bounds")
            if normal is not None and (
                normal["width"] > width or normal["height"] > height
            ):
                fail(path, "drawable area must fit inside the full-screen Canvas")
            rotated_normal = mode.get("rotated_non_fullscreen_drawable_area")
            if rotated_normal is not None and (
                rotated_normal["width"] > height
                or rotated_normal["height"] > width
            ):
                fail(
                    path,
                    "rotated drawable area must fit inside the rotated full-screen Canvas",
                )
            orientation_independent_sizes.append((min(width, height), max(width, height)))
        if len(orientation_independent_sizes) != len(set(orientation_independent_sizes)):
            fail(
                "profile.display.screen_modes",
                "screen mode dimensions must be unique including rotation",
            )
        by_id = {mode["id"]: mode for mode in screen_modes}
        if default_screen_mode not in by_id:
            fail(
                "profile.display.default_screen_mode",
                "must reference a declared screen mode",
            )
        default = by_id[default_screen_mode]
        expected_fullscreen = {"width": canvas_width, "height": canvas_height}
        if default["fullscreen_canvas"] != expected_fullscreen or default[
            "non_fullscreen_drawable_area"
        ] != display["non_fullscreen_drawable_area"]["value"]:
            fail(
                "profile.display.default_screen_mode",
                "default mode must match the profile's base Canvas dimensions",
            )
    archive_name_hints = profile["device"].get("archive_name_hints", [])
    normalized_models = [
        model.strip().lower()
        for hint in archive_name_hints
        for model in ([hint["model"]] if "model" in hint else hint["models"])
    ]
    if any(not re.search(r"[a-z0-9]", model) for model in normalized_models):
        fail("profile.device.archive_name_hints", "models must contain an ASCII token")
    if len(normalized_models) != len(set(normalized_models)):
        fail("profile.device.archive_name_hints", "models must be unique")
    if screen_modes is None:
        declared_dimensions = {(canvas_width, canvas_height)}
        if canvas_width != canvas_height:
            declared_dimensions.add((canvas_height, canvas_width))
    else:
        declared_dimensions = {
            dimensions
            for mode in screen_modes
            for dimensions in (
                (mode["fullscreen_canvas"]["width"], mode["fullscreen_canvas"]["height"]),
                (mode["fullscreen_canvas"]["height"], mode["fullscreen_canvas"]["width"]),
            )
        }
    for index, hint in enumerate(archive_name_hints):
        path = f"profile.device.archive_name_hints[{index}]"
        dimensions = (
            hint["fullscreen_canvas"]["width"],
            hint["fullscreen_canvas"]["height"],
        )
        if dimensions[0] > 4096 or dimensions[1] > 4096 or dimensions[0] * dimensions[1] > 4_194_304:
            fail(path, "full-screen Canvas dimensions exceed implementation bounds")
        if dimensions not in declared_dimensions:
            fail(path, "Canvas must match a declared screen mode, including rotation")
    pointer = profile["input"].get("pointer")
    if pointer is not None and pointer["motion_events"]["value"] and not pointer["events"]["value"]:
        fail("profile.input.pointer", "pointer motion requires pointer events")


def manufacturer_slug(manufacturer):
    return "-".join(re.findall(r"[a-z0-9]+", manufacturer.lower()))


def validate_catalog_location(path, profile):
    try:
        relative = path.resolve().relative_to((ROOT / "profiles").resolve())
    except ValueError:
        fail(str(path), "catalog profile must live below profiles/")
    expected_directory = manufacturer_slug(profile["device"]["manufacturer"])
    if len(relative.parts) != 2 or relative.parts[0] != expected_directory:
        fail(str(path), f"must live in profiles/{expected_directory}/")


def main(argv):
    if len(argv) not in (1, 2):
        print(f"usage: {Path(argv[0]).name} [profile.json]", file=sys.stderr)
        return 2
    catalog_mode = len(argv) == 1
    paths = [Path(argv[1])] if not catalog_mode else sorted(
        path for path in (ROOT / "profiles").rglob("*.json")
        if "schema" not in path.relative_to(ROOT / "profiles").parts
    )
    try:
        schema = json.loads(DEFAULT_SCHEMA.read_text(encoding="utf-8"))
        profile_ids = set()
        model_owners = {}
        catalog = []
        for path in paths:
            profile = json.loads(path.read_text(encoding="utf-8"))
            validate(profile, schema)
            if catalog_mode:
                validate_catalog_location(path, profile)
            if profile["profile_id"] in profile_ids:
                fail(str(path), f"duplicate profile_id {profile['profile_id']!r}")
            profile_ids.add(profile["profile_id"])
            for hint in profile["device"].get("archive_name_hints", []):
                for model in ([hint["model"]] if "model" in hint else hint["models"]):
                    normalized = model.strip().lower()
                    if normalized in model_owners:
                        fail(
                            str(path),
                            f"model {model!r} is already owned by {model_owners[normalized]}",
                        )
                    model_owners[normalized] = profile["profile_id"]
            catalog.append((path, profile))
        if catalog_mode:
            profiles_by_id = {
                profile["profile_id"]: (path, profile) for path, profile in catalog
            }
            used_automatic_hosts = set()
            for path, profile in catalog:
                persona = profile["composition"]["persona"]
                host_id = persona["automatic_host"]
                if host_id not in profiles_by_id:
                    fail(str(path), f"unknown automatic host {host_id!r}")
                _, runtime_host = profiles_by_id[host_id]
                host = runtime_host["composition"]["host"]
                if not host["automatic"]:
                    fail(str(path), f"automatic route references manual-only host {host_id!r}")
                if persona["lineage"] not in host["accepts_lineages"]:
                    fail(
                        str(path),
                        f"host {host_id!r} does not accept lineage {persona['lineage']!r}",
                    )
                if runtime_host["device"]["manufacturer"].casefold() != profile["device"]["manufacturer"].casefold():
                    fail(str(path), f"automatic host {host_id!r} changes manufacturer")
                capacities = host["capacity_limits"]
                for field in ("heap_bytes", "rms_bytes", "jar_bytes"):
                    target_limit = profile["limits"][field]["value"]
                    if target_limit is not None and capacities[field] < target_limit:
                        fail(
                            str(path),
                            f"host {host_id!r} {field} is below the persona limit",
                        )
                used_automatic_hosts.add(host_id)
            declared_automatic_hosts = {
                profile["profile_id"]
                for _, profile in catalog
                if profile["composition"]["host"]["automatic"]
            }
            unused_hosts = declared_automatic_hosts - used_automatic_hosts
            if unused_hosts:
                fail("profiles", f"unused automatic hosts: {sorted(unused_hosts)!r}")
    except (OSError, KeyError, json.JSONDecodeError, ValueError) as error:
        print(f"invalid DeviceProfile: {error}", file=sys.stderr)
        return 1
    print(f"valid DeviceProfiles: {len(paths)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
