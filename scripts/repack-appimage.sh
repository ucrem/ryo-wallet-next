#!/usr/bin/env bash
set -euo pipefail

shopt -s nullglob
appdirs=(target/release/bundle/appimage/*.AppDir)
appimages=(target/release/bundle/appimage/*.AppImage)
test "${#appdirs[@]}" -eq 1
test "${#appimages[@]}" -eq 1
appdir="${appdirs[0]}"
appimage="${appimages[0]}"
original_size="$(stat -c %s "$appimage")"

# linuxdeploy patches ELF RUNPATH in external binaries, changing the reviewed
# wallet RPC digest. Restore the exact verified executable before finalizing.
source_rpc=src-tauri/binaries/ryo-wallet-rpc-x86_64-unknown-linux-gnu
python3 scripts/verify-packaged-runtime.py --target x86_64-unknown-linux-gnu --path "$source_rpc"
install -m 755 "$source_rpc" "$appdir/usr/bin/ryo-wallet-rpc"
python3 scripts/verify-packaged-runtime.py --target x86_64-unknown-linux-gnu --path "$appdir/usr/bin/ryo-wallet-rpc"

# Tauri 2.11/linuxdeploy copies Ubuntu's display-protocol libraries into the
# AppImage. AppRun places them before Fedora's NVIDIA/Wayland stack, causing
# WebKitWebProcess to abort during EGL initialization. Keep WebKit/GTK and let
# the host provide only these display protocol interfaces.
display_libraries=(
  libwayland-client.so.0 libwayland-cursor.so.0 libwayland-egl.so.1
  libwayland-server.so.0 libxkbcommon.so.0 libxcb-randr.so.0
  libxcb-render.so.0 libxcb-shm.so.0 libXau.so.6 libXdmcp.so.6
)
for library in "${display_libraries[@]}"; do
  rm -f -- "$appdir/usr/lib/$library"
done

output_plugin="${XDG_CACHE_HOME:-$HOME/.cache}/tauri/linuxdeploy-plugin-appimage.AppImage"
test -x "$output_plugin"
rm -f -- "$appimage" "$appimage.sig"
ARCH=x86_64 LDAI_OUTPUT="$(realpath -m "$appimage")" APPIMAGE_EXTRACT_AND_RUN=1 \
  "$output_plugin" --appimage-extract-and-run --appdir "$(realpath "$appdir")"

version="$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
pnpm exec tauri signer sign --app-version "$version" "$appimage" >/dev/null
test -s "$appimage"
test -s "$appimage.sig"
echo "AppImage before: $original_size bytes; after: $(stat -c %s "$appimage") bytes"
