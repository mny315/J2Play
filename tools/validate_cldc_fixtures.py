#!/usr/bin/env python3
"""Validate the immutable guest archive and its CLDC method expectations."""

import hashlib
import io
import json
import struct
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT / "tests/fixtures/java-me/conformance.jar"
MANIFEST = ROOT / "tests/fixtures/java-me/cldc-methods.json"


def u2(raw, offset):
    if offset + 2 > len(raw):
        raise ValueError("truncated class file")
    return struct.unpack_from(">H", raw, offset)[0], offset + 2


def u4(raw, offset):
    if offset + 4 > len(raw):
        raise ValueError("truncated class file")
    return struct.unpack_from(">I", raw, offset)[0], offset + 4


def public_methods(raw):
    if raw[:4] != b"\xca\xfe\xba\xbe":
        raise ValueError("invalid class magic")
    count, offset = u2(raw, 8)
    utf8 = {}
    classes = {}
    index = 1
    while index < count:
        tag = raw[offset]
        offset += 1
        if tag == 1:
            size, offset = u2(raw, offset)
            encoded = raw[offset : offset + size].replace(b"\xc0\x80", b"\0")
            utf8[index] = encoded.decode("utf-8", "surrogatepass")
            offset += size
        elif tag == 7:
            classes[index], offset = u2(raw, offset)
        elif tag in (3, 4):
            offset += 4
        elif tag in (5, 6):
            offset += 8
            index += 1
        elif tag in (8, 16):
            offset += 2
        elif tag in (9, 10, 11, 12, 18):
            offset += 4
        elif tag == 15:
            offset += 3
        else:
            raise ValueError(f"unsupported constant tag {tag}")
        if offset > len(raw):
            raise ValueError("truncated constant pool")
        index += 1

    class_access, offset = u2(raw, offset)
    this_class, offset = u2(raw, offset)
    _, offset = u2(raw, offset)
    owner = utf8[classes[this_class]]
    interface_count, offset = u2(raw, offset)
    offset += interface_count * 2

    def skip_members(position):
        member_count, position = u2(raw, position)
        members = []
        for _ in range(member_count):
            access, position = u2(raw, position)
            name_index, position = u2(raw, position)
            descriptor_index, position = u2(raw, position)
            attribute_count, position = u2(raw, position)
            for _ in range(attribute_count):
                _, position = u2(raw, position)
                size, position = u4(raw, position)
                position += size
                if position > len(raw):
                    raise ValueError("truncated member attribute")
            members.append((access, utf8[name_index], utf8[descriptor_index]))
        return members, position

    _, offset = skip_members(offset)
    methods, _ = skip_members(offset)
    return owner, class_access & 1 != 0, [
        (name, descriptor) for access, name, descriptor in methods if access & 1
    ]


def validate(jar=JAR, manifest=MANIFEST):
    document = json.loads(manifest.read_text(encoding="utf-8"))
    if document.get("schema") != 1 or document.get("surface") != "cldc-bootstrap":
        raise ValueError("invalid method-fixture manifest header")
    archive_bytes = jar.read_bytes()
    if hashlib.sha256(archive_bytes).hexdigest() != document.get("archive_sha256"):
        raise ValueError("fixture archive SHA-256 mismatch")
    fixtures = document.get("fixtures")
    if not isinstance(fixtures, list) or not fixtures:
        raise ValueError("method-fixture manifest is empty")
    ids = [item.get("id") for item in fixtures]
    signatures = [item.get("signature") for item in fixtures]
    if len(ids) != len(set(ids)) or len(signatures) != len(set(signatures)):
        raise ValueError("duplicate method fixture")
    fixture_methods = set()
    with zipfile.ZipFile(io.BytesIO(archive_bytes)) as archive:
        platform_entries = [
            name for name in archive.namelist()
            if name.startswith("java/") or name.startswith("javax/")
        ]
        if platform_entries:
            raise ValueError(f"test fixture archive contains platform classes: {platform_entries[:3]}")
        owner, _, methods = public_methods(archive.read("fixtures/MethodFixtures.class"))
        fixture_methods = {f"{owner}::{name}{descriptor}" for name, descriptor in methods}
    entrypoints = [item.get("entrypoint") for item in fixtures]
    if len(entrypoints) != len(set(entrypoints)):
        raise ValueError("method fixtures do not have unique entrypoints")
    for index, item in enumerate(fixtures, 1):
        entrypoint = item.get("entrypoint")
        if entrypoint not in fixture_methods or not entrypoint.endswith("()I"):
            raise ValueError(f"method fixture has no executable checkpoint: {entrypoint}")
        if not isinstance(item.get("expected"), int):
            raise ValueError("method fixture has no integer expectation")
        expected_entrypoint = f"fixtures/MethodFixtures::method{index:04d}()I"
        if entrypoint != expected_entrypoint:
            raise ValueError(f"incorrect checkpoint for {item['signature']}")
    if set(entrypoints) != fixture_methods:
        missing = sorted(fixture_methods - set(entrypoints))
        extra = sorted(set(entrypoints) - fixture_methods)
        raise ValueError(f"method fixture mismatch: missing={missing}, extra={extra}")
    return len(fixtures)


def main(argv):
    try:
        count = validate()
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        print(f"invalid CLDC method fixtures: {error}", file=sys.stderr)
        return 1
    print(f"valid CLDC method fixtures: {count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
