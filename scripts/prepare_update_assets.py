#!/usr/bin/env python3
"""Validate one staging build and prepare its exact release/update assets."""

import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path
from urllib.parse import quote

REPOSITORY = "ucrem/ryo-wallet-next"
GROUPS = {
    "preview-linux-x64-deb": (".deb",),
    "preview-linux-x64-rpm": (".rpm",),
    "preview-linux-x64-appimage": (".AppImage", ".AppImage.sig"),
    "preview-macos-intel-dmg": (".dmg",),
    "preview-macos-intel-updater": (".app.tar.gz", ".app.tar.gz.sig"),
    "preview-macos-apple-silicon-dmg": (".dmg",),
    "preview-macos-apple-silicon-updater": (".app.tar.gz", ".app.tar.gz.sig"),
    "preview-windows-x64-exe": ("-setup.exe", "-setup.exe.sig"),
}


def collect(source: Path) -> dict[tuple[str, str], Path]:
    if not source.is_dir():
        raise ValueError("staging artifact directory is missing")
    if {item.name for item in source.iterdir()} != set(GROUPS):
        raise ValueError("staging artifact groups differ from the expected set")
    result = {}
    for group, suffixes in GROUPS.items():
        entries = [path for path in (source / group).rglob("*") if path.is_file()]
        if len(entries) != len(suffixes):
            raise ValueError(f"{group}: expected {len(suffixes)} files, found {len(entries)}")
        for suffix in suffixes:
            matches = [path for path in entries if path.name.endswith(suffix)]
            if len(matches) != 1 or matches[0].stat().st_size == 0:
                raise ValueError(f"{group}: missing or empty {suffix} asset")
            result[group, suffix] = matches[0]
        signed = [suffix for suffix in suffixes if suffix.endswith(".sig")]
        if signed:
            signature = signed[0]
            installer = signature.removesuffix(".sig")
            if result[group, signature].name != result[group, installer].name + ".sig":
                raise ValueError(f"{group}: signature does not match the installer name")
    return result


def prepare(source: Path, output: Path, version: str) -> dict:
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid version")
    files = collect(source)
    for (group, suffix), path in files.items():
        if suffix in (".deb", ".rpm", ".AppImage", ".dmg", "-setup.exe") and version not in path.name:
            raise ValueError(f"{group}: package version does not match the release")
    output.mkdir(parents=True, exist_ok=False)
    published = {}

    def copy(group: str, suffix: str, name: str) -> Path:
        destination = output / name
        if destination.exists():
            raise ValueError(f"duplicate release asset: {name}")
        shutil.copyfile(files[group, suffix], destination)
        published[name] = destination
        return destination

    copy("preview-linux-x64-deb", ".deb", f"ryo-wallet-next_{version}_amd64.deb")
    copy("preview-linux-x64-rpm", ".rpm", f"ryo-wallet-next-{version}-1.x86_64.rpm")
    appimage = f"ryo-wallet-next_{version}_x86_64.AppImage"
    copy("preview-linux-x64-appimage", ".AppImage", appimage)
    appimage_sig = copy("preview-linux-x64-appimage", ".AppImage.sig", appimage + ".sig")
    intel_dmg = f"ryo-wallet-next_{version}_darwin_x86_64.dmg"
    copy("preview-macos-intel-dmg", ".dmg", intel_dmg)
    intel_update = f"ryo-wallet-next_{version}_darwin_x86_64.app.tar.gz"
    copy("preview-macos-intel-updater", ".app.tar.gz", intel_update)
    intel_sig = copy("preview-macos-intel-updater", ".app.tar.gz.sig", intel_update + ".sig")
    arm_dmg = f"ryo-wallet-next_{version}_darwin_aarch64.dmg"
    copy("preview-macos-apple-silicon-dmg", ".dmg", arm_dmg)
    arm_update = f"ryo-wallet-next_{version}_darwin_aarch64.app.tar.gz"
    copy("preview-macos-apple-silicon-updater", ".app.tar.gz", arm_update)
    arm_sig = copy("preview-macos-apple-silicon-updater", ".app.tar.gz.sig", arm_update + ".sig")
    windows = f"ryo-wallet-next_{version}_windows_x86_64-setup.exe"
    copy("preview-windows-x64-exe", "-setup.exe", windows)
    windows_sig = copy("preview-windows-x64-exe", "-setup.exe.sig", windows + ".sig")

    def platform(name: str, signature: Path) -> dict[str, str]:
        return {
            "url": f"https://github.com/{REPOSITORY}/releases/download/v{version}/{quote(name)}",
            "signature": signature.read_text(encoding="ascii").strip(),
        }

    manifest = {
        "version": version,
        "notes": "Development preview. See the release notes for details.",
        "platforms": {
            # DEB/RPM installations use this entry only to discover a new version.
            "linux-x86_64": platform(appimage, appimage_sig),
            "darwin-x86_64": platform(intel_update, intel_sig),
            "darwin-aarch64": platform(arm_update, arm_sig),
            "windows-x86_64": platform(windows, windows_sig),
        },
    }
    if any(not item["signature"] for item in manifest["platforms"].values()):
        raise ValueError("an updater signature is empty")
    (output / "latest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    checksums = []
    for name, path in sorted(published.items()):
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        checksums.append(f"{digest}  {name}")
    (output / "SHA256SUMS.txt").write_text("\n".join(checksums) + "\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("version")
    args = parser.parse_args()
    manifest = prepare(args.source, args.output, args.version)
    print(f"Prepared {len(list(args.output.iterdir()))} release files for v{manifest['version']}")
