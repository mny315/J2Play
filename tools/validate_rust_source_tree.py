#!/usr/bin/env python3
"""Enforce Rust application sources, crate ownership and native boundaries."""

from __future__ import annotations

import os
import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
IGNORED_TOP_LEVEL = {
    ".android-build",
    "android-build",
    "linux-build",
    "builds",
    ".git",
    ".agents",
    ".codex",
    ".direnv",
    "target",
    "out",
    "games",
    "games1",
    "done",
    "j2play-data",
    "logs",
    "test-results",
}
TOOLCHAIN_PACKAGE = re.compile(
    r"\b(?:jdk(?:[0-9]+)?(?:_headless)?|openjdk(?:[0-9_-]*)?|jre(?:_headless)?|proguard)\b",
    re.IGNORECASE,
)
TOOLCHAIN_COMMAND = re.compile(
    r"(?:^\s*|[;&|]\s*)(?:(?:env|command)\s+)?(?:[^\s;&|]*/)?"
    r"(?:javac|java|gradle|gradlew|kotlinc|kotlin)(?:\s|$)",
    re.MULTILINE,
)
ANDROID_ENTRY_CRATE = Path("crates/j2play-android")
ANDROID_EXPORT_SOURCE = ANDROID_ENTRY_CRATE / "src/lib.rs"
ANDROID_CALLBACK_EXPORT_SOURCE = ANDROID_ENTRY_CRATE / "src/android_adapter/ffi.rs"
ANDROID_EXPORT_ATTRIBUTE = "#[unsafe(no_mangle)]"
NATIVE_CODE_CRATE = Path("crates/native-code")
NATIVE_EXECUTABLE_SOURCE = NATIVE_CODE_CRATE / "src/executable.rs"
AMR_CRATE = Path("crates/amr-nb")
AMR_FFI_SOURCE = AMR_CRATE / "src/ffi.rs"
UNSAFE_RUST_OPERATION = re.compile(
    r"\bunsafe\s*(?:\{|fn\b|impl\b|extern\b|trait\b)|#\s*\[\s*unsafe\s*\("
)
RUST_TEST_ATTRIBUTE = re.compile(r"^\s*#\[\s*test\s*\]", re.MULTILINE)
ALLOWED_BUILD_HOST_TOOLCHAINS = {"jdk17_headless"}
RETIRED_FRONTEND_SCRIPTS = {Path("play-games.sh"), Path("tools/run-x11-game.sh")}
LINUX_ENTRY_CRATE = Path("crates/j2play-linux")
FORBIDDEN_SOURCE_SUFFIXES = {".java", ".kt", ".kts"}
FORBIDDEN_ANDROID_BUILD_FILES = {
    "build.gradle",
    "build.gradle.kts",
    "gradlew",
    "gradlew.bat",
    "settings.gradle",
    "settings.gradle.kts",
}


def repository_files(root: Path):
    for directory, subdirectories, filenames in os.walk(root):
        directory = Path(directory)
        if directory == root:
            # Prune SDKs, build products and user data before walking them.
            # Filtering rglob's results still traverses every ignored subtree.
            subdirectories[:] = [
                name for name in subdirectories if name not in IGNORED_TOP_LEVEL
            ]
        for filename in filenames:
            path = directory / filename
            if path.is_file():
                yield path.relative_to(root), path


