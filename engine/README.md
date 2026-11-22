# engine/ — el motor de Wachi en Rust

Reescritura de `api/` (FastAPI) con **el mismo contrato HTTP y las mismas respuestas, byte a
byte**. Un solo binario (`wachi`) lee el mismo `api/.env`, el mismo pbf y el mismo
`eventos.parquet`.

**Estado: propuesta.** Producción sigue en `api/`. La unidad `deploy/wachi-engine.service` queda
lista pero sin activar (ver [Migración](#migración-y-vuelta-atrás)).

## Cifras

Red real de Lima (Geofabrik 2026-11-20: 261.899 nodos, 368.490 aristas, 303 recintos), 250.000
eventos sintéticos (ver [Límites](#límites)), 504 pares origen-destino (los 51 de
`docs/experiments/od_pairs.yaml` + 450 aleatorios de 0,3 a 6 km + 3 bordes), misma máquina
(8 núcleos).

| | Python (`api/`) | Rust (`engine/`) |
|---|---:|---:|
| Construir la red desde el pbf | 26,5 s · 1.178 MB pico | **13,2 s · 279 MB** |
| Cargar el motor (con caché) | 3,0 s | **0,3 s** |
| Memoria sirviendo | 788 MB | **113 MB** |
| `/route` secuencial, p50 / p95 / máx | 12,2 / 96,6 / 529 ms | **0,7 / 3,4 / 18,6 ms** |
| HTTP, 8 conexiones (ruta de 2,7 km) | 70 rps · p99 157 ms | **4.742 rps · p99 3,1 ms** |
| HTTP, 16 conexiones | 124 rps · **48 % de 503** | **5.488 rps · 0 % de 503** |

Con `WACHI_BETAS=[0.5,1,2,4]` el p95 de Python pasa de 97 ms a **1.220 ms** (su caché de 8
grafos ponderados por (β, recintos bloqueados) se vacía y se reconstruye); el de Rust, de 3,4 a
14 ms. No cambia la decisión del β (ADR 014), pero el costo ya no sería un argumento.

Sobre los 503: `--limit-concurrency 16` de uvicorn cuenta *conexiones abiertas*, así que con 16
clientes con keep-alive ya está en el tope y rechaza casi todo. En Rust el límite es de
*peticiones en curso* (16); la 17.ª simultánea recibe 503 en vez de encolarse.

## Paridad

| Prueba | Resultado |
|---|---|
| Red construida (vías, nodos, aristas, km, recintos por tipo) | idéntica |
| Proyección UTM, 20.000 puntos con 7 decimales, ida y vuelta | **bit a bit** igual a pyproj |
| `/route`, 504 pares, 250.000 eventos | **504/504 idénticas** |
| `/route`, 5.004 pares (otra semilla) | **5.004/5.004 idénticas** |
| `/route`, 504 pares, 20.000 eventos (68 rutas con tramos de pocos datos) | **504/504 idénticas** |
| `/route`, 504 pares, `WACHI_BETAS=[0.5,1,2,4]` (varias candidatas) | **504/504 idénticas** |
| HTTP: `/health`, 504 rutas, 9 cuerpos raros, 404 | **517/517 idénticas byte a byte** |
| A* frente a Dijkstra puro: 1.200 búsquedas en grilla con recinto y β ∈ {0; 0,5; 2} | mismo óptimo |
| Suite de `api/tests` portada (`tests/motor.rs`) + unitarios | 33 tests en verde |

Para lograrlo se replicó la semántica exacta de lo que usa Python:

- **Proyección**: `geo_utm.rs` es un port operación por operación de PROJ 9.8.1 (`tmerc` exacto
  de Poder/Engsager, como pyproj por defecto) llamando a la misma libm. OSM guarda 7 decimales y
  la API redondea a 6, así que una de cada diez coordenadas cae en un empate de redondeo: con
  cualquier otra implementación (incluida una serie de Krüger correcta al nanómetro) la geometría
  difería en ±1e-6° en el 80 % de las rutas.
- **Aritmética de numpy/Python**: `np.round` y `round` (empates al par), `np.percentile` lineal
  (con su `lerp`), la suma por pares de `np.sum`, `round(x, 6)` sobre el decimal exacto.
- **GEOS**: `line_interpolate_point` y `length` con el mismo recorrido de segmentos.
- **Orden**: nodos por id de OSM, aristas en el orden de las vías del archivo, desempate de
  aristas paralelas como el `lexsort` de `router.graph`.

## Diferencias de diseño (no cambian el resultado)

- **A\*** con heurística euclidiana en vez del Dijkstra completo de scipy. Toda arista mide al
  menos la recta entre sus extremos y el costo `largo·(1+β·r̄)` es ≥ largo, así que la heurística
  es admisible y consistente: mismo camino, explorando una fracción de la red.
- **Sin caché de grafos ponderados** por (β, recintos bloqueados): el peso se calcula al relajar
  cada arista y el bloqueo es una comprobación por arista. Sin `CSR_CACHE_MAX`, sin candados.
- **Caché de la red propia** (`data/network.wachi`, 31 MB frente a 54 MB de la `.npz`). No comparte formato con
  `network.npz`; se reconstruye sola si cambia el pbf o la versión.
- La lectura del pbf hace tres pasadas (paralelas donde se puede) para no guardar las
  coordenadas de todo el Perú.

## Uso

```bash
cd engine && cargo build --release          # → target/release/wachi
cd ../api                                   # mismo directorio de trabajo que la API
../engine/target/release/wachi prepare      # construye data/network.wachi, imprime los sha256
../engine/target/release/wachi serve --port 8000 [--host 127.0.0.1] [--max-concurrency 16]
../engine/target/release/wachi batch PARES.jsonl SALIDA.jsonl   # paridad/benchmark
```

Configuración: las mismas variables `WACHI_*` de `api/wachi/config.py` (y `api/.env`), salvo la
caché: `WACHI_GRAPH_CACHE_RS` (por defecto `data/network.wachi`). Logs a stderr con `RUST_LOG`;
una línea por petición (`POST /route 200 2.0ms`), nunca el cuerpo ni la IP.

Tests: `cargo test` (con `WACHI_PROJ_REF=archivo.bin` compara además una muestra de pyproj).

## Reproducir la paridad

Con los datos reales del host (pbf + `eventos.parquet` de SIDPOL), un solo comando que no saca
nada de la máquina ni toca los servicios:

```bash
cd ~/wachi && engine/paridad/validar_en_host.sh      # 51 pares del experimento + 950 aleatorios
```

Paso a paso, desde `api/`, con el pbf y `data/network.npz` de Python:

```bash
uv run python ../engine/paridad/eventos_sinteticos.py data/network.npz data/eventos.parquet  # solo si no hay datos reales
uv run --with pyyaml python ../engine/paridad/pares.py data/network.npz data/pares.jsonl
uv run python ../engine/paridad/referencia.py data/pares.jsonl data/ref_python.jsonl
../engine/target/release/wachi batch data/pares.jsonl data/ref_rust.jsonl
python3 ../engine/paridad/comparar.py data/ref_python.jsonl data/ref_rust.jsonl --detalle
# HTTP: levantar ambos (uvicorn :8100, wachi serve :8101) y
uv run python ../engine/paridad/paridad_http.py http://127.0.0.1:8100 http://127.0.0.1:8101 data/pares.jsonl
```

## Límites

- **La paridad se midió con eventos sintéticos** (`paridad/eventos_sinteticos.py`: focos, ruido
  sobre la red y acumulaciones; no son denuncias). La red es la real. Falta repetir la
  comparación con el `eventos.parquet` de SIDPOL en la 4060: son los comandos de arriba sin el
  primero, y nada sale de esa máquina.
- `fecha` con zona horaria se toma como fecha UTC (pandas la tomaría en su zona).
- El 405 de un método no permitido no trae el cuerpo `{"detail": ...}` de FastAPI.

## Migración y vuelta atrás

En la 4060, con la API en Python funcionando:

1. `deploy/deploy.sh 4060 engine` — compila aquí, copia el binario a `~/wachi/engine/wachi`,
   instala la unidad y la arranca. `Conflicts=wachi-api.service` detiene la de Python (mismo
   puerto). La primera vez construye `data/network.wachi` (~15 s; `/health` responde 503
   `iniciando` mientras tanto).
2. Comparar con datos reales (sección anterior) antes o después del cambio.
3. Volver a Python: `systemctl --user start wachi-api` (detiene `wachi-engine`).
4. Dejarlo fijo: `systemctl --user disable wachi-api && systemctl --user enable wachi-engine`.
