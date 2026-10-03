#!/usr/bin/env python3
"""Package upstream notices for the actual locked Android/Linux dependency graph."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

import linux_sources as sources

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / "legal/sources.json"


def cargo_notices(destination, cargo, env, package, target, cache, offline=False):
    extra = json.loads(SOURCES.read_text())
    if extra.get("schema") != 1:
        raise ValueError("Unknown legal source manifest")
    options = ["--locked"] + (["--offline"] if offline else [])
    tree = subprocess.check_output(
        [cargo, "tree", *options, "-p", package, "--target", target,
         "--edges", "normal,no-proc-macro", "--prefix", "none", "--format", "{p}"],
        cwd=ROOT, env=env, text=True)
    linked = {tuple(line.split()[:2]) for line in tree.splitlines()}
    metadata = json.loads(subprocess.check_output(
        [cargo, "metadata", *options, "--format-version=1", "--filter-platform", target],
        cwd=ROOT, env=env))
    locked = {(p["name"], p["version"]): p for p in
              tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]}
    ui_inputs = json.loads((ROOT / "crates/frontend-ui/build-support/sources.json").read_text())
    patched = {p["name"]: p for p in ui_inputs["crates"]}
    entries = []
    missing = []
    total = 0
    for item in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if (item["name"], "v" + item["version"]) not in linked or item["id"] in metadata["workspace_members"]:
            continue
        name = item["name"] + "-" + item["version"]
        root = Path(item["manifest_path"]).parent
        count = 0

        def copy_notice(relative, data):
            nonlocal count, total
            count += 1
            total += len(data)
            if len(data) > 2 * 1024 * 1024 or count > 128 or total > 32 * 1024 * 1024:
                raise ValueError("Dependency notices exceed the inventory limit")
            sources.atomic_write(destination / name / relative, data)

        for path in sorted(root.rglob("*")):
            if not path.is_file() or not path.name.upper().startswith(
                    ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "OFL")):
                continue
            if path.stat().st_size > 2 * 1024 * 1024:
                raise ValueError(f"Oversized license input: {name}")
            copy_notice(path.relative_to(root), path.read_bytes())
        # Some published crates omit workspace licenses. Also retain explicitly
        # pinned supplemental texts even when a crate contains other notices.
        for index, source in enumerate(extra["licenses"]):
            if source["crates"].get(item["name"]) == item["version"]:
                data = sources.download(source, cache / source["sha256"], offline)
                copy_notice(f"UPSTREAM-LICENSE-{index}.txt", data)
        if not count or not item["license"]:
            missing.append(name)
        entry = {"name": item["name"], "version": item["version"], "license": item["license"]}
        patch = patched.get(item["name"])
        if patch and patch["version"] == item["version"]:
            entry.update({"sha256": patch["sha256"],
                          "patches": ["crates/frontend-ui/build-support/" + p for p in patch["patches"]]})
        else:
            entry["sha256"] = locked[(item["name"], item["version"])]["checksum"]
        entry["source"] = f"https://static.crates.io/crates/{item['name']}/{name}.crate"
        entries.append(entry)
    if missing or not entries:
        raise ValueError(f"Dependencies have no packaged license text: {', '.join(missing) or package}")
    sources.atomic_write(destination / "dependencies.json", (json.dumps(entries, indent=2) + "\n").encode())
    for name in ("LICENSE", "NOTICE"):
        shutil.copyfile(ROOT / name, destination / name)
    shutil.copyfile(ROOT / "legal/THIRD_PARTY_NOTICES.txt", destination / "THIRD_PARTY_NOTICES.txt")
    for name in ("sources.json", "Unicode-3.0.txt"):
        shutil.copyfile(ROOT / "legal" / name, destination / name)
    # Keep the provenance alongside the font licenses in binary distributions.
    for relative in ("crates/vm/assets/fonts", "crates/frontend-ui/assets/fonts"):
        folder = ROOT / relative
        for path in folder.iterdir():
            if path.suffix in (".md", ".txt"):
                sources.atomic_write(destination / relative / path.name, path.read_bytes())
    for relative in ("crates/frontend-ui/assets/artwork.provenance.md",
                     "crates/frontend-ui/assets/gamepad-body.provenance.md"):
        sources.atomic_write(destination / relative, (ROOT / relative).read_bytes())
    return entries


def remove_asset_backup(backup):
    # Rust toolchain notices retain Nix's read-only directory permissions.
    for directory, _children, _files in os.walk(backup):
        os.chmod(directory, 0o700)
    shutil.rmtree(backup)


def android_assets(rust, ndk, offline=False):
    build = ROOT / "android-build"
    build.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".legal-", dir=build) as temporary:
        assets = Path(temporary) / "assets"
        legal = assets / "legal"
        entries = cargo_notices(legal, "cargo", os.environ.copy(), "j2play-android",
                                "aarch64-linux-android", build / "downloads/licenses", offline)
        rust_docs = rust / "share/doc/rust"
        shutil.copytree(rust_docs / "licenses", legal / "rust/licenses")
        shutil.copyfile(rust_docs / "COPYRIGHT-library.html", legal / "rust/COPYRIGHT-library.html")
        for name in ("NOTICE", "NOTICE.toolchain", "source.properties"):
            sources.atomic_write(legal / "ndk" / name, (ndk / name).read_bytes())
        for source in (ROOT / "Cargo.lock", ROOT / "flake.lock",
                       ROOT / "crates/frontend-ui/build-support/sources.json"):
            sources.atomic_write(legal / "build-inputs" / source.name, source.read_bytes())
        # This directory contains only generated packaging inputs. A failed
        # notice collection leaves the previous assets and published APK intact.
        destination = build / "assets"
        # A failed rollback must survive staging cleanup, including Ctrl-C.
        backup = Path(tempfile.mkdtemp(prefix=".previous-assets-", dir=build))
        previous = backup / "assets"
        try:
            if destination.exists() or destination.is_symlink():
                destination.rename(previous)
            assets.rename(destination)
        except BaseException:
            try:
                if previous.exists() or previous.is_symlink():
                    previous.rename(destination)
            except OSError as error:
                raise OSError(f"Cannot restore the previous Android assets; retained files: {previous}") from error
            remove_asset_backup(backup)
            raise
        remove_asset_backup(backup)
    print(f"Android legal assets: {len(entries)} dependencies in {destination / 'legal'}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["android"])
    parser.add_argument("--rust", type=Path, required=True)
    parser.add_argument("--ndk", type=Path, required=True)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    android_assets(args.rust, args.ndk, args.offline)


if __name__ == "__main__":
    main()
