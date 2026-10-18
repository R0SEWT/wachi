# Comparación de ruteo, superficie de riesgo, regla de selección y recintos (wachi-06f.1)

- **Fecha:** 2026-10-07
- **Estado:** resultados de experimento, insumo para reevaluar ADR 001, 002, 003, 006, 011 y 013. No cambia código de `src/`.
- **Script:** `scripts/experiments/comparacion_ruteo.py` · **pares:** `config/experiments/od_pairs.yaml` · **salidas crudas:** `data/experiments/` (gitignored).

> **Aviso (política CIP).** Usa denuncias SIDPOL/MININTER, que son datos de terceros sin licencia publicada. Este documento solo trae agregados por ruta y por estrategia, nunca eventos ni sus coordenadas. Se escribió con apoyo de IA generativa: el equipo debe validar cifras y conclusiones antes de decidir.

## Resumen

| # | Decisión | Recomendación | Evidencia principal (medida) |
|---|---|---|---|
| 1 | Motor de ruteo | **B: grafo peatonal OSM propio + Dijkstra**, con ORS como respaldo opcional | Con la misma superficie (S150) y la misma regla, B ofrece la 2.ª tarjeta en el **33 %** de las consultas y A en el **18 %**. Fuera de muestra, la reducción es parecida (~31 % frente a ~30 %). B coincide con ORS `shortest` con una diferencia de **≤ 0,8 %** (p95). Tarda unos 2-7 ms por búsqueda; ORS, ~1 s por llamada. |
| 2 | Superficie | **S150** (conteo a ≤150 m) para costo y selección; H3 res 8 solo para comunicar y medir cobertura | Con S8 la 2.ª tarjeta aparece en **4-6 %** de las consultas con cualquiera de los dos motores. Fuera de muestra, las Tranquilas S150 reducen la exposición futura S150 en el **79-88 %** de los pares distintos (88 % con B · β=0,5), pero **no** la S8: son desvíos dentro de las mismas celdas. |
| 3 | Regla de selección | **Adoptar la regla** (Rápida = más corta; Tranquila con tope min(+15 %, +5 min); 2.ª tarjeta si reduce ≥20 %) en lugar de la J actual para elegir tarjetas | Con las unidades actuales, en la J SAFE el término de riesgo pesa ~74 veces el de distancia (mediana): equivale a "mínima exposición" sin tope. En el 6 % de los pares supera el tope de desvío (máximo +658 m). |
| 4 | Filtro de recintos (ADR 013) | **Mantenerlo**: en A como post-filtro y en B como exclusión de aristas, permitiendo el recinto de origen o destino | Solo 4 de 51 pares tienen una alternativa de ORS que cruza un recinto, pero en 3 de esos 4 la regla elige justamente esa ruta como Tranquila: dentro del hospital no hay denuncias geocodificadas. En B, la exclusión no alargó ninguna Rápida (0/51). |
| – | β por defecto (B) | **β = 0,5** | Los β mayores mejoran la reducción dentro de la muestra pero no fuera de ella (S150 futura: 15,9 % con β=0,5, entre 14,2 y 15,8 % con β=1-4) y exceden más el tope de desvío (10 % con β=0,5 frente a 20-35 %). |

## 1. Preguntas

1. **Ruteo.** (A) ORS `foot-walking` con K=3 y selección posterior, como hoy (`src/routing/ors_client.py`, con los mismos parámetros de alternativas), frente a (B) un grafo peatonal OSM propio con Dijkstra y costo de arista `largo·(1 + β·r)`, donde `r = riesgo / p75` de la superficie.
2. **Superficie.** (S8) conteo por celda H3 res 8 frente a (S150) conteo de eventos a ≤150 m de cada punto.
3. **Regla de selección.** Rápida = la más corta. Tranquila = la de menor exposición con desvío ≤ min(+15 % de la distancia, +5 min a 5 km/h). La 2.ª tarjeta se muestra solo si reduce la exposición ≥20 %. Como referencia, qué elige la J actual con SAFE (α, β) = (1, 2) sobre las rutas de A.
4. **Recintos (ADR 013).** Metros dentro de `amenity=hospital|university` y `landuse=cemetery|military`, sin contar el recinto donde está el origen o el destino.

