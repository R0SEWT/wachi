# Wachi v2 — diseño del repo nuevo, la API y el despliegue

Fecha: 2026-10-17. Estado: borrador para revisión de Rody.

## 1. Objetivo

Que `wachi.rosewt.dev` (la PWA que hoy está en Netlify con rutas de OSRM y riesgo
inventado) pida rutas **Rápida** y **Tranquila** a un backend propio que aplique los
ADR 003 y 007-016 del Wachi viejo, sobre el grafo peatonal de Lima y los robos
registrados por SIDPOL.

El resultado sirve a tres cosas a la vez:

1. **Producto para El Comercio**: algo usable desde un celular, con el mismo nombre y
   dominio que ya se les mostró.
2. **Validación de la UX con uso real**: la regla de selección (ADR 015) tiene tres
   umbrales (+15 %, +5 min, 20 %) que el experimento no exploró; solo caminando se ajustan.
3. **Demostración aplicada del paper de SIMBig 2026** (ver §9).

## 2. Lo ya decidido (no se reabre aquí)

- UX cerrada para este avance: `wachi-ux/UX.md`, cinco pantallas, dos tarjetas.
- ADR 013-016 aceptados como base del motor: grafo peatonal OSM propio con Dijkstra
  (014), regla de selección (015), superficie de robos a ≤ 150 m (016), sin rutear por
  dentro de recintos cerrados (013). ADR 011 define la presentación. ADR 009: sin
  franja horaria por lugar. ADR 010: "sin datos" es un indicador, no un riesgo.
- **Repo nuevo** (opción A del brainstorming): monorepo `api/` + `web/`. El Wachi viejo
  queda como repo de investigación y sigue produciendo el archivo de eventos hasta que
  exista `sidpol-lake`.
- **Backend en la 4060** (WSL Ubuntu 24.04, alias ssh `4060`) detrás de Tailscale Funnel.
  Rody recuperó `sudo` el 2026-10-17; gorgo queda como alternativa con el mismo mecanismo.
