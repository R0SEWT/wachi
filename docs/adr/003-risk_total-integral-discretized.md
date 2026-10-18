# ADR 003 — risk_total como integral discretizada (Σ risk_cell * step_m)

> **Segunda integración (2026-10-07):** sigue vigente. La exposición de una ruta se calcula con la superficie de ADR 016, muestreando cada 20 m.

## Estado
Aceptado (MVP)

## Decisión
Definir risk_total por ruta como:
  risk_total = Σ risk_cell(p_i) * step_m
donde p_i son puntos muestreados cada step_m metros sobre la geometría.

## Justificación
- Invarianza aproximada al sampling step
- Interpretación como exposición acumulada (integral sobre longitud)
- Estabilidad y auditabilidad

## Consecuencias
- β se calibra respecto a unidades de metros
- Se reporta risk_per_km como métrica secundaria
