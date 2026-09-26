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

MANIFEST = (
    Path(__file__).resolve().parents[1]
    / "src-tauri"
    / "runtime-manifest.json"
)


def sha256_file(path: Path) -> str:
    """Calculate SHA-256 without requiring Python 3.11 hashlib.file_digest."""
    hasher = hashlib.sha256()

    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)

    return hasher.hexdigest()


def expected_digest(target: str) -> str:
    """Return the reviewed wallet RPC SHA-256 for a target."""
    manifest = json.loads(MANIFEST.read_text())

    try:
        digest = manifest["platforms"][target]["binarySha256"]
    except KeyError as error:
        raise ValueError(
            f"unsupported wallet RPC target: {target}"
        ) from error

    if not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ValueError(
            f"invalid wallet RPC digest in runtime manifest for {target}"
        )

    return digest


def verify(path: Path, target: str) -> None:
    """Verify an extracted packaged wallet RPC binary."""
    expected = expected_digest(target)

    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ValueError(
            f"packaged wallet RPC is missing: {path}"
        ) from error

    if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
        raise ValueError(
            "wallet RPC must be a regular, non-symlink file"
        )

    if (
        target != "x86_64-pc-windows-msvc"
        and not os.access(path, os.X_OK)
    ):
        raise ValueError("wallet RPC is not executable")

    actual = sha256_file(path)

    if actual.lower() != expected.lower():
        raise ValueError(
            f"wallet RPC digest mismatch for {target}: "
            f"expected {expected}, got {actual}"
        )

    if target == "x86_64-unknown-linux-gnu":
        libraries = subprocess.run(
            ["ldd", str(path)],
            capture_output=True,
            text=True,
            check=False,
        )

        output = f"{libraries.stdout}\n{libraries.stderr}"

        if libraries.returncode != 0:
            raise ValueError(
                "wallet RPC Linux dependency check failed: "
                f"{output.strip()}"
            )

        if "not found" in output:
            raise ValueError(
                "wallet RPC has unresolved Linux libraries: "
                f"{output.strip()}"
            )

    print(
        f"Verified packaged wallet RPC for {target}: {actual}"
    )


def verify_rpm_package(path: Path, target: str) -> None:
    """
    Verify RPM integrity and the wallet RPC digest recorded in its file metadata.

    PAYLOADSHA256 is deliberately not required because it is optional for
    RPM v4 packages. The wallet RPC itself is checked through RPM's per-file
    digest metadata against the reviewed SHA-256 in runtime-manifest.json.
    """
    expected = expected_digest(target)

    if not path.is_file():
        raise ValueError(f"RPM package is missing: {path}")

    integrity = subprocess.run(
        ["rpm", "-K", str(path)],
        capture_output=True,
        text=True,
        check=False,
    )

    if integrity.returncode != 0:
        raise ValueError(
            "RPM package integrity check failed: "
            f"{integrity.stdout}{integrity.stderr}"
        )

    algorithm_result = subprocess.run(
        [
            "rpm",
            "-qp",
            "--qf",
            "%{FILEDIGESTALGO}",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=True,
    )

    algorithm = algorithm_result.stdout.strip()

    # RPM hash algorithm ID 8 is SHA-256.
    if algorithm != "8":
        raise ValueError(
            f"RPM uses unexpected file digest algorithm: {algorithm!r}"
        )

    files_result = subprocess.run(
        [
            "rpm",
            "-qp",
            "--qf",
            "[%{FILENAMES}\\t%{FILEDIGESTS}\\t%{FILEMODES}\\n]",
            str(path),
        ],
        capture_output=True,
        text=True,
        check=True,
    )

    files = files_result.stdout.splitlines()

    if not files:
        raise ValueError("RPM contains no file metadata")

    for line in files:
        parts = line.split("\t")

        if not parts:
            continue

        filename = parts[0]

        if ".dev-runtime" in filename.split("/"):
            raise ValueError(
                "Development runtime path leaked into the RPM"
            )

    matches = []

    for line in files:
        parts = line.split("\t")

        if (
            len(parts) == 3
            and parts[0] == "/usr/bin/ryo-wallet-rpc"
        ):
            matches.append(parts)

    if len(matches) != 1:
        raise ValueError(
            "RPM must contain exactly one "
            "/usr/bin/ryo-wallet-rpc binary"
        )

    _, actual, mode_text = matches[0]

    if not re.fullmatch(r"[0-9a-fA-F]{64}", actual):
        raise ValueError(
            "RPM wallet RPC does not have a valid SHA-256 file digest"
        )

    try:
        mode = int(mode_text)
    except ValueError as error:
        raise ValueError(
            f"RPM wallet RPC has an invalid file mode: {mode_text!r}"
        ) from error

    if not stat.S_ISREG(mode):
        raise ValueError(
            "RPM wallet RPC must be a regular file"
        )

    if not mode & 0o111:
        raise ValueError(
            "RPM wallet RPC must be executable"
        )

    if actual.lower() != expected.lower():
        raise ValueError(
            f"RPM wallet RPC digest mismatch for {target}: "
            f"expected {expected}, got {actual}"
        )

    print(
        f"Verified RPM wallet RPC for {target}: {actual}"
    )


def main() -> None:
    parser = argparse.ArgumentParser()

    parser.add_argument(
        "--target",
        required=True,
    )

    source = parser.add_mutually_exclusive_group(
        required=True
    )

    source.add_argument(
        "--path",
        type=Path,
    )

    source.add_argument(
        "--rpm-package",
        type=Path,
    )

    args = parser.parse_args()

    if args.rpm_package is not None:
        verify_rpm_package(
            args.rpm_package,
            args.target,
        )
    else:
        verify(
            args.path,
            args.target,
        )


if __name__ == "__main__":
    main()