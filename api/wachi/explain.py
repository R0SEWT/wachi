"""Presentación (ADR 011): beneficio en minutos y cuadras, tramos con pocos datos (ADR 010)."""
from __future__ import annotations

from wachi.config import Settings
from wachi.geo import TO_LL
from wachi.router import Route, Router
from wachi.select import Selection


def blocks_near_robberies(route: Route, router: Router, cfg: Settings) -> int:
    """Cuadras (aristas) de la ruta con ≥ block_min_events robos registrados a 150 m, en promedio."""
    if not route.edges:
        return 0
    events_per_edge = router.rbar[route.edges] * router.p75
    return int((events_per_edge >= cfg.block_min_events).sum())


def low_data_segments(cells: list[str], surf) -> list[list[int]]:
    low = surf.is_low(cells)
    out, start = [], None
    for i, flag in enumerate(low):
        if flag and start is None:
            start = i
        if not flag and start is not None:
            out.append([start, i - 1])
            start = None
    if start is not None:
        out.append([start, len(low) - 1])
    return out


def _geometry(route: Route) -> list[list[float]]:
    lon, lat = TO_LL.transform(route.xy[:, 0], route.xy[:, 1])
    return [[round(float(a), 6), round(float(b), 6)] for a, b in zip(lat, lon)]


def _duration_s(length_m: float, cfg: Settings) -> int:
    return round(length_m / (cfg.walk_kmh * 1000 / 3600))


def build_response(sel: Selection, router: Router, surf, cfg: Settings) -> dict:
    fast = {
        "geometry": _geometry(sel.fast),
        "distance_m": round(sel.fast.length_m),
        "duration_s": _duration_s(sel.fast.length_m, cfg),
        "low_data_segments": low_data_segments(sel.fast_m.cells, surf),
    }
    calm = None
    if sel.calm is not None:
        fewer = blocks_near_robberies(sel.fast, router, cfg) - blocks_near_robberies(sel.calm, router, cfg)
        calm = {
            "geometry": _geometry(sel.calm),
            "distance_m": round(sel.calm.length_m),
            "duration_s": _duration_s(sel.calm.length_m, cfg),
            "detour_m": round(sel.detour_m),
            "detour_s": _duration_s(sel.detour_m, cfg),
            "exposure_reduction_pct": round(100 * sel.reduction),
            "blocks_fewer_near_robberies": max(0, fewer),
            "low_data_segments": low_data_segments(sel.calm_m.cells, surf),
        }
    return {
        "fast": fast,
        "calm": calm,
        "data_as_of": surf.data_as_of,
        "params": {"cap_pct": cfg.cap_pct, "cap_min": cfg.cap_min, "gain_min": cfg.gain_min,
                   "radius_m": cfg.radius_m, "beta": cfg.betas[0]},
    }
