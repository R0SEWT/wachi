# ADR 013 — Recintos cerrados: no se rutea por dentro

> **Segunda integración (2026-10-07):** ajustado al motor propio (ADR 014). El prototipo de UX pide rutas al backend, no a OSRM.

## Estado
Aceptado (2026-10-17). Implementado en Wachi v2.

## Contexto

- El prototipo de UX (`wachi-ux`) pedía rutas a `router.project-osrm.org/route/v1/foot/...`. Ese servidor demo público solo carga el perfil de auto: para el caso de Elena devuelve la misma ruta con `foot`, `car` y `driving` (3.051 m en 5,8 min). De ahí las vueltas en U y los sentidos únicos.
- ORS `foot-walking` no tiene ese problema, pero una de sus alternativas recorre ~364 m por dentro del Hospital Rebagliati (pasajes internos mapeados en OSM).
- En la comparación de 51 pares, 4 tenían una alternativa de ORS que cruzaba un recinto, y en 3 de ellos la regla de ADR 015 la habría elegido como Tranquila: dentro de un recinto no hay denuncias geocodificadas, así que parece tranquilo. En el grafo propio, excluir esas aristas no alargó ninguna ruta rápida (0 de 51).

## Decisión

1. El prototipo de UX pide sus rutas al backend (`POST /route`). OSRM no se usa.
2. En el grafo propio (ADR 014) se **excluyen las aristas que están dentro de recintos de acceso restringido** de OSM: `amenity=hospital`, `amenity=university`, `landuse=cemetery`, `landuse=military` (lista en config). Se permite el recinto que contiene el origen o el destino.
3. Si se usa el respaldo ORS, se descartan las alternativas que cruzan esos recintos; si todas cruzan, se conserva la que recorre menos metros dentro.

## Justificación

- Sin denuncias dentro de un recinto, el modelo lo vería como el lugar más tranquilo; la regla evita ese artefacto.
- Se explica en una frase y no cambia la regla de selección.

## Consecuencias

- El extracto de polígonos de OSM se versiona junto con el grafo.
- Test con el caso Rebagliati y con un origen dentro de un campus (San Marcos).

## Criterios de revisión

- Aparecen otros tipos de recinto que generen rutas absurdas.
