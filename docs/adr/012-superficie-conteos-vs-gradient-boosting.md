# ADR 012 — Superficie de riesgo: conteos observados vs gradient boosting (experimento)

> **Segunda integración (2026-10-07):** no adoptado. Ver la sección Resultado.

## Estado
No adoptado (2026-10-07). La superficie es de conteos (ADR 009, 016).

## Contexto

- En la tesis de infelix, para robo/hurto callejero en Lima (H3 res 8, entrenamiento 2019-2022, prueba 2023, orden de celdas dentro de cada distrito), un gradient boosting tabular (`HistGradientBoosting` de sklearn) con variables de forma urbana (manzana censal, OSM, transporte) obtuvo ρ intra-distrito 0,59, recall@10 % 0,37 y PAI 3,7; el historial de conteos (lo que hace Wachi) 0,44-0,46, 0,28-0,32 y 2,8-3,2; un modelo de deep learning espacio-temporal (STGNN) 0,42, 0,31 y 3,1 (`infelix-slr-supplementary/simbig/metrics/q4_modality_ladder_metrics.csv`). Afinar hiperparámetros movió +0,005.
- El gradient boosting predice desde la estructura urbana, así que también asigna valor a celdas sin denuncias geocodificadas, el problema de ADR 010.
- `CLAUDE.md` exige scoring explicable, sin modelos black-box.
- Las variables viven en infelix (investigación no publicada, otra licencia) y el modelo es anual, sin franja horaria.

## Decisión

Se descarta deep learning. Se comparan dos superficies con el mismo protocolo y se adopta la que gane según la regla de abajo.

- **A, conteos**: eventos de ADR 007/008 por celda en el periodo de entrenamiento.
- **B, gradient boosting**: `HistGradientBoostingRegressor` sobre `log1p(conteo)` por celda, con las variables de `infelix/data/silver/h3_feature_matrix.parquet` del año del entrenamiento.

Protocolo:

1. Universo y limpieza: ADR 007 y 008; celdas H3 res 8 de Lima Metropolitana.
2. Entrenamiento: 2022-07-01 → 2023-06-30. Prueba: 2023-07-01 → 2024-04-30, el último tramo con cobertura útil estable (47-58 % por mes).
3. Métricas (las del paper, para comparar): ρ de Spearman intra-distrito (macro), recall@10 % y PAI@10 % contra los conteos de prueba.
4. Métrica de producto: sobre un set fijo de pares origen-destino (`wachi-06f.1`), % de consultas donde SAFE difiere de FAST con cada superficie.

Regla de decisión: **B reemplaza a A** si mejora ρ intra-distrito en ≥ 0,05 sin empeorar recall@10 %, **y** cambia rutas al menos tanto como A, **y** cada celda puede explicarse con sus 3 variables principales (SHAP o importancia local). Si no, se queda A.

## Justificación

- El paper sugiere que B ordena mejor, pero en otro periodo y con otro objetivo (tesis, no rutas). Medirlo con los datos y la ventana de Wachi es barato y evita adoptar un modelo más complejo por un resultado ajeno.
- La métrica de producto evita elegir un modelo que ordena mejor celdas pero no cambia rutas.

## Consecuencias

- Si gana B: se necesita un ADR que cambie la regla de explicabilidad de `CLAUDE.md`, resolver dónde viven y con qué licencia las variables (repo `sidpol-lake` o propio de Wachi) y cómo derivar la franja horaria (por ejemplo, proporciones históricas por celda).
- Si gana A: ADR 007-011 quedan como el modelo v2 sin cambios.
- El experimento usa material de investigación no publicado (infelix): queda en uso interno y no se publica.

## Criterios de revisión

- Entra la fuente vigente y cambia la cobertura de prueba.
- El diseño de UX deja de necesitar un orden fino de celdas (por ejemplo, si solo muestra zonas).

## Resultado (2026-10-07)

Se corrió el protocolo (entrenamiento 2022-07..2023-06, prueba 2023-07..2024-04, H3 res 8):

| Superficie | ρ intra-distrito | recall@10 % | PAI@10 % |
|---|---|---|---|
| Conteos 24 meses | 0,884 | 0,402 | 4,02 |
| Conteos 12 meses | 0,864 | 0,405 | 4,05 |
| Gradient boosting (forma urbana) | 0,811 | 0,429 | 4,29 |

- El gradient boosting no supera a los conteos.
- La ventaja reportada en la tesis no se reproduce: el oráculo de infelix tiene varias filas por (celda, año) y la evaluación las unía sin deduplicar (bug `slr-natb` en infelix). Sin duplicados, historial 0,834 vs gradient boosting 0,813.
- Las variables más importantes del modelo son población, decil de pobreza y área construida: su explicación sería "riesgo alto por pobreza", inaceptable para el producto.
- Solo gana cuando la cobertura útil cae a ~5 % (simulado), el régimen de la fuente desde 2024-11; en ese régimen ninguna superficie es buena.

