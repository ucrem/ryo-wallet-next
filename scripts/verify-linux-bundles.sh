#!/usr/bin/env bash
set -euo pipefail

shopt -s nullglob
version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
debs=(target/release/bundle/deb/*"$version"*.deb)
rpms=(target/release/bundle/rpm/*"$version"*.rpm)
appimages=(target/release/bundle/appimage/*"$version"*.AppImage)
test "${#debs[@]}" -eq 1
test "${#rpms[@]}" -eq 1
test "${#appimages[@]}" -eq 1
test -s "${appimages[0]}.sig"

inspection="$(mktemp -d)"
trap 'rm -rf "$inspection"' EXIT
mkdir -p "$inspection/deb" "$inspection/rpm" "$inspection/appimage"
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
(
  cd "$inspection/appimage"
  "$OLDPWD/${appimages[0]}" --appimage-extract >/dev/null
)

target=x86_64-unknown-linux-gnu
for package in deb rpm; do
  python3 scripts/verify-packaged-runtime.py --target "$target" --path "$inspection/$package/usr/bin/ryo-wallet-rpc"
done
python3 scripts/verify-packaged-runtime.py --target "$target" --path "$inspection/appimage/squashfs-root/usr/bin/ryo-wallet-rpc"
if find "$inspection" -name .dev-runtime | grep -q .; then
  echo 'Development runtime path leaked into an installer' >&2
  exit 1
fi
for library in libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1 libwayland-server.so.0 libxkbcommon.so.0 libxcb-randr.so.0 libxcb-render.so.0 libxcb-shm.so.0 libXau.so.6 libXdmcp.so.6; do
  test ! -e "$inspection/appimage/squashfs-root/usr/lib/$library"
done
# The verified binary is now safe to invoke for a minimal loader/version smoke test.
"$inspection/deb/usr/bin/ryo-wallet-rpc" --version >/dev/null
