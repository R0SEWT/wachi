# ADR 010 — Cobertura explícita: "sin datos" es un indicador, no un riesgo

> **Segunda integración (2026-10-07):** revisado; el indicador no cambia la regla de selección (ADR 015).

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- `CLAUDE.md` ya establece que "sin datos" ≠ "seguro", pero el modelo no lo distingue: una celda sin denuncias tiene riesgo 0.
- La cobertura de geocodificación es desigual y regresiva: geocodificación × pobreza ρ = −0,60 (`infelix/analysis/equity_audit_lima.md`). En 2024-11..2025-06, San Isidro tiene 32,9 % de filas útiles y Los Olivos 2,4 %, Puente Piedra 2,5 %, SJL 4,5 %. Sin un indicador, los distritos pobres parecen "tranquilos".
- La UX pide marcar tramos con "datos insuficientes" (`wachi-ux/UX.md`, línea gris punteada) y avisa que "menos denuncias no siempre significa tranquilidad".
- Ver el paper de SIMBig 2026 (README) sobre el efecto del sesgo de registro y la geocodificación.

## Decisión

1. El risk store guarda por celda `risk_score` y `n_events`, y por distrito `cobertura_util` (filas útiles / filas del universo de ADR 007 en la ventana).
2. La API devuelve por punto muestreado `risk` y `data_sufficient` (`n_events` de la celda, sumando las 6 franjas, ≥ 5), y por ruta la lista de tramos sin datos suficientes. Tramo provisional: puntos consecutivos en la misma celda H3.
3. **La regla de selección (ADR 015) no cambia**: el indicador informa, no corrige.

## Justificación

- Es la forma más simple de cumplir "sin datos ≠ seguro" sin inventar un modelo de corrección.
- Hace visible el sesgo en vez de esconderlo detrás de un número.

## Consecuencias

- No corrige el sesgo, solo lo declara: es honesto y explicable, no equitativo.
- La API agrega campos (`data_sufficient`, tramos insuficientes); requiere actualizar `openapi.yaml` y los schemas.

## Criterios de revisión

- El umbral de 5 eventos o la definición de tramo no funcionan con el diseño final de la UX.
- La comparación de ADR 012 adopta una superficie predictiva, que da valor también a celdas sin denuncias.
