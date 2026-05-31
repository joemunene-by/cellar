#!/bin/bash
# Build a macOS .app bundle that wraps a cellar launch script.
# Result lands in /Applications/cellar Games/<name>.app, which
# Launchpad/Dock/Spotlight all index normally.
#
# Usage:
#   make-game-app.sh "Display Name" /path/to/launch-script.sh [icon.icns]
set -euo pipefail

NAME="${1:?display name required}"
SCRIPT="${2:?launch script path required}"
ICON="${3:-}"
APPS_DIR="/Applications/cellar Games"
APP_DIR="$APPS_DIR/$NAME.app"
BUNDLE_ID="dev.cellar.$(echo "$NAME" | tr ' ' '_' | tr '[:upper:]' '[:lower:]')"

if [ ! -f "$SCRIPT" ]; then
  echo "launch script not found: $SCRIPT" >&2
  exit 1
fi

# Ask for sudo upfront, /Applications needs it.
if [ ! -w "/Applications" ]; then
  sudo mkdir -p "$APPS_DIR"
  sudo chown "$USER" "$APPS_DIR"
fi
mkdir -p "$APPS_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

# The MacOS executable is a tiny shell script that backgrounds the real launcher
# and exits immediately. macOS treats long-running foreground processes in
# Contents/MacOS as "the app", and they show in the Dock until they exit. We
# want the wine launcher (with its own window) to be the visible thing, not us.
cat > "$APP_DIR/Contents/MacOS/$NAME" <<EOF
#!/bin/bash
# launcher wrapper for cellar game: $NAME
exec /bin/bash "$SCRIPT" >>/tmp/cellar-game.log 2>&1
EOF
chmod +x "$APP_DIR/Contents/MacOS/$NAME"

# Info.plist. LSBackgroundOnly=false so it shows in Launchpad,
# LSUIElement=false so it appears in the Dock briefly while running.
cat > "$APP_DIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>$NAME</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.games</string>
  <key>LSPrefersRosetta2AheadOfTime</key>
  <true/>
  <key>LSRequiresNativeExecution</key>
  <false/>
EOF

if [ -n "$ICON" ] && [ -f "$ICON" ]; then
  cp "$ICON" "$APP_DIR/Contents/Resources/AppIcon.icns"
  cat >> "$APP_DIR/Contents/Info.plist" <<EOF
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
EOF
fi

cat >> "$APP_DIR/Contents/Info.plist" <<EOF
</dict>
</plist>
EOF

# Touch the bundle so Launch Services notices.
touch "$APP_DIR"
/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -f "$APP_DIR" 2>/dev/null || true

echo "built $APP_DIR"
echo "launchpad / dock will show it as: $NAME"
