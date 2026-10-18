# ADR 011 — Presentación: dos tarjetas, sin puntajes, beneficio en minutos y cuadras

> **Segunda integración (2026-10-07):** reescrito sobre ADR 014-016. Sigue dependiendo del diseño de UX, aún abierto.

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- La UX (`wachi-ux/UX.md`) muestra a lo sumo dos tarjetas ("Rápida" / "Tranquila"), no muestra puntajes ni la palabra "segura", y explica el beneficio con hitos ("+3 min · evita 2 esquinas con robo").
- ADR 015 ya define cuándo hay una o dos tarjetas. ADR 016 mide el riesgo como robos registrados a ≤ 150 m y muestra que "esquina" no es una unidad que los datos sostengan.
- "Robo/hurto de celular" como modalidad explícita es una fracción mínima del universo (~3,4 mil filas útiles en 24 meses).

## Decisión

1. La API devuelve la ruta Rápida y, si ADR 015 lo permite, la Tranquila, cada una con: duración, distancia, desvío respecto de la rápida, reducción de exposición (%) y los tramos con pocos datos (ADR 010). No devuelve puntajes.
2. El beneficio se expresa en **tiempo o cuadras con robos registrados cerca**, por ejemplo: "+3 min · pasa 6 cuadras menos junto a robos registrados". Nunca "esquinas", nunca "segura", nunca "iluminada" (no hay dato; ver `wachi-tc1.5`).
3. La categoría se nombra como **"robos y hurtos"**, no "robo de celular".
4. Los tramos con pocos datos se marcan siempre, aunque la ruta sea la Rápida.

## Justificación

- Todo lo que dice la UI sale de algo que el modelo calcula.
- Mantiene la promesa de la UX (decidir rápido, sin números abstractos) sin afirmar lo que los datos no sostienen.

## Consecuencias

- El copy del prototipo ("esquinas", "alameda iluminada", "Serenazgo") debe ajustarse o marcarse como ilustrativo.
- La definición exacta de "cuadra con robos cerca" (umbral) se fija al implementar, con el set de pares del experimento.

## Criterios de revisión

- La UX cierra un diseño distinto (por ejemplo, tres opciones o un mapa de calor).
- Aparece una fuente de iluminación por tramo validada.
