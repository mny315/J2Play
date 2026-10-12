"""Bounded, checksum-verified inputs for the portable Linux build tools."""

import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import tarfile
import tempfile
import urllib.request

MAX_ARCHIVE = 256 * 1024 * 1024
MAX_TREE = 3 * 1024 * 1024 * 1024
MAX_FILES = 100_000
MAX_INVENTORY = 32 * 1024 * 1024


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def download(item, cache, offline=False):
    url, expected = item["url"], item["sha256"]
    if not url.startswith("https://") or not re.fullmatch(r"[a-f0-9]{64}", expected):
        raise ValueError("Linux SDK inputs require HTTPS and an exact SHA-256")
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / url.rsplit("/", 1)[-1]
    if path.exists():
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NONBLOCK), "rb") as source:
            if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                raise ValueError(f"SDK cache input is not a regular file: {path}")
            data = source.read(MAX_ARCHIVE + 1)
    else:
        if offline:
            raise ValueError(f"Missing offline SDK archive: {path}")
        print(f"Fetching {url}", flush=True)
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(MAX_ARCHIVE + 1)
    if len(data) > MAX_ARCHIVE or sha256(data) != expected:
        raise ValueError(f"SDK checksum or size mismatch: {path.name}")
    if not path.exists():
        atomic_write(path, data)
    return data


def atomic_write(path, data, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            output.write(data)
            os.fchmod(output.fileno(), mode)
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def extract_tar(data, destination):
    """Extract a verified byte snapshot; confine links and reject special files."""
    destination.mkdir(parents=True, exist_ok=True)
    root = destination.resolve()
    total = 0
    links = []
    seen = set()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:*") as archive:
        for count, member in enumerate(archive, 1):
            name = PurePosixPath(member.name)
            if (count > MAX_FILES or name.is_absolute() or ".." in name.parts
                    or name in seen or not (member.isdir() or member.isfile()
                                           or member.issym() or member.islnk())):
                raise ValueError("Unsafe or oversized Linux SDK archive")
            seen.add(name)
            target = destination.joinpath(*name.parts)
            if not target.resolve().is_relative_to(root):
                raise ValueError("Linux SDK archive escapes its extraction root")
            if member.issym() or member.islnk():
                links.append((target, member.linkname, member.islnk()))
                continue
            if target.is_symlink():
                raise ValueError("Linux SDK archive overwrites a symbolic link")
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            total += member.size
            if member.size < 0 or total > MAX_TREE or target == destination:
                raise ValueError("Oversized Linux SDK archive contents")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output, 1024 * 1024)
            target.chmod(0o755 if member.mode & 0o111 else 0o644)
    for target, link, hard in links:
        source = (destination / link.lstrip("/") if hard or link.startswith("/")
                  else target.parent / link)
        if not source.resolve().is_relative_to(root) or not target.parent.resolve().is_relative_to(root):
            raise ValueError("Linux SDK link escapes its extraction root")
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() or target.is_symlink():
            raise ValueError("Duplicate Linux SDK archive link")
        if hard:
            if not source.is_file():
                raise ValueError("Invalid SDK hard link")
            total += source.stat().st_size
            if total > MAX_TREE:
                raise ValueError("Oversized Linux SDK archive contents")
            shutil.copyfile(source, target)
            target.chmod(source.stat().st_mode & 0o777)
        else:
            target.symlink_to(os.path.relpath(source, target.parent))


def deb_data(data):
    if not data.startswith(b"!<arch>\n"):
        raise ValueError("Invalid Debian SDK input")
    offset = 8
    for _ in range(16):
        header = data[offset:offset + 60]
        if len(header) != 60 or header[58:] != b"`\n":
            break
        size = int(header[48:58])
        if size < 0 or offset + 60 + size > len(data):
            break
        name = header[:16].decode("ascii").strip().rstrip("/")
        payload = data[offset + 60:offset + 60 + size]
        if name in ("data.tar.xz", "data.tar.gz", "data.tar"):
            return payload
        offset += 60 + size + size % 2
    raise ValueError("Missing Debian SDK data archive")


def inventory(root):
    result = {}
    total = 0
    count = 0
    root = root.resolve()
    pending = [root]
    while pending:
        with os.scandir(pending.pop()) as entries:
            for entry in entries:
                path = Path(entry.path)
                relative = path.relative_to(root).as_posix()
                if relative == ".inventory.json":
                    continue
                count += 1
                if count > MAX_FILES:
                    raise ValueError("Linux SDK exceeds its inventory limit")
                mode = entry.stat(follow_symlinks=False).st_mode
                if stat.S_ISDIR(mode):
                    pending.append(path)
                elif stat.S_ISLNK(mode):
                    if not path.resolve().is_relative_to(root):
                        raise ValueError("Linux SDK has an external link")
                    result[relative] = ["link", os.readlink(path)]
                elif stat.S_ISREG(mode):
                    digest = hashlib.sha256()
                    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NONBLOCK), "rb") as source:
                        metadata = os.fstat(source.fileno())
                        if not stat.S_ISREG(metadata.st_mode):
                            raise ValueError("Linux SDK input is not a regular file")
                        while chunk := source.read(1024 * 1024):
                            total += len(chunk)
                            if total > MAX_TREE:
                                raise ValueError("Linux SDK exceeds its size limit")
                            digest.update(chunk)
                    result[relative] = [metadata.st_mode & 0o777, digest.hexdigest()]
                else:
                    raise ValueError("Linux SDK contains a special file")
    return result


def seal(root):
    data = json.dumps(inventory(root), sort_keys=True).encode()
    if len(data) > MAX_INVENTORY:
        raise ValueError("Linux SDK exceeds its inventory size limit")
    atomic_write(root / ".inventory.json", data)


def verify(root):
    stamp = root / ".inventory.json"
    with os.fdopen(os.open(stamp, os.O_RDONLY | os.O_NONBLOCK), "rb") as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("Linux SDK inventory is not a regular file")
        if metadata.st_size > MAX_INVENTORY:
            raise ValueError("Linux SDK exceeds its inventory size limit")
        data = source.read(MAX_INVENTORY + 1)
    if len(data) > MAX_INVENTORY or json.loads(data) != inventory(root):
        raise ValueError("Linux SDK was modified; use a fresh linux-build SDK generation")
