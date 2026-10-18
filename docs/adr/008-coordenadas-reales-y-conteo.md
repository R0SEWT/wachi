# ADR 008 — Coordenadas reales y unidad de conteo

> **Segunda integración (2026-10-07):** revisado; se precisa la ventana del conteo de pile-ups.

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- `lat_hecho`/`long_hecho` nunca son nulos, pero en las denuncias `SIN COORDENADA` la fuente imputa el centroide del distrito (`infelix/analysis/denuncias_mininter_quality.md`). En el risk store v1, 73,6 % de los eventos de la ventana eran centroides.
- Entre las `CON COORDENADA` hay acumulaciones artificiales en la comisaría (snapping): 37 % de los robos geocodificados de Lima caen en coordenadas exactas repetidas más de 30 veces (`infelix/analysis/comisaria_artifact_multicity.md`). infelix e InWatch descartan esas coordenadas con un umbral de 30.
- Sobre el universo peatonal (24 meses a 2025-06-30): `CON COORDENADA` 219.983; con a lo sumo 10 / 30 / 50 / 100 filas por coordenada: 108.486 / 138.492 / 152.069 / 169.949.
- `id_dgc` no es único (una denuncia, varias filas). Colapsar duplicados exactos (`id_dgc`, tipología, fecha, lat, lon) reduce 138.492 → 136.697 (−1,3 %).
- Ver el paper de SIMBig 2026 (README) sobre el efecto del sesgo de registro y la geocodificación.

## Decisión

1. Entran solo filas con `estado_coord = 'CON COORDENADA'`.
2. Se descartan las filas cuya coordenada exacta (lat/lon redondeadas a 5 decimales) aparece **más de 30 veces** en la ventana del modelo (ADR 009), contando **todas** las filas de la fuente en esa ventana (no solo robo/hurto). El conteo depende de la ventana, así que se recalcula con ella.
3. Se cuenta **una fila = un evento**. No se colapsan duplicados.

## Justificación

- Sin el paso 1, tres de cada cuatro puntos del modelo están en una posición inventada.
- El umbral 30 es el de infelix e InWatch: usar el mismo hace comparables las cifras entre los tres repos.
- Contar todas las filas para detectar el pile-up evita que una comisaría pase el filtro solo porque en esa categoría tiene pocas filas.
- Colapsar duplicados cambia 1,3 %: no puede reordenar celdas y añade una regla más que explicar.

## Consecuencias

- El modelo pierde la señal de los distritos con baja geocodificación; eso no se corrige aquí, se declara (ADR 010).
- La fuente vigente (`SIDPOL_DELITOS_TOTAL`) usa otro vocabulario (`ESTADO_COORD` ∈ `CERO_AMBAS`, `INVERTIDA`, `UBIGEO_NO_COINCIDE`); su equivalencia se decide con el spike `wachi-hdr` antes de usarla.

## Criterios de revisión

- La fuente vigente entra al modelo.
- La tasa de duplicados exactos supera 5 %.
