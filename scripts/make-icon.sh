#!/bin/sh
# Compiles assets/AppIcon.icon (an Icon Composer icon with light and dark
# versions) into the files DiskMap.app uses:
#   assets/Assets.car    light, dark and tinted icons for macOS 26 and later
#   assets/AppIcon.icns  a single icon for older macOS
#   assets/icon.png      the Dock icon when running with `cargo run`
# Needs Xcode 26 or later. Only needed after changing the icon; the outputs are
# committed so the app can be built without Xcode.
# You can also open assets/AppIcon.icon in Icon Composer to preview or edit it.
set -e
cd "$(dirname "$0")/.."

OUT=$(mktemp -d)
xcrun actool assets/AppIcon.icon --compile "$OUT" --platform macosx --minimum-deployment-target 12.0 \
  --app-icon AppIcon --output-partial-info-plist "$OUT/partial.plist" >/dev/null
cp "$OUT/Assets.car" assets/Assets.car
cp "$OUT/AppIcon.icns" assets/AppIcon.icns
sips -s format png -Z 1024 assets/AppIcon.icns --out assets/icon.png >/dev/null
echo "Made assets/Assets.car, assets/AppIcon.icns and assets/icon.png"
