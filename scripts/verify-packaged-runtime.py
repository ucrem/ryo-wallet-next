#!/usr/bin/env python3
"""Reject missing, modified, or unsafe wallet RPC files in built packages."""

import argparse
import hashlib
import json
import os
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


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--path", required=True, type=Path)
    args = parser.parse_args()
    verify(args.path, args.target)
