#!/usr/bin/env python3
"""Reject missing, modified, or unsafe wallet RPC files in built packages."""

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path

MANIFEST = Path(__file__).resolve().parents[1] / "src-tauri" / "runtime-manifest.json"


def verify(path: Path, target: str) -> None:
    manifest = json.loads(MANIFEST.read_text())
    try:
        expected = manifest["platforms"][target]["binarySha256"]
    except KeyError as error:
        raise ValueError(f"unsupported wallet RPC target: {target}") from error
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
        raise ValueError("wallet RPC must be a regular, non-symlink file")
    if not os.access(path, os.X_OK) and target != "x86_64-pc-windows-msvc":
        raise ValueError("wallet RPC is not executable")
    with path.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != expected:
        raise ValueError(f"wallet RPC digest mismatch for {target}")
    if target == "x86_64-unknown-linux-gnu":
        libraries = subprocess.run(["ldd", str(path)], capture_output=True, text=True, check=False)
        if libraries.returncode or "not found" in libraries.stdout or "not found" in libraries.stderr:
            raise ValueError("wallet RPC has unresolved Linux libraries")
    print(f"Verified packaged wallet RPC for {target}: {actual}")


def verify_rpm_package(path: Path, target: str) -> None:
    """Verify RPM payload integrity and the recorded wallet RPC file digest."""
    manifest = json.loads(MANIFEST.read_text())
    expected = manifest["platforms"][target]["binarySha256"]
    integrity = subprocess.run(["rpm", "-K", str(path)], capture_output=True, text=True, check=False)
    if integrity.returncode:
        raise ValueError(f"RPM package integrity check failed: {integrity.stdout}{integrity.stderr}")

    digest_metadata = subprocess.run(
        ["rpm", "-qp", "--qf", "%{FILEDIGESTALGO}\n%{PAYLOADSHA256}", str(path)],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    if len(digest_metadata) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", digest_metadata[1]):
        raise ValueError("RPM must contain a SHA-256 payload digest")
    algorithm = digest_metadata[0]
    if algorithm != "8":  # RPM digest algorithm 8 is SHA-256.
        raise ValueError(f"RPM uses unexpected file digest algorithm: {algorithm}")

    files = subprocess.run(
        ["rpm", "-qp", "--qf", "[%{FILENAMES}\t%{FILEDIGESTS}\t%{FILEMODES}\n]", str(path)],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    if any(".dev-runtime" in line.split("\t", 1)[0].split("/") for line in files):
        raise ValueError("Development runtime path leaked into the RPM")
    matches = [line.split("\t") for line in files if line.split("\t", 1)[0] == "/usr/bin/ryo-wallet-rpc"]
    if len(matches) != 1 or len(matches[0]) != 3:
        raise ValueError("RPM must contain exactly one wallet RPC binary")
    _, actual, mode_text = matches[0]
    mode = int(mode_text)
    if not stat.S_ISREG(mode) or not mode & 0o111:
        raise ValueError("RPM wallet RPC must be a regular executable file")
    if actual != expected:
        raise ValueError(f"RPM wallet RPC digest mismatch for {target}")
    print(f"Verified RPM wallet RPC for {target}: {actual}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--path", type=Path)
    source.add_argument("--rpm-package", type=Path)
    args = parser.parse_args()
    if args.rpm_package:
        verify_rpm_package(args.rpm_package, args.target)
    else:
        verify(args.path, args.target)