## 2. Método

### 2.1 Datos y versión

| Insumo | Versión |
|---|---|
| Denuncias `LIMA.parquet` (SIDPOL histórico) | sha256 `15ed53f29d9a489d4fa614899b2f2e1c0fcb51d0e6860c43c5012a3d5831dd63` |
| OSM Geofabrik `peru-latest.osm.pbf` (2026-10-07) | sha256 `7be6d84ad9eaca07990442afb496cf1a291cf86964b1ee331ac0116f05756c8c` |
| ORS hosted `foot-walking` | consultado el 2026-10-07 |

**Universo (ADR 007/008).** `tipo_hecho='PATRIMONIO (DELITO)'`, `subtipo_hecho IN ('ROBO','HURTO')`, sin las modalidades vehiculares ni `HURTO AGRAVADO EN CASA HABITADA`, `estado_coord='CON COORDENADA'`. Se descartan las coordenadas (lat/lon redondeadas a 5 decimales) que acumulan más de 30 filas, contando todas las filas de la ventana. La ventana va del 2023-07-01 al 2024-04-30 en fecha local (`fecha_hora_hecho`, epoch-ms UTC → America/Lima). La superficie es de todo el día, sin franja.

| Paso | Filas |
|---|---|
| Filas de la ventana (todas) | 456.095 |
| Universo con coordenada | 114.941 |
| Tras descartar coordenadas con >30 filas | 76.640 |
| Dentro del área de trabajo | 75.036 (1.012 celdas H3 res 8 activas) |

**Área de trabajo.** Bbox S −12,28 / O −77,13 / N −11,87 / E −76,85. Cubre los 51 pares con al menos 2 km de margen, lo que el script verifica al correr.

### 2.2 Superficies

- **S8:** conteo de eventos de la celda H3 res 8 (ADR 006) que contiene el punto.
- **S150:** número de eventos a ≤150 m del punto (árbol KD en UTM 18S, solo en memoria).
- **Exposición de una ruta:** `E = Σ riesgo(pᵢ) × km`, con los pᵢ en el punto medio de cada tramo de 20 m. Cada superficie tiene su unidad, así que solo se comparan variaciones porcentuales.
- **Normalización para el costo de B:** `r = riesgo / p75`, con el p75 medido sobre ~962 mil puntos de la red cada 20 m. Da p75(S8) = 152 y p75(S150) = 12. Hay un 10 % de la red con S8 = 0 y un 26 % con S150 = 0. La correlación de Spearman entre S8 y S150 en la red es 0,80.

### 2.3 Motores

- **A (ORS):** `POST /v2/directions/foot-walking/geojson` con `alternative_routes = {target_count: 3, share_factor: 0.6, weight_factor: 1.4}`, idéntico a `ors_client.py`, sin `preference`, lo que equivale a `recommended`. Devolvió 3 rutas en 39 pares, 2 en 11 y 1 en uno.
- **B (grafo propio):** vías `highway=*` del pbf filtradas al estilo osmnx `walk`. Se excluyen `motorway*`, `cycleway`, `construction`, `busway`, `area=yes`, `foot=no`, `access=private|no` (salvo `foot=yes|designated|permissive`), `service=private`, las calles con `sidewalk*=separate` y los `trunk*` sin acera ni permiso peatonal. Las vías se cortan en los nodos compartidos y se queda la componente conexa mayor: 261.132 nodos, 367.485 aristas, 18.032 km, sin sentidos únicos. Dijkstra (`scipy.sparse.csgraph`) usa como cota el costo de la ruta β=0 bajo el β pedido, lo que garantiza el óptimo. El costo de arista es `largo·(1 + β·r̄)`, con `r̄` = media de `r` en las muestras de 20 m de la arista.
- **B+recintos:** igual que B, pero se eliminan las aristas con ≥50 % de su largo dentro de un recinto que no contenga el origen ni el destino.

