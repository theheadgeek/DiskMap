#!/bin/sh
# Builds DiskMap.app, a standalone Mac app you can keep in Applications.
# Usage: scripts/make-app.sh            (builds target/release/DiskMap.app)
#        scripts/make-app.sh --install  (also copies it to /Applications)
set -e
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || . "$HOME/.cargo/env"

cargo build --release

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
APP=target/release/DiskMap.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/diskmap "$APP/Contents/MacOS/DiskMap"
# Assets.car has the light and dark icons (macOS 26+); AppIcon.icns is the fallback for older macOS.
cp assets/Assets.car "$APP/Contents/Resources/Assets.car"
cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DiskMap</string>
  <key>CFBundleDisplayName</key><string>DiskMap</string>
  <key>CFBundleIdentifier</key><string>com.inactivenode.diskmap</string>
  <key>CFBundleExecutable</key><string>DiskMap</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleIconName</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Copyright © $(date +%Y), Inactive Node</string>
  <key>NSAppleEventsUsageDescription</key><string>DiskMap asks Finder to move items to the Trash, so Put Back still works.</string>
  <key>NSDesktopFolderUsageDescription</key><string>DiskMap reads folder sizes to show what is using your disk.</string>
  <key>NSDocumentsFolderUsageDescription</key><string>DiskMap reads folder sizes to show what is using your disk.</string>
  <key>NSDownloadsFolderUsageDescription</key><string>DiskMap reads folder sizes to show what is using your disk.</string>
  <key>NSRemovableVolumesUsageDescription</key><string>DiskMap reads folder sizes to show what is using your disk.</string>
  <key>NSNetworkVolumesUsageDescription</key><string>DiskMap reads folder sizes to show what is using your disk.</string>
</dict>
</plist>
PLIST

# Sign it locally ("-" means no developer certificate) so macOS can remember its permissions.
codesign --force --sign - --identifier com.inactivenode.diskmap "$APP"
echo "Built $APP"

if [ "$1" = "--install" ]; then
  rm -rf /Applications/DiskMap.app
  cp -R "$APP" /Applications/
  echo "Installed /Applications/DiskMap.app"
fi
