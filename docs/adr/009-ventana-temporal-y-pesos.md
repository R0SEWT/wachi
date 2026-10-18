# ADR 009 — Ventana temporal y pesos: 24 meses, peso uniforme

> **Segunda integración (2026-10-07):** revisado; la franja horaria por lugar se elimina (ADR 016).

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- ADR 005 fijó una ventana de 24 meses con decaimiento `exp(-días/λ)`, λ = 90. El decaimiento nunca se evaluó contra un resultado (`infelix/design/paper-sesgo-observado-vs-latente.md`).
- La fuente histórica está congelada al 2025-06-30. Con λ = 90 y referencia 2025-06-30, la masa efectiva es el 3,3 % de los eventos; con referencia en la fecha de hoy, prácticamente cero.
- La geocodificación de robo/hurto en `LIMA.parquet` cambia de régimen en 2024: 47-58 % de filas útiles por mes hasta 2024-04, 0,3-2,8 % en 2024-05..07, 60 % en 2024-08 y 7-10 % desde 2024-11. Un peso por recencia premia justamente los meses peor cubiertos.
- Hora: 26,9 % de los eventos útiles están en minuto :00 y 1,6 % en 00:00:00 exacto. Con franjas de 4 h el redondeo es inocuo; la tesis recomienda no usar resoluciones menores a un turno (`infelix/analysis/crime_chronotypes.md`, `chronotypes_by_crimetype.md`).

## Decisión

1. Ventana de **24 meses que termina en `data_as_of`** (la última fecha con datos de la fuente usada; hoy 2025-06-30). `reference_date = data_as_of`, nunca la fecha del build.
2. **Peso uniforme**: cada evento vale 1. Se elimina `lambda_days`.
3. **Sin franja horaria por lugar** (ADR 016): la superficie es de todo el día. Si la UX necesita hora, se multiplica por un perfil horario por distrito. No hay tratamiento especial para la medianoche exacta ni para el minuto :00.

## Justificación

- Con la fuente congelada y la cobertura reciente colapsada, "reciente" no significa "más informativo".
- Un conteo sin pesos es la versión más fácil de explicar ("N robos/hurtos registrados en 2 años").
- La medianoche exacta es 1,6 %: tratarla aparte añade una regla sin efecto material.

## Consecuencias

- El modelo no reacciona a cambios recientes; es una "climatología" de dos años.
- La franja horaria por lugar se elimina: con H3 res 8, 41 % de los pares (celda, franja) tenían menos de 5 eventos, y ordenar con la franja propia no predice mejor que con el día completo.
- `config/model_params.yml` pierde `lambda_days`; `normalize.py` usa `w = 1`.

## Criterios de revisión

- Entra la fuente vigente con cobertura útil demostrada (`wachi-hdr`).
- La UX cierra otros bordes de franja (el prototipo tiene 5 franjas desfasadas 2 h): se re-versiona el store.
- La comparación de ADR 012 muestra que otra ventana ordena mejor.