- El código del motor **sale del script del experimento**
  (`scripts/experiments/comparacion_ruteo.py`, rama `exp/comparacion-ruteo`, PR #12):
  no se escribe de cero.

## 3. Alcance de este corte

Dentro:

- API con `POST /route` y `GET /health`.
- Construcción y caché del grafo peatonal de Lima a partir del `.pbf` de Geofabrik.
- Superficie S150 y cobertura por celda H3 res 8 desde un parquet de eventos.
- Regla Rápida/Tranquila con parámetros en config.
- Texto del beneficio en minutos y cuadras (ADR 011) y tramos con pocos datos (ADR 010).
- PWA conectada a `/api/route`, copy ajustado al ADR 011.
- Despliegue reproducible (unidad systemd de usuario, `deploy.sh`, `_redirects`).
- CI con tests en GitHub Actions.

Fuera (van a beads como pendientes, no a este plan):

- ETL v2 / risk store v2 (`wachi-1ew.6`): vive en el Wachi viejo.
- Franjas horarias, modo acompañante en tiempo real, teselas offline, navegación paso a
  paso simulada.
- Geocodificación propia: la PWA sigue usando Photon.
- Cualquier modelo que no sea conteo (ADR 012: no adoptado).

## 4. Estructura del repo

```
wachi/
  api/
    pyproject.toml          uv; Python ≥ 3.12
    wachi/
      config.py             parámetros (Settings de pydantic, env WACHI_*)
      graph.py              pbf → red peatonal + recintos → caché .npz
      surface.py            parquet de eventos → S150 (cKDTree) + cobertura H3
      router.py             Dijkstra con costo largo·(1+β·r)
      select.py             regla Rápida/Tranquila
      explain.py            beneficio en minutos y cuadras; tramos con pocos datos
      api.py                FastAPI: /route, /health
    tests/
    data/                   gitignored: pbf, eventos.parquet, caché del grafo
  web/                      la PWA (index.html, app.js, style.css, sw.js, manifest, iconos)
    _redirects              /api/*  →  backend
  docs/
    adr/                    003 y 007-016 copiados del Wachi viejo, estado "Aceptado"
    experiments/            informe 2026-10 y comparacion_ruteo.py tal cual (reproducible)
    superpowers/specs/      esta spec
  deploy/
    wachi-api.service       unidad systemd --user
    deploy.sh               git pull + uv sync + restart
  .github/workflows/ci.yml  ruff + pytest sobre api/
  README.md, LICENSE, CLAUDE.md, AGENTS.md, .beads/
```

El scaffold (beads, uv, CLAUDE.md/AGENTS.md, CI) se hace con `project-kit:scaffold`.

## 5. Componentes de `api/`

Cada módulo tiene una función pública clara y se prueba solo.

| Módulo | Entrada | Salida | De dónde sale |
|---|---|---|---|
| `graph.py` | `.pbf`, lista de recintos (tags) | `Network`: nodos (UTM), aristas (largo, nodo a/b, flag recinto), índice espacial de nodos | `parse_osm`, `walk_ok`, `Network` del experimento |
| `surface.py` | parquet de eventos (lat, lon, fecha) | `Surface`: cKDTree de eventos en UTM; conteo por celda H3 res 8; `data_as_of`; estadísticas | `load_surfaces`, `_surface`, `Surfaces.at` |
| `router.py` | `Network`, `Surface`, origen, destino, β | ruta (lista de nodos, largo, exposición S150 muestreada cada 20 m) | `Router.route` del experimento (scipy `dijkstra` con cota) |
| `select.py` | ruta β=0, ruta β=0,5, parámetros | `fast`, `calm` o `None`, con desvío y reducción | `select`, `pair_metrics` |
| `explain.py` | `fast`, `calm`, `Surface` | minutos de desvío, cuadras con robos cerca, tramos con pocos datos | nuevo, sobre `Surfaces.at` |
| `api.py` | HTTP | JSON del contrato (§6) | nuevo |

**Costo de arista:** `largo·(1 + β·r̄)`, con `r̄` = media de `S150/p75` en las muestras de
20 m de la arista (como en el experimento). `p75` se calcula sobre las aristas al cargar.

**Recintos (ADR 013):** aristas con ≥ 50 % de su largo dentro de un recinto se excluyen,
salvo que el recinto contenga el origen o el destino. Igual que en el experimento.

**Cuadras con robos cerca:** una "cuadra" es una arista del grafo; una arista "tiene
robos cerca" si su `r̄ · p75 ≥ umbral` (`WACHI_BLOCK_MIN_EVENTS`, inicial 1). El texto es
"pasa N cuadras menos junto a robos registrados" con N = cuadras de la Rápida menos
cuadras de la Tranquila. Si N ≤ 0 se omite esa frase y queda solo la reducción en %.

**Tramos con pocos datos (ADR 010):** tramos de la ruta cuyas muestras caen en celdas H3
res 8 con conteo bajo el percentil 10 de las celdas activas. Se devuelven como índices
de la geometría para que la PWA los pinte punteados.

**Carga en memoria:** al arrancar, la API carga el grafo (desde caché `.npz` si existe,
si no lo construye desde el `.pbf` en ~25 s y lo guarda) y la superficie. Los eventos
viven solo en memoria; ningún endpoint devuelve coordenadas de denuncias.

## 6. Contrato `POST /route`

Petición:

```json
{ "origin": {"lat": -12.0695, "lon": -77.0795},
  "destination": {"lat": -12.0768, "lon": -77.0770} }
```

Respuesta 200:

```json
{
  "fast": {
    "geometry": [[-12.0695, -77.0795], ...],
    "distance_m": 900,
    "duration_s": 648,
    "low_data_segments": [[12, 18]]
  },
  "calm": {
    "geometry": [...],
    "distance_m": 1040,
    "duration_s": 749,
    "detour_m": 140,
    "detour_s": 101,
    "exposure_reduction_pct": 48,
    "blocks_fewer_near_robberies": 6,
    "low_data_segments": []
  },
  "data_as_of": "2025-06-30",
  "params": {"cap_pct": 0.15, "cap_min": 5, "gain_min": 0.20, "radius_m": 150, "beta": 0.5}
}
```

Reglas:

- `calm` es `null` cuando la regla no la muestra (reducción < 20 % o desvío sobre el
  tope). La PWA entonces muestra una sola tarjeta.
- Duración = distancia a 5 km/h (`WALK_KMH`, config). Sin `safety_level`, sin hora, sin
  puntajes.
- Geometría en `[lat, lon]`, como la usa Leaflet en `app.js`.

Errores:

| Caso | Código | Cuerpo |
|---|---|---|
| Origen o destino fuera del bbox de Lima (config) | 422 | `{"error": "fuera_de_cobertura"}` |
| Sin nodo del grafo a ≤ 300 m del punto | 422 | `{"error": "sin_via_peatonal_cerca", "which": "origin"\|"destination"}` |
| Origen y destino en componentes distintas (no debería pasar: se usa la componente mayor) | 422 | `{"error": "sin_ruta"}` |
| Grafo o superficie no cargados | 503 | `{"error": "iniciando"}` |

`GET /health` responde `{"status": "ok", "nodes": 261132, "events": N, "data_as_of": "..."}`
o 503 mientras carga.

## 7. Datos de entrada

| Archivo | Origen | Cómo se fija |
|---|---|---|
| `api/data/peru-latest.osm.pbf` | Geofabrik | `WACHI_PBF_SHA256` en config (el del experimento: `7be6d84a…`) |
| `api/data/eventos.parquet` | Wachi viejo: `eventos` con el universo ADR 007/008 (robo/hurto callejero, `CON COORDENADA`, pile-up ≤ 30) y 24 meses a `data_as_of` (ADR 009). Columnas: `lat`, `lon`, `fecha` | `WACHI_EVENTS_SHA256` en config; respaldo en Drive |

El filtro del universo **no** se reimplementa aquí: se exporta ya filtrado desde el Wachi
viejo con un script pequeño (`scripts/export_eventos.py`, tarea del plan) que reutiliza
`load_surfaces` para no divergir del experimento. Cuando exista `sidpol-lake`, cambia la
fuente, no el contrato.

Hasta que el risk store v2 esté construido, el parquet se genera con la ventana del
experimento (2023-07 a 2024-04) para tener algo real que servir; `data_as_of` lo dice.

Los datos SIDPOL son de terceros: el repo público no los contiene (gitignore) y la API
solo devuelve agregados por ruta.

## 8. Despliegue

- **Backend:** clon en `~/wachi` en la 4060 (o `/shared/Code/wachi` en gorgo). Servicio
  `systemd --user` (`loginctl enable-linger r0sewt`), `ExecStart=uv run uvicorn
  wachi.api:app --port 8000`, `Nice=10`, `Restart=on-failure`. Sin Docker ni `sudo`.
- **Exposición:** `tailscale funnel 8000` → `https://wsl-lizfer-4060.taile3f034.ts.net`
  (tailnet de Sttep, donde vive la 4060; Funnel activado el 2026-10-17). Si el backend se
  mueve a un host del tailnet de Rody, hay que activar Funnel allí.
- **PWA:** Netlify, sitio `wachi-rosewt`, ya con dominio y sin badge. `web/_redirects`:
  `/api/*  https://<host>.tail9f1c87.ts.net/:splat  200`. Netlify se conecta al repo
  (`web/` como directorio de publicación) para que cada push a `main` publique.
- **Redespliegue del backend:** `deploy/deploy.sh` = `git pull --ff-only && uv sync &&
  systemctl --user restart wachi-api`, lanzado por `ssh` desde la laptop. Manual.
- **Cambio de host:** instalar Tailscale en el nuevo, clonar, copiar `api/data/`, cambiar
  la línea de `_redirects`.

## 9. Relación con SIMBig 2026

El paper "What Survives an Uneven Record? Fine-Grained Crime Prediction under Reporting
Bias: Evidence from Peru" (Vilchez, Velasquez, Alarcon; Springer CCIS, por aparecer;
replicación en `R0SEWT/Crime-Prediction-Learn-under-Reporting-Bias`) muestra cómo el
sesgo de registro y la geocodificación degradan la predicción fina. Wachi v2 es su
aplicación:

