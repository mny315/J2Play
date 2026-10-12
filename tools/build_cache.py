#!/usr/bin/env python3
"""Bound retained Cargo artifacts in the project's standard build directories."""

import argparse
from contextlib import contextmanager
import fcntl
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GIB = 24
GIB = 1024 ** 3
CARGO_DIRECTORIES = ("incremental", "deps", "build", ".fingerprint", "examples")
ACTIVE = "J2PLAY_BUILD_CACHE_ACTIVE"


def profiles(root):
    """Recognize Cargo profiles without descending into SDKs or user data."""
    targets = [root / name for name in ("target", "android-build/target", ".android-build/target")]
    portable = root / "linux-build/target"
    if portable.resolve() == portable and portable.is_dir():
        for generation in portable.iterdir():
            if re.fullmatch(r"[0-9a-f]{64}", generation.name):
                targets.extend((generation, generation / "x86_64", generation / "aarch64"))
    for target in targets:
        if target.resolve() != target or not target.is_dir():
            continue
        containers = [target, *(child for child in target.iterdir()
                               if re.fullmatch(r"(?:x86_64|aarch64|armv7|arm|i686)-[a-z0-9_.-]+", child.name))]
        for container in containers:
            for name in ("debug", "release"):
                path = container / name
                if path.resolve() == path and (path / ".cargo-lock").is_file():
                    yield path


def inventory(path):
    """Count allocated bytes once per inode, including Cargo's hard-linked binaries."""
    total, modified = 0, 0
    seen = set()
    for directory, children, files in os.walk(path, followlinks=False):
        parent = Path(directory)
        children[:] = [name for name in children if not (parent / name).is_symlink()]
        for name in files:
            try:
                info = (parent / name).lstat()
            except FileNotFoundError:
                continue
            identity = (info.st_dev, info.st_ino)
            if not stat.S_ISREG(info.st_mode) or identity in seen:
                continue
            seen.add(identity)
            total += info.st_blocks * 512
            modified = max(modified, info.st_mtime_ns)
    return total, modified


def clear_profile(path, names):
    # Cargo uses this same flock while compiling. Retain the lock inode so an
    # already waiting Cargo process cannot acquire a different replacement lock.
    descriptor = os.open(path / ".cargo-lock", os.O_RDWR | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "r+b") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return False
        for name in names:
            child = path / name
            if child.is_symlink():
                child.unlink()
            elif child.is_dir():
                shutil.rmtree(child)
    return True


def trim(root=ROOT, budget=DEFAULT_GIB * GIB):
    before = 0
    records = []
    # Incremental compilation is disabled by the workspace profile. Retire its
    # old snapshots first; complete compiled dependencies remain reusable.
    for path in profiles(root):
        info = inventory(path)
        before += info[0]
        if (path / "incremental").exists() and clear_profile(path, ("incremental",)):
            info = inventory(path)
        records.append((info, path))
    total = sum(info[0] for info, _ in records)
    for (size, _), path in sorted(records, key=lambda record: record[0][1]):
        if total <= budget:
            break
        if clear_profile(path, CARGO_DIRECTORIES):
            total += inventory(path)[0] - size
    if before > total:
        print(f"Build cache: released {(before - total) / GIB:.2f} GiB; "
              f"{total / GIB:.2f}/{budget / GIB:g} GiB retained", file=sys.stderr)
    if total > budget:
        raise ValueError("Build cache exceeds its budget; remaining artifacts are in use or "
                         "outside Cargo scratch directories. Stop other builds before retrying.")
    return total


@contextmanager
def managed_build():
    if os.environ.get(ACTIVE) == str(ROOT):
        yield
        return
    limit = int(os.environ.get("J2PLAY_BUILD_CACHE_GIB", DEFAULT_GIB))
    if limit < 1:
        raise ValueError("J2PLAY_BUILD_CACHE_GIB must be a positive integer")
    target = ROOT / "target"
    if target.resolve() != target:
        raise ValueError("Build-cache management requires a non-symlink project target directory")
    target.mkdir(exist_ok=True)
    descriptor = os.open(target / ".j2play-build-cache.lock",
                         os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "r+b") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        trim(ROOT, limit * GIB)
        previous = os.environ.get(ACTIVE)
        os.environ[ACTIVE] = str(ROOT)
        try:
            yield
        finally:
            if previous is None:
                os.environ.pop(ACTIVE, None)
            else:
                os.environ[ACTIVE] = previous
            trim(ROOT, limit * GIB)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--trim", action="store_true", help="trim idle build caches without building")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not args.trim and not command:
        parser.error("supply --trim or -- followed by a build command")
    with managed_build():
        if command:
            result = subprocess.run(command, cwd=ROOT, check=False)
            return result.returncode if result.returncode >= 0 else 128 - result.returncode
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print(f"Build cache: {error}", file=sys.stderr)
        raise SystemExit(1) from None
    except KeyboardInterrupt:
        raise SystemExit(130) from None
