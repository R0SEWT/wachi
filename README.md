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
