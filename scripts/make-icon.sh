#!/bin/sh
# Renders assets/icon.svg into packaging/macos/Pin.icns. Run after changing the icon; the
# result is committed, so releases don't depend on an SVG renderer.
set -eu
cd "$(dirname "$0")/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
qlmanage -t -s 1024 -o "$work" assets/icon.svg >/dev/null
master="$work/icon.svg.png"

set=$work/Pin.iconset
mkdir "$set"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$master" --out "$set/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" "$master" --out "$set/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set" -o packaging/macos/Pin.icns
echo "Wrote packaging/macos/Pin.icns"
