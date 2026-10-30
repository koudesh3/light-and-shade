#!/bin/sh
# Build target/Light and Shade.app with the example painting.
set -eu
cd "$(dirname "$0")/.."
cargo build --release -p light-and-shade
app="target/Light and Shade.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/projects"
cp target/release/light-and-shade "$app/Contents/MacOS/"
cp -R projects/pothos "$app/Contents/Resources/projects/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Light and Shade</string>
  <key>CFBundleDisplayName</key><string>Light and Shade</string>
  <key>CFBundleIdentifier</key><string>com.koudesh3.light-and-shade</string>
  <key>CFBundleExecutable</key><string>light-and-shade</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
</dict></plist>
PLIST
echo "$app"