def validate(root: Path = ROOT) -> int:
    violations = []
    inspected = 0
    workspace_path = root / "Cargo.toml"
    workspace_manifest = (
        tomllib.loads(workspace_path.read_text(encoding="utf-8"))
        if workspace_path.is_file() else {}
    )
    workspace_dependencies = workspace_manifest.get("workspace", {}).get("dependencies", {})
    conformance_root = (root / "tests/conformance").resolve()
    for relative, path in repository_files(root):
        inspected += 1
        if relative in RETIRED_FRONTEND_SCRIPTS:
            violations.append(f"retired frontend launcher: {relative}")
        if path.suffix.casefold() in FORBIDDEN_SOURCE_SUFFIXES:
            violations.append(f"source file: {relative}")
            continue
        if path.name.casefold() in FORBIDDEN_ANDROID_BUILD_FILES:
            violations.append(f"Android build system file: {relative}")
            continue
        if path.suffix == ".rs":
            text = path.read_text(encoding="utf-8")
            if relative.parts[:2] == ("tests", "conformance") and "CARGO_BIN_EXE_" in text:
                violations.append(f"conformance depends on a frontend executable: {relative}")
            if relative.parts[0] == "crates" and RUST_TEST_ATTRIBUTE.search(text):
                violations.append(f"Rust test outside tests/: {relative}")
            if relative.parts[: len(ANDROID_ENTRY_CRATE.parts)] == ANDROID_ENTRY_CRATE.parts:
                count = -1 if relative == ANDROID_CALLBACK_EXPORT_SOURCE else int(relative == ANDROID_EXPORT_SOURCE)
                stripped = text.replace(ANDROID_EXPORT_ATTRIBUTE, "", count)
                if UNSAFE_RUST_OPERATION.search(stripped):
                    violations.append(f"unsafe Android entry operation: {relative}")
            if (
                relative.parts[: len(NATIVE_CODE_CRATE.parts)] == NATIVE_CODE_CRATE.parts
                and relative != NATIVE_EXECUTABLE_SOURCE
                and UNSAFE_RUST_OPERATION.search(text)
            ):
                violations.append(f"unsafe numeric compiler operation: {relative}")
            if (
                relative.parts[: len(AMR_CRATE.parts)] == AMR_CRATE.parts
                and relative != AMR_FFI_SOURCE
                and UNSAFE_RUST_OPERATION.search(text)
            ):
                violations.append(f"unsafe operation outside AMR codec boundary: {relative}")
        if path.name.casefold() == "proguard.pro":
            violations.append(f"toolchain configuration: {relative}")
            continue
        if relative.as_posix() == "flake.nix":
            text = path.read_text(encoding="utf-8")
            disallowed = {
                match.group(0).casefold()
                for match in TOOLCHAIN_PACKAGE.finditer(text)
                if match.group(0).casefold() not in ALLOWED_BUILD_HOST_TOOLCHAINS
            }
            if disallowed:
                violations.append(
                    f"toolchain package: {relative} ({', '.join(sorted(disallowed))})"
                )
        if path.suffix == ".sh":
            text = path.read_text(encoding="utf-8")
            if TOOLCHAIN_COMMAND.search(text):
                violations.append(f"toolchain command: {relative}")
        if path.name == "Cargo.toml":
            manifest = tomllib.loads(path.read_text(encoding="utf-8"))
            if manifest.get("package", {}).get("name") == "j2play-cli":
                violations.append(f"retired frontend crate: {relative}")
            if relative.parent == LINUX_ENTRY_CRATE and manifest.get("bin") != [
                {"name": "j2play", "path": "src/main.rs"}
            ]:
                violations.append("Linux GUI must own the sole j2play executable")
            if relative.as_posix() == "tests/conformance/Cargo.toml":
                if (
                    manifest.get("package", {}).get("name") != "j2play-conformance"
                    or manifest.get("package", {}).get("publish") is not False
                    or manifest.get("test") != [{"name": "conformance", "path": "main.rs"}]
                    or any(key in manifest for key in ("bin", "lib", "example", "bench", "dependencies"))
                ):
                    violations.append("conformance must remain a single test-only Cargo target")
            if relative.parts[0] == "crates":
                tables = [manifest, *manifest.get("target", {}).values()]
                for table in tables:
                    for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
                        for name, dependency in table.get(kind, {}).items():
                            dependency_root = path.parent
                            if isinstance(dependency, dict) and dependency.get("workspace") is True:
                                dependency = workspace_dependencies.get(name, {})
                                dependency_root = root
                            package = dependency.get("package", name) if isinstance(dependency, dict) else name
                            if package in {"sdl2", "sdl2-sys"} and relative.parent != LINUX_ENTRY_CRATE:
                                violations.append(f"SDL dependency outside Linux shell: {relative}")
                            if name == "j2play-conformance" or (
                                isinstance(dependency, dict) and (
                                    dependency.get("package") == "j2play-conformance"
                                    or isinstance(dependency.get("path"), str)
                                    and (dependency_root / dependency["path"]).resolve() == conformance_root
                                )
                            ):
                                violations.append(f"product crate depends on conformance helpers: {relative}")
    if violations:
        raise ValueError(", ".join(sorted(violations)))
    return inspected


def main() -> int:
    try:
        count = validate()
    except (OSError, UnicodeError, ValueError) as error:
        print(f"invalid source tree boundary: {error}", file=sys.stderr)
        return 1
    print(f"valid source tree boundary: {count} files inspected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
