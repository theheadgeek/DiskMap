#!/bin/sh
# Publishes a GitHub release of DiskMap.app for the version in Cargo.toml.
# Before running: bump the version in Cargo.toml, write docs/release-notes/v<version>.md,
# and commit and push to main. Needs the GitHub CLI (gh), logged in.
set -e
cd "$(dirname "$0")/.."

VERSION=$(grep -m1 '^version' Cargo.toml | cut -d '"' -f2)
TAG="v$VERSION"
NOTES="docs/release-notes/$TAG.md"

[ -f "$NOTES" ] || { echo "Write $NOTES first." >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "Commit your changes first." >&2; exit 1; }
git fetch -q origin
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "Push main to GitHub first." >&2; exit 1; }
if gh release view "$TAG" >/dev/null 2>&1; then echo "Release $TAG already exists." >&2; exit 1; fi

./scripts/make-app.sh --universal

# ditto keeps the app bundle's structure and signature intact inside the zip.
ZIP="target/release/DiskMap-$VERSION-macOS-universal.zip"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent target/release/DiskMap.app "$ZIP"

# Publish the notes with the zip's checksum added, so downloads can be verified.
FINAL_NOTES=$(mktemp)
cat "$NOTES" > "$FINAL_NOTES"
printf '\n## Checksum\n\n`%s`  SHA-256 of %s\n' "$(shasum -a 256 "$ZIP" | cut -d ' ' -f1)" "$(basename "$ZIP")" >> "$FINAL_NOTES"

gh release create "$TAG" "$ZIP" --title "DiskMap $VERSION" --notes-file "$FINAL_NOTES" --target main
