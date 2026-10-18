#!/usr/bin/env bash
# Despliega backend (ssh) y PWA (Netlify). Uso: deploy/deploy.sh [host] [api|web|all]
set -euo pipefail
HOST="${1:-4060}"; WHAT="${2:-all}"
SITE_ID="2cdd8c04-f21e-4637-af90-8e3c3de220a6"
SSH="${SSH:-/usr/bin/ssh}"
cd "$(dirname "$0")/.."
if [[ "$WHAT" == api || "$WHAT" == all ]]; then
  "$SSH" "$HOST" 'set -e; cd ~/wachi && git pull --ff-only && cd api && ~/.local/bin/uv sync --frozen \
    && systemctl --user restart wachi-api && sleep 5 && systemctl --user is-active wachi-api'
  "$SSH" "$HOST" 'for i in $(seq 1 30); do curl -sf localhost:8000/health && break; sleep 5; done; echo'
fi
if [[ "$WHAT" == web || "$WHAT" == all ]]; then
  netlify deploy --prod --dir web --site "$SITE_ID" --message "$(git rev-parse --short HEAD)"
fi
