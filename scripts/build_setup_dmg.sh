#!/bin/bash
#
# build_setup_dmg.sh
#
# Builds a release SmartBridge Setup.app and packages it as a UDZO .dmg
# in a clean tmp staging dir, sidestepping a known issue where Tauri's
# bundled bundle_dmg.sh chokes when prior runs leave temp files inside
# the source folder.
#
# The result lands in ~/Downloads/ by default. Override with --out PATH.
#
# Usage:
#   ./installer/scripts/build_setup_dmg.sh
#   ./installer/scripts/build_setup_dmg.sh --out /tmp/SmartBridge_Setup.dmg
#
# Notes:
#   * Builds in release mode with FAST 1 (CARGO_BUILD_JOBS=1, no
#     incremental compilation).
#   * The resulting .dmg is NOT code-signed. macOS Gatekeeper will warn
#     on first launch; the user must right-click → Open once to whitelist.

set -euo pipefail

INSTALLER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DMG_NAME="SmartBridge_Setup_0.1.0_arm64.dmg"
OUT="${HOME}/Downloads/${DMG_NAME}"
APP_NAME=""   # auto-detected from tauri.conf.json (productName + ".app")
VOL_NAME=""   # auto-derived from APP_NAME

while [ $# -gt 0 ]; do
    case "$1" in
        --out) OUT="$2"; shift 2 ;;
        --app-name) APP_NAME="$2"; shift 2 ;;
        --vol-name) VOL_NAME="$2"; shift 2 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

# Auto-detect product name from tauri.conf.json so this script works for
# every Setup flavor (Release / Demo / Beta) - the matrix workflow rewrites
# productName before invoking us.
if [ -z "${APP_NAME}" ]; then
    APP_NAME="$(jq -r '.productName' "${INSTALLER_DIR}/src-tauri/tauri.conf.json").app"
fi
if [ -z "${VOL_NAME}" ]; then
    VOL_NAME="${APP_NAME%.app}"
fi

echo "Installer dir : ${INSTALLER_DIR}"
echo "App name      : ${APP_NAME}"
echo "Volume name   : ${VOL_NAME}"
echo "Out path      : ${OUT}"
echo

cd "${INSTALLER_DIR}"

echo "→ Building release .app (FAST 1; first run takes ~4-6 minutes)..."
CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 npm run tauri build -- --bundles app

APP_DIR="${INSTALLER_DIR}/src-tauri/target/release/bundle/macos"
APP_PATH="${APP_DIR}/${APP_NAME}"

if [ ! -d "${APP_PATH}" ]; then
    echo "ERROR: expected .app not produced at ${APP_PATH}" >&2
    exit 1
fi

STAGE="$(mktemp -d -t smartbridge-setup-dmg-XXXXXX)"
trap 'rm -rf "${STAGE}"' EXIT

cp -R "${APP_PATH}" "${STAGE}/"

rm -f "${OUT}"
echo "→ Creating .dmg from clean staging dir..."
hdiutil create \
    -srcfolder "${STAGE}" \
    -volname "${VOL_NAME}" \
    -fs HFS+ \
    -format UDZO \
    "${OUT}"

SIZE=$(stat -f '%z' "${OUT}")
SHA=$(shasum -a 256 "${OUT}" | cut -d' ' -f1)

echo
echo "Built SmartBridge Setup .dmg"
echo "  path  : ${OUT}"
echo "  size  : ${SIZE} bytes"
echo "  sha256: ${SHA}"