### 2.4 Pares origen-destino (51)

Generados con `--gen-pairs` (semilla 20261007) y ajustados a un nodo de la red (el ajuste mediano fue de ~25 m en los casos fijos).

| Estrato | n | Contenido |
|---|---|---|
| caso | 7 | Personas del prototipo UX (Elena, Mateo) y los 6 del prototipo previo (Elena aparece en ambos) |
| recinto | 4 | Origen dentro de los hospitales Rebagliati y Almenara; destino dentro de la PUCP y de la Ciudad Universitaria de San Marcos. Con Mateo (PUCP) y San Marcos previo suman **6 pares con origen o destino dentro de un recinto**. El Dos de Mayo se descartó porque ninguna vía peatonal pública de OSM entra al polígono. |
| cobertura_alta | 20 | Miraflores, San Isidro, Lince, Jesús María, Cercado: 4 pares cada uno, origen a ≤1,2 km del ancla |
| cobertura_baja | 20 | SJL, Los Olivos, Comas, Villa El Salvador, Ate: 4 pares cada uno |

**Bandas de distancia** (camino más corto en B): 0,8-1,5 km (16 pares), 1,5-2,5 km (18) y 2,5-4 km (17). La cobertura medida confirma los estratos: la mediana de S150 por km de la ruta más corta es 30 en cobertura alta y 8,9 en cobertura baja, y la de S8, 319 frente a 128.

### 2.5 Estrategias evaluadas

Para cada estrategia y superficie de selección s ∈ {S8, S150}:

- **A:** candidatas = alternativas de ORS.
- **A+recintos:** igual, pero se descartan las alternativas con >20 m dentro de un recinto ajeno; si todas cruzan, se queda la que menos cruza (ADR 013).
- **B · β:** candidatas = {ruta β=0, ruta β}, con β ∈ {0,5, 1, 2, 4}.
- **B · multi:** {β=0} ∪ {las 4 β}.
- **B+recintos:** igual que B, sobre el grafo con exclusión.
- **J SAFE (referencia):** sobre las rutas de A, `argmin 1·distancia_m + 2·Σ r·paso_m` (ADR 002/003), con `r` = S8/6 como aproximación al riesgo por franja del RiskStore.

A todas se les aplica la misma regla (§1.3). Cada estrategia se evalúa **con la superficie con la que fue optimizada y con la otra**.

### 2.6 Validación fuera de muestra (añadida)

Evaluar con "la otra superficie" no resuelve la circularidad, porque cada superficie premia lo suyo. Por eso se agregó `--holdout`: las superficies, el p75, los costos de B y la selección se construyen con los eventos de 2023-07 a 2023-11 (36.272), y las rutas elegidas se evalúan con los de 2023-12 a 2024-04 (38.764), en S8 y en S150. Es el mismo corte A/B del análisis espacial previo.

### 2.7 Reproducción

```bash
uv run scripts/experiments/comparacion_ruteo.py --gen-pairs   # solo si se quiere regenerar los pares
uv run scripts/experiments/comparacion_ruteo.py --geojson     # experimento principal (~40 s con caché ORS)
uv run scripts/experiments/comparacion_ruteo.py --holdout     # validación fuera de muestra
```

Las opciones `--raw`, `--pbf` y `--env` aceptan otras rutas. Las respuestas de ORS se cachean en `data/experiments/cache/ors/`, con un contador de llamadas y un tope de `--ors-budget 120`.

## 3. Resultados (medido)

Todas las cifras son de 51 pares. "Distintos" son los pares donde la Tranquila ≠ la Rápida. Las reducciones son `1 − E_T/E_R`. Ninguna estrategia dejó consultas sin ruta (0 %): ORS respondió 200 en los 51 pares, y B ajusta a la componente conexa.

