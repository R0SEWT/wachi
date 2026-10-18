"""Regla de selección (ADR 015): Rápida = más corta; Tranquila = menor exposición dentro del tope."""
from __future__ import annotations

from dataclasses import dataclass

from wachi.config import Settings
from wachi.router import Measure, Route


@dataclass
class Selection:
    fast: Route
    fast_m: Measure
    calm: Route | None
    calm_m: Measure | None
    reduction: float
    detour_m: float


def select(fast: Route, fast_m: Measure, cands: list[tuple[Route, Measure]], cfg: Settings) -> Selection:
    L0 = fast.length_m
    cap = min(L0 * (1 + cfg.cap_pct), L0 + cfg.cap_min * cfg.walk_m_per_min)
    ok = [(r, m) for r, m in cands if r.length_m <= cap + 1e-6 and r.edges != fast.edges]
    if not ok or fast_m.exposure <= 0:
        return Selection(fast, fast_m, None, None, 0.0, 0.0)
    calm, calm_m = min(ok, key=lambda rm: (round(rm[1].exposure, 6), round(rm[0].length_m, 3)))
    if calm_m.exposure > (1 - cfg.gain_min) * fast_m.exposure:
        return Selection(fast, fast_m, None, None, 0.0, 0.0)
    reduction = 1 - calm_m.exposure / fast_m.exposure
    return Selection(fast, fast_m, calm, calm_m, reduction, calm.length_m - L0)
