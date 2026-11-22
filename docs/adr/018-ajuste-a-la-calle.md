# ADR 018 — Origen y destino en el punto más cercano de la calle

## Estado
Propuesto (2026-11-22). Implementado como opción (`WACHI_SNAP=arista`) en el motor Rust (ADR 017); por defecto sigue `nodo`.

## Contexto

- El motor ajusta el origen y el destino al nodo más cercano de la red. En la red de Lima eso separa la ruta de la persona p50 43 m y p95 143 m (sumando los dos extremos), y la distancia y la duración que muestra la tarjeta se miden desde esa esquina.
- En calles largas sin cruces, una persona parada sobre la vía puede quedar a más de 300 m de todo nodo y recibir `sin_via_peatonal_cerca` (1,3 % de las consultas en el experimento).
- Evidencia: `../experiments/2026-11-snap-a-la-calle.md`.

## Decisión

1. Origen y destino se ajustan al **punto más cercano de una arista peatonal** (`snap_max_m` sigue siendo el tope, ahora medido hasta la calle).
2. Las aristas de origen y destino se recorren en parte, con costo proporcional al tramo, y nunca se bloquean por recinto: la persona ya está ahí. Los recintos permitidos son los que contienen a los puntos ajustados.
3. La regla de selección (ADR 015) y la presentación (ADR 011) no cambian.

## Justificación

- La ruta empieza y termina ~4 veces más cerca de la persona (p50 11 m, p95 27 m).
- Desaparecen los `sin_via_peatonal_cerca` con la persona sobre la red.
- La proporción de consultas con Tranquila y su reducción no cambian (36,2 → 35,9 %; 38 % → 38 %).
- Mismo costo de cómputo; +27 MB de memoria por el índice de muestras.

## Consecuencias

- Cambian los valores de `distance_m`, `duration_s` y `detour_*` respecto de hoy (no los campos). Se rompe la paridad con `api/`: debe adoptarse después de la migración (ADR 017), no a la vez.
- La geometría empieza en un punto de la calle que no es un nodo de OSM.

## Criterios de revisión

- Con los eventos de SIDPOL la proporción de Tranquilas o su reducción cambian de forma apreciable.
- La UX necesita que la ruta empiece en la puerta (no en la calle): haría falta un tramo final fuera de la red.
