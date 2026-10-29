#!/bin/sh
# Builds dist/Hamr-<version>-x86_64.AppImage from the tree made by stage.sh.
# Usage: packaging/linux/build-appimage.sh <version> [path/to/appimagetool]
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
version=$1
tool=${2:-appimagetool}
id=io.github.tharkis.Hamr
staged=$root/dist/hamr-linux
appdir=$root/dist/Hamr.AppDir
rm -rf "$appdir"
install -Dm755 "$staged/hamr" "$appdir/usr/bin/hamr"
cp -r "$staged/share" "$appdir/usr/"
cp "$staged/share/applications/$id.desktop" "$appdir/$id.desktop"
cp "$staged/share/icons/hicolor/256x256/apps/$id.png" "$appdir/$id.png"
ln -s usr/bin/hamr "$appdir/AppRun"
# Runners have no FUSE, so let appimagetool unpack itself instead of mounting.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 VERSION=$version \
    "$tool" --no-appstream "$appdir" "$root/dist/Hamr-$version-x86_64.AppImage"
