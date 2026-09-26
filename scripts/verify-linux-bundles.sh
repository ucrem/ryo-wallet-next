#!/usr/bin/env bash
set -Eeuo pipefail
trap 'status=$?; echo "Linux package verification failed at line $LINENO: $BASH_COMMAND (exit $status)" >&2' ERR

shopt -s nullglob
version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
debs=(target/release/bundle/deb/*"$version"*.deb)
rpms=(target/release/bundle/rpm/*"$version"*.rpm)
if (( ${#debs[@]} != 1 || ${#rpms[@]} != 1 )); then
  echo "Expected one DEB and one RPM for $version; found ${#debs[@]} DEB and ${#rpms[@]} RPM" >&2
  exit 1
fi
for package in "${debs[0]}" "${rpms[0]}"; do
  if [[ ! -s "$package.sig" ]]; then
    echo "Missing or empty updater signature: $package.sig" >&2
    exit 1
  fi
done

deb_path="$(realpath "${debs[0]}")"
rpm_path="$(realpath "${rpms[0]}")"

inspection="$(mktemp -d)"
trap 'rm -rf "$inspection"' EXIT
mkdir -p "$inspection/deb"
if command -v dpkg-deb >/dev/null; then
  echo "Extracting DEB: $deb_path"
  dpkg-deb -x "$deb_path" "$inspection/deb"
else
  echo "Extracting DEB with ar: $deb_path"
  (
    cd "$inspection/deb"
    ar x "$deb_path"
    tar -xf data.tar.*
  )
fi
target=x86_64-unknown-linux-gnu
python3 scripts/verify-packaged-runtime.py --target "$target" --path "$inspection/deb/usr/bin/ryo-wallet-rpc"
python3 scripts/verify-packaged-runtime.py --target "$target" --rpm-package "$rpm_path"
if find "$inspection" -name .dev-runtime | grep -q .; then
  echo 'Development runtime path leaked into an installer' >&2
  exit 1
fi
# The verified binary is now safe to invoke for a minimal loader/version smoke test.
"$inspection/deb/usr/bin/ryo-wallet-rpc" --version >/dev/null
