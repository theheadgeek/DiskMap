#!/bin/sh
# Turns assets/icon.svg into assets/icon.png (1024 px, used for the Dock icon
# when running with cargo) and assets/AppIcon.icns (used by DiskMap.app).
# Only needed after changing icon.svg; both outputs are committed.
set -e
cd "$(dirname "$0")/.."

# Render the SVG with AppKit, which reads SVG natively on macOS 14 and later.
WORK=$(mktemp -d)
cat > "$WORK/render.swift" <<'SWIFT'
import AppKit
let args = CommandLine.arguments
guard let image = NSImage(contentsOfFile: args[1]) else { fatalError("Couldn't read \(args[1])") }
let size = 1024
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
image.draw(in: NSRect(x: 0, y: 0, width: size, height: size))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: args[2]))
SWIFT
swiftc -O -o "$WORK/render" "$WORK/render.swift"
"$WORK/render" assets/icon.svg assets/icon.png

# Every size macOS asks for, then pack them into an .icns.
ICONSET="$WORK/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z $size $size assets/icon.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double assets/icon.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o assets/AppIcon.icns
echo "Made assets/icon.png and assets/AppIcon.icns"
