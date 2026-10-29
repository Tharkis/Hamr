#!/bin/sh
# Lays out a built Hamr for packaging: dist/hamr-linux/{hamr, share/..., licences}.
# The tarball, AppImage, Arch package and Flatpak are all made from this tree.
# Usage: packaging/linux/stage.sh [path/to/hamr]   (default target/release/hamr)
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
bin=${1:-$root/target/release/hamr}
id=io.github.tharkis.Hamr
out=$root/dist/hamr-linux
rm -rf "$out"
install -Dm755 "$bin" "$out/hamr"
install -Dm644 "$root/packaging/linux/$id.desktop" "$out/share/applications/$id.desktop"
install -Dm644 "$root/packaging/linux/$id.metainfo.xml" "$out/share/metainfo/$id.metainfo.xml"
install -Dm644 "$root/assets/icon/hamr.svg" "$out/share/icons/hicolor/scalable/apps/$id.svg"
for size in 16 32 48 64 128 256 512; do
    install -Dm644 "$root/assets/icon/hamr-$size.png" \
        "$out/share/icons/hicolor/${size}x${size}/apps/$id.png"
done
install -Dm644 "$root/README.md" "$root/LICENSE-MIT" "$root/LICENSE-APACHE" -t "$out"
echo "$out"
