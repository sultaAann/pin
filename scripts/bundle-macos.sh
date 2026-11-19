#!/bin/sh
# Builds dist/Pin.app and dist/Pin.zip from the workspace.
#   TARGETS        Rust targets to build and merge (default: Apple Silicon and Intel)
#   SIGN_IDENTITY  codesign identity; ad-hoc signing when unset
set -eu
cd "$(dirname "$0")/.."

targets=${TARGETS:-"aarch64-apple-darwin x86_64-apple-darwin"}
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
app=dist/Pin.app

binaries=""
for target in $targets; do
    cargo build --release --locked -p pin-app --target "$target"
    binaries="$binaries target/$target/release/pin-app"
done

rm -rf dist
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp packaging/macos/Pin.icns "$app/Contents/Resources/Pin.icns"
# shellcheck disable=SC2086 # one path per target
lipo -create -output "$app/Contents/MacOS/Pin" $binaries
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist >"$app/Contents/Info.plist"

# A stable identity keeps macOS from asking for the keychain password again after updates.
codesign --force --sign "${SIGN_IDENTITY:--}" --identifier io.github.sultaann.pin "$app"
codesign --verify --strict "$app"

ditto -c -k --keepParent "$app" dist/Pin.zip
echo "Built $app $version and dist/Pin.zip"
