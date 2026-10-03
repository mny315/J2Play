#!/usr/bin/env python3
"""Create and use a persistent local Android release signing identity."""

import argparse
import os
from pathlib import Path
import secrets
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
SIGNING = ROOT / ".android-signing"
KEY = SIGNING / "release.keystore"
PASSWORD = SIGNING / "release.keystore.password"


def create():
    # Exclusive directory creation also prevents concurrent key generation.
    # Keep partial output on failure so a retry cannot replace a signing identity.
    if SIGNING.exists():
        raise ValueError(
            f"Signing directory already exists: {SIGNING}. Nothing was replaced. "
            "Keep the existing key, or restore the directory from your backup."
        )
    SIGNING.mkdir(mode=0o700)
    with os.fdopen(os.open(PASSWORD, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as file:
        file.write(secrets.token_urlsafe(48) + "\n")
    subprocess.run([
        "keytool", "-genkeypair", "-noprompt", "-storetype", "PKCS12",
        "-keystore", str(KEY), "-storepass:file", str(PASSWORD),
        "-keypass:file", str(PASSWORD), "-alias", "j2play",
        "-keyalg", "RSA", "-keysize", "3072", "-validity", "10000",
        "-dname", "CN=J2Play,O=J2Play",
    ], check=True, umask=0o077)
    print(f"Created Android signing key: {KEY}\nPassword file: {PASSWORD}")
    print("Back up both files outside this checkout. Keep them private; Git ignores them.")
    print("Use this same key for future APK updates.")


def check():
    if not SIGNING.exists():
        print("No local Android signing key found. Creating your own key for this checkout.", flush=True)
        print("Your APKs will use this key and cannot update official APKs signed with another key.", flush=True)
        create()
    if not KEY.is_file() or not PASSWORD.is_file():
        raise ValueError(
            f"Android signing files are incomplete in {SIGNING}. "
            "Restore release.keystore and release.keystore.password from your backup. "
            "No replacement key was generated."
        )
    if PASSWORD.stat().st_size > 1024 or not PASSWORD.read_bytes().strip():
        raise ValueError("The Android signing password file is empty or invalid. Restore your backup.")
    print(f"Android APK signing key: {KEY}", flush=True)
    print("Keep .android-signing/ for future updates and back it up outside this checkout.", flush=True)


def sign(apk, apksigner):
    check()
    subprocess.run([
        apksigner, "sign", "--ks", str(KEY), "--ks-key-alias", "j2play",
        # PKCS12 uses the same password for the store and key. Reusing a file
        # for --key-pass would make apksigner consume a second password line.
        "--ks-pass", f"file:{PASSWORD}", "--debuggable-apk-permitted", "false", str(apk),
    ], check=True, stdin=subprocess.DEVNULL)
    subprocess.run([apksigner, "verify", "--verbose", str(apk)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("create")
    commands.add_parser("check")
    signer = commands.add_parser("sign")
    signer.add_argument("apk", type=Path)
    signer.add_argument("--apksigner", required=True)
    args = parser.parse_args()
    try:
        if args.command == "create":
            create()
        elif args.command == "check":
            check()
        else:
            sign(args.apk, args.apksigner)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Android signing: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
