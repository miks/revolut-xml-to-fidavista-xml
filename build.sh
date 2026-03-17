#!/bin/bash
# build.sh — builds Rust binary + Swift wrapper, packages into .app
set -e

APP_NAME="RevolutToFidavista"
APP_DIR="bundle/${APP_NAME}.app/Contents/MacOS"
RESOURCES_DIR="bundle/${APP_NAME}.app/Contents/Resources"

mkdir -p "$APP_DIR"
mkdir -p "$RESOURCES_DIR"

echo "→ Adding macOS ARM target..."
rustup target add aarch64-apple-darwin

echo "→ Building Rust binary (release)..."
cargo build --release --target aarch64-apple-darwin
cp "target/aarch64-apple-darwin/release/revolut2fidavista" "$APP_DIR/revolut2fidavista"
chmod +x "$APP_DIR/revolut2fidavista"

echo "→ Compiling Swift wrapper..."
swiftc \
    -target arm64-apple-macos12 \
    -o "$APP_DIR/${APP_NAME}" \
    AppWrapper/AppDelegate.swift

chmod +x "$APP_DIR/${APP_NAME}"

echo "→ Writing Info.plist..."
cat > "bundle/${APP_NAME}.app/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>RevolutToFidavista</string>
    <key>CFBundleIdentifier</key>
    <string>lv.tools.revolut2fidavista</string>
    <key>CFBundleName</key>
    <string>RevolutToFidavista</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>CSV File</string>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>csv</string>
            </array>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
        </dict>
    </array>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
</dict>
</plist>
EOF

echo "→ Clearing quarantine..."
xattr -cr "bundle/${APP_NAME}.app" 2>/dev/null || true

echo ""
echo "✓ Done!  →  bundle/${APP_NAME}.app"
echo ""
echo "Usage: drag any Revolut CSV onto the app icon in Finder or Dock."
