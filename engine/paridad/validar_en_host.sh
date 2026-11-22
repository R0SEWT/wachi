#!/usr/bin/env bash
# Paridad Python ↔ Rust con los datos reales del host (pbf + eventos.parquet de SIDPOL).
#
#   cd ~/wachi && engine/paridad/validar_en_host.sh [N_PARES_ALEATORIOS]   # por defecto 950
#
# Todo queda en api/data/ (gitignored): pares, respuestas de ambos motores y el resumen.
# Ningún dato sale de la máquina. No toca los servicios (usa el motor directo, sin HTTP).
set -euo pipefail
cd "$(dirname "$0")/../../api"
N="${1:-950}"
WACHI="${WACHI_BIN:-../engine/target/release/wachi}"
[ -x "$WACHI" ] || WACHI=../engine/wachi            # binario copiado por deploy.sh engine
[ -x "$WACHI" ] || { echo "falta el binario: cd engine && cargo build --release" >&2; exit 1; }
for f in data/eventos.parquet data/network.npz; do
  [ -f "$f" ] || { echo "falta api/$f (python -m wachi.prepare)" >&2; exit 1; }
done
echo "== pares (51 de od_pairs.yaml + $N aleatorios)"
uv run --with pyyaml python ../engine/paridad/pares.py data/network.npz data/val_pares.jsonl "$N"
echo "== motor Python"
uv run python ../engine/paridad/referencia.py data/val_pares.jsonl data/val_python.jsonl 2>/dev/null
echo "== motor Rust"
"$WACHI" batch data/val_pares.jsonl data/val_rust.jsonl 2>/dev/null
echo "== comparación"
python3 ../engine/paridad/comparar.py data/val_python.jsonl data/val_rust.jsonl
