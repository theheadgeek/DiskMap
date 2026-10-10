#!/bin/sh
# Builds DiskMap.app, a standalone Mac app you can keep in Applications.
# Usage: scripts/make-app.sh               (builds target/release/DiskMap.app for this Mac)
#        scripts/make-app.sh --universal   (one app for both Apple Silicon and Intel Macs)
#        scripts/make-app.sh --install     (also copies it to /Applications)
set -e
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || . "$HOME/.cargo/env"

UNIVERSAL=false
INSTALL=false
for arg in "$@"; do
  case "$arg" in
    --universal) UNIVERSAL=true ;;
    --install) INSTALL=true ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

# Match LSMinimumSystemVersion below.
export MACOSX_DEPLOYMENT_TARGET=12.0

if $UNIVERSAL; then
  # Needs both targets: rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  BINARY=target/release/diskmap-universal
  mkdir -p target/release
  lipo -create -output "$BINARY" \
    target/aarch64-apple-darwin/release/diskmap target/x86_64-apple-darwin/release/diskmap
else
  cargo build --release
  BINARY=target/release/diskmap
fi

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
APP=target/release/DiskMap.app
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/DiskMap"
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

if $INSTALL; then
  rm -rf /Applications/DiskMap.app
  cp -R "$APP" /Applications/
  echo "Installed /Applications/DiskMap.app"
fi