### 3.1 Agregado por estrategia (superficies de la ventana completa)

| Estrategia (motor · superficie de selección) | Tranquila ≠ Rápida | 2.ª tarjeta (≥20 %) | Reducción mediana, superficie propia | Reducción mediana, la otra superficie | Desvío mediano (m) | m de desvío por p.p. | Solape T∩R (20 m) | Jaccard celdas H3 |
|---|---|---|---|---|---|---|---|---|
| A · S8 | 20 % | 4 % | 8,3 % | 4,5 % | 102 | 15,7 | 0,41 | 0,71 |
| B · S8 · β=0,5 | 41 % | 6 % | 6,0 % | 4,7 % | 19 | 3,6 | 0,49 | 0,80 |
| A · S150 | 33 % | 18 % | 24,5 % | −2,9 % | 134 | 4,2 | 0,32 | 0,80 |
| **B · S150 · β=0,5** | **78 %** | **33 %** | 18,2 % | −1,1 % | 46 | 2,8 | 0,59 | 1,00 |
| B · S150 · β∈{0,5,1,2,4} | 80 % | 39 % | 19,6 % | −3,6 % | 85 | 4,5 | 0,52 | 1,00 |

Las columnas desde "Reducción mediana" en adelante se calculan solo sobre los pares distintos. El solape es la fracción del largo de la Tranquila a ≤20 m de la Rápida. La columna Jaccard compara las celdas H3 res 8 de las dos tarjetas.

**Lo que ve el usuario** (solo los pares con 2.ª tarjeta):

| Estrategia | 2.ª tarjeta | Reducción mediana (propia) | Reducción mediana (otra) | Desvío mediano | % del largo sin señal: Rápida → Tranquila |
|---|---|---|---|---|---|
| A · S8 | 4 % (2 pares) | 37,2 % | 22,7 % | 286 m · 3,4 min | 0 → 0 |
| B · S8 · β=0,5 | 6 % (3) | 26,1 % | 14,7 % | 164 m · 2,0 min | 0 → 0 |
| A · S150 | 18 % (9) | 33,7 % | −3,4 % | 134 m · 1,6 min | 4,3 → 5,4 |
| **B · S150 · β=0,5** | **33 % (17)** | 43,5 % | −0,5 % | 110 m · 1,3 min | 4,9 → 6,3 |
| B · S150 · multi | 39 % (20) | 42,0 % | −1,9 % | 158 m · 1,9 min | 4,8 → 6,0 |

### 3.2 Validación fuera de muestra (`--holdout`)

| Estrategia (entrenada 2023-07..11) | Tranquila ≠ Rápida | 2.ª tarjeta | Reducción en entrenamiento (2.ª tarjeta) | Reducción futura S8 (2.ª tarjeta) | Reducción futura S150 (2.ª tarjeta) | Pares distintos con reducción futura S8 > 0 | … con reducción futura S150 > 0 |
|---|---|---|---|---|---|---|---|
| A · S8 | 18 % | 4 % | 37,9 % | 36,4 % | 25,6 % | 78 % | 67 % |
| B · S8 · β=0,5 | 37 % | 4 % | 35,9 % | 29,8 % | 29,5 % | 89 % | 79 % |
| A · S150 | 37 % | 20 % | 37,9 % | −3,2 % | 29,6 % | 21 % | 79 % |
| **B · S150 · β=0,5** | 80 % | **37 %** | 40,9 % | −1,5 % | **30,8 %** | 24 % | **88 %** |
| B · S150 · multi | 84 % | 43 % | 38,2 % | −1,2 % | 30,8 % | 28 % | 84 % |

- Del entrenamiento al periodo futuro, la reducción de la 2.ª tarjeta se encoge de ~38-41 % a ~30 %: hay algo de sobreajuste, pero la mayor parte se sostiene.
- Las Tranquilas elegidas con S150 reducen la exposición futura **medida con S150**, pero no la medida con S8 (alrededor de −1 a −3 %). Las elegidas con S8 reducen ambas, pero casi nunca llegan al 20 %.

