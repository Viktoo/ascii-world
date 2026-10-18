#!/bin/sh
# Bundle the release build as "Infinite Pocket.app", zipped for a GitHub
# release: dist/Infinite-Pocket-<version>-macos-<arch>.zip.
# Run after `cargo build --release`.
set -eu

cd "$(dirname "$0")/.."
version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
arch=$(uname -m)
app="dist/Infinite Pocket.app"
rm -rf dist
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/pocket "$app/Contents/MacOS/pocket"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Infinite Pocket</string>
  <key>CFBundleDisplayName</key><string>Infinite Pocket</string>
  <key>CFBundleIdentifier</key><string>com.infinitepocket.game</string>
  <key>CFBundleExecutable</key><string>pocket</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
  <key>NSMicrophoneUsageDescription</key><string>Speak to talk listens to you when you turn it on in settings.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Finds friends' games on your network, and lets them join yours.</string>
</dict>
</plist>
EOF
zip="dist/Infinite-Pocket-$version-macos-$arch.zip"
ditto -c -k --keepParent "$app" "$zip"
echo "$zip"
