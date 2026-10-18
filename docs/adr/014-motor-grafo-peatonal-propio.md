# ADR 014 — Motor de ruteo: grafo peatonal OSM propio

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- ADR 001 eligió ORS hosted (`foot-walking`, K=3 alternativas) y seleccionar después por riesgo. La comparación de 51 pares (`docs/experiments/2026-10-comparacion-ruteo.md`, PR #12) mostró sus límites:
  - Las alternativas de ORS se ven distintas en el mapa pero pasan por zonas parecidas: con la regla de ADR 015, ORS ofrece una ruta tranquila distinta de la rápida en el 18 % de las consultas; un grafo propio que minimiza el riesgo, en el 33 %, con menos desvío por punto de reducción (2,8 m frente a 4,2 m).
  - Sin `preference`, ORS usa `recommended`, que no es la ruta más corta (+2,5 % en mediana, hasta +12 %).
  - Cada consulta tarda ~1 s y consume cuota (~2.000 por día en el plan gratuito).
  - `avoid_polygons` empeoró 3 de 4 pares en una prueba previa: esquiva una zona y entra en otras.
- Un grafo propio construido desde un extracto de OSM coincide con ORS `shortest` con una diferencia ≤ 0,8 % (p95) y resuelve cada búsqueda en 2-7 ms.

## Decisión

1. El backend rutea sobre un **grafo peatonal propio** construido desde un extracto de OSM versionado (Geofabrik, con su sha256), con un filtro de vías caminables tipo osmnx `walk` y sin sentidos únicos.
2. Cada ruta es un camino de costo mínimo (Dijkstra) con costo de arista `largo · (1 + β · r)`, donde `r` es el riesgo de la arista normalizado por el p75 de la red (ADR 016). La ruta rápida usa β = 0; la tranquila, **β = 0,5**.
3. ORS `foot-walking` con `preference=shortest` queda como **respaldo** cuando el origen o el destino caen fuera del grafo o el grafo no encuentra ruta.
4. Se descartan `avoid_polygons` y los puntos intermedios forzados.

## Justificación

- Encuentra la ruta tranquila entre todas las calles, no entre tres que eligió otro motor.
- Es el mismo criterio para las dos tarjetas, así que la tranquila nunca sale más corta que la rápida por diferencias entre motores.
- Sin cuota ni latencia de red.

## Consecuencias

- Hay que mantener el extracto de OSM y el script que construye el grafo (refresco periódico, tests con casos conocidos).
- El grafo no da indicaciones paso a paso; si la UX las pide, se evalúa GraphHopper self-hosted con `custom_model`.
- El extracto de OSM es ODbL: el entregable debe atribuirlo.

## Criterios de revisión

- La calidad del grafo (veredas, accesos) produce rutas que ORS no daría.
- La UX necesita indicaciones paso a paso.