### 3.3 Calidad del grafo propio

| Comparación | n | Diferencia de largo, B más corta frente a ORS |
|---|---|---|
| frente a la más corta de las alternativas de ORS (`recommended`) | 51 | mediana −3,6 %, p5 −12,0 %, p95 0,0 %; en 18 % de los pares B es >10 % más corta |
| frente a ORS `preference=shortest` (subconjunto: casos, recintos y 1 par por distrito) | 21 | mediana +0,04 %, p95 +0,8 %; los 21 pares quedan dentro de ±5 % |

La ruta más corta de ORS `recommended` es un 2,5 % más larga que la de ORS `shortest` (mediana; p95 12 %). En los pares donde B es >10 % más corta, B0 va más por avenidas (tertiary 30 %, secondary 26 %, primary 11 % del largo, frente a 12 %, 16 % y 6 % en el resto): `recommended` evita las vías principales. La longitud geométrica calculada coincide con la que reporta ORS (−0,2 %).

### 3.4 Latencia y costo

| | p50 | p95 |
|---|---|---|
| A: llamada ORS (incluye red) | 986 ms | 1.096 ms |
| B: una búsqueda Dijkstra + reconstrucción, en proceso | 1,8 ms | 6,7 ms |
| B: consulta de 1 superficie con β=0 + 4 β | 8,8 ms | 25,9 ms |

La latencia de B varió de 2 a 3 veces entre corridas según la carga del equipo (en otra corrida, el p50 fue 3,5 ms y el p95 11,5 ms). La red de B se arma una vez desde el pbf (~25 s) y su caché pesa 52 MB.

**Llamadas ORS:** 112 en vivo, de un presupuesto de 120. Se gastaron 51 en los pares de la primera generación, de las cuales 40 quedaron sin uso al regenerar los pares tras excluir `busway`; 40 en los pares regenerados; y 21 en `shortest`. No se reutilizó la caché del prototipo previo (31 llamadas) porque los pares se ajustaron a nodos y cambiaron las coordenadas.

### 3.5 Sensibilidad a β (ruta β sin tope, frente a la más corta de B)

| Superficie | β | Ruta distinta de β=0 | Excede el tope de desvío | Desvío mediano | Reducción propia | Reducción otra | Reducción futura S8 (holdout) | Reducción futura S150 (holdout) |
|---|---|---|---|---|---|---|---|---|
| S8 | 0,5 | 37 % | 2 % | 25 m (1,8 %) | 8,6 % | 4,7 % | 7,3 % | 7,4 % |
| S8 | 1 | 55 % | 10 % | 50 m (2,7 %) | 8,3 % | 3,5 % | 8,8 % | 8,2 % |
| S8 | 2 | 59 % | 16 % | 97 m (4,2 %) | 11,9 % | 3,4 % | 9,9 % | 7,6 % |
| S8 | 4 | 61 % | 16 % | 107 m (4,5 %) | 11,6 % | 4,3 % | 8,6 % | 7,7 % |
| S150 | **0,5** | 88 % | **12 %** | 75 m (4,0 %) | 20,7 % | −1,0 % | −1,4 % | **15,9 %** |
| S150 | 1 | 94 % | 22 % | 123 m (5,0 %) | 23,3 % | −3,0 % | −2,0 % | 15,8 % |
| S150 | 2 | 96 % | 31 % | 143 m (5,9 %) | 25,2 % | −3,6 % | −3,2 % | 14,2 % |
| S150 | 4 | 96 % | 47 % | 254 m (12,2 %) | 25,8 % | −4,1 % | −4,5 % | 15,4 % |

Las medianas se calculan sobre los pares cuya ruta β difiere de β=0. Las columnas de holdout vienen de la corrida `--holdout`, cuyos porcentajes de "excede el tope" son parecidos (10-35 %).

