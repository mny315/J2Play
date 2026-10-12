#!/usr/bin/env python3
"""Prepare and build with the pinned, Nix-independent Linux SDK."""

import argparse
import fcntl
import hashlib
import inspect
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tempfile

import linux_sources as sources
from build_cache import managed_build

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / "crates/j2play-linux/build-support"
BUILD = ROOT / "linux-build"
MANIFEST = SUPPORT / "sdk.json"
ARCHITECTURES = ("x86_64", "aarch64")


def run(command, **kwargs):
    print(shlex.join(map(str, command)), flush=True)
    return subprocess.run(list(map(str, command)), check=True, **kwargs)


def manifest():
    value = json.loads(MANIFEST.read_text())
    if value.get("schema") != 1 or tuple(value["targets"]) != ARCHITECTURES:
        raise ValueError("Unsupported Linux SDK manifest")
    return value


def prepare(offline=False):
    spec = manifest()
    host = platform.machine()
    if sys.platform != "linux" or host not in spec["hosts"]:
        raise ValueError("The portable SDK needs a Linux x86_64 or aarch64 build host")
    fingerprint = hashlib.sha256(MANIFEST.read_bytes() + inspect.getsource(prepare).encode()
                                 + Path(sources.__file__).read_bytes()).hexdigest()
    sdk = BUILD / "sdk" / fingerprint / host
    sdk.parent.mkdir(parents=True, exist_ok=True)
    with (BUILD / ".sdk.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if sdk.exists():
            sources.verify(sdk)
            return sdk
        with tempfile.TemporaryDirectory(prefix=".prepare-", dir=sdk.parent) as temporary:
            staging = Path(temporary) / "sdk"
            staging.mkdir()
            for name, item in spec["hosts"][host].items():
                unpack = Path(temporary) / name
                sources.extract_tar(sources.download(item, BUILD / "downloads", offline), unpack)
                roots = list(unpack.iterdir())
                if len(roots) != 1:
                    raise ValueError("Unexpected Linux SDK archive layout")
                if name == "zig":
                    shutil.copytree(roots[0], staging / "zig", symlinks=True)
                else:
                    shutil.copytree(roots[0] / name, staging / "rust", dirs_exist_ok=True, symlinks=True)
            for arch, target in spec["targets"].items():
                unpack = Path(temporary) / ("std-" + arch)
                sources.extract_tar(sources.download(target["rust_std"], BUILD / "downloads", offline), unpack)
                roots = list(unpack.iterdir())
                if len(roots) != 1:
                    raise ValueError("Unexpected Rust standard library layout")
                shutil.copytree(roots[0] / ("rust-std-" + target["triple"]), staging / "rust",
                                dirs_exist_ok=True, symlinks=True)
                for package in target["packages"]:
                    archive = sources.download(package, BUILD / "downloads", offline)
                    sources.extract_tar(sources.deb_data(archive), staging / "sysroots" / arch)
            for name, item in spec["sources"].items():
                sources.extract_tar(sources.download(item, BUILD / "downloads", offline), staging / "sources" / name)
            sources.seal(staging)
            staging.rename(sdk)
    return sdk


def write_toolchain_file(path, data, mode=0o644):
    # Cargo fingerprints compiler timestamps, and CMake tracks its toolchain
    # file. Identical setup must preserve them across builds and architectures.
    if path.is_file() and not path.is_symlink():
        metadata = path.stat()
        if metadata.st_size == len(data) and metadata.st_mode & 0o777 == mode and path.read_bytes() == data:
            return
    sources.atomic_write(path, data, mode)


def script(path, command):
    write_toolchain_file(path, ("#!/bin/sh\nset -eu\nexec " + shlex.join(map(str, command))
                                + ' "$@"\n').encode(), 0o755)


def environment(sdk, arch):
    spec = manifest()
    target = spec["targets"][arch]
    triple = target["triple"]
    generation = hashlib.sha256(str(sdk).encode() + Path(__file__).read_bytes()
                                + (ROOT / "tools/linux_cc.py").read_bytes()).hexdigest()
    work = BUILD / "toolchains" / generation / arch
    work.mkdir(parents=True, exist_ok=True)
    sysroot = sdk / "sysroots" / arch
    multiarch = arch + "-linux-gnu"
    zig_target = arch + "-linux-gnu." + spec["glibc"]
    for name, language in (("cc", "cc"), ("c++", "c++")):
        script(work / name, [sys.executable, ROOT / "tools/linux_cc.py", sdk / "zig/zig",
                            language, zig_target, sysroot, multiarch])
    for name in ("ar", "ranlib"):
        script(work / name, [sdk / "zig/zig", name])
    env = dict(os.environ)
    for key in list(env):
        if (key.startswith(("CARGO_TARGET_", "CARGO_ENCODED_", "PKG_CONFIG", "CMAKE_"))
                or key in ("RUSTFLAGS", "RUSTDOCFLAGS", "CC", "CXX", "AR", "RANLIB",
                           "CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS", "LIBRARY_PATH",
                           "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "SDL2_TOOLCHAIN")):
            del env[key]
    # Use the official compiler and std together: Nix's source-built compiler
    # can have incompatible metadata despite the identical Rust version.
    # NixOS has no standard ELF interpreter; invoke its host loader explicitly.
    # This wrapper is build-host tooling and is never copied into the payload.
    nix_host = env.get("J2PLAY_DEV_SHELL") == "1"
    cargo, rustc = (str(sdk / "rust/bin" / name) for name in ("cargo", "rustc"))
    if nix_host:
        host_root = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
        host_rustc = host_root / "bin/rustc"
        loader = subprocess.check_output(["patchelf", "--print-interpreter", str(host_rustc)], text=True).strip()
        dependencies = subprocess.check_output(["ldd", str(host_rustc)], text=True)
        host_libraries = sorted({str(Path(path).parent) for path in re.findall(r"=> (/\S+)", dependencies)})
        libraries = os.pathsep.join([str(sdk / "rust/lib"), *host_libraries])
        script(work / "cargo", [loader, "--library-path", libraries, sdk / "rust/bin/cargo"])
        compiler = shlex.join([loader, "--library-path", libraries, str(sdk / "rust/bin/rustc")])
        host_flags = shlex.join(["-Clinker=" + shutil.which("cc"),
                                *(["-Clinker-features=-lld"] if platform.machine() == "x86_64" else []),
                                "-Clink-arg=-Wl,--dynamic-linker," + loader,
                                "-Clink-arg=-Wl,-rpath," + libraries])
        write_toolchain_file(work / "rustc", (
            '#!/bin/sh\nset -eu\nfor argument in "$@"; do\n'
            '  case "$argument" in --target|--target=*) exec ' + compiler + ' "$@" ;; esac\ndone\n'
            'exec ' + compiler + ' "$@" ' + host_flags + '\n').encode(), 0o755)
        cargo, rustc = (str(work / name) for name in ("cargo", "rustc"))
    version = subprocess.check_output([rustc, "--version"], text=True)
    if not version.startswith("rustc " + spec["rust_version"] + " "):
        raise ValueError("The Linux build requires the pinned Rust version")
    env["RUSTC"] = rustc
    env["CARGO"] = cargo
    env["CARGO_HOME"] = str(BUILD / "cargo")
    # Cargo also fingerprints the target linker for host build scripts and
    # proc macros. Keep those outputs apart when switching architectures.
    env["CARGO_TARGET_DIR"] = str(BUILD / "target" / generation / arch)
    env["CARGO_BUILD_JOBS"] = str(min(os.cpu_count() or 2, 12))
    env["J2PLAY_LINUX_AMR_ARCHIVE"] = str(work / "amr/lib/libopencore-amrnb.a")
    env["ZIG_GLOBAL_CACHE_DIR"] = str(BUILD / "zig-cache")
    env["ZIG_LOCAL_CACHE_DIR"] = str(BUILD / "zig-local-cache" / arch)
    env["PKG_CONFIG_ALLOW_CROSS"] = "1"
    env["PKG_CONFIG_SYSROOT_DIR"] = str(sysroot)
    env["PKG_CONFIG_LIBDIR"] = os.pathsep.join(map(str, [sysroot / "usr/lib" / multiarch / "pkgconfig", sysroot / "usr/share/pkgconfig"]))
    env["PKG_CONFIG_PATH"] = ""
    env["CMAKE_POLICY_VERSION_MINIMUM"] = "3.5"
    for key, name in (("CC", "cc"), ("CXX", "c++"), ("AR", "ar"), ("RANLIB", "ranlib")):
        env[key + "_" + triple.replace("-", "_")] = str(work / name)
    options = [
        'set(CMAKE_SYSTEM_NAME Linux)',
        f'set(CMAKE_SYSTEM_PROCESSOR {arch})',
        f'set(CMAKE_C_COMPILER "{work / "cc"}")',
        f'set(CMAKE_CXX_COMPILER "{work / "c++"}")',
        f'set(CMAKE_AR "{work / "ar"}")',
        f'set(CMAKE_RANLIB "{work / "ranlib"}")',
        'set(CMAKE_C_STANDARD 99)',
        f'set(CMAKE_FIND_ROOT_PATH "{sysroot}")',
        'set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)',
        'set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)',
        'set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)',
        'set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)',
    ]
    for key in ("SDL_ALSA", "SDL_ALSA_SHARED", "SDL_PULSEAUDIO", "SDL_PULSEAUDIO_SHARED"):
        options.append(f'set({key} ON CACHE BOOL "" FORCE)')
    for key in ("SDL_VIDEO", "SDL_RENDER", "SDL_TEST", "SDL_PIPEWIRE", "SDL_JACK", "SDL_KMSDRM"):
        options.append(f'set({key} OFF CACHE BOOL "" FORCE)')
    write_toolchain_file(work / "sdl.cmake", ("\n".join(options) + "\n").encode())
    env["SDL2_TOOLCHAIN"] = str(work / "sdl.cmake")
    key = "CARGO_TARGET_" + triple.upper().replace("-", "_")
    env[key + "_LINKER"] = str(work / "cc")
    flags = ["--sysroot", str(sdk / "rust"),
             *(["-Clinker-features=-lld"] if arch == "x86_64" else []),
             "-Lnative=" + str(work / "amr/lib"),
             "-Cstrip=symbols", "--remap-path-prefix=" + str(ROOT) + "=/j2play",
             "--remap-path-prefix=" + str(Path.home()) + "=/build-home"]
    # Cargo splits target RUSTFLAGS on whitespace without shell unquoting.
    # Every build supplies --target, so encoded flags still apply only to
    # target crates and preserve spaces in SDK and checkout paths.
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    return cargo, env, work


def build_amr(sdk, work, env):
    destination = work / "amr"
    if destination.exists():
        sources.verify(destination)
        return
    with tempfile.TemporaryDirectory(prefix=".amr-", dir=work) as temporary:
        source = Path(temporary) / "source"
        shutil.copytree(sdk / "sources/opencore-amr/opencore-amr-0.1.6", source)
        build_env = dict(env)
        # Autoconf expands compiler variables without preserving embedded
        # spaces. Resolve the wrappers through PATH instead of splitting paths.
        build_env["PATH"] = str(work) + os.pathsep + env.get("PATH", os.defpath)
        for key, name in (("CC", "cc"), ("CXX", "c++"), ("AR", "ar"), ("RANLIB", "ranlib")):
            build_env[key] = name
        build_env["CFLAGS"] = "-O2 -fPIC"
        build_env["CXXFLAGS"] = "-O2 -fPIC"
        run([source / "configure", "--host=" + work.name + "-linux-gnu", "--disable-shared",
             "--enable-static"], cwd=source, env=build_env)
        run(["make", "-C", "amrnb", "-j" + env["CARGO_BUILD_JOBS"]], cwd=source, env=build_env)
        # The linker consumes only this static archive. Libtool's install step
        # splits destination paths with spaces and installs unused SDK files.
        output = Path(temporary) / "output"
        (output / "lib").mkdir(parents=True)
        shutil.copy2(source / "amrnb/.libs/libopencore-amrnb.a", output / "lib")
        sources.seal(output)
        output.rename(destination)


@managed_build()
def build(arch, offline=False, interpreter_only=False):
    sdk = prepare(offline)
    cargo, env, work = environment(sdk, arch)
    for tool in ("cmake", "make", "pkg-config", "patch"):
        if shutil.which(tool) is None:
            raise ValueError(f"Linux SDK build prerequisite is missing: {tool}")
    run([sys.executable, ROOT / "tools/prepare_ui_sources.py", "--cargo-home", env["CARGO_HOME"]]
        + (["--offline"] if offline else []), cwd=ROOT, env=env)
    run([sys.executable, ROOT / "tools/prepare_ui_sources.py", "--check-cargo"], cwd=ROOT, env=env)
    build_amr(sdk, work, env)
    command = [cargo, "build", "--release", "--locked", "-p", "j2play-linux", "--bin", "j2play",
               "--target", manifest()["targets"][arch]["triple"], "--message-format=json-render-diagnostics"]
    if offline:
        command.append("--offline")
    if interpreter_only:
        command.append("--no-default-features")
    with tempfile.TemporaryFile(mode="w+t", encoding="utf-8") as output:
        run(command, cwd=ROOT, env=env, stdout=output)
        output.seek(0)
        executable = None
        for line in output:
            message = json.loads(line)
            target = message.get("target", {})
            if (message.get("reason") == "compiler-artifact" and target.get("name") == "j2play"
                    and "bin" in target.get("kind", []) and message.get("executable")):
                executable = Path(message["executable"])
        if executable is None:
            raise ValueError("Cargo did not report the Linux executable")
    return executable, env, sdk


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "build"))
    parser.add_argument("architecture", nargs="?", choices=ARCHITECTURES, default=platform.machine())
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--interpreter-only", action="store_true")
    args = parser.parse_args()
    if args.action == "prepare":
        print(prepare(args.offline))
    else:
        print(build(args.architecture, args.offline, args.interpreter_only)[0])


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Linux SDK: {error}", file=sys.stderr)
        raise SystemExit(1) from None
