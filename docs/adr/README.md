# ADR de Wachi v2

| ADR | Tema | Estado |
|---|---|---|
| [003](003-risk_total-integral-discretized.md) | Exposición como integral discretizada | Vigente |
| [007](007-universo-robo-hurto-callejero.md) | Universo: robo y hurto callejero | Aceptado |
| [008](008-coordenadas-reales-y-conteo.md) | Coordenadas reales (pile-up ≤ 30), una fila = un evento | Aceptado |
| [009](009-ventana-temporal-y-pesos.md) | 24 meses a `data_as_of`, peso uniforme, sin franja por lugar | Aceptado |
| [010](010-cobertura-sin-datos.md) | "Sin datos" es un indicador, no un riesgo | Aceptado |
| [011](011-scoring-y-presentacion.md) | Presentación: dos tarjetas, beneficio en minutos y cuadras | Aceptado |
| [012](012-superficie-conteos-vs-gradient-boosting.md) | Gradient boosting como superficie | No adoptado |
| [013](013-descartar-rutas-por-recintos-cerrados.md) | No rutear por dentro de recintos cerrados | Aceptado |
| [014](014-motor-grafo-peatonal-propio.md) | Motor: grafo peatonal OSM propio (β=0,5) | Aceptado |
| [015](015-regla-de-seleccion.md) | Regla: menor exposición con desvío ≤ min(15 %, 5 min), si reduce ≥ 20 % | Aceptado |
| [016](016-superficie-radio-150m.md) | Superficie: robos registrados a ≤ 150 m | Aceptado |

Los ADR 001, 002, 004, 005 y 006 quedaron en `wachi-research` (reemplazados o archivados).
Evidencia: `../experiments/2026-10-comparacion-ruteo.md`.