### 3.6 J SAFE actual (referencia, sobre las rutas de A)

- Elige una ruta que no es la más corta en el **24 %** de los pares, y coincide con la Tranquila de la regla (A · S8) en el **94 %**.
- En el **6 %** de los pares elige una ruta por encima del tope de desvío (máximo +658 m). La reducción S8 mediana cuando difiere es 8,3 %.
- Mediana de `β·risk_total / (α·distancia)` = **74**: el riesgo domina y la distancia casi no cuenta.

### 3.7 Recintos (ADR 013)

- **A:** 4 de 140 rutas (2,9 %) cruzan más de 20 m de un recinto ajeno, y están en 4 pares distintos. En ningún par cruzan todas las alternativas.

  | Par | Ruta de ORS | Recinto | m dentro | Rol |
  |---|---|---|---|---|
  | Elena (UX) | alt. 3 | Hospital Rebagliati | 352 | Tranquila de A·S150 (sin filtro) |
  | Jesús María 2 | alt. 2 | Hospital Rebagliati | 352 | Tranquila de A·S150 (sin filtro) |
  | San Isidro 3 | alt. 2 | Hospital Rebagliati | 352 | Tranquila de A·S150 (no llega a 2.ª tarjeta) |
  | San Marcos (previo) | alt. 1 = más corta | PUCP | 497 | Rápida de A; con el filtro, la Rápida pasa a ser la alt. 2, **+442 m (+14 %)** |

- **B:** la ruta más corta no cruzó recintos en ningún par (0/51). Las rutas β sí cruzan en el 2,2 % de los casos (el atajo por el Rebagliati en Elena y Jesús María 2). Con la exclusión, cruzan 0 %, y ninguna ruta más corta se alargó (0/51).
- **Efecto en la 2.ª tarjeta (S150):** A pasa de 18 % a 16 % con el filtro. B · β=0,5 queda en 33 % con o sin exclusión.

### 3.8 Por estrato

Solo es indicativo: los grupos son de 4 a 20 pares.

| Estrato | n | Estrategia | 2.ª tarjeta | Reducción mediana (propia, distintos) | Desvío mediano (m, distintos) | Reducción futura S150 (holdout, distintos) |
|---|---|---|---|---|---|---|
| caso | 7 | A · S150 | 29 % | 39,6 % | 139 | 47,0 % |
| caso | 7 | B · S150 · β=0,5 | 57 % | 32,7 % | 148 | 26,9 % |
| caso | 7 | B · S8 · β=0,5 | 0 % | 10,7 % | 56 | 6,7 % |
| recinto | 4 | A · S150 | 25 % | 33,7 % | 53 | 29,5 % |
| recinto | 4 | B · S150 · β=0,5 | 75 % | 29,4 % | 55 | 18,2 % |
| cobertura alta | 20 | A · S150 | 10 % | 15,6 % | 101 | 13,3 % |
| cobertura alta | 20 | B · S150 · β=0,5 | 20 % | 11,7 % | 22 | 9,1 % |
| cobertura alta | 20 | B · S8 · β=0,5 | 5 % | 6,8 % | 18 | 7,2 % |
| cobertura baja | 20 | A · S150 | 20 % | 27,0 % | 142 | 6,4 % |
| cobertura baja | 20 | B · S150 · β=0,5 | 30 % | 14,8 % | 32 | 8,4 % |
| cobertura baja | 20 | B · S8 · β=0,5 | 10 % | 11,6 % | 55 | 15,4 % |
| 0,8-1,5 km | 16 | A · S150 | 6 % | 12,7 % | 137 | 6,9 % |
| 0,8-1,5 km | 16 | B · S150 · β=0,5 | 25 % | 14,8 % | 19 | 7,1 % |
| 1,5-2,5 km | 18 | A · S150 | 28 % | 27,9 % | 101 | 25,9 % |
| 1,5-2,5 km | 18 | B · S150 · β=0,5 | 39 % | 15,1 % | 38 | 11,8 % |
| 2,5-4 km | 17 | A · S150 | 18 % | 24,0 % | 136 | 14,8 % |
| 2,5-4 km | 17 | B · S150 · β=0,5 | 35 % | 25,3 % | 116 | 18,6 % |

