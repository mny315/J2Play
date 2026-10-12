#!/usr/bin/env python3
"""Create and validate the portable Linux AppDir payload."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile

import linux_sdk as sdk
import linux_sources as sources
import package_legal
from build_cache import managed_build

APP_ID = "io.github.mny315.j2play"
GLIBC_LIBRARIES = {"libc.so.6", "libm.so.6", "libdl.so.2", "libpthread.so.0", "librt.so.1",
                   "libutil.so.1", "libresolv.so.2", "libgcc_s.so.1",
                   "ld-linux-x86-64.so.2", "ld-linux-aarch64.so.1"}
INTERPRETERS = {"x86_64": "/lib64/ld-linux-x86-64.so.2", "aarch64": "/lib/ld-linux-aarch64.so.1"}


def readelf(path, *options):
    return subprocess.check_output(["readelf", "--wide", *options, str(path)], text=True,
                                   env={**os.environ, "LC_ALL": "C"})


def elf_contract(path, arch):
    with path.open("rb") as source:
        header = source.read(20)
    if (header[:6] != b"\x7fELF\x02\x01" or len(header) != 20
            or int.from_bytes(header[18:20], "little") != {"x86_64": 62, "aarch64": 183}[arch]):
        raise ValueError(f"Wrong ELF architecture: {path}")
    program = readelf(path, "-l")
    interpreter = re.findall(r"Requesting program interpreter: ([^\]]+)", program)
    if interpreter != [INTERPRETERS[arch]]:
        raise ValueError(f"Nonportable ELF interpreter: {interpreter}")
    dynamic = readelf(path, "-d")
    needed = re.findall(r"\(NEEDED\).*\[([^\]]+)\]", dynamic)
    if set(needed) - GLIBC_LIBRARIES:
        raise ValueError(f"Undeclared direct runtime dependencies: {set(needed) - GLIBC_LIBRARIES}")
    paths = re.findall(r"\((?:RUNPATH|RPATH)\).*\[([^\]]*)\]", dynamic)
    if any(value for value in paths):
        raise ValueError(f"Unexpected runtime search path: {paths}")
    versions = readelf(path, "--version-info")
    glibc = {tuple(map(int, value.split("."))) for value in re.findall(r"Name: GLIBC_([0-9.]+)", versions)}
    maximum = tuple(map(int, sdk.manifest()["glibc"].split(".")))
    if any(version > maximum for version in glibc) or re.search(r"Name: (GLIBCXX_|CXXABI_|GLIBC_PRIVATE)", versions):
        raise ValueError("The payload exceeds its declared glibc/C++ ABI")
    forbidden = (b"/nix/store/", str(sdk.ROOT).encode(), str(sdk.BUILD).encode())
    overlap = b""
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            block = overlap + chunk
            if any(value in block for value in forbidden):
                raise ValueError("The payload contains a development environment path")
            overlap = block[-4096:]
    return {"architecture": arch, "interpreter": interpreter[0], "needed": sorted(needed),
            "maximum_glibc": ".".join(map(str, max(glibc, default=(0,))))}


def checksums(root):
    lines = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError("Published AppDir payload must not contain symbolic links")
        if path.is_file() and path != root / "SHA256SUMS":
            digest = hashlib.sha256()
            with path.open("rb") as source:
                while chunk := source.read(1024 * 1024):
                    digest.update(chunk)
            lines.append(digest.hexdigest() + "  " + path.relative_to(root).as_posix())
    return "\n".join(lines) + "\n"


def validate(root, arch):
    if (root / "SHA256SUMS").read_text() != checksums(root):
        raise ValueError("Linux package checksum inventory mismatch")
    required = ["J2Play", "install.sh", "build-id", "usr/bin/j2play",
                f"usr/share/mime/packages/{APP_ID}.xml",
                f"usr/share/applications/{APP_ID}.desktop", f"usr/share/metainfo/{APP_ID}.metainfo.xml",
                f"usr/share/icons/hicolor/192x192/apps/{APP_ID}.png",
                "usr/share/doc/j2play/README.md", "usr/share/doc/j2play/Cargo.lock",
                "usr/share/doc/j2play/sdk.json", "usr/share/doc/j2play/legal-sources.json",
                "usr/share/doc/j2play/ui-sources.json", "usr/share/doc/j2play/legal/LICENSE",
                "usr/share/doc/j2play/legal/NOTICE", "usr/share/doc/j2play/legal/dependencies.json",
                "usr/share/doc/j2play/legal/THIRD_PARTY_NOTICES.txt"]
    if any(not (root / name).is_file() for name in required):
        raise ValueError("Linux package is missing a required integration or legal file")
    for name in ("J2Play", "install.sh", "usr/bin/j2play"):
        if (root / name).stat().st_mode & 0o111 != 0o111:
            raise ValueError(f"Linux package launcher is not executable: {name}")
    for path in root.rglob("*"):
        if path.is_file() and path.suffix in (".jar", ".jad", ".rms", ".log"):
            raise ValueError("Linux payload contains guest archives or runtime data")
    return elf_contract(root / "usr/bin/j2play", arch)


def copy_build_manifests(destination):
    for source, name in (
            (sdk.ROOT / "Cargo.lock", "Cargo.lock"),
            (sdk.MANIFEST, "sdk.json"),
            (package_legal.SOURCES, "legal-sources.json"),
            (sdk.ROOT / "crates/frontend-ui/build-support/sources.json", "ui-sources.json")):
        shutil.copyfile(source, destination / name)


def archive(root, destination):
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as contents:
                    for path in [root] + sorted(root.rglob("*")):
                        info = contents.gettarinfo(path, "J2Play.AppDir" + ("/" + path.relative_to(root).as_posix() if path != root else ""))
                        info.uid = info.gid = 0
                        info.uname = info.gname = ""
                        info.mtime = 0
                        if info.isfile():
                            with path.open("rb") as incoming:
                                contents.addfile(info, incoming)
                        else:
                            contents.addfile(info)
            output.flush()
            os.fsync(output.fileno())
            temporary.chmod(0o644)
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)


def ports_entries(root, arch, build_id):
    metadata = {"version": 4, "name": "j2play.zip", "items": ["J2Play.sh", "j2play"],
                "items_opt": [], "attr": {"title": "J2Play", "porter": ["mny315"],
                "desc": "Independent Java ME emulator. Import your own MIDlet archives.",
                "desc_md": None, "inst": "Experimental desktop-services package: requires Wayland/X11, compatible Vulkan/EGL and logind. Includes an in-window file picker; external links require an OpenURI portal. Read the compatibility matrix before installing.",
                "inst_md": None, "genres": [], "image": None, "rtr": False, "exp": True,
                "runtime": [], "store": [], "availability": "full", "reqs": [],
                "arch": [arch], "min_glibc": sdk.manifest()["glibc"]}}
    return {
        "J2Play.sh": ((sdk.SUPPORT / "ports-launcher.sh").read_text().replace("@BUILD_ID@", build_id).encode(), 0o755),
        "j2play/port.json": ((json.dumps(metadata, indent=2) + "\n").encode(), 0o644),
        "j2play/README.md": ((root / "usr/share/doc/j2play/README.md").read_bytes(), 0o644),
        "j2play/gameinfo.xml": (b'<?xml version="1.0" encoding="UTF-8"?>\n<gameList><game><path>./J2Play.sh</path><name>J2Play</name><desc>Independent Java ME emulator. Games are not included.</desc><developer>J2Play contributors</developer><publisher>Open Source</publisher><genre>Emulator</genre></game></gameList>\n', 0o644),
    }


def ports_archive(root, output, arch, build_id):
    """Package the same payload; updates add a version and retain SD-card data."""
    with tempfile.NamedTemporaryFile(dir=output, delete=False) as temporary:
        path = Path(temporary.name)
        try:
            with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED) as contents:
                for name, (data, mode) in ports_entries(root, arch, build_id).items():
                    info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                    info.external_attr = (0o100000 | mode) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    contents.writestr(info, data)
                for source in sorted(root.rglob("*")):
                    if source.is_file():
                        name = f"j2play/versions/{build_id}/" + source.relative_to(root).as_posix()
                        info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                        metadata = source.stat()
                        info.external_attr = (0o100000 | (metadata.st_mode & 0o777)) << 16
                        info.compress_type = zipfile.ZIP_DEFLATED
                        info.file_size = metadata.st_size
                        with source.open("rb") as incoming, contents.open(info, "w") as outgoing:
                            shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
            temporary.flush()
            os.fsync(temporary.fileno())
            path.chmod(0o644)
            path.replace(output / ("j2play-ports-" + arch + ".zip"))
        finally:
            path.unlink(missing_ok=True)


def same_bytes(left, right):
    """Compare streams without retaining complete executables or rewinding gzip."""
    while block := right.read(1024 * 1024):
        if left.read(len(block)) != block:
            return False
    return not left.read(1)


def validate_tar(root, archive, files):
    prefix = "J2Play.AppDir/"
    directories = {"J2Play.AppDir"} | {
        prefix + path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_dir()
    }
    seen = set()
    # Read each compressed byte once, in archive order. Random access after
    # getmembers() repeatedly decompresses the executable for later notices.
    with tarfile.open(archive, "r|gz") as contents:
        for member in contents:
            if member.name in seen or not (member.isfile() or member.isdir()):
                raise ValueError("Linux tar contains duplicate paths, links or special files")
            seen.add(member.name)
            if member.isdir():
                if member.name not in directories:
                    raise ValueError("Linux tar directory inventory differs from the AppDir")
                continue
            path = files.get(member.name.removeprefix(prefix)) if member.name.startswith(prefix) else None
            if path is None:
                raise ValueError("Linux tar file inventory differs from the AppDir")
            metadata = path.stat()
            if member.size != metadata.st_size or member.mode != metadata.st_mode & 0o777:
                raise ValueError(f"Linux tar differs from the AppDir: {member.name}")
            with contents.extractfile(member) as incoming, path.open("rb") as expected:
                if not same_bytes(incoming, expected):
                    raise ValueError(f"Linux tar differs from the AppDir: {member.name}")
    if seen != directories | {prefix + name for name in files}:
        raise ValueError("Linux tar inventory differs from the AppDir")


def validate_archives(root, output, arch):
    """Check the actual delivered bytes and executable modes, including Ports."""
    files = {path.relative_to(root).as_posix(): path for path in root.rglob("*") if path.is_file()}
    validate_tar(root, output / ("j2play-linux-" + arch + ".tar.gz"), files)
    build_id = (root / "build-id").read_text().strip()
    prefix = f"j2play/versions/{build_id}/"
    expected = ports_entries(root, arch, build_id)
    with zipfile.ZipFile(output / ("j2play-ports-" + arch + ".zip")) as contents:
        names = contents.namelist()
        if len(set(names)) != len(names) or set(names) != set(expected) | {prefix + name for name in files}:
            raise ValueError("Ports archive file inventory differs from the AppDir")
        for name in names:
            info = contents.getinfo(name)
            if name in expected:
                data, mode = expected[name]
                size = len(data)
                source = io.BytesIO(data)
            else:
                path = files[name.removeprefix(prefix)]
                metadata = path.stat()
                size, mode = metadata.st_size, metadata.st_mode & 0o777
                source = path.open("rb")
            with source, contents.open(info) as incoming:
                if (info.file_size != size or info.external_attr >> 16 != 0o100000 | mode
                        or not same_bytes(incoming, source)):
                    raise ValueError(f"Ports archive differs from the AppDir: {name}")


def publish(staging, output, arch):
    """Replace only finished artifacts, restoring the previous set on failure."""
    names = ("J2Play.AppDir", f"j2play-linux-{arch}.tar.gz", f"j2play-ports-{arch}.zip")
    output.mkdir(parents=True, exist_ok=True)
    # Keep backups outside the staging context: if restoration itself fails,
    # its cleanup must not remove the last successful package.
    backup = Path(tempfile.mkdtemp(prefix=".previous-", dir=staging.parent))
    previous, published = [], []
    try:
        for name in names:
            destination = output / name
            if destination.exists() or destination.is_symlink():
                destination.rename(backup / name)
                previous.append(name)
            (staging / name).rename(destination)
            published.append(name)
    except BaseException:
        try:
            for name in reversed(published):
                (output / name).rename(staging / name)
            for name in reversed(previous):
                (backup / name).rename(output / name)
        except OSError as error:
            raise OSError(f"Cannot restore the previous Linux package; retained files: {backup}") from error
        shutil.rmtree(backup)
        raise
    shutil.rmtree(backup)


@managed_build()
def package(arch, offline=False):
    executable, env, prepared = sdk.build(arch, offline)
    contract = elf_contract(executable, arch)
    output = sdk.ROOT / "builds/linux" / arch
    work = sdk.BUILD / "packages" / arch
    work.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".package-", dir=work) as temporary:
        appdir = Path(temporary) / "J2Play.AppDir"
        for name in ("usr/bin", "usr/share/applications", "usr/share/metainfo",
                     "usr/share/mime/packages",
                     "usr/share/icons/hicolor/192x192/apps", "usr/share/doc/j2play/legal"):
            (appdir / name).mkdir(parents=True, exist_ok=True)
        shutil.copyfile(executable, appdir / "usr/bin/j2play")
        for name in ("J2Play", "install.sh"):
            shutil.copyfile(sdk.SUPPORT / name, appdir / name)
        for name in ("J2Play", "install.sh", "usr/bin/j2play"):
            (appdir / name).chmod(0o755)
        for directory, suffix in (("applications", "desktop"), ("metainfo", "metainfo.xml")):
            shutil.copyfile(sdk.SUPPORT / (APP_ID + "." + suffix), appdir / "usr/share" / directory / (APP_ID + "." + suffix))
        shutil.copyfile(sdk.SUPPORT / (APP_ID + ".mime.xml"), appdir / "usr/share/mime/packages" / (APP_ID + ".xml"))
        shutil.copyfile(sdk.ROOT / "crates/j2play-android/res/mipmap-xxxhdpi/ic_launcher.png",
                        appdir / "usr/share/icons/hicolor/192x192/apps" / (APP_ID + ".png"))
        shutil.copyfile(sdk.ROOT / "README.md", appdir / "usr/share/doc/j2play/README.md")
        cargo = env["CARGO"]
        package_legal.cargo_notices(
            appdir / "usr/share/doc/j2play/legal", cargo, env, "j2play-linux",
            sdk.manifest()["targets"][arch]["triple"], sdk.BUILD / "downloads/licenses", offline)
        legal = appdir / "usr/share/doc/j2play/legal"
        rust_notices = prepared / "rust/share/doc/rust"
        shutil.copytree(rust_notices / "licenses", legal / "rust/licenses")
        shutil.copyfile(rust_notices / "COPYRIGHT-library.html", legal / "rust/COPYRIGHT-library.html")
        for origin, name in (
                ("sources/opencore-amr/opencore-amr-0.1.6/LICENSE", "opencore-amr/LICENSE"),
                ("sources/opencore-amr/opencore-amr-0.1.6/opencore/NOTICE", "opencore-amr/NOTICE"),
                ("zig/LICENSE", "zig/LICENSE"),
                ("zig/lib/libc/glibc/LICENSES", "glibc-startup/LICENSES"),
                ("zig/lib/libunwind/LICENSE.TXT", "libunwind/LICENSE.TXT")):
            sources.atomic_write(legal / name, (prepared / origin).read_bytes())
        copy_build_manifests(appdir / "usr/share/doc/j2play")
        contract.update({"sdk_sha256": sources.sha256(sdk.MANIFEST.read_bytes()),
                         "cargo_lock_sha256": sources.sha256((sdk.ROOT / "Cargo.lock").read_bytes())})
        sources.atomic_write(appdir / "usr/share/doc/j2play/build.json", (json.dumps(contract, indent=2) + "\n").encode())
        build_id = sources.sha256(checksums(appdir).encode())
        sources.atomic_write(appdir / "build-id", (build_id + "\n").encode())
        sources.atomic_write(appdir / "SHA256SUMS", checksums(appdir).encode())
        validate(appdir, arch)
        staging = Path(temporary)
        archive(appdir, staging / ("j2play-linux-" + arch + ".tar.gz"))
        ports_archive(appdir, staging, arch, build_id)
        validate_archives(appdir, staging, arch)
        publish(staging, output, arch)
    print(f"Linux package: {output / ('j2play-linux-' + arch + '.tar.gz')}", flush=True)
    return output / "J2Play.AppDir"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "check"))
    parser.add_argument("architecture", nargs="?", choices=(*sdk.ARCHITECTURES, "all"), default="all")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    for arch in sdk.ARCHITECTURES if args.architecture == "all" else (args.architecture,):
        if args.action == "build":
            package(arch, args.offline)
        else:
            output = sdk.ROOT / "builds/linux" / arch
            root = output / "J2Play.AppDir"
            contract = validate(root, arch)
            validate_archives(root, output, arch)
            print(json.dumps(contract, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, tarfile.TarError, zipfile.BadZipFile, subprocess.CalledProcessError) as error:
        print(f"Linux package: {error}", file=sys.stderr)
        raise SystemExit(1) from None
