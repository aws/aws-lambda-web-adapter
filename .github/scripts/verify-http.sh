#!/usr/bin/env bash
#
# Verify that an example's HTTP endpoint serves the expected response.
#
# Unlike a plain liveness check (any 2xx-4xx == "OK"), this asserts that a
# specific route returns an exact status code and, optionally, that the body
# contains a known substring. This catches broken routing and apps that boot
# but serve the wrong (or empty) content.
#
# Usage:
#   verify-http.sh <base_url> <path> <expect_status> [expect_body]
#
#   base_url      e.g. http://127.0.0.1:3000
#   path          request path, e.g. / or /healthz
#   expect_status comma-separated acceptable status codes, e.g. "200" or "200,307"
#   expect_body   optional substring that must appear in the response body
#
# Waits up to 90s (wall-clock) for the endpoint to satisfy BOTH conditions
# (the app may still be warming up), then exits 0 on success or 1 with
# diagnostics. Uses a wall-clock deadline plus a short per-request timeout
# (curl -m 5) so a hanging endpoint can't multiply the wait window.

set -uo pipefail

BASE_URL="${1:?base_url required}"
REQ_PATH="${2:-/}"
EXPECT_STATUS="${3:-200}"
EXPECT_BODY="${4:-}"

URL="${BASE_URL%/}${REQ_PATH}"
IFS=',' read -ra ACCEPT <<< "$EXPECT_STATUS"

status_ok() {
  local s="$1"
  for code in "${ACCEPT[@]}"; do
    [ "$s" = "$code" ] && return 0
  done
  return 1
}

echo "Verifying ${URL} (expect status ${EXPECT_STATUS}${EXPECT_BODY:+, body contains \"${EXPECT_BODY}\"})"

last_status=""
last_body=""
START=$SECONDS
DEADLINE=$((START + 90))
while [ $SECONDS -lt $DEADLINE ]; do
  resp="$(curl -s -m 5 -w $'\n%{http_code}' "$URL" 2>/dev/null || true)"
  last_status="${resp##*$'\n'}"
  last_body="${resp%$'\n'*}"
  if status_ok "$last_status"; then
    if [ -z "$EXPECT_BODY" ] || grep -qF -- "$EXPECT_BODY" <<< "$last_body"; then
      echo "OK after $((SECONDS - START))s: status ${last_status}${EXPECT_BODY:+, body contains \"${EXPECT_BODY}\"}"
      exit 0
    fi
  fi
  sleep 1
done

echo "FAIL: expectation not met within 90s (elapsed $((SECONDS - START))s)"
echo "  last status: ${last_status:-<none>} (expected ${EXPECT_STATUS})"
[ -n "$EXPECT_BODY" ] && echo "  expected body substring: ${EXPECT_BODY}"
echo "--- last response body (first 30 lines) ---"
printf '%s\n' "$last_body" | head -30
exit 1
