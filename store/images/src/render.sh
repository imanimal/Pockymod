#!/bin/sh
# Renders the cover and the GitHub social preview from cover.html with Edge (or Chrome) in headless mode.
# Run from Git Bash on Windows: sh store/images/src/render.sh
set -e
here="$(cd "$(dirname "$0")" && pwd -W 2>/dev/null || pwd)"
out="$here/.."
browser="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
[ -x "$browser" ] || browser="/c/Program Files/Google/Chrome/Application/chrome.exe"
shoot() { # page query width height file
  "$browser" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 --allow-file-access-from-files \
    --virtual-time-budget=3000 --window-size="$2,$3" --screenshot="$out/$4" "file:///$here/cover.html$1" >/dev/null 2>&1
}
shoot "" 1920 1080 cover.png
shoot "?social" 1280 640 github-social.png
echo "Wrote $out/cover.png and $out/github-social.png"
