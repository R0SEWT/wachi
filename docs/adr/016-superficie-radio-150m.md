# ADR 016 — Superficie espacial: robos registrados a 150 m

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- Con H3 res 8 (arista ~460 m) una ruta de 1,4 km cruza unas 3 celdas, y las alternativas cercanas caen en las mismas. En la comparación de 51 pares, con H3 res 8 la tarjeta Tranquila aparece en el 4-6 % de las consultas; con un conteo en radio de 150 m, en el 33 %.
- Fuera de muestra (elegir con 2023-07..11, medir con 2023-12..2024-04), las Tranquilas elegidas con el radio de 150 m reducen la exposición futura en 79-88 % de los pares donde difieren.
- Entre unidades finas, la densidad suavizada a ~150 m es la más estable entre periodos (ρ = 0,88 en la red de Lince, Jesús María, Miraflores y Cercado); la cuadra (0,43) y la esquina (0,47) no lo son. Los robos no están más cerca de las esquinas que un punto al azar de la red (47 % vs 47 % a ≤ 25 m).
- La coordenada de la denuncia está pegada al eje de la calle (mediana 7 m), pero ~1 de cada 5 solo tiene el nombre de la vía y cae en un punto por defecto.
- El riesgo por (unidad, franja horaria) predice peor que el de todo el día: 57 % de los pares (celda res 8, franja) tienen menos de 5 eventos en 10 meses.

## Decisión

1. El riesgo de un punto es el **número de robos/hurtos registrados a ≤ 150 m** (universo de ADR 007-009). El riesgo de una arista del grafo es el promedio en puntos cada 20 m; la exposición de una ruta, `Σ riesgo × km` (ADR 003).
2. Para el costo del grafo (ADR 014) el riesgo se normaliza por el p75 de la red.
3. **Sin franja horaria por lugar.** Si la UX necesita hora, se usa la superficie de todo el día multiplicada por un perfil horario por distrito.
4. H3 res 8 se mantiene solo para el indicador de cobertura (ADR 010) y para agregados.
5. En la UI se habla de **cuadras o tramos**, nunca de esquinas.

## Justificación

- Es la unidad fina más estable y la única que distingue calles paralelas.
- Se explica en una línea: "N robos registrados a menos de 150 m de este tramo".
- Bajo costo: un árbol KD sobre los eventos.

## Consecuencias

- Cambia la estructura del risk store: puntos de eventos (o una grilla fina) en vez de celdas por franja.
- La señal puede reflejar un patrón estable de geocodificación y no solo de delito; no hay forma de separarlo con estos datos.

## Criterios de revisión

- La validación en distritos de baja cobertura (solo se midieron en profundidad cuatro distritos céntricos) contradice estos resultados.
- Mejora la precisión de geocodificación (por ejemplo, con la fuente vigente).
