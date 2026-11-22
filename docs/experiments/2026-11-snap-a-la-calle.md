# Ajustar origen y destino a la calle, no a la esquina (2026-11-22)

**Pregunta.** El motor ajusta el origen y el destino al **nodo** más cercano (una esquina o un
cruce). ¿Cuánto se separa la ruta de donde está la persona, y qué cambia si se ajusta al **punto
más cercano de la calle** (la arista)?

**Respuesta corta.** Con el ajuste a la calle, la ruta empieza y termina ~4 veces más cerca de
la persona (p50 43 → 11 m sumando los dos extremos; p95 143 → 27 m), desaparecen los
`sin_via_peatonal_cerca` falsos en calles largas y la regla de la Tranquila se comporta igual
(36,2 % → 35,9 % de consultas con dos tarjetas, reducción mediana 38 % en ambos).

## Método

- Red: Geofabrik `peru-261120.osm.pbf` (261.899 nodos, 368.490 aristas), motor Rust (`engine/`).
- Personas: un punto al azar sobre la red, ponderado por largo, desplazado 0-25 m hacia un lado
  (vereda, puerta); destino a 0,5-5 km, también una persona (`engine/paridad/pares_personas.py`,
  3.000 pares, semilla 31).
- `WACHI_SNAP=nodo` (como `api/`) frente a `WACHI_SNAP=arista` (`Router::snap_edge` +
  `route_points`: A* con extremos virtuales; las aristas de origen y destino se recorren en
  parte y nunca se bloquean). Mismo β, misma regla (ADR 015), misma presentación (ADR 011).
- **Eventos sintéticos** (`eventos_sinteticos.py`, 250.000). Las cifras de distancia y separación
  dependen solo de la red; las de la Tranquila dependen de los eventos y hay que repetirlas con
  SIDPOL.

Corrección de `route_points`: costo igual a la fuerza bruta (mínimo entre la llegada directa y
las cuatro combinaciones de extremos con Dijkstra entre nodos) en 600 búsquedas; largo igual a
la geometría (`engine/tests/motor.rs`, `arista_*`).

## Resultados (2.819 pares comparables)

| | Nodo (hoy) | Arista |
|---|---:|---:|
| Separación pedido ↔ ruta, suma de extremos, p5 / p50 / p95 | 15 / 43 / 143 m | 2 / 11 / 27 m |
| `sin_via_peatonal_cerca` con la persona sobre la red | 38 (1,3 %) | 0 |
| Distancia de la Rápida, arista − nodo, p5 / p50 / p95 | | −136 / +5 / +134 m |
| Consultas con Tranquila | 36,2 % | 35,9 % |
| Reducción mediana de la Tranquila | 38 % | 38 % |
| Latencia p50 / p95 | 0,8 / 3,6 ms | 0,8 / 3,7 ms |
| Memoria sirviendo | 113 MB | 140 MB |

Además, sobre toda la red: al nodo más cercano hay p50 18 m / p90 50 m / p99 161 m; a la calle
más cercana, p50 4 m / p90 14 m / p99 22 m. En las aristas de más de 600 m (395 km de 18.057) el
15,9 % de los puntos **sobre la vía** quedan a más de 300 m de todo nodo.

## Lectura

- La distancia y la duración de cada tarjeta pasan a medirse desde donde está la persona; en una
  ruta de 1 km el error del ajuste a la esquina es del orden de ±5-15 %.
- La regla de selección no cambia de comportamiento en el agregado.
- El contrato HTTP no cambia (mismos campos); cambian los valores.

## Reproducción

```bash
cd api
uv run python ../engine/paridad/pares_personas.py data/network.npz data/personas.jsonl 3000 31
WACHI_SNAP=nodo   ../engine/target/release/wachi batch data/personas.jsonl data/snap_nodo.jsonl
WACHI_SNAP=arista ../engine/target/release/wachi batch data/personas.jsonl data/snap_arista.jsonl
```
