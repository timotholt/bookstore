#!/usr/bin/env bash
set -euo pipefail

base_url="${1:-http://127.0.0.1:8081}"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "$scratch_dir"' EXIT

# Bound startup time, and fail explicitly if migrations/readiness never succeed.
for attempt in {1..60}; do
  if curl --fail --silent --max-time 2 "$base_url/readyz" >/dev/null; then
    break
  fi
  sleep 1
done

check_route() {
  local route="$1"
  shift
  local status
  status="$(curl --silent --show-error --max-time 10 \
    --output "$scratch_dir/body" --write-out '%{http_code}' "$@" "$base_url$route")"
  if [[ "$status" != 200 || ! -s "$scratch_dir/body" ]]; then
    echo "FAIL $route: HTTP $status (expected 200 with a nonempty body)" >&2
    return 1
  fi
  echo "PASS $route: HTTP $status"
}

for route in /readyz /healthz /search /cart /signup /login /styles.css /app.js /assets/htmx.min.js; do
  check_route "$route"
done
check_route /catalog -H 'HX-Request: true'
check_route /

# Follow a real seeded book link, rather than assuming a particular database ID.
book_path="$(python3 - "$scratch_dir/body" <<'PY'
import sys
from html.parser import HTMLParser
from pathlib import Path

class BookLinks(HTMLParser):
    path = None

    def handle_starttag(self, tag, attrs):
        href = dict(attrs).get('href', '')
        if tag == 'a' and href.startswith('/books/') and self.path is None:
            self.path = href

links = BookLinks()
links.feed(Path(sys.argv[1]).read_text())
if links.path is None:
    sys.exit('FAIL homepage: no seeded book link')
print(links.path)
PY
)"
check_route "$book_path"
echo 'Packaged application smoke checks passed.'