- ADR 008 (solo coordenadas reales, pile-up ≤ 30) y ADR 010 ("sin datos" es indicador)
  son las decisiones de producto que salen de ese hallazgo. Los ADR los citan.
- El README del repo nuevo presenta a Wachi como demostración aplicada y cita el paper.
- `docs/experiments/` conserva el experimento de ruteo reproducible; es material para un
  trabajo posterior (comparación de superficies para ruteo), no para el paper ya enviado.
- Restricciones: no se deposita el manuscrito (licencia Springer); las variables de
  infelix no entran (ADR 012, licencia distinta).

## 10. Cambios en la PWA

- `calculateDynamicRoute` deja de llamar a OSRM y llama a `/api/route`; pinta `fast` y
  `calm`, o una sola tarjeta si `calm` es `null`.
- Copy de las tarjetas desde la respuesta: `+{detour_min} min · pasa {N} cuadras menos
  junto a robos registrados`. Se eliminan los textos de "iluminada", "serenazgo",
  "esquinas", "bares".
- `low_data_segments` se pinta con la línea punteada gris que ya existe (`layerSparse`).
- El chip de hora ("Salgo ahora") deja de afectar la ruta; se mantiene solo como
  información del modo En Marcha o se oculta (decisión al implementar; recomendación:
  ocultarlo en este corte).
