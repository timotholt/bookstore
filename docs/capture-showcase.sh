#!/usr/bin/env bash
# Capture the seeded local demo. Writes a cart only in the chosen local app.
set -euo pipefail
cd "$(dirname "$0")/.."
base_url="${1:-http://127.0.0.1:8083}"
case "$base_url" in
  http://127.0.0.1:*|http://localhost:*) ;;
  *) echo 'Use an isolated local demo server (http://127.0.0.1:PORT).' >&2; exit 1 ;;
esac
browser_bin="${AGENT_BROWSER_BIN:-agent-browser}"
session="chantels-showcase-$$"
capture_dir="$(mktemp -d)"
ab() { "$browser_bin" --session "$session" "$@"; }
cleanup() { ab close >/dev/null 2>&1 || true; rm -rf "$capture_dir"; }
trap cleanup EXIT
mkdir -p docs/assets
ab open "$base_url/"
ab set viewport 1440 1000
ab wait --load networkidle
ab screenshot docs/assets/homepage.png
ab record start "$capture_dir/walkthrough.webm"
ab wait 1500
ab open "$base_url/search"
ab wait --load networkidle
ab wait 1200
ab select '#genreFilter' 'Science Fiction'
ab wait --load networkidle
ab wait 1200
ab screenshot docs/assets/catalog.png
ab click 'a[href="/books/b003"]'
ab wait --load networkidle
ab wait 1800
ab screenshot docs/assets/book-detail.png
ab click '.add-to-stack'
ab wait --load networkidle
ab wait 1200
ab open "$base_url/cart"
ab wait --load networkidle
ab wait 2200
ab screenshot docs/assets/cart.png
ab record stop
ab errors
ffmpeg -hide_banner -loglevel error -y -i "$capture_dir/walkthrough.webm" \
  -an -c:v libx264 -crf 24 -pix_fmt yuv420p -movflags +faststart \
  docs/assets/shopping-walkthrough.mp4
ffmpeg -hide_banner -loglevel error -y -i docs/assets/shopping-walkthrough.mp4 \
  -filter_complex 'fps=8,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=3' \
  -loop 0 docs/assets/shopping-walkthrough.gif