**Casos UX.** En **Elena**, la J SAFE no cambia de ruta. Con A · S150, la Tranquila es la alternativa que cruza el Rebagliati; con el filtro pasa a ser la alt. 2 (+151 m, −34 % S150). Con B · S150 · β=0,5, la Tranquila también cruza el hospital, y con la exclusión toma otra ruta. En **Mateo**, A y B coinciden en una Tranquila de ~+140 m con −48 a −50 % de exposición S150, que sí se muestra.

## 4. Lo que se infiere (no medido directamente)

- **Las alternativas de ORS no apuntan al riesgo.** Por construcción (`share_factor`/`weight_factor`), ORS genera alternativas geométricamente distintas pero ciegas al riesgo. Por eso A encuentra una Tranquila útil en menos pares y paga más desvío por punto de reducción (4,2 frente a 2,8 m/p.p. con S150, y 15,7 frente a 3,6 con S8). Un motor que optimiza el costo encuentra desvíos cortos que ORS no propone.
- **Las dos superficies miden escalas distintas.** S150 discrimina entre calles paralelas: sus Tranquilas tienen Jaccard H3 = 1,0, es decir, se quedan en las mismas celdas. S8 no puede ver esos desvíos, y solo distingue rutas que cambian de celda, que suelen ser desvíos largos y raros dentro del tope. Que S150 no reduzca S8 no contradice a S150: es una diferencia de resolución. Lo coherente es que la superficie de S8 no sirve para elegir entre cuadras.
- **La persistencia temporal de S150 no prueba riesgo real.** La reducción futura en S150 se sostiene (88 % de los pares), pero eso también ocurriría si la señal fuera un patrón estable de geocodificación, como denuncias asignadas a la avenida de la dirección. El análisis espacial previo, en 4 distritos centrales, encontró que el 62 % de las direcciones tiene número y que el 66 % de los eventos comparte coordenada. Separar las dos cosas requiere otra fuente (por ejemplo, encuestas de victimización o datos municipales).
- **"Sin datos ≠ seguro" (ADR 010).** Las Tranquilas S150 tienen 1-2 p.p. más de largo sin señal que las Rápidas (4,9 → 6,3 % con B · β=0,5). El caso de los recintos es la versión extrema: la ruta por dentro del hospital parece la más tranquila precisamente porque allí no se geocodifican denuncias.
- **B como Rápida.** La Rápida de B es la más corta de verdad. Va más por avenidas que la de ORS `recommended` y es un 3,6 % más corta (mediana). Para la UX puede convenir una penalidad suave por tipo de vía, pero este experimento no lo evaluó.

## 5. Limitaciones

- **Muestra pequeña:** 51 pares, con estratos de 4 a 20. Las diferencias entre estratos son indicativas.
- **Una sola ventana** (10 meses, de 2023-07 a 2024-04) y **sin franja horaria**. El holdout usa 5 meses para entrenar y 5 para evaluar, el mismo par de periodos que el análisis espacial previo.
- **El grafo B no modela** cruces peatonales, semáforos, la seguridad de cruzar avenidas, las aceras faltantes ni la iluminación. Tampoco tiene sentidos únicos, que es lo correcto a pie. Su calidad se midió solo por largo contra ORS `shortest` en 21 pares.
- **El filtro peatonal es "tipo osmnx"**, con dos agregados: `trunk` sin acera y `busway`/`bus_stop`. Excluir `busway` obligó a regenerar los pares, porque cambió los índices de nodos; ver §3.4.
- **Recintos:** los polígonos se encogen 5 m para no contar veredas del borde; una ruta "cruza" si recorre >20 m dentro; en B se bloquea la arista si ≥50 % de su largo está dentro. La lista de recintos es la de ADR 013. El hospital Dos de Mayo no tiene vías públicas internas en OSM, así que no se pudo probar.
- **J SAFE:** se aproximó el riesgo por franja con S8/6 en lugar del RiskStore real, que tiene decaimiento y pesos (ADR 009). La conclusión sobre la dominancia del riesgo depende de esa escala.
- **ORS:** respuestas de un solo día y con la preferencia por defecto. La latencia incluye la red desde Lima.
- **Umbrales sin barrido:** el 20 % de la 2.ª tarjeta y el tope de +15 %/+5 min son los propuestos y no se exploraron. Tampoco se probaron otros radios para S150 (el análisis espacial previo comparó 100, 150 y 200 m).

