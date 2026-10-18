# ADR 015 — Regla de selección: menor exposición con tope de desvío

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- ADR 002 ordena rutas por `J = α·distancia + β·riesgo_total` con tres niveles. En la comparación de 51 pares, con las unidades actuales el término de riesgo pesa ~74 veces el de distancia en el nivel SAFE (mediana): en la práctica elige la ruta de menor exposición sin tope, y en el 6 % de los pares se desvía más de lo razonable (máximo +658 m). Los β no tienen unidades que se puedan explicar y cambian con cada versión del risk store.
- La UX muestra a lo sumo dos tarjetas ("Rápida" y "Tranquila") y no muestra puntajes.
- No hay evidencia de cuántos minutos acepta un peatón para evitar robos: el "12-15 %" que cita la UX (Quercia et al. 2014) es cuánto se alargaban las rutas de su algoritmo, no una tolerancia medida.

## Decisión

1. **Rápida** = la ruta más corta (ADR 014, β = 0).
2. **Tranquila** = la ruta de menor exposición (ADR 016) cuyo desvío no supera **min(+15 % de la distancia, +5 min a 5 km/h)**.
3. La tarjeta Tranquila se muestra **solo si reduce la exposición al menos 20 %**; si no, una sola tarjeta.
4. Los tres parámetros (15 %, 5 min, 20 %) van en `config/` y se revisan con uso real.

## Justificación

- Se explica en una frase: "la ruta con menos robos registrados entre las que no te alargan más de 5 minutos".
- Parámetros en unidades humanas, que no dependen de la escala del riesgo.
- La convergencia a una tarjeta (UX) sale de la regla, sin lógica aparte.

## Consecuencias

- La API deja de recibir `safety_level` y devuelve la ruta rápida y, opcionalmente, la tranquila con su desvío y su reducción. Cambia el contrato (`openapi.yaml`, schemas, tests de scoring): requiere change de OpenSpec.
- ADR 003 (exposición como integral discretizada) sigue vigente como forma de medir el riesgo de una ruta.

## Criterios de revisión

- Datos de uso muestran que la gente acepta desvíos distintos.
- La tarjeta Tranquila aparece demasiado poco o demasiado seguido.
