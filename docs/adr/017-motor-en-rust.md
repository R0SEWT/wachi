# ADR 017 — Motor de rutas en Rust con paridad exacta

## Estado
Propuesto (2026-11-22). Implementado en `engine/` (rama `feat/motor-rust`); producción sigue en `api/`.

## Contexto

- La API en Python (FastAPI + numpy/scipy/shapely/pyproj) sirve cada ruta en 12 ms de mediana, pero con colas largas: p95 97 ms y máximo 529 ms sobre 504 pares. La causa es la caché de grafos ponderados por (β, recintos bloqueados), de 8 entradas: cada consulta cerca de un recinto tiene su propio conjunto bloqueado y reconstruye un CSR de 368 mil aristas. Con varios β (`WACHI_BETAS=[0.5,1,2,4]`) el p95 sube a 1,2 s.
- El GIL limita el servicio a ~23-124 rps según la ruta. Además, `--limit-concurrency 16` de uvicorn cuenta *conexiones abiertas*: con 16 clientes con keep-alive rechaza con 503 casi todo (48-87 % de las peticiones en las pruebas).
- El proceso usa ~0,8 GB sirviendo y ~1,2 GB al reconstruir la caché del grafo; la unidad lo limita a 2 GB y 2 CPU en la 4060.

## Decisión

1. Reemplazar `api/` por `engine/` (un binario Rust) **con el mismo contrato HTTP y las mismas respuestas byte a byte**, verificado antes del cambio con los datos reales en la 4060 (`engine/paridad/validar_en_host.sh`).
2. Mantener la API en Python en el repositorio como referencia ejecutable mientras dure la transición: es contra lo que se mide la paridad.
3. Cualquier cambio de comportamiento (por ejemplo, ajustar el origen a la calle y no a la esquina) va en un ADR propio, con el motor Rust ya en producción y la paridad como punto de partida.

## Justificación

| Medido (misma máquina, red real de Lima) | Python | Rust |
|---|---:|---:|
| Construir la red desde el pbf | 26,5 s · 1,18 GB | 13,2 s · 0,28 GB |
| Cargar con caché · memoria sirviendo | 3,0 s · 788 MB | 0,3 s · 113 MB |
| `/route` p50 / p95 / máx | 12 / 97 / 529 ms | 0,7 / 3,4 / 19 ms |
| HTTP, 16 conexiones | 124 rps, 48 % de 503 | 5.488 rps, 0 % de 503 |

- Paridad: 6.516 respuestas de `/route` idénticas byte a byte en cuatro escenarios (504 y 5.004 pares, eventos escasos, varios β) y 517/517 por HTTP. La proyección es un port operación por operación de PROJ 9.8.1 (idéntica bit a bit a pyproj); sin eso la geometría difería en ±1e-6° en el 80 % de las rutas por empates de redondeo.
- A* con heurística euclidiana da el mismo óptimo que Dijkstra (propiedad verificada en 1.200 búsquedas) y explora una fracción de la red.
- Un binario sin dependencias de Python simplifica el despliegue y deja margen de CPU y memoria en la 4060.

## Consecuencias

- Hay que mantener la paridad mientras convivan los dos motores: un cambio en `api/` sin su par en `engine/` rompe `validar_en_host.sh`.
- La caché del grafo es otra (`data/network.wachi`, `WACHI_GRAPH_CACHE_RS`); se construye sola al arrancar si falta o no coincide con el pbf.
- El despliegue compila en la laptop y copia el binario (`deploy/deploy.sh 4060 engine`); el host no necesita Rust.
- Volver a Python es un comando: `systemctl --user start wachi-api` (las unidades se excluyen entre sí).

## Criterios de revisión

- La paridad con los datos reales de SIDPOL no da 100 %: se investiga cada diferencia antes de cambiar.
- Mantener dos motores cuesta más de lo que aporta: se retira `api/`.