## 6. Recomendaciones por decisión

1. **Ruteo → B (grafo propio).** Duplica las consultas con alternativa útil (S150: de 18 % a 33 %, con 37 % fuera de muestra), con desvíos menores por punto de reducción, latencia de milisegundos y sin cuota externa. El grafo coincide con ORS `shortest` (p95 0,8 %). Esto toca ADR 001, que debe pasar a "ruteo propio", y ya cumple sus criterios de revisión: "ORS no entrega alternativas suficientemente diversas" y "se necesita control fino". Hace falta un ADR nuevo que fije el filtro peatonal, el versionado del pbf y la actualización de la red. ORS puede quedar como respaldo o como verificación de calidad. Antes de producción conviene revisar la penalidad por tipo de vía de la Rápida (§4).
2. **Superficie → S150 para costo y selección.** Es la única con la que la regla produce una 2.ª tarjeta en una fracción relevante de consultas, y su beneficio se sostiene fuera de muestra en la misma métrica. Mantener H3 res 8 para coberturas, mensajes de "zona" y ADR 010, y revisar ADR 006, que hoy fija res 8 como unidad de scoring. **Pendiente:** contrastar S150 con una fuente independiente antes de comunicar que una ruta tiene "menos robos" (§4).
3. **Regla → adoptarla para elegir las dos tarjetas** y retirar J para la UI. Revisa ADR 002/003/011. La exposición integrada (`Σ riesgo × km`) sigue siendo la métrica. Lo que cambia es la combinación con la distancia, que pasa de la suma ponderada al tope explícito más el umbral de 20 %. Con la escala actual, la J SAFE es "mínima exposición sin tope". En B, β define qué rutas se generan y la regla, cuál se muestra.
4. **Recintos → mantener ADR 013** y aplicarlo en ambos motores (post-filtro en A, exclusión de aristas en B), permitiendo el recinto de origen o destino. Es poco frecuente (≈8 % de los pares), pero cuando aparece, la selección por exposición **elige** el atajo por el recinto. Agregar un test de regresión con el caso Rebagliati.
5. **β por defecto = 0,5** (con S150). Logra rutas distintas en el ~90 % de los pares, excede el tope en el 10-12 % y da la mejor reducción fuera de muestra. Los β mayores solo mejoran dentro de la muestra. Usar varios β (0,5 a 4) sube la 2.ª tarjeta de 33 a 39 % con unas 4 búsquedas más (~10-25 ms). Queda como opción si la UX quiere más alternativas.

## Anexo: archivos

- `scripts/experiments/comparacion_ruteo.py`: el experimento completo, con dependencias PEP 723.
- `config/experiments/od_pairs.yaml`: los 51 pares.
- `data/experiments/comparacion_ruteo/` y `data/experiments/comparacion_ruteo_holdout/` (gitignored): `rutas.csv` (métricas por ruta), `seleccion.csv` (por par y estrategia), `agregados*.csv`, `calidad_grafo.csv`, `latencias.csv`, `resumen.json`, `tablas.md` y `rutas.geojson`, que solo contiene geometrías de rutas OSM/ORS, sin eventos.
- `data/experiments/cache/`: red OSM procesada y respuestas de ORS.
