#!/usr/bin/env bash
set -euo pipefail
if [[ ${EUID:-$(id -u)} -ne 0 ]]; then printf 'Run with sudo; this installs only the whole-board service.\n' >&2; exit 1; fi
WB_USER="${SUDO_USER:-}"
WB_RELEASE=""
while [[ $# -gt 0 ]]; do
 case "$1" in
  --user) WB_USER="${2:?Provide a service username}"; shift 2 ;;
  --release) WB_RELEASE="${2:?Provide the extracted release directory}"; shift 2 ;;
  *) printf 'Usage: sudo ./install-service.sh --user USER --release DIRECTORY\n' >&2; exit 1 ;;
 esac
done
if [[ -z "$WB_USER" || "$WB_USER" == root || ! "$WB_USER" =~ ^[a-z_][a-z0-9_-]*$ ]]; then printf 'Choose an existing non-root service user with --user.\n' >&2; exit 1; fi
id "$WB_USER" >/dev/null
WB_GROUP=$(id -gn "$WB_USER")
if [[ ! "$WB_GROUP" =~ ^[a-z_][a-z0-9_-]*$ ]]; then printf 'Unsupported group name.\n' >&2; exit 1; fi
if [[ -z "$WB_RELEASE" ]]; then WB_RELEASE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); fi
WB_RELEASE=$(cd "$WB_RELEASE" && pwd)
[[ -x "$WB_RELEASE/bin/whole-board" && -f "$WB_RELEASE/static/index.html" && -f "$WB_RELEASE/deploy/whole-board.service" ]] || { printf 'Build and extract a release first.\n' >&2; exit 1; }
install -d -m 755 /opt/whole-board/bin /opt/whole-board/static
install -d -o "$WB_USER" -g "$WB_GROUP" -m 750 /var/lib/whole-board
install -d -o root -g "$WB_GROUP" -m 750 /etc/whole-board
# Replace the executable atomically, so an existing running process is not truncated.
install -m 755 "$WB_RELEASE/bin/whole-board" /opt/whole-board/bin/whole-board.new
mv /opt/whole-board/bin/whole-board.new /opt/whole-board/bin/whole-board
cp "$WB_RELEASE"/static/* /opt/whole-board/static/
if [[ ! -e /etc/whole-board/whole-board.env ]]; then install -o root -g "$WB_GROUP" -m 640 "$WB_RELEASE/deploy/whole-board.env.example" /etc/whole-board/whole-board.env; fi
sed -e "s/@USER@/$WB_USER/g" -e "s/@GROUP@/$WB_GROUP/g" "$WB_RELEASE/deploy/whole-board.service" > /etc/systemd/system/whole-board.service
systemctl daemon-reload
printf '\nInstalled. Existing configuration and database were preserved.\n'
printf '1. Edit /etc/whole-board/whole-board.env with your engine, model and TLS paths.\n'
printf '2. Import a corpus into /var/lib/whole-board/trainer.sqlite as %s.\n' "$WB_USER"
printf '3. Run: sudo systemctl enable --now whole-board\n'
printf 'This installer does not start/restart services or change firewall settings.\n'
