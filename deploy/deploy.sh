#!/usr/bin/env bash
# Despliega backend (ssh) y PWA (Netlify). Uso: deploy/deploy.sh [host] [api|engine|web|all]
# `engine` compila el motor Rust aquí, copia el binario al host y reinicia wachi-engine (no forma
# parte de `all`: la API en Python sigue siendo la de producción hasta decidir el cambio).
set -euo pipefail
HOST="${1:-4060}"; WHAT="${2:-all}"
SITE_ID="2cdd8c04-f21e-4637-af90-8e3c3de220a6"
SSH="${SSH:-/usr/bin/ssh}"
cd "$(dirname "$0")/.."
if [[ "$WHAT" == api || "$WHAT" == all ]]; then
  "$SSH" "$HOST" 'set -e; cd ~/wachi && git pull --ff-only && cd api && ~/.local/bin/uv sync --frozen \
    && systemctl --user restart wachi-api && sleep 5 && systemctl --user is-active wachi-api'
  # Espera hasta 150 s a que /health responda 200; si no, el comando remoto (y el script) falla.
  "$SSH" "$HOST" 'ok=0; for i in $(seq 1 30); do curl -sf localhost:8000/health && ok=1 && break; sleep 5; done; [ "$ok" = 1 ]'
fi
if [[ "$WHAT" == engine ]]; then
  (cd engine && cargo build --release --locked)
  "$SSH" "$HOST" 'mkdir -p ~/wachi/engine && cd ~/wachi && git pull --ff-only'
  scp engine/target/release/wachi "$HOST":wachi/engine/wachi.new
  "$SSH" "$HOST" 'set -e; mv ~/wachi/engine/wachi.new ~/wachi/engine/wachi \
    && mkdir -p ~/.config/systemd/user && cp ~/wachi/deploy/wachi-engine.service ~/.config/systemd/user/ \
    && systemctl --user daemon-reload && systemctl --user restart wachi-engine'
  "$SSH" "$HOST" 'ok=0; for i in $(seq 1 30); do curl -sf localhost:8000/health && ok=1 && break; sleep 5; done; [ "$ok" = 1 ]'
fi
if [[ "$WHAT" == web || "$WHAT" == all ]]; then
  netlify deploy --prod --dir web --site "$SITE_ID" --message "$(git rev-parse --short HEAD)"
fi
