#!/usr/bin/env bash
set -euo pipefail

shopt -s nullglob
version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
debs=(target/release/bundle/deb/*"$version"*.deb)
rpms=(target/release/bundle/rpm/*"$version"*.rpm)
test "${#debs[@]}" -eq 1
test "${#rpms[@]}" -eq 1
test -s "${debs[0]}.sig"
test -s "${rpms[0]}.sig"

inspection="$(mktemp -d)"
trap 'rm -rf "$inspection"' EXIT
mkdir -p "$inspection/deb" "$inspection/rpm"
if command -v dpkg-deb >/dev/null; then
  dpkg-deb -x "${debs[0]}" "$inspection/deb"
else
  deb_path="$(realpath "${debs[0]}")"
  (
    cd "$inspection/deb"
    ar x "$deb_path"
    tar -xf data.tar.*
  )
fi
(
  cd "$inspection/rpm"
  rpm2cpio "$OLDPWD/${rpms[0]}" | cpio -idm --quiet
)

target=x86_64-unknown-linux-gnu
for package in deb rpm; do
  python3 scripts/verify-packaged-runtime.py --target "$target" --path "$inspection/$package/usr/bin/ryo-wallet-rpc"
done
if find "$inspection" -name .dev-runtime | grep -q .; then
  echo 'Development runtime path leaked into an installer' >&2
  exit 1
fi
# The verified binary is now safe to invoke for a minimal loader/version smoke test.
"$inspection/deb/usr/bin/ryo-wallet-rpc" --version >/dev/null