- Los presets Elena y Mateo pasan a pedir la ruta a la API en vez de llevar coordenadas
  fijas.
- Errores 422 → mensaje corto en pantalla 1 ("Fuera de Lima" / "No hay vía peatonal
  cerca"); 503 o red → "El servicio está iniciando, intenta en un momento".

## 11. Pruebas

- **Unitarias (`api/tests/`):** grafo sintético de ~20 nodos con un recinto y eventos
  fijos: `router` devuelve la más corta con β=0 y evita la zona con β=0,5; `select`
  aplica los tres umbrales en los bordes; `explain` cuenta cuadras y tramos con pocos
  datos; `surface` cuenta a 150 m y en H3.
- **Contrato:** tests de `api.py` con `TestClient` sobre el grafo sintético: forma del
  JSON, `calm: null`, los cuatro errores.
- **Regresión con datos reales (marcado `slow`, corre solo si existen los archivos):**
  pares Elena y Mateo de `od_pairs.yaml`; se espera para Mateo una Tranquila de
  +120 a +160 m con reducción ≥ 45 %, y para Elena una Tranquila que no cruce el
  Rebagliati. Las cifras exactas se fijan al primer corrido y quedan como golden.
- **CI:** ruff + tests unitarios y de contrato en cada PR. Los `slow` se corren a mano.

## 12. Decisiones pendientes de Rody

1. **Nombre y visibilidad del repo.** `R0SEWT/wachi` ya es el Wachi viejo (privado) y
   el push a `Lab-EC-UPC/wachi` está deshabilitado hasta acordar con El Comercio.
   Recomendación: renombrar el viejo a `wachi-research` y crear `R0SEWT/wachi` público
   para el nuevo. Si lo público choca con el acuerdo con El Comercio, nace privado y se
   abre después.
2. **Licencia** del repo nuevo (recomendación: MIT para el código; los datos no se
   publican).
3. ~~Dos pasos con `sudo`~~ Resuelto: la 4060 ya tiene Tailscale y Funnel activos.
