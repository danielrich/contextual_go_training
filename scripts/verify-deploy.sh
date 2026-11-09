#!/usr/bin/env bash
set -euo pipefail
WB_URL="${1:?Usage: ./verify-deploy.sh https://your-go-server.local:8443 [CA_CERTIFICATE]}"
WB_URL="${WB_URL%/}"
WB_CURL_ARGS=(--fail --silent --show-error --connect-timeout 10)
if [[ $# -gt 1 ]]; then WB_CURL_ARGS+=(--cacert "$2"); fi
WB_HEALTH=$(curl "${WB_CURL_ARGS[@]}" "$WB_URL/health")
[[ "$WB_HEALTH" == ok ]] || { printf 'Unexpected health response.\n' >&2; exit 1; }
for WB_ASSET in / /app.js /offline.js /grader.js /preview.js /counting.js /sw.js /manifest.webmanifest /icon-192.png /icon-512.png; do curl "${WB_CURL_ARGS[@]}" "$WB_URL$WB_ASSET" -o /dev/null; done
printf 'Health and PWA assets passed. Server status:\n'
curl "${WB_CURL_ARGS[@]}" "$WB_URL/api/status"
printf '\nFor offline verification, download a ready problem, stop the server, reload the phone app, answer it, then reconnect and sync twice.\n'
