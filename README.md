# Wachi

Rutas a pie para Lima con dos opciones: **Rápida** (la más corta) y **Tranquila** (la de
menos robos registrados cerca, si no te alarga más de 5 minutos o 15 %).

- `web/`: PWA (https://wachi.rosewt.dev)
- `api/`: motor de rutas (FastAPI). `cd api && uv sync && uv run uvicorn wachi.api:app`
- `docs/adr/`: decisiones; `docs/experiments/`: evidencia reproducible

Aplicación del hallazgo de *What Survives an Uneven Record? Fine-Grained Crime Prediction
under Reporting Bias: Evidence from Peru* (Vilchez, Velasquez, Alarcon; SIMBig 2026,
Springer CCIS). Replicación: https://github.com/R0SEWT/Crime-Prediction-Learn-under-Reporting-Bias

Los datos de SIDPOL no se distribuyen con el repo; la API solo devuelve agregados por ruta.

## Datos (no versionados)

1. `api/data/peru-latest.osm.pbf`: `curl -L -o api/data/peru-latest.osm.pbf https://download.geofabrik.de/south-america/peru-latest.osm.pbf`
2. `api/data/eventos.parquet`: lo produce `scripts/export_eventos.py` de `wachi-research` (datos SIDPOL, no distribuidos).
3. `cd api && uv run python -m wachi.prepare` construye `data/network.npz` e imprime los sha256 para `api/.env`.

## Despliegue

El backend corre en un host propio como servicio systemd de usuario y sale a internet por
Tailscale Funnel; la PWA está en Netlify y le pasa `/api/*` a ese host (`web/_redirects`).
Hoy el host es la 4060 (alias ssh `4060`). Para montarlo en otro:

1. Instalar uv (`curl -LsSf https://astral.sh/uv/install.sh | sh`) y Tailscale, y clonar en
   `~/wachi`: `git clone https://github.com/R0SEWT/wachi.git ~/wachi`.
2. `cp ~/wachi/api/.env.example ~/wachi/api/.env` y copiar `api/data/` desde la laptop
   (`scp api/data/eventos.parquet host:wachi/api/data/`; el pbf se puede bajar de Geofabrik
   allá, ver "Datos").
3. `cd ~/wachi/api && uv sync --locked && uv run python -m wachi.prepare`: construye
   `data/network.npz` e imprime los sha256. Poner en el `.env` de ese host los sha que
   imprime (cada host fija el de su pbf).
4. `loginctl enable-linger $USER` para que el servicio siga vivo sin sesión abierta.
5. Instalar la unidad: `mkdir -p ~/.config/systemd/user && cp ~/wachi/deploy/wachi-api.service
   ~/.config/systemd/user/ && systemctl --user daemon-reload && systemctl --user enable --now wachi-api`.
   La unidad limita a 16 conexiones, 2 GB y 2 CPU (`MemoryMax`/`CPUQuota` solo surten
   efecto si el host delega `memory` y `cpu` al usuario: ver
   `/sys/fs/cgroup/user.slice/user-$(id -u).slice/user@$(id -u).service/cgroup.controllers`).
6. `tailscale funnel --bg 8000` (con `sudo` si no se fijó `tailscale set --operator=$USER`) y
   `curl https://<host>.<tailnet>.ts.net/health`.
7. Cambiar el host en `web/_redirects` (`/api/*  https://<host>.<tailnet>.ts.net/:splat  200`)
   y publicar la PWA.

Redespliegue desde la laptop: `deploy/deploy.sh [host] [api|web|all]` (por defecto `4060 all`).
`api` hace `git pull`, `uv sync` y reinicia el servicio, y falla si `/health` no responde
200 en 150 s; `web` publica `web/` en Netlify con la CLI `netlify`.

**Cuando cambia el pbf:** correr `uv run python -m wachi.prepare` en el host y actualizar
`WACHI_PBF_SHA256` en su `.env`. La caché del grafo guarda el sha del pbf y una versión;
si no coinciden con lo configurado, el servicio la reconstruye solo al arrancar (unos 40 s
y ~1,3 GB de pico). Un pbf que no coincide con el sha fijado se rechaza.

**Logs:** `journalctl --user -u wachi-api -f`. Si la carga del motor falla, el proceso sale
con código 1 y systemd lo reinicia a los 5 s; `/health` responde 503 `iniciando` mientras
carga.
