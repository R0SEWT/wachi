"""Une grafo, superficie y regla; es lo que la API llama."""
from __future__ import annotations

import logging
import time

from wachi.config import Settings
from wachi.explain import build_response
from wachi.graph import load_or_build
from wachi.router import Router
from wachi.select import select
from wachi.surface import Surface, build_surface, load_events

log = logging.getLogger("wachi")


class RouteError(Exception):
    def __init__(self, code: str, which: str | None = None):
        super().__init__(code)
        self.code, self.which = code, which


class Engine:
    def __init__(self, router: Router, surf: Surface, cfg: Settings):
        self.router, self.surf, self.cfg = router, surf, cfg

    def _snap(self, lat: float, lon: float, which: str) -> int:
        S, W, N, E = self.cfg.bbox
        if not (S <= lat <= N and W <= lon <= E):
            raise RouteError("fuera_de_cobertura")
        node, d = self.router.snap(lat, lon)
        if d > self.cfg.snap_max_m:
            raise RouteError("sin_via_peatonal_cerca", which)
        return node

    def route(self, origin: tuple[float, float], destination: tuple[float, float]) -> dict:
        a = self._snap(*origin, "origin")
        b = self._snap(*destination, "destination")
        blocked = self.router.blocked_for(self.router.allowed_polys(a, b))
        fast = self.router.route(a, b, 0.0, blocked)
        if fast is None:
            raise RouteError("sin_ruta")
        cands = []
        for beta in self.cfg.betas:
            r = self.router.route(a, b, beta, blocked)
            if r is not None:
                cands.append((r, self.router.measure(r)))
        sel = select(fast, self.router.measure(fast), cands, self.cfg)
        return build_response(sel, self.router, self.surf, self.cfg)


def load_engine(cfg: Settings) -> Engine:
    t0 = time.time()
    net = load_or_build(cfg)
    log.info("red: %s", net.stats)
    surf = build_surface(load_events(cfg.events_path, cfg.events_sha256), cfg)
    log.info("eventos: %d, data_as_of=%s, umbral pocos datos=%.1f", surf.n_events, surf.data_as_of,
             surf.low_threshold)
    router = Router(net, surf, cfg)
    log.info("motor listo en %.0fs (p75 S150 = %.2f)", time.time() - t0, router.p75)
    return Engine(router, surf, cfg)
