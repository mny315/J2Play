#!/usr/bin/env python3
"""Move successful release builds into builds/<platform>/<architecture>/."""

import argparse
import errno
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from build_cache import managed_build


ROOT = Path(__file__).resolve().parents[1]


def publish(source, platform):
    if source.is_symlink():
        raise ValueError("Expected a build artifact, not a symbolic link")
    with source.open("rb") as incoming:
        if platform == "linux":
            header = incoming.read(20)
            if len(header) != 20 or header[:6] != b"\x7fELF\x02\x01":
                raise ValueError("Expected a 64-bit little-endian Linux executable")
            architecture = {62: "x86_64", 183: "aarch64"}.get(int.from_bytes(header[18:20], "little"))
            if architecture is None:
                raise ValueError("Unsupported Linux executable architecture")
            incoming.seek(0)
            name, mode = "j2play", 0o755
        else:
            architecture, name, mode = "arm64-v8a", "j2play.apk", 0o644
        destination = ROOT / "builds" / platform / architecture / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        os.fchmod(incoming.fileno(), mode)
        if source.resolve() == destination.parent.resolve() / destination.name:
            return destination
        try:
            source.replace(destination)
        except OSError as error:
            if error.errno != errno.EXDEV:
                raise
        else:
            # A fresh Cargo build can recreate its output as a hard link to
            # the already published binary. rename is a no-op for that pair.
            if source.exists() and source.samefile(destination):
                source.unlink()
            return destination

        # Custom target directories may live on a different filesystem. Keep
        # both the source and previous output until the copy is fully published.
        temporary_path = None
        try:
            with tempfile.NamedTemporaryFile(prefix=".j2play-", dir=destination.parent, delete=False) as temporary:
                temporary_path = Path(temporary.name)
                shutil.copyfileobj(incoming, temporary, length=1024 * 1024)
                os.fchmod(temporary.fileno(), mode)
            temporary_path.replace(destination)
            source.unlink()
        finally:
            if temporary_path is not None:
                temporary_path.unlink(missing_ok=True)
    return destination


@managed_build()
def build_linux():
    # Cargo reports the actual executable, including custom target directories
    # and target triples. Do not guess a path and accidentally move an old build.
    with tempfile.TemporaryFile(mode="w+t", encoding="utf-8") as messages:
        subprocess.run(
            ["cargo", "build", "--release", "-p", "j2play-linux", "--bin", "j2play",
             "--locked", "--message-format=json-render-diagnostics"],
            cwd=ROOT, stdout=messages, check=True,
        )
        messages.seek(0)
        executable = None
        for line in messages:
            message = json.loads(line)
            target = message.get("target", {})
            if (message.get("reason") == "compiler-artifact" and target.get("name") == "j2play"
                    and "bin" in target.get("kind", []) and message.get("executable")):
                executable = Path(message["executable"])
        if executable is None:
            raise ValueError("Cargo did not report the j2play executable")
    return publish(executable, "linux")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("linux", "android"))
    parser.add_argument("apk", type=Path, nargs="?")
    args = parser.parse_args()
    if (args.platform == "android") != (args.apk is not None):
        parser.error("android requires an APK path; linux builds through Cargo")
    try:
        destination = build_linux() if args.platform == "linux" else publish(args.apk, "android")
        print(destination)
        return 0
    except subprocess.CalledProcessError as error:
        print(f"Build failed (exit {error.returncode}); previous build outputs were kept.", file=sys.stderr)
        return error.returncode if error.returncode > 0 else 128 - error.returncode
    except (OSError, ValueError) as error:
        print(f"Cannot collect J2Play build: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130) from None
