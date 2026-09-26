#!/usr/bin/env python3
"""Translate Cargo's clang target flag to the locked Zig/glibc target."""

import os
from pathlib import Path
import sys


def target_arguments(arguments):
    filtered = []
    skip = False
    for argument in arguments:
        if skip:
            skip = False
        elif argument in ("-target", "--target"):
            skip = True
        elif not argument.startswith("--target="):
            filtered.append(argument)
    if skip:
        raise ValueError("Missing compiler target value")
    return filtered


def main():
    zig, language, target, sysroot, multiarch, *arguments = sys.argv[1:]
    arguments = target_arguments(arguments)
    # Rust's default #[link] requests dynamic lookup. GNU ld falls back to an
    # archive, but Zig requires the exact archive when no shared object exists.
    arguments = [os.environ["J2PLAY_LINUX_AMR_ARCHIVE"] if value == "-lopencore-amrnb" else value
                 for value in arguments]
    if arguments == ["-dumpmachine"]:
        print(target.split(".", 1)[0])
        return
    command = [zig, language, *arguments, "-target", target, "-mcpu=baseline",
               "-isystem", str(Path(sysroot) / "usr/include"),
               "-isystem", str(Path(sysroot) / "usr/include" / multiarch)]
    os.execv(zig, command)


if __name__ == "__main__":
    main()
