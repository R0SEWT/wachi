# ADR 007 — Universo de eventos: robo y hurto callejero

> **Segunda integración (2026-10-07):** revisado, sin cambios.

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- El risk store v1 no correspondía al universo calibrado en los EDA: `normalize.py` ignoraba la precedencia tipo+subtipo (corregido en PR #10) y quedó dominado por conducción en estado de ebriedad y faltas.
- Con el filtro curado corregido, el universo peatonal en 24 meses (2023-06-30 → 2025-06-30) tiene 450.053 eventos que mezclan robo, hurto, lesiones, violencia sexual, faltas, peligro común, etc.
- La tesis de infelix midió tasas de denuncia por categoría en Lima 2019-2024: robo/hurto callejero 0,215, extorsión 0,147, violencia familiar 0,092, estafa 0,015 (`infelix/analysis/reporting_rate.md`). Sumar categorías en un solo puntaje sin reponderarlas mezcla escalas: una estafa denunciada equivale a ~76 hechos; un robo, a ~4,6.
- Robo/hurto callejero es la categoría espacialmente más legible: escena pública, alto volumen, buena geocodificación y poco rezago (`infelix/analysis/crime_type_observability.md`). Varias modalidades de hurto son "aorísticas" (sin hora ni lugar del hecho fiables) o no ocurren al peatón.
- La UX (`wachi-ux/UX.md`) solo nombra robo/arrebato de celular. "Robo/hurto de celular" como modalidad explícita son 3.353 filas útiles: la UI debe hablar de robo/hurto, no de celular.

## Decisión

El riesgo se calcula solo con filas de `tipo_hecho = 'PATRIMONIO (DELITO)'` y `subtipo_hecho ∈ {ROBO, HURTO}`, **excluyendo** las modalidades:

- `HURTO DE VEHICULO`
- `HURTO DE ACCESORIOS Y AUTOPARTES DE VEHICULOS`
- `ASALTO Y ROBO DE VEHICULOS`
- `HURTO AGRAVADO EN CASA HABITADA`

Las demás categorías no suman al riesgo. No hay pesos por categoría (`w_type = 1`).

## Justificación

- Una sola categoría elimina el problema de mezclar escalas de subregistro sin introducir pesos que no sabemos calibrar.
- Con coordenadas limpias (ADR 008) son 91.017 eventos en 24 meses, suficientes para H3 res 8 (ver addendum de ADR 006).
- Coincide con lo que la UX promete y con lo que el paper muestra que se puede ubicar.

## Consecuencias

- Homicidio, lesiones, violencia sexual, extorsión y peligro común no entran al puntaje. Si la UX los necesita, se agregan como **capa separada** con su propio indicador, nunca sumados al riesgo sin un peso explícito.
- `filtro_peatonal.yml` deja de definir el universo del modelo; queda como catálogo de referencia.
- El riesgo pasa a significar "robo/hurto callejero registrado", lo que hay que decir así en la API y en la UI.

## Criterios de revisión

- La taxonomía UX (`wachi-tc1.3`) decide mostrar otra categoría.
- Aparece una tasa de denuncia defendible para otra categoría que permita ponderarla.
